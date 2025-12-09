//! Undo stack for BitFont editing
//!
//! Provides traits and types for undo/redo operations on bitmap fonts.

use crate::Result;

use super::BitFontEditState;

/// Trait for types that support undo/redo operations
pub trait BitFontUndoState {
    /// Get description of the next undo operation
    fn undo_description(&self) -> Option<String>;

    /// Check if undo is available
    fn can_undo(&self) -> bool;

    /// Perform undo operation
    fn undo(&mut self) -> Result<()>;

    /// Get description of the next redo operation
    fn redo_description(&self) -> Option<String>;

    /// Check if redo is available
    fn can_redo(&self) -> bool;

    /// Perform redo operation
    fn redo(&mut self) -> Result<()>;
}

/// Type of operation for grouping related undos
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BitFontOperationType {
    /// Unknown/default operation
    Unknown,
    /// Pixel editing (drawing)
    EditPixels,
    /// Glyph transformation (move, flip, etc.)
    Transform,
    /// Font resize
    Resize,
}

/// Trait for individual undo operations
pub trait BitFontUndoOperation: Send + Sync {
    /// Get a description of this operation for display
    fn get_description(&self) -> String;

    /// Undo this operation
    fn undo(&mut self, state: &mut BitFontEditState) -> Result<()>;

    /// Redo this operation
    fn redo(&mut self, state: &mut BitFontEditState) -> Result<()>;

    /// Get the operation type for grouping
    fn get_operation_type(&self) -> BitFontOperationType {
        BitFontOperationType::Unknown
    }

    /// Whether this operation changes data (affects dirty flag)
    fn changes_data(&self) -> bool {
        true
    }
}
