
#[cfg(test)]
mod tests {
    use crate::{AttributedChar, BitFont, Color, OutputFormat, TextAttribute, TextBuffer, TextPane, compare_buffers};

    #[test]
    pub fn test_ice() {
        let mut buffer = create_buffer();
        buffer.ice_mode = crate::IceMode::Ice;
        buffer.layers[0].set_char((0, 0), AttributedChar::new('A', TextAttribute::from_u8(0b0000_1000, crate::IceMode::Ice)));
        buffer.layers[0].set_char((1, 0), AttributedChar::new('B', TextAttribute::from_u8(0b1100_1111, crate::IceMode::Ice)));
        test_artworx(&mut buffer);
    }

    #[test]
    pub fn test_custom_palette() {
        let mut buffer = create_buffer();
        buffer.ice_mode = crate::IceMode::Ice;

        for i in 0..4 {
            buffer.palette.set_color(i, Color::new(8 + i as u8 * 8, 0, 0));
        }
        for i in 0..4 {
            buffer.palette.set_color(4 + i, Color::new(0, 8 + i as u8 * 8, 0));
        }
        for i in 0..4 {
            buffer.palette.set_color(8 + i, Color::new(0, 0, 8 + i as u8 * 8));
        }
        for i in 0..3 {
            buffer.palette.set_color(12 + i, Color::new(i as u8 * 16, i as u8 * 8, 8 + i as u8 * 8));
        }

        buffer.layers[0].set_char((0, 0), AttributedChar::new('A', TextAttribute::from_u8(0b0000_1000, crate::IceMode::Ice)));
        buffer.layers[0].set_char((1, 0), AttributedChar::new('B', TextAttribute::from_u8(0b1100_1111, crate::IceMode::Ice)));
        test_artworx(&mut buffer);
    }

    #[test]
    pub fn test_custom_font() {
        let mut buffer = create_buffer();
        buffer.set_font(0, BitFont::from_ansi_font_page(42).unwrap());
        buffer.ice_mode = crate::IceMode::Ice;
        buffer.layers[0].set_char((0, 0), AttributedChar::new('A', TextAttribute::from_u8(0b0000_1000, crate::IceMode::Blink)));
        test_artworx(&mut buffer);
    }

    fn create_buffer() -> TextBuffer {
        let mut buffer = TextBuffer::new((80, 25));
        for y in 0..buffer.get_height() {
            for x in 0..buffer.get_width() {
                buffer.layers[0].set_char((x, y), AttributedChar::new(' ', TextAttribute::default()));
            }
        }
        buffer
    }

    fn test_artworx(buffer: &mut TextBuffer) -> TextBuffer {
        let xb = super::Artworx::default();
        let mut opt = crate::SaveOptions::default();
        opt.compress = false;
        let bytes = xb.to_bytes(buffer, &opt).unwrap();
        let buffer2 = xb.load_buffer(std::path::Path::new("test.adf"), &bytes, None).unwrap();
        compare_buffers(buffer, &buffer2, crate::CompareOptions::ALL);
        buffer2
    }
}
