use crate::{AnsiSaveOptionsV2, BitFont, TextAttribute, TextBuffer, TextPane};
use libyaff::GlyphDefinition;
use std::collections::HashMap;

enum GlyphShape {
    Whitespace,
    Block,
    Mixed,
}

/// Reduces the amount of color changes inside a buffer.
/// Ignoring foreground color changes on whitespaces and background color changes on blocks.
///
/// That reduces the amount of color switches required in the output formats.
pub struct ColorOptimizer {
    normalize_whitespace: bool,
    shape_map: HashMap<u8, HashMap<char, GlyphShape>>,
}

impl ColorOptimizer {
    pub fn new(buf: &TextBuffer, opt: &AnsiSaveOptionsV2) -> Self {
        let shape_map = generate_shape_map(buf);
        Self {
            shape_map,
            normalize_whitespace: opt.normalize_whitespaces,
        }
    }

    /// .
    ///
    /// # Panics
    ///
    /// Panics if .
    pub fn optimize(&self, buffer: &TextBuffer) -> TextBuffer {
        let mut b = buffer.clone();
        let tags_enabled = b.show_tags;
        b.show_tags = false;
        for layer in &mut b.layers {
            let mut cur_attr = TextAttribute::default();
            for y in 0..layer.height() {
                for x in 0..layer.width() {
                    let attr_ch = layer.char_at((x, y).into());
                    let map = self.shape_map.get(&attr_ch.font_page()).unwrap();
                    let mut ch = attr_ch.ch;
                    let mut attribute = attr_ch.attribute;
                    match map.get(&attr_ch.ch) {
                        Some(&GlyphShape::Whitespace) => {
                            attribute.set_foreground(cur_attr.foreground());
                            if self.normalize_whitespace && map.contains_key(&' ') {
                                ch = ' ';
                            }
                        }
                        Some(&GlyphShape::Block) => {
                            attribute.set_background(cur_attr.background());
                        }
                        _ => {}
                    }
                    layer.set_char((x, y), crate::AttributedChar { ch, attribute });
                    cur_attr = attribute;
                }
            }
        }
        b.show_tags = tags_enabled;
        b
    }
}

fn generate_shape_map(buf: &TextBuffer) -> HashMap<u8, HashMap<char, GlyphShape>> {
    let mut shape_map = HashMap::new();
    for (slot, font) in buf.font_iter() {
        let mut font_map = HashMap::new();
        // Iterate over all glyphs in the yaff font
        for glyph in &font.yaff_font.glyphs {
            // Try to get character from glyph labels
            if let Some(ch) = get_char_from_labels(&glyph.labels) {
                font_map.insert(ch, get_shape(font, glyph));
            }
        }
        shape_map.insert(*slot, font_map);
    }
    shape_map
}

fn get_char_from_labels(labels: &[libyaff::Label]) -> Option<char> {
    // Try to parse any label as a character
    for label in labels {
        match label {
            libyaff::Label::Codepoint(codepoints) => {
                // Get the first codepoint and convert to char
                if let Some(&code) = codepoints.first() {
                    if let Some(ch) = char::from_u32(code as u32) {
                        return Some(ch);
                    }
                }
            }
            _ => {
                // Fallback to debug string parsing for other label types
                let label_str = format!("{:?}", label);
                // Try hex format like "0x41"
                if let Some(hex_str) = label_str.strip_prefix("0x") {
                    if let Ok(code) = u32::from_str_radix(hex_str, 16) {
                        if let Some(ch) = char::from_u32(code) {
                            return Some(ch);
                        }
                    }
                }
                // Try decimal
                if let Ok(code) = label_str.parse::<u32>() {
                    if let Some(ch) = char::from_u32(code) {
                        return Some(ch);
                    }
                }
            }
        }
    }
    None
}

fn get_shape(font: &BitFont, glyph: &GlyphDefinition) -> GlyphShape {
    let mut ones = 0;
    let size = font.size();
    for row in &glyph.bitmap.pixels {
        ones += row.iter().filter(|&&b| b).count();
    }
    if ones == 0 {
        GlyphShape::Whitespace
    } else if ones == (size.width * size.height) as usize {
        GlyphShape::Block
    } else {
        GlyphShape::Mixed
    }
}
