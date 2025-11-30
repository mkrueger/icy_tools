//! Lua scripting support for icy_term
//!
//! This module provides terminal-specific Lua scripting capabilities,
//! extending the base functionality from `icy_engine_scripting`.

mod script_runner;
mod terminal_extension;

pub use script_runner::{ScriptResult, ScriptRunner};
pub use terminal_extension::{TerminalLuaExtension, parse_key_string};
