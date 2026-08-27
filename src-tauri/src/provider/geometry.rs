//! Where a provider pane sits, and how that is decided.
//!
//! Two coordinate spaces meet here. The frontend measures the pane's host
//! element with `getBoundingClientRect`, which is relative to the main
//! WebView's client area. Child WebViews are placed relative to the native
//! view that hosts them, with the origin at that view's top-left.
//!
//! Nothing in this module infers the relationship between those spaces from
//! window metrics. A title-bar height derived from `outer_position` minus
//! `inner_position` is a guess about a platform detail that the platform is
//! free to change, and a wrong guess is invisible until it puts the pane over
//! the title bar. Instead the relationship is *measured*: the main WebView is
//! asked for its own frame in the very space the panes are placed into, and
//! that answer is the origin. It stays correct across title-bar styles,
//! display scales, and future AppKit releases, because it is not a prediction.
//!
//! The pane is then remembered as **insets** — its distance from each edge of
//! the client area — rather than as an absolute rect. The CSS shell fixes those
//! distances (a padded workspace column beside a fixed-width sidebar, above a
//! fixed-height dock), so insets are the part of the layout that survives a
//! window resize. Holding them lets the backend re-derive an exact rect for any
//! surface size without asking the frontend to measure again.

use serde::Deserialize;
use tauri::{LogicalPosition, LogicalSize, Rect, Runtime, Webview};

use super::error::{ProviderCommandError, ProviderErrorCode};

const MIN_PROVIDER_SIZE: f64 = 240.0;
const MAX_PROVIDER_SIZE: f64 = 20_000.0;
const MAX_PROVIDER_COORDINATE: f64 = 20_000.0;

/// A placement counts as honored when the platform reports it back within this
/// many logical points. Sub-point differences are the platform snapping a frame
/// to backing pixels, not drift.
const PLACEMENT_TOLERANCE: f64 = 1.0;

/// Every rejection here means the same thing to the caller: the layout that was
/// measured is not one the pane can be placed against yet. It is never fatal —
/// the next layout change or window event produces another measurement.
fn not_ready() -> ProviderCommandError {
    ProviderCommandError::new(
        ProviderErrorCode::InvalidBounds,
        "The embedded browser area is not ready yet.",
    )
}

/// The pane's host element as the frontend measured it, in client coordinates
/// of the main WebView.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderBounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

/// Where the main WebView sits inside the native view that hosts the panes, and
/// how large its client area is. Measured from the platform, never inferred.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct HostSurface {
    origin_x: f64,
    origin_y: f64,
    width: f64,
    height: f64,
}

/// The pane's distance from each edge of the main WebView's client area.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PaneInsets {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
}

impl ProviderBounds {
    /// Converts a frontend measurement into insets against the surface it was
    /// taken from.
    ///
    /// A measurement that does not fit inside the surface is rejected rather
    /// than clamped. The two are read at slightly different moments, so a rect
    /// that overflows means the DOM and the native view tree disagree about how
    /// big the window is right now — exactly the transient state that produces
    /// a misplaced pane. Refusing it keeps the last good insets in play until
    /// the two agree again.
    pub(crate) fn into_insets(
        self,
        surface: HostSurface,
    ) -> Result<PaneInsets, ProviderCommandError> {
        if !self.x.is_finite()
            || !self.y.is_finite()
            || !self.width.is_finite()
            || !self.height.is_finite()
            || self.x.abs() > MAX_PROVIDER_COORDINATE
            || self.y.abs() > MAX_PROVIDER_COORDINATE
            || self.width < MIN_PROVIDER_SIZE
            || self.height < MIN_PROVIDER_SIZE
            || self.width > MAX_PROVIDER_SIZE
            || self.height > MAX_PROVIDER_SIZE
        {
            return Err(not_ready());
        }

        let left = self.x.max(0.0);
        let top = self.y.max(0.0);
        let right = surface.width - (left + self.width);
        let bottom = surface.height - (top + self.height);
        if right < -PLACEMENT_TOLERANCE || bottom < -PLACEMENT_TOLERANCE {
            return Err(not_ready());
        }

        Ok(PaneInsets {
            left,
            top,
            right: right.max(0.0),
            bottom: bottom.max(0.0),
        })
    }
}

impl PaneInsets {
    /// Re-derives the native rect for a surface of the current size.
    pub(crate) fn resolve(self, surface: HostSurface) -> Result<Rect, ProviderCommandError> {
        let width = surface.width - self.left - self.right;
        let height = surface.height - self.top - self.bottom;
        if width < MIN_PROVIDER_SIZE || height < MIN_PROVIDER_SIZE {
            return Err(not_ready());
        }

        Ok(Rect {
            position: LogicalPosition::new(
                surface.origin_x + self.left,
                surface.origin_y + self.top,
            )
            .into(),
            size: LogicalSize::new(width, height).into(),
        })
    }
}

/// Asks the main WebView where it is and how big it is, in the same space child
/// panes are placed into.
///
/// The platform reports positions and sizes in whichever unit it stores them,
/// so both are converted through the window's scale factor rather than assumed
/// to be logical.
pub(crate) fn measure_host_surface<R: Runtime>(
    main: &Webview<R>,
) -> Result<HostSurface, ProviderCommandError> {
    let scale = scale_factor(main).ok_or_else(not_ready)?;
    let bounds = main.bounds().map_err(|error| {
        ProviderCommandError::new(
            ProviderErrorCode::WebviewOperationFailed,
            format!("Could not measure the Prompter window: {error}"),
        )
    })?;
    let position = bounds.position.to_logical::<f64>(scale);
    let size = bounds.size.to_logical::<f64>(scale);

    let surface = HostSurface {
        origin_x: position.x,
        origin_y: position.y,
        width: size.width,
        height: size.height,
    };
    surface.validate()?;
    Ok(surface)
}

impl HostSurface {
    /// The measured offset between the client area and the space panes are
    /// placed in — the number this module exists to stop guessing.
    pub(crate) fn origin(self) -> (f64, f64) {
        (self.origin_x, self.origin_y)
    }

    fn validate(self) -> Result<(), ProviderCommandError> {
        if !self.origin_x.is_finite()
            || !self.origin_y.is_finite()
            || self.origin_x.abs() > MAX_PROVIDER_COORDINATE
            || self.origin_y.abs() > MAX_PROVIDER_COORDINATE
            || self.width < MIN_PROVIDER_SIZE
            || self.height < MIN_PROVIDER_SIZE
            || self.width > MAX_PROVIDER_SIZE
            || self.height > MAX_PROVIDER_SIZE
        {
            return Err(not_ready());
        }
        Ok(())
    }
}

/// The scale factor of the window a WebView belongs to, or `None` when the
/// platform reports one that cannot be divided by.
pub(crate) fn scale_factor<R: Runtime>(webview: &Webview<R>) -> Option<f64> {
    webview
        .window()
        .scale_factor()
        .ok()
        .filter(|scale| scale.is_finite() && *scale > 0.0)
}

/// Whether the platform placed a pane where it was asked to.
pub(crate) fn placement_matches(requested: Rect, actual: Rect, scale: f64) -> bool {
    let requested_position = requested.position.to_logical::<f64>(scale);
    let requested_size = requested.size.to_logical::<f64>(scale);
    let actual_position = actual.position.to_logical::<f64>(scale);
    let actual_size = actual.size.to_logical::<f64>(scale);

    within_tolerance(requested_position.x, actual_position.x)
        && within_tolerance(requested_position.y, actual_position.y)
        && within_tolerance(requested_size.width, actual_size.width)
        && within_tolerance(requested_size.height, actual_size.height)
}

fn within_tolerance(requested: f64, actual: f64) -> bool {
    (requested - actual).abs() <= PLACEMENT_TOLERANCE
}

/// Renders a rect for diagnostics. Placement bugs are only debuggable when the
/// log says which numbers disagreed.
pub(crate) fn describe(rect: Rect, scale: f64) -> String {
    let position = rect.position.to_logical::<f64>(scale);
    let size = rect.size.to_logical::<f64>(scale);
    format!(
        "{:.1},{:.1} {:.1}x{:.1}",
        position.x, position.y, size.width, size.height
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SURFACE: HostSurface = HostSurface {
        origin_x: 0.0,
        origin_y: 0.0,
        width: 1380.0,
        height: 818.0,
    };

    fn logical(rect: Rect) -> (f64, f64, f64, f64) {
        let position = rect.position.to_logical::<f64>(1.0);
        let size = rect.size.to_logical::<f64>(1.0);
        (position.x, position.y, size.width, size.height)
    }

    fn host(x: f64, y: f64, width: f64, height: f64) -> ProviderBounds {
        ProviderBounds {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn measurements_reject_non_finite_and_out_of_range_values() {
        let valid = host(0.0, 0.0, MIN_PROVIDER_SIZE, MIN_PROVIDER_SIZE);
        assert!(valid.into_insets(SURFACE).is_ok());

        for invalid in [
            host(f64::NAN, 0.0, 400.0, 300.0),
            host(0.0, f64::INFINITY, 400.0, 300.0),
            host(0.0, 0.0, MIN_PROVIDER_SIZE - 1.0, 300.0),
            host(0.0, 0.0, 400.0, 0.0),
            host(0.0, 0.0, MAX_PROVIDER_SIZE + 1.0, 300.0),
            host(0.0, 0.0, 400.0, f64::MAX),
            host(MAX_PROVIDER_COORDINATE + 1.0, 0.0, 400.0, 300.0),
            host(0.0, -MAX_PROVIDER_COORDINATE - 1.0, 400.0, 300.0),
        ] {
            assert!(invalid.into_insets(SURFACE).is_err());
        }
    }

    #[test]
    fn insets_are_the_distance_to_each_surface_edge() {
        let insets = host(282.0, 10.0, 1088.0, 690.0)
            .into_insets(SURFACE)
            .expect("bounds should convert");

        assert_eq!(
            insets,
            PaneInsets {
                left: 282.0,
                top: 10.0,
                right: 10.0,
                bottom: 118.0,
            }
        );
    }

    #[test]
    fn a_measurement_larger_than_its_surface_is_refused_rather_than_clamped() {
        // The DOM has already laid out for a maximized window while the native
        // view is still the pre-maximize size. Adopting this would bake insets
        // that place the pane outside the window.
        assert!(host(282.0, 10.0, 1088.0, 1400.0)
            .into_insets(SURFACE)
            .is_err());
        assert!(host(282.0, 10.0, 1800.0, 690.0)
            .into_insets(SURFACE)
            .is_err());
    }

    #[test]
    fn insets_survive_a_resize_by_re_deriving_the_rect() {
        let insets = host(282.0, 10.0, 1088.0, 690.0)
            .into_insets(SURFACE)
            .expect("bounds should convert");

        let maximized = HostSurface {
            width: 2684.0,
            height: 1714.0,
            ..SURFACE
        };
        let rect = insets.resolve(maximized).expect("insets should resolve");

        // Same distance from every edge; the pane grew with the window.
        assert_eq!(logical(rect), (282.0, 10.0, 2392.0, 1586.0));
    }

    #[test]
    fn the_measured_surface_origin_offsets_the_pane() {
        let insets = host(282.0, 10.0, 1088.0, 690.0)
            .into_insets(SURFACE)
            .expect("bounds should convert");

        let inset_surface = HostSurface {
            origin_x: 0.0,
            origin_y: 28.0,
            ..SURFACE
        };
        let rect = insets
            .resolve(inset_surface)
            .expect("insets should resolve");

        assert_eq!(logical(rect), (282.0, 38.0, 1088.0, 690.0));
    }

    #[test]
    fn a_surface_too_small_for_the_insets_is_refused() {
        let insets = host(282.0, 10.0, 1088.0, 690.0)
            .into_insets(SURFACE)
            .expect("bounds should convert");

        let collapsed = HostSurface {
            width: 400.0,
            height: 400.0,
            ..SURFACE
        };
        assert!(insets.resolve(collapsed).is_err());
    }

    #[test]
    fn round_tripping_a_measurement_reproduces_the_requested_rect() {
        let requested = host(282.0, 10.0, 1088.0, 690.0);
        let insets = requested
            .into_insets(SURFACE)
            .expect("bounds should convert");

        let rect = insets.resolve(SURFACE).expect("insets should resolve");

        assert_eq!(
            logical(rect),
            (requested.x, requested.y, requested.width, requested.height)
        );
        assert!(placement_matches(rect, rect, 1.0));
    }

    #[test]
    fn placement_tolerates_pixel_snapping_but_not_drift() {
        let rect = Rect {
            position: LogicalPosition::new(282.0, 10.0).into(),
            size: LogicalSize::new(1088.0, 690.0).into(),
        };
        let snapped = Rect {
            position: LogicalPosition::new(282.5, 9.5).into(),
            size: LogicalSize::new(1087.5, 690.5).into(),
        };
        let drifted = Rect {
            position: LogicalPosition::new(282.0, -18.0).into(),
            size: LogicalSize::new(1088.0, 690.0).into(),
        };

        assert!(placement_matches(rect, snapped, 1.0));
        assert!(!placement_matches(rect, drifted, 1.0));
    }

    #[test]
    fn placement_comparison_normalizes_mixed_units() {
        let logical_rect = Rect {
            position: LogicalPosition::new(282.0, 10.0).into(),
            size: LogicalSize::new(1088.0, 690.0).into(),
        };
        let physical_rect = Rect {
            position: tauri::PhysicalPosition::new(564.0, 20.0).into(),
            size: tauri::PhysicalSize::new(2176.0, 1380.0).into(),
        };

        assert!(placement_matches(logical_rect, physical_rect, 2.0));
    }
}
