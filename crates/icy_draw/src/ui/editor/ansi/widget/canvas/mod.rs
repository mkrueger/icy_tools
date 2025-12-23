//! Canvas view component
//!
//! The main editing area in the center, displaying the buffer with terminal rendering.
//! This is similar to PreviewView in icy_view but for editing.
//! Includes scrollbars, zoom support, and CRT shader effects.
//!
//! NOTE: Some scroll methods are prepared for future smooth animation support.

#![allow(dead_code)]

use std::sync::Arc;

use iced::{
    widget::{container, stack},
    Alignment, Element, Length, Task,
};
use icy_engine::Screen;

use icy_engine_gui::theme::main_area_background;
use icy_engine_gui::TerminalMessage;
use icy_engine_gui::{EditorMarkers, HorizontalScrollbarOverlay, MonitorSettings, ScalingMode, ScrollbarOverlay, Terminal, TerminalView, ZoomMessage};
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
        self.terminal.scroll_x_by(dx);
        self.terminal.scroll_y_by(dy);
        self.terminal.sync_scrollbar_with_viewport();
    }

    /// Scroll viewport by delta with smooth animation
    pub fn scroll_by_smooth(&mut self, dx: f32, dy: f32) {
        self.terminal.scroll_x_by_smooth(dx);
        self.terminal.scroll_y_by_smooth(dy);
        self.terminal.sync_scrollbar_with_viewport();
    }

    /// Scroll viewport to absolute position
    pub fn scroll_to(&mut self, x: f32, y: f32) {
        self.terminal.scroll_x_to(x);
        self.terminal.scroll_y_to(y);
        self.terminal.sync_scrollbar_with_viewport();
    }

    /// Scroll viewport to absolute position with smooth animation
    pub fn scroll_to_smooth(&mut self, x: f32, y: f32) {
        self.terminal.scroll_x_to_smooth(x);
        self.terminal.scroll_y_to_smooth(y);
        self.terminal.sync_scrollbar_with_viewport();
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
        match new_settings.scaling_mode {
            ScalingMode::Auto => {
                self.terminal.zoom_auto_fit(use_integer);
            }
            ScalingMode::Manual(z) => {
                self.terminal.set_zoom(z);
            }
        }
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
                let (dx, dy) = match delta {
                    icy_engine_gui::WheelDelta::Lines { x, y } => (x * 10.0, y * 20.0),
                    icy_engine_gui::WheelDelta::Pixels { x, y } => (x, y),
                };
                self.scroll_by(-dx, -dy);
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
        // Use TerminalView to render with CRT shader effect
        let terminal_view = TerminalView::show_with_effects(&self.terminal, Arc::new(self.monitor_settings.read().clone()), editor_markers);

        // Get scrollbar info using shared logic from icy_engine_gui
        let scrollbar_info = self.terminal.scrollbar_info();

        if scrollbar_info.needs_any_scrollbar() {
            let mut layers: Vec<Element<'_, TerminalMessage>> = vec![terminal_view];

            // Add vertical scrollbar if needed - uses viewport directly, no messages needed
            if scrollbar_info.needs_vscrollbar {
                let vscrollbar_view: Element<'_, ()> = ScrollbarOverlay::new(&self.terminal.viewport).view();
                // Map () to CanvasMessage - scrollbar mutates viewport directly via Arc<RwLock>
                let vscrollbar_mapped: Element<'_, TerminalMessage> = vscrollbar_view.map(|_| unreachable!());
                let vscrollbar_container: container::Container<'_, TerminalMessage> =
                    container(vscrollbar_mapped).width(Length::Fill).height(Length::Fill).align_x(Alignment::End);
                layers.push(vscrollbar_container.into());
            }

            // Add horizontal scrollbar if needed - uses viewport directly, no messages needed
            if scrollbar_info.needs_hscrollbar {
                let hscrollbar_view: Element<'_, ()> = HorizontalScrollbarOverlay::new(&self.terminal.viewport).view();
                let hscrollbar_mapped: Element<'_, TerminalMessage> = hscrollbar_view.map(|_| unreachable!());
                let hscrollbar_container: container::Container<'_, TerminalMessage> =
                    container(hscrollbar_mapped).width(Length::Fill).height(Length::Fill).align_y(Alignment::End);
                layers.push(hscrollbar_container.into());
            }

            container(stack(layers))
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|theme: &iced::Theme| container::Style {
                    background: Some(iced::Background::Color(main_area_background(theme))),
                    ..Default::default()
                })
                .into()
        } else {
            container(terminal_view)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|theme: &iced::Theme| container::Style {
                    background: Some(iced::Background::Color(main_area_background(theme))),
                    ..Default::default()
                })
                .into()
        }
    }
}
