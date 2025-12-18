#![allow(clippy::missing_errors_doc)]
use i18n_embed_fl::fl;

use super::{EditState, undo_operation::EditorUndoOp};
use crate::{AddType, AttributedChar, Position, Rectangle, Result, Selection, TextPane};

impl EditState {
    pub fn selection(&self) -> Option<Selection> {
        self.selection_opt
    }

    pub fn set_selection(&mut self, sel: impl Into<Selection>) -> Result<()> {
        let sel = sel.into();
        let selection = Some(sel);
        if self.selection_opt == selection {
            Ok(())
        } else {
            self.push_undo_action(EditorUndoOp::SetSelection {
                old: self.selection_opt,
                new: selection,
            })
        }
    }

    pub fn clear_selection(&mut self) -> Result<()> {
        if self.is_something_selected() {
            let sel = self.selection_opt.take();
            let mask = self.selection_mask.clone();
            self.push_undo_action(EditorUndoOp::SelectNothing { sel, mask })
        } else {
            Ok(())
        }
    }

    pub fn clear_selection_mask(&mut self) -> Result<()> {
        if self.selection_mask.is_empty() {
            return Ok(());
        }

        let old = self.selection_mask.clone();
        let mut new = old.clone();
        new.clear();

        self.push_undo_action(EditorUndoOp::SetSelectionMask {
            description: fl!(crate::LANGUAGE_LOADER, "undo-select-nothing"),
            old,
            new,
        })
    }

    pub fn deselect(&mut self) -> Result<()> {
        if let Some(sel) = self.selection_opt.take() {
            self.push_undo_action(EditorUndoOp::Deselect { sel })
        } else {
            Ok(())
        }
    }

    pub fn is_something_selected(&self) -> bool {
        self.selection_opt.is_some() || !self.selection_mask.is_empty()
    }

    pub fn is_selected(&self, pos: impl Into<Position>) -> bool {
        let pos = pos.into();
        if let Some(sel) = self.selection_opt {
            if sel.is_inside(pos) {
                return !matches!(sel.add_type, AddType::Subtract);
            }
        }

        self.selection_mask.is_selected(pos)
    }

    pub fn get_is_mask_selected(&self, pos: impl Into<Position>) -> bool {
        let pos = pos.into();

        self.selection_mask.is_selected(pos)
    }

    pub fn add_selection_to_mask(&mut self) -> Result<()> {
        if let Some(selection) = self.selection_opt {
            self.push_undo_action(EditorUndoOp::AddSelectionToMask {
                old: self.selection_mask.clone(),
                selection,
            })
        } else {
            Ok(())
        }
    }

    pub fn selected_rectangle(&self) -> Rectangle {
        self.selection_mask.selected_rectangle(&self.selection_opt)
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
        let old_mask = self.selection_mask.clone();
        let old_selection = self.selection_opt;
        if let Some(selection) = self.selection_opt {
            match selection.add_type {
                AddType::Default | AddType::Add => {
                    self.selection_mask.add_rectangle(selection.as_rectangle());
                }
                AddType::Subtract => {
                    self.selection_mask.remove_rectangle(selection.as_rectangle());
                }
            }
        }
        self.selection_opt = None;
        for y in 0..self.screen.buffer.height() {
            for x in 0..self.screen.buffer.width() {
                let pos = Position::new(x, y);
                let is_selected = self.is_selected(pos);
                self.selection_mask.set_is_selected(pos, !is_selected);
            }
        }
        let op = EditorUndoOp::InverseSelection {
            sel: old_selection,
            old: old_mask,
            new: self.selection_mask.clone(),
        };
        self.mark_dirty();
        self.push_plain_undo(op)
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
            cur_layer.offset()
        } else {
            log::error!("No current layer");
            return;
        };

        let old_mask = self.selection_mask.clone();
        for y in 0..self.screen.buffer.height() {
            for x in 0..self.screen.buffer.width() {
                let pos = Position::new(x, y);
                let is_selected = self.is_selected(pos);
                let ch = self.get_cur_layer().unwrap().char_at(pos - offset);
                if let Some(res) = f(pos, ch, is_selected) {
                    self.selection_mask.set_is_selected(pos, res);
                }
            }
        }

        if old_mask != self.selection_mask {
            let op = EditorUndoOp::SetSelectionMask {
                description: fl!(crate::LANGUAGE_LOADER, "undo-set_selection"),
                old: old_mask,
                new: self.selection_mask.clone(),
            };
            let _ = self.push_plain_undo(op);
        }

        self.mark_dirty();
    }
}
