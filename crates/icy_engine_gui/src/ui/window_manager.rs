//! Common window manager utilities and traits for icy_tools applications.
//!
//! This module provides shared infrastructure for multi-window applications:
//! - `Window` trait defining the interface for application windows
//! - `WindowManagerMessage<M>` generic enum for window manager messages
//! - Helper functions for common window manager operations
//! - Subscription builders for event handling

use iced::{Element, Task, Theme, window};
use std::collections::{BTreeMap, HashSet};

// ============================================================================
// Generic WindowManagerMessage enum
// ============================================================================

/// Common window manager message enum.
///
/// This enum is generic over the application's window message type `M`.
/// Use this in your WindowManager to reduce boilerplate.
#[derive(Clone, Debug)]
pub enum WindowManagerMessage<M> {
    /// Open a new window
    OpenWindow,
    /// Close a specific window
    CloseWindow(window::Id),
    /// A window was opened (from iced)
    WindowOpened(window::Id),
    /// Focus window by user-facing ID (1-10, where 0 = 10)
    FocusWindow(usize),
    /// Focus next widget
    FocusNext,
    /// Focus previous widget
    FocusPrevious,
    /// A window was closed (from iced)
    WindowClosed(window::Id),
    /// Message for a specific window
    WindowMessage(window::Id, M),
    /// Window title changed
    TitleChanged(window::Id, String),
    /// Event for a specific window
    Event(window::Id, iced::Event),
    /// Animation tick (for blink, scroll animations, etc.)
    AnimationTick,
}

// ============================================================================
// Common message handling
// ============================================================================

/// Result of trying to handle a common window manager message.
pub enum HandleResult<M> {
    /// Message was handled, return this task
    Handled(Task<WindowManagerMessage<M>>),
    /// Message was not handled, caller should handle it
    NotHandled(WindowManagerMessage<M>),
}

/// Try to handle common window manager messages.
///
/// Returns `HandleResult::Handled` with a task if the message was handled,
/// or `HandleResult::NotHandled` with the original message if not.
///
/// Handles: `CloseWindow`, `FocusNext`, `FocusPrevious`, `FocusWindow`, `WindowClosed`
pub fn try_handle_message<W, M>(message: WindowManagerMessage<M>, windows: &mut BTreeMap<window::Id, W>) -> HandleResult<M>
where
    W: Window<Message = M>,
    M: Clone + 'static,
{
    match message {
        WindowManagerMessage::CloseWindow(id) => HandleResult::Handled(window::close(id)),

        WindowManagerMessage::FocusNext => HandleResult::Handled(iced::widget::operation::focus_next()),

        WindowManagerMessage::FocusPrevious => HandleResult::Handled(iced::widget::operation::focus_previous()),

        WindowManagerMessage::FocusWindow(target_id) => HandleResult::Handled(focus_window_by_id(windows, target_id)),

        WindowManagerMessage::WindowClosed(id) => HandleResult::Handled(handle_window_closed(windows, id)),

        WindowManagerMessage::TitleChanged(id, title) => {
            if let Some(window) = windows.get_mut(&id) {
                // Note: Window trait doesn't have set_title, so this is a no-op by default
                // Apps that need title updates should handle it in their Window impl
                let _ = (window, title);
            }
            HandleResult::Handled(Task::none())
        }

        // These messages need app-specific handling
        other => HandleResult::NotHandled(other),
    }
}

/// Handle a WindowMessage by delegating to the window's update method.
///
/// This is a convenience function for the common pattern of forwarding
/// messages to individual windows.
pub fn handle_window_message<W, M>(windows: &mut BTreeMap<window::Id, W>, id: window::Id, msg: M) -> Task<WindowManagerMessage<M>>
where
    W: Window<Message = M>,
    M: Clone + Send + 'static,
{
    if let Some(window) = windows.get_mut(&id) {
        window.update(msg).map(move |m| WindowManagerMessage::WindowMessage(id, m))
    } else {
        Task::none()
    }
}

/// Handle an Event by delegating to the window's handle_event method.
///
/// Returns a batch of tasks: one for any immediate message and one for the task.
pub fn handle_event<W, M>(windows: &mut BTreeMap<window::Id, W>, window_id: window::Id, event: &iced::Event) -> Task<WindowManagerMessage<M>>
where
    W: Window<Message = M>,
    M: Clone + Send + 'static,
{
    if let Some(window) = windows.get_mut(&window_id) {
        let (msg_opt, task) = window.handle_event(event);
        let mut tasks = vec![task.map(move |m| WindowManagerMessage::WindowMessage(window_id, m))];
        if let Some(msg) = msg_opt {
            tasks.push(Task::done(WindowManagerMessage::WindowMessage(window_id, msg)));
        }
        Task::batch(tasks)
    } else {
        Task::none()
    }
}

/// Handle AnimationTick by forwarding to all windows that need animation.
///
/// The `make_tick_msg` parameter is a function that creates the tick message
/// for a window (e.g., `|| Message::AnimationTick`).
pub fn handle_animation_tick<W, M, F>(windows: &BTreeMap<window::Id, W>, make_tick_msg: F) -> Task<WindowManagerMessage<M>>
where
    W: Window<Message = M>,
    M: Clone + Send + 'static,
    F: Fn() -> M,
{
    let mut tasks = Vec::new();
    for (window_id, window) in windows.iter() {
        if window.needs_animation() {
            let id = *window_id;
            tasks.push(Task::done(WindowManagerMessage::WindowMessage(id, make_tick_msg())));
        }
    }
    Task::batch(tasks)
}

// ============================================================================
// Window trait
// ============================================================================

/// Trait for application windows managed by a WindowManager.
///
/// Implement this trait for your MainWindow type to use the
/// shared window manager helper functions.
pub trait Window {
    /// The message type for this window.
    type Message;

    /// Get the user-facing window ID (1-10, displayed in title as ⌘1-⌘9, ⌘0).
    fn id(&self) -> usize;

    /// Get the window title.
    fn title(&self) -> &str;

    /// Get zoom info string for display in title (e.g., "[AUTO]" or "[150%]").
    fn get_zoom_info_string(&self) -> String {
        String::new()
    }

    /// Update the window with a message.
    fn update(&mut self, msg: Self::Message) -> Task<Self::Message>;

    /// Render the window view.
    fn view(&self) -> Element<'_, Self::Message>;

    /// Get the window theme.
    fn theme(&self) -> Theme;

    /// Handle an event, returning an optional message and a task.
    fn handle_event(&mut self, event: &iced::Event) -> (Option<Self::Message>, Task<Self::Message>);

    /// Whether this window needs animation ticks.
    fn needs_animation(&self) -> bool {
        false
    }
}

// ============================================================================
// Helper functions
// ============================================================================

/// Find the next available window ID (1-based, for user display).
///
/// This function finds the lowest available ID starting from 1.
/// Used to assign IDs like ⌘1, ⌘2, etc. to windows.
pub fn find_next_window_id<W: Window>(windows: &BTreeMap<window::Id, W>) -> usize {
    let used_ids: HashSet<usize> = windows.values().map(|w| w.id()).collect();
    for id in 1.. {
        if !used_ids.contains(&id) {
            return id;
        }
    }
    1
}

/// Format a window title with optional zoom info and window number.
///
/// - For single-window apps: "Title [Zoom]"
/// - For multi-window apps: "Title [Zoom] - ⌘N" (where N is the window ID, up to 10)
pub fn format_window_title<W: Window>(window: &W, window_count: usize) -> String {
    let zoom_info = window.get_zoom_info_string();

    if window_count == 1 {
        if zoom_info.is_empty() {
            window.title().to_string()
        } else {
            format!("{} {}", window.title(), zoom_info)
        }
    } else {
        let id = window.id();
        if id <= 10 {
            // Show ⌘0 for window 10, ⌘1-⌘9 for windows 1-9
            let display_key = if id == 10 { 0 } else { id };
            if zoom_info.is_empty() {
                format!("{} - ⌘{}", window.title(), display_key)
            } else {
                format!("{} {} - ⌘{}", window.title(), zoom_info, display_key)
            }
        } else {
            // Windows beyond 10 don't get a keyboard shortcut
            if zoom_info.is_empty() {
                window.title().to_string()
            } else {
                format!("{} {}", window.title(), zoom_info)
            }
        }
    }
}

/// Create a task to focus a window, ensuring it comes to front.
pub fn focus_window_task<T: 'static>(window_id: window::Id) -> Task<T> {
    window::gain_focus(window_id)
}

/// Find and focus a window by its user-facing ID (1-10).
///
/// Returns a task that focuses the window, or `Task::none()` if not found.
pub fn focus_window_by_id<W: Window, T: 'static>(windows: &BTreeMap<window::Id, W>, target_id: usize) -> Task<T> {
    for (window_id, window) in windows.iter() {
        if window.id() == target_id {
            return focus_window_task(*window_id);
        }
    }
    Task::none()
}

/// Handle the common WindowClosed logic.
///
/// Removes the window and returns `iced::exit()` if no windows remain.
pub fn handle_window_closed<W, T: 'static>(windows: &mut BTreeMap<window::Id, W>, id: window::Id) -> Task<T> {
    windows.remove(&id);
    if windows.is_empty() { iced::exit() } else { Task::none() }
}

/// Check if any window needs animation ticks.
pub fn any_window_needs_animation<W: Window>(windows: &BTreeMap<window::Id, W>) -> bool {
    windows.values().any(|w| w.needs_animation())
}

// Re-export ANIMATION_TICK_MS from viewport module for convenience
pub use crate::viewport::ANIMATION_TICK_MS;

/// Check if a keyboard event is Alt/Cmd+Number for focusing windows.
/// Returns Some(target_window_id) if the key combination matches (Alt/Cmd + 0-9),
/// or None if not a window focus key.
///
/// Window IDs: 1-9 map to windows 1-9, 0 maps to window 10.
///
/// Note: Uses Alt on all platforms. On macOS, Cmd is also accepted (without Ctrl).
pub fn check_window_focus_key(key: &iced::keyboard::Key, modifiers: &iced::keyboard::Modifiers) -> Option<usize> {
    // Alt+Number works on all platforms
    // Cmd+Number works on macOS (command() returns true for Cmd on macOS, Ctrl on Linux/Windows)
    // We accept Alt always, or Cmd without Ctrl (to avoid Ctrl+Number conflicts)
    let is_alt_or_cmd = modifiers.alt() || (modifiers.command() && !modifiers.control());

    if is_alt_or_cmd && !modifiers.shift() {
        if let iced::keyboard::Key::Character(s) = key {
            if let Some(digit) = s.chars().next() {
                if digit.is_ascii_digit() {
                    let target_id = digit.to_digit(10).unwrap() as usize;
                    // Special case: Alt+0 focuses window 10
                    let target_id = if target_id == 0 { 10 } else { target_id };
                    return Some(target_id);
                }
            }
        }
    }
    None
}
