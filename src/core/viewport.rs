use crate::core::render_budget::FrameBudgetPolicy;
use crate::renderer::RenderViewport;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewportMode {
    Fullscreen,
    CinemaScope,
}

pub(crate) const CINEMASCOPE_ASPECT: f64 = 2.39;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewportLayout {
    pub terminal_cols: u16,
    pub terminal_rows: u16,
    pub offset_x: u16,
    pub offset_y: u16,
    pub pixel_width: u32,
    pub pixel_height: u32,
}

impl ViewportLayout {
    pub(crate) fn calculate(
        terminal_cols: u16,
        terminal_rows: u16,
        viewport_mode: ViewportMode,
        requested_width: Option<u32>,
        requested_height: Option<u32>,
        budget_policy: FrameBudgetPolicy,
        source_aspect: f64,
        pixel_aspect_correction: f64,
    ) -> Self {
        let terminal_cols = terminal_cols.max(1);
        let terminal_rows = terminal_rows.max(1);

        let max_pixel_width = terminal_cols as u32;
        let max_pixel_height = (terminal_rows as u32).saturating_mul(2).max(2);

        let (pixel_width, pixel_height) = match viewport_mode {
            ViewportMode::Fullscreen => budget_policy.apply_to_dimensions(
                requested_width
                    .map(|value| value.min(max_pixel_width).max(1))
                    .unwrap_or(max_pixel_width),
                requested_height
                    .map(|value| value.min(max_pixel_height).max(2))
                    .unwrap_or(max_pixel_height),
                viewport_mode,
            ),
            ViewportMode::CinemaScope => {
                let cinema_pixel_aspect =
                    corrected_pixel_aspect(CINEMASCOPE_ASPECT, pixel_aspect_correction);
                let fitted_width = max_pixel_width;
                let fitted_height = make_even(
                    ((fitted_width as f64 / cinema_pixel_aspect).floor() as u32)
                        .min(max_pixel_height)
                        .max(2),
                );

                let (bounded_width, bounded_height) = if fitted_height > max_pixel_height {
                    let height = max_pixel_height;
                    let width = ((height as f64 * cinema_pixel_aspect).floor() as u32)
                        .min(max_pixel_width)
                        .max(1);
                    (width, height)
                } else {
                    (fitted_width, fitted_height)
                };

                let limit_width = requested_width
                    .map(|value| value.min(bounded_width).max(1))
                    .unwrap_or(bounded_width);
                let limit_height = requested_height
                    .map(|value| value.min(bounded_height).max(2))
                    .unwrap_or(bounded_height);

                let (width, height) = fit_aspect(limit_width, limit_height, cinema_pixel_aspect);
                budget_policy.apply_to_dimensions(width, height, viewport_mode)
            }
        };

        let char_width = pixel_width as u16;
        let char_height = (pixel_height / 2) as u16;
        let offset_x = (terminal_cols.saturating_sub(char_width)) / 2;
        let offset_y = (terminal_rows.saturating_sub(char_height)) / 2;

        Self {
            terminal_cols,
            terminal_rows,
            offset_x,
            offset_y,
            pixel_width,
            pixel_height,
        }
    }

    pub(crate) fn recentered_for_terminal(self, terminal_cols: u16, terminal_rows: u16) -> Self {
        let terminal_cols = terminal_cols.max(1);
        let terminal_rows = terminal_rows.max(1);
        let char_width = self.pixel_width as u16;
        let char_height = (self.pixel_height / 2) as u16;

        Self {
            terminal_cols,
            terminal_rows,
            offset_x: (terminal_cols.saturating_sub(char_width)) / 2,
            offset_y: (terminal_rows.saturating_sub(char_height)) / 2,
            ..self
        }
    }

    pub(crate) fn as_render_viewport(self) -> RenderViewport {
        RenderViewport {
            offset_x: self.offset_x,
            offset_y: self.offset_y,
            terminal_cols: self.terminal_cols,
            terminal_rows: self.terminal_rows,
            pixel_width: self.pixel_width,
            pixel_height: self.pixel_height,
        }
    }
}

pub(crate) fn corrected_pixel_aspect(visual_aspect: f64, pixel_aspect_correction: f64) -> f64 {
    let correction = if pixel_aspect_correction.is_finite() && pixel_aspect_correction > 0.0 {
        pixel_aspect_correction
    } else {
        1.0
    };
    (visual_aspect / correction).max(0.1)
}

pub(crate) fn fit_aspect(max_width: u32, max_height: u32, aspect: f64) -> (u32, u32) {
    let max_width = max_width.max(1);
    let max_height = make_even(max_height.max(2));
    let aspect = if aspect.is_finite() && aspect > 0.0 {
        aspect
    } else {
        CINEMASCOPE_ASPECT
    };
    let width_from_height = ((max_height as f64) * aspect).floor() as u32;

    if width_from_height <= max_width {
        (width_from_height.max(1), max_height)
    } else {
        let width = max_width;
        let height = make_even(((width as f64) / aspect).floor() as u32).max(2);
        (width.max(1), height)
    }
}

pub(crate) fn make_even(value: u32) -> u32 {
    let clamped = value.max(2);
    if clamped % 2 == 0 {
        clamped
    } else {
        clamped - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::render_budget::{FrameBudgetPolicy, RenderQuality};
    use crate::renderer::{ActiveRenderBackend, DisplayMode};

    #[test]
    fn cinema_layout_keeps_cinemascope_ratio() {
        let layout = ViewportLayout::calculate(
            240,
            68,
            ViewportMode::CinemaScope,
            None,
            None,
            FrameBudgetPolicy::for_backend(
                DisplayMode::Rgb,
                ActiveRenderBackend::AnsiRgb,
                RenderQuality::Full,
            ),
            16.0 / 9.0,
            1.0,
        );
        let ratio = layout.pixel_width as f64 / layout.pixel_height as f64;
        assert!((ratio - CINEMASCOPE_ASPECT).abs() < 0.05);
    }

    #[test]
    fn cinema_layout_compensates_for_narrow_ascii_glyphs() {
        let correction = 0.5;
        let layout = ViewportLayout::calculate(
            240,
            68,
            ViewportMode::CinemaScope,
            None,
            None,
            FrameBudgetPolicy::for_backend(
                DisplayMode::Ascii,
                ActiveRenderBackend::AnsiAscii,
                RenderQuality::Full,
            ),
            16.0 / 9.0,
            correction,
        );

        let visual_ratio = (layout.pixel_width as f64 / layout.pixel_height as f64) * correction;
        assert!((visual_ratio - CINEMASCOPE_ASPECT).abs() < 0.05);
    }

    #[test]
    fn fullscreen_layout_uses_requested_limits_without_preserving_aspect() {
        let layout = ViewportLayout::calculate(
            240,
            68,
            ViewportMode::Fullscreen,
            Some(120),
            Some(80),
            FrameBudgetPolicy::for_backend(
                DisplayMode::Rgb,
                ActiveRenderBackend::AnsiRgb,
                RenderQuality::Full,
            ),
            16.0 / 9.0,
            1.0,
        );
        assert_eq!(layout.pixel_width, 120);
        assert_eq!(layout.pixel_height, 80);
    }

    #[test]
    fn fullscreen_layout_uses_entire_terminal_canvas() {
        let layout = ViewportLayout::calculate(
            320,
            120,
            ViewportMode::Fullscreen,
            None,
            None,
            FrameBudgetPolicy::for_backend(
                DisplayMode::Rgb,
                ActiveRenderBackend::AnsiRgb,
                RenderQuality::Full,
            ),
            16.0 / 9.0,
            1.0,
        );
        assert_eq!(layout.pixel_width, 320);
        assert_eq!(layout.pixel_height, 240);
        assert_eq!(layout.offset_x, 0);
        assert_eq!(layout.offset_y, 0);
    }

    #[test]
    fn fullscreen_layout_ignores_non_16_9_source_aspect() {
        let layout = ViewportLayout::calculate(
            320,
            120,
            ViewportMode::Fullscreen,
            None,
            None,
            FrameBudgetPolicy::for_backend(
                DisplayMode::Rgb,
                ActiveRenderBackend::AnsiRgb,
                RenderQuality::Full,
            ),
            4.0 / 3.0,
            1.0,
        );
        assert_eq!(layout.pixel_width, 320);
        assert_eq!(layout.pixel_height, 240);
    }
}
