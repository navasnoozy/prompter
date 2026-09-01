//! Where the main window is, and how big it is — decided in one place.
//!
//! # Why this is not `tauri-plugin-window-state`
//!
//! The plugin persists `outer_position()` and replays it through
//! `set_position()`. Both are `PhysicalPosition`, and on macOS tao produces one
//! by taking AppKit's frame origin — which is *already* in the global points
//! space — and multiplying it by **the window's current scale factor**. The
//! number it stores is therefore `points × whichever display the window
//! happened to be on`, with the scale factor itself never recorded.
//!
//! Move a window between a 1x display and a 2x one and the same spot on screen
//! is two different numbers. Prompter's own machine is exactly that setup, and
//! it is how the window came back at half its saved x and mostly off screen.
//!
//! The plugin's on-screen guard cannot catch it either: it compares the saved
//! rect against `Monitor::position()`, which is scaled by *that monitor's* own
//! factor rather than the window's, so on a mixed-DPI desktop the two sides of
//! the comparison are in different spaces. It also passes a rect whose four
//! corners are tested individually, so a window with one corner grazing a
//! display counts as visible.
//!
//! # What this module does instead
//!
//! Logical points are the canonical macOS global coordinate — AppKit reports
//! frames in them and CoreGraphics reports display bounds in them, both with a
//! top-left origin and both independent of any display's backing scale. So the
//! frame is stored in points, and every display is converted into the same
//! space before anything is compared. Nothing is left in a unit whose meaning
//! depends on where the window is standing.
//!
//! Position and size are stored **together**, as one value, because a position
//! can only be judged against the size it belongs to.

use log::{info, warn};
use tauri::{LogicalPosition, LogicalSize, Manager, Monitor, Runtime, Window};

use crate::{settings, MAIN_WINDOW_LABEL};

/// The main window's floor, in logical points, mirroring `minWidth` and
/// `minHeight` in `tauri.conf.json`. A test below pins the two together.
const MIN_WINDOW_WIDTH: f64 = 1000.0;
const MIN_WINDOW_HEIGHT: f64 = 780.0;

/// Rejects a stored frame whose numbers are too large to be a real desktop.
/// Anything beyond this is a damaged document rather than a wide display wall.
const MAX_COORDINATE: f64 = 100_000.0;

/// How much of the window has to be on screen for its frame to be left alone.
///
/// The threshold exists to separate "the user parked it slightly over an edge"
/// from "this frame belongs to a display that is no longer here". Anything that
/// clears it is a window the user can see and grab, which is the only property
/// worth enforcing.
const MIN_VISIBLE_FRACTION: f64 = 0.6;

/// What the settings document holds for the main window.
///
/// The two cases are not the same question. A frame has a position that was
/// chosen on a real desktop and has to be judged against the displays attached
/// now; a bare size has no position at all and only needs placing.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Stored {
    /// A frame written by this module.
    Frame(Frame),
    /// A size carried over from the split-ownership build, whose position was
    /// deliberately not kept.
    Size { width: f64, height: f64 },
}

/// A window frame in logical points, with a top-left origin — the space AppKit
/// and CoreGraphics both report in, and the only one that means the same thing
/// on every display.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Frame {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

/// A display's usable area — its bounds less the menu bar and the Dock — in the
/// same space as [`Frame`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct WorkArea {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl Frame {
    fn right(self) -> f64 {
        self.x + self.width
    }

    fn bottom(self) -> f64 {
        self.y + self.height
    }

    /// `WIDTHxHEIGHT@X,Y`, points rounded to whole numbers.
    fn encode(self) -> String {
        format!(
            "{:.0}x{:.0}@{:.0},{:.0}",
            self.width, self.height, self.x, self.y
        )
    }

    /// Reads a frame back.
    ///
    /// Anything unparseable, non-finite, non-positive in size, or implausibly
    /// far from the origin is rejected rather than repaired. The configured
    /// default window is a better answer than a guess assembled from a damaged
    /// document, and rejecting is how the caller asks for it.
    fn decode(value: &str) -> Option<Self> {
        let (size, position) = value.split_once('@')?;
        let (width, height) = size.split_once('x')?;
        let (x, y) = position.split_once(',')?;

        let frame = Self {
            x: x.trim().parse().ok()?,
            y: y.trim().parse().ok()?,
            width: width.trim().parse().ok()?,
            height: height.trim().parse().ok()?,
        };

        let finite = frame.x.is_finite()
            && frame.y.is_finite()
            && frame.width.is_finite()
            && frame.height.is_finite();
        let plausible = frame.x.abs() <= MAX_COORDINATE
            && frame.y.abs() <= MAX_COORDINATE
            && frame.width > 0.0
            && frame.height > 0.0
            && frame.width <= MAX_COORDINATE
            && frame.height <= MAX_COORDINATE;

        (finite && plausible).then_some(frame)
    }
}

impl WorkArea {
    fn right(self) -> f64 {
        self.x + self.width
    }

    fn bottom(self) -> f64 {
        self.y + self.height
    }

    /// How much of `frame` this display would actually show.
    fn overlap(self, frame: Frame) -> f64 {
        let width = frame.right().min(self.right()) - frame.x.max(self.x);
        let height = frame.bottom().min(self.bottom()) - frame.y.max(self.y);
        if width <= 0.0 || height <= 0.0 {
            0.0
        } else {
            width * height
        }
    }
}

/// Fits a remembered frame to the displays that exist now.
///
/// The first and most important rule is that a usable frame is returned
/// **exactly as saved**. A window may hang slightly over an edge, and it may
/// straddle two displays — macOS allows both, users arrange windows that way on
/// purpose, and an app that quietly tidies them up on every launch is fighting
/// the window manager rather than helping. Correcting a frame is reserved for
/// one that the current desktop genuinely cannot show.
///
/// A frame is left alone when all of the following hold:
///
///  * enough of it lands on the union of the current work areas to see and grab
///  * it is at least the size the layout needs
///  * it is no larger than the display it mostly occupies
///
/// Otherwise it is re-fitted: sized to the floor and to that display, then
/// either slid into view or — when no display shows any part of it, because the
/// one it lived on is gone — centred, which is what the user reads as "the app
/// opened normally". Sliding is preferred to centring wherever it applies, so
/// unplugging a display nudges a window back into view instead of teleporting
/// it to the middle of the screen.
fn fit(saved: Frame, work_areas: &[WorkArea], primary: WorkArea) -> Frame {
    let (target, overlapped) = work_areas
        .iter()
        .map(|area| (*area, area.overlap(saved)))
        .max_by(|(_, left), (_, right)| left.total_cmp(right))
        .filter(|(_, overlap)| *overlap > 0.0)
        .unwrap_or((primary, 0.0));

    // Work areas never overlap each other, so the parts are safe to add up:
    // this is how much of the window the desktop as a whole would show.
    let visible: f64 = work_areas.iter().map(|area| area.overlap(saved)).sum();
    if visible >= saved.width * saved.height * MIN_VISIBLE_FRACTION
        && saved.width >= MIN_WINDOW_WIDTH
        && saved.height >= MIN_WINDOW_HEIGHT
        && saved.width <= target.width
        && saved.height <= target.height
    {
        return saved;
    }

    if overlapped == 0.0 {
        return centre(saved.width, saved.height, target);
    }

    let width = saved.width.max(MIN_WINDOW_WIDTH).min(target.width);
    let height = saved.height.max(MIN_WINDOW_HEIGHT).min(target.height);

    Frame {
        // `min` before `max` so a window wider than the display still starts at
        // the display's left edge rather than hanging off its right.
        x: saved.x.min(target.right() - width).max(target.x),
        y: saved.y.min(target.bottom() - height).max(target.y),
        width,
        height,
    }
}

/// Places a window of this size in the middle of a work area, sized to what the
/// layout needs and to what the display can hold.
fn centre(width: f64, height: f64, area: WorkArea) -> Frame {
    let width = width.max(MIN_WINDOW_WIDTH).min(area.width);
    let height = height.max(MIN_WINDOW_HEIGHT).min(area.height);
    Frame {
        x: area.x + (area.width - width) / 2.0,
        y: area.y + (area.height - height) / 2.0,
        width,
        height,
    }
}

/// Every display's work area, in logical points.
///
/// `Monitor::work_area()` already excludes the menu bar and the Dock, so the
/// menu-bar offset is read from the platform rather than reproduced here. It
/// arrives scaled by *that monitor's* own factor, which is what makes the
/// plugin's comparisons mix spaces; dividing each one by the factor it was
/// scaled with puts every display back into the single shared space.
fn work_area_of(monitor: &Monitor) -> WorkArea {
    let scale = monitor.scale_factor();
    let area = monitor.work_area();
    let position = area.position.to_logical::<f64>(scale);
    let size = area.size.to_logical::<f64>(scale);
    WorkArea {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
    }
}

/// The window's frame right now, or `None` while it is in a state whose frame
/// means nothing.
///
/// A minimized window reports the frame it will be restored to on some
/// platforms and the dock tile's on others, and a zero-sized window is one the
/// window server has not placed yet. Neither is worth remembering, and refusing
/// to record them leaves the last good frame on disk.
///
/// `outer_position` and `inner_size` are read as a pair because they are the
/// pair that can be set back: `set_position` takes an outer position and
/// `set_size` an inner size. Storing what can be restored keeps the round trip
/// exact and stops the title bar's height from ever needing to be guessed.
fn current_frame<R: Runtime>(window: &Window<R>) -> Option<Frame> {
    if window.is_minimized().unwrap_or(false) {
        return None;
    }

    let scale = window.scale_factor().ok()?;
    let position = window.outer_position().ok()?.to_logical::<f64>(scale);
    let size = window.inner_size().ok()?.to_logical::<f64>(scale);
    if size.width <= 0.0 || size.height <= 0.0 {
        return None;
    }

    Some(Frame {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
    })
}

/// Restores the frame the user last left the window at.
///
/// Called once during setup, while the window is still hidden, so the window is
/// only ever shown where it belongs — there is no move to observe and nothing
/// to sequence against. With nothing stored, this does nothing at all and the
/// window opens where `tauri.conf.json` says: centred, at the configured size.
pub(crate) fn restore<R: Runtime>(app: &tauri::AppHandle<R>) {
    let Some(window) = app.get_window(MAIN_WINDOW_LABEL) else {
        return;
    };

    let Some(saved) = read(app) else {
        info!(
            target: "prompter::lifecycle",
            "event=window_frame_restore outcome=default reason=nothing_stored"
        );
        return;
    };

    let monitors: Vec<WorkArea> = window
        .available_monitors()
        .unwrap_or_default()
        .iter()
        .map(work_area_of)
        .collect();
    let primary = window
        .primary_monitor()
        .ok()
        .flatten()
        .as_ref()
        .map(work_area_of)
        .or_else(|| monitors.first().copied());

    // No display at all means the window server has nothing to place against.
    // The configured default is the only sane answer.
    let Some(primary) = primary else {
        warn!(
            target: "prompter::lifecycle",
            "event=window_frame_restore outcome=default reason=no_monitors"
        );
        return;
    };

    let frame = match saved {
        Stored::Frame(frame) => fit(frame, &monitors, primary),
        Stored::Size { width, height } => centre(width, height, primary),
    };

    if let Err(error) = window.set_position(LogicalPosition::new(frame.x, frame.y)) {
        warn!(
            target: "prompter::lifecycle",
            "event=window_frame_restore outcome=failure stage=position reason={error}"
        );
        return;
    }
    if let Err(error) = window.set_size(LogicalSize::new(frame.width, frame.height)) {
        warn!(
            target: "prompter::lifecycle",
            "event=window_frame_restore outcome=failure stage=size reason={error}"
        );
        return;
    }

    info!(
        target: "prompter::lifecycle",
        "event=window_frame_restore outcome=success applied={}",
        frame.encode()
    );
}

/// Records where the window is now.
///
/// The window itself is the source of truth, read at the moment it matters,
/// rather than a copy kept in step with move and resize events. A copy has to
/// be maintained across every path that can change a frame — including the ones
/// that raise no event, such as a resize applied while the window is hidden —
/// and the only thing it buys is avoiding two reads that cost nothing.
pub(crate) fn persist<R: Runtime>(app: &tauri::AppHandle<R>) {
    let Some(window) = app.get_window(MAIN_WINDOW_LABEL) else {
        return;
    };
    let Some(frame) = current_frame(&window) else {
        return;
    };

    let coordinator = app.state::<settings::SettingsCoordinator>();
    if let Err(error) = settings::write_backend_string(
        app,
        &coordinator,
        settings::WINDOW_FRAME_KEY,
        &frame.encode(),
    ) {
        warn!(
            target: "prompter::lifecycle",
            "event=window_frame_persist outcome=failure reason={error:?}"
        );
    }
}

/// Reads the stored frame, adopting a size left by the previous split-ownership
/// build when there is no frame yet.
///
/// That build kept the size in `windowSize` and left the position to
/// `tauri-plugin-window-state`. The size is worth carrying over; the position
/// is not, because it is the physical-pixel value whose replay is the reason
/// this module exists. Reporting the size alone centres the window once, after
/// which the frame written here takes over.
fn read<R: Runtime>(app: &tauri::AppHandle<R>) -> Option<Stored> {
    let coordinator = app.state::<settings::SettingsCoordinator>();

    if let Some(stored) =
        settings::read_backend_string(app, &coordinator, settings::WINDOW_FRAME_KEY)
    {
        return match Frame::decode(&stored) {
            Some(frame) => Some(Stored::Frame(frame)),
            None => {
                warn!(
                    target: "prompter::lifecycle",
                    "event=window_frame_restore outcome=unreadable value={stored}"
                );
                None
            }
        };
    }

    let legacy =
        settings::read_backend_string(app, &coordinator, settings::LEGACY_WINDOW_SIZE_KEY)?;
    let (width, height) = legacy.split_once('x')?;
    let width: f64 = width.trim().parse().ok()?;
    let height: f64 = height.trim().parse().ok()?;
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return None;
    }

    info!(
        target: "prompter::lifecycle",
        "event=window_frame_migrated from=window_size value={legacy}"
    );
    Some(Stored::Size { width, height })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The built-in display: 1512x982 points, menu bar taken off the top.
    const BUILT_IN: WorkArea = WorkArea {
        x: 0.0,
        y: 38.0,
        width: 1512.0,
        height: 944.0,
    };

    /// A portrait 1x display left of and above the built-in — the arrangement
    /// that produced the off-screen window this module replaces.
    const PORTRAIT_LEFT: WorkArea = WorkArea {
        x: -1440.0,
        y: -258.0,
        width: 1440.0,
        height: 2560.0,
    };

    fn frame(x: f64, y: f64, width: f64, height: f64) -> Frame {
        Frame {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn the_window_floor_matches_the_bundled_configuration() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../../tauri.conf.json")).unwrap();
        let main = &config["app"]["windows"][0];

        assert_eq!(main["label"], serde_json::json!(MAIN_WINDOW_LABEL));
        assert_eq!(main["minWidth"].as_f64().unwrap(), MIN_WINDOW_WIDTH);
        assert_eq!(main["minHeight"].as_f64().unwrap(), MIN_WINDOW_HEIGHT);
    }

    #[test]
    fn a_frame_survives_the_round_trip() {
        let original = frame(-613.0, -129.0, 1373.0, 780.0);
        assert_eq!(Frame::decode(&original.encode()), Some(original));
    }

    #[test]
    fn a_frame_on_a_display_left_of_the_origin_keeps_its_sign() {
        // Negative coordinates are ordinary on a multi-display desktop, and
        // losing the sign is how a window ends up on the wrong side.
        assert_eq!(
            Frame::decode("1373x780@-1226,-258"),
            Some(frame(-1226.0, -258.0, 1373.0, 780.0))
        );
    }

    #[test]
    fn a_damaged_frame_is_refused_rather_than_guessed() {
        for value in [
            "",
            "1373x780",
            "1373@0,0",
            "1373x780@0",
            "x780@0,0",
            "1373x0@0,0",
            "0x780@0,0",
            "-1373x780@0,0",
            "widexhigh@0,0",
            "1373x780@a,b",
            "NaNxNaN@0,0",
            "infxinf@0,0",
            "1373x780@inf,0",
            "1373x780@999999,0",
        ] {
            assert_eq!(Frame::decode(value), None, "{value} should be refused");
        }
    }

    #[test]
    fn a_frame_already_on_screen_is_left_exactly_where_it_was() {
        let saved = frame(100.0, 100.0, 1373.0, 780.0);
        assert_eq!(fit(saved, &[BUILT_IN], BUILT_IN), saved);
    }

    #[test]
    fn the_off_screen_frame_that_caused_this_refactor_is_brought_back() {
        // The real failure: a position saved on the portrait display, halved by
        // the scale-factor round trip, replayed against the built-in alone.
        let saved = frame(-613.0, -129.0, 1373.0, 780.0);

        let fitted = fit(saved, &[BUILT_IN], BUILT_IN);

        assert_eq!(fitted, frame(0.0, 38.0, 1373.0, 780.0));
        assert!(fitted.x >= BUILT_IN.x && fitted.right() <= BUILT_IN.right());
        assert!(fitted.y >= BUILT_IN.y && fitted.bottom() <= BUILT_IN.bottom());
    }

    #[test]
    fn a_frame_on_a_display_that_is_still_attached_stays_on_it() {
        // Nothing is "corrected" onto the primary display just because the
        // coordinates are negative.
        let saved = frame(-1200.0, 200.0, 1373.0, 780.0);
        assert_eq!(fit(saved, &[BUILT_IN, PORTRAIT_LEFT], BUILT_IN), saved);
    }

    #[test]
    fn a_frame_whose_display_was_unplugged_is_centred_on_the_primary() {
        let saved = frame(-1400.0, -200.0, 1200.0, 800.0);

        let fitted = fit(saved, &[BUILT_IN], BUILT_IN);

        assert_eq!(fitted.width, 1200.0);
        assert_eq!(fitted.height, 800.0);
        assert_eq!(fitted.x, (1512.0 - 1200.0) / 2.0);
        assert_eq!(fitted.y, 38.0 + (944.0 - 800.0) / 2.0);
    }

    #[test]
    fn a_frame_hanging_off_an_edge_is_slid_in_rather_than_recentred() {
        // Still mostly visible, so the window should not jump to the middle.
        let saved = frame(400.0, 900.0, 1373.0, 780.0);

        let fitted = fit(saved, &[BUILT_IN], BUILT_IN);

        assert_eq!(fitted.x, 1512.0 - 1373.0);
        assert_eq!(fitted.y, 38.0 + 944.0 - 780.0);
    }

    #[test]
    fn a_window_straddling_two_displays_is_left_where_the_user_put_it() {
        // Three quarters on the portrait display, the rest on the built-in.
        // macOS allows this and people arrange windows this way deliberately;
        // tidying it onto one display would move a window nobody asked to move.
        let saved = frame(-900.0, 300.0, 1200.0, 780.0);

        assert_eq!(fit(saved, &[BUILT_IN, PORTRAIT_LEFT], BUILT_IN), saved);
    }

    #[test]
    fn a_size_carried_over_from_the_split_ownership_build_is_centred() {
        // The migration has a size and deliberately no position, so it must be
        // placed rather than fitted. Landing at the origin would put the title
        // bar under the menu bar.
        let fitted = centre(1373.0, 780.0, BUILT_IN);

        assert_eq!(fitted.x, (1512.0 - 1373.0) / 2.0);
        assert_eq!(fitted.y, 38.0 + (944.0 - 780.0) / 2.0);
        assert!(fitted.y >= BUILT_IN.y);
    }

    #[test]
    fn a_remembered_size_below_the_floor_is_raised_on_the_way_in() {
        // What the previous build's physical-pixel replay wrote to disk, and
        // the size that dropped the sidebar out of the layout entirely.
        let fitted = fit(frame(100.0, 100.0, 676.0, 505.0), &[BUILT_IN], BUILT_IN);

        assert_eq!(fitted.width, MIN_WINDOW_WIDTH);
        assert_eq!(fitted.height, MIN_WINDOW_HEIGHT);
    }

    #[test]
    fn a_window_larger_than_its_display_is_shrunk_to_fit_it() {
        let small = WorkArea {
            x: 0.0,
            y: 38.0,
            width: 1280.0,
            height: 700.0,
        };

        let fitted = fit(frame(0.0, 38.0, 1920.0, 1200.0), &[small], small);

        assert_eq!(fitted, frame(0.0, 38.0, 1280.0, 700.0));
    }

    #[test]
    fn fitting_is_stable() {
        // A frame that came out of `fit` must survive another pass unchanged,
        // or every launch would walk the window across the screen.
        for saved in [
            frame(-613.0, -129.0, 1373.0, 780.0),
            frame(400.0, 900.0, 1373.0, 780.0),
            frame(-1400.0, -200.0, 1200.0, 800.0),
            frame(100.0, 100.0, 676.0, 505.0),
            frame(0.0, 38.0, 1920.0, 1200.0),
        ] {
            let once = fit(saved, &[BUILT_IN, PORTRAIT_LEFT], BUILT_IN);
            let twice = fit(once, &[BUILT_IN, PORTRAIT_LEFT], BUILT_IN);
            assert_eq!(once, twice, "{saved:?} should settle in one pass");
        }
    }

    #[test]
    fn every_fitted_frame_is_usably_on_screen() {
        // The invariant is visibility across the desktop as a whole, not
        // containment in one display: a frame may legitimately span two.
        let areas = [BUILT_IN, PORTRAIT_LEFT];
        for saved in [
            frame(-613.0, -129.0, 1373.0, 780.0),
            frame(-9000.0, -9000.0, 1373.0, 780.0),
            frame(9000.0, 9000.0, 1373.0, 780.0),
            frame(0.0, 0.0, 1.0, 1.0),
            frame(-1200.0, 200.0, 1373.0, 780.0),
        ] {
            let fitted = fit(saved, &areas, BUILT_IN);
            let visible: f64 = areas.iter().map(|area| area.overlap(fitted)).sum();
            assert!(
                visible >= fitted.width * fitted.height * MIN_VISIBLE_FRACTION,
                "{saved:?} produced {fitted:?}, which is not usably on screen"
            );
        }
    }

    #[test]
    fn no_displays_at_all_falls_back_to_the_primary_that_was_passed_in() {
        let fitted = fit(frame(-613.0, -129.0, 1373.0, 780.0), &[], BUILT_IN);

        assert_eq!(fitted.x, (1512.0 - 1373.0) / 2.0);
        assert_eq!(fitted.y, 38.0 + (944.0 - 780.0) / 2.0);
    }
}
