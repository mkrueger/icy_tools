//! Command Mapping Macros
//!
//! Provides declarative macros for creating command handlers.
//!
//! # Example
//! ```ignore
//! use icy_engine_gui::command_handler;
//! use icy_engine_gui::commands::{cmd, create_common_commands};
//!
//! // Create a command handler with embedded mappings:
//! // Commands can be LazyLock<CommandDef> or plain &str
//! command_handler!(WindowCommands, create_common_commands(), window_id: window::Id => WindowManagerMessage {
//!     cmd::WINDOW_NEW => WindowManagerMessage::OpenWindow,
//!     cmd::WINDOW_CLOSE => WindowManagerMessage::CloseWindow(window_id),
//! });
//!
//! // Usage:
//! let commands = WindowCommands::new();
//! if let Some(msg) = commands.handle(&event, current_window_id) {
//!     return Task::done(msg);
//! }
//! ```

use std::sync::LazyLock;

use super::CommandDef;

/// Helper trait to get the command ID from LazyLock<CommandDef>, CommandDef, or &str
pub trait CommandId {
    fn command_id(&self) -> &str;
}

impl CommandId for &str {
    fn command_id(&self) -> &str {
        self
    }
}

impl CommandId for CommandDef {
    fn command_id(&self) -> &str {
        &self.id
    }
}

impl CommandId for &CommandDef {
    fn command_id(&self) -> &str {
        &self.id
    }
}

impl CommandId for LazyLock<CommandDef> {
    fn command_id(&self) -> &str {
        let cmd: &CommandDef = self;
        &cmd.id
    }
}

impl CommandId for &LazyLock<CommandDef> {
    fn command_id(&self) -> &str {
        let cmd: &CommandDef = *self;
        &cmd.id
    }
}

/// Creates a CommandHandler struct with embedded command mappings.
///
/// # Syntax
/// ```ignore
/// // Single context parameter:
/// command_handler!(StructName, command_set, ctx: CtxType => MessageType {
///     CMD_ID_1 => message_expression,
///     CMD_ID_2 => message_using(ctx),
/// });
///
/// // Multiple context parameters:
/// command_handler!(StructName, command_set, (ctx1, ctx2): (Type1, Type2) => MessageType {
///     CMD_ID_1 => message_using(ctx1, ctx2),
/// });
///
/// // No context parameter:
/// command_handler!(StructName, command_set, => MessageType {
///     CMD_ID_1 => MessageType::DoSomething,
/// });
/// ```
///
/// Commands can be either:
/// - `CommandRef` statics (with embedded translation source)
/// - Plain `&str` constants
///
/// The generated struct also implements `Debug` for easy debugging,
/// showing all commands with their IDs, hotkeys (platform-specific), and categories.
#[macro_export]
macro_rules! command_handler {
    // Pattern: Single context parameter
    (
        $name:ident,
        $commands_init:expr,
        $ctx:ident : $ctx_ty:ty => $msg_ty:ty {
            $( $cmd:expr => $msg:expr ),* $(,)?
        }
    ) => {
        struct $name {
            commands: $crate::commands::CommandSet,
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                $crate::commands::format_command_set_debug(&self.commands, stringify!($name), f)
            }
        }

        impl $name {
            fn new() -> Self {
                Self { commands: $commands_init }
            }

            fn handle<H: $crate::commands::IntoHotkey>(&self, event: H, $ctx: $ctx_ty) -> Option<$msg_ty> {
                use $crate::commands::macros::CommandId;
                // Try hotkey (keyboard) match first
                if let Some(hotkey) = event.into_hotkey() {
                    if let Some(cmd_id) = self.commands.match_hotkey(&hotkey) {
                        $( if cmd_id == $cmd.command_id() { return Some($msg); } )*
                    }
                }
                // Try mouse binding match
                if let Some(mouse_binding) = event.into_mouse_binding() {
                    if let Some(cmd_id) = self.commands.match_mouse_binding(&mouse_binding) {
                        $( if cmd_id == $cmd.command_id() { return Some($msg); } )*
                    }
                }
                None
            }

            #[allow(dead_code)]
            fn commands(&self) -> &$crate::commands::CommandSet {
                &self.commands
            }

            #[allow(dead_code)]
            fn commands_mut(&mut self) -> &mut $crate::commands::CommandSet {
                &mut self.commands
            }
        }
    };

    // Pattern: Tuple context parameter (multiple params)
    (
        $name:ident,
        $commands_init:expr,
        ( $( $ctx:ident ),+ ) : ( $( $ctx_ty:ty ),+ ) => $msg_ty:ty {
            $( $cmd:expr => $msg:expr ),* $(,)?
        }
    ) => {
        struct $name {
            commands: $crate::commands::CommandSet,
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                $crate::commands::format_command_set_debug(&self.commands, stringify!($name), f)
            }
        }

        impl $name {
            fn new() -> Self {
                Self { commands: $commands_init }
            }

            fn handle<H: $crate::commands::IntoHotkey>(&self, event: H, ( $( $ctx ),+ ): ( $( $ctx_ty ),+ )) -> Option<$msg_ty> {
                use $crate::commands::macros::CommandId;
                // Try hotkey (keyboard) match first
                if let Some(hotkey) = event.into_hotkey() {
                    if let Some(cmd_id) = self.commands.match_hotkey(&hotkey) {
                        $( if cmd_id == $cmd.command_id() { return Some($msg); } )*
                    }
                }
                // Try mouse binding match
                if let Some(mouse_binding) = event.into_mouse_binding() {
                    if let Some(cmd_id) = self.commands.match_mouse_binding(&mouse_binding) {
                        $( if cmd_id == $cmd.command_id() { return Some($msg); } )*
                    }
                }
                None
            }

            #[allow(dead_code)]
            fn commands(&self) -> &$crate::commands::CommandSet {
                &self.commands
            }

            #[allow(dead_code)]
            fn commands_mut(&mut self) -> &mut $crate::commands::CommandSet {
                &mut self.commands
            }
        }
    };

    // Pattern: No context parameter
    (
        $name:ident,
        $commands_init:expr,
        => $msg_ty:ty {
            $( $cmd:expr => $msg:expr ),* $(,)?
        }
    ) => {
        struct $name {
            commands: $crate::commands::CommandSet,
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                $crate::commands::format_command_set_debug(&self.commands, stringify!($name), f)
            }
        }

        impl $name {
            fn new() -> Self {
                Self { commands: $commands_init }
            }

            fn handle<H: $crate::commands::IntoHotkey>(&self, event: H) -> Option<$msg_ty> {
                use $crate::commands::macros::CommandId;
                // Try hotkey (keyboard) match first
                if let Some(hotkey) = event.into_hotkey() {
                    if let Some(cmd_id) = self.commands.match_hotkey(&hotkey) {
                        $( if cmd_id == $cmd.command_id() { return Some($msg); } )*
                    }
                }
                // Try mouse binding match
                if let Some(mouse_binding) = event.into_mouse_binding() {
                    if let Some(cmd_id) = self.commands.match_mouse_binding(&mouse_binding) {
                        $( if cmd_id == $cmd.command_id() { return Some($msg); } )*
                    }
                }
                None
            }

            #[allow(dead_code)]
            fn commands(&self) -> &$crate::commands::CommandSet {
                &self.commands
            }

            #[allow(dead_code)]
            fn commands_mut(&mut self) -> &mut $crate::commands::CommandSet {
                &mut self.commands
            }
        }
    };
}

/// Creates a command handler function that maps command IDs to messages.
///
/// # Syntax
/// ```ignore
/// command_handlers! {
///     fn handler_name(ctx1: Type1, ctx2: Type2) -> Option<MessageType> {
///         CMD_ID_1 => message_expression,
///         CMD_ID_2 => message_using(ctx1, ctx2),
///     }
/// }
/// ```
///
/// This generates a function:
/// ```ignore
/// fn handler_name(command_id: &str, ctx1: Type1, ctx2: Type2) -> Option<MessageType> {
///     if command_id == CMD_ID_1.command_id() { return Some(message_expression); }
///     if command_id == CMD_ID_2.command_id() { return Some(message_using(ctx1, ctx2)); }
///     None
/// }
/// ```
#[macro_export]
macro_rules! command_handlers {
    // Pattern: fn name(params) -> Option<Type> { mappings }
    (
        fn $name:ident( $( $param:ident : $ptype:ty ),* $(,)? ) -> Option<$ret:ty> {
            $( $cmd:expr => $msg:expr ),* $(,)?
        }
    ) => {
        fn $name(command_id: &str $(, $param: $ptype)*) -> Option<$ret> {
            use $crate::commands::macros::CommandId;
            $( if command_id == $cmd.command_id() { return Some($msg); } )*
            None
        }
    };

    // Pattern: pub fn name(params) -> Option<Type> { mappings }
    (
        pub fn $name:ident( $( $param:ident : $ptype:ty ),* $(,)? ) -> Option<$ret:ty> {
            $( $cmd:expr => $msg:expr ),* $(,)?
        }
    ) => {
        pub fn $name(command_id: &str $(, $param: $ptype)*) -> Option<$ret> {
            use $crate::commands::macros::CommandId;
            $( if command_id == $cmd.command_id() { return Some($msg); } )*
            None
        }
    };
}

#[cfg(test)]
mod tests {
    use crate::commands::{cmd, create_common_commands, macros::CommandId, Hotkey};

    #[derive(Debug, Clone, PartialEq)]
    enum TestMessage {
        Open,
        Close(u32),
        ZoomIn(u32),
        ZoomOut(u32),
        Move(u32, i32, i32),
    }

    command_handlers! {
        fn simple_handler() -> Option<TestMessage> {
            cmd::FILE_OPEN => TestMessage::Open,
        }
    }

    command_handlers! {
        fn window_handler(window_id: u32) -> Option<TestMessage> {
            cmd::WINDOW_NEW => TestMessage::Open,
            cmd::WINDOW_CLOSE => TestMessage::Close(window_id),
            cmd::VIEW_ZOOM_IN => TestMessage::ZoomIn(window_id),
            cmd::VIEW_ZOOM_OUT => TestMessage::ZoomOut(window_id),
        }
    }

    command_handlers! {
        fn multi_param_handler(window_id: u32, x: i32, y: i32) -> Option<TestMessage> {
            cmd::WINDOW_NEW => TestMessage::Move(window_id, x, y),
        }
    }

    #[test]
    fn test_simple_handler() {
        assert_eq!(simple_handler(cmd::FILE_OPEN.command_id()), Some(TestMessage::Open));
        assert_eq!(simple_handler("unknown"), None);
    }

    #[test]
    fn test_window_handler() {
        let window_id = 42u32;

        assert_eq!(window_handler(cmd::WINDOW_NEW.command_id(), window_id), Some(TestMessage::Open));
        assert_eq!(window_handler(cmd::WINDOW_CLOSE.command_id(), window_id), Some(TestMessage::Close(42)));
        assert_eq!(window_handler(cmd::VIEW_ZOOM_IN.command_id(), window_id), Some(TestMessage::ZoomIn(42)));
        assert_eq!(window_handler("unknown", window_id), None);
    }

    #[test]
    fn test_multi_param_handler() {
        assert_eq!(
            multi_param_handler(cmd::WINDOW_NEW.command_id(), 1, 100, 200),
            Some(TestMessage::Move(1, 100, 200))
        );
    }

    // Tests for command_handler! macro (struct generation)

    command_handler!(SingleParamCommands, create_common_commands(), window_id: u32 => TestMessage {
        cmd::WINDOW_NEW => TestMessage::Open,
        cmd::WINDOW_CLOSE => TestMessage::Close(window_id),
    });

    command_handler!(TupleParamCommands, create_common_commands(), (window_id, offset): (u32, i32) => TestMessage {
        cmd::WINDOW_NEW => TestMessage::Move(window_id, offset, 0),
    });

    command_handler!(NoParamCommands, create_common_commands(), => TestMessage {
        cmd::FILE_OPEN => TestMessage::Open,
    });

    #[test]
    fn test_command_handler_single_param() {
        let handler = SingleParamCommands::new();

        let hotkey = Hotkey::parse("Ctrl+Shift+N").unwrap();
        assert_eq!(handler.handle(&hotkey, 42), Some(TestMessage::Open));

        let hotkey = Hotkey::parse("Ctrl+W").unwrap();
        assert_eq!(handler.handle(&hotkey, 42), Some(TestMessage::Close(42)));
    }

    #[test]
    fn test_command_handler_tuple_param() {
        let handler = TupleParamCommands::new();

        let hotkey = Hotkey::parse("Ctrl+Shift+N").unwrap();
        assert_eq!(handler.handle(&hotkey, (42, 10)), Some(TestMessage::Move(42, 10, 0)));
    }

    #[test]
    fn test_command_handler_no_param() {
        let handler = NoParamCommands::new();

        let hotkey = Hotkey::parse("Ctrl+O").unwrap();
        assert_eq!(handler.handle(&hotkey), Some(TestMessage::Open));
    }
}
