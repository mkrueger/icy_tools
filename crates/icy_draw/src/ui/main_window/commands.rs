//! Command definitions for icy_draw
//!
//! Extends the common commands from icy_engine_gui with draw-specific commands.

#![allow(dead_code)]

use icy_engine_gui::commands::{create_common_commands, load_commands_from_str, CommandSet};

/// The embedded draw-specific commands TOML
const DRAW_COMMANDS_TOML: &str = include_str!("../../../data/commands_draw.toml");

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

/// Color-related command definitions
pub mod color_cmd {
    use icy_engine_gui::define_commands;

    const TOML: &str = include_str!("../../../data/commands_draw.toml");

    define_commands! {
        loader: crate::LANGUAGE_LOADER,
        commands: TOML,

        NEXT_FG = "color.next_fg",
        PREV_FG = "color.prev_fg",
        NEXT_BG = "color.next_bg",
        PREV_BG = "color.prev_bg",
        PICK_ATTRIBUTE_UNDER_CARET = "color.pick_attribute_under_caret",
        SWAP = "color.swap",
    }
}

/// View-related (draw-specific) command definitions
pub mod view_cmd {
    use icy_engine_gui::define_commands;

    const TOML: &str = include_str!("../../../data/commands_draw.toml");

    define_commands! {
        loader: crate::LANGUAGE_LOADER,
        commands: TOML,

        REFERENCE_IMAGE = "view.reference_image",
        TOGGLE_REFERENCE_IMAGE = "view.toggle_reference_image",
    }
}

/// Selection command definitions
pub mod selection_cmd {
    use icy_engine_gui::define_commands;

    const TOML: &str = include_str!("../../../data/commands_draw.toml");

    define_commands! {
        loader: crate::LANGUAGE_LOADER,
        commands: TOML,

        SELECT_NONE = "select.none",
        SELECT_INVERSE = "select.inverse",
        SELECT_FLIP_X = "select.flip_x",
        SELECT_FLIP_Y = "select.flip_y",
        SELECT_CROP = "select.crop",
        SELECT_JUSTIFY_LEFT = "select.justify_left",
        SELECT_JUSTIFY_CENTER = "select.justify_center",
        SELECT_JUSTIFY_RIGHT = "select.justify_right",
    }
}

/// Area operations command definitions
pub mod area_cmd {
    use icy_engine_gui::define_commands;

    const TOML: &str = include_str!("../../../data/commands_draw.toml");

    define_commands! {
        loader: crate::LANGUAGE_LOADER,
        commands: TOML,

        JUSTIFY_LINE_LEFT = "area.justify_line_left",
        JUSTIFY_LINE_CENTER = "area.justify_line_center",
        JUSTIFY_LINE_RIGHT = "area.justify_line_right",
        INSERT_ROW = "area.insert_row",
        DELETE_ROW = "area.delete_row",
        INSERT_COLUMN = "area.insert_column",
        DELETE_COLUMN = "area.delete_column",
        ERASE_ROW = "area.erase_row",
        ERASE_ROW_TO_START = "area.erase_row_to_start",
        ERASE_ROW_TO_END = "area.erase_row_to_end",
        ERASE_COLUMN = "area.erase_column",
        ERASE_COLUMN_TO_START = "area.erase_column_to_start",
        ERASE_COLUMN_TO_END = "area.erase_column_to_end",
        SCROLL_UP = "area.scroll_up",
        SCROLL_DOWN = "area.scroll_down",
        SCROLL_LEFT = "area.scroll_left",
        SCROLL_RIGHT = "area.scroll_right",
    }
}

/// BitFont editor command definitions
pub mod bitfont_cmd {
    use icy_engine_gui::define_commands;

    const TOML: &str = include_str!("../../../data/commands_draw.toml");

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
