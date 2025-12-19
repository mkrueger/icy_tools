//! Undo/Redo system for BitFont editor
//!
//! All modifications go through the undo system:
//! - Single operations push one item to the undo stack
//! - Multi-character operations use `begin_atomic_undo()`/`end()` to group operations
//! - Atomic groups are undone/redone as a single unit

use crate::Result;
use crate::bitfont::{BitFontOperationType, BitFontUndoOp, BitFontUndoState};

use super::{BitFontAtomicUndoGuard, BitFontEditState};

impl BitFontEditState {
    /// Begin an atomic undo group
    ///
    /// All operations pushed while the guard is active will be undone/redone together.
    /// Call `end_atomic_undo(base_count, description, op_type)` to close the group.
    #[must_use]
    pub fn begin_atomic_undo(&mut self, description: impl Into<String>) -> BitFontAtomicUndoGuard {
        self.begin_typed_atomic_undo(description, BitFontOperationType::Unknown)
    }

    /// Begin a typed atomic undo group
    ///
    /// Same as `begin_atomic_undo` but with an operation type for categorization.
    #[must_use]
    pub fn begin_typed_atomic_undo(&mut self, description: impl Into<String>, operation_type: BitFontOperationType) -> BitFontAtomicUndoGuard {
        let base_count = self.undo_stack.undo_len();
        BitFontAtomicUndoGuard::new(description.into(), base_count, operation_type)
    }

    /// Push an undo operation and execute it (redo)
    pub(crate) fn push_undo_action(&mut self, op: BitFontUndoOp) -> Result<()> {
        op.redo(self)?;
        self.push_plain_undo(op)
    }

    /// Push an undo operation without executing it
    pub(crate) fn push_plain_undo(&mut self, op: BitFontUndoOp) -> Result<()> {
        if op.changes_data() {
            self.is_dirty = true;
        }
        self.undo_stack.push(op);
        Ok(())
    }

    /// End an atomic undo group
    pub fn end_atomic_undo(&mut self, base_count: usize, description: String, operation_type: BitFontOperationType) {
        if base_count >= self.undo_stack.undo_len() {
            return;
        }
        let operations = self.undo_stack.drain_from(base_count);
        self.undo_stack.create_atomic(description, operations, operation_type);
    }

    /// Get undo stack length
    pub fn undo_stack_len(&self) -> usize {
        self.undo_stack.undo_len()
    }

    /// Get redo stack length
    pub fn redo_stack_len(&self) -> usize {
        self.undo_stack.redo_len()
    }

    /// Mark as saved (clears dirty flag and marks save point in undo stack)
    pub fn mark_saved(&mut self) {
        self.is_dirty = false;
        self.undo_stack.mark_saved();
    }

    /// Get access to the undo stack for serialization
    pub fn undo_stack(&self) -> &crate::bitfont::undo_stack::BitFontUndoStack {
        &self.undo_stack
    }

    /// Get mutable access to the undo stack
    pub fn undo_stack_mut(&mut self) -> &mut crate::bitfont::undo_stack::BitFontUndoStack {
        &mut self.undo_stack
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// BitFontUndoState Trait Implementation
// ═══════════════════════════════════════════════════════════════════════════

impl BitFontUndoState for BitFontEditState {
    fn undo_description(&self) -> Option<String> {
        self.undo_stack.undo_description()
    }

    fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }

    fn undo(&mut self) -> Result<()> {
        let Some(op) = self.undo_stack.pop_undo() else {
            return Ok(());
        };

        if op.changes_data() {
            self.is_dirty = true;
        }

        let result = op.undo(self);
        self.undo_stack.push_redo(op);
        result
    }

    fn redo_description(&self) -> Option<String> {
        self.undo_stack.redo_description()
    }

    fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }

    fn redo(&mut self) -> Result<()> {
        let Some(op) = self.undo_stack.pop_redo() else {
            return Ok(());
        };

        if op.changes_data() {
            self.is_dirty = true;
        }

        let result = op.redo(self);
        self.undo_stack.push(op);
        result
    }
}
