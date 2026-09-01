//! Keeps every provider pane where the layout says it belongs.
//!
//! Placement is owned by the backend, not by the frontend that measures it.
//! The frontend can only observe DOM layout; it cannot see a window resize that
//! AppKit has already applied to the native view tree, and its
//! `requestAnimationFrame` callbacks are throttled — or suspended outright —
//! while the window is occluded or hidden to the tray. A pane whose position
//! depends on those callbacks is a pane that drifts.
//!
//! So the frontend reports what the layout wants, once, whenever it changes;
//! the backend remembers it as edge insets and re-derives the native rect from
//! the live surface whenever the window's own geometry moves.
//!
//! Every placement is confirmed. `set_bounds` converts a top-left origin into
//! AppKit's bottom-left coordinates by subtracting from the host view's height
//! *at the moment the frame is applied*. When the window is mid-resize — a
//! restored-maximized launch dispatches its zoom asynchronously, so this is not
//! a rare case — that height is stale and the pane lands off by the difference,
//! which is how a pane ends up drawn over the title bar. Reading the frame back
//! catches exactly that, and re-deriving against the settled surface fixes it.

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Mutex,
    },
    time::Duration,
};

use log::{debug, info, warn};
use tauri::{AppHandle, Manager, Rect, Runtime, Webview};

use super::{
    config::Provider,
    error::{ProviderCommandError, ProviderErrorCode},
    geometry::{self, HostSurface, PaneInsets, ProviderBounds},
};
use crate::{platform, MAIN_WINDOW_LABEL};

/// A live window drag emits a resize event per frame. One correction pass after
/// the burst settles is enough, because the pane's autoresizing mask already
/// tracked the window natively for every frame in between.
const REFRESH_COALESCE: Duration = Duration::from_millis(32);

/// The layout each provider pane was last asked for, in a form that outlives
/// the window size it was measured at.
///
/// Insets are deliberately not forgotten when a pane closes: they describe the
/// shell's layout, not the pane's lifetime, and a pane that is recreated lands
/// in the same place it left. A refresh with no live WebView is simply skipped.
#[derive(Default)]
pub(crate) struct ProviderPlacement {
    insets: Mutex<HashMap<Provider, PaneInsets>>,
    refresh_pending: AtomicBool,
    /// The last surface origin that was reported. Held so the log records the
    /// measurement once and on every change, rather than on every placement.
    reported_origin: Mutex<Option<(f64, f64)>>,
    /// TEMPORARY: how many diagnostic readings have been taken.
    probes: AtomicUsize,
}

impl ProviderPlacement {
    fn remember(&self, provider: Provider, insets: PaneInsets) {
        self.lock().insert(provider, insets);
    }

    fn recall(&self, provider: Provider) -> Option<PaneInsets> {
        self.lock().get(&provider).copied()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<Provider, PaneInsets>> {
        self.insets
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Records the offset between the window's client area and the space panes
    /// are placed in, the first time it is measured and whenever it changes.
    ///
    /// This is the value that used to be inferred from window metrics, so it is
    /// worth having in the log of a shipped build: a title-bar style change or
    /// an AppKit revision shows up here as a number, not as a misplaced pane.
    /// It changes almost never, which is why reporting it is affordable.
    fn note_origin(&self, surface: HostSurface) {
        let origin = surface.origin();
        let mut reported = self
            .reported_origin
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if *reported == Some(origin) {
            return;
        }
        *reported = Some(origin);
        info!(
            target: "prompter::provider",
            "event=pane_surface_measured origin_x={:.1} origin_y={:.1}",
            origin.0, origin.1
        );
    }
}

/// Translates a frontend measurement into the rect to place `provider` at, and
/// remembers the layout it implies.
pub(crate) fn adopt<R: Runtime>(
    app: &AppHandle<R>,
    provider: Provider,
    bounds: ProviderBounds,
) -> Result<Rect, ProviderCommandError> {
    let surface = host_surface(app)?;
    let insets = bounds.into_insets(surface)?;
    let rect = insets.resolve(surface)?;
    app.state::<ProviderPlacement>().remember(provider, insets);
    Ok(rect)
}

/// Places a pane and confirms the platform put it there.
pub(crate) fn apply<R: Runtime>(
    app: &AppHandle<R>,
    provider: Provider,
    webview: &Webview<R>,
    requested: Rect,
) -> Result<(), ProviderCommandError> {
    set_bounds(webview, requested)?;
    confirm(app, provider, webview, requested);
    Ok(())
}

/// Re-derives and re-applies every live pane against the surface as it is now.
///
/// Called after the window's geometry changes, where the frontend's own
/// measurements are either late or unavailable.
pub(crate) fn refresh<R: Runtime>(app: &AppHandle<R>) {
    let surface = match host_surface(app) {
        Ok(surface) => surface,
        // Mid-transition the window has no measurable surface. The event that
        // ends the transition brings another refresh.
        Err(error) => {
            debug!(
                target: "prompter::provider",
                "event=pane_refresh_skipped reason={}", error.message
            );
            return;
        }
    };

    let placement = app.state::<ProviderPlacement>();
    for provider in Provider::ALL {
        let (Some(insets), Some(webview)) = (
            placement.recall(provider),
            app.get_webview(provider.config().webview_label),
        ) else {
            continue;
        };
        let Ok(rect) = insets.resolve(surface) else {
            continue;
        };
        if set_bounds(&webview, rect).is_ok() {
            confirm(app, provider, &webview, rect);
        }
    }
}

/// Schedules a refresh off the main thread so a burst of geometry events
/// collapses into a single pass.
///
/// Dispatching from a worker matters beyond coalescing: the window event that
/// triggers this is delivered from inside the event loop, and placement calls
/// made from there would re-enter it. Hopping threads turns the refresh into an
/// ordinary queued main-thread task instead.
pub(crate) fn schedule_refresh<R: Runtime>(app: &AppHandle<R>) {
    if app
        .state::<ProviderPlacement>()
        .refresh_pending
        .swap(true, Ordering::AcqRel)
    {
        return;
    }

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(REFRESH_COALESCE).await;
        // Cleared before the pass runs, so an event that arrives while it is
        // in flight schedules the follow-up it needs.
        app.state::<ProviderPlacement>()
            .refresh_pending
            .store(false, Ordering::Release);

        let handle = app.clone();
        if let Err(error) = app.run_on_main_thread(move || refresh(&handle)) {
            debug!(
                target: "prompter::provider",
                "event=pane_refresh_dispatch_failed reason={error}"
            );
        }
    });
}

/// TEMPORARY DIAGNOSTIC — remove once the pane offset is understood.
///
/// Logs the rect we asked for beside AppKit's own reading of where the pane and
/// the main WebView actually sit, in screen coordinates. This is precisely what
/// `confirm` below cannot tell us: `webview.bounds()` inverts the same flip
/// that `set_bounds` applied, so it agrees with a wrong placement instead of
/// catching it. AppKit does not share that arithmetic.
///
/// Rate-limited, because a live drag produces a placement per frame and the
/// question is settled by the first few readings.
pub(crate) fn probe(app: &AppHandle, provider: Provider, requested: Rect) {
    const PROBE_LIMIT: usize = 8;

    if app
        .state::<ProviderPlacement>()
        .probes
        .fetch_add(1, Ordering::Relaxed)
        >= PROBE_LIMIT
    {
        return;
    }

    let label = provider.config().webview_label;
    let Some(main) = app.get_webview(MAIN_WINDOW_LABEL) else {
        return;
    };
    if let Some(scale) = geometry::scale_factor(&main) {
        info!(
            target: "prompter::provider",
            "event=geometry_probe role=requested provider={label} rect={} scale={scale}",
            geometry::describe(requested, scale)
        );
    }
    platform::log_webview_geometry(&main, "main");
    if let Some(pane) = app.get_webview(label) {
        platform::log_webview_geometry(&pane, "pane");
    }
    probe_dom(app);
}

/// TEMPORARY DIAGNOSTIC — asks the DOM where it thinks it is.
///
/// The native probe reads the WebView's frame; this reads the coordinate space
/// the frontend measures in. If the two disagree, every rect the frontend
/// reports is being handed to AppKit in the wrong space, and no amount of
/// correctness on the native side can place the pane where the card is.
///
/// The answer comes back through the window title because Tauri's `eval` is
/// one-way: there is no JavaScript return channel to a Rust caller.
pub(crate) fn probe_dom(app: &AppHandle) {
    let Some(main) = app.get_webview(MAIN_WINDOW_LABEL) else {
        return;
    };
    let script = r#"(() => {
  try {
    const rect = (sel) => {
      const el = document.querySelector(sel);
      if (!el) return null;
      const r = el.getBoundingClientRect();
      return [+r.left.toFixed(1), +r.top.toFixed(1), +r.width.toFixed(1), +r.height.toFixed(1)];
    };
    const payload = JSON.stringify({
      inner: [window.innerWidth, window.innerHeight],
      outer: [window.outerWidth, window.outerHeight],
      screen_xy: [window.screenX, window.screenY],
      doc_el: [document.documentElement.clientWidth, document.documentElement.clientHeight],
      dpr: window.devicePixelRatio,
      host: rect(".provider-webview-host"),
      card: rect(".browser-card"),
      dock: rect(".bottom-dock"),
      roots: Array.from(document.body.children).map((el) => String(el.className)).join("|").slice(0, 120),
      body: (document.body.innerText || "").replace(/\s+/g, " ").slice(0, 200)
    });
    window.__TAURI_INTERNALS__.invoke("probe_dom_metrics", { metrics: payload });
  } catch (error) {
    window.__TAURI_INTERNALS__.invoke("probe_dom_metrics", {
      metrics: JSON.stringify({ error: String(error) }),
    });
  }
})();"#;
    if let Err(error) = main.eval(script) {
        info!(target: "prompter::provider", "event=dom_probe outcome=eval_failed reason={error}");
    }
}

/// Reads back what `probe_dom` wrote, then restores the real title.
pub(crate) fn read_dom_probe(app: &AppHandle) {
    let Some(window) = app.get_window(MAIN_WINDOW_LABEL) else {
        return;
    };
    match window.title() {
        Ok(title) if title.starts_with("PROBE ") => {
            info!(target: "prompter::provider", "event=dom_probe {}", &title[6..]);
            let _ = window.set_title("Prompter");
        }
        Ok(title) => {
            info!(target: "prompter::provider", "event=dom_probe outcome=not_reported title={title}");
        }
        Err(error) => {
            info!(target: "prompter::provider", "event=dom_probe outcome=unreadable reason={error}");
        }
    }
}

/// TEMPORARY DIAGNOSTIC — reads again once the window has stopped moving.
///
/// A restored-maximized launch places the pane mid-zoom, so the first reading
/// can be correct for a window size that no longer exists. The number that
/// matters is the one on screen while the user is looking at it.
pub(crate) fn probe_when_settled(app: &AppHandle, provider: Provider) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(2500)).await;
        if let Some(main) = app.get_webview(MAIN_WINDOW_LABEL) {
            platform::log_webview_geometry(&main, "main_settled");
        }
        if let Some(pane) = app.get_webview(provider.config().webview_label) {
            platform::log_webview_geometry(&pane, "pane_settled");
        }
        probe_dom(&app);
        tokio::time::sleep(Duration::from_millis(400)).await;
        read_dom_probe(&app);
    });
}

/// Reads the applied frame back, and corrects it once against the settled
/// surface if the platform placed the pane somewhere else.
///
/// A pane that is still wrong after that correction is logged rather than
/// raised: the placement is cosmetic and self-healing — the next layout change
/// or window event runs this again — so failing the caller's command would
/// report an error for something already on its way to being fixed.
fn confirm<R: Runtime>(
    app: &AppHandle<R>,
    provider: Provider,
    webview: &Webview<R>,
    requested: Rect,
) {
    let Some(scale) = geometry::scale_factor(webview) else {
        return;
    };
    let Ok(actual) = webview.bounds() else {
        return;
    };
    if geometry::placement_matches(requested, actual, scale) {
        return;
    }

    let Some(corrected) = host_surface(app)
        .ok()
        .zip(app.state::<ProviderPlacement>().recall(provider))
        .and_then(|(surface, insets)| insets.resolve(surface).ok())
    else {
        return;
    };

    // Reported at info: the platform declining an explicit placement is never
    // routine, the tolerance above already absorbs pixel snapping, and this is
    // the line that tells a shipped build whether panes are being placed
    // straight away or landing somewhere else first.
    info!(
        target: "prompter::provider",
        "event=pane_placement_corrected provider={} requested={} actual={} corrected={}",
        provider.config().webview_label,
        geometry::describe(requested, scale),
        geometry::describe(actual, scale),
        geometry::describe(corrected, scale)
    );

    if set_bounds(webview, corrected).is_err() {
        return;
    }
    let Ok(settled) = webview.bounds() else {
        return;
    };
    if !geometry::placement_matches(corrected, settled, scale) {
        warn!(
            target: "prompter::provider",
            "event=pane_placement_unconfirmed provider={} requested={} actual={}",
            provider.config().webview_label,
            geometry::describe(corrected, scale),
            geometry::describe(settled, scale)
        );
    }
}

/// Measures the surface the panes are placed into, from the main WebView.
fn host_surface<R: Runtime>(app: &AppHandle<R>) -> Result<HostSurface, ProviderCommandError> {
    let main = app.get_webview(MAIN_WINDOW_LABEL).ok_or_else(|| {
        ProviderCommandError::new(
            ProviderErrorCode::WindowMissing,
            "The Prompter window was not found.",
        )
    })?;
    let surface = geometry::measure_host_surface(&main)?;
    app.state::<ProviderPlacement>().note_origin(surface);
    Ok(surface)
}

fn set_bounds<R: Runtime>(webview: &Webview<R>, rect: Rect) -> Result<(), ProviderCommandError> {
    webview.set_bounds(rect).map_err(|error| {
        ProviderCommandError::new(
            ProviderErrorCode::WebviewOperationFailed,
            format!("Could not resize the embedded browser: {error}"),
        )
    })
}
