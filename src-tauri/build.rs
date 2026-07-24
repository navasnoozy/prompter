include!("src/command_manifest.rs");

fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(APP_COMMAND_NAMES)),
    )
    .expect("failed to prepare the Prompter Tauri build");
}
