//! BitFont atomic undo guard

use std::sync::{Arc, Mutex};

use crate::bitfont::{AtomicUndo, BitFontOperationType, BitFontUndoOperation};

/// Guard for grouping multiple operations into a single undo step
pub struct BitFontAtomicUndoGuard {
    pub(super) base_count: usize,
    pub(super) description: String,
    pub(super) operation_type: BitFontOperationType,
    pub(super) undo_stack: Arc<Mutex<Vec<Box<dyn BitFontUndoOperation>>>>,
}

impl BitFontAtomicUndoGuard {
    pub fn new(description: String, undo_stack: Arc<Mutex<Vec<Box<dyn BitFontUndoOperation>>>>, operation_type: BitFontOperationType) -> Self {
        let base_count = undo_stack.lock().unwrap().len();
        Self {
            base_count,
            description,
            operation_type,
            undo_stack,
        }
    }

    /// End the atomic undo group explicitly
    pub fn end(&mut self) {
        self.end_action();
    }

    fn end_action(&mut self) {
        let count = self.undo_stack.lock().unwrap().len();
        if self.base_count >= count {
            return;
        }

        let stack: Vec<Box<dyn BitFontUndoOperation>> = self.undo_stack.lock().unwrap().drain(self.base_count..).collect();
        let stack = Arc::new(Mutex::new(stack));

        self.undo_stack
            .lock()
            .unwrap()
            .push(Box::new(AtomicUndo::new(self.description.clone(), stack, self.operation_type)));
        self.base_count = usize::MAX;
    }
}

impl Drop for BitFontAtomicUndoGuard {
    fn drop(&mut self) {
        let count = self.undo_stack.lock().unwrap().len();
        if self.base_count >= count {
            return;
        }
        self.end_action();
    }
}
