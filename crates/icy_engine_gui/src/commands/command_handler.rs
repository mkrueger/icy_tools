//! Command Handler
//!
//! Combines a CommandSet with a handler function for clean event handling.

use super::{CommandSet, IntoHotkey};

/// A command handler that combines a CommandSet with a handler function.
///
/// # Type Parameters
/// - `M`: The message type returned by the handler
/// - `C`: The context type passed to the handler (e.g., window::Id)
///
/// # Example
/// ```ignore
/// // Create with the command_handler! macro:
/// let commands = command_handler!(create_common_commands(), window_id: window::Id => WindowManagerMessage {
///     cmd::WINDOW_NEW => WindowManagerMessage::OpenWindow,
///     cmd::WINDOW_CLOSE => WindowManagerMessage::CloseWindow(window_id),
/// });
///
/// // Use in event handling:
/// if let Some(msg) = commands.handle(&event, current_window_id) {
///     return Task::done(msg);
/// }
/// ```
pub struct CommandHandler<M, C> {
    commands: CommandSet,
    handler: fn(&str, C) -> Option<M>,
}

impl<M, C: Copy> CommandHandler<M, C> {
    /// Create a new CommandHandler with the given commands and handler function
    pub fn new(commands: CommandSet, handler: fn(&str, C) -> Option<M>) -> Self {
        Self { commands, handler }
    }

    /// Handle an event, returning a message if a command matches
    pub fn handle<H: IntoHotkey>(&self, event: H, ctx: C) -> Option<M> {
        let hotkey = event.into_hotkey()?;
        let cmd_id = self.commands.match_hotkey(&hotkey)?;
        (self.handler)(cmd_id, ctx)
    }

    /// Get a reference to the underlying CommandSet
    pub fn commands(&self) -> &CommandSet {
        &self.commands
    }

    /// Get a mutable reference to the underlying CommandSet
    pub fn commands_mut(&mut self) -> &mut CommandSet {
        &mut self.commands
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{cmd, create_common_commands, macros::CommandId, Hotkey};

    #[derive(Debug, Clone, PartialEq)]
    enum TestMessage {
        Open,
        Close(u32),
    }

    fn test_handler(cmd_id: &str, window_id: u32) -> Option<TestMessage> {
        if cmd_id == cmd::WINDOW_NEW.command_id() {
            Some(TestMessage::Open)
        } else if cmd_id == cmd::WINDOW_CLOSE.command_id() {
            Some(TestMessage::Close(window_id))
        } else {
            None
        }
    }

    #[test]
    fn test_command_handler() {
        let handler = CommandHandler::new(create_common_commands(), test_handler);

        let hotkey = Hotkey::parse("Ctrl+Shift+N").unwrap();
        assert_eq!(handler.handle(&hotkey, 42), Some(TestMessage::Open));

        let hotkey = Hotkey::parse("Ctrl+W").unwrap();
        assert_eq!(handler.handle(&hotkey, 42), Some(TestMessage::Close(42)));

        let hotkey = Hotkey::parse("Ctrl+X").unwrap();
        assert_eq!(handler.handle(&hotkey, 42), None);
    }
}
