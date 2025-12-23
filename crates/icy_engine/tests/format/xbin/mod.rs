mod optional_components;
mod roundtrip;

use super::ansi2::{compare_buffers, CompareOptions};
use icy_engine::{AttributedChar, BitFont, Color, FileFormat, IceMode, SaveOptions, TextAttribute, TextBuffer, TextPane};

#[test]
pub fn test_blink() {
    let mut buffer = create_xb_buffer();
    buffer.ice_mode = IceMode::Blink;
    buffer.layers[0].set_char((0, 0), AttributedChar::new('A', TextAttribute::from_u8(0b0000_1000, IceMode::Blink)));
    buffer.layers[0].set_char((1, 0), AttributedChar::new('B', TextAttribute::from_u8(0b1000_1000, IceMode::Blink)));
    let res = test_xbin(&mut buffer);
    let ch = res.layers[0].char_at((1, 0).into());

    assert_eq!(ch.attribute.foreground(), 0b1000);
    assert_eq!(ch.attribute.background(), 0b0000);
    assert!(ch.attribute.is_blinking());
}

#[test]
pub fn test_ice() {
    let mut buffer = create_xb_buffer();
    buffer.ice_mode = IceMode::Ice;
    buffer.layers[0].set_char((0, 0), AttributedChar::new('A', TextAttribute::from_u8(0b0000_1000, IceMode::Ice)));
    buffer.layers[0].set_char((1, 0), AttributedChar::new('B', TextAttribute::from_u8(0b1100_1111, IceMode::Ice)));
    let res = test_xbin(&mut buffer);
    let ch = res.layers[0].char_at((1, 0).into());

    assert_eq!(ch.attribute.foreground(), 0b1111);
    assert_eq!(ch.attribute.background(), 0b1100);
}

#[test]
pub fn test_custom_palette() {
    let mut buffer = create_xb_buffer();
    buffer.ice_mode = IceMode::Ice;

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

    buffer.layers[0].set_char((0, 0), AttributedChar::new('A', TextAttribute::from_u8(0b0000_1000, IceMode::Ice)));
    buffer.layers[0].set_char((1, 0), AttributedChar::new('B', TextAttribute::from_u8(0b1100_1111, IceMode::Ice)));
    let res = test_xbin(&mut buffer);
    let ch = res.layers[0].char_at((1, 0).into());

    assert_eq!(ch.attribute.foreground(), 0b1111);
    assert_eq!(ch.attribute.background(), 0b1100);
}

#[test]
pub fn test_custom_font() {
    let mut buffer = create_xb_buffer();
    buffer.set_font(0, BitFont::from_ansi_font_page(42, 16).unwrap().clone());
    buffer.ice_mode = IceMode::Blink;
    buffer.layers[0].set_char((0, 0), AttributedChar::new('A', TextAttribute::from_u8(0b0000_1000, IceMode::Blink)));
    test_xbin(&mut buffer);
}

#[test]
pub fn test_extended_font_ice() {
    let mut buffer = create_xb_buffer();
    buffer.ice_mode = IceMode::Ice;
    buffer.set_font(1, BitFont::from_ansi_font_page(42, 16).unwrap().clone());
    let mut attr = TextAttribute::from_u8(0b1111_0111, IceMode::Ice);
    attr.set_font_page(0);
    buffer.layers[0].set_char((0, 0), AttributedChar::new('A', attr));
    attr.set_font_page(1);
    buffer.layers[0].set_char((1, 0), AttributedChar::new('B', attr));
    let res = test_xbin(&mut buffer);

    let ch = res.layers[0].char_at((1, 0).into());

    assert_eq!(ch.attribute.foreground(), 0b0111);
    assert_eq!(ch.attribute.background(), 0b1111);
    assert_eq!(ch.attribute.font_page(), 1);
}

#[test]
pub fn test_extended_font_blink() {
    let mut buffer = create_xb_buffer();
    buffer.ice_mode = IceMode::Blink;
    buffer.set_font(1, BitFont::from_ansi_font_page(42, 16).unwrap().clone());
    let mut attr: TextAttribute = TextAttribute::from_u8(0b1111_0111, IceMode::Blink);
    attr.set_font_page(0);
    buffer.layers[0].set_char((0, 0), AttributedChar::new('A', attr));
    attr.set_font_page(1);
    buffer.layers[0].set_char((1, 0), AttributedChar::new('B', attr));

    let mut opt = SaveOptions::default();
    opt.format = icy_engine::FormatOptions::Compressed(icy_engine::CompressedFormatOptions { compress: false });

    let res = test_xbin(&mut buffer);

    let ch = res.layers[0].char_at((1, 0).into());

    assert_eq!(ch.attribute.foreground(), 0b0111);
    assert_eq!(ch.attribute.background(), 0b0111);
    assert_eq!(ch.attribute.font_page(), 1);
    assert!(ch.attribute.is_blinking());
}

fn create_xb_buffer() -> TextBuffer {
    let mut buffer: TextBuffer = TextBuffer::new((80, 25));
    for y in 0..buffer.height() {
        for x in 0..buffer.width() {
            buffer.layers[0].set_char((x, y), AttributedChar::new(' ', TextAttribute::default()));
        }
    }
    buffer
}

fn test_xbin(buffer: &mut TextBuffer) -> TextBuffer {
    let xb = FileFormat::XBin;
    let mut opt = SaveOptions::default();
    opt.format = icy_engine::FormatOptions::Compressed(icy_engine::CompressedFormatOptions { compress: false });
    opt.preprocess.optimize_colors = false;
    let bytes = xb.to_bytes(buffer, &opt).unwrap();
    let buffer2 = xb.from_bytes(&bytes, None).unwrap().screen.buffer;
    compare_buffers(buffer, &buffer2, CompareOptions::ALL);

    opt.format = icy_engine::FormatOptions::Compressed(icy_engine::CompressedFormatOptions { compress: true });
    let bytes = xb.to_bytes(buffer, &opt).unwrap();
    let buffer2 = xb.from_bytes(&bytes, None).unwrap().screen.buffer;
    compare_buffers(buffer, &buffer2, CompareOptions::ALL);

    buffer2
}
