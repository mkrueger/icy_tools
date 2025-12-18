//! Undo stack for BitFont editing
//!
//! Provides the undo/redo stack with serialization support via serde.

use serde::{Deserialize, Serialize};

use crate::Result;

use super::undo_operation::{BitFontOperationType, BitFontUndoOp};

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

/// Serializable undo stack for BitFont editing
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BitFontUndoStack {
    /// Undo operations
    undo_stack: Vec<BitFontUndoOp>,
    /// Redo operations
    redo_stack: Vec<BitFontUndoOp>,
    /// Index of last save (operations before this don't need to be serialized for session)
    #[serde(default)]
    last_save_index: usize,
}

impl BitFontUndoStack {
    /// Create a new empty undo stack
    pub fn new() -> Self {
        Self::default()
    }

    /// Push an operation onto the undo stack
    pub fn push(&mut self, op: BitFontUndoOp) {
        self.undo_stack.push(op);
        self.redo_stack.clear();
    }

    /// Pop an operation from the undo stack
    pub fn pop_undo(&mut self) -> Option<BitFontUndoOp> {
        self.undo_stack.pop()
    }

    /// Push an operation onto the redo stack
    pub fn push_redo(&mut self, op: BitFontUndoOp) {
        self.redo_stack.push(op);
    }

    /// Pop an operation from the redo stack
    pub fn pop_redo(&mut self) -> Option<BitFontUndoOp> {
        self.redo_stack.pop()
    }

    /// Get the number of undo operations
    pub fn undo_len(&self) -> usize {
        self.undo_stack.len()
    }

    /// Get the number of redo operations
    pub fn redo_len(&self) -> usize {
        self.redo_stack.len()
    }

    /// Check if undo is available
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Check if redo is available
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Get description of next undo operation
    pub fn undo_description(&self) -> Option<String> {
        self.undo_stack.last().map(|op| op.get_description())
    }

    /// Get description of next redo operation
    pub fn redo_description(&self) -> Option<String> {
        self.redo_stack.last().map(|op| op.get_description())
    }

    /// Clear both stacks
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.last_save_index = 0;
    }

    /// Mark the current state as saved
    pub fn mark_saved(&mut self) {
        self.last_save_index = self.undo_stack.len();
    }

    /// Check if the document has unsaved changes
    pub fn has_unsaved_changes(&self) -> bool {
        self.undo_stack.len() != self.last_save_index
    }

    /// Get only the operations since last save (for session serialization)
    pub fn operations_since_save(&self) -> &[BitFontUndoOp] {
        if self.last_save_index < self.undo_stack.len() {
            &self.undo_stack[self.last_save_index..]
        } else {
            &[]
        }
    }

    /// Get all undo operations
    pub fn undo_operations(&self) -> &[BitFontUndoOp] {
        &self.undo_stack
    }

    /// Get all redo operations
    pub fn redo_operations(&self) -> &[BitFontUndoOp] {
        &self.redo_stack
    }

    /// Drain operations from base_count for atomic grouping
    pub fn drain_from(&mut self, base_count: usize) -> Vec<BitFontUndoOp> {
        self.undo_stack.drain(base_count..).collect()
    }

    /// Create an atomic operation from collected operations
    pub fn create_atomic(&mut self, description: String, operations: Vec<BitFontUndoOp>, operation_type: BitFontOperationType) {
        self.undo_stack.push(BitFontUndoOp::Atomic {
            description,
            operations,
            operation_type,
        });
    }
}
