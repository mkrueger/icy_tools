//! Tool session state for the ANSI editor
//!
//! Captures per-tool settings (brush mode, paint char, color filters, brush
//! size, fill mode, selection mode, font slot, shape variant, ...) so they
//! can be persisted alongside the rest of the editor session and restored
//! when the editor is reopened.
//!
//! The struct is serialized to a bitcode blob and stored in
//! `AnsiEditorSessionState::tool_state_blob`.

use icy_engine_edit::tools::Tool;
use serde::{Deserialize, Serialize};

use super::tools::BrushSettings;
use super::widget::toolbar::top::{BrushPrimaryMode, SelectionMode};

/// Brush-style tool settings shared by Pencil, Shape and Fill tools.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct BrushSessionState {
    pub primary: BrushPrimaryMode,
    pub paint_char: char,
    pub brush_size: u32,
    pub colorize_fg: bool,
    pub colorize_bg: bool,
    /// Fill tool: only fill cells whose character/colors exactly match
    /// (ignored by Pencil/Shape).
    #[serde(default)]
    pub exact: bool,
}

impl Default for BrushSessionState {
    fn default() -> Self {
        let b = BrushSettings::default();
        Self::from(b)
    }
}

impl From<BrushSettings> for BrushSessionState {
    fn from(b: BrushSettings) -> Self {
        Self {
            primary: b.primary,
            paint_char: b.paint_char,
            brush_size: b.brush_size,
            colorize_fg: b.colorize_fg,
            colorize_bg: b.colorize_bg,
            exact: b.exact,
        }
    }
}

impl From<BrushSessionState> for BrushSettings {
    fn from(s: BrushSessionState) -> Self {
        Self {
            primary: s.primary,
            paint_char: s.paint_char,
            brush_size: s.brush_size.max(1),
            colorize_fg: s.colorize_fg,
            colorize_bg: s.colorize_bg,
            exact: s.exact,
        }
    }
}

/// Shape tool extends brush with the chosen shape variant.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct ShapeSessionState {
    /// Currently selected shape (Line, RectangleOutline, RectangleFilled,
    /// EllipseOutline, EllipseFilled).
    #[serde(default = "default_shape_tool")]
    pub shape: Tool,
}

fn default_shape_tool() -> Tool {
    Tool::RectangleOutline
}

/// Aggregate tool session state serialized into the editor session blob.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AnsiToolSessionState {
    /// Currently active tool.
    #[serde(default)]
    pub selected_tool: Tool,

    /// Single shared brush state covering Pencil/Shape/Fill.
    #[serde(default)]
    pub brush: BrushSessionState,

    #[serde(default)]
    pub shape: ShapeSessionState,

    #[serde(default)]
    pub selection_mode: SelectionMode,

    #[serde(default)]
    pub font_slot: usize,
}

impl AnsiToolSessionState {
    /// Encode to a bitcode blob suitable for `AnsiEditorSessionState::tool_state_blob`.
    pub fn encode(&self) -> Vec<u8> {
        bitcode::serialize(self).unwrap_or_default()
    }

    /// Decode from a bitcode blob. Returns `None` if the blob is empty or invalid.
    pub fn decode(blob: &[u8]) -> Option<Self> {
        if blob.is_empty() {
            return None;
        }
        bitcode::deserialize(blob).ok()
    }
}
