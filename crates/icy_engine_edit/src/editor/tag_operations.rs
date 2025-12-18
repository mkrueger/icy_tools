#![allow(clippy::missing_errors_doc)]
use crate::{Position, Result, Tag};

use super::{EditState, undo_operation::EditorUndoOp};

impl EditState {
    pub fn add_new_tag(&mut self, new_tag: Tag) -> Result<()> {
        let op = EditorUndoOp::AddTag { new_tag, clone: false };
        self.push_undo_action(op)?;
        self.current_tag = self.screen.buffer.tags.len() - 1;
        Ok(())
    }

    pub fn update_tag(&mut self, new_tag: Tag, index: usize) -> Result<()> {
        let old_tag = self.screen.buffer.tags[index].clone();
        let op = EditorUndoOp::EditTag {
            tag_index: index,
            old_tag,
            new_tag,
        };
        self.push_undo_action(op)?;
        Ok(())
    }

    pub fn show_tags(&mut self, show_tags: bool) -> Result<()> {
        let op = EditorUndoOp::ShowTags { show: show_tags };
        self.push_undo_action(op)?;
        Ok(())
    }

    pub fn move_tag(&mut self, tag: usize, pos: Position) -> Result<()> {
        let old_pos = self.screen.buffer.tags[tag].position;
        let op = EditorUndoOp::MoveTag { tag, new_pos: pos, old_pos };
        self.push_undo_action(op)?;
        Ok(())
    }

    pub fn remove_tag(&mut self, tag: usize) -> Result<()> {
        let op = EditorUndoOp::RemoveTag {
            tag_index: tag,
            tag: self.screen.buffer.tags[tag].clone(),
        };
        self.push_undo_action(op)?;
        Ok(())
    }

    pub fn clone_tag(&mut self, tag: usize) -> Result<()> {
        let op = EditorUndoOp::AddTag {
            new_tag: self.screen.buffer.tags[tag].clone(),
            clone: true,
        };
        self.push_undo_action(op)?;
        self.current_tag = self.screen.buffer.tags.len() - 1;
        Ok(())
    }
}
