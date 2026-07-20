use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{ErrorKind, Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex, MutexGuard,
    },
};

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Manager, State};

const SETTINGS_FILE_NAME: &str = "settings.json";
const MAX_SETTINGS_BYTES: u64 = 16 * 1024 * 1024;
const ALLOWED_KEYS: [&str; 4] = ["presets", "selectedInstructionId", "theme", "provider"];

static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Default)]
struct SettingsState {
    active_session_id: u64,
    latest_revisions: BTreeMap<String, u64>,
}

#[derive(Default)]
pub(crate) struct SettingsCoordinator {
    state: Mutex<SettingsState>,
}

impl SettingsCoordinator {
    fn lock(&self) -> Result<MutexGuard<'_, SettingsState>, SettingsCommandError> {
        self.state
            .lock()
            .map_err(|_| SettingsCommandError::unavailable("The settings manager is unavailable."))
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SettingsLoadResponse {
    version: u8,
    session_id: u64,
    entries: BTreeMap<String, Value>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum SettingsErrorCode {
    InvalidRequest,
    Unavailable,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SettingsCommandError {
    version: u8,
    code: SettingsErrorCode,
    message: String,
}

impl SettingsCommandError {
    fn invalid_request(message: impl Into<String>) -> Self {
        Self {
            version: 1,
            code: SettingsErrorCode::InvalidRequest,
            message: message.into(),
        }
    }

    fn unavailable(message: impl Into<String>) -> Self {
        Self {
            version: 1,
            code: SettingsErrorCode::Unavailable,
            message: message.into(),
        }
    }
}

#[tauri::command]
pub(crate) fn load_settings(
    app: AppHandle,
    coordinator: State<'_, SettingsCoordinator>,
) -> Result<SettingsLoadResponse, SettingsCommandError> {
    let mut state = coordinator.lock()?;
    // Invalidate the previous page session before any fallible path or disk
    // read. A queued command from that page must not remain authorized when a
    // reload fails and the new page cannot finish booting.
    let session_id = begin_settings_session(&mut state)?;
    let path = settings_path(&app)?;
    let document = read_document(&path)?;

    Ok(SettingsLoadResponse {
        version: 1,
        session_id,
        entries: document
            .into_iter()
            .filter(|(key, _)| is_allowed_key(key))
            .collect(),
    })
}

#[tauri::command]
pub(crate) fn save_settings(
    app: AppHandle,
    coordinator: State<'_, SettingsCoordinator>,
    entries: BTreeMap<String, Value>,
    session_id: u64,
    revision: u64,
) -> Result<(), SettingsCommandError> {
    if session_id == 0
        || revision == 0
        || entries.is_empty()
        || entries.keys().any(|key| !is_allowed_key(key))
    {
        return Err(SettingsCommandError::invalid_request(
            "The settings update contains an invalid session, revision, or key set.",
        ));
    }

    let mut state = coordinator.lock()?;
    ensure_active_session(&state, session_id)?;

    let path = settings_path(&app)?;
    apply_settings_update(&path, &mut state, entries, revision)
}

fn begin_settings_session(state: &mut SettingsState) -> Result<u64, SettingsCommandError> {
    let previous_session_id = std::mem::replace(&mut state.active_session_id, 0);
    state.latest_revisions.clear();
    let session_id = previous_session_id.checked_add(1).ok_or_else(|| {
        SettingsCommandError::unavailable("The settings session could not be started.")
    })?;
    state.active_session_id = session_id;
    Ok(session_id)
}

fn ensure_active_session(
    state: &SettingsState,
    session_id: u64,
) -> Result<(), SettingsCommandError> {
    if session_id == state.active_session_id {
        Ok(())
    } else {
        Err(SettingsCommandError::invalid_request(
            "The settings update belongs to an inactive session.",
        ))
    }
}

fn apply_settings_update(
    path: &Path,
    state: &mut SettingsState,
    entries: BTreeMap<String, Value>,
    revision: u64,
) -> Result<(), SettingsCommandError> {
    let applicable_entries: BTreeMap<_, _> = entries
        .into_iter()
        .filter(|(key, _)| {
            state
                .latest_revisions
                .get(key)
                .is_none_or(|latest| revision > *latest)
        })
        .collect();
    if applicable_entries.is_empty() {
        return Ok(());
    }

    let mut document = read_document(path)?;
    document.extend(applicable_entries.clone());
    write_document_atomically(path, &document)?;
    for key in applicable_entries.keys() {
        state.latest_revisions.insert(key.clone(), revision);
    }
    Ok(())
}

fn is_allowed_key(key: &str) -> bool {
    ALLOWED_KEYS.contains(&key)
}

fn settings_path(app: &AppHandle) -> Result<PathBuf, SettingsCommandError> {
    app.path()
        .app_data_dir()
        .map(|directory| directory.join(SETTINGS_FILE_NAME))
        .map_err(|error| {
            log::warn!(
                target: "prompter::settings",
                "event=settings_path_failed reason={error}"
            );
            SettingsCommandError::unavailable("Prompter could not access its settings folder.")
        })
}

fn read_document(path: &Path) -> Result<BTreeMap<String, Value>, SettingsCommandError> {
    let primary = read_document_file(path);
    match primary {
        Ok(Some(document)) => Ok(document),
        Ok(None) => match read_document_file(&backup_path(path)) {
            Ok(Some(document)) => {
                log::warn!(
                    target: "prompter::settings",
                    "event=settings_recovered source=backup reason=primary_missing"
                );
                Ok(document)
            }
            Ok(None) => Ok(BTreeMap::new()),
            Err(backup_error) => Err(backup_error),
        },
        Err(primary_error) => match read_document_file(&backup_path(path)) {
            Ok(Some(document)) => {
                log::warn!(
                    target: "prompter::settings",
                    "event=settings_recovered source=backup reason=primary_unreadable"
                );
                Ok(document)
            }
            _ => Err(primary_error),
        },
    }
}

fn read_document_file(
    path: &Path,
) -> Result<Option<BTreeMap<String, Value>>, SettingsCommandError> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io_failure("settings_open_failed", error)),
    };

    let advertised_length = file
        .metadata()
        .map_err(|error| io_failure("settings_metadata_failed", error))?
        .len();
    if advertised_length > MAX_SETTINGS_BYTES {
        return Err(SettingsCommandError::unavailable(
            "The settings file is unexpectedly large and was not loaded.",
        ));
    }

    let mut bytes = Vec::with_capacity(advertised_length as usize);
    file.take(MAX_SETTINGS_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| io_failure("settings_read_failed", error))?;
    if bytes.len() as u64 > MAX_SETTINGS_BYTES {
        return Err(SettingsCommandError::unavailable(
            "The settings file is unexpectedly large and was not loaded.",
        ));
    }
    if bytes.is_empty() {
        log::warn!(
            target: "prompter::settings",
            "event=settings_decode_failed reason=empty_document"
        );
        return Err(SettingsCommandError::unavailable(
            "Prompter could not read the settings file because it is damaged.",
        ));
    }

    serde_json::from_slice(&bytes).map(Some).map_err(|error| {
        log::warn!(
            target: "prompter::settings",
            "event=settings_decode_failed reason={error}"
        );
        SettingsCommandError::unavailable(
            "Prompter could not read the settings file because it is damaged.",
        )
    })
}

fn write_document_atomically(
    path: &Path,
    document: &BTreeMap<String, Value>,
) -> Result<(), SettingsCommandError> {
    let bytes = serde_json::to_vec(document).map_err(|error| {
        log::warn!(
            target: "prompter::settings",
            "event=settings_encode_failed reason={error}"
        );
        SettingsCommandError::unavailable("Prompter could not prepare the settings update.")
    })?;
    if bytes.len() as u64 > MAX_SETTINGS_BYTES {
        return Err(SettingsCommandError::invalid_request(
            "The settings update is too large to save safely.",
        ));
    }

    let directory = path.parent().ok_or_else(|| {
        SettingsCommandError::unavailable("Prompter could not locate its settings folder.")
    })?;
    fs::create_dir_all(directory)
        .map_err(|error| io_failure("settings_directory_create_failed", error))?;

    let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temporary_path = directory.join(format!(
        ".{SETTINGS_FILE_NAME}.{}.{}.tmp",
        std::process::id(),
        sequence
    ));
    let backup_temporary_path = directory.join(format!(
        ".{SETTINGS_FILE_NAME}.bak.{}.{}.tmp",
        std::process::id(),
        sequence
    ));

    let write_result = (|| {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }

        let mut file = options
            .open(&temporary_path)
            .map_err(|error| io_failure("settings_temp_open_failed", error))?;
        file.write_all(&bytes)
            .map_err(|error| io_failure("settings_write_failed", error))?;
        file.sync_all()
            .map_err(|error| io_failure("settings_file_sync_failed", error))?;
        drop(file);

        // A recovered write must never replace the last known-good backup with
        // the damaged primary that triggered recovery.
        if matches!(read_document_file(path), Ok(Some(_))) {
            fs::copy(path, &backup_temporary_path)
                .map_err(|error| io_failure("settings_backup_copy_failed", error))?;
            File::open(&backup_temporary_path)
                .and_then(|backup_file| backup_file.sync_all())
                .map_err(|error| io_failure("settings_backup_sync_failed", error))?;
            fs::rename(&backup_temporary_path, backup_path(path))
                .map_err(|error| io_failure("settings_backup_replace_failed", error))?;
        }

        fs::rename(&temporary_path, path)
            .map_err(|error| io_failure("settings_replace_failed", error))?;
        File::open(directory)
            .and_then(|directory_file| directory_file.sync_all())
            .map_err(|error| io_failure("settings_directory_sync_failed", error))?;
        Ok(())
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temporary_path);
        let _ = fs::remove_file(&backup_temporary_path);
    }
    write_result
}

fn backup_path(path: &Path) -> PathBuf {
    path.with_extension("json.bak")
}

fn io_failure(event: &'static str, error: std::io::Error) -> SettingsCommandError {
    log::warn!(target: "prompter::settings", "event={event} reason={error}");
    SettingsCommandError::unavailable("Prompter could not access its settings file.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_key_allowlist_is_exact() {
        for key in ALLOWED_KEYS {
            assert!(is_allowed_key(key));
        }
        assert!(!is_allowed_key("../outside.json"));
        assert!(!is_allowed_key("unknown"));
    }

    #[test]
    fn atomic_document_round_trip_preserves_unknown_future_keys() {
        let directory = std::env::temp_dir().join(format!(
            "prompter-settings-test-{}-{}",
            std::process::id(),
            TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join(SETTINGS_FILE_NAME);
        let mut document = BTreeMap::new();
        document.insert("theme".into(), Value::String("dark".into()));
        document.insert("futureKey".into(), Value::Bool(true));

        write_document_atomically(&path, &document).unwrap();
        assert_eq!(read_document(&path).unwrap(), document);

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn oversized_settings_are_rejected_before_writing() {
        let directory = std::env::temp_dir().join(format!(
            "prompter-settings-oversize-test-{}-{}",
            std::process::id(),
            TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let path = directory.join(SETTINGS_FILE_NAME);
        let mut document = BTreeMap::new();
        document.insert(
            "presets".into(),
            Value::String("x".repeat(MAX_SETTINGS_BYTES as usize)),
        );

        assert!(write_document_atomically(&path, &document).is_err());
        assert!(!path.exists());
    }

    #[test]
    fn damaged_primary_settings_recover_from_the_last_atomic_backup() {
        let directory = std::env::temp_dir().join(format!(
            "prompter-settings-recovery-test-{}-{}",
            std::process::id(),
            TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let path = directory.join(SETTINGS_FILE_NAME);
        let mut previous = BTreeMap::new();
        previous.insert("theme".into(), Value::String("light".into()));
        let mut current = BTreeMap::new();
        current.insert("theme".into(), Value::String("dark".into()));

        write_document_atomically(&path, &previous).unwrap();
        write_document_atomically(&path, &current).unwrap();
        fs::write(&path, b"not json").unwrap();

        let mut recovered = read_document(&path).unwrap();
        assert_eq!(recovered, previous);

        recovered.insert("provider".into(), Value::String("gemini".into()));
        write_document_atomically(&path, &recovered).unwrap();

        assert_eq!(read_document(&path).unwrap(), recovered);
        assert_eq!(
            read_document_file(&backup_path(&path)).unwrap(),
            Some(previous)
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn empty_primary_recovers_from_backup_and_damaged_backup_is_not_suppressed() {
        let directory = std::env::temp_dir().join(format!(
            "prompter-settings-empty-recovery-test-{}-{}",
            std::process::id(),
            TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join(SETTINGS_FILE_NAME);
        let backup = backup_path(&path);
        let expected = BTreeMap::from([("theme".into(), Value::String("dark".into()))]);

        fs::write(&path, b"").unwrap();
        fs::write(&backup, serde_json::to_vec(&expected).unwrap()).unwrap();
        assert_eq!(read_document(&path).unwrap(), expected);

        fs::remove_file(&path).unwrap();
        fs::write(&backup, b"").unwrap();
        assert!(read_document(&path).is_err());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn per_key_revisions_reject_stale_updates_without_dropping_other_keys() {
        let directory = std::env::temp_dir().join(format!(
            "prompter-settings-revision-test-{}-{}",
            std::process::id(),
            TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let path = directory.join(SETTINGS_FILE_NAME);
        let mut state = SettingsState {
            active_session_id: 1,
            latest_revisions: BTreeMap::new(),
        };

        apply_settings_update(
            &path,
            &mut state,
            BTreeMap::from([("selectedInstructionId".into(), Value::String("new".into()))]),
            2,
        )
        .unwrap();
        apply_settings_update(
            &path,
            &mut state,
            BTreeMap::from([
                (
                    "presets".into(),
                    serde_json::json!({"version": 2, "instructions": []}),
                ),
                ("selectedInstructionId".into(), Value::String("old".into())),
            ]),
            1,
        )
        .unwrap();

        let document = read_document(&path).unwrap();
        assert_eq!(document["selectedInstructionId"], "new");
        assert!(document.contains_key("presets"));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn failed_load_invalidates_the_previous_write_session() {
        let directory = std::env::temp_dir().join(format!(
            "prompter-settings-failed-load-test-{}-{}",
            std::process::id(),
            TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join(SETTINGS_FILE_NAME);
        fs::write(&path, b"damaged").unwrap();
        let mut state = SettingsState {
            active_session_id: 7,
            latest_revisions: BTreeMap::from([("theme".into(), 3)]),
        };

        let replacement_session = begin_settings_session(&mut state).unwrap();
        assert!(read_document(&path).is_err());
        assert_eq!(replacement_session, 8);
        assert!(state.latest_revisions.is_empty());
        assert!(ensure_active_session(&state, 7).is_err());
        assert!(ensure_active_session(&state, replacement_session).is_ok());

        fs::remove_dir_all(directory).unwrap();
    }
}
