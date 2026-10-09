//! Editing live TheDraw text layers in place, like the text tool of an image editor.

use icy_engine::{LiveText, Position, Rectangle, TextPane};

use crate::document::{Document, DrawResult};

/// The live text layer edited on the canvas and the caret in its text.
#[derive(Clone, Debug, PartialEq)]
pub struct TextEdit {
    /// The layer being edited; `None` until the first character creates it.
    pub layer: Option<usize>,
    /// Settings and document position of the layer the first character creates.
    pending: Option<(LiveText, Position)>,
    /// The number of characters before the caret.
    pub cursor: usize,
    /// The undo length after the last typed change, so continued typing extends that step.
    typed: Option<usize>,
}

/// Keys that move the caret or delete text while editing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextKey {
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
}

/// The caret index nearest to `target` on its line; lines are the rows the carets start at.
fn nearest_caret(carets: &[Position], target: Position) -> usize {
    let line = carets.iter().map(|caret| caret.y).filter(|y| *y <= target.y).max().unwrap_or(0);
    carets
        .iter()
        .enumerate()
        .filter(|(_, caret)| caret.y == line)
        .min_by_key(|(_, caret)| (caret.x - target.x).abs())
        .map_or(0, |(index, _)| index)
}

fn byte_index(text: &str, characters: usize) -> usize {
    text.char_indices().nth(characters).map_or(text.len(), |(index, _)| index)
}

impl Document {
    pub(crate) fn prepare_text_history(&mut self, redo: bool) {
        use icy_engine_edit::EditorUndoOp;

        fn changes_layers(op: &EditorUndoOp) -> bool {
            match op {
                EditorUndoOp::Atomic { operations, .. } => operations.iter().any(changes_layers),
                EditorUndoOp::Reversed { op, .. } => changes_layers(op),
                EditorUndoOp::AddLayer { .. }
                | EditorUndoOp::RemoveLayer { .. }
                | EditorUndoOp::RaiseLayer { .. }
                | EditorUndoOp::LowerLayer { .. }
                | EditorUndoOp::MergeLayerDown { .. }
                | EditorUndoOp::Paste { .. }
                | EditorUndoOp::Crop { .. } => true,
                EditorUndoOp::ReplaceLayers { .. } => true,
                _ => false,
            }
        }

        if self.text_edit.is_none() {
            return;
        }
        let changes_layers = self.with_state(|state| {
            let stack = state.get_undo_stack();
            let stack = stack.lock().unwrap();
            let operations = if redo { stack.redo_stack() } else { stack.undo_stack() };
            operations.last().is_some_and(changes_layers)
        });
        if changes_layers {
            // Keep a new draft at the same position so automatic typing cannot reopen
            // another layer. Do not remove empty layers while traversing history.
            let position = self
                .text_frame()
                .map(|frame| frame.start)
                .or_else(|| self.text_caret().map(|(position, _)| position));
            self.text_edit = self.text_source().zip(position).map(|(mut source, position)| {
                source.text.clear();
                TextEdit {
                    layer: None,
                    pending: Some((source, position)),
                    cursor: 0,
                    typed: None,
                }
            });
        } else if let Some(edit) = &mut self.text_edit {
            edit.typed = None;
        }
    }

    /// The topmost visible live text layer covering `position`.
    pub fn live_text_at(&self, position: Position) -> Option<usize> {
        self.with_state(|state| {
            state
                .get_buffer()
                .layers
                .iter()
                .enumerate()
                .rfind(|(index, layer)| layer.is_text() && state.get_buffer().layer_is_visible(*index) && layer.rectangle().contains_pt(position))
                .map(|(index, _)| index)
        })
    }

    /// The text layer the font tool's settings apply to: the one being edited, or else the
    /// current layer if it is a live text layer.
    pub fn text_layer(&self) -> Option<usize> {
        if let Some(edit) = &self.text_edit {
            // A new text without its first character has no layer yet.
            return edit.layer;
        }
        self.with_state(|state| {
            let index = state.get_current_layer().ok()?;
            state.get_buffer().layers.get(index)?.is_text().then_some(index)
        })
    }

    /// The text settings of the edited or selected text layer, or of the layer about to be created.
    pub fn text_source(&self) -> Option<LiveText> {
        if let Some((source, _)) = self.text_edit.as_ref().and_then(|edit| edit.pending.as_ref()) {
            return Some(source.clone());
        }
        let index = self.text_layer()?;
        self.with_state(|state| state.get_buffer().layers.get(index)?.live_text().cloned())
    }

    /// Starts editing at `position`: in the live text layer there when `reuse` allows it, or
    /// else in a new layer made from `new` with the first typed character. Returns whether
    /// editing started.
    pub fn begin_text_edit(&mut self, position: Position, new: Option<LiveText>, reuse: bool) -> DrawResult<bool> {
        if self.paste_active() {
            return Ok(false);
        }
        self.finish();
        self.finish_text_edit()?;
        if let Some(index) = self.live_text_at(position).filter(|_| reuse) {
            let cursor = self.with_state(|state| {
                state.set_current_layer(index);
                let layer = &state.get_buffer().layers[index];
                let (carets, _) = layer.live_text()?.caret_layout().ok()?;
                Some(nearest_caret(&carets, position - layer.offset()))
            });
            self.text_edit = Some(TextEdit {
                layer: Some(index),
                pending: None,
                cursor: cursor.unwrap_or(0),
                typed: None,
            });
            return Ok(true);
        }
        let Some(mut source) = new else {
            return Ok(false);
        };
        source.text.clear();
        self.text_edit = Some(TextEdit {
            layer: None,
            pending: Some((source, position)),
            cursor: 0,
            typed: None,
        });
        Ok(true)
    }

    /// Starts editing the live text layer `index` with the caret after its text.
    pub fn edit_text_layer(&mut self, index: usize) -> DrawResult<bool> {
        self.finish();
        self.finish_text_edit()?;
        let Some(length) = self.with_state(|state| {
            state.set_current_layer(index);
            state.get_buffer().layers.get(index)?.live_text().map(|source| source.text.chars().count())
        }) else {
            return Ok(false);
        };
        self.text_edit = Some(TextEdit {
            layer: Some(index),
            pending: None,
            cursor: length,
            typed: None,
        });
        Ok(true)
    }

    /// Ends editing; a text layer left empty is removed.
    pub fn finish_text_edit(&mut self) -> DrawResult<()> {
        if !self.valid_text_edit() {
            return Ok(());
        }
        let Some(index) = self.text_edit.take().and_then(|edit| edit.layer) else {
            return Ok(());
        };
        let empty = self.with_state(|state| state.get_buffer().layers[index].live_text().is_some_and(|source| source.text.is_empty()));
        if empty && self.with_state(|state| state.get_buffer().layers.len()) > 1 {
            self.with_state(|state| state.remove_layer(index)).map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    /// Whether a text is being edited; ends editing of a layer that undo or another edit removed.
    pub(crate) fn valid_text_edit(&mut self) -> bool {
        let Some(edit) = &self.text_edit else {
            return false;
        };
        let Some(index) = edit.layer else {
            return true;
        };
        let length = self.with_state(|state| {
            state
                .get_buffer()
                .layers
                .get(index)
                .and_then(|layer| layer.live_text())
                .map(|source| source.text.chars().count())
        });
        match (length, &mut self.text_edit) {
            (Some(length), Some(edit)) => {
                edit.cursor = edit.cursor.min(length);
                true
            }
            _ => {
                self.text_edit = None;
                false
            }
        }
    }

    /// Types `text` at the caret, leaving out characters the font has no glyph for.
    pub fn text_edit_insert(&mut self, text: &str) -> DrawResult<()> {
        let Some(mut source) = self.valid_text_edit().then(|| self.text_source()).flatten() else {
            return Ok(());
        };
        let text = source.renderable(text).map_err(|error| error.to_string())?;
        if text.is_empty() {
            return Ok(());
        }
        let edit = self.text_edit.as_ref().unwrap();
        let cursor = edit.cursor;
        source.text.insert_str(byte_index(&source.text, cursor), &text);
        self.commit_text(source)?;
        if let Some(edit) = &mut self.text_edit {
            edit.cursor = cursor + text.chars().count();
        }
        Ok(())
    }

    /// Deletes text or moves the caret.
    pub fn text_edit_key(&mut self, key: TextKey) -> DrawResult<()> {
        let Some(mut source) = self.valid_text_edit().then(|| self.text_source()).flatten() else {
            return Ok(());
        };
        let cursor = self.text_edit.as_ref().unwrap().cursor;
        let length = source.text.chars().count();
        match key {
            TextKey::Backspace | TextKey::Delete => {
                let removed = if key == TextKey::Backspace {
                    cursor.checked_sub(1)
                } else {
                    (cursor < length).then_some(cursor)
                };
                let Some(removed) = removed else {
                    return Ok(());
                };
                let start = byte_index(&source.text, removed);
                source.text.replace_range(start..byte_index(&source.text, removed + 1), "");
                self.commit_text(source)?;
                if let Some(edit) = &mut self.text_edit {
                    edit.cursor = removed;
                }
            }
            _ => {
                let (carets, _) = source.caret_layout().map_err(|error| error.to_string())?;
                let current = carets[cursor.min(carets.len() - 1)];
                let line = |y: i32| (0..carets.len()).filter(|index| carets[*index].y == y).collect::<Vec<_>>();
                let target = match key {
                    TextKey::Left => cursor.saturating_sub(1),
                    TextKey::Right => (cursor + 1).min(length),
                    TextKey::Home => line(current.y).into_iter().min().unwrap_or(cursor),
                    TextKey::End => line(current.y).into_iter().max().unwrap_or(cursor),
                    TextKey::Up | TextKey::Down => {
                        let next = if key == TextKey::Up {
                            carets.iter().map(|caret| caret.y).filter(|y| *y < current.y).max()
                        } else {
                            carets.iter().map(|caret| caret.y).filter(|y| *y > current.y).min()
                        };
                        next.map_or(cursor, |y| nearest_caret(&carets, Position::new(current.x, y)))
                    }
                    TextKey::Backspace | TextKey::Delete => unreachable!(),
                };
                if let Some(edit) = &mut self.text_edit {
                    edit.cursor = target;
                }
            }
        }
        Ok(())
    }

    /// Changes the font, colors or spacing of the edited or selected text layer as one undo step.
    pub fn modify_text(&mut self, change: impl FnOnce(&mut LiveText)) -> DrawResult<()> {
        self.valid_text_edit();
        if let Some((source, _)) = self.text_edit.as_mut().and_then(|edit| edit.pending.as_mut()) {
            let mut changed = source.clone();
            change(&mut changed);
            changed.font().map_err(|error| error.to_string())?;
            *source = changed;
            return Ok(());
        }
        let Some(index) = self.text_layer() else {
            return Ok(());
        };
        let Some(mut source) = self.text_source() else {
            return Ok(());
        };
        change(&mut source);
        self.with_state(|state| state.update_live_text(index, source))
            .map_err(|error| error.to_string())?;
        if let Some(edit) = &mut self.text_edit {
            edit.typed = None;
        }
        self.valid_text_edit();
        Ok(())
    }

    fn commit_text(&mut self, source: LiveText) -> DrawResult<()> {
        let edit = self.text_edit.as_mut().unwrap();
        if let Some((original, position)) = edit.pending.take() {
            let result = self.with_state(|state| {
                state.add_live_text(source.clone(), position)?;
                state.get_current_layer()
            });
            match result {
                Ok(index) => self.text_edit.as_mut().unwrap().layer = Some(index),
                Err(error) => {
                    self.text_edit.as_mut().unwrap().pending = Some((original, position));
                    return Err(error.to_string());
                }
            }
            return Ok(());
        }
        let index = edit.layer.unwrap();
        let typed = edit.typed;
        let continue_typing = typed.is_some_and(|length| length == self.with_state(|state| state.undo_stack_len()));
        let length = self
            .with_state(|state| {
                state.type_live_text(index, source, continue_typing)?;
                Ok::<_, icy_engine::EngineError>(state.undo_stack_len())
            })
            .map_err(|error| error.to_string())?;
        self.text_edit.as_mut().unwrap().typed = Some(length);
        Ok(())
    }

    /// The caret in document coordinates and its height in rows.
    pub fn text_caret(&self) -> Option<(Position, i32)> {
        let edit = self.text_edit.as_ref()?;
        if let Some((source, position)) = &edit.pending {
            return Some((*position, source.caret_layout().ok()?.1));
        }
        self.with_state(|state| {
            let layer = state.get_buffer().layers.get(edit.layer?)?;
            let (carets, height) = layer.live_text()?.caret_layout().ok()?;
            Some((carets[edit.cursor.min(carets.len() - 1)] + layer.offset(), height))
        })
    }

    /// The area of the text layer the font tool's settings apply to, in document coordinates.
    pub fn text_frame(&self) -> Option<Rectangle> {
        let index = self.text_layer()?;
        self.with_state(|state| state.get_buffer().layers.get(index).map(|layer| layer.rectangle()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use icy_engine::{Size, TextAttribute};
    use retrofont::{
        tdf::{TdfFont, TdfFontType},
        Glyph, GlyphPart,
    };

    fn source(name: &str, part: char) -> LiveText {
        let mut font = TdfFont::new(name, TdfFontType::Block, 1);
        for character in ['A', 'B'] {
            let mut glyph = Glyph::new(2, 2);
            glyph.parts = vec![
                GlyphPart::Char(part),
                GlyphPart::Char(character),
                GlyphPart::NewLine,
                GlyphPart::Char(part),
                GlyphPart::Char(part),
            ];
            font.add_glyph(character, glyph);
        }
        LiveText::new(&font, TextAttribute::new(4, 0)).unwrap()
    }

    fn text(document: &Document) -> String {
        document.text_source().unwrap().text
    }

    #[test]
    fn undo_creation_does_not_retarget_another_text_layer() {
        let mut document = Document::new(Size::new(20, 10));
        let mut existing = source("Other", 'Y');
        existing.text = "B".into();
        document.with_state(|state| {
            state.add_live_text(existing.clone(), Position::new(10, 0)).unwrap();
            state.set_current_layer(0);
        });
        document.begin_text_edit(Position::default(), Some(source("Font", 'X')), false).unwrap();
        document.text_edit_insert("A").unwrap();
        document.undo().unwrap();
        assert_eq!(document.text_layer(), None);
        document.with_state(|state| {
            assert_eq!(state.get_buffer().layers.len(), 2);
            assert_eq!(state.get_buffer().layers[1].live_text(), Some(&existing));
        });
        document.redo().unwrap();
        assert_eq!(document.text_layer(), None, "redo does not resume the old layer's session");
        document.undo().unwrap();
        document.text_edit_insert("A").unwrap();
        document.with_state(|state| {
            assert_eq!(state.get_buffer().layers.len(), 3);
            assert_eq!(state.get_buffer().layers[1].live_text(), Some(&existing));
            assert_eq!(state.get_buffer().layers[2].live_text().unwrap().text, "A");
            assert_eq!(state.get_buffer().layers[2].offset(), Position::default());
        });
    }

    #[test]
    fn undo_and_redo_layer_reordering_end_inline_editing() {
        let mut document = Document::new(Size::new(20, 10));
        document.with_state(|state| {
            for text in ["A", "B"] {
                let mut source = source("Font", 'X');
                source.text = text.into();
                state.add_live_text(source, Position::default()).unwrap();
            }
            let _undo = state.begin_atomic_undo("Reorder");
            state.raise_layer(1).unwrap();
        });
        document.edit_text_layer(2).unwrap();
        document.undo().unwrap();
        assert_eq!(document.text_layer(), None, "nested structural undo ends editing the layer");
        document.edit_text_layer(1).unwrap();
        document.redo().unwrap();
        assert_eq!(document.text_layer(), None, "structural redo ends editing the layer too");
        document.with_state(|state| {
            assert_eq!(state.get_buffer().layers[1].live_text().unwrap().text, "B");
            assert_eq!(state.get_buffer().layers[2].live_text().unwrap().text, "A");
        });
    }

    #[test]
    fn rejected_initial_input_preserves_the_pending_draft() {
        for length in [501, 65_537] {
            let mut document = Document::new(Size::new(20, 10));
            let source = source("Font", 'X');
            document.begin_text_edit(Position::new(3, 2), Some(source.clone()), false).unwrap();
            let before = document.text_edit.clone();
            let undo = document.with_state(|state| state.undo_stack_len());
            assert!(document.text_edit_insert(&"A".repeat(length)).is_err());
            assert_eq!(document.text_edit, before);
            assert_eq!(document.text_source(), Some(source));
            assert_eq!(document.with_state(|state| state.undo_stack_len()), undo);
            assert_eq!(document.with_state(|state| state.get_buffer().layers.len()), 1);
            document.text_edit_insert("B").unwrap();
            assert_eq!(text(&document), "B");
            assert_eq!(document.text_caret(), Some((Position::new(5, 2), 2)));
        }
    }

    #[test]
    fn typing_creates_a_layer_and_one_undo_step_per_session() {
        let mut document = Document::new(Size::new(20, 10));
        assert!(document.begin_text_edit(Position::new(3, 2), Some(source("Font", 'X')), true).unwrap());
        assert_eq!(
            document.with_state(|state| state.get_buffer().layers.len()),
            1,
            "nothing until the first character"
        );
        assert_eq!(document.text_caret(), Some((Position::new(3, 2), 2)));
        let before = document.with_state(|state| state.undo_stack_len());
        document.text_edit_insert("a?").unwrap();
        assert_eq!(document.with_state(|state| state.get_buffer().layers.len()), 2);
        assert_eq!(document.text_layer(), Some(1));
        assert_eq!(document.with_state(|state| state.get_buffer().layers[1].offset()), Position::new(3, 2));
        for character in ["b", "\n", "A"] {
            document.text_edit_insert(character).unwrap();
        }
        assert_eq!(text(&document), "ab\nA");
        assert_eq!(document.text_caret(), Some((Position::new(5, 4), 2)));
        assert_eq!(document.with_state(|state| state.undo_stack_len()), before + 2, "creation and typing");
        document.undo().unwrap();
        assert_eq!(text(&document), "a");
        assert_eq!(document.text_edit.as_ref().unwrap().cursor, 1, "the caret stays inside the text");
        document.text_edit_insert("B").unwrap();
        assert_eq!(text(&document), "aB");
        document.undo().unwrap();
        document.undo().unwrap();
        assert_eq!(document.with_state(|state| state.get_buffer().layers.len()), 1);
        assert_eq!(document.text_layer(), None, "editing the removed layer ends");
        document.text_edit_insert("A").unwrap();
        assert_eq!(text(&document), "A", "subsequent typing creates a new text layer");
    }

    #[test]
    fn caret_keys_clicks_and_deletion_follow_the_layout() {
        let mut document = Document::new(Size::new(20, 10));
        document.begin_text_edit(Position::new(1, 1), Some(source("Font", 'X')), true).unwrap();
        document.text_edit_insert("AB\nBA").unwrap();
        document.finish_text_edit().unwrap();
        assert!(document.begin_text_edit(Position::new(4, 4), Some(source("Other", 'Y')), true).unwrap());
        assert_eq!(document.text_edit.as_ref().unwrap().cursor, 4, "clicked between the glyphs of the second line");
        document.text_edit_key(TextKey::Up).unwrap();
        assert_eq!(document.text_edit.as_ref().unwrap().cursor, 1);
        document.text_edit_key(TextKey::End).unwrap();
        assert_eq!(document.text_edit.as_ref().unwrap().cursor, 2);
        document.text_edit_key(TextKey::Down).unwrap();
        assert_eq!(document.text_edit.as_ref().unwrap().cursor, 5);
        document.text_edit_key(TextKey::Home).unwrap();
        assert_eq!(document.text_edit.as_ref().unwrap().cursor, 3);
        document.text_edit_key(TextKey::Backspace).unwrap();
        assert_eq!(text(&document), "ABBA");
        document.text_edit_key(TextKey::Delete).unwrap();
        assert_eq!(text(&document), "ABA");
        document.text_edit_key(TextKey::Left).unwrap();
        document.text_edit_key(TextKey::Right).unwrap();
        assert_eq!(document.text_edit.as_ref().unwrap().cursor, 2);
        // A new layer instead of the one under the position.
        assert!(document.begin_text_edit(Position::new(2, 2), Some(source("Other", 'Y')), false).unwrap());
        assert!(document.text_edit.as_ref().unwrap().layer.is_none());
    }

    #[test]
    fn settings_change_the_selected_layer_and_empty_layers_are_removed() {
        let mut document = Document::new(Size::new(20, 10));
        document.begin_text_edit(Position::new(0, 0), Some(source("Font", 'X')), true).unwrap();
        document.text_edit_insert("A").unwrap();
        let other = source("Other", 'Y');
        let undo = document.with_state(|state| state.undo_stack_len());
        document.modify_text(|text| text.font_data = other.font_data.clone()).unwrap();
        assert_eq!(document.with_state(|state| state.undo_stack_len()), undo + 1);
        assert_eq!(document.with_state(|state| state.get_buffer().char_at(Position::default()).ch), 'Y');
        document.text_edit_insert("B").unwrap();
        assert_eq!(
            document.with_state(|state| state.undo_stack_len()),
            undo + 2,
            "typing after a font change is its own step"
        );
        document.finish_text_edit().unwrap();
        assert_eq!(document.text_layer(), Some(1), "the layer stays selected");
        document.modify_text(|text| text.letter_spacing = 2).unwrap();
        assert_eq!(document.text_source().unwrap().letter_spacing, 2);
        document.edit_text_layer(1).unwrap();
        document.text_edit_key(TextKey::Backspace).unwrap();
        document.text_edit_key(TextKey::Backspace).unwrap();
        document.finish_text_edit().unwrap();
        assert_eq!(document.with_state(|state| state.get_buffer().layers.len()), 1);
        document.undo().unwrap();
        assert_eq!(document.with_state(|state| state.get_buffer().layers[1].live_text().unwrap().text.clone()), "");
    }
}
