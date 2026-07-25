use std::{
    cell::RefCell,
    collections::HashMap,
    ffi::c_void,
    ptr::null_mut,
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
    time::Duration,
};

use objc2::{
    define_class, msg_send,
    rc::Retained,
    runtime::{AnyObject, NSObject, NSObjectProtocol},
    DefinedClass, MainThreadMarker, MainThreadOnly, Message,
};
use objc2_foundation::{
    ns_string, NSDictionary, NSKeyValueChangeKey, NSKeyValueObservingOptions,
    NSObjectNSKeyValueObserverRegistration, NSString,
};
use objc2_web_kit::WKWebView;
use tauri::{AppHandle, Webview};
use tokio::{sync::oneshot, time::timeout};

const MAIN_THREAD_OPERATION_TIMEOUT: Duration = Duration::from_secs(5);
const OPERATION_PENDING: u8 = 0;
const OPERATION_CLAIMED: u8 = 1;
const OPERATION_CANCELLED: u8 = 2;

struct PendingMutationCancellation {
    claim: Arc<AtomicU8>,
}

impl Drop for PendingMutationCancellation {
    fn drop(&mut self) {
        // Cancels a callback that is still queued if its owning async command
        // is dropped. A callback already claimed on the main thread is
        // synchronous and cannot be interrupted between its guard and typed
        // WebKit mutation.
        let _ = self.claim.compare_exchange(
            OPERATION_PENDING,
            OPERATION_CANCELLED,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct NativeNavigationSnapshot {
    pub(crate) can_go_back: bool,
    pub(crate) can_go_forward: bool,
    pub(crate) is_loading: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NativeNavigationAction {
    Back,
    Forward,
    Reload,
    Stop,
}

struct NavigationObserverIvars {
    webview: Retained<WKWebView>,
    handler: Box<dyn Fn(NativeNavigationSnapshot)>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements. The observer is
    // main-thread-only because it retains and reads a WKWebView.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "PrompterProviderNavigationObserver"]
    #[ivars = NavigationObserverIvars]
    struct NavigationObserver;

    impl NavigationObserver {
        #[unsafe(method(observeValueForKeyPath:ofObject:change:context:))]
        fn observe_value_for_key_path(
            &self,
            _key_path: Option<&NSString>,
            _object: Option<&AnyObject>,
            _change: Option<&NSDictionary<NSKeyValueChangeKey, AnyObject>>,
            _context: *mut c_void,
        ) {
            self.publish_snapshot();
        }
    }

    unsafe impl NSObjectProtocol for NavigationObserver {}
);

impl NavigationObserver {
    fn new(
        webview: Retained<WKWebView>,
        handler: Box<dyn Fn(NativeNavigationSnapshot)>,
        main_thread: MainThreadMarker,
    ) -> Retained<Self> {
        let observer =
            Self::alloc(main_thread).set_ivars(NavigationObserverIvars { webview, handler });
        // SAFETY: NSObject's init signature is stable and the ivars were set.
        let observer: Retained<Self> = unsafe { msg_send![super(observer), init] };
        let options = NSKeyValueObservingOptions::New;

        // SAFETY: The observer is retained in OBSERVERS until all four
        // registrations are removed by its Drop implementation.
        unsafe {
            observer
                .ivars()
                .webview
                .addObserver_forKeyPath_options_context(
                    &observer,
                    ns_string!("loading"),
                    options,
                    null_mut(),
                );
            observer
                .ivars()
                .webview
                .addObserver_forKeyPath_options_context(
                    &observer,
                    ns_string!("canGoBack"),
                    options,
                    null_mut(),
                );
            observer
                .ivars()
                .webview
                .addObserver_forKeyPath_options_context(
                    &observer,
                    ns_string!("canGoForward"),
                    options,
                    null_mut(),
                );
            // URL is observed only as a change signal. Its value is never
            // read, serialized, emitted, or logged. This covers same-document
            // history changes where loading and history availability can stay
            // unchanged.
            observer
                .ivars()
                .webview
                .addObserver_forKeyPath_options_context(
                    &observer,
                    ns_string!("URL"),
                    options,
                    null_mut(),
                );
        }

        // Publish one explicit baseline after all registrations are active.
        observer.publish_snapshot();
        observer
    }

    fn publish_snapshot(&self) {
        (self.ivars().handler)(snapshot(&self.ivars().webview));
    }
}

impl Drop for NavigationObserver {
    fn drop(&mut self) {
        // SAFETY: These exact registrations are installed in new, and Drop
        // runs on the main thread before the retained WKWebView is released.
        unsafe {
            self.ivars()
                .webview
                .removeObserver_forKeyPath(self, ns_string!("loading"));
            self.ivars()
                .webview
                .removeObserver_forKeyPath(self, ns_string!("canGoBack"));
            self.ivars()
                .webview
                .removeObserver_forKeyPath(self, ns_string!("canGoForward"));
            self.ivars()
                .webview
                .removeObserver_forKeyPath(self, ns_string!("URL"));
        }
    }
}

struct ObserverRegistration {
    generation: u32,
    native_identity: usize,
    _observer: Retained<NavigationObserver>,
}

thread_local! {
    // WKWebView and its observer are main-thread-only Objective-C objects.
    // Keeping them in a thread-local registry makes that ownership explicit
    // and guarantees observer removal before release.
    static OBSERVERS: RefCell<HashMap<String, ObserverRegistration>> =
        RefCell::new(HashMap::new());
}

fn snapshot(webview: &WKWebView) -> NativeNavigationSnapshot {
    // SAFETY: All WKWebView property access occurs on AppKit's main thread.
    unsafe {
        NativeNavigationSnapshot {
            can_go_back: webview.canGoBack(),
            can_go_forward: webview.canGoForward(),
            is_loading: webview.isLoading(),
        }
    }
}

async fn await_main_thread_result<T>(
    receiver: oneshot::Receiver<Result<T, String>>,
    operation: &str,
) -> Result<T, String> {
    timeout(MAIN_THREAD_OPERATION_TIMEOUT, receiver)
        .await
        .map_err(|_| format!("Timed out while {operation}."))?
        .map_err(|_| format!("The embedded browser closed while {operation}."))?
}

async fn await_cancellable_main_thread_result<T>(
    mut receiver: oneshot::Receiver<Result<T, String>>,
    claim: &AtomicU8,
    operation: &str,
) -> Result<T, String> {
    match timeout(MAIN_THREAD_OPERATION_TIMEOUT, &mut receiver).await {
        Ok(result) => {
            result.map_err(|_| format!("The embedded browser closed while {operation}."))?
        }
        Err(_) => {
            if claim
                .compare_exchange(
                    OPERATION_PENDING,
                    OPERATION_CANCELLED,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                return Err(format!("Timed out while {operation}."));
            }

            // Once the main-thread callback claims the mutation it must
            // complete before the caller releases its lifecycle lease.
            receiver
                .await
                .map_err(|_| format!("The embedded browser closed while {operation}."))?
        }
    }
}

pub(crate) async fn observe_provider_navigation(
    webview: &Webview,
    generation: u32,
    handler: impl Fn(NativeNavigationSnapshot) + Send + 'static,
) -> Result<(), String> {
    let label = webview.label().to_string();
    let callback_label = label.clone();
    let (sender, receiver) = oneshot::channel();
    webview
        .with_webview(move |platform_webview| {
            if sender.is_closed() {
                return;
            }
            let Some(main_thread) = MainThreadMarker::new() else {
                let _ = sender.send(Err(
                    "The embedded browser observer was not created on the main thread.".to_string(),
                ));
                return;
            };

            // SAFETY: Tauri guarantees this is a valid WKWebView pointer for
            // the callback duration. Retaining it extends its lifetime until
            // the observer is explicitly detached.
            let native_webview = unsafe { &*platform_webview.inner().cast::<WKWebView>() };
            let native_identity = native_webview as *const WKWebView as usize;
            let observer =
                NavigationObserver::new(native_webview.retain(), Box::new(handler), main_thread);
            OBSERVERS.with(|observers| {
                observers.borrow_mut().insert(
                    callback_label.clone(),
                    ObserverRegistration {
                        generation,
                        native_identity,
                        _observer: observer,
                    },
                );
            });

            if sender.send(Ok(())).is_err() {
                // The caller timed out or was dropped while setup was running.
                // Remove only the registration installed by this callback.
                OBSERVERS.with(|observers| {
                    let mut observers = observers.borrow_mut();
                    let is_same = observers.get(&callback_label).is_some_and(|registration| {
                        registration.generation == generation
                            && registration.native_identity == native_identity
                    });
                    if is_same {
                        observers.remove(&callback_label);
                    }
                });
            }
        })
        .map_err(|error| format!("Could not schedule embedded browser observation: {error}"))?;

    await_main_thread_result(receiver, "starting embedded browser observation").await
}

/// Removes whatever observer is registered for a provider label. Callers hold
/// the provider creation lock, so a replacement observer cannot be installed
/// concurrently and there is no older registration to distinguish.
pub(crate) async fn detach_provider_navigation_observer(
    app: &AppHandle,
    label: &str,
) -> Result<(), String> {
    let label = label.to_string();
    let (sender, receiver) = oneshot::channel();
    app.run_on_main_thread(move || {
        OBSERVERS.with(|observers| {
            observers.borrow_mut().remove(&label);
        });
        let _ = sender.send(Ok(()));
    })
    .map_err(|error| format!("Could not schedule embedded browser observer cleanup: {error}"))?;

    await_main_thread_result(receiver, "stopping embedded browser observation").await
}

pub(crate) async fn control_provider_navigation(
    webview: &Webview,
    action: NativeNavigationAction,
    should_run: impl FnOnce() -> bool + Send + 'static,
) -> Result<Option<NativeNavigationSnapshot>, String> {
    let (sender, receiver) = oneshot::channel();
    let claim = Arc::new(AtomicU8::new(OPERATION_PENDING));
    let _pending_mutation_cancellation = PendingMutationCancellation {
        claim: Arc::clone(&claim),
    };
    let callback_claim = Arc::clone(&claim);
    webview
        .with_webview(move |platform_webview| {
            if callback_claim
                .compare_exchange(
                    OPERATION_PENDING,
                    OPERATION_CLAIMED,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_err()
            {
                return;
            }
            if !should_run() {
                let _ = sender.send(Ok(None));
                return;
            }

            // SAFETY: Tauri invokes with_webview on the main thread with a
            // valid WKWebView pointer. Every action uses WebKit's typed API.
            let webview = unsafe { &*platform_webview.inner().cast::<WKWebView>() };
            unsafe {
                match action {
                    NativeNavigationAction::Back if webview.canGoBack() => {
                        webview.goBack();
                    }
                    NativeNavigationAction::Forward if webview.canGoForward() => {
                        webview.goForward();
                    }
                    NativeNavigationAction::Reload if !webview.isLoading() => {
                        webview.reload();
                    }
                    NativeNavigationAction::Stop if webview.isLoading() => {
                        webview.stopLoading();
                    }
                    _ => {}
                }
            }

            let _ = sender.send(Ok(Some(snapshot(webview))));
        })
        .map_err(|error| format!("Could not schedule embedded browser navigation: {error}"))?;

    await_cancellable_main_thread_result(
        receiver,
        &claim,
        "controlling embedded browser navigation",
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropped_pending_mutation_is_cancelled_but_a_claimed_callback_is_untouched() {
        let pending = Arc::new(AtomicU8::new(OPERATION_PENDING));
        {
            let _guard = PendingMutationCancellation {
                claim: Arc::clone(&pending),
            };
        }
        assert_eq!(pending.load(Ordering::Acquire), OPERATION_CANCELLED);

        let claimed = Arc::new(AtomicU8::new(OPERATION_CLAIMED));
        {
            let _guard = PendingMutationCancellation {
                claim: Arc::clone(&claimed),
            };
        }
        assert_eq!(claimed.load(Ordering::Acquire), OPERATION_CLAIMED);
    }
}
