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
    /// Colors for glyphs without their own (block, outline and Figlet fonts).
    caret_attribute: TextAttribute,
}

impl<'a> TdfEditStateRenderer<'a> {
    /// Create a new renderer starting at the given position.
    ///
    /// The renderer will use the current layer from the EditState.
    pub fn new(edit_state: &'a mut EditState, start_x: i32, start_y: i32) -> Result<Self> {
        let _layer_idx = edit_state.get_current_layer()?;
        let buffer_type = edit_state.get_buffer().buffer_type;
        let caret_attribute = edit_state.get_caret().attribute;
        Ok(Self {
            edit_state,
            cur_x: start_x,
            cur_y: start_y,
            start_x,
            start_y,
            max_x: start_x,
            buffer_type,
            caret_attribute,
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

    /// Paint the full footprint of a block/outline glyph before its nonempty cells are rendered.
    pub fn fill_background(&mut self, width: usize, height: usize) -> Result<()> {
        if let Ok(width) = i32::try_from(width) {
            if let Some(end) = self.start_x.checked_add(width) {
                self.max_x = self.max_x.max(end);
            }
        }
        let buffer = self.edit_state.get_buffer();
        let (buffer_width, buffer_height) = (buffer.width(), buffer.height());
        let background = AttributedChar::new(' ', self.caret_attribute);
        for y in 0..height {
            for x in 0..width {
                let (Ok(x), Ok(y)) = (i32::try_from(x), i32::try_from(y)) else {
                    continue;
                };
                let (Some(x), Some(y)) = (self.start_x.checked_add(x), self.start_y.checked_add(y)) else {
                    continue;
                };
                if x >= 0 && x < buffer_width && y >= 0 && y < buffer_height {
                    self.edit_state.set_char_in_atomic(Position::new(x, y), background)?;
                }
            }
        }
        Ok(())
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
            let attr = match (cell.fg, cell.bg) {
                (Some(fg), Some(bg)) => TextAttribute::from_color(fg, bg),
                (fg, bg) => {
                    let mut attr = self.caret_attribute;
                    if let Some(fg) = fg {
                        attr.set_foreground(fg as u32);
                    }
                    if let Some(bg) = bg {
                        attr.set_background(bg as u32);
                    }
                    attr
                }
            };

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

    fn skip(&mut self) -> std::result::Result<(), Self::Error> {
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

    #[test]
    fn colorless_glyphs_use_caret_colors_and_color_fonts_keep_their_own() {
        use retrofont::{tdf::TdfFont, tdf::TdfFontType, Font, Glyph, GlyphPart, RenderOptions};

        let render = |font_type, parts: Vec<GlyphPart>| {
            let mut glyph = Glyph::new(4, 2);
            glyph.parts = parts;
            let mut tdf = TdfFont::new("test", font_type, 0);
            tdf.add_glyph('A', glyph);
            let font = Font::Tdf(Box::new(tdf));

            let mut edit_state = EditState::from_buffer(TextBuffer::create((10, 3)));
            edit_state.set_caret_attribute(TextAttribute::from_color(4, 1));
            let _undo = edit_state.begin_atomic_undo("test");
            let mut renderer = TdfEditStateRenderer::new(&mut edit_state, 0, 0).unwrap();
            if matches!(font_type, TdfFontType::Block | TdfFontType::Outline) {
                renderer.fill_background(4, 2).unwrap();
            }
            font.render_glyph(&mut renderer, 'A', &RenderOptions::default()).unwrap();
            drop(_undo);
            (0..3)
                .flat_map(|y| (0..5).map(move |x| (x, y)))
                .map(|(x, y)| {
                    let ch = edit_state.get_buffer().char_at(Position::new(x, y));
                    (ch.ch, ch.attribute.foreground(), ch.attribute.background())
                })
                .collect::<Vec<_>>()
        };
        let empty = TextBuffer::create((1, 1)).char_at(Position::new(0, 0));
        let untouched = (empty.ch, empty.attribute.foreground(), empty.attribute.background());

        let block = render(
            TdfFontType::Block,
            vec![GlyphPart::Char('\u{2588}'), GlyphPart::HardBlank, GlyphPart::Skip, GlyphPart::Char('\u{2580}')],
        );
        assert_eq!(block[0], ('\u{DB}', 4, 1));
        assert_eq!(block[1], (' ', 4, 1), "hard blanks take the caret background");
        assert_eq!(block[2], (' ', 4, 1), "skipped cells take the caret background");
        assert_eq!(block[3], ('\u{DF}', 4, 1));
        assert_eq!(block[5], (' ', 4, 1), "unwritten glyph rows take the caret background");
        assert_eq!(block[4], untouched, "outside the glyph width is unchanged");
        assert_eq!(block[10], untouched, "outside the glyph height is unchanged");

        let outline = render(TdfFontType::Outline, vec![GlyphPart::OutlinePlaceholder(b'A'), GlyphPart::OutlineHole]);
        assert_eq!((outline[0].1, outline[0].2), (4, 1));
        assert_eq!(outline[1], (' ', 4, 1), "outline holes take the caret background");
        assert_eq!(outline[2], (' ', 4, 1), "unwritten outline cells take the caret background");

        let color = render(
            TdfFontType::Color,
            vec![GlyphPart::AnsiChar {
                ch: 'X',
                fg: 2,
                bg: 3,
                blink: false,
            }],
        );
        assert_eq!(color[0], ('X', 2, 3));
        assert_eq!(color[1], untouched, "color font transparency is preserved");
    }
}
