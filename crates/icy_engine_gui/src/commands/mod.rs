//! Command System for icy_engine_gui
//!
//! Provides a flexible, string-based command system with:
//! - Platform-specific hotkeys (Mac vs. Win/Linux)
//! - TOML-compatible textual definitions
//! - Configurable key bindings
//!
//! # Example TOML format:
//! ```toml
//! [[commands]]
//! id = "copy"
//! hotkey = ["Ctrl+C"]
//! hotkey_mac = ["Cmd+C"]
//! ```

mod hotkey;
mod command_def;
mod command_set;
mod toml_loader;
mod defaults;
mod iced_adapter;

pub use hotkey::{Hotkey, KeyCode, Modifiers};
pub use command_def::CommandDef;
pub use command_set::CommandSet;
pub use toml_loader::{load_commands_from_str, load_commands_from_file, CommandLoadError};
pub use defaults::{create_common_commands, cmd};
pub use iced_adapter::{hotkey_from_iced, from_iced_modifiers, from_iced_key};

#[cfg(test)]
mod tests;
