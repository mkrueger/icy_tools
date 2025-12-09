//! Atomic undo operation - groups multiple operations into a single undo step

use std::sync::{Arc, Mutex};

use crate::Result;
use crate::bitfont::{BitFontEditState, BitFontOperationType, BitFontUndoOperation};

/// Groups multiple operations into a single undo step
pub struct AtomicUndo {
    description: String,
    operations: Arc<Mutex<Vec<Box<dyn BitFontUndoOperation>>>>,
    operation_type: BitFontOperationType,
}

impl AtomicUndo {
    pub fn new(description: String, operations: Arc<Mutex<Vec<Box<dyn BitFontUndoOperation>>>>, operation_type: BitFontOperationType) -> Self {
        Self {
            description,
            operations,
            operation_type,
        }
    }
}

impl BitFontUndoOperation for AtomicUndo {
    fn get_description(&self) -> String {
        self.description.clone()
    }

    fn undo(&mut self, state: &mut BitFontEditState) -> Result<()> {
        let mut ops = self.operations.lock().unwrap();
        for op in ops.iter_mut().rev() {
            op.undo(state)?;
        }
        Ok(())
    }

    fn redo(&mut self, state: &mut BitFontEditState) -> Result<()> {
        let mut ops = self.operations.lock().unwrap();
        for op in ops.iter_mut() {
            op.redo(state)?;
        }
        Ok(())
    }

    fn get_operation_type(&self) -> BitFontOperationType {
        self.operation_type
    }
}
