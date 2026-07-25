use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard},
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State, Webview};

use super::{
    commands::ProviderLifecycle,
    config::Provider,
    error::{ProviderCommandError, ProviderErrorCode},
};
use crate::{platform, MAIN_WINDOW_LABEL};

const PROVIDER_NAVIGATION_EVENT: &str = "prompter://provider-navigation-state";
const PROVIDER_NAVIGATION_CONTRACT_VERSION: u8 = 1;

fn operation_failed(message: impl Into<String>) -> ProviderCommandError {
    ProviderCommandError::new(ProviderErrorCode::WebviewOperationFailed, message)
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProviderNavigationAction {
    Back,
    Forward,
    Reload,
    Stop,
}

impl From<ProviderNavigationAction> for platform::NativeNavigationAction {
    fn from(action: ProviderNavigationAction) -> Self {
        match action {
            ProviderNavigationAction::Back => Self::Back,
            ProviderNavigationAction::Forward => Self::Forward,
            ProviderNavigationAction::Reload => Self::Reload,
            ProviderNavigationAction::Stop => Self::Stop,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderNavigationState {
    version: u8,
    provider: Provider,
    generation: u32,
    revision: u32,
    available: bool,
    can_go_back: bool,
    can_go_forward: bool,
    is_loading: bool,
}

impl ProviderNavigationState {
    fn unavailable(provider: Provider) -> Self {
        Self {
            version: PROVIDER_NAVIGATION_CONTRACT_VERSION,
            provider,
            generation: 0,
            revision: 0,
            available: false,
            can_go_back: false,
            can_go_forward: false,
            is_loading: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StoredNavigationState {
    generation: u32,
    revision: u32,
    available: bool,
    can_go_back: bool,
    can_go_forward: bool,
    is_loading: bool,
}

impl StoredNavigationState {
    fn payload(self, provider: Provider) -> ProviderNavigationState {
        ProviderNavigationState {
            version: PROVIDER_NAVIGATION_CONTRACT_VERSION,
            provider,
            generation: self.generation,
            revision: self.revision,
            available: self.available,
            can_go_back: self.can_go_back,
            can_go_forward: self.can_go_forward,
            is_loading: self.is_loading,
        }
    }
}

#[derive(Default)]
struct NavigationRegistry {
    next_generation: u32,
    states: HashMap<Provider, StoredNavigationState>,
}

#[derive(Default)]
pub(crate) struct ProviderNavigationCoordinator {
    registry: Mutex<NavigationRegistry>,
}

impl ProviderNavigationCoordinator {
    fn lock(&self) -> Result<MutexGuard<'_, NavigationRegistry>, ProviderCommandError> {
        self.registry
            .lock()
            .map_err(|_| operation_failed("The provider navigation manager is unavailable."))
    }

    fn begin_generation(
        &self,
        provider: Provider,
    ) -> Result<ProviderNavigationState, ProviderCommandError> {
        let mut registry = self.lock()?;
        let generation = registry.next_generation.checked_add(1).ok_or_else(|| {
            operation_failed("The provider browser generation limit was reached.")
        })?;
        registry.next_generation = generation;

        let state = StoredNavigationState {
            generation,
            revision: 1,
            available: true,
            can_go_back: false,
            can_go_forward: false,
            // A newly-created WKWebView immediately begins its initial load.
            // KVO replaces this conservative state with the native value.
            is_loading: true,
        };
        registry.states.insert(provider, state);
        Ok(state.payload(provider))
    }

    fn update(
        &self,
        provider: Provider,
        generation: u32,
        snapshot: platform::NativeNavigationSnapshot,
    ) -> Result<Option<ProviderNavigationState>, ProviderCommandError> {
        let mut registry = self.lock()?;
        let Some(current) = registry.states.get_mut(&provider) else {
            return Ok(None);
        };
        if !current.available || current.generation != generation {
            return Ok(None);
        }

        if current.can_go_back == snapshot.can_go_back
            && current.can_go_forward == snapshot.can_go_forward
            && current.is_loading == snapshot.is_loading
        {
            return Ok(None);
        }

        current.revision = current.revision.checked_add(1).ok_or_else(|| {
            operation_failed("The provider browser state revision limit was reached.")
        })?;
        current.can_go_back = snapshot.can_go_back;
        current.can_go_forward = snapshot.can_go_forward;
        current.is_loading = snapshot.is_loading;
        Ok(Some(current.payload(provider)))
    }

    fn invalidate(
        &self,
        provider: Provider,
        expected_generation: u32,
    ) -> Result<Option<ProviderNavigationState>, ProviderCommandError> {
        let mut registry = self.lock()?;
        let Some(current) = registry.states.get_mut(&provider) else {
            return Ok(None);
        };
        if !current.available || current.generation != expected_generation {
            return Ok(None);
        }

        current.revision = current.revision.checked_add(1).ok_or_else(|| {
            operation_failed("The provider browser state revision limit was reached.")
        })?;
        current.available = false;
        current.can_go_back = false;
        current.can_go_forward = false;
        current.is_loading = false;
        Ok(Some(current.payload(provider)))
    }

    fn get(&self, provider: Provider) -> Result<ProviderNavigationState, ProviderCommandError> {
        let registry = self.lock()?;
        Ok(registry.states.get(&provider).copied().map_or_else(
            || ProviderNavigationState::unavailable(provider),
            |state| state.payload(provider),
        ))
    }

    fn is_current(
        &self,
        provider: Provider,
        generation: u32,
    ) -> Result<bool, ProviderCommandError> {
        let registry = self.lock()?;
        Ok(registry
            .states
            .get(&provider)
            .is_some_and(|state| state.available && state.generation == generation))
    }

    fn current_generation(&self, provider: Provider) -> Result<Option<u32>, ProviderCommandError> {
        let registry = self.lock()?;
        Ok(registry
            .states
            .get(&provider)
            .filter(|state| state.available)
            .map(|state| state.generation))
    }
}

fn emit_navigation_state(app: &AppHandle, state: ProviderNavigationState) {
    if let Err(error) = app.emit_to(MAIN_WINDOW_LABEL, PROVIDER_NAVIGATION_EVENT, state) {
        log::warn!(
            target: "prompter::provider",
            "event=navigation_state_emit_failed reason={error}"
        );
    }
}

/// WebKit is the sole source of truth for main-frame loading and history
/// availability. Every snapshot flows through here: the lifecycle uses
/// `is_loading` to hold prompt placement, and the coordinator emits a new
/// revision to the navigation capsule whenever the values actually change.
fn record_native_snapshot(
    app: &AppHandle,
    provider: Provider,
    generation: u32,
    snapshot: platform::NativeNavigationSnapshot,
) {
    let Some(coordinator) = app.try_state::<ProviderNavigationCoordinator>() else {
        log::error!(
            target: "prompter::provider",
            "event=navigation_coordinator_missing"
        );
        return;
    };
    let Some(lifecycle) = app.try_state::<ProviderLifecycle>() else {
        log::error!(
            target: "prompter::provider",
            "event=provider_lifecycle_missing"
        );
        return;
    };

    lifecycle.record_navigation_observation(provider, generation, snapshot.is_loading);
    match coordinator.update(provider, generation, snapshot) {
        Ok(Some(state)) => emit_navigation_state(app, state),
        Ok(None) => {}
        Err(error) => {
            log::error!(
                target: "prompter::provider",
                "event=navigation_state_update_failed reason={}",
                error.message
            );
        }
    }
}

pub(super) async fn register_provider_navigation(
    app: &AppHandle,
    webview: &Webview,
    provider: Provider,
) -> Result<(), ProviderCommandError> {
    let coordinator = app.state::<ProviderNavigationCoordinator>();
    let lifecycle = app.state::<ProviderLifecycle>();
    let initial = coordinator.begin_generation(provider)?;
    if let Err(error) = lifecycle.begin_navigation_generation(provider, initial.generation) {
        let _ = coordinator.invalidate(provider, initial.generation);
        return Err(error);
    }
    emit_navigation_state(app, initial);

    let observer_app = app.clone();
    if let Err(error) =
        platform::observe_provider_navigation(webview, initial.generation, move |snapshot| {
            record_native_snapshot(&observer_app, provider, initial.generation, snapshot);
        })
        .await
    {
        invalidate_provider_navigation(app, provider, initial.generation)?;
        return Err(operation_failed(error));
    }

    Ok(())
}

pub(super) fn invalidate_provider_navigation(
    app: &AppHandle,
    provider: Provider,
    expected_generation: u32,
) -> Result<Option<u32>, ProviderCommandError> {
    let coordinator = app.state::<ProviderNavigationCoordinator>();
    if let Some(state) = coordinator.invalidate(provider, expected_generation)? {
        app.state::<ProviderLifecycle>()
            .invalidate_navigation_generation(provider, state.generation);
        emit_navigation_state(app, state);
        return Ok(Some(state.generation));
    }
    Ok(None)
}

/// Retires whichever generation the coordinator currently holds for a provider.
/// Used by teardown paths that are about to destroy the pane and only need the
/// serializable state to stop describing it.
pub(super) fn invalidate_current_provider_navigation(
    app: &AppHandle,
    provider: Provider,
) -> Result<(), ProviderCommandError> {
    if let Some(generation) = current_provider_navigation_generation(app, provider)? {
        invalidate_provider_navigation(app, provider, generation)?;
    }
    Ok(())
}

pub(super) fn current_provider_navigation_generation(
    app: &AppHandle,
    provider: Provider,
) -> Result<Option<u32>, ProviderCommandError> {
    app.state::<ProviderNavigationCoordinator>()
        .current_generation(provider)
}

#[tauri::command]
pub(crate) fn get_provider_navigation_state(
    coordinator: State<'_, ProviderNavigationCoordinator>,
    provider: Provider,
) -> Result<ProviderNavigationState, ProviderCommandError> {
    coordinator.get(provider)
}

#[tauri::command]
pub(crate) async fn control_provider_navigation(
    app: AppHandle,
    provider: Provider,
    generation: u32,
    action: ProviderNavigationAction,
) -> Result<ProviderNavigationState, ProviderCommandError> {
    if !app
        .state::<ProviderNavigationCoordinator>()
        .is_current(provider, generation)?
    {
        // The provider was closed or recreated after the UI rendered. A stale
        // click must never control the replacement WebView.
        return app.state::<ProviderNavigationCoordinator>().get(provider);
    }

    let Some(webview) = app.get_webview(provider.config().webview_label) else {
        return Err(ProviderCommandError::new(
            ProviderErrorCode::WebviewMissing,
            format!(
                "The {} panel is not available.",
                provider.config().display_name
            ),
        ));
    };

    // Re-checked on the main thread immediately before the typed WebKit call,
    // so a pane closed while this command was queued is never controlled.
    let guard_app = app.clone();
    let snapshot = platform::control_provider_navigation(&webview, action.into(), move || {
        guard_app
            .try_state::<ProviderNavigationCoordinator>()
            .is_some_and(|coordinator| {
                coordinator
                    .is_current(provider, generation)
                    .unwrap_or(false)
            })
    })
    .await
    .map_err(operation_failed)?;

    if let Some(snapshot) = snapshot {
        record_native_snapshot(&app, provider, generation, snapshot);
    }
    app.state::<ProviderNavigationCoordinator>().get(provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_action_contract_is_closed_and_snake_case() {
        let cases = [
            ("\"back\"", ProviderNavigationAction::Back),
            ("\"forward\"", ProviderNavigationAction::Forward),
            ("\"reload\"", ProviderNavigationAction::Reload),
            ("\"stop\"", ProviderNavigationAction::Stop),
        ];
        for (value, expected) in cases {
            assert_eq!(
                serde_json::from_str::<ProviderNavigationAction>(value).unwrap(),
                expected
            );
        }
        for invalid in ["\"close\"", "\"Back\"", "\"\"", "null"] {
            assert!(serde_json::from_str::<ProviderNavigationAction>(invalid).is_err());
        }
    }

    #[test]
    fn every_navigation_action_maps_to_the_matching_native_operation() {
        let cases = [
            (
                ProviderNavigationAction::Back,
                platform::NativeNavigationAction::Back,
            ),
            (
                ProviderNavigationAction::Forward,
                platform::NativeNavigationAction::Forward,
            ),
            (
                ProviderNavigationAction::Reload,
                platform::NativeNavigationAction::Reload,
            ),
            (
                ProviderNavigationAction::Stop,
                platform::NativeNavigationAction::Stop,
            ),
        ];

        for (action, expected) in cases {
            assert_eq!(platform::NativeNavigationAction::from(action), expected);
        }
    }

    #[test]
    fn navigation_payload_is_versioned_and_contains_no_location_data() {
        let coordinator = ProviderNavigationCoordinator::default();
        let state = coordinator.begin_generation(Provider::Chatgpt).unwrap();
        let value = serde_json::to_value(state).unwrap();

        assert_eq!(
            value,
            serde_json::json!({
                "version": 1,
                "provider": "chatgpt",
                "generation": 1,
                "revision": 1,
                "available": true,
                "canGoBack": false,
                "canGoForward": false,
                "isLoading": true
            })
        );
    }

    #[test]
    fn coordinator_deduplicates_and_rejects_stale_generations() {
        let coordinator = ProviderNavigationCoordinator::default();
        let first = coordinator.begin_generation(Provider::Chatgpt).unwrap();
        let loading = platform::NativeNavigationSnapshot {
            can_go_back: false,
            can_go_forward: false,
            is_loading: true,
        };
        assert_eq!(
            coordinator
                .update(Provider::Chatgpt, first.generation, loading)
                .unwrap(),
            None
        );

        let ready = platform::NativeNavigationSnapshot {
            can_go_back: true,
            can_go_forward: false,
            is_loading: false,
        };
        let changed = coordinator
            .update(Provider::Chatgpt, first.generation, ready)
            .unwrap()
            .unwrap();
        assert_eq!(changed.revision, 2);
        assert!(changed.can_go_back);

        let closed = coordinator
            .invalidate(Provider::Chatgpt, first.generation)
            .unwrap()
            .unwrap();
        assert!(!closed.available);
        assert_eq!(
            coordinator
                .update(Provider::Chatgpt, first.generation, loading)
                .unwrap(),
            None
        );

        let replacement = coordinator.begin_generation(Provider::Chatgpt).unwrap();
        assert!(replacement.generation > first.generation);
        assert_eq!(
            coordinator
                .invalidate(Provider::Chatgpt, first.generation)
                .unwrap(),
            None
        );
        assert!(coordinator
            .is_current(Provider::Chatgpt, replacement.generation)
            .unwrap());
        assert_eq!(
            coordinator
                .update(Provider::Chatgpt, first.generation, ready)
                .unwrap(),
            None
        );
    }
}
