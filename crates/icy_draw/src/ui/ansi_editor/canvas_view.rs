//! Canvas view component
//!
//! The main editing area in the center, displaying the buffer with terminal rendering.
//! This is similar to PreviewView in icy_view but for editing.

use std::sync::Arc;

use iced::{Element, Length, Task, widget::container};
use icy_engine::Screen;
use icy_engine_gui::{MonitorSettings, Terminal, TerminalView};
use parking_lot::Mutex;

/// Messages for the canvas view
#[derive(Clone, Debug)]
pub enum CanvasMessage {
    /// Viewport tick for animations
    ViewportTick,
    /// Scroll viewport by delta
    ScrollBy(f32, f32),
    /// Scroll viewport to position
    ScrollTo(f32, f32),
    /// Zoom changed
    Zoom(f32),
    /// Terminal message from the view
    TerminalMessage(icy_engine_gui::Message),
    /// Mouse pressed on canvas
    MousePress(iced::Point, iced::mouse::Button),
    /// Mouse released on canvas
    MouseRelease(iced::Point, iced::mouse::Button),
    /// Mouse moved on canvas
    MouseMove(iced::Point),
    /// Mouse scroll on canvas
    MouseScroll(f32, f32),
}

/// Canvas view state for the ANSI editor
pub struct CanvasView {
    /// Terminal widget for rendering
    pub terminal: Terminal,
    /// Monitor settings for CRT effects
    pub monitor_settings: MonitorSettings,
    /// Current zoom level (1.0 = 100%)
    pub zoom: f32,
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
            zoom: 1.0,
        }
    }

    /// Scroll viewport by delta
    pub fn scroll_by(&mut self, dx: f32, dy: f32) {
        self.terminal.scroll_x_by(dx);
        self.terminal.scroll_y_by(dy);
        self.terminal.sync_scrollbar_with_viewport();
    }

    /// Scroll viewport to absolute position
    pub fn scroll_to(&mut self, x: f32, y: f32) {
        self.terminal.scroll_x_to(x);
        self.terminal.scroll_y_to(y);
        self.terminal.sync_scrollbar_with_viewport();
    }

    /// Update animations (called from ViewportTick)
    pub fn update_animations(&mut self) {
        self.terminal.update_animations();
    }

    /// Set zoom level
    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom.clamp(0.25, 4.0);
    }

    /// Update the canvas view state
    pub fn update(&mut self, message: CanvasMessage) -> Task<CanvasMessage> {
        match message {
            CanvasMessage::ViewportTick => {
                self.update_animations();
                Task::none()
            }
            CanvasMessage::ScrollBy(dx, dy) => {
                self.scroll_by(dx, dy);
                Task::none()
            }
            CanvasMessage::ScrollTo(x, y) => {
                self.scroll_to(x, y);
                Task::none()
            }
            CanvasMessage::Zoom(zoom) => {
                self.set_zoom(zoom);
                Task::none()
            }
            CanvasMessage::TerminalMessage(_msg) => {
                // TODO: Handle terminal message
                Task::none()
            }
            CanvasMessage::MousePress(_pos, _button) => {
                // TODO: Forward to active tool
                Task::none()
            }
            CanvasMessage::MouseRelease(_pos, _button) => Task::none(),
            CanvasMessage::MouseMove(_pos) => Task::none(),
            CanvasMessage::MouseScroll(dx, dy) => {
                self.scroll_by(dx, dy);
                Task::none()
            }
        }
    }

    /// Render the canvas view
    pub fn view(&self) -> Element<'_, CanvasMessage> {
        // Use TerminalView to render with CRT shader effect
        let terminal_view = TerminalView::show_with_effects(&self.terminal, self.monitor_settings.clone()).map(CanvasMessage::TerminalMessage);

        container(terminal_view)
            .style(|theme: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(theme.extended_palette().background.weaker.color)),
                ..Default::default()
            })
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}
