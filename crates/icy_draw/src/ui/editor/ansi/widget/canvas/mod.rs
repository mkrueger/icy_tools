//! Canvas view component
//!
//! The main editing area in the center, displaying the buffer with terminal rendering.
//! This is similar to PreviewView in icy_view but for editing.
//! Includes scrollbars, zoom support, and CRT shader effects.
//!
//! NOTE: Some scroll methods are prepared for future smooth animation support.

#![allow(dead_code)]

use std::sync::Arc;

use icy_engine::Screen;
use icy_ui::{
    widget::{container, scroll_area, scrollable},
    Element, Length, Task,
};

use icy_engine_gui::theme::main_area_background;
use icy_engine_gui::TerminalMessage;
use icy_engine_gui::{EditorMarkers, MonitorSettings, Terminal, TerminalView, ZoomMessage};
use parking_lot::{Mutex, RwLock};

/// Canvas view state for the ANSI editor
pub struct CanvasView {
    /// Terminal widget for rendering
    pub terminal: Terminal,
    /// Monitor settings for CRT effects (cached as Arc for efficient rendering)
    pub monitor_settings: Arc<RwLock<MonitorSettings>>,
}

impl CanvasView {
    /// Create a new canvas view with a screen
    /// The screen should be an EditState wrapped as Box<dyn Screen>
    pub fn new(screen: Arc<Mutex<Box<dyn Screen>>>, monitor_settings: Arc<RwLock<MonitorSettings>>) -> Self {
        // Create terminal widget
        let mut terminal = Terminal::new(screen);
        terminal.set_fit_terminal_height_to_bounds(true);

        Self { terminal, monitor_settings }
    }

    /// Set whether the terminal has focus (controls caret visibility/blinking)
    pub fn set_has_focus(&mut self, has_focus: bool) {
        self.terminal.has_focus = has_focus;
    }

    /// Scroll viewport by delta
    pub fn scroll_by(&mut self, dx: f32, dy: f32) {
        let _ = (dx, dy);
        // scroll_area owns scrolling; programmatic scrolling is task-driven
    }

    /// Scroll viewport by delta with smooth animation
    pub fn scroll_by_smooth(&mut self, dx: f32, dy: f32) {
        let _ = (dx, dy);
        // scroll_area owns scrolling; programmatic scrolling is task-driven
    }

    /// Scroll viewport to absolute position
    pub fn scroll_to(&mut self, x: f32, y: f32) {
        let _ = (x, y);
        // scroll_area owns scrolling; programmatic scrolling is task-driven
    }

    /// Scroll viewport to absolute position with smooth animation
    pub fn scroll_to_smooth(&mut self, x: f32, y: f32) {
        let _ = (x, y);
        // scroll_area owns scrolling; programmatic scrolling is task-driven
    }

    /// Set zoom level
    pub fn set_zoom(&mut self, zoom: f32) {
        self.apply_zoom(ZoomMessage::Set(zoom));
    }

    /// Zoom in by one step
    pub fn zoom_in(&mut self) {
        self.apply_zoom(ZoomMessage::In);
    }

    /// Zoom out by one step
    pub fn zoom_out(&mut self) {
        self.apply_zoom(ZoomMessage::Out);
    }

    /// Reset zoom to 100%
    pub fn zoom_reset(&mut self) {
        self.apply_zoom(ZoomMessage::Reset);
    }

    /// Apply a zoom message (unified zoom handling like icy_view)
    fn apply_zoom(&mut self, zoom_msg: ZoomMessage) {
        let current_zoom = self.terminal.get_zoom();
        let use_integer = self.monitor_settings.read().use_integer_scaling;
        // Create a new Arc with updated scaling_mode
        let mut new_settings = self.monitor_settings.read().clone();
        new_settings.scaling_mode = new_settings.scaling_mode.apply_zoom(zoom_msg, current_zoom, use_integer);
        *self.monitor_settings.write() = new_settings;
    }

    /// Handle unified terminal mouse events from icy_engine_gui.
    /// For icy_draw, we mainly need to handle selection and forwarding to tools.
    /// The actual event type (Press, Release, Move, Drag) is distinguished
    /// by the caller in the TerminalMessage match.
    fn handle_terminal_mouse_event(&mut self, _evt: icy_engine_gui::TerminalMouseEvent) {
        // For now, icy_draw handles selection at the CanvasMessage level
        // Individual tool handling will be added later
    }

    /// Set raster grid spacing (in characters/pixels)
    /// Use None to disable the raster grid
    pub fn set_raster(&mut self, spacing: Option<(f32, f32)>) {
        let mut markers = self.terminal.markers.write();
        markers.raster = spacing;
    }

    /// Set guide crosshair position (in pixels)
    /// Use None to disable the guide crosshair
    pub fn set_guide(&mut self, position: Option<(f32, f32)>) {
        let mut markers = self.terminal.markers.write();
        markers.guide = position;
    }

    /// Update viewport size after document size changes.
    /// Call this after operations that change the buffer dimensions (e.g., apply_remote_document).
    pub fn update_viewport_size(&mut self) {
        self.terminal.update_viewport_size();
    }

    /// Set the selection rectangle for shader rendering (in pixels)
    /// bounds: (x, y, width, height) in document pixels
    pub fn set_selection(&mut self, bounds: Option<(f32, f32, f32, f32)>) {
        let mut markers = self.terminal.markers.write();
        markers.selection_rect = bounds;
    }

    /// Set the brush/pencil preview rectangle for shader rendering (in pixels)
    /// bounds: (x, y, width, height) in document pixels
    pub fn set_brush_preview(&mut self, bounds: Option<(f32, f32, f32, f32)>) {
        let mut markers = self.terminal.markers.write();
        markers.brush_preview_rect = bounds;
    }

    /// Set the selection border color for marching ants display
    pub fn set_selection_color(&mut self, color: [f32; 4]) {
        let mut markers = self.terminal.markers.write();
        markers.selection_color = color;
    }

    /// Set the selection mask for complex (non-rectangular) selections
    /// mask_data: (RGBA texture data, width in cells, height in cells)
    pub fn set_selection_mask(&mut self, mask_data: Option<(Vec<u8>, u32, u32)>) {
        let mut markers = self.terminal.markers.write();
        markers.selection_mask_data = mask_data;
    }

    /// Set the tool overlay for Moebius-style translucent tool previews.
    /// mask_data: (RGBA texture data, width in pixels, height in pixels)
    /// rect_px: (x, y, width, height) in document pixels
    pub fn set_tool_overlay_mask(&mut self, mask_data: Option<(Vec<u8>, u32, u32)>, rect_px: Option<(f32, f32, f32, f32)>) {
        let mut markers = self.terminal.markers.write();
        markers.tool_overlay_mask_data = mask_data;
        markers.tool_overlay_rect = rect_px;
    }

    /// Set or update the reference image
    /// The image will be loaded and displayed as an overlay
    pub fn set_reference_image(&mut self, path: Option<std::path::PathBuf>, alpha: f32) {
        let mut markers = self.terminal.markers.write();
        if let Some(p) = path {
            markers.reference_image = Some(icy_engine_gui::ReferenceImageSettings {
                path: p,
                alpha,
                offset: (0.0, 0.0),
                scale: 1.0,
                lock_aspect_ratio: true,
                mode: icy_engine_gui::ReferenceImageMode::Stretch,
                visible: true,
                cached_data: None,
                cached_path_hash: 0,
            });
        } else {
            markers.reference_image = None;
        }
    }

    /// Toggle reference image visibility
    pub fn toggle_reference_image(&mut self) {
        let mut markers = self.terminal.markers.write();
        if let Some(ref mut settings) = markers.reference_image {
            settings.visible = !settings.visible;
        }
    }

    /// Update the canvas view state
    pub fn update(&mut self, message: TerminalMessage) -> Task<TerminalMessage> {
        match message {
            icy_engine_gui::TerminalMessage::Press(evt)
            | icy_engine_gui::TerminalMessage::Release(evt)
            | icy_engine_gui::TerminalMessage::Move(evt)
            | icy_engine_gui::TerminalMessage::Drag(evt) => {
                self.handle_terminal_mouse_event(evt);
            }
            icy_engine_gui::TerminalMessage::Scroll(delta) => {
                let _ = delta;
                // scroll_area handles wheel scrolling
            }
            icy_engine_gui::TerminalMessage::Zoom(zoom_msg) => {
                self.apply_zoom(zoom_msg);
            }
        }
        Task::none()
    }

    /// Scroll viewport by delta
    pub fn scroll_viewport(&mut self, dx: f32, dy: f32) {
        self.scroll_by(dx, dy);
    }

    /// Scroll viewport with smooth animation
    pub fn scroll_viewport_smooth(&mut self, dx: f32, dy: f32) {
        self.scroll_by_smooth(dx, dy);
    }

    /// Scroll viewport to absolute position
    pub fn scroll_viewport_to(&mut self, x: f32, y: f32) {
        self.scroll_to(x, y);
    }

    /// Scroll viewport to absolute position with smooth animation
    pub fn scroll_viewport_to_smooth(&mut self, x: f32, y: f32) {
        self.scroll_to_smooth(x, y);
    }

    /// Render the canvas view with scrollbars
    ///
    /// # Arguments
    /// * `editor_markers` - Optional editor markers (layer bounds, selection, etc.)
    ///   The caller should set layer_bounds, selection_rect, etc. before calling view().
    pub fn view(&self, editor_markers: Option<EditorMarkers>) -> Element<'_, TerminalMessage> {
        // Get scrollable content size from screen (virtual_size includes full document)
        let screen = self.terminal.screen.lock();
        let virtual_size = screen.virtual_size();
        drop(screen);

        // Get zoom from viewport
        let zoom = self.terminal.get_zoom();

        let monitor_settings = Arc::new(self.monitor_settings.read().clone());

        // Scrollable size in zoomed pixels (for scroll_area)
        // In FitWidth mode, zoom depends on the available viewport width. Ensure the content
        // width is always >= the widget width so `show_viewport` reports the real viewport width.
        // Horizontal scrolling is disabled in FitWidth.
        let is_fit_width = monitor_settings.scaling_mode.is_fit_width();
        let scrollable_width = if is_fit_width {
            (virtual_size.width as f32 * zoom).max(100_000.0)
        } else {
            virtual_size.width as f32 * zoom
        };
        let scrollable_height = virtual_size.height as f32 * zoom;

        let scrollable_size = icy_ui::Size::new(scrollable_width, scrollable_height);

        let content = scroll_area()
            .id(self.terminal.scroll_area_id())
            .width(Length::Fill)
            .height(Length::Fill)
            .direction(if is_fit_width {
                scrollable::Direction::Vertical(scrollable::Scrollbar::new().width(8).scroller_width(6))
            } else {
                scrollable::Direction::Both {
                    vertical: scrollable::Scrollbar::new().width(8).scroller_width(6),
                    horizontal: scrollable::Scrollbar::new().width(8).scroller_width(6),
                }
            })
            .show_viewport(scrollable_size, move |scroll_viewport| {
                self.terminal.update_scroll_from_viewport(scroll_viewport, zoom);

                TerminalView::show_with_effects(&self.terminal, monitor_settings.clone(), editor_markers.clone()).map(|msg| msg)
            });

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|theme: &icy_ui::Theme| container::Style {
                background: Some(icy_ui::Background::Color(main_area_background(theme))),
                ..Default::default()
            })
            .into()
    }
}
