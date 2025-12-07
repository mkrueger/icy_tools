//! Canvas view component
//!
//! The main editing area in the center, displaying the buffer with terminal rendering.
//! This is similar to PreviewView in icy_view but for editing.

use std::sync::Arc;

use iced::{Element, Length, Task, widget::container};
use icy_engine::{Screen, Size, TextBuffer, TextPane, TextScreen};
use icy_engine_edit::EditState;
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
    /// Screen for the terminal (wraps the buffer)
    screen: Arc<Mutex<Box<dyn Screen>>>,
    /// Monitor settings for CRT effects
    pub monitor_settings: MonitorSettings,
    /// Current zoom level (1.0 = 100%)
    pub zoom: f32,
    /// Whether to show grid
    pub show_grid: bool,
    /// Whether to show guides
    pub show_guides: bool,
    /// Reference image (optional)
    pub reference_image: Option<Vec<u8>>,
}

impl Default for CanvasView {
    fn default() -> Self {
        Self::new()
    }
}

impl CanvasView {
    pub fn new() -> Self {
        // Create a default screen (80x25 terminal)
        let screen: Box<dyn Screen> = Box::new(TextScreen::new(Size::new(80, 25)));
        let screen = Arc::new(Mutex::new(screen));
        
        // Create terminal widget
        let terminal = Terminal::new(screen.clone());
        
        Self {
            terminal,
            screen,
            monitor_settings: MonitorSettings::default(),
            zoom: 1.0,
            show_grid: false,
            show_guides: false,
            reference_image: None,
        }
    }

    /// Initialize or re-initialize the terminal with a buffer's dimensions
    pub fn init_from_buffer(&mut self, buffer: &TextBuffer) {
        // Create a new TextScreen from the buffer dimensions
        let screen: Box<dyn Screen> = Box::new(TextScreen::new(Size::new(
            buffer.get_width(),
            buffer.get_height(),
        )));
        let screen = Arc::new(Mutex::new(screen));
        
        // Create terminal widget
        let terminal = Terminal::new(screen.clone());
        
        self.screen = screen;
        self.terminal = terminal;
    }

    /// Sync the screen with the buffer contents
    /// This should be called when the buffer changes
    pub fn sync_with_buffer(&mut self, buffer: &TextBuffer) {
        // Update the screen with buffer data
        let mut screen = self.screen.lock();
        
        // Downcast to TextScreen to access buffer
        if let Some(text_screen) = screen.as_any_mut().downcast_mut::<TextScreen>() {
            // Copy buffer contents
            // For now just sync the dimensions
            if text_screen.buffer.get_width() != buffer.get_width() 
                || text_screen.buffer.get_height() != buffer.get_height() 
            {
                drop(screen);
                self.init_from_buffer(buffer);
            }
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

    /// Update animations (called rrom ViewportTick)
    pub fn update_animations(&mut self) {
        self.terminal.update_animations();
    }

    /// Chlck if animations are active
    pub fn needs_animation(&self) -> bool {
        self.terminal.needs_animation()
    }

    /// Set zoom level
    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom.clamp(0.25, 4.0);
    }

    /// Update the canvas view state
    pub fn update(&mut self, message: CanvasMessage, _edit_state: &Arc<Mutex<EditState>>) -> Task<CanvasMessage> {
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
            CanvasMessage::MouseRelease(_pos, _button) => {
                Task::none()
            }
            CanvasMessage::MouseMove(_pos) => {
                Task::none()
            }
            CanvasMessage::MouseScroll(dx, dy) => {
                self.scroll_by(dx, dy);
                Task::none()
            }
        }
    }

    /// Render the canvas view
    pub fn view<'a>(&'a self, _edit_state: &'a Arc<Mutex<EditState>>) -> Element<'a, CanvasMessage> {
        // Use TerminalView to render with CRT shader effect
        let terminal_view = TerminalView::show_with_effects(&self.terminal, self.monitor_settings.clone())
            .map(CanvasMessage::TerminalMessage);

        container(terminal_view)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}
