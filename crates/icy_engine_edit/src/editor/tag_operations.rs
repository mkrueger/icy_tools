#![allow(clippy::missing_errors_doc)]
use crate::{Position, Result, Tag};

use super::{undo_operation::EditorUndoOp, EditState};

impl EditState {
    pub fn add_new_tag(&mut self, new_tag: Tag) -> Result<()> {
        let op = EditorUndoOp::AddTag { new_tag, clone: false };
        self.push_undo_action(op)?;
        self.current_tag = self.screen.buffer.tags.len().saturating_sub(1);
        Ok(())
    }

    pub fn update_tag(&mut self, new_tag: Tag, index: usize) -> Result<()> {
        let Some(old_tag) = self.screen.buffer.tags.get(index).cloned() else {
            log::warn!("update_tag: index {} out of bounds (len={})", index, self.screen.buffer.tags.len());
            return Ok(());
        };
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
        let Some(tag_data) = self.screen.buffer.tags.get(tag) else {
            log::warn!("move_tag: index {} out of bounds (len={})", tag, self.screen.buffer.tags.len());
            return Ok(());
        };
        let old_pos = tag_data.position;
        let op = EditorUndoOp::MoveTag { tag, new_pos: pos, old_pos };
        self.push_undo_action(op)?;
        Ok(())
    }

    pub fn remove_tag(&mut self, tag: usize) -> Result<()> {
        let Some(tag_data) = self.screen.buffer.tags.get(tag).cloned() else {
            log::warn!("remove_tag: index {} out of bounds (len={})", tag, self.screen.buffer.tags.len());
            return Ok(());
        };
        let op = EditorUndoOp::RemoveTag { tag_index: tag, tag: tag_data };
        self.push_undo_action(op)?;
        Ok(())
    }

    pub fn clone_tag(&mut self, tag: usize) -> Result<()> {
        let Some(tag_data) = self.screen.buffer.tags.get(tag).cloned() else {
            log::warn!("clone_tag: index {} out of bounds (len={})", tag, self.screen.buffer.tags.len());
            return Ok(());
        };
        let op = EditorUndoOp::AddTag {
            new_tag: tag_data,
            clone: true,
        };
        self.push_undo_action(op)?;
        self.current_tag = self.screen.buffer.tags.len().saturating_sub(1);
        Ok(())
    }
}
