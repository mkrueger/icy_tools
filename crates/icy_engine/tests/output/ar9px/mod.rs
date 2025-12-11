use std::fs;

use icy_engine::{Position, TextBuffer, TextPane, formats::FileFormat};

const TEST_FILE: &str = "tests/output/ar9px/files/aeleus-usta1.ans";

fn load_test_buffer() -> TextBuffer {
    let path = std::path::Path::new(TEST_FILE);
    let data = fs::read(path).expect("Failed to read test file");
    FileFormat::Ansi.from_bytes(path, &data, None).expect("Failed to parse test file").buffer
}

/// Test 9px font rendering (letter spacing)
#[test]
pub fn test_9px_rendering() {
    crate::init_logging();

    let mut buffer = load_test_buffer();

    // Debug: Check what character is at position 29, 0 (pixel 269 = 29*9 + 8)
    let ch = buffer.get_char(Position::new(29, 0));
    println!("Character at (29, 0): '{}' code=0x{:02X}", ch.ch, ch.ch as u32);
    println!("Is box-drawing (0xC0-0xDF): {}", (ch.ch as u32) >= 0xC0 && (ch.ch as u32) <= 0xDF);

    // Check the original font
    let font = buffer.get_font(0).unwrap();
    println!("Original font size: {:?}", font.size());
    if let Some(glyph) = font.get_glyph(ch.ch) {
        println!("Original glyph width: {}", glyph.bitmap.width);
        if let Some(row) = glyph.bitmap.pixels.get(0) {
            println!("Original row len: {}, last pixel: {:?}", row.len(), row.last());
        }
    }

    // Set letter spacing to trigger 9px font creation
    buffer.set_use_letter_spacing(true);

    // Check the 9px font
    let font_9px = buffer.get_font_for_render(0).unwrap();
    println!("9px font size: {:?}", font_9px.size());
    if let Some(glyph) = font_9px.get_glyph(ch.ch) {
        println!("9px glyph width: {}", glyph.bitmap.width);
        if let Some(row) = glyph.bitmap.pixels.get(0) {
            println!("9px row len: {}, 9th pixel (index 8): {:?}", row.len(), row.get(8));
        }
    } else {
        println!("ERROR: No glyph found for char 0x{:02X} in 9px font!", ch.ch as u32);
    }

    crate::compare_buffer_output_with_options(
        &mut buffer,
        std::path::Path::new("tests/output/ar9px/files/aeleus-usta1_9px"),
        true,  // use_letter_spacing
        false, // use_aspect_ratio
    );
}

/// Test aspect ratio rendering
#[test]
pub fn test_aspect_ratio_rendering() {
    crate::init_logging();

    let mut buffer = load_test_buffer();

    crate::compare_buffer_output_with_options(
        &mut buffer,
        std::path::Path::new("tests/output/ar9px/files/aeleus-usta1_ar"),
        false, // use_letter_spacing
        true,  // use_aspect_ratio
    );
}

/// Test combined 9px + aspect ratio rendering
#[test]
pub fn test_9px_and_aspect_ratio_rendering() {
    crate::init_logging();

    let mut buffer = load_test_buffer();

    crate::compare_buffer_output_with_options(
        &mut buffer,
        std::path::Path::new("tests/output/ar9px/files/aeleus-usta1_9pxar"),
        true, // use_letter_spacing
        true, // use_aspect_ratio
    );
}
