#![allow(clippy::missing_errors_doc)]
use i18n_embed_fl::fl;

use super::{undo_operation::EditorUndoOp, EditState};
use crate::{AddType, AttributedChar, CellMatchMode, CellMatcher, Position, Rectangle, Result, Selection, SelectionMask, TextPane};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SelectionOptions {
    /// Restrict matching to cells connected by an edge to the clicked cell.
    pub connected: bool,
    /// Sample the visible text composite instead of the current layer.
    pub sample_merged: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Layer, TextAttribute, TextBuffer, UndoState};

    fn state(rows: &[&str]) -> EditState {
        let mut buffer = TextBuffer::new((rows[0].len() as i32, rows.len() as i32));
        for (y, row) in rows.iter().enumerate() {
            for (x, ch) in row.chars().enumerate() {
                buffer.layers[0].set_char(Position::new(x as i32, y as i32), AttributedChar::from_char(ch));
            }
        }
        EditState::from_buffer(buffer)
    }

    fn selected(state: &EditState) -> Vec<(i32, i32)> {
        let mut positions = Vec::new();
        for y in 0..state.get_buffer().height() {
            for x in 0..state.get_buffer().width() {
                if state.is_selected(Position::new(x, y)) {
                    positions.push((x, y));
                }
            }
        }
        positions
    }

    #[test]
    fn matching_selection_connected_excludes_diagonals_and_disjoint_regions() {
        let mut state = state(&["AA..A", ".A.A.", "..A.."]);
        state
            .select_matching(
                Position::default(),
                CellMatchMode::Character,
                SelectionOptions {
                    connected: true,
                    sample_merged: false,
                },
                AddType::Default,
            )
            .unwrap();
        assert_eq!(selected(&state), [(0, 0), (1, 0), (1, 1)]);
        assert_eq!(state.get_undo_stack().lock().unwrap().undo_len(), 1);
        state.undo().unwrap();
        assert!(selected(&state).is_empty());
        state.redo().unwrap();
        assert_eq!(selected(&state), [(0, 0), (1, 0), (1, 1)]);
        state
            .select_matching(Position::default(), CellMatchMode::Character, SelectionOptions::default(), AddType::Default)
            .unwrap();
        assert_eq!(selected(&state), [(0, 0), (1, 0), (4, 0), (1, 1), (3, 1), (2, 2)]);
    }

    #[test]
    fn matching_selection_preserves_rectangles_and_masks_through_undo() {
        for (add_type, expected) in [
            (AddType::Default, vec![(0, 0), (3, 0)]),
            (AddType::Add, vec![(0, 0), (1, 0), (2, 0), (3, 0)]),
            (AddType::Subtract, vec![(1, 0), (2, 0)]),
        ] {
            let mut state = state(&["ABBA"]);
            let rect = Selection::from(Rectangle::from(0, 0, 3, 1));
            state.set_selection(rect).unwrap();
            let before = state.get_undo_stack().lock().unwrap().undo_len();
            state
                .select_matching(Position::default(), CellMatchMode::Character, SelectionOptions::default(), add_type)
                .unwrap();
            assert_eq!(selected(&state), expected);
            assert!(state.selection().is_none());
            assert_eq!(state.get_undo_stack().lock().unwrap().undo_len(), before + 1);
            state.undo().unwrap();
            assert_eq!(state.selection(), Some(rect));
            assert_eq!(selected(&state), [(0, 0), (1, 0), (2, 0)]);
            state.redo().unwrap();
            assert_eq!(selected(&state), expected);
            assert!(state.selection().is_none());
        }
    }

    #[test]
    fn matching_selection_samples_visible_composite_with_offsets_and_transparency() {
        let mut state = state(&["AAAAA"]);
        let mut upper = Layer::new("Upper", (3, 1));
        upper.set_offset((1, 0));
        upper.properties.has_alpha_channel = true;
        upper.set_char(Position::new(0, 0), AttributedChar::from_char('B'));
        upper.set_char(Position::new(1, 0), AttributedChar::invisible());
        upper.set_char(Position::new(2, 0), AttributedChar::from_char('B'));
        let mut hidden = Layer::new("Hidden", (5, 1));
        hidden.set_is_visible(false);
        for x in 0..5 {
            hidden.set_char(Position::new(x, 0), AttributedChar::from_char('Z'));
        }
        state.get_buffer_mut().layers.extend([upper, hidden]);
        state.set_current_layer(1);
        state
            .select_matching(Position::new(2, 0), CellMatchMode::Character, SelectionOptions::default(), AddType::Default)
            .unwrap();
        assert_eq!(selected(&state), [(0, 0), (2, 0), (4, 0)], "current-layer sampling sees transparent cells");
        state
            .select_matching(
                Position::new(0, 0),
                CellMatchMode::Character,
                SelectionOptions {
                    connected: false,
                    sample_merged: true,
                },
                AddType::Default,
            )
            .unwrap();
        assert_eq!(selected(&state), [(0, 0), (2, 0), (4, 0)]);
        state
            .select_matching(
                Position::new(0, 0),
                CellMatchMode::Character,
                SelectionOptions {
                    connected: true,
                    sample_merged: true,
                },
                AddType::Default,
            )
            .unwrap();
        assert_eq!(selected(&state), [(0, 0)], "upper-layer B separates the visible A regions");
        state
            .select_matching(Position::new(1, 0), CellMatchMode::Character, SelectionOptions::default(), AddType::Default)
            .unwrap();
        assert_eq!(selected(&state), [(1, 0), (3, 0)], "layer offsets use document coordinates");
    }

    #[test]
    fn matching_selection_appearance_uses_the_merged_colors() {
        let mut state = state(&["AB"]);
        for x in 0..2 {
            state.get_buffer_mut().layers[0].set_char(Position::new(x, 0), AttributedChar::new(' ', TextAttribute::new(7, 4)));
        }
        let mut upper = Layer::new("Upper", (2, 1));
        upper.properties.has_alpha_channel = true;
        let mut attr = TextAttribute::new(4, 0);
        attr.set_background_transparent();
        upper.set_char(Position::default(), AttributedChar::new('A', attr));
        upper.set_char(Position::new(1, 0), AttributedChar::invisible());
        state.get_buffer_mut().layers.push(upper);
        state.set_current_layer(1);
        state
            .select_matching(Position::default(), CellMatchMode::Appearance, SelectionOptions::default(), AddType::Default)
            .unwrap();
        assert_eq!(selected(&state), [(0, 0)]);
        state
            .select_matching(
                Position::default(),
                CellMatchMode::Appearance,
                SelectionOptions {
                    connected: true,
                    sample_merged: true,
                },
                AddType::Default,
            )
            .unwrap();
        assert_eq!(selected(&state), [(0, 0), (1, 0)], "inherited red makes both cells solid red");
    }

    #[test]
    fn matching_selection_rejects_outside_samples_without_changing_selection_or_undo() {
        let mut state = state(&["AAA"]);
        state.set_selection(Selection::from(Rectangle::from(1, 0, 1, 1))).unwrap();
        let before = state.get_undo_stack().lock().unwrap().undo_len();
        for position in [Position::new(-1, 0), Position::new(3, 0), Position::new(0, 1)] {
            assert!(state
                .select_matching(position, CellMatchMode::Character, SelectionOptions::default(), AddType::Default)
                .is_err());
        }
        assert_eq!(selected(&state), [(1, 0)]);
        assert_eq!(state.get_undo_stack().lock().unwrap().undo_len(), before);
    }
}

impl EditState {
    pub fn select_matching(&mut self, position: Position, mode: CellMatchMode, options: SelectionOptions, add_type: AddType) -> Result<()> {
        let buffer = self.get_buffer();
        if position.x < 0 || position.y < 0 || position.x >= buffer.width() || position.y >= buffer.height() {
            return Err(icy_engine::EngineError::Generic("Selection sample is outside the canvas".into()));
        }
        let layer = self
            .get_cur_layer()
            .ok_or_else(|| icy_engine::EngineError::Generic("Current layer is invalid".into()))?;
        let sample = |pos| {
            if options.sample_merged {
                buffer.char_at(pos)
            } else if mode == CellMatchMode::Appearance {
                layer.display_char_at(pos - layer.offset())
            } else {
                layer.char_at(pos - layer.offset())
            }
        };
        let matcher = CellMatcher::new(buffer, sample(position), mode);
        let mut matched = SelectionMask::default();
        matched.set_size(buffer.size());
        if options.connected {
            let mut visited = SelectionMask::default();
            visited.set_size(buffer.size());
            visited.set_is_selected(position, true);
            let mut pending = vec![position];
            while let Some(pos) = pending.pop() {
                if !matcher.matches(sample(pos)) {
                    continue;
                }
                matched.set_is_selected(pos, true);
                for next in [
                    Position::new(pos.x - 1, pos.y),
                    Position::new(pos.x + 1, pos.y),
                    Position::new(pos.x, pos.y - 1),
                    Position::new(pos.x, pos.y + 1),
                ] {
                    if next.x >= 0 && next.y >= 0 && next.x < buffer.width() && next.y < buffer.height() && !visited.is_selected(next) {
                        visited.set_is_selected(next, true);
                        pending.push(next);
                    }
                }
            }
        } else {
            for y in 0..buffer.height() {
                for x in 0..buffer.width() {
                    let pos = Position::new(x, y);
                    matched.set_is_selected(pos, matcher.matches(sample(pos)));
                }
            }
        }
        if add_type != AddType::Default {
            for y in 0..buffer.height() {
                for x in 0..buffer.width() {
                    let pos = Position::new(x, y);
                    let selected = match add_type {
                        AddType::Add => self.is_selected(pos) || matched.is_selected(pos),
                        AddType::Subtract => self.is_selected(pos) && !matched.is_selected(pos),
                        AddType::Default => unreachable!(),
                    };
                    matched.set_is_selected(pos, selected);
                }
            }
        }
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-set_selection"));
        self.deselect()?;
        if matched != self.selection_mask {
            self.push_undo_action(EditorUndoOp::SetSelectionMask {
                description: fl!(crate::LANGUAGE_LOADER, "undo-set_selection"),
                old: self.selection_mask.clone(),
                new: matched,
            })?;
        }
        Ok(())
    }

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
            let sel: Option<Selection> = self.selection_opt.take();
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
        if let Some(sel) = self.selection_opt {
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

    /// Extend selection by moving the lead position by (dx, dy).
    /// If no selection exists, creates one with anchor at current caret position.
    /// Used for Shift+Arrow key selection.
    pub fn extend_selection(&mut self, dx: i32, dy: i32) {
        let caret_pos = Position::new(self.get_caret().x, self.get_caret().y);

        let selection = if let Some(mut sel) = self.selection_opt {
            // Extend existing selection
            sel.lead.x = (sel.lead.x + dx).max(0);
            sel.lead.y = (sel.lead.y + dy).max(0);
            sel
        } else {
            // Create new selection from current caret position
            let mut sel = Selection::new(caret_pos);
            sel.lead.x = (caret_pos.x + dx).max(0);
            sel.lead.y = (caret_pos.y + dy).max(0);
            sel
        };

        self.selection_opt = Some(selection);
        self.screen.buffer.mark_overlay_dirty();
        // Move caret to lead position
        self.set_caret_position(selection.lead);
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
