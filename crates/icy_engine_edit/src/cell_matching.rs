use icy_engine::{attribute, AttributeColor, AttributedChar, BufferType, TextBuffer, XTERM_256_PALETTE};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellMatchMode {
    Character,
    Attribute,
    Foreground,
    Background,
    Appearance,
}

/// Literal matching keeps stored colors and attributes distinct. Appearance matching
/// additionally equates cells that display the same solid color, using the document font.
pub struct CellMatcher<'a> {
    buffer: &'a TextBuffer,
    sample: AttributedChar,
    mode: CellMatchMode,
    solid: Option<(u8, u8, u8)>,
}

impl<'a> CellMatcher<'a> {
    pub fn new(buffer: &'a TextBuffer, sample: AttributedChar, mode: CellMatchMode) -> Self {
        let solid = (mode == CellMatchMode::Appearance).then(|| solid_color(buffer, sample)).flatten();
        Self { buffer, sample, mode, solid }
    }

    pub fn matches(&self, cell: AttributedChar) -> bool {
        match self.mode {
            CellMatchMode::Character => cell.ch == self.sample.ch,
            CellMatchMode::Attribute => cell.attribute == self.sample.attribute,
            CellMatchMode::Foreground => cell.attribute.foreground_color() == self.sample.attribute.foreground_color(),
            CellMatchMode::Background => cell.attribute.background_color() == self.sample.attribute.background_color(),
            CellMatchMode::Appearance => match (self.solid, solid_color(self.buffer, cell)) {
                (Some(a), Some(b)) => a == b,
                (None, None) => cell == self.sample,
                _ => false,
            },
        }
    }
}

fn color_rgb(buffer: &TextBuffer, color: AttributeColor) -> Option<(u8, u8, u8)> {
    match color {
        AttributeColor::Palette(index) if usize::from(index) < buffer.palette.len() => Some(buffer.palette.rgb(u32::from(index))),
        AttributeColor::ExtendedPalette(index) => {
            let palette_index = icy_engine::ansi_to_internal_palette_index(u32::from(index));
            Some(if (palette_index as usize) < buffer.palette.len() {
                buffer.palette.rgb(palette_index)
            } else {
                XTERM_256_PALETTE[usize::from(index)].1.rgb()
            })
        }
        AttributeColor::Rgb(r, g, b) => Some((r, g, b)),
        AttributeColor::Transparent | AttributeColor::Palette(_) => None,
    }
}

fn solid_color(buffer: &TextBuffer, cell: AttributedChar) -> Option<(u8, u8, u8)> {
    // Styled and Unicode cells can depend on neighboring cells or non-bitmap glyphs.
    // Keep their exact matching rather than claiming they are visually interchangeable.
    if !cell.is_visible() || buffer.buffer_type == BufferType::Unicode || cell.attribute.attr & !(attribute::BOLD | attribute::BLINK) != 0 {
        return None;
    }
    let mut foreground = cell.attribute.foreground_color();
    if let AttributeColor::Palette(index @ 0..=7) = foreground {
        if cell.attribute.is_bold() {
            foreground = AttributeColor::Palette(index + 8);
        }
    }
    let fg = color_rgb(buffer, foreground);
    let bg = color_rgb(buffer, cell.attribute.background_color());
    if fg.is_some() && fg == bg {
        return fg;
    }
    let font = buffer.font_for_render(cell.font_page()).or_else(|| buffer.font_for_render(0))?;
    if !(1..=8).contains(&font.width) || !(1..=32).contains(&font.height) || font.size() != buffer.font_for_render(0)?.size() {
        return None;
    }
    let glyph = font.glyphs.get(cell.ch as usize)?;
    if glyph.width != font.width || glyph.height != font.height {
        return None;
    }
    let mask = u8::MAX << (8 - font.width);
    let empty = (0..usize::from(font.height)).all(|y| glyph.get_row(y) & mask == 0);
    if empty {
        return bg;
    }
    // Blink must match in both phases, and a ninth background column breaks a solid glyph.
    let extra_background = buffer.use_letter_spacing() && font.width == 8 && !(192..=223).contains(&(cell.ch as u32));
    if !cell.attribute.is_blinking() && !extra_background && (0..usize::from(font.height)).all(|y| glyph.get_row(y) & mask == mask) {
        return fg;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use icy_engine::{BitFont, CompactGlyph, TextAttribute};

    fn cell(code: u8, fg: u32, bg: u32) -> AttributedChar {
        AttributedChar::new(char::from(code), TextAttribute::new(fg, bg))
    }

    #[test]
    fn appearance_matches_solid_cells_without_changing_literal_modes() {
        let buffer = TextBuffer::new((8, 1));
        let sample = cell(32, 7, 0);
        let matcher = CellMatcher::new(&buffer, sample, CellMatchMode::Appearance);
        for candidate in [cell(0, 4, 0), cell(255, 2, 0), cell(219, 0, 5), cell(b'A', 0, 0), sample] {
            assert!(matcher.matches(candidate), "{candidate:?}");
        }
        for candidate in [cell(32, 7, 1), cell(219, 1, 0), cell(b'A', 7, 0), AttributedChar::invisible()] {
            assert!(!matcher.matches(candidate), "{candidate:?}");
        }
        assert!(!CellMatcher::new(&buffer, sample, CellMatchMode::Character).matches(cell(219, 0, 5)));
        assert!(!CellMatcher::new(&buffer, sample, CellMatchMode::Attribute).matches(cell(32, 2, 0)));
        assert!(CellMatcher::new(&buffer, sample, CellMatchMode::Foreground).matches(cell(b'X', 7, 4)));
        assert!(CellMatcher::new(&buffer, sample, CellMatchMode::Background).matches(cell(b'X', 2, 0)));
        let text = cell(b'A', 7, 0);
        assert!(CellMatcher::new(&buffer, text, CellMatchMode::Appearance).matches(text));
        assert!(!CellMatcher::new(&buffer, text, CellMatchMode::Appearance).matches(cell(b'B', 7, 0)));
    }

    #[test]
    fn appearance_uses_font_slots_glyph_width_and_letter_spacing() {
        let mut buffer = TextBuffer::new((8, 1));
        let mut font = BitFont::default();
        font.glyphs[219] = CompactGlyph::from_rows(8, 16, &[0x80; 16]);
        font.glyphs[b'X' as usize] = CompactGlyph::from_rows(8, 16, &[0xff; 16]);
        buffer.set_font(1, font);
        let mut block = cell(219, 0, 5);
        block.set_font_page(1);
        let mut full = cell(b'X', 0, 5);
        full.set_font_page(1);
        let black = cell(32, 7, 0);
        let matcher = CellMatcher::new(&buffer, black, CellMatchMode::Appearance);
        assert!(!matcher.matches(block), "CP437 block code is not solid in this font");
        assert!(matcher.matches(full));
        buffer.set_use_letter_spacing(true);
        let matcher = CellMatcher::new(&buffer, black, CellMatchMode::Appearance);
        assert!(!matcher.matches(full), "X gains a background-colored ninth column");
        assert!(matcher.matches(cell(219, 0, 5)), "CP437 full block repeats its edge");

        let mut narrow = BitFont::default();
        narrow.width = 6;
        narrow.glyphs[b'X' as usize] = CompactGlyph::from_rows(6, 16, &[0xfc; 16]);
        narrow.glyphs[32] = CompactGlyph::new(6, 16);
        buffer.set_font(0, narrow.clone());
        buffer.set_font(1, narrow);
        assert!(CellMatcher::new(&buffer, black, CellMatchMode::Appearance).matches(full));
    }

    #[test]
    fn appearance_resolves_colors_bold_blink_and_transparency() {
        let mut buffer = TextBuffer::new((8, 1));
        buffer.palette.set_color_rgb(3, 0, 0, 0);
        let black = cell(32, 7, 0);
        let matcher = CellMatcher::new(&buffer, black, CellMatchMode::Appearance);
        let rgb = AttributedChar::new(' ', TextAttribute::from_colors(AttributeColor::Transparent, AttributeColor::Rgb(0, 0, 0)));
        assert!(matcher.matches(rgb));
        assert!(matcher.matches(cell(219, 3, 7)));
        let extended = AttributedChar::new(' ', TextAttribute::from_colors(AttributeColor::Palette(7), AttributeColor::ExtendedPalette(6)));
        assert!(matcher.matches(extended));
        let mut transparent = cell(32, 0, 0);
        transparent.attribute.set_background_transparent();
        assert!(!matcher.matches(transparent));
        let mut bold = cell(219, 0, 5);
        bold.attribute.set_is_bold(true);
        assert!(!matcher.matches(bold));
        let mut blink = cell(219, 0, 5);
        blink.attribute.set_is_blinking(true);
        assert!(!matcher.matches(blink));
        blink.attribute.set_background(0);
        assert!(matcher.matches(blink));
        let mut styled = black;
        styled.attribute.set_is_underlined(true);
        assert!(!matcher.matches(styled));
    }
}
