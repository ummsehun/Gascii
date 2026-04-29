use crate::core::render_budget::FrameBudgetPolicy;
use crate::renderer::RenderViewport;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewportMode {
    Fullscreen,
    Cinema16x9,
}

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
    ) -> Self {
        let terminal_cols = terminal_cols.max(1);
        let terminal_rows = terminal_rows.max(1);

        let max_pixel_width = terminal_cols as u32;
        let max_pixel_height = (terminal_rows as u32).saturating_mul(2).max(2);

        let (pixel_width, pixel_height) = match viewport_mode {
            ViewportMode::Fullscreen => {
                let width = requested_width
                    .map(|value| value.min(max_pixel_width).max(1))
                    .unwrap_or(max_pixel_width);
                let height = requested_height
                    .map(|value| value.min(max_pixel_height).max(2))
                    .unwrap_or(max_pixel_height);
                let (width, height) = fit_aspect(width, height, source_aspect);
                budget_policy.apply_to_dimensions(width, height, viewport_mode)
            }
            ViewportMode::Cinema16x9 => {
                let fitted_width = max_pixel_width;
                let fitted_height = make_even(
                    ((fitted_width as f64 / (16.0 / 9.0)).floor() as u32)
                        .min(max_pixel_height)
                        .max(2),
                );

                let (bounded_width, bounded_height) = if fitted_height > max_pixel_height {
                    let height = max_pixel_height;
                    let width = ((height as f64 * (16.0 / 9.0)).floor() as u32)
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

                let (width, height) = fit_aspect_16_9(limit_width, limit_height);
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

pub(crate) fn fit_aspect_16_9(max_width: u32, max_height: u32) -> (u32, u32) {
    fit_aspect(max_width, max_height, 16.0 / 9.0)
}

pub(crate) fn fit_aspect(max_width: u32, max_height: u32, aspect: f64) -> (u32, u32) {
    let max_width = max_width.max(1);
    let max_height = make_even(max_height.max(2));
    let aspect = if aspect.is_finite() && aspect > 0.0 {
        aspect
    } else {
        16.0 / 9.0
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
    fn cinema_layout_keeps_16_9_ratio() {
        let layout = ViewportLayout::calculate(
            240,
            68,
            ViewportMode::Cinema16x9,
            None,
            None,
            FrameBudgetPolicy::for_backend(
                DisplayMode::Rgb,
                ActiveRenderBackend::AnsiRgb,
                RenderQuality::Full,
            ),
            16.0 / 9.0,
        );
        let ratio = layout.pixel_width as f64 / layout.pixel_height as f64;
        assert!((ratio - (16.0 / 9.0)).abs() < 0.05);
    }

    #[test]
    fn fullscreen_layout_uses_requested_limits() {
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
        );
        assert_eq!(layout.pixel_width, 120);
        assert_eq!(layout.pixel_height, 66);
    }

    #[test]
    fn ansi_rgb_uses_full_source_aspect_viewport_resolution() {
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
        );
        assert_eq!(layout.pixel_width, 320);
        assert_eq!(layout.pixel_height, 180);
    }

    #[test]
    fn fullscreen_layout_preserves_non_16_9_source_aspect() {
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
        );
        assert_eq!(layout.pixel_width, 320);
        assert_eq!(layout.pixel_height, 240);
    }
}
