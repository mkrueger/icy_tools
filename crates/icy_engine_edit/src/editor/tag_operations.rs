#![allow(clippy::missing_errors_doc)]
use crate::{EngineResult, Position, Tag};

use super::{EditState, undo_operations};

impl EditState {
    pub fn add_new_tag(&mut self, new_tag: Tag) -> EngineResult<()> {
        let op = undo_operations::AddTag::new(false, new_tag);
        self.push_undo_action(Box::new(op))?;
        self.current_tag = self.buffer.tags.len() - 1;
        Ok(())
    }

    pub fn update_tag(&mut self, new_tag: Tag, index: usize) -> EngineResult<()> {
        let old_tag = self.buffer.tags[index].clone();
        let op = undo_operations::EditTag::new(index, old_tag, new_tag);
        self.push_undo_action(Box::new(op))?;
        Ok(())
    }

    pub fn show_tags(&mut self, show_tags: bool) -> EngineResult<()> {
        let op = undo_operations::ShowTags::new(show_tags);
        self.push_undo_action(Box::new(op))?;
        Ok(())
    }

    pub fn move_tag(&mut self, tag: usize, pos: Position) -> EngineResult<()> {
        let old_pos = self.buffer.tags[tag].position;
        let op = undo_operations::MoveTag::new(tag, old_pos, pos);
        self.push_undo_action(Box::new(op))?;
        Ok(())
    }

    pub fn remove_tag(&mut self, tag: usize) -> EngineResult<()> {
        let op = undo_operations::RemoveTag::new(tag, self.buffer.tags[tag].clone());
        self.push_undo_action(Box::new(op))?;
        Ok(())
    }

    pub fn clone_tag(&mut self, tag: usize) -> EngineResult<()> {
        let op = undo_operations::AddTag::new(true, self.buffer.tags[tag].clone());
        self.push_undo_action(Box::new(op))?;
        self.current_tag = self.buffer.tags.len() - 1;
        Ok(())
    }
}
