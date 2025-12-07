use crate::Result;

use super::EditState;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
