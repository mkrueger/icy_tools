#![allow(static_mut_refs)]
use crate::{create_crt_shader, MonitorSettings, Terminal, ZoomMessage};
use iced::Element;
use icy_engine::{KeyModifiers, MouseButton, Position};
use std::sync::Arc;

/// Re-export iced's scroll delta for convenience
pub use iced::mouse::ScrollDelta as WheelDelta;

/// Mouse event data with both pixel and text coordinates.
/// Used by all mouse event variants in Message.
#[derive(Debug, Clone)]
pub struct TerminalMouseEvent {
    /// Mouse position in pixel coordinates (relative to terminal widget)
    pub pixel_position: (f32, f32),
    /// Mouse position in text coordinates, if over the terminal area.
    ///
    /// Note: When the terminal is in `MouseTracking::HalfBlock`, this is *not* character-cell
    /// coordinates. Instead it carries half-block coordinates (Y has 2× resolution).
    pub text_position: Option<Position>,
    /// Which mouse button was involved (for Press/Release events)
    pub button: MouseButton,
    /// Current keyboard modifier state
    pub modifiers: KeyModifiers,
}

impl TerminalMouseEvent {
    /// Create a new terminal mouse event
    pub fn new(pixel_position: (f32, f32), text_position: Option<Position>, button: MouseButton, modifiers: KeyModifiers) -> Self {
        Self {
            pixel_position,
            text_position,
            button,
            modifiers,
        }
    }

    /// Helper to check for a hyperlink at the click position.
    /// Returns the URL if a hyperlink is found at the current text_position.
    pub fn get_hyperlink(&self, screen: &dyn icy_engine::Screen) -> Option<String> {
        let cell = self.text_position?;
        for hyperlink in screen.hyperlinks() {
            if screen.is_position_in_range(cell, hyperlink.position, hyperlink.length) {
                return Some(hyperlink.url(screen));
            }
        }
        None
    }

    /// Helper to check for a RIP mouse field at the click position.
    /// Returns (clear_screen, command) if a RIP field is found.
    pub fn get_rip_field(&self, screen: &dyn icy_engine::Screen) -> Option<(bool, String)> {
        let cell = self.text_position?;
        for mouse_field in screen.mouse_fields() {
            if !mouse_field.style.is_mouse_button() {
                continue;
            }
            if mouse_field.contains(cell.x, cell.y) {
                if let Some(cmd) = &mouse_field.host_command {
                    return Some((mouse_field.style.reset_screen_after_click(), cmd.clone()));
                }
            }
        }
        None
    }
}

/// Messages emitted by the terminal view widget.
/// Applications handle these to implement their own mouse logic.
#[derive(Debug, Clone)]
pub enum TerminalMessage {
    /// Mouse button was pressed
    Press(TerminalMouseEvent),
    /// Mouse button was released
    Release(TerminalMouseEvent),
    /// Mouse moved (no button held)
    Move(TerminalMouseEvent),
    /// Mouse dragged (button held while moving)
    Drag(TerminalMouseEvent),
    /// Mouse wheel scrolled
    Scroll(WheelDelta),
    /// Zoom message (Cmd/Ctrl + scroll, or explicit zoom commands)
    Zoom(ZoomMessage),
}

pub struct TerminalView<'a> {
    _term: &'a Terminal,
}

impl<'a> TerminalView<'a> {
    /// Show terminal with CRT shader effects.
    ///
    /// # Arguments
    /// * `term` - The terminal to render
    /// * `settings` - Monitor/CRT shader settings
    /// * `editor_markers` - Optional editor markers (layer bounds, selection, etc.)
    ///   Pass `Some(markers)` for editor views, `None` for simple terminal/viewer.
    pub fn show_with_effects(term: &'a Terminal, settings: Arc<MonitorSettings>, editor_markers: Option<crate::EditorMarkers>) -> Element<'a, TerminalMessage> {
        iced::widget::container(create_crt_shader(term, settings, editor_markers))
            .id(term.id.clone())
            .into()
    }
}
