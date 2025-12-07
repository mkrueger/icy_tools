#![allow(clippy::missing_errors_doc)]
use i18n_embed_fl::fl;

use super::{EditState, undo_operations};
use crate::{AddType, AttributedChar, Result, Position, Rectangle, Selection, TextPane};

impl EditState {
    pub fn get_selection(&self) -> Option<Selection> {
        self.screen.selection_opt
    }

    pub fn set_selection(&mut self, sel: impl Into<Selection>) -> Result<()> {
        let sel = sel.into();
        let selection = Some(sel);
        if self.screen.selection_opt == selection {
            Ok(())
        } else {
            self.push_undo_action(Box::new(undo_operations::SetSelection::new(self.screen.selection_opt, selection)))
        }
    }

    pub fn clear_selection(&mut self) -> Result<()> {
        if self.is_something_selected() {
            let sel = self.screen.selection_opt.take();
            let mask = self.screen.selection_mask.clone();
            self.push_undo_action(Box::new(undo_operations::SelectNothing::new(sel, mask)))
        } else {
            Ok(())
        }
    }

    pub fn deselect(&mut self) -> Result<()> {
        if let Some(sel) = self.screen.selection_opt.take() {
            self.push_undo_action(Box::new(undo_operations::Deselect::new(sel)))
        } else {
            Ok(())
        }
    }

    pub fn is_something_selected(&self) -> bool {
        self.screen.selection_opt.is_some() || !self.screen.selection_mask.is_empty()
    }

    pub fn get_is_selected(&self, pos: impl Into<Position>) -> bool {
        let pos = pos.into();
        if let Some(sel) = self.screen.selection_opt {
            if sel.is_inside(pos) {
                return !matches!(sel.add_type, AddType::Subtract);
            }
        }

        self.screen.selection_mask.get_is_selected(pos)
    }

    pub fn get_is_mask_selected(&self, pos: impl Into<Position>) -> bool {
        let pos = pos.into();

        self.screen.selection_mask.get_is_selected(pos)
    }

    pub fn add_selection_to_mask(&mut self) -> Result<()> {
        if let Some(selection) = self.screen.selection_opt {
            self.push_undo_action(Box::new(undo_operations::AddSelectionToMask::new(self.screen.selection_mask.clone(), selection)))
        } else {
            Ok(())
        }
    }

    pub fn get_selected_rectangle(&self) -> Rectangle {
        self.screen.selection_mask.get_selected_rectangle(&self.screen.selection_opt)
    }

    /// Returns the inverse selection of this [`EditState`].
    ///
    /// # Panics
    ///
    /// Panics if .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn inverse_selection(&mut self) -> Result<()> {
        let old_mask = self.screen.selection_mask.clone();
        let old_selection = self.screen.selection_opt;
        if let Some(selection) = self.screen.selection_opt {
            match selection.add_type {
                AddType::Default | AddType::Add => {
                    self.screen.selection_mask.add_rectangle(selection.as_rectangle());
                }
                AddType::Subtract => {
                    self.screen.selection_mask.remove_rectangle(selection.as_rectangle());
                }
            }
        }
        self.screen.selection_opt = None;
        for y in 0..self.screen.buffer.get_height() {
            for x in 0..self.screen.buffer.get_width() {
                let pos = Position::new(x, y);
                let is_selected = self.get_is_selected(pos);
                self.screen.selection_mask.set_is_selected(pos, !is_selected);
            }
        }
        let op = undo_operations::InverseSelection::new(old_selection, old_mask, self.screen.selection_mask.clone());
        self.set_is_buffer_dirty();
        self.push_plain_undo(Box::new(op))
    }

    /// .
    ///
    /// # Panics
    ///
    /// Panics if .
    pub fn enumerate_selections<F>(&mut self, f: F)
    where
        F: Fn(Position, AttributedChar, bool) -> Option<bool>,
    {
        let offset = if let Some(cur_layer) = self.get_cur_layer() {
            cur_layer.get_offset()
        } else {
            log::error!("No current layer");
            return;
        };

        let old_mask = self.screen.selection_mask.clone();
        for y in 0..self.screen.buffer.get_height() {
            for x in 0..self.screen.buffer.get_width() {
                let pos = Position::new(x, y);
                let is_selected = self.get_is_selected(pos);
                let ch = self.get_cur_layer().unwrap().get_char(pos - offset);
                if let Some(res) = f(pos, ch, is_selected) {
                    self.screen.selection_mask.set_is_selected(pos, res);
                }
            }
        }

        if old_mask != self.screen.selection_mask {
            let op = undo_operations::SetSelectionMask::new(fl!(crate::LANGUAGE_LOADER, "undo-set_selection"), old_mask, self.screen.selection_mask.clone());
            let _ = self.push_plain_undo(Box::new(op));
        }

        self.set_is_buffer_dirty();
    }
}
