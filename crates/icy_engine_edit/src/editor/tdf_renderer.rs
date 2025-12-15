//! TDF Font rendering with EditState integration
//!
//! Provides a `FontTarget` implementation that renders TDF/Figlet fonts
//! through the EditState, supporting full undo/redo functionality.

use crate::{AttributedChar, Position, Result, TextAttribute, TextPane};

use super::EditState;
use retrofont::{Cell, FontTarget};

/// A renderer that writes TDF/Figlet font glyphs to an EditState with undo support.
///
/// This implements `FontTarget` so it can be used with retrofont's `Font::render_glyph()`.
/// All character writes go through `EditState::set_char_in_atomic()` for proper undo tracking.
///
/// # Example
/// ```ignore
/// let _undo = edit_state.begin_atomic_undo("Render character");
/// let mut renderer = TdfEditStateRenderer::new(&mut edit_state, start_x, start_y)?;
/// font.render_glyph(&mut renderer, 'A', &options)?;
/// let end_pos = renderer.position();
/// ```
pub struct TdfEditStateRenderer<'a> {
    edit_state: &'a mut EditState,
    cur_x: i32,
    cur_y: i32,
    start_x: i32,
    start_y: i32,
    max_x: i32,
    buffer_type: crate::BufferType,
}

impl<'a> TdfEditStateRenderer<'a> {
    /// Create a new renderer starting at the given position.
    ///
    /// The renderer will use the current layer from the EditState.
    pub fn new(edit_state: &'a mut EditState, start_x: i32, start_y: i32) -> Result<Self> {
        let _layer_idx = edit_state.get_current_layer()?;
        let buffer_type = edit_state.get_buffer().buffer_type;
        Ok(Self {
            edit_state,
            cur_x: start_x,
            cur_y: start_y,
            start_x,
            start_y,
            max_x: start_x,
            buffer_type,
        })
    }

    /// Get the current cursor position
    pub fn position(&self) -> Position {
        Position::new(self.cur_x, self.cur_y)
    }

    /// Get the current X position
    pub fn x(&self) -> i32 {
        self.cur_x
    }

    /// Get the current Y position
    pub fn y(&self) -> i32 {
        self.cur_y
    }

    /// Advance to the next character position (for multi-char rendering)
    /// Resets Y to start and advances X to current position
    pub fn next_char(&mut self) {
        self.start_x = self.max_x;
        self.cur_x = self.max_x;
        self.cur_y = self.start_y;
    }

    /// Get the maximum X position reached during rendering
    pub fn max_x(&self) -> i32 {
        self.max_x
    }
}

impl FontTarget for TdfEditStateRenderer<'_> {
    type Error = crate::EngineError;

    fn draw(&mut self, cell: Cell) -> std::result::Result<(), Self::Error> {
        // Get buffer dimensions
        let (width, height) = {
            let buffer = self.edit_state.get_buffer();
            (buffer.width(), buffer.height())
        };

        // Only draw if within bounds
        if self.cur_x >= 0 && self.cur_x < width && self.cur_y >= 0 && self.cur_y < height {
            let fg = cell.fg.unwrap_or(15);
            let bg = cell.bg.unwrap_or(0);
            let attr = TextAttribute::from_color(fg, bg);

            // Convert unicode to buffer type
            let ch = self.buffer_type.convert_from_unicode(cell.ch);
            let attributed_char = AttributedChar::new(ch, attr);

            // Use set_char_in_atomic since caller manages the atomic undo guard
            self.edit_state.set_char_in_atomic(Position::new(self.cur_x, self.cur_y), attributed_char)?;
        }

        self.cur_x += 1;
        // Track the maximum X position reached
        if self.cur_x > self.max_x {
            self.max_x = self.cur_x;
        }
        Ok(())
    }

    fn next_line(&mut self) -> std::result::Result<(), Self::Error> {
        self.cur_y += 1;
        self.cur_x = self.start_x;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TextBuffer;

    #[test]
    fn test_tdf_renderer_basic() {
        let buffer = TextBuffer::create((80, 25));
        let mut edit_state = EditState::from_buffer(buffer);

        let _undo = edit_state.begin_atomic_undo("test");
        let result = TdfEditStateRenderer::new(&mut edit_state, 0, 0);
        assert!(result.is_ok());

        let renderer = result.unwrap();
        assert_eq!(renderer.x(), 0);
        assert_eq!(renderer.y(), 0);
    }
}
