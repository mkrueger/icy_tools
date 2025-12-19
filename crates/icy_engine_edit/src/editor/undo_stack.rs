use serde::{Deserialize, Serialize};

use crate::Result;

use super::EditState;
use super::undo_operation::EditorUndoOp;

pub trait UndoState {
    fn undo_description(&self) -> Option<String>;
    fn can_undo(&self) -> bool;
    /// .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    fn undo(&mut self) -> Result<()>;

    fn redo_description(&self) -> Option<String>;
    fn can_redo(&self) -> bool;
    /// .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    fn redo(&mut self) -> Result<()>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperationType {
    Unknown,
    RenderCharacter,
    ReversedRenderCharacter,
}

pub trait UndoOperation: Send + Sync {
    fn get_description(&self) -> String;

    /// .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    fn undo(&mut self, edit_state: &mut EditState) -> Result<()>;

    /// .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    fn redo(&mut self, edit_state: &mut EditState) -> Result<()>;

    fn get_operation_type(&self) -> OperationType {
        OperationType::Unknown
    }

    fn changes_data(&self) -> bool {
        true
    }

    fn try_clone(&self) -> Option<Box<dyn UndoOperation>> {
        None
    }
}

/// Serializable undo stack for editor operations
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EditorUndoStack {
    /// Undo operations
    undo_stack: Vec<EditorUndoOp>,
    /// Redo operations
    redo_stack: Vec<EditorUndoOp>,
    /// Index of last save (operations before this don't need to be serialized for session)
    #[serde(default)]
    last_save_index: usize,
}

impl EditorUndoStack {
    /// Create a new empty undo stack
    pub fn new() -> Self {
        Self::default()
    }

    /// Push an operation onto the undo stack (clears redo stack)
    pub fn push(&mut self, op: EditorUndoOp) {
        self.undo_stack.push(op);
        self.redo_stack.clear();
    }

    /// Push an operation onto the undo stack without clearing redo
    pub fn push_undo(&mut self, op: EditorUndoOp) {
        self.undo_stack.push(op);
    }

    /// Clear the redo stack
    pub fn clear_redo(&mut self) {
        self.redo_stack.clear();
    }

    /// Pop an operation from the undo stack
    pub fn pop_undo(&mut self) -> Option<EditorUndoOp> {
        self.undo_stack.pop()
    }

    /// Push an operation onto the redo stack
    pub fn push_redo(&mut self, op: EditorUndoOp) {
        self.redo_stack.push(op);
    }

    /// Pop an operation from the redo stack
    pub fn pop_redo(&mut self) -> Option<EditorUndoOp> {
        self.redo_stack.pop()
    }

    /// Get the number of undo operations
    pub fn undo_len(&self) -> usize {
        self.undo_stack.len()
    }

    /// Alias for undo_len for compatibility
    pub fn len(&self) -> usize {
        self.undo_stack.len()
    }

    /// Check if the stack is empty
    pub fn is_empty(&self) -> bool {
        self.undo_stack.is_empty()
    }

    /// Get reference to operation at index
    pub fn get(&self, index: usize) -> Option<&EditorUndoOp> {
        self.undo_stack.get(index)
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

    /// Get operation type of next undo operation
    pub fn get_last_operation_type(&self) -> OperationType {
        self.undo_stack.last().map(|op| op.get_operation_type()).unwrap_or(OperationType::Unknown)
    }

    /// Check if last operation changes data
    pub fn last_changes_data(&self) -> bool {
        self.undo_stack.last().map(|op| op.changes_data()).unwrap_or(false)
    }

    /// Mark the current state as saved
    pub fn mark_saved(&mut self) {
        self.last_save_index = self.undo_stack.len();
    }

    /// Check if the document has been modified since last save
    pub fn is_modified(&self) -> bool {
        self.undo_stack.len() != self.last_save_index
    }

    /// Get operations since last save (for session serialization)
    pub fn operations_since_save(&self) -> &[EditorUndoOp] {
        if self.last_save_index < self.undo_stack.len() {
            &self.undo_stack[self.last_save_index..]
        } else {
            &[]
        }
    }

    /// Get direct read access to the undo stack for collaboration sync
    pub fn undo_stack(&self) -> &[EditorUndoOp] {
        &self.undo_stack
    }

    /// Get direct read access to the redo stack for collaboration sync
    pub fn redo_stack(&self) -> &[EditorUndoOp] {
        &self.redo_stack
    }

    /// Clear all operations
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.last_save_index = 0;
    }
}

impl std::ops::Index<usize> for EditorUndoStack {
    type Output = EditorUndoOp;

    fn index(&self, index: usize) -> &Self::Output {
        &self.undo_stack[index]
    }
}
