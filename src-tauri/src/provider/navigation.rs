use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard},
    time::Duration,
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State, Webview};
use tokio::time::timeout;

use super::{
    commands::{NavigationQuarantineGuard, ProviderLifecycle},
    config::Provider,
    error::{ProviderCommandError, ProviderErrorCode},
};
use crate::{platform, MAIN_WINDOW_LABEL};

const PROVIDER_NAVIGATION_EVENT: &str = "prompter://provider-navigation-state";
const PROVIDER_NAVIGATION_CONTRACT_VERSION: u8 = 1;
const ACTION_WEBKIT_ACK_TIMEOUT: Duration = Duration::from_secs(1);
const TRANSITION_WEBKIT_ACK_TIMEOUT: Duration = Duration::from_secs(2);

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

    fn known_generation(&self, provider: Provider) -> Result<Option<u32>, ProviderCommandError> {
        let registry = self.lock()?;
        Ok(registry.states.get(&provider).map(|state| state.generation))
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

fn record_native_snapshot(
    app: &AppHandle,
    provider: Provider,
    generation: u32,
    snapshot: platform::NativeNavigationSnapshot,
    acknowledged_by_webkit: bool,
    acknowledges_transition: bool,
    transition_token: Option<u64>,
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

    // WebKit is the sole source of truth for loading. Action dispatch and an
    // accepted navigation decision are serialized separately in the lifecycle
    // so same-document history changes can never fabricate a stuck load.
    lifecycle.record_navigation_observation(
        provider,
        generation,
        snapshot.is_loading,
        acknowledged_by_webkit,
        acknowledges_transition,
        transition_token,
    );
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

fn record_native_action_outcome(
    app: &AppHandle,
    provider: Provider,
    generation: u32,
    token: u64,
    outcome: platform::NativeNavigationOutcome,
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

    lifecycle.record_navigation_action_outcome(
        provider,
        generation,
        token,
        outcome.started_navigation,
        outcome.snapshot.is_loading,
    );
    match coordinator.update(provider, generation, outcome.snapshot) {
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
        platform::observe_provider_navigation(webview, initial.generation, move |observation| {
            record_native_snapshot(
                &observer_app,
                provider,
                initial.generation,
                observation.snapshot,
                observation.acknowledges_action,
                observation.acknowledges_transition,
                None,
            );
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

pub(super) fn current_provider_navigation_generation(
    app: &AppHandle,
    provider: Provider,
) -> Result<Option<u32>, ProviderCommandError> {
    app.state::<ProviderNavigationCoordinator>()
        .current_generation(provider)
}

pub(super) fn known_provider_navigation_generation(
    app: &AppHandle,
    provider: Provider,
) -> Result<Option<u32>, ProviderCommandError> {
    app.state::<ProviderNavigationCoordinator>()
        .known_generation(provider)
}

async fn fail_closed_provider_navigation(
    app: &AppHandle,
    provider: Provider,
    generation: u32,
    webview: Option<Webview>,
    guard: NavigationQuarantineGuard,
    reason: &str,
) -> bool {
    let lifecycle = app.state::<ProviderLifecycle>();
    let _creation_guard = lifecycle.lock_creation().await;
    let is_current = match app
        .state::<ProviderNavigationCoordinator>()
        .is_current(provider, generation)
    {
        Ok(is_current) => is_current,
        Err(error) => {
            // If coordinator state cannot be read, classifying the pane as
            // stale would fail open. Quarantine and close the supplied pane;
            // invalidation failure below deliberately preserves MustClose.
            log::error!(
                target: "prompter::provider",
                "event=navigation_fail_closed_currency_check_failed reason={}",
                error.message
            );
            true
        }
    };
    if !is_current {
        // A delayed failure from an old generation may clean up only its own
        // observer. It must never close or clear the replacement page.
        if let Err(error) = platform::detach_provider_navigation_observer_by_label(
            app,
            provider.config().webview_label,
            generation,
        )
        .await
        {
            log::warn!(
                target: "prompter::provider",
                "event=stale_navigation_observer_cleanup_failed reason={error}"
            );
        }
        return false;
    }

    match lifecycle.quarantine_navigation_failure(provider, generation, guard) {
        Ok(true) => {}
        Ok(false) => return false,
        Err(error) => {
            // Failure to inspect lifecycle state is not evidence that the
            // operation became safe. Continue with containment.
            log::error!(
                target: "prompter::provider",
                "event=navigation_fail_closed_guard_check_failed reason={}",
                error.message
            );
        }
    }

    let invalidated = match invalidate_provider_navigation(app, provider, generation) {
        Ok(Some(_)) => true,
        Ok(None) => false,
        Err(error) => {
            log::error!(
                target: "prompter::provider",
                "event=navigation_fail_closed_invalidation_failed reason={}",
                error.message
            );
            false
        }
    };

    let detach_result = if let Some(webview) = webview.as_ref() {
        match platform::detach_provider_navigation_observer(webview, generation).await {
            Ok(true) => Ok(()),
            Ok(false) => {
                platform::detach_provider_navigation_observer_by_label(
                    app,
                    provider.config().webview_label,
                    generation,
                )
                .await
            }
            Err(primary_error) => platform::detach_provider_navigation_observer_by_label(
                app,
                provider.config().webview_label,
                generation,
            )
            .await
            .map_err(|fallback_error| {
                format!("{primary_error} Fallback cleanup also failed: {fallback_error}")
            }),
        }
    } else {
        platform::detach_provider_navigation_observer_by_label(
            app,
            provider.config().webview_label,
            generation,
        )
        .await
    };
    let observer_detached = match detach_result {
        Ok(()) => true,
        Err(error) => {
            log::warn!(
                target: "prompter::provider",
                "event=navigation_fail_closed_detach_failed reason={error}"
            );
            false
        }
    };

    let closed = match webview {
        Some(webview) => match webview.close() {
            Ok(()) => true,
            Err(error) => {
                log::error!(
                    target: "prompter::provider",
                    "event=navigation_fail_closed_close_failed reason={error}"
                );
                false
            }
        },
        None => true,
    };
    if invalidated && observer_detached && closed {
        lifecycle.confirm_closed(provider, Some(generation));
    }
    log::error!(
        target: "prompter::provider",
        "event=navigation_failed_contained invalidated={invalidated} observer_detached={observer_detached} closed={closed} reason={reason}"
    );
    true
}

/// Closes the decision-to-start gap for page-initiated navigation. The
/// lifecycle receives a monotonic transition lease synchronously in
/// `on_navigation`, then this task reads WebKit on the next main-loop turn
/// after the policy callback has returned. Only the matching lease can settle
/// the transition, so an older callback cannot unlock a newer navigation.
pub(super) fn reconcile_accepted_provider_navigation(app: &AppHandle, provider: Provider) {
    let transition = match app
        .state::<ProviderLifecycle>()
        .begin_navigation_transition(provider)
    {
        Ok(Some(transition)) => transition,
        Ok(None) => return,
        Err(error) => {
            let generation = known_provider_navigation_generation(app, provider)
                .ok()
                .flatten();
            app.state::<ProviderLifecycle>()
                .mark_navigation_failure_for_close(provider, generation);
            log::error!(
                target: "prompter::provider",
                "event=navigation_transition_begin_failed reason={}",
                error.message
            );
            return;
        }
    };
    let reconcile_app = app.clone();

    tauri::async_runtime::spawn(async move {
        let Some(webview) = reconcile_app.get_webview(provider.config().webview_label) else {
            fail_closed_provider_navigation(
                &reconcile_app,
                provider,
                transition.generation,
                None,
                NavigationQuarantineGuard::UnacknowledgedTransition {
                    token: transition.token,
                },
                "The accepted provider navigation lost its WebView.",
            )
            .await;
            return;
        };

        match platform::read_provider_navigation_snapshot(&webview).await {
            Ok(snapshot) => {
                record_native_snapshot(
                    &reconcile_app,
                    provider,
                    transition.generation,
                    snapshot,
                    false,
                    false,
                    Some(transition.token),
                );

                if reconcile_app
                    .state::<ProviderLifecycle>()
                    .navigation_transition_is_current(
                        provider,
                        transition.generation,
                        transition.token,
                    )
                {
                    // `on_navigation` runs before WebKit's Allow decision.
                    // A false snapshot on the next main-loop turn is not proof
                    // that the accepted navigation has started. Wait for KVO
                    // (loading=true or URL) or a true loading snapshot.
                    let acknowledged = transition.notification.notified();
                    tokio::pin!(acknowledged);
                    if reconcile_app
                        .state::<ProviderLifecycle>()
                        .navigation_transition_is_current(
                            provider,
                            transition.generation,
                            transition.token,
                        )
                    {
                        let _ = timeout(TRANSITION_WEBKIT_ACK_TIMEOUT, &mut acknowledged).await;
                    }

                    if reconcile_app
                        .state::<ProviderLifecycle>()
                        .navigation_transition_is_current(
                            provider,
                            transition.generation,
                            transition.token,
                        )
                    {
                        fail_closed_provider_navigation(
                            &reconcile_app,
                            provider,
                            transition.generation,
                            Some(webview),
                            NavigationQuarantineGuard::UnacknowledgedTransition {
                                token: transition.token,
                            },
                            "WebKit did not acknowledge an accepted provider navigation.",
                        )
                        .await;
                    }
                }
            }
            Err(error) => {
                fail_closed_provider_navigation(
                    &reconcile_app,
                    provider,
                    transition.generation,
                    Some(webview),
                    NavigationQuarantineGuard::UnacknowledgedTransition {
                        token: transition.token,
                    },
                    &format!("Could not reconcile accepted navigation: {error}"),
                )
                .await;
            }
        }
    });
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
    let Some(action_lease) = app
        .state::<ProviderLifecycle>()
        .begin_navigation_action(provider, generation)?
    else {
        return app.state::<ProviderNavigationCoordinator>().get(provider);
    };
    let action_token = action_lease.token;

    let Some(webview) = app.get_webview(provider.config().webview_label) else {
        fail_closed_provider_navigation(
            &app,
            provider,
            generation,
            None,
            NavigationQuarantineGuard::CurrentGeneration,
            "The provider WebView disappeared before its navigation action.",
        )
        .await;
        return Err(ProviderCommandError::new(
            ProviderErrorCode::WebviewMissing,
            format!(
                "The {} panel is not available.",
                provider.config().display_name
            ),
        ));
    };

    let guard_app = app.clone();
    let completion_app = app.clone();
    let completion_webview = webview.clone();
    let completion_notification = action_lease.notification.clone();
    let outcome = platform::control_provider_navigation(
        &webview,
        action.into(),
        move || {
            let Some(coordinator) = guard_app.try_state::<ProviderNavigationCoordinator>() else {
                return false;
            };
            let Some(lifecycle) = guard_app.try_state::<ProviderLifecycle>() else {
                return false;
            };
            coordinator
                .is_current(provider, generation)
                .unwrap_or(false)
                && lifecycle.claim_navigation_action_for_dispatch(
                    provider,
                    generation,
                    action_token,
                )
        },
        move |outcome| {
            record_native_action_outcome(
                &completion_app,
                provider,
                generation,
                action_token,
                outcome,
            );
            if !outcome.started_navigation {
                return;
            }

            let watchdog_app = completion_app.clone();
            tauri::async_runtime::spawn(async move {
                let acknowledged = completion_notification.notified();
                tokio::pin!(acknowledged);
                if !watchdog_app
                    .state::<ProviderLifecycle>()
                    .navigation_action_is_acknowledged(provider, generation, action_token)
                {
                    let _ = timeout(ACTION_WEBKIT_ACK_TIMEOUT, &mut acknowledged).await;
                }
                fail_closed_provider_navigation(
                    &watchdog_app,
                    provider,
                    generation,
                    Some(completion_webview),
                    NavigationQuarantineGuard::UnacknowledgedAction {
                        token: action_token,
                    },
                    "An abandoned WebKit navigation action was never acknowledged.",
                )
                .await;
            });
        },
    )
    .await;

    let outcome = match outcome {
        Ok(Some(outcome)) => outcome,
        Ok(None) => {
            app.state::<ProviderLifecycle>().finish_navigation_action(
                provider,
                generation,
                action_token,
                None,
            );
            return app.state::<ProviderNavigationCoordinator>().get(provider);
        }
        Err(error) => {
            fail_closed_provider_navigation(
                &app,
                provider,
                generation,
                Some(webview),
                NavigationQuarantineGuard::CurrentGeneration,
                "The typed WebKit navigation operation did not complete reliably.",
            )
            .await;
            return Err(operation_failed(error));
        }
    };

    if outcome.started_navigation
        && !app
            .state::<ProviderLifecycle>()
            .navigation_action_is_acknowledged(provider, generation, action_token)
    {
        // WebKit may return a WKNavigation before its policy/KVO callbacks.
        // Keep the action lease until one of those native signals arrives.
        // Notify stores a permit, and the second state check closes the
        // check-before-wait race.
        let acknowledged = action_lease.notification.notified();
        tokio::pin!(acknowledged);
        if !app
            .state::<ProviderLifecycle>()
            .navigation_action_is_acknowledged(provider, generation, action_token)
        {
            let _ = timeout(ACTION_WEBKIT_ACK_TIMEOUT, &mut acknowledged).await;
        }

        if !app
            .state::<ProviderLifecycle>()
            .navigation_action_is_acknowledged(provider, generation, action_token)
        {
            // Without a policy/KVO signal there is no proof that a delayed
            // provisional navigation cannot begin after this command returns.
            // Dispose the pane instead of releasing prompt placement on an
            // inferred or best-effort cancellation.
            let reason = "WebKit did not acknowledge the navigation action.";
            let contained = fail_closed_provider_navigation(
                &app,
                provider,
                generation,
                Some(webview),
                NavigationQuarantineGuard::UnacknowledgedAction {
                    token: action_token,
                },
                reason,
            )
            .await;
            if contained {
                return Err(operation_failed(
                    "The provider browser could not confirm the navigation. Reopen the provider and try again.",
                ));
            }
            let generation_is_current = app
                .state::<ProviderNavigationCoordinator>()
                .is_current(provider, generation)?;
            let action_was_acknowledged = app
                .state::<ProviderLifecycle>()
                .navigation_action_is_acknowledged(provider, generation, action_token);
            if generation_is_current && action_was_acknowledged {
                // A strong signal won the race while fail-close was waiting
                // for the lifecycle lock. Preserve the healthy pane.
                app.state::<ProviderLifecycle>().finish_navigation_action(
                    provider,
                    generation,
                    action_token,
                    None,
                );
                return app.state::<ProviderNavigationCoordinator>().get(provider);
            }
            // The independent watchdog may already have contained this exact
            // action. Stale/unavailable is not a successful acknowledgment.
            return Err(operation_failed(
                "The provider browser could not confirm the navigation. Reopen the provider and try again.",
            ));
        }
    }

    app.state::<ProviderLifecycle>().finish_navigation_action(
        provider,
        generation,
        action_token,
        None,
    );
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
