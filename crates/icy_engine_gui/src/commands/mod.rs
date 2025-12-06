//! Command System for icy_engine_gui
//!
//! Provides a flexible, string-based command system with:
//! - Platform-specific hotkeys (Mac vs. Win/Linux)
//! - TOML-compatible textual definitions
//! - Configurable key bindings
//! - Declarative command handler macros
//!
//! # Example TOML format:
//! ```toml
//! [[commands]]
//! id = "copy"
//! hotkey = ["Ctrl+C"]
//! hotkey_mac = ["Cmd+C"]
//! ```
//!
//! # Example macro usage:
//! ```ignore
//! command_handlers! {
//!     fn my_handler(window_id: Id) -> Option<Message> {
//!         cmd::WINDOW_NEW => Message::OpenWindow,
//!         cmd::WINDOW_CLOSE => Message::CloseWindow(window_id),
//!     }
//! }
//!
//! if let Some(msg) = handle_command!(commands, &hotkey, my_handler, window_id) {
//!     return Task::done(msg);
//! }
//! ```

#[macro_use]
mod macros;

mod command_def;
mod command_handler;
mod command_set;
mod defaults;
mod hotkey;
mod iced_adapter;
mod toml_loader;

pub use command_def::CommandDef;
pub use command_handler::CommandHandler;
pub use command_set::{CategoryMeta, CommandSet, HelpCommandInfo};
pub use defaults::{cmd, create_common_commands};
pub use hotkey::{Hotkey, KeyCode, Modifiers, MouseBinding, MouseButton};
pub use iced_adapter::{IntoHotkey, from_iced_key, from_iced_modifiers, from_iced_mouse_button, hotkey_from_iced, mouse_binding_from_iced, try_handle_event};
pub use toml_loader::{CommandLoadError, load_commands_from_file, load_commands_from_str};

#[cfg(test)]
mod tests;
