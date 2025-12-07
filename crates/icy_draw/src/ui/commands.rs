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

/// Command IDs specific to icy_draw
pub mod cmd {
    // File operations
    pub const FILE_NEW: &str = "file.new";
    pub const FILE_SAVE: &str = "file.save";
    pub const FILE_SAVE_AS: &str = "file.save_as";
    
    // Edit operations
    pub const EDIT_UNDO: &str = "edit.undo";
    pub const EDIT_REDO: &str = "edit.redo";
    pub const EDIT_CUT: &str = "edit.cut";
    
    // Selection
    pub const SELECT_NONE: &str = "select.none";
    pub const SELECT_INVERSE: &str = "select.inverse";
    
    // Tools
    pub const TOOL_DRAW: &str = "tool.draw";
    pub const TOOL_LINE: &str = "tool.line";
    pub const TOOL_RECTANGLE: &str = "tool.rectangle";
    pub const TOOL_ELLIPSE: &str = "tool.ellipse";
    pub const TOOL_FILL: &str = "tool.fill";
    pub const TOOL_TEXT: &str = "tool.text";
    pub const TOOL_ERASE: &str = "tool.erase";
    pub const TOOL_PICKUP: &str = "tool.pickup";
    pub const TOOL_SELECT: &str = "tool.select";
    
    // Layers
    pub const LAYER_NEW: &str = "layer.new";
    pub const LAYER_DELETE: &str = "layer.delete";
    pub const LAYER_MERGE_DOWN: &str = "layer.merge_down";
    pub const LAYER_FLATTEN: &str = "layer.flatten";
    
    // View
    pub const VIEW_GRID: &str = "view.grid";
    pub const VIEW_RULERS: &str = "view.rulers";
    pub const VIEW_GUIDES: &str = "view.guides";
}
