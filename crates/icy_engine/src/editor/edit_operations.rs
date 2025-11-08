#![allow(clippy::missing_errors_doc)]

use std::mem;

use i18n_embed_fl::fl;

use crate::{AttributedChar, EditableScreen, EngineResult, Layer, Palette, Position, Rectangle, Role, Sixel, Size, TextPane, parse_with_parser, parsers};

use super::{
    EditState, OperationType, UndoOperation,
    undo_operations::{Paste, ReverseCaretPosition, ReversedUndo, UndoSetChar, UndoSwapChar},
};

impl EditState {
    pub fn set_char(&mut self, pos: impl Into<Position>, attributed_char: AttributedChar) -> EngineResult<()> {
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-set_char"));

        if let Some(layer) = self.get_cur_layer() {
            let pos = pos.into();
            let old = layer.get_char(pos);

            if self.mirror_mode {
                let mirror_pos = Position::new(layer.get_width() - pos.x - 1, pos.y);
                let mirror_old = layer.get_char(mirror_pos);
                self.push_undo_action(Box::new(UndoSetChar {
                    pos: mirror_pos,
                    layer: self.get_current_layer()?,
                    old: mirror_old,
                    new: attributed_char,
                }))?;
            }

            self.push_undo_action(Box::new(UndoSetChar {
                pos,
                layer: self.get_current_layer()?,
                old,
                new: attributed_char,
            }))
        } else {
            Err(anyhow::anyhow!("Current layer is invalid"))
        }
    }

    pub fn swap_char(&mut self, pos1: impl Into<Position>, pos2: impl Into<Position>) -> EngineResult<()> {
        let pos1 = pos1.into();
        let pos2 = pos2.into();
        let layer = self.get_current_layer()?;
        let op = UndoSwapChar { layer, pos1, pos2 };
        self.push_undo_action(Box::new(op))
    }

    /// .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn paste_clipboard_data(&mut self, data: &[u8]) -> EngineResult<()> {
        if let Some(layer) = self.from_clipboard_data(data) {
            let op = Paste::new(self.get_current_layer()?, layer);
            self.push_undo_action(Box::new(op))?;
        }
        self.selection_opt = None;
        Ok(())
    }

    pub fn paste_sixel(&mut self, sixel: Sixel) -> EngineResult<()> {
        let dims = self.get_buffer().get_font_dimensions();

        let mut layer = Layer::new(
            fl!(crate::LANGUAGE_LOADER, "layer-pasted-name"),
            (
                (sixel.get_width() as f32 / dims.width as f32).ceil() as i32,
                (sixel.get_height() as f32 / dims.height as f32).ceil() as i32,
            ),
        );
        layer.role = crate::Role::PasteImage;
        layer.properties.has_alpha_channel = true;
        layer.sixels.push(sixel);

        let op = Paste::new(self.get_current_layer()?, layer);
        self.push_undo_action(Box::new(op))?;
        self.selection_opt = None;
        Ok(())
    }

    pub fn paste_text(&mut self, text: &str) -> EngineResult<()> {
        let x = self.caret.position().x;
        let y = self.caret.position().y;

        let width = self.get_buffer().get_size().width - x;
        let mut result = crate::TextScreen::new((width, 25));
        result.terminal_state_mut().is_terminal_buffer = false;

        let mut parser = parsers::ansi::Parser::default();
        parser.bs_is_ctrl_char = false;

        let text = text
            .chars()
            .map(|ch| self.unicode_converter.convert_from_unicode(ch, self.caret.font_page()))
            .collect::<String>();
        parse_with_parser(&mut result, &mut parser, &text, true)?;

        let mut layer: Layer = result.buffer.layers.remove(0);
        layer.properties.has_alpha_channel = true;
        layer.role = Role::PastePreview;
        layer.set_offset((x, y));

        let op = Paste::new(self.get_current_layer()?, layer);
        self.push_undo_action(Box::new(op))?;
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
    pub fn resize_buffer(&mut self, resize_layer: bool, size: impl Into<Size>) -> EngineResult<()> {
        if resize_layer {
            let size = size.into();
            let rect = Rectangle::from_min_size(Position::default(), size);
            let old_size = self.get_buffer().get_size();
            let mut old_layers = Vec::new();
            mem::swap(&mut self.get_buffer_mut().layers, &mut old_layers);

            self.get_buffer_mut().set_size(rect.size);
            self.get_buffer_mut().layers.clear();

            for old_layer in &old_layers {
                let mut new_layer = old_layer.clone();
                new_layer.lines.clear();
                let new_rectangle = old_layer.get_rectangle().intersect(&rect);
                if new_rectangle.is_empty() {
                    continue;
                }

                new_layer.set_offset(new_rectangle.start - rect.start);
                new_layer.set_size(new_rectangle.size);

                for y in 0..new_rectangle.get_height() {
                    for x in 0..new_rectangle.get_width() {
                        let ch = old_layer.get_char((x + new_rectangle.left(), y + new_rectangle.top()).into());
                        new_layer.set_char((x, y), ch);
                    }
                }
                self.get_buffer_mut().layers.push(new_layer);
            }
            if self.get_buffer_mut().layers[0].get_size() == old_size {
                self.get_buffer_mut().layers[0].set_size(size);
            }

            let op = super::undo_operations::Crop::new(old_size, rect.get_size(), old_layers);

            return self.push_plain_undo(Box::new(op));
        }

        let op = super::undo_operations::ResizeBuffer::new(self.get_buffer().get_size(), size);
        self.push_undo_action(Box::new(op))
    }

    pub fn center_line(&mut self) -> EngineResult<()> {
        let offset = if let Some(layer) = self.get_cur_layer() { layer.get_offset().y } else { 0 };
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-delete-selection"));

        let y = self.get_caret().position().y + offset;
        self.set_selection(Rectangle::from_coords(-1_000_000, y, 1_000_000, y + 1))?;
        let res = self.center();
        self.clear_selection()?;
        res
    }

    pub fn justify_line_left(&mut self) -> EngineResult<()> {
        let offset: i32 = if let Some(layer) = self.get_cur_layer() { layer.get_offset().y } else { 0 };
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-delete-selection"));

        let y = self.get_caret().position().y + offset;
        self.set_selection(Rectangle::from_coords(-1_000_000, y, 1_000_000, y + 1))?;
        let res = self.justify_left();
        self.clear_selection()?;
        res
    }

    pub fn justify_line_right(&mut self) -> EngineResult<()> {
        let offset: i32 = if let Some(layer) = self.get_cur_layer() { layer.get_offset().y } else { 0 };
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-delete-selection"));

        let y = self.get_caret().position().y + offset;
        self.set_selection(Rectangle::from_coords(-1_000_000, y, 1_000_000, y + 1))?;
        let res = self.justify_right();
        self.clear_selection()?;
        res
    }

    pub fn delete_row(&mut self) -> EngineResult<()> {
        let y = self.get_caret().position().y;
        let layer = self.get_current_layer()?;
        let op = super::undo_operations::DeleteRow::new(layer, y);
        self.push_undo_action(Box::new(op))
    }

    pub fn insert_row(&mut self) -> EngineResult<()> {
        let y = self.get_caret().position().y;
        let layer = self.get_current_layer()?;
        let op = super::undo_operations::InsertRow::new(layer, y);
        self.push_undo_action(Box::new(op))
    }

    pub fn insert_column(&mut self) -> EngineResult<()> {
        let x = self.get_caret().position().x;
        let layer = self.get_current_layer()?;
        let op = super::undo_operations::InsertColumn::new(layer, x);
        self.push_undo_action(Box::new(op))
    }

    pub fn delete_column(&mut self) -> EngineResult<()> {
        let x = self.get_caret().position().x;
        let layer = self.get_current_layer()?;
        let op = super::undo_operations::DeleteColumn::new(layer, x);
        self.push_undo_action(Box::new(op))
    }

    pub fn erase_row(&mut self) -> EngineResult<()> {
        let offset = if let Some(layer) = self.get_cur_layer() { layer.get_offset().y } else { 0 };
        let y = self.get_caret().position().y + offset;
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-delete-selection"));

        self.set_selection(Rectangle::from_coords(-1_000_000, y, 1_000_000, y + 1))?;
        self.erase_selection()
    }

    pub fn erase_row_to_start(&mut self) -> EngineResult<()> {
        let offset = if let Some(layer) = self.get_cur_layer() {
            layer.get_offset()
        } else {
            Position::default()
        };
        let y = self.get_caret().position().y + offset.y;
        let x = self.get_caret().position().x + offset.x;
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-delete-selection"));

        self.set_selection(Rectangle::from_coords(-1_000_000, y, x, y + 1))?;
        self.erase_selection()
    }

    pub fn erase_row_to_end(&mut self) -> EngineResult<()> {
        let offset = if let Some(layer) = self.get_cur_layer() {
            layer.get_offset()
        } else {
            Position::default()
        };
        let y = self.get_caret().position().y + offset.y;
        let x = self.get_caret().position().x + offset.x;
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-delete-selection"));

        self.set_selection(Rectangle::from_coords(x, y, 1_000_000, y + 1))?;
        self.erase_selection()
    }

    pub fn erase_column(&mut self) -> EngineResult<()> {
        let offset = if let Some(layer) = self.get_cur_layer() {
            layer.get_offset()
        } else {
            Position::default()
        };
        let x = self.get_caret().position().x + offset.x;
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-delete-selection"));

        self.set_selection(Rectangle::from_coords(x, -1_000_000, x, 1_000_000))?;
        self.erase_selection()
    }

    pub fn erase_column_to_start(&mut self) -> EngineResult<()> {
        let offset = if let Some(layer) = self.get_cur_layer() {
            layer.get_offset()
        } else {
            Position::default()
        };
        let y = self.get_caret().position().y + offset.y;
        let x = self.get_caret().position().x + offset.x;
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-delete-selection"));

        self.set_selection(Rectangle::from_coords(x, -1_000_000, x, y))?;
        self.erase_selection()
    }

    pub fn erase_column_to_end(&mut self) -> EngineResult<()> {
        let offset = if let Some(layer) = self.get_cur_layer() {
            layer.get_offset()
        } else {
            Position::default()
        };
        let y = self.get_caret().position().y + offset.y;
        let x = self.get_caret().position().x + offset.x;
        let _undo = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-delete-selection"));
        self.set_selection(Rectangle::from_coords(x, y, x, 1_000_000))?;
        self.erase_selection()
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
    pub fn push_reverse_undo(&mut self, description: impl Into<String>, op: Box<dyn UndoOperation>, operation_type: OperationType) -> EngineResult<()> {
        self.push_undo_action(Box::new(ReversedUndo::new(description.into(), op, operation_type)))
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
    pub fn undo_caret_position(&mut self) -> EngineResult<()> {
        let op = ReverseCaretPosition::new(self.caret.position());
        self.redo_stack.clear();
        self.undo_stack.lock().unwrap().push(Box::new(op));
        Ok(())
    }

    pub fn switch_to_palette(&mut self, pal: Palette) -> EngineResult<()> {
        let op = super::undo_operations::SwitchPalettte::new(pal);
        self.push_undo_action(Box::new(op))
    }

    pub fn update_sauce_data(&mut self, sauce: icy_sauce::MetaData) -> EngineResult<()> {
        let op = super::undo_operations::SetSauceData::new(sauce, self.buffer.get_sauce_meta().clone());
        self.push_undo_action(Box::new(op))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Size, TextPane,
        editor::{EditState, UndoState},
    };

    #[test]
    fn test_resize_buffer() {
        let mut state = EditState::default();
        assert_eq!(Size::new(80, 25), state.buffer.get_size());
        state.resize_buffer(false, Size::new(10, 10)).unwrap();
        assert_eq!(Size::new(10, 10), state.buffer.get_size());
        state.undo().unwrap();
        assert_eq!(Size::new(80, 25), state.buffer.get_size());
        state.redo().unwrap();
        assert_eq!(Size::new(10, 10), state.buffer.get_size());
    }
}
