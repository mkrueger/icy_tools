//! Canvas view component
//!
//! The main editing area in the center, displaying the buffer with terminal rendering.
//! This is similar to PreviewView in icy_view but for editing.
//! Includes scrollbars, zoom support, and CRT shader effects.

use std::sync::Arc;

use iced::{
    Alignment, Element, Length, Task, Theme,
    widget::{button, column, container, stack, text},
};
use iced_aw::ContextMenu;
use icy_engine::Screen;

use crate::fl;
use icy_engine_gui::theme::main_area_background;
use icy_engine_gui::{HorizontalScrollbarOverlay, MonitorSettings, ScalingMode, ScrollbarOverlay, Terminal, TerminalView, ZoomMessage};
use parking_lot::{Mutex, RwLock};

/// Messages for the canvas view
#[derive(Clone, Debug)]
pub enum CanvasMessage {
    /// Scroll viewport by delta (direct, no animation - for mouse wheel)
    ScrollViewport(f32, f32),
    /// Scroll viewport with smooth animation (for PageUp/PageDown)
    ScrollViewportSmooth(f32, f32),
    /// Scroll viewport to absolute position (direct, no animation)
    ScrollViewportTo(f32, f32),
    /// Scroll viewport to absolute position with smooth animation (for Home/End)
    ScrollViewportToSmooth(f32, f32),
    /// Unified zoom message
    Zoom(ZoomMessage),
    /// Terminal message from the view
    TerminalMessage(icy_engine_gui::Message),
    /// Mouse pressed on canvas
    MousePress(iced::Point, iced::mouse::Button),
    /// Mouse released on canvas
    MouseRelease(iced::Point, iced::mouse::Button),
    /// Mouse moved on canvas
    MouseMove(iced::Point),
    /// Cut selection (context menu)
    Cut,
    /// Copy selection (context menu)
    Copy,
    /// Paste clipboard (context menu)
    Paste,
}

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

    /// Get current zoom level
    pub fn get_zoom(&self) -> f32 {
        self.terminal.get_zoom()
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

    /// Auto-fit zoom to viewport
    pub fn zoom_auto_fit(&mut self) {
        self.apply_zoom(ZoomMessage::AutoFit);
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

    /// Overwrite monitor settings (used when switching the backing shared settings)
    pub fn set_monitor_settings_source(&mut self, settings: Arc<RwLock<MonitorSettings>>) {
        self.monitor_settings = settings;
    }

    /// Set marker settings (colors, alphas) from app settings
    pub fn set_marker_settings(&mut self, settings: icy_engine_gui::MarkerSettings) {
        let mut markers = self.terminal.markers.write();
        markers.marker_settings = Some(settings);
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

    /// Get current raster spacing
    pub fn get_raster(&self) -> Option<(f32, f32)> {
        self.terminal.markers.read().raster
    }

    /// Get current guide position
    pub fn get_guide(&self) -> Option<(f32, f32)> {
        self.terminal.markers.read().guide
    }

    /// Set layer bounds for the current layer (in pixels)
    /// bounds: (x, y, width, height) in document pixels
    /// show: whether to show the layer bounds
    pub fn set_layer_bounds(&mut self, bounds: Option<(f32, f32, f32, f32)>, show: bool) {
        let mut markers = self.terminal.markers.write();
        markers.layer_bounds = bounds;
    }

    pub fn set_show_layer_borders(&mut self, show: bool) {
        let mut markers: parking_lot::lock_api::RwLockWriteGuard<'_, parking_lot::RawRwLock, icy_engine_gui::EditorMarkers> = self.terminal.markers.write();
        markers.show_layer_bounds = show;
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
    /// font_dimensions: (font_width, font_height) in pixels
    pub fn set_selection_mask(&mut self, mask_data: Option<(Vec<u8>, u32, u32)>, font_dimensions: Option<(f32, f32)>) {
        let mut markers = self.terminal.markers.write();
        markers.selection_mask_data = mask_data;
        markers.font_dimensions = font_dimensions;
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
    pub fn update(&mut self, message: CanvasMessage) -> Task<CanvasMessage> {
        match message {
            CanvasMessage::ScrollViewport(dx, dy) => {
                self.scroll_by(dx, dy);
                Task::none()
            }
            CanvasMessage::ScrollViewportSmooth(dx, dy) => {
                self.scroll_by_smooth(dx, dy);
                Task::none()
            }
            CanvasMessage::ScrollViewportTo(x, y) => {
                self.scroll_to(x, y);
                Task::none()
            }
            CanvasMessage::ScrollViewportToSmooth(x, y) => {
                self.scroll_to_smooth(x, y);
                Task::none()
            }
            CanvasMessage::Zoom(zoom_msg) => {
                self.apply_zoom(zoom_msg);
                Task::none()
            }
            CanvasMessage::TerminalMessage(msg) => {
                match msg {
                    icy_engine_gui::Message::Press(evt)
                    | icy_engine_gui::Message::Release(evt)
                    | icy_engine_gui::Message::Move(evt)
                    | icy_engine_gui::Message::Drag(evt) => {
                        self.handle_terminal_mouse_event(evt);
                    }
                    icy_engine_gui::Message::Scroll(delta) => {
                        let (dx, dy) = match delta {
                            icy_engine_gui::WheelDelta::Lines { x, y } => (x * 10.0, y * 20.0),
                            icy_engine_gui::WheelDelta::Pixels { x, y } => (x, y),
                        };
                        self.scroll_by(-dx, -dy);
                    }
                    icy_engine_gui::Message::Zoom(zoom_msg) => {
                        self.apply_zoom(zoom_msg);
                    }
                }
                Task::none()
            }
            CanvasMessage::MousePress(_pos, _button) => {
                // TODO: Forward to active tool
                Task::none()
            }
            CanvasMessage::MouseRelease(_pos, _button) => Task::none(),
            CanvasMessage::MouseMove(_pos) => Task::none(),
            CanvasMessage::Cut | CanvasMessage::Copy | CanvasMessage::Paste => {
                // These are handled by the parent AnsiEditor
                Task::none()
            }
        }
    }

    /// Create a menu item button for the context menu
    fn menu_item(label: String, message: Option<CanvasMessage>) -> Element<'static, CanvasMessage> {
        let is_enabled = message.is_some();

        button(text(label).size(13))
            .on_press_maybe(message)
            .width(Length::Fill)
            .padding([6, 10])
            .style(move |theme: &Theme, status: button::Status| {
                let palette = theme.extended_palette();

                match status {
                    button::Status::Hovered if is_enabled => button::Style {
                        background: Some(iced::Background::Color(palette.primary.base.color)),
                        text_color: palette.primary.base.text,
                        border: iced::Border {
                            radius: 3.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    button::Status::Pressed if is_enabled => button::Style {
                        background: Some(iced::Background::Color(palette.primary.strong.color)),
                        text_color: palette.primary.strong.text,
                        border: iced::Border {
                            radius: 3.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    _ if !is_enabled => button::Style {
                        background: None,
                        text_color: palette.background.weak.text.scale_alpha(0.4),
                        ..Default::default()
                    },
                    _ => button::Style {
                        background: None,
                        text_color: palette.background.weak.text,
                        ..Default::default()
                    },
                }
            })
            .into()
    }

    /// Build the context menu for the canvas
    fn build_context_menu() -> Element<'static, CanvasMessage> {
        let cut_btn = Self::menu_item(fl!("menu-cut"), Some(CanvasMessage::Cut));
        let copy_btn = Self::menu_item(fl!("menu-copy"), Some(CanvasMessage::Copy));
        let paste_btn = Self::menu_item(fl!("menu-paste"), Some(CanvasMessage::Paste));

        container(column![cut_btn, copy_btn, paste_btn].spacing(2).width(Length::Fixed(150.0)))
            .style(|theme: &Theme| {
                let palette = theme.extended_palette();
                container::Style {
                    background: Some(iced::Background::Color(palette.background.weak.color)),
                    border: iced::Border {
                        color: palette.background.strong.color,
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..Default::default()
                }
            })
            .padding(4)
            .into()
    }

    /// Render the canvas view with scrollbars
    pub fn view(&self) -> Element<'_, CanvasMessage> {
        self.view_with_context_menu(true)
    }

    /// Render the canvas view with scrollbars, optionally wrapped with the Cut/Copy/Paste context menu.
    pub fn view_with_context_menu(&self, show_context_menu: bool) -> Element<'_, CanvasMessage> {
        // Use TerminalView to render with CRT shader effect
        let terminal_view = TerminalView::show_with_effects(&self.terminal, Arc::new(self.monitor_settings.read().clone())).map(CanvasMessage::TerminalMessage);

        // Get scrollbar info using shared logic from icy_engine_gui
        let scrollbar_info = self.terminal.scrollbar_info();

        if scrollbar_info.needs_any_scrollbar() {
            let mut layers: Vec<Element<'_, CanvasMessage>> = vec![terminal_view];

            // Add vertical scrollbar if needed - uses viewport directly, no messages needed
            if scrollbar_info.needs_vscrollbar {
                let vscrollbar_view: Element<'_, ()> = ScrollbarOverlay::new(&self.terminal.viewport).view();
                // Map () to CanvasMessage - scrollbar mutates viewport directly via Arc<RwLock>
                let vscrollbar_mapped: Element<'_, CanvasMessage> = vscrollbar_view.map(|_| unreachable!());
                let vscrollbar_container: container::Container<'_, CanvasMessage> =
                    container(vscrollbar_mapped).width(Length::Fill).height(Length::Fill).align_x(Alignment::End);
                layers.push(vscrollbar_container.into());
            }

            // Add horizontal scrollbar if needed - uses viewport directly, no messages needed
            if scrollbar_info.needs_hscrollbar {
                let hscrollbar_view: Element<'_, ()> = HorizontalScrollbarOverlay::new(&self.terminal.viewport).view();
                let hscrollbar_mapped: Element<'_, CanvasMessage> = hscrollbar_view.map(|_| unreachable!());
                let hscrollbar_container: container::Container<'_, CanvasMessage> =
                    container(hscrollbar_mapped).width(Length::Fill).height(Length::Fill).align_y(Alignment::End);
                layers.push(hscrollbar_container.into());
            }

            let canvas_view = container(stack(layers))
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|theme: &iced::Theme| container::Style {
                    background: Some(iced::Background::Color(main_area_background(theme))),
                    ..Default::default()
                });

            if show_context_menu {
                ContextMenu::new(canvas_view, || Self::build_context_menu()).into()
            } else {
                canvas_view.into()
            }
        } else {
            let canvas_view = container(terminal_view)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|theme: &iced::Theme| container::Style {
                    background: Some(iced::Background::Color(main_area_background(theme))),
                    ..Default::default()
                });

            if show_context_menu {
                ContextMenu::new(canvas_view, || Self::build_context_menu()).into()
            } else {
                canvas_view.into()
            }
        }
    }

    /// Render the canvas view with custom monitor settings override
    pub fn view_with_settings(&self, settings: Option<Arc<MonitorSettings>>) -> Element<'_, CanvasMessage> {
        let monitor_settings = settings.unwrap_or_else(|| Arc::new(self.monitor_settings.read().clone()));

        // Use TerminalView to render with CRT shader effect
        let terminal_view = TerminalView::show_with_effects(&self.terminal, monitor_settings).map(CanvasMessage::TerminalMessage);

        // Get scrollbar info using shared logic from icy_engine_gui
        let scrollbar_info = self.terminal.scrollbar_info();

        if scrollbar_info.needs_any_scrollbar() {
            let mut layers: Vec<Element<'_, CanvasMessage>> = vec![terminal_view];

            // Add vertical scrollbar if needed - uses viewport directly, no messages needed
            if scrollbar_info.needs_vscrollbar {
                let vscrollbar_view: Element<'_, ()> = ScrollbarOverlay::new(&self.terminal.viewport).view();
                let vscrollbar_mapped: Element<'_, CanvasMessage> = vscrollbar_view.map(|_| unreachable!());
                let vscrollbar_container: container::Container<'_, CanvasMessage> =
                    container(vscrollbar_mapped).width(Length::Fill).height(Length::Fill).align_x(Alignment::End);
                layers.push(vscrollbar_container.into());
            }

            // Add horizontal scrollbar if needed - uses viewport directly, no messages needed
            if scrollbar_info.needs_hscrollbar {
                let hscrollbar_view: Element<'_, ()> = HorizontalScrollbarOverlay::new(&self.terminal.viewport).view();
                let hscrollbar_mapped: Element<'_, CanvasMessage> = hscrollbar_view.map(|_| unreachable!());
                let hscrollbar_container: container::Container<'_, CanvasMessage> =
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
