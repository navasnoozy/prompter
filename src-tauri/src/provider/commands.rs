use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};

use log::warn;
use serde::Serialize;
use tauri::{
    webview::{NewWindowResponse, WebviewBuilder},
    AppHandle, Manager, Rect, State, Url, WebviewUrl,
};
use tokio::sync::{Mutex as AsyncMutex, MutexGuard as AsyncMutexGuard, Notify};

use super::{
    bridge::{self, is_valid_request_id},
    config::Provider,
    error::{ProviderCommandError, ProviderErrorCode},
    geometry::{self, ProviderBounds},
    navigation,
};
use crate::{platform, prompt::PromptInput, MAIN_WINDOW_LABEL};

const FILL_PROMPT_SOURCE: &str = include_str!("fill_prompt.js");
fn operation_failed(message: impl Into<String>) -> ProviderCommandError {
    ProviderCommandError::new(ProviderErrorCode::WebviewOperationFailed, message)
}

fn retain_first_error(
    retained: &mut Option<ProviderCommandError>,
    result: Result<(), ProviderCommandError>,
) {
    if let Err(error) = result {
        retained.get_or_insert(error);
    }
}

#[derive(Default)]
pub(crate) struct ProviderLifecycle {
    creation_lock: AsyncMutex<()>,
    next_navigation_action_token: AtomicU64,
    next_navigation_transition_token: AtomicU64,
    operation_states: Arc<Mutex<HashMap<Provider, ProviderOperationState>>>,
}

#[derive(Default)]
struct ProviderOperationState {
    request: Option<ProviderRequestState>,
    navigation_generation: Option<u32>,
    navigation_action: Option<NavigationActionState>,
    navigation_loading: bool,
    navigation_transition: Option<NavigationTransitionState>,
}

struct NavigationActionState {
    acknowledged_by_webkit: bool,
    dispatch_completed: bool,
    dispatched_to_webkit: bool,
    notification: Arc<Notify>,
    owner_active: bool,
    started_navigation: bool,
    token: u64,
}

pub(super) struct NavigationActionLease {
    generation: u32,
    pub(super) notification: Arc<Notify>,
    operation_states: Arc<Mutex<HashMap<Provider, ProviderOperationState>>>,
    provider: Provider,
    pub(super) token: u64,
}

struct NavigationTransitionState {
    notification: Arc<Notify>,
    token: u64,
}

pub(super) struct NavigationTransitionLease {
    pub(super) generation: u32,
    pub(super) notification: Arc<Notify>,
    pub(super) token: u64,
}

#[derive(Clone, Copy)]
pub(super) enum NavigationQuarantineGuard {
    CurrentGeneration,
    UnacknowledgedAction { token: u64 },
    UnacknowledgedTransition { token: u64 },
}

enum ProviderRequestState {
    Pending { generation: u32, request_id: String },
    MustClose { generation: Option<u32> },
}

enum NavigationCleanupTarget {
    Known(Option<u32>),
    CoordinatorUnavailable(ProviderCommandError),
}

impl NavigationCleanupTarget {
    fn from_lookup(result: Result<Option<u32>, ProviderCommandError>) -> Self {
        match result {
            Ok(generation) => Self::Known(generation),
            Err(error) => Self::CoordinatorUnavailable(error),
        }
    }

    fn expected_generation(&self) -> Option<u32> {
        match self {
            Self::Known(generation) => *generation,
            Self::CoordinatorUnavailable(_) => None,
        }
    }
}

impl Drop for NavigationActionLease {
    fn drop(&mut self) {
        let Ok(mut states) = self.operation_states.lock() else {
            return;
        };
        let Some(state) = states.get_mut(&self.provider) else {
            return;
        };
        if state.navigation_generation != Some(self.generation) {
            return;
        }
        let Some(action) = state.navigation_action.as_mut() else {
            return;
        };
        if action.token != self.token {
            return;
        }

        action.owner_active = false;
        // A command cancelled before dispatch is harmless. Once WebKit has
        // acknowledged a dispatched action, native loading/transition state
        // remains authoritative. An abandoned, dispatched-but-unacknowledged
        // action stays locked until a later WebKit signal or explicit close.
        if !action.dispatched_to_webkit
            || action.acknowledged_by_webkit
            || (action.dispatch_completed && !action.started_navigation)
        {
            state.navigation_action = None;
        }
    }
}

impl ProviderLifecycle {
    pub(super) async fn lock_creation(&self) -> AsyncMutexGuard<'_, ()> {
        self.creation_lock.lock().await
    }

    fn register_request(
        &self,
        provider: Provider,
        generation: u32,
        request_id: &str,
    ) -> Result<(), ProviderCommandError> {
        if !is_valid_request_id(request_id) {
            return Err(ProviderCommandError::new(
                ProviderErrorCode::InvalidRequest,
                "The prompt request identifier is invalid.",
            ));
        }
        let mut states = self
            .operation_states
            .lock()
            .map_err(|_| operation_failed("The provider request manager is unavailable."))?;
        let state = states.entry(provider).or_default();
        if state.navigation_generation != Some(generation) {
            return Err(ProviderCommandError::new(
                ProviderErrorCode::NavigationBlocked,
                "The provider browser is still initializing. Reopen it and try again.",
            ));
        }
        if state.navigation_action.is_some()
            || state.navigation_loading
            || state.navigation_transition.is_some()
        {
            return Err(ProviderCommandError::new(
                ProviderErrorCode::NavigationBlocked,
                "Wait for the provider page to finish loading before placing the prompt.",
            ));
        }
        match state.request {
            Some(ProviderRequestState::MustClose { .. }) => {
                return Err(operation_failed(
                    "The embedded provider page could not be closed safely. Reopen the provider before placing another prompt.",
                ));
            }
            Some(ProviderRequestState::Pending { .. }) => {
                return Err(ProviderCommandError::new(
                    ProviderErrorCode::NavigationBlocked,
                    "Wait for the current prompt placement to finish.",
                ));
            }
            None => {}
        }
        state.request = Some(ProviderRequestState::Pending {
            generation,
            request_id: request_id.to_string(),
        });
        Ok(())
    }

    pub(super) fn complete_request(
        &self,
        provider: Provider,
        request_id: &str,
    ) -> Result<bool, ProviderCommandError> {
        let mut states = self
            .operation_states
            .lock()
            .map_err(|_| operation_failed("The provider request manager is unavailable."))?;
        let is_current = states.get(&provider).is_some_and(|state| {
            matches!(
                state.request,
                Some(ProviderRequestState::Pending {
                    request_id: ref current,
                    ..
                }) if current == request_id
            )
        });
        if is_current {
            if let Some(state) = states.get_mut(&provider) {
                state.request = None;
            }
        }
        Ok(is_current)
    }

    /// Marks any in-flight fill as requiring page closure. The marker remains
    /// until a close succeeds, so a failed close cannot turn a live routine
    /// into an untracked, merely hidden page.
    fn mark_provider_for_close(&self, provider: Provider) -> bool {
        self.operation_states
            .lock()
            .map(|mut states| {
                let state = states.entry(provider).or_default();
                match &state.request {
                    Some(ProviderRequestState::Pending { generation, .. }) => {
                        state.request = Some(ProviderRequestState::MustClose {
                            generation: Some(*generation),
                        });
                        true
                    }
                    Some(ProviderRequestState::MustClose { .. }) => true,
                    None => false,
                }
            })
            // A poisoned manager is treated as unsafe so callers close the
            // page instead of merely hiding it.
            .unwrap_or(true)
    }

    /// Marks an eval failure for closure only when it still belongs to the
    /// current request. An older command must not cancel a newer fill.
    fn mark_request_for_close(&self, provider: Provider, request_id: &str) -> bool {
        self.operation_states
            .lock()
            .map(|mut states| {
                let state = states.entry(provider).or_default();
                match &state.request {
                    Some(ProviderRequestState::Pending {
                        generation,
                        request_id: current,
                    }) if current == request_id => {
                        state.request = Some(ProviderRequestState::MustClose {
                            generation: Some(*generation),
                        });
                        true
                    }
                    Some(ProviderRequestState::MustClose { .. }) => true,
                    _ => false,
                }
            })
            .unwrap_or(true)
    }

    fn must_close(&self, provider: Provider) -> bool {
        self.operation_states
            .lock()
            .map(|states| {
                states.get(&provider).is_some_and(|state| {
                    matches!(state.request, Some(ProviderRequestState::MustClose { .. }))
                })
            })
            .unwrap_or(true)
    }

    /// Clears only the close marker. A concurrently registered newer request
    /// is deliberately preserved.
    pub(super) fn confirm_closed(&self, provider: Provider, generation: Option<u32>) {
        if let Ok(mut states) = self.operation_states.lock() {
            if let Some(state) = states.get_mut(&provider) {
                let owns_marker = matches!(
                    state.request,
                    Some(ProviderRequestState::MustClose {
                        generation: marker_generation
                    }) if marker_generation == generation
                );
                if owns_marker {
                    state.request = None;
                }
            }
        }
    }

    /// An unregistered replacement can inherit an unavailable coordinator
    /// generation from the previously closed page. While the creation lock is
    /// held, bind its generic close marker to the generation used for cleanup
    /// so a successful recovery close can clear exactly that marker.
    fn bind_unregistered_close_marker(
        &self,
        provider: Provider,
        generation: Option<u32>,
    ) -> Result<(), ProviderCommandError> {
        let mut states = self
            .operation_states
            .lock()
            .map_err(|_| operation_failed("The provider request manager is unavailable."))?;
        let state = states.entry(provider).or_default();
        if matches!(
            state.request,
            Some(ProviderRequestState::MustClose { generation: None })
        ) {
            state.request = Some(ProviderRequestState::MustClose { generation });
        }
        Ok(())
    }

    pub(super) fn begin_navigation_generation(
        &self,
        provider: Provider,
        generation: u32,
    ) -> Result<(), ProviderCommandError> {
        let mut states = self
            .operation_states
            .lock()
            .map_err(|_| operation_failed("The provider request manager is unavailable."))?;
        let state = states.entry(provider).or_default();
        if state.request.is_some() {
            return Err(ProviderCommandError::new(
                ProviderErrorCode::NavigationBlocked,
                "Wait for prompt placement to finish before navigating.",
            ));
        }
        state.navigation_generation = Some(generation);
        if let Some(action) = state.navigation_action.take() {
            action.notification.notify_one();
        }
        state.navigation_loading = true;
        if let Some(transition) = state.navigation_transition.take() {
            transition.notification.notify_one();
        }
        Ok(())
    }

    fn next_navigation_token(
        counter: &AtomicU64,
        exhausted_message: &str,
    ) -> Result<u64, ProviderCommandError> {
        let previous = counter
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .map_err(|_| operation_failed(exhausted_message))?;
        Ok(previous + 1)
    }

    pub(super) fn begin_navigation_action(
        &self,
        provider: Provider,
        generation: u32,
    ) -> Result<Option<NavigationActionLease>, ProviderCommandError> {
        let mut states = self
            .operation_states
            .lock()
            .map_err(|_| operation_failed("The provider request manager is unavailable."))?;
        let state = states.entry(provider).or_default();
        if state.request.is_some() {
            return Err(ProviderCommandError::new(
                ProviderErrorCode::NavigationBlocked,
                "Wait for prompt placement to finish before navigating.",
            ));
        }
        if state.navigation_generation != Some(generation) {
            return Ok(None);
        }
        if state.navigation_action.is_some() {
            return Ok(None);
        }
        let token = Self::next_navigation_token(
            &self.next_navigation_action_token,
            "The provider navigation action limit was reached.",
        )?;
        let notification = Arc::new(Notify::new());
        state.navigation_action = Some(NavigationActionState {
            acknowledged_by_webkit: false,
            dispatch_completed: false,
            dispatched_to_webkit: false,
            notification: Arc::clone(&notification),
            owner_active: true,
            started_navigation: false,
            token,
        });
        Ok(Some(NavigationActionLease {
            generation,
            notification,
            operation_states: Arc::clone(&self.operation_states),
            provider,
            token,
        }))
    }

    pub(super) fn finish_navigation_action(
        &self,
        provider: Provider,
        generation: u32,
        token: u64,
        native_loading: Option<bool>,
    ) {
        if let Ok(mut states) = self.operation_states.lock() {
            let state = states.entry(provider).or_default();
            let owns_action = state.navigation_action.as_ref().is_some_and(|action| {
                state.navigation_generation == Some(generation) && action.token == token
            });
            if owns_action {
                state.navigation_action = None;
                if let Some(is_loading) = native_loading {
                    state.navigation_loading = is_loading;
                }
            }
        }
    }

    /// Claims an action at the last possible point before the typed WebKit
    /// mutation. KVO events that predate dispatch must not acknowledge a new
    /// lease merely because they were delivered while its callback was queued.
    pub(super) fn claim_navigation_action_for_dispatch(
        &self,
        provider: Provider,
        generation: u32,
        token: u64,
    ) -> bool {
        self.operation_states
            .lock()
            .map(|mut states| {
                let Some(state) = states.get_mut(&provider) else {
                    return false;
                };
                if state.navigation_generation != Some(generation) {
                    return false;
                }
                let Some(action) = state.navigation_action.as_mut() else {
                    return false;
                };
                if action.token != token || action.dispatched_to_webkit {
                    return false;
                }
                action.dispatched_to_webkit = true;
                true
            })
            .unwrap_or(false)
    }

    pub(super) fn navigation_action_is_acknowledged(
        &self,
        provider: Provider,
        generation: u32,
        token: u64,
    ) -> bool {
        self.operation_states
            .lock()
            .map(|states| {
                states.get(&provider).is_some_and(|state| {
                    state.navigation_generation == Some(generation)
                        && !matches!(
                            state.request,
                            Some(ProviderRequestState::MustClose {
                                generation: Some(marker_generation)
                            }) if marker_generation == generation
                        )
                        && state.navigation_action.as_ref().is_some_and(|action| {
                            action.token == token
                                && action.dispatched_to_webkit
                                && action.acknowledged_by_webkit
                        })
                })
            })
            .unwrap_or(false)
    }

    pub(super) fn record_navigation_action_outcome(
        &self,
        provider: Provider,
        generation: u32,
        token: u64,
        started_navigation: bool,
        is_loading: bool,
    ) {
        if let Ok(mut states) = self.operation_states.lock() {
            let state = states.entry(provider).or_default();
            if state.navigation_generation != Some(generation) {
                return;
            }
            state.navigation_loading = is_loading;
            let Some(action) = state.navigation_action.as_mut() else {
                return;
            };
            if action.token != token || !action.dispatched_to_webkit {
                return;
            }

            action.dispatch_completed = true;
            action.started_navigation = started_navigation;
            if !started_navigation || is_loading {
                action.acknowledged_by_webkit = true;
                action.notification.notify_one();
            }
            if !action.owner_active && action.acknowledged_by_webkit {
                state.navigation_action = None;
            }
        }
    }

    /// Atomically revalidates a timed-out operation and installs its
    /// generation-correlated close marker. A WebKit acknowledgment cannot
    /// slip between these two decisions.
    pub(super) fn quarantine_navigation_failure(
        &self,
        provider: Provider,
        generation: u32,
        guard: NavigationQuarantineGuard,
    ) -> Result<bool, ProviderCommandError> {
        let mut states = self
            .operation_states
            .lock()
            .map_err(|_| operation_failed("The provider request manager is unavailable."))?;
        let state = states.entry(provider).or_default();
        if state.navigation_generation != Some(generation) {
            return match guard {
                NavigationQuarantineGuard::CurrentGeneration => Err(operation_failed(
                    "The provider navigation managers became inconsistent.",
                )),
                _ => Ok(false),
            };
        }
        let guard_matches = match guard {
            NavigationQuarantineGuard::CurrentGeneration => true,
            NavigationQuarantineGuard::UnacknowledgedAction { token } => {
                state.navigation_action.as_ref().is_some_and(|action| {
                    action.token == token
                        && action.dispatched_to_webkit
                        && action.dispatch_completed
                        && action.started_navigation
                        && !action.acknowledged_by_webkit
                })
            }
            NavigationQuarantineGuard::UnacknowledgedTransition { token } => state
                .navigation_transition
                .as_ref()
                .is_some_and(|transition| transition.token == token),
        };
        if !guard_matches {
            return Ok(false);
        }
        if matches!(
            state.request,
            Some(ProviderRequestState::Pending {
                generation: request_generation,
                ..
            }) if request_generation != generation
        ) {
            return Err(operation_failed(
                "The provider lifecycle generation became inconsistent.",
            ));
        }
        state.request = Some(ProviderRequestState::MustClose {
            generation: Some(generation),
        });
        Ok(true)
    }

    pub(super) fn begin_navigation_transition(
        &self,
        provider: Provider,
    ) -> Result<Option<NavigationTransitionLease>, ProviderCommandError> {
        let mut states = self
            .operation_states
            .lock()
            .map_err(|_| operation_failed("The provider request manager is unavailable."))?;
        let state = states.entry(provider).or_default();
        let Some(generation) = state.navigation_generation else {
            return Ok(None);
        };
        let token = Self::next_navigation_token(
            &self.next_navigation_transition_token,
            "The provider navigation transition limit was reached.",
        )?;
        let notification = Arc::new(Notify::new());
        if let Some(previous) = state
            .navigation_transition
            .replace(NavigationTransitionState {
                notification: Arc::clone(&notification),
                token,
            })
        {
            previous.notification.notify_one();
        }
        if let Some(action) = state.navigation_action.as_mut() {
            if action.dispatched_to_webkit {
                action.acknowledged_by_webkit = true;
                action.notification.notify_one();
            }
        }
        if state
            .navigation_action
            .as_ref()
            .is_some_and(|action| !action.owner_active && action.acknowledged_by_webkit)
        {
            state.navigation_action = None;
        }
        Ok(Some(NavigationTransitionLease {
            generation,
            notification,
            token,
        }))
    }

    #[cfg(test)]
    fn record_navigation_snapshot(
        &self,
        provider: Provider,
        generation: u32,
        is_loading: bool,
        acknowledged_by_webkit: bool,
        transition_token: Option<u64>,
    ) {
        self.record_navigation_observation(
            provider,
            generation,
            is_loading,
            acknowledged_by_webkit,
            acknowledged_by_webkit,
            transition_token,
        );
    }

    pub(super) fn record_navigation_observation(
        &self,
        provider: Provider,
        generation: u32,
        is_loading: bool,
        acknowledged_by_webkit: bool,
        acknowledges_transition: bool,
        transition_token: Option<u64>,
    ) {
        if let Ok(mut states) = self.operation_states.lock() {
            let state = states.entry(provider).or_default();
            if state.navigation_generation == Some(generation) {
                state.navigation_loading = is_loading;
                let settles_transition = match transition_token {
                    Some(expected_token) => {
                        is_loading
                            && state
                                .navigation_transition
                                .as_ref()
                                .is_some_and(|transition| transition.token == expected_token)
                    }
                    None => acknowledges_transition && state.navigation_transition.is_some(),
                };
                if settles_transition {
                    if let Some(transition) = state.navigation_transition.take() {
                        transition.notification.notify_one();
                    }
                }
                if is_loading || acknowledged_by_webkit {
                    if let Some(action) = state.navigation_action.as_mut() {
                        if action.dispatched_to_webkit {
                            action.acknowledged_by_webkit = true;
                            action.notification.notify_one();
                        }
                    }
                    if state
                        .navigation_action
                        .as_ref()
                        .is_some_and(|action| !action.owner_active && action.acknowledged_by_webkit)
                    {
                        state.navigation_action = None;
                    }
                }
            }
        }
    }

    pub(super) fn navigation_transition_is_current(
        &self,
        provider: Provider,
        generation: u32,
        token: u64,
    ) -> bool {
        self.operation_states
            .lock()
            .map(|states| {
                states.get(&provider).is_some_and(|state| {
                    state.navigation_generation == Some(generation)
                        && state
                            .navigation_transition
                            .as_ref()
                            .is_some_and(|transition| transition.token == token)
                })
            })
            .unwrap_or(false)
    }

    pub(super) fn invalidate_navigation_generation(&self, provider: Provider, generation: u32) {
        if let Ok(mut states) = self.operation_states.lock() {
            let state = states.entry(provider).or_default();
            if state.navigation_generation == Some(generation) {
                state.navigation_generation = None;
                if let Some(action) = state.navigation_action.take() {
                    action.notification.notify_one();
                }
                state.navigation_loading = false;
                if let Some(transition) = state.navigation_transition.take() {
                    transition.notification.notify_one();
                }
            }
        }
    }

    pub(super) fn mark_navigation_failure_for_close(
        &self,
        provider: Provider,
        generation: Option<u32>,
    ) {
        if let Ok(mut states) = self.operation_states.lock() {
            let state = states.entry(provider).or_default();
            if matches!(
                state.request,
                Some(ProviderRequestState::Pending {
                    generation: request_generation,
                    ..
                }) if Some(request_generation) != generation
            ) {
                return;
            }
            state.request = Some(ProviderRequestState::MustClose { generation });
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FillScriptInput<'a> {
    provider: &'static str,
    request_id: &'a str,
    display_name: &'static str,
    selectors: &'static [&'static str],
    expected_host: &'static str,
    prompt: &'a str,
}

async fn close_provider_webview(
    app: &AppHandle,
    lifecycle: &ProviderLifecycle,
    provider: Provider,
    webview: &tauri::Webview,
    cleanup_target: NavigationCleanupTarget,
    failure_context: &str,
) -> Result<(), ProviderCommandError> {
    let (expected_generation, coordinator_error) = match cleanup_target {
        NavigationCleanupTarget::Known(generation) => (generation, None),
        NavigationCleanupTarget::CoordinatorUnavailable(error) => (None, Some(error)),
    };
    // Bookkeeping failure must never prevent physical containment. In
    // particular, a poisoned lifecycle mutex is itself classified as unsafe,
    // so still invalidate, detach observers, and close the live WebView before
    // reporting that failure. The close marker is cleared only when every
    // cleanup step succeeds.
    let marker_binding_result =
        lifecycle.bind_unregistered_close_marker(provider, expected_generation);
    // Invalidate the serializable state before scheduling native teardown so
    // stale frontend actions cannot target this generation.
    let invalidation_result = if let Some(generation) = expected_generation {
        navigation::invalidate_provider_navigation(app, provider, generation)
    } else {
        Ok(None)
    };
    let observer_result = if let Some(generation) = expected_generation {
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
        platform::detach_provider_navigation_observer_by_label_any_generation(
            app,
            provider.config().webview_label,
        )
        .await
    };
    let close_result = webview.close();

    if let Err(error) = close_result {
        return Err(operation_failed(format!("{failure_context}: {error}")));
    }

    invalidation_result?;
    observer_result.map_err(operation_failed)?;
    marker_binding_result?;
    if let Some(error) = coordinator_error {
        // Without coordinator state, exact invalidation cannot be proven.
        // Physical containment has completed, but the generic MustClose marker
        // deliberately remains and the original error is surfaced.
        return Err(error);
    }
    lifecycle.confirm_closed(provider, expected_generation);
    Ok(())
}

async fn confirm_missing_provider_closed(
    app: &AppHandle,
    lifecycle: &ProviderLifecycle,
    provider: Provider,
    cleanup_target: NavigationCleanupTarget,
) -> Result<(), ProviderCommandError> {
    let (expected_generation, coordinator_error) = match cleanup_target {
        NavigationCleanupTarget::Known(generation) => (generation, None),
        NavigationCleanupTarget::CoordinatorUnavailable(error) => (None, Some(error)),
    };
    let marker_binding_result =
        lifecycle.bind_unregistered_close_marker(provider, expected_generation);
    let invalidation_result = if let Some(generation) = expected_generation {
        navigation::invalidate_provider_navigation(app, provider, generation)
    } else {
        Ok(None)
    };
    let observer_result = if let Some(generation) = expected_generation {
        platform::detach_provider_navigation_observer_by_label(
            app,
            provider.config().webview_label,
            generation,
        )
        .await
        .map_err(operation_failed)
    } else {
        platform::detach_provider_navigation_observer_by_label_any_generation(
            app,
            provider.config().webview_label,
        )
        .await
        .map_err(operation_failed)
    };

    invalidation_result?;
    observer_result?;
    marker_binding_result?;
    if let Some(error) = coordinator_error {
        return Err(error);
    }
    lifecycle.confirm_closed(provider, expected_generation);
    Ok(())
}

#[tauri::command]
pub(crate) async fn show_provider_webview(
    app: AppHandle,
    lifecycle: State<'_, ProviderLifecycle>,
    provider: Provider,
    bounds: ProviderBounds,
) -> Result<(), ProviderCommandError> {
    let _creation_guard = lifecycle.lock_creation().await;
    let config = provider.config();
    let mut containment_error = None;
    let close_requirements = Provider::ALL.map(|candidate| {
        let must_close = if candidate == provider {
            lifecycle.must_close(candidate)
        } else {
            lifecycle.mark_provider_for_close(candidate)
        };
        (candidate, must_close)
    });

    // Finish every required containment attempt before normal visibility work.
    // One pane's teardown error must not leave another unsafe pane running.
    for (candidate, must_close) in close_requirements {
        if !must_close {
            continue;
        }
        let cleanup_target = NavigationCleanupTarget::from_lookup(
            navigation::known_provider_navigation_generation(&app, candidate),
        );
        if let Some(webview) = app.get_webview(candidate.config().webview_label) {
            retain_first_error(
                &mut containment_error,
                close_provider_webview(
                    &app,
                    &lifecycle,
                    candidate,
                    &webview,
                    cleanup_target,
                    if candidate == provider {
                        "Could not close the unsafe provider page"
                    } else {
                        "Could not close the active provider fill"
                    },
                )
                .await,
            );
        } else {
            retain_first_error(
                &mut containment_error,
                confirm_missing_provider_closed(&app, &lifecycle, candidate, cleanup_target).await,
            );
        }
    }

    if let Some(error) = containment_error {
        return Err(error);
    }

    let window = app.get_window(MAIN_WINDOW_LABEL).ok_or_else(|| {
        ProviderCommandError::new(
            ProviderErrorCode::WindowMissing,
            "The Prompter window was not found.",
        )
    })?;
    let rect = Rect::from(bounds.validate(geometry::content_offset_y(&window))?);

    for (inactive_provider, was_closed) in close_requirements
        .into_iter()
        .filter(|(candidate, _)| *candidate != provider)
    {
        if was_closed {
            continue;
        }
        if let Some(webview) = app.get_webview(inactive_provider.config().webview_label) {
            webview.hide().map_err(|hide_error| {
                operation_failed(format!(
                    "Could not hide the inactive embedded provider: {hide_error}"
                ))
            })?;
        }
    }

    if let Some(webview) = app.get_webview(config.webview_label) {
        let cleanup_target =
            match navigation::current_provider_navigation_generation(&app, provider) {
                Ok(Some(_)) => {
                    platform::apply_provider_corner_radius(&webview).map_err(operation_failed)?;
                    webview.set_bounds(rect).map_err(|error| {
                        operation_failed(format!("Could not resize the embedded browser: {error}"))
                    })?;
                    webview.show().map_err(|error| {
                        operation_failed(format!("Could not show the embedded browser: {error}"))
                    })?;
                    return Ok(());
                }
                Ok(None) => NavigationCleanupTarget::from_lookup(
                    navigation::known_provider_navigation_generation(&app, provider),
                ),
                Err(error) => NavigationCleanupTarget::CoordinatorUnavailable(error),
            };

        // A page without an active navigation generation is not observed and
        // cannot safely accept prompt placement. Close it before recreating.
        lifecycle.mark_navigation_failure_for_close(provider, cleanup_target.expected_generation());
        close_provider_webview(
            &app,
            &lifecycle,
            provider,
            &webview,
            cleanup_target,
            "Could not close the unobserved provider page",
        )
        .await?;
    }

    let external_url = config
        .url
        .parse()
        .map_err(|error| operation_failed(format!("Invalid provider URL: {error}")))?;
    let bridge_app = app.clone();
    let popup_app = app.clone();
    let popup_label = config.webview_label.to_string();

    let builder = WebviewBuilder::new(config.webview_label, WebviewUrl::External(external_url))
        .focused(false)
        .on_navigation(move |url| {
            if url.scheme() == "prompter" {
                bridge::handle_provider_bridge_url(&bridge_app, provider, url);
                return false;
            }

            if url.as_str() == "about:blank" {
                // An accepted navigation invalidates the old correlation. Keep
                // a close requirement instead of simply forgetting it: hash
                // navigation can preserve the JavaScript context.
                bridge_app
                    .state::<ProviderLifecycle>()
                    .mark_provider_for_close(provider);
                navigation::reconcile_accepted_provider_navigation(&bridge_app, provider);
                return true;
            }

            if provider.accepts_navigation_url(url) {
                bridge_app
                    .state::<ProviderLifecycle>()
                    .mark_provider_for_close(provider);
                navigation::reconcile_accepted_provider_navigation(&bridge_app, provider);
                return true;
            }

            // Links the provider page can't display (e.g., external references in
            // AI responses, Terms of Service, documentation links) are handed to the
            // user's default browser instead of being silently swallowed.
            open_url_externally(&bridge_app, url);
            false
        })
        .on_new_window(move |url, _| {
            if provider.accepts_navigation_url(&url) {
                if let Some(webview) = popup_app.get_webview(&popup_label) {
                    if let Err(error) = webview.navigate(url) {
                        warn!(
                            target: "prompter::provider",
                            "event=popup_navigation_failed reason={error}"
                        );
                    }
                }
            } else {
                open_url_externally(&popup_app, &url);
            }
            NewWindowResponse::Deny
        });

    let webview = window
        .add_child(builder, rect.position, rect.size)
        .map_err(|error| {
            operation_failed(format!("Could not embed the provider browser: {error}"))
        })?;
    if let Err(error) = platform::apply_provider_corner_radius(&webview) {
        lifecycle.mark_navigation_failure_for_close(provider, None);
        if let Err(close_error) = close_provider_webview(
            &app,
            &lifecycle,
            provider,
            &webview,
            NavigationCleanupTarget::Known(None),
            "Could not close the provider after native setup failed",
        )
        .await
        {
            return Err(operation_failed(format!(
                "{error} The provider also could not be closed safely: {}",
                close_error.message
            )));
        }
        return Err(operation_failed(error));
    }
    if let Err(error) = navigation::register_provider_navigation(&app, &webview, provider).await {
        let cleanup_target = NavigationCleanupTarget::from_lookup(
            navigation::known_provider_navigation_generation(&app, provider),
        );
        lifecycle.mark_navigation_failure_for_close(provider, cleanup_target.expected_generation());
        if let Err(close_error) = close_provider_webview(
            &app,
            &lifecycle,
            provider,
            &webview,
            cleanup_target,
            "Could not close the provider after navigation setup failed",
        )
        .await
        {
            return Err(operation_failed(format!(
                "{} The provider also could not be closed safely: {}",
                error.message, close_error.message
            )));
        }
        return Err(error);
    }

    Ok(())
}

#[tauri::command]
pub(crate) fn resize_provider_webview(
    app: AppHandle,
    provider: Provider,
    bounds: ProviderBounds,
) -> Result<(), ProviderCommandError> {
    let Some(webview) = app.get_webview(provider.config().webview_label) else {
        return Ok(());
    };
    let window = app.get_window(MAIN_WINDOW_LABEL).ok_or_else(|| {
        ProviderCommandError::new(
            ProviderErrorCode::WindowMissing,
            "The Prompter window was not found.",
        )
    })?;

    webview
        .set_bounds(Rect::from(
            bounds.validate(geometry::content_offset_y(&window))?,
        ))
        .map_err(|error| {
            operation_failed(format!("Could not resize the embedded browser: {error}"))
        })
}

#[tauri::command]
pub(crate) async fn set_provider_visibility(
    app: AppHandle,
    lifecycle: State<'_, ProviderLifecycle>,
    provider: Provider,
    visible: bool,
) -> Result<(), ProviderCommandError> {
    let _creation_guard = lifecycle.lock_creation().await;
    let close_requirements = Provider::ALL.map(|candidate| {
        let selected = visible && candidate == provider;
        let must_close = if selected {
            lifecycle.must_close(candidate)
        } else {
            lifecycle.mark_provider_for_close(candidate)
        };
        (candidate, must_close)
    });
    let mut containment_error = None;

    for (candidate, must_close) in close_requirements {
        if !must_close {
            continue;
        }
        let cleanup_target = NavigationCleanupTarget::from_lookup(
            navigation::known_provider_navigation_generation(&app, candidate),
        );
        if let Some(webview) = app.get_webview(candidate.config().webview_label) {
            retain_first_error(
                &mut containment_error,
                close_provider_webview(
                    &app,
                    &lifecycle,
                    candidate,
                    &webview,
                    cleanup_target,
                    "Could not close the active provider fill",
                )
                .await,
            );
        } else {
            retain_first_error(
                &mut containment_error,
                confirm_missing_provider_closed(&app, &lifecycle, candidate, cleanup_target).await,
            );
        }
    }
    if let Some(error) = containment_error {
        return Err(error);
    }

    // Hide every safe inactive pane before showing the selected one.
    for (candidate, was_closed) in close_requirements {
        if was_closed || (visible && candidate == provider) {
            continue;
        }
        if let Some(webview) = app.get_webview(candidate.config().webview_label) {
            navigation::known_provider_navigation_generation(&app, candidate)?;
            webview.hide().map_err(|error| {
                operation_failed(format!("Could not hide the embedded browser: {error}"))
            })?;
        }
    }

    if visible {
        let selected_was_closed = close_requirements
            .iter()
            .any(|(candidate, was_closed)| *candidate == provider && *was_closed);
        if !selected_was_closed {
            if let Some(webview) = app.get_webview(provider.config().webview_label) {
                navigation::known_provider_navigation_generation(&app, provider)?;
                webview.show().map_err(|error| {
                    operation_failed(format!("Could not update the embedded browser: {error}"))
                })?;
            }
        }
    }

    Ok(())
}

/// Composes the prompt natively and places it into the provider's editor in a
/// single IPC round trip. Never submits; the user presses Send.
#[tauri::command]
pub(crate) async fn place_prompt(
    app: AppHandle,
    lifecycle: State<'_, ProviderLifecycle>,
    provider: Provider,
    composition: PromptInput,
    request_id: String,
) -> Result<(), ProviderCommandError> {
    let _creation_guard = lifecycle.lock_creation().await;
    let prompt = composition.compose()?;
    let config = provider.config();
    let current_generation = navigation::current_provider_navigation_generation(&app, provider)?
        .ok_or_else(|| {
            ProviderCommandError::new(
                ProviderErrorCode::NavigationBlocked,
                format!(
                    "The {} browser is still initializing. Reopen it and try again.",
                    config.display_name
                ),
            )
        })?;
    let webview = app.get_webview(config.webview_label).ok_or_else(|| {
        ProviderCommandError::new(
            ProviderErrorCode::WebviewMissing,
            format!("The {} panel is still loading.", config.display_name),
        )
    })?;
    let current_url = webview.url().map_err(|error| {
        operation_failed(format!(
            "Could not read the {} page: {error}",
            config.display_name
        ))
    })?;

    if !provider.accepts_fill_url(&current_url) {
        return Err(ProviderCommandError::new(
            ProviderErrorCode::WrongHost,
            format!(
                "{} is showing a sign-in or external page. Finish signing in and return to {} before placing the prompt.",
                config.display_name, config.expected_fill_host
            ),
        ));
    }

    let script = provider_fill_script(provider, &request_id, &prompt)?;
    webview
        .show()
        .and_then(|_| webview.set_focus())
        .map_err(|error| {
            operation_failed(format!("Could not focus {}: {error}", config.display_name))
        })?;

    lifecycle.register_request(provider, current_generation, &request_id)?;
    if let Err(eval_error) = webview.eval(script) {
        if lifecycle.mark_request_for_close(provider, &request_id) {
            if let Err(close_error) = close_provider_webview(
                &app,
                &lifecycle,
                provider,
                &webview,
                NavigationCleanupTarget::Known(Some(current_generation)),
                "Could not safely close the provider page",
            )
            .await
            {
                return Err(operation_failed(format!(
                    "Could not place the prompt in {} ({eval_error}) or safely close its page ({}).",
                    config.display_name, close_error.message
                )));
            }
        }
        return Err(operation_failed(format!(
            "Could not place the prompt in {}: {eval_error}",
            config.display_name
        )));
    }

    Ok(())
}

fn provider_fill_script(
    provider: Provider,
    request_id: &str,
    prompt: &str,
) -> Result<String, ProviderCommandError> {
    if !is_valid_request_id(request_id) {
        return Err(ProviderCommandError::new(
            ProviderErrorCode::InvalidRequest,
            "The prompt request identifier is invalid.",
        ));
    }
    let config = provider.config();
    let input = FillScriptInput {
        provider: config.id,
        request_id,
        display_name: config.display_name,
        selectors: config.editor_selectors,
        expected_host: config.expected_fill_host,
        prompt,
    };
    let input_json = serde_json::to_string(&input).map_err(|error| {
        operation_failed(format!("Could not prepare the provider prompt: {error}"))
    })?;

    Ok(format!("void ({FILL_PROMPT_SOURCE})({input_json});"))
}

/// Hands a URL the embedded pane may not display to the user's default
/// browser. Content is never logged; only failure reasons are.
fn open_url_externally(app: &AppHandle, url: &Url) {
    // Allow http:// and https:// to open in the user's default browser.
    // Block javascript:, data:, file:, and other dangerous schemes that
    // could execute code or access local files outside the sandbox.
    if !matches!(url.scheme(), "https" | "http") {
        warn!(
            target: "prompter::provider",
            "event=external_navigation_blocked scheme={}",
            url.scheme()
        );
        return;
    }

    let target = url.to_string();
    let dispatched = app.run_on_main_thread(move || {
        if let Err(open_error) = platform::open_in_default_browser(&target) {
            warn!(
                target: "prompter::provider",
                "event=external_open_failed reason={open_error}"
            );
        }
    });
    if let Err(dispatch_error) = dispatched {
        warn!(
            target: "prompter::provider",
            "event=external_open_dispatch_failed reason={dispatch_error}"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{provider_fill_script, NavigationQuarantineGuard, Provider, ProviderLifecycle};

    #[test]
    fn fill_script_escapes_input_and_never_submits() {
        let script = provider_fill_script(
            Provider::Chatgpt,
            "request-1",
            "A quote: \"hello\"\nA slash: \\",
        )
        .unwrap();

        assert!(script.contains("#prompt-textarea"));
        assert!(script.contains("request-1"));
        assert!(script.contains("\\\"hello\\\""));
        assert!(script.contains("\\nA slash: \\\\"));
        assert!(!script.contains("requestSubmit"));
        assert!(!script.contains(".submit("));
        assert!(!script.contains("KeyboardEvent"));
        assert!(!script.contains("send-button"));
    }

    #[test]
    fn fill_script_rejects_invalid_request_ids() {
        assert!(provider_fill_script(Provider::Chatgpt, "", "prompt").is_err());
        assert!(provider_fill_script(Provider::Chatgpt, "bad\nid", "prompt").is_err());
    }

    #[test]
    fn cancellation_requires_a_confirmed_close_before_reuse() {
        let lifecycle = ProviderLifecycle::default();
        lifecycle
            .begin_navigation_generation(Provider::Chatgpt, 7)
            .unwrap();
        lifecycle.record_navigation_snapshot(Provider::Chatgpt, 7, false, true, None);

        assert!(!lifecycle.mark_provider_for_close(Provider::Chatgpt));
        lifecycle
            .register_request(Provider::Chatgpt, 7, "request-1")
            .unwrap();
        assert!(lifecycle.mark_provider_for_close(Provider::Chatgpt));
        assert!(lifecycle.mark_provider_for_close(Provider::Chatgpt));
        assert!(lifecycle
            .register_request(Provider::Chatgpt, 7, "request-2")
            .is_err());

        lifecycle.confirm_closed(Provider::Chatgpt, Some(6));
        assert!(lifecycle
            .register_request(Provider::Chatgpt, 7, "request-2")
            .is_err());
        lifecycle.confirm_closed(Provider::Chatgpt, Some(7));
        lifecycle
            .register_request(Provider::Chatgpt, 7, "request-2")
            .unwrap();
    }

    #[test]
    fn unregistered_close_marker_binds_to_recovery_generation() {
        let lifecycle = ProviderLifecycle::default();
        lifecycle.mark_navigation_failure_for_close(Provider::Chatgpt, None);
        assert!(lifecycle.must_close(Provider::Chatgpt));

        lifecycle
            .bind_unregistered_close_marker(Provider::Chatgpt, Some(4))
            .unwrap();
        lifecycle.confirm_closed(Provider::Chatgpt, Some(4));
        assert!(!lifecycle.must_close(Provider::Chatgpt));
    }

    #[test]
    fn concurrent_request_is_rejected_and_an_old_failure_cannot_cancel_the_next_request() {
        let lifecycle = ProviderLifecycle::default();
        lifecycle
            .begin_navigation_generation(Provider::Chatgpt, 7)
            .unwrap();
        lifecycle.record_navigation_snapshot(Provider::Chatgpt, 7, false, true, None);
        lifecycle
            .register_request(Provider::Chatgpt, 7, "request-1")
            .unwrap();
        assert!(lifecycle
            .register_request(Provider::Chatgpt, 7, "request-2")
            .is_err());
        assert!(lifecycle
            .complete_request(Provider::Chatgpt, "request-1")
            .unwrap());
        lifecycle
            .register_request(Provider::Chatgpt, 7, "request-2")
            .unwrap();

        assert!(!lifecycle.mark_request_for_close(Provider::Chatgpt, "request-1"));
        assert!(lifecycle
            .complete_request(Provider::Chatgpt, "request-2")
            .unwrap());
    }

    #[test]
    fn navigation_style_invalidation_requires_close_and_rejects_stale_completion() {
        let lifecycle = ProviderLifecycle::default();
        lifecycle
            .begin_navigation_generation(Provider::Chatgpt, 7)
            .unwrap();
        lifecycle.record_navigation_snapshot(Provider::Chatgpt, 7, false, true, None);
        lifecycle
            .register_request(Provider::Chatgpt, 7, "request-1")
            .unwrap();
        assert!(lifecycle.mark_provider_for_close(Provider::Chatgpt));
        assert!(!lifecycle
            .complete_request(Provider::Chatgpt, "request-1")
            .unwrap());
        assert!(lifecycle.must_close(Provider::Chatgpt));
    }

    #[test]
    fn navigation_and_prompt_placement_are_mutually_exclusive() {
        let lifecycle = ProviderLifecycle::default();
        lifecycle
            .begin_navigation_generation(Provider::Chatgpt, 7)
            .unwrap();

        let loading_error = lifecycle
            .register_request(Provider::Chatgpt, 7, "request-1")
            .unwrap_err();
        assert_eq!(
            loading_error.code,
            super::ProviderErrorCode::NavigationBlocked
        );

        lifecycle.record_navigation_snapshot(Provider::Chatgpt, 7, false, true, None);
        lifecycle
            .register_request(Provider::Chatgpt, 7, "request-1")
            .unwrap();
        let action_error = lifecycle
            .begin_navigation_action(Provider::Chatgpt, 7)
            .err()
            .unwrap();
        assert_eq!(
            action_error.code,
            super::ProviderErrorCode::NavigationBlocked
        );

        assert!(lifecycle
            .complete_request(Provider::Chatgpt, "request-1")
            .unwrap());
        assert!(lifecycle
            .begin_navigation_action(Provider::Chatgpt, 7)
            .unwrap()
            .is_some());
    }

    #[test]
    fn navigation_actions_are_serialized_until_native_acknowledgement() {
        let lifecycle = ProviderLifecycle::default();
        lifecycle
            .begin_navigation_generation(Provider::Chatgpt, 7)
            .unwrap();
        lifecycle.record_navigation_snapshot(Provider::Chatgpt, 7, false, true, None);

        let action = lifecycle
            .begin_navigation_action(Provider::Chatgpt, 7)
            .unwrap()
            .unwrap();
        assert!(lifecycle
            .begin_navigation_action(Provider::Chatgpt, 7)
            .unwrap()
            .is_none());
        assert!(lifecycle
            .register_request(Provider::Chatgpt, 7, "request-1")
            .is_err());

        lifecycle.finish_navigation_action(Provider::Chatgpt, 7, action.token, Some(false));
        lifecycle
            .register_request(Provider::Chatgpt, 7, "request-1")
            .unwrap();
    }

    #[test]
    fn only_post_dispatch_webkit_signals_can_acknowledge_an_action() {
        let lifecycle = ProviderLifecycle::default();
        lifecycle
            .begin_navigation_generation(Provider::Chatgpt, 7)
            .unwrap();
        lifecycle.record_navigation_snapshot(Provider::Chatgpt, 7, false, true, None);
        let action = lifecycle
            .begin_navigation_action(Provider::Chatgpt, 7)
            .unwrap()
            .unwrap();

        // A KVO callback queued before this action cannot acknowledge it while
        // its main-thread mutation is still waiting to be dispatched.
        lifecycle.record_navigation_snapshot(Provider::Chatgpt, 7, false, true, None);
        assert!(lifecycle
            .register_request(Provider::Chatgpt, 7, "request-1")
            .is_err());
        assert!(!lifecycle.navigation_action_is_acknowledged(Provider::Chatgpt, 7, action.token));

        assert!(lifecycle.claim_navigation_action_for_dispatch(Provider::Chatgpt, 7, action.token));
        // The immediate post-dispatch read is not proof that WebKit accepted
        // the action. A false loading snapshot cannot unlock placement until
        // a policy/KVO signal acknowledges the dispatched action.
        lifecycle.record_navigation_snapshot(Provider::Chatgpt, 7, false, false, None);
        assert!(!lifecycle.navigation_action_is_acknowledged(Provider::Chatgpt, 7, action.token));

        lifecycle.record_navigation_snapshot(Provider::Chatgpt, 7, false, true, None);
        assert!(lifecycle.navigation_action_is_acknowledged(Provider::Chatgpt, 7, action.token));
        lifecycle.finish_navigation_action(Provider::Chatgpt, 7, action.token, None);
        lifecycle
            .register_request(Provider::Chatgpt, 7, "request-1")
            .unwrap();
    }

    #[test]
    fn false_immediate_snapshot_cannot_settle_an_accepted_navigation() {
        let lifecycle = ProviderLifecycle::default();
        lifecycle
            .begin_navigation_generation(Provider::Chatgpt, 7)
            .unwrap();
        lifecycle.record_navigation_snapshot(Provider::Chatgpt, 7, false, true, None);

        let transition = lifecycle
            .begin_navigation_transition(Provider::Chatgpt)
            .unwrap()
            .unwrap();
        assert_eq!(transition.generation, 7);
        assert!(lifecycle
            .register_request(Provider::Chatgpt, 7, "request-1")
            .is_err());

        // This is the next-main-queue snapshot taken after the policy callback
        // returns. False is not a causally post-policy start signal.
        lifecycle.record_navigation_snapshot(
            Provider::Chatgpt,
            7,
            false,
            false,
            Some(transition.token),
        );
        assert!(lifecycle.navigation_transition_is_current(Provider::Chatgpt, 7, transition.token));
        assert!(lifecycle
            .register_request(Provider::Chatgpt, 7, "request-1")
            .is_err());

        // A later KVO/URL signal settles same-document navigation even when
        // WebKit truthfully remains non-loading.
        lifecycle.record_navigation_snapshot(Provider::Chatgpt, 7, false, true, None);
        lifecycle
            .register_request(Provider::Chatgpt, 7, "request-1")
            .unwrap();
    }

    #[test]
    fn stale_action_completion_cannot_unlock_a_newer_action() {
        let lifecycle = ProviderLifecycle::default();
        lifecycle
            .begin_navigation_generation(Provider::Chatgpt, 7)
            .unwrap();
        lifecycle.record_navigation_snapshot(Provider::Chatgpt, 7, false, true, None);

        let first = lifecycle
            .begin_navigation_action(Provider::Chatgpt, 7)
            .unwrap()
            .unwrap();
        lifecycle.finish_navigation_action(Provider::Chatgpt, 7, first.token, None);
        let second = lifecycle
            .begin_navigation_action(Provider::Chatgpt, 7)
            .unwrap()
            .unwrap();

        lifecycle.finish_navigation_action(Provider::Chatgpt, 7, first.token, Some(false));
        assert!(lifecycle
            .register_request(Provider::Chatgpt, 7, "request-1")
            .is_err());

        lifecycle.finish_navigation_action(Provider::Chatgpt, 7, second.token, None);
        lifecycle
            .register_request(Provider::Chatgpt, 7, "request-1")
            .unwrap();
    }

    #[test]
    fn stale_transition_reconciliation_cannot_unlock_a_newer_transition() {
        let lifecycle = ProviderLifecycle::default();
        lifecycle
            .begin_navigation_generation(Provider::Chatgpt, 7)
            .unwrap();
        lifecycle.record_navigation_snapshot(Provider::Chatgpt, 7, false, true, None);

        let first = lifecycle
            .begin_navigation_transition(Provider::Chatgpt)
            .unwrap()
            .unwrap();
        let second = lifecycle
            .begin_navigation_transition(Provider::Chatgpt)
            .unwrap()
            .unwrap();
        lifecycle.record_navigation_snapshot(Provider::Chatgpt, 7, true, false, Some(first.token));
        assert!(lifecycle.navigation_transition_is_current(Provider::Chatgpt, 7, second.token));
        assert!(lifecycle
            .register_request(Provider::Chatgpt, 7, "request-1")
            .is_err());

        lifecycle.record_navigation_snapshot(Provider::Chatgpt, 7, true, false, Some(second.token));
        assert!(!lifecycle.navigation_transition_is_current(Provider::Chatgpt, 7, second.token));
        lifecycle.record_navigation_snapshot(Provider::Chatgpt, 7, false, true, None);
        lifecycle
            .register_request(Provider::Chatgpt, 7, "request-1")
            .unwrap();
    }

    #[test]
    fn weak_trailing_kvo_cannot_settle_the_current_transition() {
        let lifecycle = ProviderLifecycle::default();
        lifecycle
            .begin_navigation_generation(Provider::Chatgpt, 7)
            .unwrap();
        lifecycle.record_navigation_snapshot(Provider::Chatgpt, 7, false, true, None);
        let transition = lifecycle
            .begin_navigation_transition(Provider::Chatgpt)
            .unwrap()
            .unwrap();

        // A canGoBack/canGoForward/loading=false callback may trail an older
        // navigation. It updates the snapshot but is not a causal start signal.
        lifecycle.record_navigation_observation(Provider::Chatgpt, 7, false, false, false, None);
        assert!(lifecycle.navigation_transition_is_current(Provider::Chatgpt, 7, transition.token));

        // URL KVO or loading=true is the strong post-policy signal.
        lifecycle.record_navigation_observation(Provider::Chatgpt, 7, false, true, true, None);
        assert!(!lifecycle.navigation_transition_is_current(
            Provider::Chatgpt,
            7,
            transition.token
        ));
    }

    #[test]
    fn strong_signal_winning_the_race_prevents_transition_quarantine() {
        let lifecycle = ProviderLifecycle::default();
        lifecycle
            .begin_navigation_generation(Provider::Chatgpt, 7)
            .unwrap();
        lifecycle.record_navigation_snapshot(Provider::Chatgpt, 7, false, true, None);
        let transition = lifecycle
            .begin_navigation_transition(Provider::Chatgpt)
            .unwrap()
            .unwrap();

        lifecycle.record_navigation_observation(Provider::Chatgpt, 7, false, true, true, None);
        assert!(!lifecycle
            .quarantine_navigation_failure(
                Provider::Chatgpt,
                7,
                NavigationQuarantineGuard::UnacknowledgedTransition {
                    token: transition.token,
                },
            )
            .unwrap());
        assert!(!lifecycle.must_close(Provider::Chatgpt));
    }

    #[test]
    fn invalidation_clears_only_the_matching_generation_locks() {
        let lifecycle = ProviderLifecycle::default();
        lifecycle
            .begin_navigation_generation(Provider::Chatgpt, 7)
            .unwrap();
        lifecycle.record_navigation_snapshot(Provider::Chatgpt, 7, false, true, None);

        assert!(lifecycle
            .begin_navigation_action(Provider::Chatgpt, 6)
            .unwrap()
            .is_none());
        assert!(lifecycle
            .begin_navigation_action(Provider::Chatgpt, 7)
            .unwrap()
            .is_some());
        assert!(lifecycle
            .begin_navigation_transition(Provider::Chatgpt)
            .unwrap()
            .is_some());

        lifecycle.invalidate_navigation_generation(Provider::Chatgpt, 6);
        assert!(lifecycle
            .register_request(Provider::Chatgpt, 7, "request-1")
            .is_err());
        lifecycle.invalidate_navigation_generation(Provider::Chatgpt, 7);
        assert!(lifecycle
            .register_request(Provider::Chatgpt, 7, "request-1")
            .is_err());
        lifecycle
            .begin_navigation_generation(Provider::Chatgpt, 8)
            .unwrap();
        lifecycle.record_navigation_snapshot(Provider::Chatgpt, 8, false, true, None);
        lifecycle
            .register_request(Provider::Chatgpt, 8, "request-1")
            .unwrap();
        assert!(lifecycle
            .complete_request(Provider::Chatgpt, "request-1")
            .unwrap());
        assert!(lifecycle
            .begin_navigation_action(Provider::Chatgpt, 7)
            .unwrap()
            .is_none());
    }

    #[test]
    fn cancelled_action_lease_is_fail_closed_until_webkit_acknowledges_dispatch() {
        let lifecycle = ProviderLifecycle::default();
        lifecycle
            .begin_navigation_generation(Provider::Chatgpt, 7)
            .unwrap();
        lifecycle.record_navigation_snapshot(Provider::Chatgpt, 7, false, true, None);

        let action = lifecycle
            .begin_navigation_action(Provider::Chatgpt, 7)
            .unwrap()
            .unwrap();
        assert!(lifecycle.claim_navigation_action_for_dispatch(Provider::Chatgpt, 7, action.token));
        drop(action);

        assert!(lifecycle
            .register_request(Provider::Chatgpt, 7, "request-1")
            .is_err());
        lifecycle.record_navigation_snapshot(Provider::Chatgpt, 7, false, true, None);
        lifecycle
            .register_request(Provider::Chatgpt, 7, "request-1")
            .unwrap();
    }

    #[test]
    fn callback_side_noop_outcome_releases_an_abandoned_action_lease() {
        let lifecycle = ProviderLifecycle::default();
        lifecycle
            .begin_navigation_generation(Provider::Chatgpt, 7)
            .unwrap();
        lifecycle.record_navigation_snapshot(Provider::Chatgpt, 7, false, true, None);

        let action = lifecycle
            .begin_navigation_action(Provider::Chatgpt, 7)
            .unwrap()
            .unwrap();
        let token = action.token;
        assert!(lifecycle.claim_navigation_action_for_dispatch(Provider::Chatgpt, 7, token));
        drop(action);
        lifecycle.record_navigation_action_outcome(Provider::Chatgpt, 7, token, false, false);

        lifecycle
            .register_request(Provider::Chatgpt, 7, "request-1")
            .unwrap();
    }

    #[test]
    fn quarantine_winning_the_race_is_terminal_even_after_a_late_strong_signal() {
        let lifecycle = ProviderLifecycle::default();
        lifecycle
            .begin_navigation_generation(Provider::Chatgpt, 7)
            .unwrap();
        lifecycle.record_navigation_snapshot(Provider::Chatgpt, 7, false, true, None);
        let action = lifecycle
            .begin_navigation_action(Provider::Chatgpt, 7)
            .unwrap()
            .unwrap();
        assert!(lifecycle.claim_navigation_action_for_dispatch(Provider::Chatgpt, 7, action.token));
        lifecycle.record_navigation_action_outcome(Provider::Chatgpt, 7, action.token, true, false);
        assert!(lifecycle
            .quarantine_navigation_failure(
                Provider::Chatgpt,
                7,
                NavigationQuarantineGuard::UnacknowledgedAction {
                    token: action.token,
                },
            )
            .unwrap());

        lifecycle.record_navigation_observation(Provider::Chatgpt, 7, false, true, true, None);
        assert!(!lifecycle.navigation_action_is_acknowledged(Provider::Chatgpt, 7, action.token));
    }
}
