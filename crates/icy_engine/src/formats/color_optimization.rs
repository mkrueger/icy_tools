use crate::{BitFont, SaveOptions, TextAttribute, TextBuffer, TextPane, fonts::CompactGlyph};
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
    pub fn new(buf: &TextBuffer, opt: &SaveOptions) -> Self {
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
        // Iterate over all 256 characters in the font
        for code in 0u8..=255 {
            let ch = code as char;
            let glyph = font.glyph(ch);
            font_map.insert(ch, get_shape(font, glyph));
        }
        shape_map.insert(*slot, font_map);
    }
    shape_map
}

fn get_shape(font: &BitFont, glyph: &CompactGlyph) -> GlyphShape {
    let mut ones = 0;
    let _size = font.size();

    // Count set bits in the glyph
    for y in 0..glyph.height as usize {
        let row = glyph.data[y];
        ones += row.count_ones() as usize;
    }

    // Calculate expected total pixels based on actual glyph dimensions
    let total_pixels = (glyph.width as usize) * (glyph.height as usize);

    if ones == 0 {
        GlyphShape::Whitespace
    } else if ones == total_pixels {
        GlyphShape::Block
    } else {
        GlyphShape::Mixed
    }
}
