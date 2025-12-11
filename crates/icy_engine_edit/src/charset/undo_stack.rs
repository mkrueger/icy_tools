//! Undo stack for CharSet (TDF) editor

use super::CharSetUndoOperation;

/// Undo stack for the CharSet editor
#[derive(Debug, Default)]
pub struct CharSetUndoStack {
    /// Undo operations
    undo_stack: Vec<CharSetUndoOperation>,
    /// Redo operations
    redo_stack: Vec<CharSetUndoOperation>,
    /// Whether currently in an atomic operation
    in_atomic: bool,
}

impl CharSetUndoStack {
    /// Create a new empty undo stack
    pub fn new() -> Self {
        Self::default()
    }

    /// Push an operation onto the undo stack
    pub fn push(&mut self, op: CharSetUndoOperation) {
        self.undo_stack.push(op);
        // Clear redo stack when a new operation is pushed
        if !self.in_atomic {
            self.redo_stack.clear();
        }
    }

    /// Pop an operation from the undo stack
    pub fn pop_undo(&mut self) -> Option<CharSetUndoOperation> {
        self.undo_stack.pop()
    }

    /// Push an operation onto the redo stack
    pub fn push_redo(&mut self, op: CharSetUndoOperation) {
        self.redo_stack.push(op);
    }

    /// Pop an operation from the redo stack
    pub fn pop_redo(&mut self) -> Option<CharSetUndoOperation> {
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
        self.undo_stack.last().map(|op| op.description())
    }

    /// Get description of next redo operation
    pub fn redo_description(&self) -> Option<String> {
        self.redo_stack.last().map(|op| op.description())
    }

    /// Clear both stacks
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.in_atomic = false;
    }

    /// Begin an atomic operation group
    pub fn begin_atomic(&mut self) {
        if !self.in_atomic {
            self.in_atomic = true;
            self.push(CharSetUndoOperation::AtomicStart);
        }
    }

    /// End an atomic operation group
    pub fn end_atomic(&mut self) {
        if self.in_atomic {
            self.push(CharSetUndoOperation::AtomicEnd);
            self.in_atomic = false;
            self.redo_stack.clear();
        }
    }

    /// Check if currently in an atomic operation
    pub fn is_in_atomic(&self) -> bool {
        self.in_atomic
    }
}
