//! Canvas view component
//!
//! The main editing area in the center, displaying the buffer with terminal rendering.
//! This is similar to PreviewView in icy_view but for editing.
//! Includes scrollbars, zoom support, and CRT shader effects.

use std::sync::Arc;

use iced::{
    Alignment, Element, Length, Task,
    widget::{container, stack},
};
use icy_engine::Screen;
use icy_engine_gui::theme::main_area_background;
use icy_engine_gui::{HorizontalScrollbarOverlay, MonitorSettings, ScalingMode, ScrollbarOverlay, Terminal, TerminalView, ZoomMessage};
use parking_lot::Mutex;

/// Messages for the canvas view
#[derive(Clone, Debug)]
pub enum CanvasMessage {
    /// Viewport tick for animations
    ViewportTick,
    /// Scroll viewport by delta (direct, no animation - for mouse wheel)
    ScrollViewport(f32, f32),
    /// Scroll viewport with smooth animation (for PageUp/PageDown)
    ScrollViewportSmooth(f32, f32),
    /// Scroll viewport to absolute position (direct, no animation)
    ScrollViewportTo(f32, f32),
    /// Scroll viewport to absolute position with smooth animation (for Home/End)
    ScrollViewportToSmooth(f32, f32),
    /// Scroll vertical only to absolute Y position immediately (scrollbar drag)
    ScrollViewportYToImmediate(f32),
    /// Scroll horizontal only to absolute X position immediately (scrollbar drag)
    ScrollViewportXToImmediate(f32),
    /// Scrollbar hover state changed (vertical)
    ScrollbarHovered(bool),
    /// Horizontal scrollbar hover state changed
    HScrollbarHovered(bool),
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
}

/// Canvas view state for the ANSI editor
pub struct CanvasView {
    /// Terminal widget for rendering
    pub terminal: Terminal,
    /// Monitor settings for CRT effects
    pub monitor_settings: MonitorSettings,
}

impl CanvasView {
    /// Create a new canvas view with a screen
    /// The screen should be an EditState wrapped as Box<dyn Screen>
    pub fn new(screen: Arc<Mutex<Box<dyn Screen>>>) -> Self {
        // Create terminal widget
        let terminal = Terminal::new(screen);

        Self {
            terminal,
            monitor_settings: MonitorSettings::default(),
        }
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

    /// Update animations (called from ViewportTick)
    pub fn update_animations(&mut self) {
        self.terminal.update_animations();
    }

    /// Check if animations are needed
    pub fn needs_animation(&self) -> bool {
        self.terminal.needs_animation()
    }

    /// Get current zoom level
    pub fn get_zoom(&self) -> f32 {
        self.terminal.get_zoom()
    }

    /// Set zoom level
    pub fn set_zoom(&mut self, zoom: f32) {
        self.terminal.set_zoom(zoom);
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
        let use_integer = self.monitor_settings.use_integer_scaling;
        self.monitor_settings.scaling_mode = self.monitor_settings.scaling_mode.apply_zoom(zoom_msg, current_zoom, use_integer);
        match self.monitor_settings.scaling_mode {
            ScalingMode::Auto => {
                self.terminal.zoom_auto_fit(use_integer);
            }
            ScalingMode::Manual(z) => {
                self.terminal.set_zoom(z);
            }
        }
    }

    /// Set monitor settings for CRT effects
    pub fn set_monitor_settings(&mut self, settings: MonitorSettings) {
        self.monitor_settings = settings;
    }

    /// Update the canvas view state
    pub fn update(&mut self, message: CanvasMessage) -> Task<CanvasMessage> {
        match message {
            CanvasMessage::ViewportTick => {
                self.update_animations();
                Task::none()
            }
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
            CanvasMessage::ScrollViewportYToImmediate(y) => {
                self.terminal.scroll_y_to(y);
                self.terminal.sync_scrollbar_with_viewport();
                Task::none()
            }
            CanvasMessage::ScrollViewportXToImmediate(x) => {
                self.terminal.scroll_x_to(x);
                self.terminal.sync_scrollbar_with_viewport();
                Task::none()
            }
            CanvasMessage::ScrollbarHovered(is_hovered) => {
                self.terminal.scrollbar.set_hovered(is_hovered);
                Task::none()
            }
            CanvasMessage::HScrollbarHovered(is_hovered) => {
                self.terminal.scrollbar.set_hovered_x(is_hovered);
                Task::none()
            }
            CanvasMessage::Zoom(zoom_msg) => {
                self.apply_zoom(zoom_msg);
                Task::none()
            }
            CanvasMessage::TerminalMessage(msg) => {
                match msg {
                    icy_engine_gui::Message::ScrollViewport(dx, dy) => {
                        self.scroll_by(dx, dy);
                    }
                    icy_engine_gui::Message::Zoom(zoom_msg) => {
                        self.apply_zoom(zoom_msg);
                    }
                    icy_engine_gui::Message::StartSelection(sel) => {
                        let mut screen = self.terminal.screen.lock();
                        let _ = screen.set_selection(sel);
                    }
                    icy_engine_gui::Message::UpdateSelection(pos) => {
                        let mut screen = self.terminal.screen.lock();
                        if let Some(mut sel) = screen.selection().clone() {
                            if !sel.locked {
                                sel.lead = pos;
                                let _ = screen.set_selection(sel);
                            }
                        }
                    }
                    icy_engine_gui::Message::EndSelection => {
                        let mut screen = self.terminal.screen.lock();
                        if let Some(mut sel) = screen.selection().clone() {
                            sel.locked = true;
                            let _ = screen.set_selection(sel);
                        }
                    }
                    icy_engine_gui::Message::ClearSelection => {
                        let mut screen = self.terminal.screen.lock();
                        let _ = screen.clear_selection();
                    }
                    _ => {}
                }
                Task::none()
            }
            CanvasMessage::MousePress(_pos, _button) => {
                // TODO: Forward to active tool
                Task::none()
            }
            CanvasMessage::MouseRelease(_pos, _button) => Task::none(),
            CanvasMessage::MouseMove(_pos) => Task::none(),
        }
    }

    /// Render the canvas view with scrollbars
    pub fn view(&self) -> Element<'_, CanvasMessage> {
        // Use TerminalView to render with CRT shader effect
        let terminal_view = TerminalView::show_with_effects(&self.terminal, self.monitor_settings.clone()).map(CanvasMessage::TerminalMessage);

        // Get scrollbar info using shared logic from icy_engine_gui
        let scrollbar_info = self.terminal.scrollbar_info();

        if scrollbar_info.needs_any_scrollbar() {
            let mut layers: Vec<Element<'_, CanvasMessage>> = vec![terminal_view];

            // Add vertical scrollbar if needed
            if scrollbar_info.needs_vscrollbar {
                let vscrollbar_view = ScrollbarOverlay::new(
                    scrollbar_info.visibility_v,
                    scrollbar_info.scroll_position_v,
                    scrollbar_info.height_ratio,
                    scrollbar_info.max_scroll_y,
                    self.terminal.scrollbar_hover_state.clone(),
                    |_x, y| CanvasMessage::ScrollViewportYToImmediate(y),
                    |is_hovered| CanvasMessage::ScrollbarHovered(is_hovered),
                )
                .view();

                let vscrollbar_container: container::Container<'_, CanvasMessage> =
                    container(vscrollbar_view).width(Length::Fill).height(Length::Fill).align_x(Alignment::End);
                layers.push(vscrollbar_container.into());
            }

            // Add horizontal scrollbar if needed
            if scrollbar_info.needs_hscrollbar {
                let hscrollbar_view = HorizontalScrollbarOverlay::new(
                    scrollbar_info.visibility_h,
                    scrollbar_info.scroll_position_h,
                    scrollbar_info.width_ratio,
                    scrollbar_info.max_scroll_x,
                    self.terminal.hscrollbar_hover_state.clone(),
                    |x, _y| CanvasMessage::ScrollViewportXToImmediate(x),
                    |is_hovered| CanvasMessage::HScrollbarHovered(is_hovered),
                )
                .view();

                let hscrollbar_container: container::Container<'_, CanvasMessage> =
                    container(hscrollbar_view).width(Length::Fill).height(Length::Fill).align_y(Alignment::End);
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

    /// Render the canvas view with custom monitor settings override
    pub fn view_with_settings(&self, settings: Option<&MonitorSettings>) -> Element<'_, CanvasMessage> {
        let monitor_settings = settings.cloned().unwrap_or_else(|| self.monitor_settings.clone());

        // Use TerminalView to render with CRT shader effect
        let terminal_view = TerminalView::show_with_effects(&self.terminal, monitor_settings).map(CanvasMessage::TerminalMessage);

        // Get scrollbar info using shared logic from icy_engine_gui
        let scrollbar_info = self.terminal.scrollbar_info();

        if scrollbar_info.needs_any_scrollbar() {
            let mut layers: Vec<Element<'_, CanvasMessage>> = vec![terminal_view];

            // Add vertical scrollbar if needed
            if scrollbar_info.needs_vscrollbar {
                let vscrollbar_view = ScrollbarOverlay::new(
                    scrollbar_info.visibility_v,
                    scrollbar_info.scroll_position_v,
                    scrollbar_info.height_ratio,
                    scrollbar_info.max_scroll_y,
                    self.terminal.scrollbar_hover_state.clone(),
                    |_x, y| CanvasMessage::ScrollViewportYToImmediate(y),
                    |is_hovered| CanvasMessage::ScrollbarHovered(is_hovered),
                )
                .view();

                let vscrollbar_container: container::Container<'_, CanvasMessage> =
                    container(vscrollbar_view).width(Length::Fill).height(Length::Fill).align_x(Alignment::End);
                layers.push(vscrollbar_container.into());
            }

            // Add horizontal scrollbar if needed
            if scrollbar_info.needs_hscrollbar {
                let hscrollbar_view = HorizontalScrollbarOverlay::new(
                    scrollbar_info.visibility_h,
                    scrollbar_info.scroll_position_h,
                    scrollbar_info.width_ratio,
                    scrollbar_info.max_scroll_x,
                    self.terminal.hscrollbar_hover_state.clone(),
                    |x, _y| CanvasMessage::ScrollViewportXToImmediate(x),
                    |is_hovered| CanvasMessage::HScrollbarHovered(is_hovered),
                )
                .view();

                let hscrollbar_container: container::Container<'_, CanvasMessage> =
                    container(hscrollbar_view).width(Length::Fill).height(Length::Fill).align_y(Alignment::End);
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
