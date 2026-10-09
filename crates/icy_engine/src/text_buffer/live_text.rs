use std::io::{Cursor, Read};

use byteorder::{LittleEndian, ReadBytesExt};
use retrofont::{tdf::TdfFont, Cell, Font, FontTarget, RenderOptions};
use serde::{Deserialize, Serialize};

use crate::{AttributedChar, BufferType, EngineError, Layer, Position, Result, TextAttribute};

/// An embedded TheDraw font and its editable source. Layer cells are a render cache.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LiveText {
    pub text: String,
    pub font_data: Vec<u8>,
    pub attribute: TextAttribute,
    pub outline_style: u8,
    pub letter_spacing: u8,
    pub line_spacing: u8,
}

impl LiveText {
    pub fn new(font: &TdfFont, attribute: TextAttribute) -> Result<Self> {
        Ok(Self {
            text: String::new(),
            font_data: font.to_bytes().map_err(|error| EngineError::Generic(error.to_string()))?,
            attribute,
            outline_style: 0,
            letter_spacing: 0,
            line_spacing: 0,
        })
    }

    pub fn font(&self) -> Result<TdfFont> {
        if self.text.len() > 65_536 || self.font_data.len() > 1_048_576 || self.outline_style >= 19 || self.letter_spacing > 32 || self.line_spacing > 32 {
            return Err(EngineError::Generic("Live text exceeds its text, font, or style limits".into()));
        }
        let mut fonts = TdfFont::load(&self.font_data).map_err(|error| EngineError::Generic(error.to_string()))?;
        if fonts.len() != 1 {
            return Err(EngineError::Generic("Live text requires exactly one embedded TheDraw font".into()));
        }
        Ok(fonts.remove(0))
    }

    pub fn render(&self, buffer_type: BufferType) -> Result<Layer> {
        let tdf = self.font()?;
        let fill = tdf.font_type() != retrofont::tdf::TdfFontType::Color;
        let font = Font::Tdf(Box::new(tdf));
        let mut measure = Target::new(None, buffer_type, self.attribute);
        self.render_into(&font, fill, &mut measure, None)?;
        let mut layer = Layer::new(font.name(), (measure.width.max(1), measure.height.max(1)));
        layer.properties.has_alpha_channel = true;
        self.render_into(&font, fill, &mut Target::new(Some(&mut layer), buffer_type, self.attribute), None)?;
        layer.properties.is_locked = true;
        layer.set_live_text(Some(self.clone()));
        Ok(layer)
    }

    /// Layer positions where a caret goes before each character of `text` and after the last
    /// one, and the height of a text line.
    pub fn caret_layout(&self) -> Result<(Vec<Position>, i32)> {
        let tdf = self.font()?;
        let fill = tdf.font_type() != retrofont::tdf::TdfFontType::Color;
        let font = Font::Tdf(Box::new(tdf));
        let mut carets = Vec::with_capacity(self.text.chars().count() + 1);
        self.render_into(&font, fill, &mut Target::new(None, BufferType::CP437, self.attribute), Some(&mut carets))?;
        Ok((carets, line_height(&font)?))
    }

    /// The characters of `text` this font can render: line breaks, spaces and glyphs in either case.
    pub fn renderable(&self, text: &str) -> Result<String> {
        let font = Font::Tdf(Box::new(self.font()?));
        Ok(text
            .chars()
            .filter(|character| *character == '\n' || resolve_glyph(&font, *character).is_some())
            .collect())
    }

    fn render_into(&self, font: &Font, fill: bool, target: &mut Target<'_>, mut carets: Option<&mut Vec<Position>>) -> Result<()> {
        let line_height = line_height(font)?;
        let options = RenderOptions {
            outline_style: usize::from(self.outline_style),
            ..Default::default()
        };
        let mut start = Position::default();
        for character in self.text.chars() {
            if let Some(carets) = carets.as_deref_mut() {
                carets.push(start);
            }
            if character == '\n' {
                start.x = 0;
                start.y += line_height + i32::from(self.line_spacing);
                target.include(Position::new(0, start.y))?;
                continue;
            }
            target.start = start;
            target.position = start;
            target.end_x = start.x;
            let glyph = resolve_glyph(font, character).ok_or_else(|| EngineError::Generic(format!("TheDraw font has no glyph for {character:?}")))?;
            if let Some((width, height)) = font.glyph_size(glyph) {
                if width > crate::limits::MAX_BUFFER_WIDTH as usize || height > crate::limits::MAX_BUFFER_HEIGHT as usize {
                    return Err(EngineError::Generic("Live text glyph exceeds the layer size limits".into()));
                }
                for y in 0..height as i32 {
                    for x in 0..width as i32 {
                        let pos = start + Position::new(x, y);
                        target.include(pos)?;
                        if fill {
                            if let Some(layer) = &mut target.layer {
                                layer.set_char(pos, AttributedChar::new(' ', self.attribute));
                            }
                        }
                    }
                }
                target.end_x = start.x + width as i32;
            }
            font.render_glyph(target, glyph, &options)
                .map_err(|error| EngineError::Generic(error.to_string()))?;
            start.x = target.end_x + i32::from(self.letter_spacing);
        }
        if let Some(carets) = carets {
            carets.push(start);
        }
        Ok(())
    }

    pub(crate) fn encode(&self) -> Result<Vec<u8>> {
        self.font()?;
        let mut bytes = vec![1, self.outline_style, self.letter_spacing, self.line_spacing];
        TextAttribute::encode_attribute(&self.attribute, &mut bytes);
        bytes.extend_from_slice(&(self.text.len() as u32).to_le_bytes());
        bytes.extend_from_slice(self.text.as_bytes());
        bytes.extend_from_slice(&(self.font_data.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&self.font_data);
        Ok(bytes)
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 4 || bytes[0] != 1 {
            return Err(EngineError::Generic("Invalid live text record version".into()));
        }
        let (rest, attribute) = TextAttribute::decode_attribute(&bytes[4..])?;
        let mut input = Cursor::new(rest);
        fn blob(input: &mut Cursor<&[u8]>, limit: usize) -> Result<Vec<u8>> {
            let length = input.read_u32::<LittleEndian>()? as usize;
            if length > limit || length > input.get_ref().len().saturating_sub(input.position() as usize) {
                return Err(EngineError::Generic("Invalid live text record length".into()));
            }
            let mut bytes = vec![0; length];
            input.read_exact(&mut bytes)?;
            Ok(bytes)
        }
        let text = String::from_utf8(blob(&mut input, 65_536)?).map_err(|error| EngineError::Generic(error.to_string()))?;
        let font_data = blob(&mut input, 1_048_576)?;
        if input.position() as usize != rest.len() {
            return Err(EngineError::Generic("Trailing live text record data".into()));
        }
        let result = Self {
            text,
            font_data,
            attribute,
            outline_style: bytes[1],
            letter_spacing: bytes[2],
            line_spacing: bytes[3],
        };
        result.font()?;
        Ok(result)
    }
}

fn line_height(font: &Font) -> Result<i32> {
    i32::try_from(font.max_height().max(1)).map_err(|_| EngineError::Generic("Live text font is too tall".into()))
}

/// TheDraw fonts often contain only one case, matching the ordinary Font tool.
fn resolve_glyph(font: &Font, character: char) -> Option<char> {
    if font.has_char(character) || character == ' ' {
        return Some(character);
    }
    let other = if character.is_lowercase() {
        character.to_uppercase().collect::<String>()
    } else {
        character.to_lowercase().collect::<String>()
    };
    let mut chars = other.chars();
    let first = chars.next().filter(|ch| font.has_char(*ch));
    if chars.next().is_some() {
        return None;
    }
    first
}

struct Target<'a> {
    layer: Option<&'a mut Layer>,
    buffer_type: BufferType,
    attribute: TextAttribute,
    position: Position,
    start: Position,
    end_x: i32,
    width: i32,
    height: i32,
}

impl<'a> Target<'a> {
    fn new(layer: Option<&'a mut Layer>, buffer_type: BufferType, attribute: TextAttribute) -> Self {
        Self {
            layer,
            buffer_type,
            attribute,
            position: Position::default(),
            start: Position::default(),
            end_x: 0,
            width: 0,
            height: 0,
        }
    }

    fn include(&mut self, pos: Position) -> Result<()> {
        if !crate::limits::is_within_limits(pos.x + 1, pos.y + 1) {
            return Err(EngineError::Generic("Live text exceeds the layer size limits".into()));
        }
        self.width = self.width.max(pos.x + 1);
        self.height = self.height.max(pos.y + 1);
        Ok(())
    }
}

impl FontTarget for Target<'_> {
    type Error = EngineError;

    fn draw(&mut self, cell: Cell) -> Result<()> {
        self.include(self.position)?;
        let mut attribute = if let (Some(fg), Some(bg)) = (cell.fg, cell.bg) {
            let mut attribute = TextAttribute::from_color(fg, bg);
            attribute.set_is_blinking(cell.blink);
            attribute
        } else {
            self.attribute
        };
        if let Some(fg) = cell.fg {
            attribute.set_foreground(u32::from(fg));
        }
        if let Some(bg) = cell.bg {
            attribute.set_background(u32::from(bg));
        }
        if let Some(layer) = &mut self.layer {
            layer.set_char(self.position, AttributedChar::new(self.buffer_type.convert_from_unicode(cell.ch), attribute));
        }
        self.position.x += 1;
        self.end_x = self.end_x.max(self.position.x);
        Ok(())
    }

    fn skip(&mut self) -> Result<()> {
        self.include(self.position)?;
        self.position.x += 1;
        self.end_x = self.end_x.max(self.position.x);
        Ok(())
    }

    fn next_line(&mut self) -> Result<()> {
        self.position.y += 1;
        self.position.x = self.start.x;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Size, TextPane};
    use retrofont::{tdf::TdfFontType, Glyph, GlyphPart};

    fn source() -> LiveText {
        let mut font = TdfFont::new("Live test", TdfFontType::Block, 2);
        let mut glyph = Glyph::new(2, 1);
        glyph.parts = vec![GlyphPart::Char('\u{2588}'), GlyphPart::Skip];
        font.add_glyph('A', glyph);
        let mut source = LiveText::new(&font, TextAttribute::new(4, 1)).unwrap();
        source.text = "a A\nA".into();
        source.letter_spacing = 1;
        source.line_spacing = 2;
        source
    }

    #[test]
    fn live_text_renders_case_spaces_newlines_and_colors() {
        let source = source();
        let layer = source.render(BufferType::CP437).unwrap();
        assert_eq!(layer.size(), Size::new(8, 4));
        assert_eq!(layer.char_at(Position::default()).ch, '\u{db}');
        assert_eq!(layer.char_at(Position::new(1, 0)), AttributedChar::new(' ', source.attribute));
        assert_eq!(layer.char_at(Position::new(6, 0)).ch, '\u{db}');
        assert_eq!(layer.char_at(Position::new(0, 3)).ch, '\u{db}');
        assert!(!layer.char_at(Position::new(2, 0)).is_visible());
        assert_eq!(layer.live_text(), Some(&source));
        assert_eq!(layer.role, crate::Role::Text);
        assert!(layer.properties.is_locked);
    }

    #[test]
    fn live_text_codec_preserves_source_and_rejects_corruption() {
        let source = source();
        let bytes = source.encode().unwrap();
        assert_eq!(LiveText::decode(&bytes).unwrap(), source);
        for end in 0..bytes.len() {
            assert!(LiveText::decode(&bytes[..end]).is_err(), "truncated at {end}");
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(LiveText::decode(&trailing).is_err());
        for (index, value) in [(0, 2), (1, 19), (2, 33), (3, 33)] {
            let mut invalid = bytes.clone();
            invalid[index] = value;
            assert!(LiveText::decode(&invalid).is_err());
        }
    }

    #[test]
    fn live_text_retains_color_glyph_attributes_and_outline_styles() {
        let mut font = TdfFont::new("Color", TdfFontType::Color, 1);
        let mut glyph = Glyph::new(1, 1);
        glyph.parts = vec![GlyphPart::AnsiChar {
            ch: 'X',
            fg: 2,
            bg: 3,
            blink: true,
        }];
        font.add_glyph('A', glyph);
        let mut attribute = TextAttribute::new(4, 1);
        attribute.set_is_bold(true);
        let mut source = LiveText::new(&font, attribute).unwrap();
        source.text = "A".into();
        let cell = source.render(BufferType::CP437).unwrap().char_at(Position::default());
        assert_eq!(cell.attribute.foreground(), 2);
        assert_eq!(cell.attribute.background(), 3);
        assert!(!cell.attribute.is_bold());
        assert!(cell.attribute.is_blinking());

        let mut font = TdfFont::new("Outline", TdfFontType::Outline, 1);
        let mut glyph = Glyph::new(1, 1);
        glyph.parts = vec![GlyphPart::OutlinePlaceholder(b'A')];
        font.add_glyph('A', glyph);
        source.font_data = font.to_bytes().unwrap();
        for style in [0, 1, 18] {
            source.outline_style = style;
            let cell = source.render(BufferType::CP437).unwrap().char_at(Position::default());
            assert_eq!(
                cell.ch,
                BufferType::CP437.convert_from_unicode(retrofont::transform_outline(usize::from(style), b'A'))
            );
            assert_eq!(cell.attribute, attribute);
        }
    }

    #[test]
    fn live_text_reports_caret_positions_and_renderable_characters() {
        let source = source();
        let (carets, height) = source.caret_layout().unwrap();
        assert_eq!(height, 1);
        let expected = [(0, 0), (3, 0), (6, 0), (9, 0), (0, 3), (3, 3)].map(|(x, y)| Position::new(x, y));
        assert_eq!(carets, expected);
        assert_eq!(source.renderable("a?\nb A").unwrap(), "a\n A");
    }

    #[test]
    fn live_text_rejects_missing_glyphs_and_oversized_layouts() {
        let mut source = source();
        source.text = "?".into();
        assert!(source.render(BufferType::CP437).is_err());
        source.text = "A".repeat(1000);
        assert!(source.render(BufferType::CP437).is_err());
        source.text = "\n".repeat(20_001);
        assert!(source.render(BufferType::CP437).is_err());
        source.text.clear();
        assert_eq!(source.render(BufferType::CP437).unwrap().size(), Size::new(1, 1));
    }
}
