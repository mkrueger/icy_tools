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

/// BitFont editor command definitions
pub mod bitfont_cmd {
    use icy_engine_gui::define_commands;

    const TOML: &str = include_str!("../../data/commands_draw.toml");

    define_commands! {
        loader: crate::LANGUAGE_LOADER,
        commands: TOML,

        BITFONT_CLEAR = "bitfont.clear",
        BITFONT_FILL = "bitfont.fill",
        BITFONT_INVERSE = "bitfont.inverse",
        BITFONT_FLIP_X = "bitfont.flip_x",
        BITFONT_FLIP_Y = "bitfont.flip_y",
        BITFONT_TOGGLE_LETTER_SPACING = "bitfont.toggle_letter_spacing",
        BITFONT_SWAP_CHARS = "bitfont.swap_chars",
        BITFONT_DUPLICATE_LINE = "bitfont.duplicate_line",
        BITFONT_SHOW_PREVIEW = "bitfont.show_preview",
    }
}
