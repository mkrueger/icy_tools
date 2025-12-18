//! Session state for the Ansi Editor
//!
//! Contains all data needed to restore an editor session, including:
//! - Undo/redo stack
//! - Caret position
//! - Scroll offset
//! - Current attribute/colors
//! - Selected tool
//! - Zoom level
//! - Layer visibility
//! - Selection state

use serde::{Deserialize, Serialize};

use icy_engine::{Position, TextAttribute};

use super::{EditorUndoStack, undo_operation::SauceMetaDataSerde};

/// Session state for the Ansi editor
///
/// This struct contains everything needed to fully restore an editing session.
/// It is serialized to disk when the app exits and restored on startup.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnsiEditorSessionState {
    /// Version for future compatibility
    #[serde(default = "default_version")]
    pub version: u32,

    /// The undo/redo stack
    pub undo_stack: EditorUndoStack,

    /// Caret position
    pub caret_position: Position,

    /// Caret attribute (current drawing colors)
    pub caret_attribute: TextAttribute,

    /// Viewport scroll offset (x, y) in pixels
    #[serde(default)]
    pub scroll_offset: (f32, f32),

    /// Zoom level (1.0 = 100%)
    #[serde(default = "default_zoom")]
    pub zoom_level: f32,

    /// Whether zoom is in auto mode
    #[serde(default = "default_true")]
    pub auto_zoom: bool,

    /// Currently selected tool ID
    #[serde(default)]
    pub selected_tool: String,

    /// Current outline style index
    #[serde(default)]
    pub outline_style: usize,

    /// Mirror mode enabled
    #[serde(default)]
    pub mirror_mode: bool,

    /// Current tag index
    #[serde(default)]
    pub current_tag: usize,

    /// Layer visibility (layer index -> visible)
    #[serde(default)]
    pub layer_visibility: Vec<bool>,

    /// Currently selected layer index
    #[serde(default)]
    pub selected_layer: usize,

    /// SAUCE metadata
    #[serde(default)]
    pub sauce_meta: SauceMetaDataSerde,

    /// Reference image path (if any)
    #[serde(default)]
    pub reference_image_path: Option<String>,

    /// Reference image opacity
    #[serde(default = "default_reference_opacity")]
    pub reference_image_opacity: f32,
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
fn default_reference_opacity() -> f32 {
    0.5
}

impl Default for AnsiEditorSessionState {
    fn default() -> Self {
        Self {
            version: 1,
            undo_stack: EditorUndoStack::default(),
            caret_position: Position::default(),
            caret_attribute: TextAttribute::default(),
            scroll_offset: (0.0, 0.0),
            zoom_level: 1.0,
            auto_zoom: true,
            selected_tool: String::new(),
            outline_style: 0,
            mirror_mode: false,
            current_tag: 0,
            layer_visibility: Vec::new(),
            selected_layer: 0,
            sauce_meta: SauceMetaDataSerde::default(),
            reference_image_path: None,
            reference_image_opacity: 0.5,
        }
    }
}

impl AnsiEditorSessionState {
    /// Create a new empty session state
    pub fn new() -> Self {
        Self::default()
    }
}
