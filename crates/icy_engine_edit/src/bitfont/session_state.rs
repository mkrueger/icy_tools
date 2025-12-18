//! Session state for the BitFont Editor
//!
//! Contains all data needed to restore an editing session, including:
//! - Undo/redo stack
//! - Selected glyph
//! - Cursor position in glyph editor
//! - Zoom level
//! - View mode settings

use serde::{Deserialize, Serialize};

use super::undo_stack::BitFontUndoStack;

/// Session state for the BitFont editor
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BitFontSessionState {
    /// Version for future compatibility
    #[serde(default = "default_version")]
    pub version: u32,

    /// The undo/redo stack
    pub undo_stack: BitFontUndoStack,

    /// Currently selected glyph index (0-255 typically)
    #[serde(default)]
    pub selected_glyph: usize,

    /// Cursor position within the glyph editor (x, y)
    #[serde(default)]
    pub cursor_position: (i32, i32),

    /// Zoom level for the glyph editor
    #[serde(default = "default_zoom")]
    pub edit_zoom: f32,

    /// Zoom level for the glyph selector
    #[serde(default = "default_zoom")]
    pub selector_zoom: f32,

    /// Which panel is focused (editor, selector, preview)
    #[serde(default)]
    pub focused_panel: BitFontFocusedPanelState,

    /// Show grid lines in editor
    #[serde(default = "default_true")]
    pub show_grid: bool,

    /// Currently active tool
    #[serde(default)]
    pub selected_tool: String,
}

fn default_version() -> u32 {
    1
}
fn default_zoom() -> f32 {
    1.0
}
fn default_true() -> bool {
    true
}

/// Which panel is focused in the BitFont editor
/// Matches BitFontFocusedPanel but is serializable
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum BitFontFocusedPanelState {
    /// Edit grid has focus - operations work on pixel selection
    #[default]
    EditGrid,
    /// Character set has focus - operations work on selected characters
    CharSet,
}

impl Default for BitFontSessionState {
    fn default() -> Self {
        Self {
            version: 1,
            undo_stack: BitFontUndoStack::default(),
            selected_glyph: 0,
            cursor_position: (0, 0),
            edit_zoom: 1.0,
            selector_zoom: 1.0,
            focused_panel: BitFontFocusedPanelState::EditGrid,
            show_grid: true,
            selected_tool: String::new(),
        }
    }
}

impl BitFontSessionState {
    /// Create a new empty session state
    pub fn new() -> Self {
        Self::default()
    }
}
