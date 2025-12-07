//! Command definitions for icy_draw
//!
//! Extends the common commands from icy_engine_gui with draw-specific commands.

#![allow(dead_code)]

use icy_engine_gui::commands::{CommandSet, create_common_commands, load_commands_from_str};

/// The embedded draw-specific commands TOML
const DRAW_COMMANDS_TOML: &str = include_str!("../../data/commands_draw.toml");

/// Create the command set for icy_draw
///
/// Includes common commands plus draw-specific commands.
pub fn create_draw_commands() -> CommandSet {
    let mut commands = create_common_commands();

    // Load and merge draw-specific commands
    if let Ok(draw_commands) = load_commands_from_str(DRAW_COMMANDS_TOML) {
        commands.merge(draw_commands);
    }

    commands
}
