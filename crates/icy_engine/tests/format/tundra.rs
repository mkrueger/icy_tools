
#[cfg(test)]
mod tests {
    use crate::{AttributedChar, OutputFormat, TextAttribute, TextBuffer, compare_buffers};

    #[test]
    pub fn test_ice() {
        let mut buffer = TextBuffer::new((80, 25));
        buffer.ice_mode = crate::IceMode::Ice;
        buffer.layers[0].set_char((0, 0), AttributedChar::new('A', TextAttribute::from_u8(0b0000_1000, crate::IceMode::Ice)));
        buffer.layers[0].set_char((1, 0), AttributedChar::new('B', TextAttribute::from_u8(0b1100_1111, crate::IceMode::Ice)));
        test_tundra(&mut buffer);
    }

    fn test_tundra(buffer: &mut TextBuffer) -> TextBuffer {
        let xb = super::TundraDraw::default();
        let mut opt = crate::SaveOptions::default();
        opt.compress = false;
        let bytes = xb.to_bytes(buffer, &opt).unwrap();
        let buffer2 = xb.load_buffer(std::path::Path::new("test.xb"), &bytes, None).unwrap();
        let mut opt = crate::CompareOptions::ALL;
        opt.compare_palette = false;
        opt.ignore_invisible_chars = true;
        compare_buffers(buffer, &buffer2, opt);
        buffer2
    }
}
