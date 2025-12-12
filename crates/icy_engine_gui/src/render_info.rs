use parking_lot::RwLock;
use std::sync::Arc;

/// Shared render information calculated by the shader and used by mouse mapping.
/// This ensures both use the exact same values, avoiding calculation mismatches.
#[derive(Debug, Clone, Default)]
pub struct RenderInfo {
    /// The display scale factor applied to the terminal texture
    pub display_scale: f32,
    /// Viewport X offset (where rendering starts in screen coordinates)
    pub viewport_x: f32,
    /// Viewport Y offset (where rendering starts in screen coordinates)
    pub viewport_y: f32,
    /// Viewport width in screen pixels
    pub viewport_width: f32,
    /// Viewport height in screen pixels
    pub viewport_height: f32,
    /// Terminal texture width in content pixels
    pub terminal_width: f32,
    /// Terminal texture height in content pixels
    pub terminal_height: f32,
    /// Font width in pixels
    pub font_width: f32,
    /// Font height in pixels
    pub font_height: f32,
    /// Whether scanlines are enabled (doubles vertical resolution)
    pub scan_lines: bool,
    /// The bounds of the widget (for coordinate calculations)
    pub bounds_x: f32,
    pub bounds_y: f32,
    pub bounds_width: f32,
    pub bounds_height: f32,
}

impl RenderInfo {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new Arc<RwLock<RenderInfo>> for sharing between shader and mouse mapping
    pub fn new_shared() -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(Self::default()))
    }

    /// Convert screen coordinates to terminal-local pixel coordinates
    /// Returns None if the coordinates are outside the rendered area
    pub fn screen_to_terminal_pixels(&self, screen_x: f32, screen_y: f32) -> Option<(f32, f32)> {
        // Convert to widget-local coordinates
        let local_x = screen_x - self.bounds_x;
        let local_y = screen_y - self.bounds_y;

        // Check if within viewport
        let vp_local_x = local_x - self.viewport_x;
        let vp_local_y = local_y - self.viewport_y;

        if vp_local_x < 0.0 || vp_local_y < 0.0 || vp_local_x >= self.viewport_width || vp_local_y >= self.viewport_height {
            return None;
        }

        // Convert from screen pixels to terminal pixels
        let term_x = vp_local_x / self.display_scale;
        let term_y = vp_local_y / self.display_scale;

        Some((term_x, term_y))
    }

    /// Convert screen coordinates to cell position
    /// Returns None if the coordinates are outside the rendered area or font info is missing
    pub fn screen_to_cell(&self, screen_x: f32, screen_y: f32) -> Option<(i32, i32)> {
        if self.font_width <= 0.0 || self.font_height <= 0.0 {
            return None;
        }

        let (term_x, mut term_y) = self.screen_to_terminal_pixels(screen_x, screen_y)?;

        // Handle scanlines (doubled vertical resolution)
        let effective_font_height = if self.scan_lines {
            term_y /= 2.0;
            self.font_height
        } else {
            self.font_height
        };

        let cell_x = (term_x / self.font_width).floor() as i32;
        let cell_y = (term_y / effective_font_height).floor() as i32;

        Some((cell_x, cell_y))
    }

    /// Convert screen coordinates to terminal-local pixel coordinates for dragging.
    /// Unlike screen_to_terminal_pixels, this allows coordinates outside the viewport
    /// for smooth drag operations that extend beyond the canvas bounds.
    pub fn screen_to_terminal_pixels_unclamped(&self, screen_x: f32, screen_y: f32) -> (f32, f32) {
        // Convert to widget-local coordinates
        let local_x = screen_x - self.bounds_x;
        let local_y = screen_y - self.bounds_y;

        // Calculate position relative to viewport (can be negative or > viewport size)
        let vp_local_x = local_x - self.viewport_x;
        let vp_local_y = local_y - self.viewport_y;

        // Convert from screen pixels to terminal pixels (no bounds check)
        let term_x = vp_local_x / self.display_scale.max(0.001);
        let term_y = vp_local_y / self.display_scale.max(0.001);

        (term_x, term_y)
    }
}
