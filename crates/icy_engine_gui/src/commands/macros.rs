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

        impl $name {
            fn new() -> Self {
                Self { commands: $commands_init }
            }

            fn handle<H: $crate::commands::IntoHotkey>(&self, event: H, $ctx: $ctx_ty) -> Option<$msg_ty> {
                let hotkey = event.into_hotkey()?;
                let cmd_id = self.commands.match_hotkey(&hotkey)?;
                $( if cmd_id == $cmd { return Some($msg); } )*
                None
            }

            #[allow(dead_code)]
            fn commands(&self) -> &$crate::commands::CommandSet {
                &self.commands
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

        impl $name {
            fn new() -> Self {
                Self { commands: $commands_init }
            }

            fn handle<H: $crate::commands::IntoHotkey>(&self, event: H, ( $( $ctx ),+ ): ( $( $ctx_ty ),+ )) -> Option<$msg_ty> {
                let hotkey = event.into_hotkey()?;
                let cmd_id = self.commands.match_hotkey(&hotkey)?;
                $( if cmd_id == $cmd { return Some($msg); } )*
                None
            }

            #[allow(dead_code)]
            fn commands(&self) -> &$crate::commands::CommandSet {
                &self.commands
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

        impl $name {
            fn new() -> Self {
                Self { commands: $commands_init }
            }

            fn handle<H: $crate::commands::IntoHotkey>(&self, event: H) -> Option<$msg_ty> {
                let hotkey = event.into_hotkey()?;
                let cmd_id = self.commands.match_hotkey(&hotkey)?;
                $( if cmd_id == $cmd { return Some($msg); } )*
                None
            }

            #[allow(dead_code)]
            fn commands(&self) -> &$crate::commands::CommandSet {
                &self.commands
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
///     if command_id == CMD_ID_1 { return Some(message_expression); }
///     if command_id == CMD_ID_2 { return Some(message_using(ctx1, ctx2)); }
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
            $( if command_id == $cmd { return Some($msg); } )*
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
            $( if command_id == $cmd { return Some($msg); } )*
            None
        }
    };
}

/// Convenience macro to match a hotkey and call a handler in one step.
///
/// # Syntax
/// ```ignore
/// // With Iced Event (extracts hotkey automatically)
/// if let Some(msg) = handle_command!(commands, &event, handler, param1, param2) {
///     return Task::done(msg);
/// }
///
/// // With Hotkey directly
/// if let Some(msg) = handle_command!(commands, &hotkey, handler, param1, param2) {
///     return Task::done(msg);
/// }
/// ```
#[macro_export]
macro_rules! handle_command {
    ($commands:expr, $event_or_hotkey:expr, $handler:ident $(, $param:expr)*) => {
        $crate::commands::try_handle_event($commands, $event_or_hotkey)
            .and_then(|cmd_id| $handler(cmd_id $(, $param)*))
    };
}

#[cfg(test)]
mod tests {
    use crate::commands::{cmd, create_common_commands, Hotkey};

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
        assert_eq!(simple_handler(cmd::FILE_OPEN), Some(TestMessage::Open));
        assert_eq!(simple_handler("unknown"), None);
    }

    #[test]
    fn test_window_handler() {
        let window_id = 42u32;
        
        assert_eq!(window_handler(cmd::WINDOW_NEW, window_id), Some(TestMessage::Open));
        assert_eq!(window_handler(cmd::WINDOW_CLOSE, window_id), Some(TestMessage::Close(42)));
        assert_eq!(window_handler(cmd::VIEW_ZOOM_IN, window_id), Some(TestMessage::ZoomIn(42)));
        assert_eq!(window_handler("unknown", window_id), None);
    }

    #[test]
    fn test_multi_param_handler() {
        assert_eq!(
            multi_param_handler(cmd::WINDOW_NEW, 1, 100, 200),
            Some(TestMessage::Move(1, 100, 200))
        );
    }

    #[test]
    fn test_handle_command_macro() {
        let commands = create_common_commands();
        let hotkey = Hotkey::parse("Ctrl+Shift+N").unwrap();
        let window_id = 1u32;

        let result = handle_command!(&commands, &hotkey, window_handler, window_id);
        assert_eq!(result, Some(TestMessage::Open));
    }

    #[test]
    fn test_full_workflow() {
        let commands = create_common_commands();
        let window_id = 42u32;

        // Simulate various hotkeys
        let test_cases = [
            ("Ctrl+Shift+N", Some(TestMessage::Open)),
            ("Ctrl+W", Some(TestMessage::Close(42))),
            ("Ctrl++", Some(TestMessage::ZoomIn(42))),
            ("Ctrl+-", Some(TestMessage::ZoomOut(42))),
            ("Ctrl+X", None), // Unknown command
        ];

        for (hotkey_str, expected) in test_cases {
            let hotkey = Hotkey::parse(hotkey_str).unwrap();
            let result = handle_command!(&commands, &hotkey, window_handler, window_id);
            assert_eq!(result, expected, "Failed for hotkey: {}", hotkey_str);
        }
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
