//! Modal Dialog System
//!
//! Provides a unified trait-based system for modal dialogs.
//! Dialogs are stacked and handle events (including Escape/Enter) consistently.
//!
//! # Features
//! - Stack-based dialog management (multiple dialogs can be open)
//! - Automatic Escape (cancel) and Enter (confirm) handling with dialog control
//! - Dialogs can validate and prevent closing
//! - Dialogs can push nested dialogs (e.g., confirmation dialogs)
//!
//! # Usage
//!
//! ```rust,ignore
//! // Define your dialog
//! struct MyDialog { ... }
//!
//! impl Dialog<AppMessage> for MyDialog {
//!     fn view(&self) -> Element<'_, AppMessage> {
//!         // Return just the dialog content - DialogStack wraps with modal overlay
//!         container(column![...]).into()
//!     }
//!     
//!     fn request_cancel(&mut self) -> DialogAction<AppMessage> {
//!         DialogAction::Close  // or validate first
//!     }
//!     
//!     fn request_confirm(&mut self) -> DialogAction<AppMessage> {
//!         if self.validate() {
//!             DialogAction::CloseWith(AppMessage::MyDialogConfirmed(self.result()))
//!         } else {
//!             DialogAction::None  // Stay open, show error
//!         }
//!     }
//! }
//!
//! // In your app
//! struct App {
//!     dialogs: DialogStack<AppMessage>,
//! }
//!
//! // Push a dialog
//! app.dialogs.push(MyDialog::new());
//!
//! // In view
//! fn view(&self) -> Element<AppMessage> {
//!     let content = self.main_view();
//!     self.dialogs.view(content)
//! }
//!
//! // In event handling
//! fn handle_event(&mut self, event: Event) -> Task<AppMessage> {
//!     self.dialogs.handle_event(&event)
//! }
//! ```

/// Helper macro to generate both wrapper and extractor functions for dialog messages.
///
/// This reduces boilerplate when creating dialogs that need message routing.
/// Returns a tuple `(wrapper_fn, extractor_fn)` - use with `*_from_msg` dialog functions.
///
/// # Example
/// ```ignore
/// // Instead of writing:
/// settings_dialog(
///     options, temp_options,
///     Message::SettingsDialog,
///     |msg| match msg {
///         Message::SettingsDialog(m) => Some(m),
///         _ => None,
///     },
/// )
///
/// // You can write:
/// settings_dialog_from_msg(options, temp_options, dialog_msg!(Message::SettingsDialog))
/// ```
#[macro_export]
macro_rules! dialog_msg {
    ($variant:path) => {
        ($variant, |msg: &_| match msg {
            $variant(inner) => Some(inner),
            _ => None,
        })
    };
}

use iced::{Event, Task, Theme, keyboard};

/// Style for how a dialog is displayed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DialogStyle {
    /// Modal dialog with semi-transparent overlay behind it (default)
    /// The dialog content is centered and the background is dimmed.
    #[default]
    Modal,
    /// Fullscreen dialog that takes the entire window
    /// No overlay, the dialog view fills the entire screen.
    /// Useful for about dialogs, splash screens, etc.
    Fullscreen,
}

/// Action to take after a dialog request (cancel, confirm, or custom event)
pub enum DialogAction<M> {
    /// Do nothing, dialog stays open
    None,
    /// Close this dialog (pop from stack)
    Close,
    /// Close this dialog and send a message to the app
    CloseWith(M),
    /// Push another dialog on the stack (e.g., confirmation dialog)
    Push(Box<dyn Dialog<M>>),
    /// Send a message but keep dialog open (e.g., show validation error)
    SendMessage(M),
    /// Run an async task (dialog stays open until task completes)
    RunTask(Task<M>),
}

impl<M: Send + 'static> DialogAction<M> {
    /// Create a CloseWith action
    pub fn close_with(message: M) -> Self {
        DialogAction::CloseWith(message)
    }

    /// Create a Push action
    pub fn push(dialog: impl Dialog<M> + 'static) -> Self {
        DialogAction::Push(Box::new(dialog))
    }

    /// Create a SendMessage action
    pub fn send(message: M) -> Self {
        DialogAction::SendMessage(message)
    }
}

/// Result returned by a dialog state's `handle_message` method.
///
/// This is used by dialog state structs to communicate their result
/// back to the wrapper, which then converts it to a `DialogAction`.
///
/// Generic parameter `T` is the success value type (e.g., `PathBuf` for export dialogs).
pub enum StateResult<T> {
    /// Keep dialog open (no action needed)
    None,
    /// Close dialog (cancelled or dismissed)
    Close,
    /// Close dialog with success result
    Success(T),
}

/// A modal dialog that renders over background content.
///
/// Generic parameter `M` is the application's Message type.
/// This allows dialogs to be stored as `Vec<Box<dyn Dialog<M>>>`.
///
/// Note: Dialogs should return just their content from `view()`. The `DialogStack`
/// handles wrapping dialogs with modal overlays automatically.
pub trait Dialog<M> {
    /// Render the dialog content.
    ///
    /// Return only the dialog box itself - the DialogStack will handle
    /// wrapping it with a modal overlay.
    fn view(&self) -> iced::Element<'_, M>;

    /// Handle a message sent to this dialog.
    ///
    /// Return `Some(action)` if this dialog handled the message,
    /// `None` if the message should be passed to the application.
    ///
    /// This allows dialogs to process their own button clicks, input changes, etc.
    /// without the application needing to know about the dialog's internal state.
    ///
    /// Default: Return None (message not handled by dialog)
    fn update(&mut self, _message: &M) -> Option<DialogAction<M>> {
        None
    }

    /// Called when Escape is pressed or user clicks outside (if `close_on_blur` is true).
    /// Dialog can validate, show confirmation, or just close.
    ///
    /// Default: Close the dialog
    fn request_cancel(&mut self) -> DialogAction<M> {
        DialogAction::Close
    }

    /// Called when Enter is pressed.
    /// Dialog can validate inputs and decide whether to confirm.
    ///
    /// Default: Do nothing (Enter doesn't auto-confirm)
    fn request_confirm(&mut self) -> DialogAction<M> {
        DialogAction::None
    }

    /// Handle events. Return `Some(action)` if the event was handled.
    ///
    /// Note: Escape and Enter are handled automatically via `request_cancel`/`request_confirm`.
    /// This method is for other events like Tab navigation, custom hotkeys, etc.
    ///
    /// Default: Pass all events through (not handled)
    fn handle_event(&mut self, _event: &Event) -> Option<DialogAction<M>> {
        None
    }

    /// Whether clicking outside the dialog should trigger `request_cancel()`.
    ///
    /// Default: true (only applies to Modal style dialogs)
    fn close_on_blur(&self) -> bool {
        true
    }

    /// Override the application theme while this dialog is displayed.
    ///
    /// This is useful for dialogs that need to preview theme changes
    /// (e.g., settings dialog with theme selection).
    ///
    /// Default: None (use application's current theme)
    fn theme(&self) -> Option<Theme> {
        None
    }

    /// The display style for this dialog.
    ///
    /// - `Modal`: Centered dialog with dimmed background overlay (default)
    /// - `Fullscreen`: Dialog content fills the entire window
    ///
    /// Default: Modal
    fn style(&self) -> DialogStyle {
        DialogStyle::Modal
    }
}

/// A stack of modal dialogs.
///
/// Dialogs are rendered bottom-to-top (last = topmost).
/// The topmost dialog receives all events.
pub struct DialogStack<M> {
    dialogs: Vec<Box<dyn Dialog<M>>>,
}

impl<M> Default for DialogStack<M>
where
    M: Send + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<M: Send + 'static> DialogStack<M> {
    /// Create an empty dialog stack
    pub fn new() -> Self {
        Self { dialogs: Vec::new() }
    }

    /// Check if any dialogs are open
    pub fn is_empty(&self) -> bool {
        self.dialogs.is_empty()
    }

    /// Get the number of open dialogs
    pub fn len(&self) -> usize {
        self.dialogs.len()
    }

    /// Push a new dialog onto the stack
    pub fn push(&mut self, dialog: impl Dialog<M> + 'static) {
        self.dialogs.push(Box::new(dialog));
    }

    /// Push a boxed dialog onto the stack
    pub fn push_boxed(&mut self, dialog: Box<dyn Dialog<M>>) {
        self.dialogs.push(dialog);
    }

    /// Pop the topmost dialog from the stack
    pub fn pop(&mut self) -> Option<Box<dyn Dialog<M>>> {
        self.dialogs.pop()
    }

    /// Clear all dialogs
    pub fn clear(&mut self) {
        self.dialogs.clear();
    }

    /// Get a mutable reference to the topmost dialog
    pub fn top_mut(&mut self) -> Option<&mut Box<dyn Dialog<M>>> {
        self.dialogs.last_mut()
    }

    /// Get the theme override from the topmost dialog, if any.
    ///
    /// Use this in your application's `theme()` method:
    /// ```ignore
    /// fn theme(&self) -> Theme {
    ///     self.dialogs.theme().unwrap_or_else(|| self.settings.theme())
    /// }
    /// ```
    pub fn theme(&self) -> Option<Theme> {
        self.dialogs.last().and_then(|d| d.theme())
    }

    /// Render all dialogs over the background content.
    ///
    /// Each dialog is rendered according to its style:
    /// - `Modal`: Wrapped with a semi-transparent overlay, content centered
    /// - `Fullscreen`: Content fills the entire window, no overlay
    pub fn view<'a>(&'a self, mut background: iced::Element<'a, M>) -> iced::Element<'a, M>
    where
        M: Clone + 'a,
    {
        for dialog in &self.dialogs {
            let content = dialog.view();
            match dialog.style() {
                DialogStyle::Modal => {
                    background = super::modal_overlay(background, content);
                }
                DialogStyle::Fullscreen => {
                    // Fullscreen dialogs replace the background entirely
                    background = content;
                }
            }
        }
        background
    }

    /// Handle an event, routing to the topmost dialog.
    /// Returns a Task with any resulting messages.
    ///
    /// Automatically handles:
    /// - Escape → `request_cancel()`
    /// - Enter → `request_confirm()`
    /// - Other events → `handle_event()`
    pub fn handle_event(&mut self, event: &Event) -> Task<M> {
        let Some(dialog) = self.dialogs.last_mut() else {
            return Task::none();
        };

        // Check for Escape/Enter first
        if let Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) = event {
            match key {
                keyboard::Key::Named(keyboard::key::Named::Escape) => {
                    let action = dialog.request_cancel();
                    return self.process_action(action);
                }
                keyboard::Key::Named(keyboard::key::Named::Enter) => {
                    let action = dialog.request_confirm();
                    return self.process_action(action);
                }
                _ => {}
            }
        }

        // Let dialog handle other events
        if let Some(action) = dialog.handle_event(event) {
            return self.process_action(action);
        }

        Task::none()
    }

    /// Route a message to the topmost dialog for processing.
    ///
    /// Returns `Some(Task)` if the dialog handled the message,
    /// `None` if the message should be handled by the application.
    ///
    /// Use this in your application's `update()` method:
    /// ```ignore
    /// if let Some(task) = self.dialogs.update(&message) {
    ///     return task;
    /// }
    /// // ... normal message handling
    /// ```
    pub fn update(&mut self, message: &M) -> Option<Task<M>> {
        let dialog = self.dialogs.last_mut()?;
        let action = dialog.update(message)?;
        Some(self.process_action(action))
    }

    /// Process a DialogAction and return the resulting Task
    fn process_action(&mut self, action: DialogAction<M>) -> Task<M> {
        match action {
            DialogAction::None => Task::none(),
            DialogAction::Close => {
                self.dialogs.pop();
                Task::none()
            }
            DialogAction::CloseWith(msg) => {
                self.dialogs.pop();
                Task::done(msg)
            }
            DialogAction::Push(dialog) => {
                self.dialogs.push(dialog);
                Task::none()
            }
            DialogAction::SendMessage(msg) => Task::done(msg),
            DialogAction::RunTask(task) => task,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    enum TestMsg {
        Cancelled,
        Confirmed(String),
    }

    struct TestDialog {
        value: String,
        allow_confirm: bool,
    }

    impl Dialog<TestMsg> for TestDialog {
        fn view(&self) -> iced::Element<'_, TestMsg> {
            // Return empty container for testing
            iced::widget::container(iced::widget::text("test")).into()
        }

        fn request_cancel(&mut self) -> DialogAction<TestMsg> {
            DialogAction::CloseWith(TestMsg::Cancelled)
        }

        fn request_confirm(&mut self) -> DialogAction<TestMsg> {
            if self.allow_confirm {
                DialogAction::CloseWith(TestMsg::Confirmed(self.value.clone()))
            } else {
                DialogAction::None
            }
        }
    }

    #[test]
    fn test_empty_stack() {
        let stack: DialogStack<TestMsg> = DialogStack::new();
        assert!(stack.is_empty());
        assert_eq!(stack.len(), 0);
    }

    #[test]
    fn test_push_pop() {
        let mut stack: DialogStack<TestMsg> = DialogStack::new();
        stack.push(TestDialog {
            value: "test".into(),
            allow_confirm: true,
        });
        assert!(!stack.is_empty());
        assert_eq!(stack.len(), 1);

        stack.pop();
        assert!(stack.is_empty());
    }
}
