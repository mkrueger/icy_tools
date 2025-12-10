//! Command System for icy_engine_gui
//!
//! Provides a flexible, string-based command system with:
//! - Platform-specific hotkeys (Mac vs. Win/Linux)
//! - TOML-compatible textual definitions
//! - Configurable key bindings
//! - Declarative command handler macros
//! - LazyLock<CommandDef> statics with embedded translations
//!
//! # Example TOML format:
//! ```toml
//! [[commands]]
//! id = "copy"
//! hotkey = ["Ctrl+C"]
//! hotkey_mac = ["Cmd+C"]
//! ```
//!
//! # Defining commands with TOML and translation source:
//! ```ignore
//! define_commands! {
//!     loader: crate::LANGUAGE_LOADER,
//!     commands: include_str!("../../data/commands_common.toml"),
//!
//!     FILE_NEW = "file.new",
//!     FILE_SAVE = "file.save",
//! }
//! ```
//!
//! # Example macro usage:
//! ```ignore
//! command_handler!(WindowCommands, create_common_commands(), window_id: Id => Message {
//!     cmd::WINDOW_NEW => Message::OpenWindow,
//!     cmd::WINDOW_CLOSE => Message::CloseWindow(window_id),
//! });
//!
//! if let Some(msg) = commands.handle(&event, window_id) {
//!     return Task::done(msg);
//! }
//! ```

#[macro_use]
pub mod macros;

mod command_def;
mod command_handler;
pub mod command_ref;
mod command_set;
mod defaults;
mod hotkey;
mod iced_adapter;
mod toml_loader;

pub use command_def::CommandDef;
pub use command_handler::CommandHandler;
pub use command_set::{CategoryMeta, CommandSet, HelpCommandInfo, format_command_set_debug};
pub use defaults::{cmd, create_common_commands};
pub use hotkey::{Hotkey, KeyCode, Modifiers, MouseBinding, MouseButton};
pub use iced_adapter::{IntoHotkey, from_iced_key, from_iced_modifiers, from_iced_mouse_button, hotkey_from_iced, mouse_binding_from_iced};
pub use toml_loader::{CommandLoadError, CommandToml, load_commands_from_file, load_commands_from_str};

#[cfg(test)]
mod tests;
