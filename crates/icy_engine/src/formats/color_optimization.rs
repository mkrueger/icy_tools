use crate::{BitFont, Buffer, Glyph, SaveOptions, TextAttribute, TextPane};
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
    shape_map: HashMap<usize, HashMap<char, GlyphShape>>,
}

impl ColorOptimizer {
    pub fn new(buf: &Buffer, opt: &SaveOptions) -> Self {
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
    pub fn optimize(&self, buffer: &Buffer) -> Buffer {
        let mut b = buffer.flat_clone(false);
        for layer in &mut b.layers {
            let mut cur_attr = TextAttribute::default();
            for y in 0..layer.get_height() {
                for x in 0..layer.get_width() {
                    let attr_ch = layer.get_char((x, y));
                    let map = self.shape_map.get(&attr_ch.get_font_page()).unwrap();
                    let mut ch = attr_ch.ch;
                    let mut attribute = attr_ch.attribute;
                    match map.get(&attr_ch.ch) {
                        Some(&GlyphShape::Whitespace) => {
                            attribute.set_foreground(cur_attr.get_foreground());
                            if self.normalize_whitespace && map.contains_key(&' ') {
                                ch = ' ';
                            }
                        }
                        Some(&GlyphShape::Block) => {
                            attribute.set_background(cur_attr.get_background());
                        }
                        _ => {}
                    }
                    layer.set_char((x, y), crate::AttributedChar { ch, attribute });
                    cur_attr = attribute;
                }
            }
        }
        b
    }
}

fn generate_shape_map(buf: &Buffer) -> HashMap<usize, HashMap<char, GlyphShape>> {
    let mut shape_map = HashMap::new();
    for (slot, font) in buf.font_iter() {
        let mut font_map = HashMap::new();
        for (char, glyph) in &font.glyphs {
            font_map.insert(*char, get_shape(font, glyph));
        }
        shape_map.insert(*slot, font_map);
    }
    shape_map
}

fn get_shape(font: &BitFont, glyph: &Glyph) -> GlyphShape {
    let mut ones = 0;
    for row in &glyph.data {
        ones += row.count_ones();
    }
    if ones == 0 {
        GlyphShape::Whitespace
    } else if ones == font.size.width as u32 * font.size.height as u32 {
        GlyphShape::Block
    } else {
        GlyphShape::Mixed
    }
}
#[cfg(test)]
mod tests {
    use crate::{AttributedChar, Buffer, ColorOptimizer, TextAttribute, TextPane};

    #[test]
    pub fn test_foreground_optimization() {
        let mut buffer = Buffer::new((5, 1));
        let attr = TextAttribute::new(14, 0);
        buffer.layers[0].set_char((0, 0), AttributedChar::new('A', attr));

        let save_options = crate::SaveOptions::default();
        let opt = ColorOptimizer::new(&buffer, &save_options);

        let opt_buf = opt.optimize(&buffer);
        for x in 0..opt_buf.get_width() {
            assert_eq!(opt_buf.layers[0].get_char((x, 0)).attribute.get_foreground(), 14, "x={x}");
        }
    }

    #[test]
    pub fn test_background_optimization() {
        let mut buffer = Buffer::new((5, 1));
        for x in 0..buffer.get_width() {
            let attr = TextAttribute::new(14, x as u32);
            buffer.layers[0].set_char((x, 0), AttributedChar::new(219 as char, attr));
        }
        let save_options = crate::SaveOptions::default();
        let opt = ColorOptimizer::new(&buffer, &save_options);

        let opt_buf = opt.optimize(&buffer);
        for x in 0..opt_buf.get_width() {
            assert_eq!(opt_buf.layers[0].get_char((x, 0)).attribute.get_background(), 0, "x={x}");
        }
    }

    #[test]
    pub fn test_ws_normalization() {
        let mut buffer = Buffer::new((5, 1));
        for x in 0..buffer.get_width() {
            buffer.layers[0].set_char((x, 0), AttributedChar::new(0 as char, TextAttribute::default()));
        }
        buffer.layers[0].set_char((3, 0), AttributedChar::new(255 as char, TextAttribute::default()));

        let mut save_options = crate::SaveOptions::default();
        save_options.normalize_whitespaces = true;
        let opt = ColorOptimizer::new(&buffer, &save_options);

        let opt_buf = opt.optimize(&buffer);
        for x in 0..opt_buf.get_width() {
            assert_eq!(opt_buf.layers[0].get_char((x, 0)).ch, ' ', "x={x}");
        }
    }
}
