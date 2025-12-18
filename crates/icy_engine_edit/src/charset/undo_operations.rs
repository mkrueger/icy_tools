//! Undo operations for CharSet (TDF) editor

use retrofont::{Glyph, tdf::TdfFont};
use serde::{Deserialize, Serialize};

/// Undo operation types for the CharSet editor
#[derive(Clone, Serialize, Deserialize)]
pub enum CharSetUndoOperation {
    /// Glyph was modified
    GlyphModified {
        /// Font index
        font_index: usize,
        /// Character code
        char_code: char,
        /// Old glyph data (None if didn't exist)
        old_glyph: Option<Glyph>,
        /// New glyph data (None if deleted)
        new_glyph: Option<Glyph>,
    },
    /// Font name was changed
    FontNameChanged {
        /// Font index
        font_index: usize,
        /// Old name
        old_name: String,
        /// New name
        new_name: String,
    },
    /// Font spacing was changed
    FontSpacingChanged {
        /// Font index
        font_index: usize,
        /// Old spacing
        old_spacing: i32,
        /// New spacing
        new_spacing: i32,
    },
    /// Font was added
    FontAdded {
        /// Font index where it was added
        font_index: usize,
        /// The font that was added
        font: TdfFont,
    },
    /// Font was removed
    FontRemoved {
        /// Font index where it was removed
        font_index: usize,
        /// The font that was removed
        font: TdfFont,
    },
    /// Atomic group start marker
    AtomicStart,
    /// Atomic group end marker
    AtomicEnd,
}

impl std::fmt::Debug for CharSetUndoOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GlyphModified { font_index, char_code, .. } => f
                .debug_struct("GlyphModified")
                .field("font_index", font_index)
                .field("char_code", char_code)
                .finish(),
            Self::FontNameChanged {
                font_index,
                old_name,
                new_name,
            } => f
                .debug_struct("FontNameChanged")
                .field("font_index", font_index)
                .field("old_name", old_name)
                .field("new_name", new_name)
                .finish(),
            Self::FontSpacingChanged {
                font_index,
                old_spacing,
                new_spacing,
            } => f
                .debug_struct("FontSpacingChanged")
                .field("font_index", font_index)
                .field("old_spacing", old_spacing)
                .field("new_spacing", new_spacing)
                .finish(),
            Self::FontAdded { font_index, font } => f
                .debug_struct("FontAdded")
                .field("font_index", font_index)
                .field("font_name", &font.name)
                .finish(),
            Self::FontRemoved { font_index, font } => f
                .debug_struct("FontRemoved")
                .field("font_index", font_index)
                .field("font_name", &font.name)
                .finish(),
            Self::AtomicStart => write!(f, "AtomicStart"),
            Self::AtomicEnd => write!(f, "AtomicEnd"),
        }
    }
}

impl CharSetUndoOperation {
    /// Get a description of this operation for UI display
    pub fn description(&self) -> String {
        match self {
            CharSetUndoOperation::GlyphModified { char_code, .. } => {
                format!("Modify glyph '{}'", char_code)
            }
            CharSetUndoOperation::FontNameChanged { new_name, .. } => {
                format!("Rename font to '{}'", new_name)
            }
            CharSetUndoOperation::FontSpacingChanged { new_spacing, .. } => {
                format!("Change spacing to {}", new_spacing)
            }
            CharSetUndoOperation::FontAdded { .. } => "Add font".to_string(),
            CharSetUndoOperation::FontRemoved { .. } => "Remove font".to_string(),
            CharSetUndoOperation::AtomicStart => "Begin group".to_string(),
            CharSetUndoOperation::AtomicEnd => "End group".to_string(),
        }
    }
}
