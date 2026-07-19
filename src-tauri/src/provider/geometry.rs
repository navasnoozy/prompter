use serde::Deserialize;
use tauri::{LogicalPosition, LogicalSize, Rect, Runtime, Window};

use super::error::{ProviderCommandError, ProviderErrorCode};

pub(crate) const MIN_PROVIDER_SIZE: f64 = 240.0;
pub(crate) const MAX_PROVIDER_SIZE: f64 = 20_000.0;

/// Used when the live title-bar height cannot be derived from the window.
const FALLBACK_CONTENT_OFFSET_Y: f64 = 32.0;
const MAX_CONTENT_OFFSET_Y: f64 = 64.0;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderBounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl ProviderBounds {
    /// `content_offset_y` is the height of the native title bar: frontend
    /// coordinates are relative to the web content area while child webviews
    /// are positioned relative to the window frame.
    pub(crate) fn validate(
        self,
        content_offset_y: f64,
    ) -> Result<ValidatedBounds, ProviderCommandError> {
        if !self.x.is_finite()
            || !self.y.is_finite()
            || !self.width.is_finite()
            || !self.height.is_finite()
            || self.width < MIN_PROVIDER_SIZE
            || self.height < MIN_PROVIDER_SIZE
            || self.width > MAX_PROVIDER_SIZE
            || self.height > MAX_PROVIDER_SIZE
        {
            return Err(ProviderCommandError::new(
                ProviderErrorCode::InvalidBounds,
                "The embedded browser area is not ready yet.",
            ));
        }

        Ok(ValidatedBounds {
            x: self.x.max(0.0),
            y: self.y.max(0.0) + content_offset_y,
            width: self.width,
            height: self.height,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ValidatedBounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl From<ValidatedBounds> for Rect {
    fn from(bounds: ValidatedBounds) -> Self {
        Self {
            position: LogicalPosition::new(bounds.x, bounds.y).into(),
            size: LogicalSize::new(bounds.width, bounds.height).into(),
        }
    }
}

/// Derives the title-bar height from the window's frame/content delta,
/// falling back to the historical constant when it cannot be read.
pub(crate) fn content_offset_y<R: Runtime>(window: &Window<R>) -> f64 {
    derive_offset(window).unwrap_or(FALLBACK_CONTENT_OFFSET_Y)
}

fn derive_offset<R: Runtime>(window: &Window<R>) -> Option<f64> {
    let inner = window.inner_position().ok()?;
    let outer = window.outer_position().ok()?;
    let scale = window.scale_factor().ok()?;
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    clamp_offset(f64::from(inner.y - outer.y) / scale)
}

fn clamp_offset(raw: f64) -> Option<f64> {
    (raw.is_finite() && (0.0..=MAX_CONTENT_OFFSET_Y).contains(&raw)).then_some(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_reject_non_finite_out_of_range_values() {
        let valid = ProviderBounds {
            x: 0.0,
            y: 0.0,
            width: MIN_PROVIDER_SIZE,
            height: MIN_PROVIDER_SIZE,
        };
        assert!(valid.validate(FALLBACK_CONTENT_OFFSET_Y).is_ok());

        for invalid in [
            ProviderBounds {
                x: f64::NAN,
                ..valid
            },
            ProviderBounds {
                y: f64::INFINITY,
                ..valid
            },
            ProviderBounds {
                width: MIN_PROVIDER_SIZE - 1.0,
                ..valid
            },
            ProviderBounds {
                height: 0.0,
                ..valid
            },
            ProviderBounds {
                width: MAX_PROVIDER_SIZE + 1.0,
                ..valid
            },
            ProviderBounds {
                height: f64::MAX,
                ..valid
            },
        ] {
            assert!(invalid.validate(FALLBACK_CONTENT_OFFSET_Y).is_err());
        }
    }

    #[test]
    fn validated_bounds_apply_the_title_bar_offset_and_clamp_origin() {
        let bounds = ProviderBounds {
            x: -5.0,
            y: 10.0,
            width: 400.0,
            height: 300.0,
        };

        let validated = bounds.validate(32.0).expect("bounds should validate");

        assert_eq!(
            validated,
            ValidatedBounds {
                x: 0.0,
                y: 42.0,
                width: 400.0,
                height: 300.0,
            }
        );
    }

    #[test]
    fn offset_clamp_accepts_realistic_title_bars_and_rejects_garbage() {
        assert_eq!(clamp_offset(0.0), Some(0.0));
        assert_eq!(clamp_offset(28.0), Some(28.0));
        assert_eq!(
            clamp_offset(MAX_CONTENT_OFFSET_Y),
            Some(MAX_CONTENT_OFFSET_Y)
        );
        assert_eq!(clamp_offset(-1.0), None);
        assert_eq!(clamp_offset(MAX_CONTENT_OFFSET_Y + 1.0), None);
        assert_eq!(clamp_offset(f64::NAN), None);
        assert_eq!(clamp_offset(f64::INFINITY), None);
    }
}
