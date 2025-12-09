//! Undo/Redo system for BitFont editor
//!
//! All modifications go through the undo system:
//! - Single operations push one item to the undo stack
//! - Multi-character operations use `begin_atomic_undo()`/`end()` to group operations
//! - Atomic groups are undone/redone as a single unit

use crate::Result;
use crate::bitfont::{BitFontOperationType, BitFontUndoOperation, BitFontUndoState};

use super::{BitFontAtomicUndoGuard, BitFontEditState};

impl BitFontEditState {
    /// Begin an atomic undo group
    ///
    /// All operations pushed while the guard is active will be undone/redone together.
    /// Call `guard.end()` to close the group.
    #[must_use]
    pub fn begin_atomic_undo(&mut self, description: impl Into<String>) -> BitFontAtomicUndoGuard {
        self.begin_typed_atomic_undo(description, BitFontOperationType::Unknown)
    }

    /// Begin a typed atomic undo group
    ///
    /// Same as `begin_atomic_undo` but with an operation type for categorization.
    #[must_use]
    pub fn begin_typed_atomic_undo(&mut self, description: impl Into<String>, operation_type: BitFontOperationType) -> BitFontAtomicUndoGuard {
        self.redo_stack.clear();
        BitFontAtomicUndoGuard::new(description.into(), self.undo_stack.clone(), operation_type)
    }

    /// Push an undo operation and execute it (redo)
    pub(crate) fn push_undo_action(&mut self, mut op: Box<dyn BitFontUndoOperation>) -> Result<()> {
        op.redo(self)?;
        self.push_plain_undo(op)
    }

    /// Push an undo operation without executing it
    pub(crate) fn push_plain_undo(&mut self, op: Box<dyn BitFontUndoOperation>) -> Result<()> {
        if op.changes_data() {
            self.is_dirty = true;
        }
        self.undo_stack.lock().unwrap().push(op);
        self.redo_stack.clear();
        Ok(())
    }

    /// Get undo stack length
    pub fn undo_stack_len(&self) -> usize {
        self.undo_stack.lock().unwrap().len()
    }

    /// Get redo stack length
    pub fn redo_stack_len(&self) -> usize {
        self.redo_stack.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// BitFontUndoState Trait Implementation
// ═══════════════════════════════════════════════════════════════════════════

impl BitFontUndoState for BitFontEditState {
    fn undo_description(&self) -> Option<String> {
        self.undo_stack.lock().unwrap().last().map(|op| op.get_description())
    }

    fn can_undo(&self) -> bool {
        !self.undo_stack.lock().unwrap().is_empty()
    }

    fn undo(&mut self) -> Result<()> {
        let Some(mut op) = self.undo_stack.lock().unwrap().pop() else {
            return Ok(());
        };

        if op.changes_data() {
            self.is_dirty = true;
        }

        let result = op.undo(self);
        self.redo_stack.push(op);
        result
    }

    fn redo_description(&self) -> Option<String> {
        self.redo_stack.last().map(|op| op.get_description())
    }

    fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    fn redo(&mut self) -> Result<()> {
        let Some(mut op) = self.redo_stack.pop() else {
            return Ok(());
        };

        if op.changes_data() {
            self.is_dirty = true;
        }

        let result = op.redo(self);
        self.undo_stack.lock().unwrap().push(op);
        result
    }
}
