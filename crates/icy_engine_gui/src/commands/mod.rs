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

mod hotkey;
mod command_def;
mod command_set;
mod command_handler;
mod toml_loader;
mod defaults;
mod iced_adapter;

pub use hotkey::{Hotkey, KeyCode, Modifiers, MouseButton, MouseBinding};
pub use command_def::CommandDef;
pub use command_set::CommandSet;
pub use command_handler::CommandHandler;
pub use toml_loader::{load_commands_from_str, load_commands_from_file, CommandLoadError};
pub use defaults::{create_common_commands, cmd};
pub use iced_adapter::{hotkey_from_iced, from_iced_modifiers, from_iced_key, from_iced_mouse_button, mouse_binding_from_iced, try_handle_event, IntoHotkey};

#[cfg(test)]
mod tests;
