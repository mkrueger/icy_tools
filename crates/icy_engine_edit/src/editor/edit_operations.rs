#![allow(clippy::missing_errors_doc)]

use std::mem;

use i18n_embed_fl::fl;

use crate::{
    clipboard, load_with_parser, AnsiParser, AttributedChar, EditableScreen, Layer, Line, Palette, Position, Rectangle, Result, Role, Sixel, Size, TextPane,
};

use super::{EditState, EditorUndoOp};

impl EditState {
    pub fn set_char(&mut self, pos: impl Into<Position>, attributed_char: AttributedChar) -> Result<()> {
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-set_char"));

        if let Some(layer) = self.get_cur_layer() {
            let pos = pos.into();
            let old = layer.char_at(pos);

            if self.mirror_mode {
                let mirror_pos = Position::new(layer.width() - pos.x - 1, pos.y);
                let mirror_old = layer.char_at(mirror_pos);
                self.push_undo_action(EditorUndoOp::SetChar {
                    pos: mirror_pos,
                    layer: self.get_current_layer()?,
                    old: mirror_old,
                    new: attributed_char,
                    undo_caret: None,
                    redo_caret: None,
                })?;
            }

            self.push_undo_action(EditorUndoOp::SetChar {
                pos,
                layer: self.get_current_layer()?,
                old,
                new: attributed_char,
                undo_caret: None,
                redo_caret: None,
            })
        } else {
            Err(crate::EngineError::Generic("Current layer is invalid".to_string()))
        }
    }

    /// Set a character without starting its own atomic undo group.
    ///
    /// This is intended for tools that manage their own `AtomicUndoGuard` (e.g. brush strokes)
    /// and need to push many `UndoSetChar` operations into a single undo entry.
    pub fn set_char_in_atomic(&mut self, pos: impl Into<Position>, attributed_char: AttributedChar) -> Result<()> {
        if let Some(layer) = self.get_cur_layer() {
            let pos = pos.into();
            let old = layer.char_at(pos);

            if self.mirror_mode {
                let mirror_pos = Position::new(layer.width() - pos.x - 1, pos.y);
                let mirror_old = layer.char_at(mirror_pos);
                self.push_undo_action(EditorUndoOp::SetChar {
                    pos: mirror_pos,
                    layer: self.get_current_layer()?,
                    old: mirror_old,
                    new: attributed_char,
                    undo_caret: None,
                    redo_caret: None,
                })?;
            }

            self.push_undo_action(EditorUndoOp::SetChar {
                pos,
                layer: self.get_current_layer()?,
                old,
                new: attributed_char,
                undo_caret: None,
                redo_caret: None,
            })
        } else {
            Err(crate::EngineError::Generic("Current layer is invalid".to_string()))
        }
    }

    /// Set a character at a specific layer without starting its own atomic undo group.
    ///
    /// This is intended for MCP operations that need to set characters on a specific layer
    /// while managing their own `AtomicUndoGuard`.
    pub fn set_char_at_layer_in_atomic(&mut self, layer_index: usize, pos: impl Into<Position>, attributed_char: AttributedChar) -> Result<()> {
        let buffer = self.get_buffer();
        if layer_index >= buffer.layers.len() {
            return Err(crate::EngineError::Generic(format!("Layer index {} out of range", layer_index)));
        }

        let pos = pos.into();
        let old = buffer.layers[layer_index].char_at(pos);

        self.push_undo_action(EditorUndoOp::SetChar {
            pos,
            layer: layer_index,
            old,
            new: attributed_char,
            undo_caret: None,
            redo_caret: None,
        })
    }

    pub fn swap_char(&mut self, pos1: impl Into<Position>, pos2: impl Into<Position>) -> Result<()> {
        let pos1 = pos1.into();
        let pos2 = pos2.into();
        let layer = self.get_current_layer()?;
        self.push_undo_action(EditorUndoOp::SwapChar { layer, pos1, pos2 })
    }

    /// .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn paste_clipboard_data(&mut self, data: &[u8]) -> Result<()> {
        log::debug!("paste_clipboard_data: received {} bytes", data.len());
        if let Some(mut layer) = clipboard::from_clipboard_data(self.get_buffer().buffer_type, data) {
            // Position the pasted layer at the current caret position
            let caret_pos = self.screen.caret.position();
            layer.set_offset((caret_pos.x, caret_pos.y));
            log::debug!(
                "paste_clipboard_data: created layer {}x{} at offset {:?}",
                layer.size().width,
                layer.size().height,
                layer.offset()
            );
            self.push_undo_action(EditorUndoOp::Paste {
                current_layer: self.get_current_layer()?,
                layer: Box::new(layer),
            })?;
        } else {
            log::warn!("paste_clipboard_data: from_clipboard_data returned None");
        }
        self.selection_opt = None;
        Ok(())
    }

    pub fn paste_sixel(&mut self, sixel: Sixel) -> Result<()> {
        let dims = self.get_buffer().font_dimensions();
        let caret_pos = self.screen.caret.position();

        let mut layer = Layer::new(
            fl!(crate::LANGUAGE_LOADER, "layer-pasted-name"),
            (
                (sixel.width() as f32 / dims.width as f32).ceil() as i32,
                (sixel.height() as f32 / dims.height as f32).ceil() as i32,
            ),
        );
        layer.role = Role::Image;
        layer.properties.has_alpha_channel = true;
        layer.sixels.push(sixel);
        // Position the pasted layer at the current caret position
        layer.set_offset((caret_pos.x, caret_pos.y));

        self.push_undo_action(EditorUndoOp::Paste {
            current_layer: self.get_current_layer()?,
            layer: Box::new(layer),
        })?;
        self.selection_opt = None;
        Ok(())
    }

    pub fn paste_text(&mut self, text: &str) -> Result<()> {
        let x = self.screen.caret.position().x;
        let y = self.screen.caret.position().y;

        let width = self.get_buffer().size().width - x;
        let mut result = crate::TextScreen::new((width, 25));
        result.terminal_state_mut().is_terminal_buffer = false;

        let mut parser = AnsiParser::new();

        let text = text
            .chars()
            .map(|ch| self.screen.buffer.buffer_type.convert_from_unicode(ch))
            .collect::<String>();
        load_with_parser(&mut result, &mut parser, text.as_bytes(), true, 0)?;

        let mut layer: Layer = result.buffer.layers.remove(0);
        layer.properties.has_alpha_channel = true;
        layer.set_offset((x, y));

        self.push_undo_action(EditorUndoOp::Paste {
            current_layer: self.get_current_layer()?,
            layer: Box::new(layer),
        })?;
        self.selection_opt = None;
        Ok(())
    }

    /// .
    ///
    /// # Panics
    ///
    /// Panics if .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn resize_buffer(&mut self, resize_layer: bool, size: impl Into<Size>) -> Result<()> {
        if resize_layer {
            let size = size.into();
            let rect = Rectangle::from_min_size(Position::default(), size);
            let old_size = self.get_buffer().size();
            let mut old_layers = Vec::new();
            mem::swap(&mut self.get_buffer_mut().layers, &mut old_layers);

            self.get_buffer_mut().set_size(rect.size);
            self.get_buffer_mut().layers.clear();

            for old_layer in &old_layers {
                let mut new_layer = old_layer.clone();
                new_layer.lines.clear();
                let new_rectangle = old_layer.rectangle().intersect(&rect);
                if new_rectangle.is_empty() {
                    continue;
                }

                new_layer.set_offset(new_rectangle.start - rect.start);
                new_layer.set_size(new_rectangle.size);

                for y in 0..new_rectangle.height() {
                    for x in 0..new_rectangle.width() {
                        let ch = old_layer.char_at((x + new_rectangle.left(), y + new_rectangle.top()).into());
                        new_layer.set_char((x, y), ch);
                    }
                }
                self.get_buffer_mut().layers.push(new_layer);
            }
            if self.get_buffer_mut().layers[0].size() == old_size {
                self.get_buffer_mut().layers[0].set_size(size);
            }

            return self.push_plain_undo(EditorUndoOp::Crop {
                orig_size: old_size,
                size: rect.size(),
                layers: old_layers,
            });
        }

        self.push_undo_action(EditorUndoOp::ResizeBuffer {
            orig_size: self.get_buffer().size(),
            size: size.into(),
        })
    }

    pub fn center_line(&mut self) -> Result<()> {
        let offset = if let Some(layer) = self.get_cur_layer() { layer.offset().y } else { 0 };
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-delete-selection"));

        let y = self.screen.caret.position().y + offset;
        self.set_selection(Rectangle::from_coords(-1_000_000, y, 1_000_000, y + 1))?;
        let res = self.center();
        self.clear_selection()?;
        res
    }

    pub fn justify_line_left(&mut self) -> Result<()> {
        let offset: i32 = if let Some(layer) = self.get_cur_layer() { layer.offset().y } else { 0 };
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-delete-selection"));

        let y = self.screen.caret.position().y + offset;
        self.set_selection(Rectangle::from_coords(-1_000_000, y, 1_000_000, y + 1))?;
        let res = self.justify_left();
        self.clear_selection()?;
        res
    }

    pub fn justify_line_right(&mut self) -> Result<()> {
        let offset: i32 = if let Some(layer) = self.get_cur_layer() { layer.offset().y } else { 0 };
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-delete-selection"));

        let y = self.screen.caret.position().y + offset;
        self.set_selection(Rectangle::from_coords(-1_000_000, y, 1_000_000, y + 1))?;
        let res = self.justify_right();
        self.clear_selection()?;
        res
    }

    pub fn delete_row(&mut self) -> Result<()> {
        let y = self.screen.caret.position().y;
        let layer = self.get_current_layer()?;
        self.push_undo_action(EditorUndoOp::DeleteRow {
            layer,
            line: y,
            deleted_row: Line::new(),
        })
    }

    pub fn insert_row(&mut self) -> Result<()> {
        let y = self.screen.caret.position().y;
        let layer = self.get_current_layer()?;
        self.push_undo_action(EditorUndoOp::InsertRow {
            layer,
            line: y,
            inserted_row: Line::new(),
        })
    }

    pub fn insert_column(&mut self) -> Result<()> {
        let x = self.screen.caret.position().x;
        let layer = self.get_current_layer()?;
        self.push_undo_action(EditorUndoOp::InsertColumn { layer, column: x })
    }

    pub fn delete_column(&mut self) -> Result<()> {
        let x = self.screen.caret.position().x;
        let layer = self.get_current_layer()?;
        self.push_undo_action(EditorUndoOp::DeleteColumn {
            layer,
            column: x,
            deleted_chars: Vec::new(),
        })
    }

    pub fn erase_row(&mut self) -> Result<()> {
        let offset = if let Some(layer) = self.get_cur_layer() { layer.offset().y } else { 0 };
        let y = self.screen.caret.position().y + offset;
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-delete-selection"));

        self.set_selection(Rectangle::from_coords(-1_000_000, y, 1_000_000, y + 1))?;
        self.erase_selection()
    }

    pub fn erase_row_to_start(&mut self) -> Result<()> {
        let offset = if let Some(layer) = self.get_cur_layer() {
            layer.offset()
        } else {
            Position::default()
        };
        let y = self.screen.caret.position().y + offset.y;
        let x = self.screen.caret.position().x + offset.x;
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-delete-selection"));

        self.set_selection(Rectangle::from_coords(-1_000_000, y, x, y + 1))?;
        self.erase_selection()
    }

    pub fn erase_row_to_end(&mut self) -> Result<()> {
        let offset = if let Some(layer) = self.get_cur_layer() {
            layer.offset()
        } else {
            Position::default()
        };
        let y = self.screen.caret.position().y + offset.y;
        let x = self.screen.caret.position().x + offset.x;
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-delete-selection"));

        self.set_selection(Rectangle::from_coords(x, y, 1_000_000, y + 1))?;
        self.erase_selection()
    }

    pub fn erase_column(&mut self) -> Result<()> {
        let offset = if let Some(layer) = self.get_cur_layer() {
            layer.offset()
        } else {
            Position::default()
        };
        let x = self.screen.caret.position().x + offset.x;
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-delete-selection"));

        self.set_selection(Rectangle::from_coords(x, -1_000_000, x, 1_000_000))?;
        self.erase_selection()
    }

    pub fn erase_column_to_start(&mut self) -> Result<()> {
        let offset = if let Some(layer) = self.get_cur_layer() {
            layer.offset()
        } else {
            Position::default()
        };
        let y = self.screen.caret.position().y + offset.y;
        let x = self.screen.caret.position().x + offset.x;
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-delete-selection"));

        self.set_selection(Rectangle::from_coords(x, -1_000_000, x, y))?;
        self.erase_selection()
    }

    pub fn erase_column_to_end(&mut self) -> Result<()> {
        let offset = if let Some(layer) = self.get_cur_layer() {
            layer.offset()
        } else {
            Position::default()
        };
        let y = self.screen.caret.position().y + offset.y;
        let x = self.screen.caret.position().x + offset.x;
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-delete-selection"));
        self.set_selection(Rectangle::from_coords(x, y, x, 1_000_000))?;
        self.erase_selection()
    }

    /// Returns the undo caret position of this [`EditState`].
    ///
    /// # Panics
    ///
    /// Panics if .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn undo_caret_position(&mut self) -> Result<()> {
        let pos = self.screen.caret.position();
        self.undo_stack.lock().unwrap().clear_redo();
        self.undo_stack
            .lock()
            .unwrap()
            .push_undo(EditorUndoOp::ReverseCaretPosition { pos, old_pos: pos });
        Ok(())
    }

    pub fn switch_to_palette(&mut self, pal: Palette) -> Result<()> {
        let old_palette = self.get_buffer().palette.clone();
        let old_layers = self.get_buffer().layers.clone();
        self.push_undo_action(EditorUndoOp::SwitchPalette {
            old_palette,
            old_layers,
            new_palette: pal,
            new_layers: Vec::new(), // Will be populated on redo
        })
    }

    /// Update SAUCE metadata with undo support
    /// Returns Ok(()) without creating an undo action if there are no changes.
    pub fn update_sauce_data(&mut self, sauce: crate::SauceMetaData) -> Result<()> {
        // Guard against no changes - don't create undo action if nothing changed
        let old = &self.sauce_meta;
        if old.title == sauce.title && old.author == sauce.author && old.group == sauce.group && old.comments == sauce.comments {
            return Ok(());
        }
        self.push_undo_action(EditorUndoOp::SetSauceData {
            new: sauce.clone(),
            old: self.sauce_meta.clone(),
        })
    }

    /// Push a reverse undo operation (for special cases like font backspace)
    pub fn push_reverse_undo(&mut self, description: impl Into<String>, op: EditorUndoOp, operation_type: super::OperationType) -> Result<()> {
        self.push_undo_action(EditorUndoOp::Reversed {
            description: description.into(),
            op: Box::new(op),
            operation_type,
        })
    }
}
