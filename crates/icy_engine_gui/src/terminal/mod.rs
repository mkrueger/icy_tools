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
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crate::EditorMarkers;
use icy_engine::Screen;
use icy_ui::{mouse, widget, Color, Rectangle, Task};

/// Scroll state for the terminal, sourced from `scroll_area().show_viewport()`.
///
/// Important: This is *not* an authorative scroll model. The scrollable widget owns scrolling.
/// We only cache the last viewport position so the renderer can sample the correct content region.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct TerminalScrollState {
    /// Horizontal scroll offset in *content pixels* (zoom 1.0).
    pub scroll_x: f32,
    /// Vertical scroll offset in *content pixels* (zoom 1.0).
    pub scroll_y: f32,
    /// Visible viewport width in *zoomed* pixels, as reported by `show_viewport`.
    pub viewport_width_px: f32,
    /// Visible viewport height in *zoomed* pixels, as reported by `show_viewport`.
    pub viewport_height_px: f32,
}

pub struct Terminal {
    pub screen: Arc<Mutex<Box<dyn Screen>>>,
    pub original_screen: Option<Arc<Mutex<Box<dyn Screen>>>>,
    scroll_state: Arc<RwLock<TerminalScrollState>>,
    scroll_area_id: widget::Id,
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
        Self {
            screen,
            original_screen: None,
            scroll_state: Arc::new(RwLock::new(TerminalScrollState::default())),
            scroll_area_id: widget::Id::unique(),
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

    /// Id of the surrounding `scroll_area` that should own scrolling for this terminal.
    /// Use this for programmatic scrolling via `icy_ui::widget::operation::scroll_to(_animated)`.
    pub fn scroll_area_id(&self) -> widget::Id {
        self.scroll_area_id.clone()
    }

    /// Create a task that scrolls the owning `scroll_area` to the given content coordinates.
    ///
    /// `x`/`y` are in content pixels at zoom 1.0. The task converts to the scroll area's
    /// coordinate system (zoomed pixels) using the last effective zoom.
    pub fn scroll_to_content<T>(&self, x: Option<f32>, y: Option<f32>) -> Task<T> {
        let zoom = self.get_zoom().max(0.001);
        let offset = icy_ui::widget::operation::AbsoluteOffset {
            x: x.map(|v| (v * zoom).max(0.0)),
            y: y.map(|v| (v * zoom).max(0.0)),
        };

        icy_ui::widget::operation::scroll_to(self.scroll_area_id.clone(), offset)
    }

    /// Like `scroll_to_content`, but animated.
    pub fn scroll_to_content_animated<T>(&self, x: Option<f32>, y: Option<f32>) -> Task<T> {
        let zoom = self.get_zoom().max(0.001);
        let offset = icy_ui::widget::operation::AbsoluteOffset {
            x: x.map(|v| (v * zoom).max(0.0)),
            y: y.map(|v| (v * zoom).max(0.0)),
        };

        icy_ui::widget::operation::scroll_to_animated(self.scroll_area_id.clone(), offset)
    }

    /// Update cached scroll offsets from `scroll_area().show_viewport(...)`.
    ///
    /// `viewport` is in the same coordinate system as the content size passed to
    /// `show_viewport` (typically *zoomed* pixels). `zoom` is the effective scale.
    pub fn update_scroll_from_viewport(&self, viewport: Rectangle, zoom: f32) {
        let zoom = zoom.max(0.001);
        let mut state = self.scroll_state.write();
        state.viewport_width_px = viewport.width.max(1.0);
        state.viewport_height_px = viewport.height.max(1.0);
        state.scroll_x = (viewport.x / zoom).max(0.0);
        state.scroll_y = (viewport.y / zoom).max(0.0);
    }

    /// Current cached scroll state (content coordinates).
    pub fn scroll_state(&self) -> TerminalScrollState {
        *self.scroll_state.read()
    }

    /// Enable/disable automatic adjustment of the terminal window height to the widget bounds.
    pub fn set_fit_terminal_height_to_bounds(&mut self, enabled: bool) {
        self.fit_terminal_height_to_bounds = enabled;
    }

    /// Update viewport when screen size changes
    pub fn update_viewport_size(&mut self) {
        // No-op: content size is derived from `Screen::virtual_size()` during rendering.
    }

    /// Update viewport visible size based on available widget dimensions
    /// Call this when the widget size changes to properly calculate scrollbar
    pub fn set_viewport_visible_size(&mut self, width: f32, height: f32) {
        let mut state = self.scroll_state.write();
        state.viewport_width_px = width.max(1.0);
        state.viewport_height_px = height.max(1.0);
    }

    /// Sync scrollbar state with viewport scroll position
    /// scroll_x/y are now in CONTENT coordinates
    /// max_scroll = content_size - visible_content_size (in content pixels)
    pub fn sync_scrollbar_with_viewport(&mut self) {
        // Deprecated: use `scroll_area` scrollbars.
    }

    /// Get maximum scroll Y value in content coordinates
    pub fn max_scroll_y(&self) -> f32 {
        let scr = self.screen.lock();
        let virtual_size = scr.virtual_size();
        let resolution = scr.resolution();
        drop(scr);
        (virtual_size.height as f32 - resolution.height as f32).max(0.0)
    }

    /// Current scroll X in content coordinates.
    pub fn scroll_x(&self) -> f32 {
        self.scroll_state.read().scroll_x
    }

    /// Current scroll Y in content coordinates.
    pub fn scroll_y(&self) -> f32 {
        self.scroll_state.read().scroll_y
    }

    /// Visible height in screen pixels (widget bounds).
    pub fn visible_height_px(&self) -> f32 {
        self.scroll_state.read().viewport_height_px
    }

    /// Visible width in screen pixels (widget bounds).
    pub fn visible_width_px(&self) -> f32 {
        self.scroll_state.read().viewport_width_px
    }

    /// Content height in content pixels (at zoom 1.0).
    pub fn content_height(&self) -> f32 {
        let scr = self.screen.lock();
        let h = scr.virtual_size().height as f32;
        drop(scr);
        h
    }

    /// Content width in content pixels (at zoom 1.0).
    pub fn content_width(&self) -> f32 {
        let scr = self.screen.lock();
        let w = scr.virtual_size().width as f32;
        drop(scr);
        w
    }

    /// Visible height in content pixels (derived from visible bounds and zoom).
    pub fn visible_content_height(&self) -> f32 {
        // Best-effort: derive from cached viewport size + effective zoom.
        self.visible_height_px() / self.get_zoom().max(0.001)
    }

    /// Visible width in content pixels (derived from visible bounds and zoom).
    pub fn visible_content_width(&self) -> f32 {
        self.visible_width_px() / self.get_zoom().max(0.001)
    }

    pub fn mark_viewport_changed(&self) {
        // No-op: scroll state is sourced from scroll_area.
    }

    pub fn update_viewport_animation(&mut self) {
        // No-op: smooth scrolling is handled by scrollable animations/tasks.
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

            // IMPORTANT: the terminal shader uses a shared tile cache.
            // When switching screens (live <-> scrollback) we must invalidate it,
            // otherwise cached textures from the previous screen will be reused and
            // the wrong content is displayed.
            self.render_cache.write().invalidate();

            // Update viewport for scrollback content
            self.update_viewport_size();
            // NOTE: Programmatic scrolling (e.g. to bottom) must be performed by the owning
            // view via `icy_ui::widget::operation::scroll_to(_animated)`.
        }
    }

    pub fn exit_scrollback_mode(&mut self) {
        if let Some(original) = self.original_screen.take() {
            self.screen = original;

            // See `enter_scrollback_mode`.
            self.render_cache.write().invalidate();

            // Update viewport back to normal content
            self.update_viewport_size();
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
        let z = self.render_info.read().display_scale;
        if z <= 0.0 {
            1.0
        } else {
            z
        }
    }

    /// Calculate and set auto-fit zoom based on content and viewport size
    /// Returns the calculated zoom factor
    pub fn zoom_auto_fit(&mut self, use_integer_scaling: bool) -> f32 {
        let _ = use_integer_scaling;
        self.get_zoom()
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
