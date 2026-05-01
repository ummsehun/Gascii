use crate::core::viewport::{make_even, ViewportMode};
use crate::renderer::{ActiveRenderBackend, DisplayMode};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum RenderQuality {
    Full,
    Balanced,
    Performance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameBudgetPolicy {
    pub quality: RenderQuality,
    pub max_render_cells: u32,
    pub drop_threshold: Duration,
}

impl FrameBudgetPolicy {
    pub fn for_backend(
        mode: DisplayMode,
        backend: ActiveRenderBackend,
        quality: RenderQuality,
    ) -> Self {
        match (quality, mode, backend) {
            (RenderQuality::Full, _, _) => Self {
                quality,
                max_render_cells: u32::MAX,
                drop_threshold: Duration::from_millis(90),
            },
            (RenderQuality::Balanced, DisplayMode::Ascii, _) => Self {
                quality,
                max_render_cells: u32::MAX,
                drop_threshold: Duration::from_millis(90),
            },
            (RenderQuality::Balanced, DisplayMode::Rgb, _) => Self {
                quality,
                max_render_cells: 24_000,
                drop_threshold: Duration::from_millis(75),
            },
            (RenderQuality::Performance, DisplayMode::Ascii, _) => Self {
                quality,
                max_render_cells: 24_000,
                drop_threshold: Duration::from_millis(75),
            },
            (RenderQuality::Performance, DisplayMode::Rgb, _) => Self {
                quality,
                max_render_cells: 18_000,
                drop_threshold: Duration::from_millis(60),
            },
        }
    }

    pub(crate) fn apply_to_dimensions(
        self,
        width: u32,
        height: u32,
        mode: ViewportMode,
    ) -> (u32, u32) {
        if self.max_render_cells == u32::MAX {
            return match mode {
                ViewportMode::Fullscreen => (width.max(1), make_even(height.max(2))),
                ViewportMode::CinemaScope => (width.max(1), make_even(height.max(2))),
            };
        }

        let current_cells = width.saturating_mul((height / 2).max(1));
        if current_cells <= self.max_render_cells {
            return (width.max(1), make_even(height.max(2)));
        }

        let scale = (self.max_render_cells as f64 / current_cells as f64).sqrt();
        let scaled_width = ((width as f64) * scale).floor() as u32;
        let scaled_height = ((height as f64) * scale).floor() as u32;
        let scaled_width = scaled_width.max(1);
        let scaled_height = make_even(scaled_height.max(2));

        match mode {
            ViewportMode::Fullscreen => (scaled_width, scaled_height),
            ViewportMode::CinemaScope => (scaled_width, scaled_height),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::viewport::ViewportLayout;

    #[test]
    fn balanced_quality_scales_down_large_rgb_viewports() {
        let layout = ViewportLayout::calculate(
            320,
            120,
            ViewportMode::Fullscreen,
            None,
            None,
            FrameBudgetPolicy::for_backend(
                DisplayMode::Rgb,
                ActiveRenderBackend::AnsiRgb,
                RenderQuality::Balanced,
            ),
            16.0 / 9.0,
            1.0,
        );
        let cells = layout.pixel_width * (layout.pixel_height / 2);
        assert!(cells <= 24_000);
    }

    #[test]
    fn performance_quality_scales_down_large_rgb_viewports() {
        let layout = ViewportLayout::calculate(
            320,
            120,
            ViewportMode::Fullscreen,
            None,
            None,
            FrameBudgetPolicy::for_backend(
                DisplayMode::Rgb,
                ActiveRenderBackend::AnsiRgb,
                RenderQuality::Performance,
            ),
            16.0 / 9.0,
            1.0,
        );
        let cells = layout.pixel_width * (layout.pixel_height / 2);
        assert!(cells <= 18_000);
    }
}
