//! Font and palette files have no screen of their own; they are rendered into a text buffer
//! (glyph sheet, sample text, colour swatches) so the viewer and the thumbnails can show them.

use icy_engine::{formats::FileFormat, AttributeColor, AttributedChar, BitFont, Position, TextAttribute, TextBuffer, TextPane};

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

/// Settings of the interactive TheDraw/FIGlet sample (text, spacing, colours, width guide).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontSampleOptions {
    /// Sample text, one rendered row per line; empty shows the alphabet and digits.
    pub text: String,
    /// Columns added between letters (negative values overlap them).
    pub spacing: i32,
    /// Rows added between text lines (negative values overlap them).
    pub line_gap: i32,
    /// TheDraw outline style (0-18) used for outline fonts.
    pub outline_style: usize,
    /// Colours for outline, block and FIGlet fonts; colour fonts keep their own.
    pub foreground: Option<u8>,
    pub background: Option<u8>,
    /// Draws a guide after this many columns (0 = off).
    pub ruler: i32,
}

impl Default for FontSampleOptions {
    fn default() -> Self {
        Self {
            text: String::new(),
            spacing: 0,
            line_gap: 0,
            outline_style: 0,
            foreground: None,
            background: None,
            ruler: 0,
        }
    }
}

pub const OUTLINE_STYLES: usize = 19;
const MAX_BUNDLE_FONTS: usize = 64;
const MAX_WIDTH: i32 = 1000;

/// One font of a TheDraw bundle or a FIGlet file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontInfo {
    pub name: String,
    pub kind: &'static str,
    pub glyphs: usize,
}

fn font_kind(font: &retrofont::Font) -> &'static str {
    match font {
        retrofont::Font::Figlet(_) => "FIGlet",
        retrofont::Font::Tdf(font) => match font.font_type() {
            retrofont::tdf::TdfFontType::Outline => "outline",
            retrofont::tdf::TdfFontType::Block => "block",
            retrofont::tdf::TdfFontType::Color => "color",
        },
    }
}

fn load_fonts(data: &[u8]) -> anyhow::Result<Vec<retrofont::Font>> {
    let fonts = retrofont::Font::load(data).map_err(|error| anyhow::anyhow!("{error}"))?;
    anyhow::ensure!(!fonts.is_empty(), "no fonts in file");
    Ok(fonts)
}

/// The fonts in a TheDraw bundle or FIGlet file, for a font picker.
pub fn font_list(data: &[u8]) -> Vec<FontInfo> {
    load_fonts(data)
        .map(|fonts| {
            fonts
                .iter()
                .map(|font| FontInfo {
                    name: font.name().to_string(),
                    kind: font_kind(font),
                    glyphs: match font {
                        retrofont::Font::Figlet(font) => font.glyph_count(),
                        retrofont::Font::Tdf(font) => font.glyph_count(),
                    },
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Draws glyph cells with the chosen colours for cells without their own attribute. Colours
/// are full palette indices, so bright backgrounds stay bright instead of blinking.
struct SampleTarget<'a> {
    buffer: &'a mut TextBuffer,
    left: i32,
    x: i32,
    y: i32,
    foreground: Option<u8>,
    background: Option<u8>,
}

impl retrofont::FontTarget for SampleTarget<'_> {
    type Error = std::fmt::Error;

    fn draw(&mut self, cell: retrofont::Cell) -> Result<(), Self::Error> {
        let position = Position::new(self.x, self.y);
        if self.x >= 0 && self.x < self.buffer.width() && self.y >= 0 && self.y < self.buffer.height() {
            let foreground = cell.fg.or(self.foreground).unwrap_or(15);
            let background = cell.bg.or(self.background).unwrap_or(0);
            let mut attribute = TextAttribute::from_colors(AttributeColor::Palette(foreground & 15), AttributeColor::Palette(background & 15));
            attribute.set_is_blinking(cell.blink);
            let ch = self.buffer.buffer_type.convert_from_unicode(cell.ch);
            self.buffer.layers[0].set_char(position, AttributedChar::new(ch, attribute));
        }
        self.x += 1;
        Ok(())
    }

    fn skip(&mut self) -> Result<(), Self::Error> {
        self.x += 1;
        Ok(())
    }

    fn next_line(&mut self) -> Result<(), Self::Error> {
        self.x = self.left;
        self.y += 1;
        Ok(())
    }
}

fn find_char(font: &retrofont::Font, ch: char) -> Option<char> {
    [ch, ch.to_ascii_uppercase(), ch.to_ascii_lowercase()].into_iter().find(|ch| font.has_char(*ch))
}

fn advance(font: &retrofont::Font, ch: char, spacing: i32) -> i32 {
    let width = match find_char(font, ch) {
        Some(ch) => font.glyph_size(ch).map_or(0, |(width, _)| width),
        None if ch == ' ' => font.spacing().unwrap_or(1),
        None => 0,
    } as i32;
    if width == 0 {
        0
    } else {
        (width + 1 + spacing).max(1)
    }
}

fn line_width(font: &retrofont::Font, line: &str, spacing: i32) -> i32 {
    line.chars().map(|ch| advance(font, ch, spacing)).sum()
}

fn line_advance(font: &retrofont::Font, options: &FontSampleOptions) -> i32 {
    (font.max_height() as i32 + 1 + options.line_gap).max(1)
}

fn sample_lines(font: &retrofont::Font, options: &FontSampleOptions) -> Vec<String> {
    if options.text.trim().is_empty() {
        ["ABCDEFGHI", "JKLMNOPQR", "STUVWXYZ", "0123456789"]
            .iter()
            .map(|line| line.chars().filter(|ch| find_char(font, *ch).is_some()).collect::<String>())
            .filter(|line| !line.is_empty())
            .collect()
    } else {
        options.text.lines().map(str::to_string).collect()
    }
}

fn draw_text(buffer: &mut TextBuffer, font: &retrofont::Font, x: i32, y: i32, line: &str, options: &FontSampleOptions) {
    let render_options = retrofont::RenderOptions {
        outline_style: options.outline_style.min(OUTLINE_STYLES - 1),
        ..Default::default()
    };
    let mut x = x;
    for ch in line.chars() {
        let step = advance(font, ch, options.spacing);
        if step == 0 {
            continue;
        }
        let mut target = SampleTarget {
            buffer: &mut *buffer,
            left: x,
            x,
            y,
            foreground: options.foreground,
            background: options.background,
        };
        let _ = font.render_glyph(&mut target, find_char(font, ch).unwrap_or(ch), &render_options);
        x += step;
    }
}

fn glyphs(font: &retrofont::Font) -> Vec<char> {
    let mut glyphs: Vec<char> = match font {
        retrofont::Font::Figlet(font) => font.iter_glyphs().map(|(ch, _)| ch).collect(),
        retrofont::Font::Tdf(font) => font.iter_glyphs().map(|(ch, _)| ch).collect(),
    };
    glyphs.retain(|ch| *ch != ' ');
    glyphs.sort_unstable();
    glyphs
}

/// A dim guide in the blank cells right after `columns`, so the art that crosses it stays visible.
fn draw_ruler(buffer: &mut TextBuffer, columns: i32, top: i32, bottom: i32) {
    if columns <= 0 || columns >= buffer.width() {
        return;
    }
    for y in top..bottom.min(buffer.height()) {
        let position = Position::new(columns, y);
        let cell = buffer.char_at(position);
        if cell.ch == ' ' && cell.attribute.background_color() == AttributeColor::Palette(0) {
            let ch = buffer.buffer_type.convert_from_unicode('│');
            buffer.layers[0].set_char(position, AttributedChar::new(ch, TextAttribute::from_color(DIM, 0)));
        }
    }
    write(buffer, columns + 1, top, &columns.to_string(), DIM);
}

/// TheDraw and FIGlet files: the sample of every font in the bundle with its name.
fn character_fonts(data: &[u8]) -> anyhow::Result<TextBuffer> {
    render_font_sample(data, None, &FontSampleOptions::default())
}

/// Renders the sample of one font (with its glyph table) or of every font in the file.
pub fn render_font_sample(data: &[u8], selected: Option<usize>, options: &FontSampleOptions) -> anyhow::Result<TextBuffer> {
    let fonts = load_fonts(data)?;
    match selected {
        Some(index) => single_font(fonts.get(index).ok_or_else(|| anyhow::anyhow!("font {index} not found"))?, options),
        None => all_fonts(&fonts, options),
    }
}

fn header(buffer: &mut TextBuffer, y: i32, font: &retrofont::Font) {
    write(buffer, 0, y, font.name(), TITLE);
    write(buffer, 2 + font.name().chars().count() as i32, y, font_kind(font), DIM);
}

fn buffer_width(content: i32, options: &FontSampleOptions) -> i32 {
    let ruler = if options.ruler > 0 {
        options.ruler + 1 + options.ruler.to_string().len() as i32
    } else {
        0
    };
    content.max(ruler).clamp(80, MAX_WIDTH)
}

fn all_fonts(fonts: &[retrofont::Font], options: &FontSampleOptions) -> anyhow::Result<TextBuffer> {
    let fonts = &fonts[..fonts.len().min(MAX_BUNDLE_FONTS)];
    let mut width = 0;
    let mut height = 0;
    for font in fonts {
        let lines = sample_lines(font, options);
        width = width.max(lines.iter().map(|line| line_width(font, line, options.spacing)).max().unwrap_or(0));
        height += 2 + lines.len() as i32 * line_advance(font, options);
    }
    let mut buffer = TextBuffer::new((buffer_width(width, options), height.max(1)));
    let mut y = 0;
    for font in fonts {
        header(&mut buffer, y, font);
        y += 2;
        for line in sample_lines(font, options) {
            draw_text(&mut buffer, font, 0, y, &line, options);
            y += line_advance(font, options);
        }
    }
    draw_ruler(&mut buffer, options.ruler, 0, height);
    Ok(buffer)
}

fn single_font(font: &retrofont::Font, options: &FontSampleOptions) -> anyhow::Result<TextBuffer> {
    let lines = sample_lines(font, options);
    let sample_width = lines.iter().map(|line| line_width(font, line, options.spacing)).max().unwrap_or(0);
    let glyphs = glyphs(font);
    let width = buffer_width(sample_width, options);
    let glyph_row = font.max_height() as i32 + 2;
    let mut rows = Vec::<Vec<char>>::new();
    let mut row_width = 0;
    for ch in &glyphs {
        let step = advance(font, *ch, 1).max(2);
        if rows.is_empty() || row_width + step > width {
            rows.push(Vec::new());
            row_width = 0;
        }
        rows.last_mut().unwrap().push(*ch);
        row_width += step;
    }
    let sample_height = lines.len() as i32 * line_advance(font, options);
    let table_top = 2 + sample_height + 1;
    let height = table_top + 2 + rows.len() as i32 * glyph_row;
    let mut buffer = TextBuffer::new((width, height.max(1)));
    header(&mut buffer, 0, font);
    let mut y = 2;
    for line in &lines {
        draw_text(&mut buffer, font, 0, y, line, options);
        y += line_advance(font, options);
    }
    draw_ruler(&mut buffer, options.ruler, 2, 2 + sample_height);
    write(&mut buffer, 0, table_top, &format!("{} glyphs", glyphs.len()), TITLE);
    let glyph_options = FontSampleOptions { spacing: 1, ..options.clone() };
    let mut y = table_top + 2;
    for row in rows {
        let mut x = 0;
        for ch in row {
            write(&mut buffer, x, y, &ch.to_string(), DIM);
            draw_text(&mut buffer, font, x, y + 1, &ch.to_string(), &glyph_options);
            x += advance(font, ch, 1).max(2);
        }
        y += glyph_row;
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

    const ZETRAX: &[u8] = include_bytes!("items/sixteencolors/ZETRAX.TDF");

    fn row_text(buffer: &TextBuffer, y: i32) -> String {
        (0..buffer.width()).map(|x| buffer.char_at(Position::new(x, y)).ch).collect()
    }

    fn inked_columns(buffer: &TextBuffer, rows: std::ops::Range<i32>) -> i32 {
        (0..buffer.width())
            .filter(|x| rows.clone().any(|y| buffer.char_at(Position::new(*x, y)).ch != ' '))
            .max()
            .map_or(0, |x| x + 1)
    }

    #[test]
    fn font_list_names_every_font_of_the_bundle() {
        let fonts = font_list(ZETRAX);
        assert!(!fonts.is_empty());
        assert!(fonts.iter().all(|font| !font.name.is_empty() && font.glyphs > 0));
        assert!(font_list(b"garbage").is_empty());
    }

    #[test]
    fn single_font_renders_sample_text_and_glyph_table() {
        let options = FontSampleOptions {
            text: "AB".into(),
            ..Default::default()
        };
        let buffer = render_font_sample(ZETRAX, Some(0), &options).unwrap();
        let fonts = font_list(ZETRAX);
        assert!(row_text(&buffer, 0).starts_with(&fonts[0].name));
        let font = &load_fonts(ZETRAX).unwrap()[0];
        let sample = 2..2 + font.max_height() as i32;
        assert!(inked_columns(&buffer, sample.clone()) > 0, "sample text not drawn");
        let table = (0..buffer.height())
            .find(|y| row_text(&buffer, *y).contains("glyphs"))
            .expect("glyph table header");
        assert!(table > sample.end);
        assert!(buffer.height() > table + 2 + font.max_height() as i32);
    }

    #[test]
    fn spacing_widens_the_sample() {
        let font = &load_fonts(ZETRAX).unwrap()[0];
        let rows = 2..2 + font.max_height() as i32;
        let render = |spacing| {
            let options = FontSampleOptions {
                text: "ABC".into(),
                spacing,
                ..Default::default()
            };
            inked_columns(&render_font_sample(ZETRAX, Some(0), &options).unwrap(), rows.clone())
        };
        assert_eq!(render(3), render(0) + 6);
    }

    #[test]
    fn multi_line_text_and_line_gap_move_the_rows() {
        let height = |line_gap| {
            let options = FontSampleOptions {
                text: "A\nB".into(),
                line_gap,
                ..Default::default()
            };
            render_font_sample(ZETRAX, None, &options).unwrap().height()
        };
        let fonts = load_fonts(ZETRAX).unwrap().len().min(MAX_BUNDLE_FONTS) as i32;
        assert_eq!(
            height(0),
            fonts * 2
                + load_fonts(ZETRAX)
                    .unwrap()
                    .iter()
                    .take(MAX_BUNDLE_FONTS)
                    .map(|font| 2 * (font.max_height() as i32 + 1))
                    .sum::<i32>()
        );
        assert_eq!(height(2) - height(0), fonts * 2 * 2);
    }

    #[test]
    fn colors_apply_to_cells_without_their_own_attribute() {
        let mut data = b"flf2a$ 2 1 4 0 0\n".to_vec();
        for _ in 0..95 + 7 {
            data.extend_from_slice(b"Y@\nY@@\n");
        }
        let options = FontSampleOptions {
            text: "!".into(),
            foreground: Some(12),
            background: Some(1),
            ..Default::default()
        };
        let buffer = render_font_sample(&data, Some(0), &options).unwrap();
        let cell = buffer.char_at(Position::new(0, 2));
        assert_eq!(cell.ch, 'Y');
        assert_eq!(cell.attribute.foreground_color(), AttributeColor::Palette(12));
        assert_eq!(cell.attribute.background_color(), AttributeColor::Palette(1));
        assert!(!cell.attribute.is_blinking());
    }

    #[test]
    fn outline_fonts_use_the_style_and_colors() {
        use retrofont::{
            tdf::{TdfFont, TdfFontType},
            Glyph, GlyphPart,
        };
        let mut font = TdfFont::new("Line", TdfFontType::Outline, 1);
        let mut glyph = Glyph::new(2, 1);
        glyph.parts = vec![GlyphPart::OutlinePlaceholder(b'A'), GlyphPart::OutlinePlaceholder(b'B')];
        font.add_glyph('A', glyph);
        let data = TdfFont::serialize_bundle(&[font]).unwrap();
        assert_eq!(font_list(&data)[0].kind, "outline");
        let cell = |outline_style| {
            let options = FontSampleOptions {
                text: "A".into(),
                outline_style,
                foreground: Some(11),
                background: Some(9),
                ..Default::default()
            };
            render_font_sample(&data, Some(0), &options).unwrap().char_at(Position::new(0, 2))
        };
        let (plain, double) = (cell(0), cell(1));
        assert_ne!(plain.ch, double.ch, "outline style changes the line characters");
        assert_eq!(plain.attribute.foreground_color(), AttributeColor::Palette(11));
        assert_eq!(plain.attribute.background_color(), AttributeColor::Palette(9), "bright backgrounds stay bright");
    }

    #[test]
    fn ruler_marks_the_column_in_blank_cells() {
        let options = FontSampleOptions {
            text: "A".into(),
            ruler: 40,
            ..Default::default()
        };
        let buffer = render_font_sample(ZETRAX, Some(0), &options).unwrap();
        let guide = buffer.buffer_type.convert_from_unicode('│');
        assert_eq!(buffer.char_at(Position::new(40, 3)).ch, guide);
        assert_eq!(buffer.char_at(Position::new(41, 2)).ch, '4');
        let wide = FontSampleOptions { ruler: 132, ..options };
        assert!(render_font_sample(ZETRAX, Some(0), &wide).unwrap().width() > 132);
    }

    #[test]
    fn invalid_character_font_is_an_error() {
        assert!(render(FileFormat::CharacterFont(CharacterFontFormat::Tdf), "bad.tdf", b"garbage").is_err());
    }
}
