//! Terminal rendering module
//!
//! This module provides the Terminal widget and all supporting rendering infrastructure.

pub mod shader;
pub use shader::*;

pub mod view;
pub use view::*;

pub mod crt_program;
pub use crt_program::*;

pub mod crt_state;
pub use crt_state::*;

pub mod tile_cache;
pub use tile_cache::*;

pub mod shared_render_cache;
pub use shared_render_cache::*;

pub mod render_info;
pub use render_info::*;

use parking_lot::{Mutex, RwLock};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::ScalingMode;
use crate::{EditorMarkers, ScrollbarState, Viewport};
use iced::{Color, mouse, widget};
use icy_engine::Screen;

pub struct Terminal {
    pub screen: Arc<Mutex<Box<dyn Screen>>>,
    pub original_screen: Option<Arc<Mutex<Box<dyn Screen>>>>,
    pub viewport: Arc<RwLock<Viewport>>,
    pub scrollbar: ScrollbarState,
    pub scrollbar_hover_state: Arc<AtomicBool>,  // Shared atomic hover state for vertical scrollbar
    pub hscrollbar_hover_state: Arc<AtomicBool>, // Shared atomic hover state for horizontal scrollbar
    /// Shared render information for mouse mapping
    /// Updated by the shader, read by mouse event handlers
    pub render_info: Arc<RwLock<RenderInfo>>,
    /// Cursor icon to display (set by shader based on hover state)
    /// None = default cursor, Some(Interaction) = custom cursor (e.g. hand for links)
    pub cursor_icon: Arc<RwLock<Option<mouse::Interaction>>>,
    pub font_size: f32,
    pub char_width: f32,
    pub char_height: f32,
    pub id: widget::Id,
    pub has_focus: bool,
    pub background_color: Arc<RwLock<[f32; 4]>>,
    /// Shared render cache for tiles - accessible by both Terminal shader and Minimap
    pub render_cache: SharedRenderCacheHandle,
    /// Editor markers (raster grid, guide crosshair, reference image)
    pub markers: Arc<RwLock<EditorMarkers>>,

    /// If enabled, the terminal's *window* height (TerminalState height) is adjusted to the
    /// available widget height. This changes `screen.resolution()` but does NOT resize the buffer.
    ///
    /// Intended for viewer/editor apps (e.g. `icy_view`, `icy_draw`). `icy_term` should typically
    /// keep program-controlled sizing.
    pub fit_terminal_height_to_bounds: bool,
}

impl Terminal {
    pub fn new(screen: Arc<Mutex<Box<dyn Screen>>>) -> Self {
        // Initialize viewport with screen size
        let viewport = {
            let scr = screen.lock();
            let virtual_size = scr.virtual_size();
            let resolution = scr.resolution();
            Arc::new(RwLock::new(Viewport::new(resolution, virtual_size)))
        };

        Self {
            screen,
            original_screen: None,
            viewport,
            scrollbar: ScrollbarState::new(),
            scrollbar_hover_state: Arc::new(AtomicBool::new(false)),
            hscrollbar_hover_state: Arc::new(AtomicBool::new(false)),
            render_info: RenderInfo::new_shared(),
            cursor_icon: Arc::new(RwLock::new(None)),
            font_size: 16.0,
            char_width: 9.6, // Approximate for monospace
            char_height: 20.0,
            id: widget::Id::unique(),
            has_focus: false,
            background_color: Arc::new(RwLock::new([0.1, 0.1, 0.12, 1.0])), // Default dark background
            render_cache: create_shared_render_cache(),
            markers: Arc::new(RwLock::new(EditorMarkers::default())),

            fit_terminal_height_to_bounds: false,
        }
    }

    /// Enable/disable automatic adjustment of the terminal window height to the widget bounds.
    pub fn set_fit_terminal_height_to_bounds(&mut self, enabled: bool) {
        self.fit_terminal_height_to_bounds = enabled;
    }

    /// Update viewport when screen size changes
    pub fn update_viewport_size(&mut self) {
        let scr = self.screen.lock();
        let virtual_size = scr.virtual_size();
        drop(scr);

        {
            let mut vp = self.viewport.write();
            // Only update content size, not visible size (which is the widget size, not screen size)
            vp.set_content_size(virtual_size.width as f32, virtual_size.height as f32);
        }
        // Sync scrollbar position with viewport (after the lock is dropped)
        self.sync_scrollbar_with_viewport();
    }

    /// Update viewport visible size based on available widget dimensions
    /// Call this when the widget size changes to properly calculate scrollbar
    pub fn set_viewport_visible_size(&mut self, width: f32, height: f32) {
        self.viewport.write().set_visible_size(width, height);
        self.sync_scrollbar_with_viewport();
    }

    /// Sync scrollbar state with viewport scroll position
    /// scroll_x/y are now in CONTENT coordinates
    /// max_scroll = content_size - visible_content_size (in content pixels)
    pub fn sync_scrollbar_with_viewport(&mut self) {
        let mut vp = self.viewport.write();

        // Use viewport's max_scroll methods which use shader-computed values if available
        let max_scroll_x = vp.max_scroll_x();
        let max_scroll_y = vp.max_scroll_y();

        vp.scroll_x = vp.scroll_x.clamp(0.0, max_scroll_x);
        vp.scroll_y = vp.scroll_y.clamp(0.0, max_scroll_y);
        vp.target_scroll_x = vp.target_scroll_x.clamp(0.0, max_scroll_x);
        vp.target_scroll_y = vp.target_scroll_y.clamp(0.0, max_scroll_y);

        // Vertical scrollbar
        if max_scroll_y > 0.0 {
            let scroll_ratio = vp.scroll_y / max_scroll_y;
            self.scrollbar.set_scroll_position(scroll_ratio.clamp(0.0, 1.0));
        } else {
            self.scrollbar.set_scroll_position(0.0);
        }

        // Horizontal scrollbar
        if max_scroll_x > 0.0 {
            let scroll_ratio_x = vp.scroll_x / max_scroll_x;
            self.scrollbar.set_scroll_position_x(scroll_ratio_x.clamp(0.0, 1.0));
        } else {
            self.scrollbar.set_scroll_position_x(0.0);
        }
    }

    /// Get maximum scroll Y value in content coordinates
    pub fn max_scroll_y(&self) -> f32 {
        self.viewport.read().max_scroll_y()
    }

    /// Scroll X by delta with proper clamping (delta in content coordinates)
    pub fn scroll_x_by(&mut self, dx: f32) {
        let mut vp = self.viewport.write();
        vp.scroll_x_by(dx);
    }

    /// Scroll Y by delta with proper clamping (delta in content coordinates)
    pub fn scroll_y_by(&mut self, dy: f32) {
        let mut vp = self.viewport.write();
        vp.scroll_y_by(dy);
    }

    /// Scroll X by delta with smooth animation (for PageUp/PageDown)
    pub fn scroll_x_by_smooth(&mut self, dx: f32) {
        let mut vp = self.viewport.write();
        vp.scroll_x_by_smooth(dx);
    }

    /// Scroll Y by delta with smooth animation (for PageUp/PageDown)
    pub fn scroll_y_by_smooth(&mut self, dy: f32) {
        let mut vp = self.viewport.write();
        vp.scroll_y_by_smooth(dy);
    }

    /// Scroll X to position with proper clamping (position in content coordinates)
    pub fn scroll_x_to(&mut self, x: f32) {
        let mut vp = self.viewport.write();
        vp.scroll_x_to(x);
    }

    /// Scroll Y to position with proper clamping (position in content coordinates)
    pub fn scroll_y_to(&mut self, y: f32) {
        let mut vp = self.viewport.write();
        vp.scroll_y_to(y);
    }

    /// Scroll X to position with smooth animation (for Home/End/PageUp/PageDown)
    pub fn scroll_x_to_smooth(&mut self, x: f32) {
        let mut vp = self.viewport.write();
        vp.scroll_x_to_smooth(x);
    }

    /// Scroll Y to position with smooth animation (for Home/End/PageUp/PageDown)
    pub fn scroll_y_to_smooth(&mut self, y: f32) {
        let mut vp = self.viewport.write();
        vp.scroll_y_to_smooth(y);
    }

    pub fn is_in_scrollback_mode(&self) -> bool {
        self.original_screen.is_some()
    }

    pub fn enter_scrollback_mode(&mut self, scrollback: Arc<Mutex<Box<dyn Screen>>>) {
        if self.original_screen.is_none() {
            // Save the original screen
            self.original_screen = Some(self.screen.clone());
            // Switch to scrollback
            self.screen = scrollback;
            // Update viewport for scrollback content
            self.update_viewport_size();

            // Get the resolution to use as visible size for scrolling calculations
            let scr = self.screen.lock();
            let resolution = scr.resolution();
            drop(scr);

            {
                let mut vp = self.viewport.write();
                // Use resolution as visible size and scroll to bottom immediately (no animation)
                // max_scroll is now in content coordinates
                let visible_content_height = resolution.height as f32 / vp.zoom;
                let max_scroll_y = (vp.content_height - visible_content_height).max(0.0);
                vp.scroll_x_to(0.0);
                vp.scroll_y_to(max_scroll_y);
                // Clamp with the correct visible size
                vp.clamp_scroll_with_size(resolution.width as f32, resolution.height as f32);
            }

            // Sync scrollbar position with the new viewport position
            self.sync_scrollbar_with_viewport();
        }
    }

    pub fn exit_scrollback_mode(&mut self) {
        if let Some(original) = self.original_screen.take() {
            self.screen = original;
            // Update viewport back to normal content
            self.update_viewport_size();
            // Sync scrollbar position when exiting scrollback
            self.sync_scrollbar_with_viewport();
        }
    }

    /// Minimum zoom level (50%)
    pub const MIN_ZOOM: f32 = 0.5;
    /// Maximum zoom level (400%)
    pub const MAX_ZOOM: f32 = 4.0;
    /// Zoom step for each zoom in/out action (25%)
    pub const ZOOM_STEP: f32 = 0.25;
    /// Zoom step for integer scaling
    pub const ZOOM_STEP_INT: f32 = 1.0;

    /// Get current zoom level
    pub fn get_zoom(&self) -> f32 {
        self.viewport.read().zoom
    }

    /// Set zoom level with clamping
    pub fn set_zoom(&mut self, zoom: f32) {
        let clamped_zoom = zoom.clamp(Self::MIN_ZOOM, Self::MAX_ZOOM);
        let mut vp = self.viewport.write();

        // Keep the center of the view stable when zooming
        let center_x = vp.visible_width / 2.0;
        let center_y = vp.visible_height / 2.0;
        vp.set_zoom(clamped_zoom, center_x, center_y);
        vp.changed.store(true, std::sync::atomic::Ordering::Relaxed);
        drop(vp);

        self.sync_scrollbar_with_viewport();
    }

    /// Zoom in by one step (respects integer scaling setting)
    pub fn zoom_in(&mut self) {
        let current = self.get_zoom();
        self.set_zoom(current + Self::ZOOM_STEP);
    }

    /// Zoom in by integer step (for integer scaling mode)
    pub fn zoom_in_int(&mut self) {
        let current = self.get_zoom();
        self.set_zoom((current + Self::ZOOM_STEP_INT).floor());
    }

    /// Zoom out by one step
    pub fn zoom_out(&mut self) {
        let current = self.get_zoom();
        self.set_zoom(current - Self::ZOOM_STEP);
    }

    /// Zoom out by integer step (for integer scaling mode)
    pub fn zoom_out_int(&mut self) {
        let current = self.get_zoom();
        self.set_zoom((current - Self::ZOOM_STEP_INT).ceil().max(1.0));
    }

    /// Reset zoom to 100% (1:1 pixel mapping)
    pub fn zoom_reset(&mut self) {
        self.set_zoom(1.0);
    }

    /// Calculate and set auto-fit zoom based on content and viewport size
    /// Returns the calculated zoom factor
    pub fn zoom_auto_fit(&mut self, use_integer_scaling: bool) -> f32 {
        let vp: parking_lot::lock_api::RwLockReadGuard<'_, parking_lot::RawRwLock, Viewport> = self.viewport.read();
        let content_width = vp.content_width;
        let content_height = vp.content_height;
        let visible_width = vp.visible_width;
        let visible_height = vp.visible_height;
        drop(vp);

        let zoom = ScalingMode::Auto.compute_zoom(content_width, content_height, visible_width, visible_height, use_integer_scaling);

        self.set_zoom(zoom);
        zoom
    }

    pub fn reset_caret_blink(&mut self) {}

    // Helper function to convert buffer color to iced Color
    pub fn buffer_color_to_iced(color: icy_engine::Color) -> Color {
        let (r, g, b) = color.get_rgb_f32();
        Color::from_rgb(r, g, b)
    }

    /// Set the background color for out-of-bounds areas in CRT shader
    pub fn set_background_color(&self, color: Color) {
        *self.background_color.write() = [color.r, color.g, color.b, color.a];
    }

    /// Get the current background color
    pub fn get_background_color(&self) -> Color {
        let bg = *self.background_color.read();
        Color::from_rgba(bg[0], bg[1], bg[2], bg[3])
    }
}
