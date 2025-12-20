use super::ansi2::{CompareOptions, compare_buffers};
use icy_engine::{AnsiSaveOptionsV2, AttributedChar, FileFormat, IceMode, TextAttribute, TextBuffer};

#[test]
pub fn test_ice() {
    let mut buffer = TextBuffer::new((80, 25));
    buffer.ice_mode = IceMode::Ice;
    buffer.layers[0].set_char((0, 0), AttributedChar::new('A', TextAttribute::from_u8(0b0000_1000, IceMode::Ice)));
    buffer.layers[0].set_char((1, 0), AttributedChar::new('B', TextAttribute::from_u8(0b1100_1111, IceMode::Ice)));
    test_tundra(&mut buffer);
}

fn test_tundra(buffer: &mut TextBuffer) -> TextBuffer {
    let xb = FileFormat::TundraDraw;
    let mut opt = AnsiSaveOptionsV2::default();
    opt.compress = false;
    opt.lossles_output = true;
    let bytes = xb.to_bytes(buffer, &opt).unwrap();
    let buffer2 = xb.from_bytes(&bytes, None).unwrap().screen.buffer;
    let mut opt = CompareOptions::ALL;
    opt.compare_palette = false;
    opt.ignore_invisible_chars = true;
    compare_buffers(buffer, &buffer2, opt);
    buffer2
}
