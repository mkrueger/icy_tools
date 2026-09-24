//! Font and palette files have no screen of their own; they are rendered into a text buffer
//! (glyph sheet, sample text, colour swatches) so the viewer and the thumbnails can show them.

use icy_engine::{char_set::TdfBufferRenderer, formats::FileFormat, AttributeColor, AttributedChar, BitFont, Position, TextAttribute, TextBuffer, TextPane};

const TITLE: u8 = 14;
const TEXT: u8 = 7;
const DIM: u8 = 8;

pub fn is_previewable(format: FileFormat) -> bool {
    matches!(format, FileFormat::BitFont(_) | FileFormat::CharacterFont(_) | FileFormat::Palette(_))
}

pub fn render(format: FileFormat, name: &str, data: &[u8]) -> anyhow::Result<TextBuffer> {
    match format {
        FileFormat::BitFont(_) => bit_font(name, data),
        FileFormat::CharacterFont(_) => character_fonts(data),
        FileFormat::Palette(_) => palette(format, name, data),
        _ => anyhow::bail!("{} has no font or palette preview", format.name()),
    }
}

fn write(buffer: &mut TextBuffer, x: i32, y: i32, text: &str, color: u8) {
    let attribute = TextAttribute::from_color(color, 0);
    for (offset, ch) in text.chars().enumerate() {
        let position = Position::new(x + offset as i32, y);
        if position.x < buffer.width() {
            let ch = buffer.buffer_type.convert_from_unicode(ch);
            buffer.layers[0].set_char(position, AttributedChar::new(ch, attribute));
        }
    }
}

/// All 256 glyphs as a 16×16 sheet followed by sample text, drawn in the font itself.
fn bit_font(name: &str, data: &[u8]) -> anyhow::Result<TextBuffer> {
    let font = BitFont::from_bytes(name, data)?;
    let size = font.size();
    let samples = [
        "ABCDEFGHIJKLMNOPQRSTUVWXYZ",
        "abcdefghijklmnopqrstuvwxyz",
        "0123456789 !?.,:;+-*/=()[]{}<>",
        "░▒▓█ ▀▄▌▐ ╔═╦╗ ┌─┬┐ ♥♦♣♠ ☺☻",
    ];
    let width = 48;
    let height = 2 + 16 + 1 + samples.len() as i32 + 1;
    let mut buffer = TextBuffer::new((width, height));
    buffer.set_font_dimensions(size);
    buffer.set_font(0, font);
    write(&mut buffer, 1, 0, &format!("{name}  {}×{}", size.width, size.height), TITLE);
    let left = (width - 32) / 2;
    for code in 0..256u32 {
        let (column, row) = ((code % 16) as i32, (code / 16) as i32);
        let color = if (column + row) % 2 == 0 { 15 } else { TEXT };
        let ch = char::from_u32(code).unwrap_or(' ');
        buffer.layers[0].set_char(
            Position::new(left + column * 2, 2 + row),
            AttributedChar::new(ch, TextAttribute::from_color(color, 0)),
        );
    }
    for (index, sample) in samples.iter().enumerate() {
        let x = ((width - sample.chars().count() as i32) / 2).max(0);
        write(&mut buffer, x, 19 + index as i32, sample, if index == 3 { DIM + 3 } else { TEXT });
    }
    Ok(buffer)
}

/// TheDraw and FIGlet files: every font of the bundle with its name and sample lines.
fn character_fonts(data: &[u8]) -> anyhow::Result<TextBuffer> {
    let fonts = retrofont::Font::load(data).map_err(|error| anyhow::anyhow!("{error}"))?;
    anyhow::ensure!(!fonts.is_empty(), "no fonts in file");
    let options = retrofont::RenderOptions::default();
    let lines = |font: &retrofont::Font| -> Vec<String> {
        ["ABCDEFGHI", "JKLMNOPQR", "STUVWXYZ", "0123456789"]
            .iter()
            .map(|line| {
                line.chars()
                    .filter(|ch| font.has_char(*ch) || font.has_char(ch.to_ascii_lowercase()))
                    .collect::<String>()
            })
            .filter(|line| !line.is_empty())
            .collect()
    };
    let glyph_width = |font: &retrofont::Font, ch: char| {
        font.glyph_size(ch)
            .or_else(|| font.glyph_size(ch.to_ascii_lowercase()))
            .map_or(0, |(width, _)| width) as i32
            + 1
    };
    let mut width = 80;
    let mut height = 0;
    let fonts: Vec<_> = fonts.into_iter().take(64).collect();
    for font in &fonts {
        height += 2;
        for line in lines(font) {
            width = width.max(2 + line.chars().map(|ch| glyph_width(font, ch)).sum::<i32>());
            height += font.max_height() as i32 + 1;
        }
    }
    let width = width.min(240);
    let mut buffer = TextBuffer::new((width, height.max(1)));
    let mut y = 0;
    for font in &fonts {
        let kind = match font {
            retrofont::Font::Figlet(_) => "FIGlet",
            retrofont::Font::Tdf(_) => "TheDraw",
        };
        write(&mut buffer, 1, y, font.name(), TITLE);
        write(&mut buffer, 3 + font.name().chars().count() as i32, y, kind, DIM);
        y += 2;
        for line in lines(font) {
            let mut x = 1;
            for ch in line.chars() {
                let mut renderer = TdfBufferRenderer::new(&mut buffer, x, y);
                let _ = font.render_glyph(&mut renderer, ch, &options);
                x += glyph_width(font, ch);
            }
            y += font.max_height() as i32 + 1;
        }
    }
    Ok(buffer)
}

/// Colour swatches with their hex values, eight per row.
fn palette(format: FileFormat, name: &str, data: &[u8]) -> anyhow::Result<TextBuffer> {
    let palette = format.load_palette(data)?;
    anyhow::ensure!(!palette.is_empty(), "palette has no colors");
    let colors: Vec<_> = palette.color_iter().map(|color| color.rgb()).collect();
    let (columns, cell) = (8, 8);
    let band = 5;
    let header = if palette.author.trim().is_empty() { 2 } else { 3 };
    let rows = colors.len().div_ceil(columns) as i32;
    let mut buffer = TextBuffer::new((columns as i32 * cell + 1, header + rows * band));
    let title = if palette.title.trim().is_empty() { name } else { palette.title.trim() };
    write(&mut buffer, 1, 0, &format!("{title}  ({} colors)", colors.len()), TITLE);
    if header == 3 {
        write(&mut buffer, 1, 1, palette.author.trim(), TEXT);
    }
    for (index, (r, g, b)) in colors.iter().enumerate() {
        let x = 1 + (index % columns) as i32 * cell;
        let y = header + (index / columns) as i32 * band;
        let mut attribute = TextAttribute::default();
        attribute.set_background_color(AttributeColor::Rgb(*r, *g, *b));
        for row in 0..3 {
            for column in 0..cell - 2 {
                buffer.layers[0].set_char(Position::new(x + column, y + row), AttributedChar::new(' ', attribute));
            }
        }
        write(&mut buffer, x, y + 3, &format!("{r:02X}{g:02X}{b:02X}"), TEXT);
    }
    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use icy_engine::formats::{BitFontFormat, CharacterFontFormat, PaletteFormat};

    #[test]
    fn bit_font_sheet_contains_all_glyphs_in_the_font() {
        let data: Vec<u8> = (0..256 * 16).map(|index| (index % 251) as u8).collect();
        let buffer = render(FileFormat::BitFont(BitFontFormat::Raw(16)), "test.f16", &data).unwrap();
        assert_eq!(buffer.font_dimensions(), icy_engine::Size::new(8, 16));
        assert_eq!(buffer.font(0).unwrap().name(), "test.f16");
        assert_eq!(buffer.char_at(Position::new(8 + 2, 2)).ch, '\u{1}');
        assert_eq!(buffer.char_at(Position::new(8 + 30, 17)).ch, '\u{FF}');
    }

    #[test]
    fn palette_swatches_use_the_file_colors() {
        let buffer = render(FileFormat::Palette(PaletteFormat::Hex), "test.hex", b"FF0000\n00FF80\n").unwrap();
        let swatch = buffer.char_at(Position::new(9, 2)).attribute.background_color();
        assert_eq!(swatch, AttributeColor::Rgb(0x00, 0xFF, 0x80));
        let label: String = (1..7).map(|x| buffer.char_at(Position::new(x, 5)).ch).collect();
        assert_eq!(label, "FF0000");
    }

    #[test]
    fn character_font_bundle_renders_name_and_glyphs() {
        let buffer = render(
            FileFormat::CharacterFont(CharacterFontFormat::Tdf),
            "zetrax.tdf",
            include_bytes!("items/sixteencolors/ZETRAX.TDF"),
        )
        .unwrap();
        let name: String = (1..buffer.width()).map(|x| buffer.char_at(Position::new(x, 0)).ch).collect();
        assert!(!name.trim().is_empty());
        let drawn = (2..buffer.height()).any(|y| (0..buffer.width()).any(|x| buffer.char_at(Position::new(x, y)).ch != ' '));
        assert!(drawn, "no glyphs rendered");
    }

    #[test]
    fn invalid_character_font_is_an_error() {
        assert!(render(FileFormat::CharacterFont(CharacterFontFormat::Tdf), "bad.tdf", b"garbage").is_err());
    }
}
