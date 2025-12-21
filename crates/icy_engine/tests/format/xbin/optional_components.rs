//! Tests for XBin files with optional components
//!
//! Tests all combinations of optional XBin components:
//! - Palette (can be omitted if default)
//! - Font (can be omitted if default 16-line VGA font)
//! - Compression (can be disabled)

use icy_engine::{BitFont, FileFormat, Palette, SaveOptions, TextBuffer, TextPane};

/// Test helper: Creates a simple test buffer with default palette and font
fn create_test_buffer_default() -> TextBuffer {
    let mut buffer = TextBuffer::new((80, 25));

    // Add some test content with default palette
    for y in 0..5 {
        for x in 0..20 {
            let ch = ((y * 20 + x) % 26) as u8 + b'A';
            buffer.layers[0].set_char((x, y), {
                icy_engine::AttributedChar::new(ch as char, icy_engine::TextAttribute::new((x % 16) as u32, ((y + 1) % 16) as u32))
            });
        }
    }

    buffer
}

/// Test helper: Creates a buffer with custom palette
fn create_test_buffer_custom_palette() -> TextBuffer {
    let mut buffer = create_test_buffer_default();

    // Modify palette to make it non-default
    let mut pal = Palette::new();
    for i in 0..16 {
        let color = icy_engine::Color::new((i * 4) as u8, (i * 3) as u8, (i * 2) as u8);
        pal.set_color(i as u32, color);
    }
    buffer.palette = pal;

    buffer
}

/// Test helper: Creates a buffer with custom font (8x14)
fn create_test_buffer_custom_font() -> TextBuffer {
    let mut buffer = create_test_buffer_default();

    // Use a different font height (14 instead of 16)
    let data = vec![0u8; 256 * 14];
    let font = BitFont::from_basic(8, 14, &data);
    buffer.set_font(0, font);

    buffer
}

/// Test XBin with no optional components (default palette, default font, no compression)
/// Expected: No palette flag, no font flag, no compress flag
#[test]
fn test_xbin_no_palette_no_font_no_compress() {
    let buffer = create_test_buffer_default();

    let mut options = SaveOptions::new();
    options.compress = false;
    options.save_sauce = None;

    let xbin_data = FileFormat::XBin.to_bytes(&buffer, &options).unwrap();

    // Verify flags byte (position 10)
    let flags = xbin_data[10];
    assert_eq!(flags & 0b0001, 0, "Palette flag should not be set");
    assert_eq!(flags & 0b0010, 0, "Font flag should not be set");
    assert_eq!(flags & 0b0100, 0, "Compress flag should not be set");

    // Verify we can load it back
    let loaded = FileFormat::XBin.from_bytes(&xbin_data, None).unwrap();
    assert_eq!(buffer.width(), loaded.screen.buffer.width());
    assert_eq!(buffer.height(), loaded.screen.buffer.height());
}

/// Test XBin with custom palette, default font, no compression
/// Expected: Palette flag set, no font flag, no compress flag
#[test]
fn test_xbin_palette_no_font_no_compress() {
    let buffer = create_test_buffer_custom_palette();

    let mut options = SaveOptions::new();
    options.compress = false;
    options.save_sauce = None;

    let xbin_data = FileFormat::XBin.to_bytes(&buffer, &options).unwrap();

    // Verify flags byte (position 10)
    let flags = xbin_data[10];
    assert_eq!(flags & 0b0001, 0b0001, "Palette flag should be set");
    assert_eq!(flags & 0b0010, 0, "Font flag should not be set");
    assert_eq!(flags & 0b0100, 0, "Compress flag should not be set");

    // Verify we can load it back
    let loaded = FileFormat::XBin.from_bytes(&xbin_data, None).unwrap();
    assert_eq!(buffer.width(), loaded.screen.buffer.width());
    assert_eq!(buffer.height(), loaded.screen.buffer.height());
}

/// Test XBin with default palette, custom font, no compression
/// Expected: No palette flag, font flag set, no compress flag
#[test]
fn test_xbin_no_palette_font_no_compress() {
    let buffer = create_test_buffer_custom_font();

    let mut options = SaveOptions::new();
    options.compress = false;
    options.save_sauce = None;

    let xbin_data = FileFormat::XBin.to_bytes(&buffer, &options).unwrap();

    // Verify flags byte (position 10)
    let flags = xbin_data[10];
    assert_eq!(flags & 0b0001, 0, "Palette flag should not be set");
    assert_eq!(flags & 0b0010, 0b0010, "Font flag should be set");
    assert_eq!(flags & 0b0100, 0, "Compress flag should not be set");

    // Verify we can load it back
    let loaded = FileFormat::XBin.from_bytes(&xbin_data, None).unwrap();
    assert_eq!(buffer.width(), loaded.screen.buffer.width());
    assert_eq!(buffer.height(), loaded.screen.buffer.height());
}

/// Test XBin with custom palette and font, no compression
/// Expected: Palette flag set, font flag set, no compress flag
#[test]
fn test_xbin_palette_font_no_compress() {
    let mut buffer = create_test_buffer_custom_palette();

    // Also add custom font
    let data = vec![0u8; 256 * 14];
    let font = BitFont::from_basic(8, 14, &data);
    buffer.set_font(0, font);

    let mut options = SaveOptions::new();
    options.compress = false;
    options.save_sauce = None;

    let xbin_data = FileFormat::XBin.to_bytes(&buffer, &options).unwrap();

    // Verify flags byte (position 10)
    let flags = xbin_data[10];
    assert_eq!(flags & 0b0001, 0b0001, "Palette flag should be set");
    assert_eq!(flags & 0b0010, 0b0010, "Font flag should be set");
    assert_eq!(flags & 0b0100, 0, "Compress flag should not be set");

    // Verify we can load it back
    let loaded = FileFormat::XBin.from_bytes(&xbin_data, None).unwrap();
    assert_eq!(buffer.width(), loaded.screen.buffer.width());
    assert_eq!(buffer.height(), loaded.screen.buffer.height());
}

/// Test XBin with default palette, default font, with compression
/// Expected: No palette flag, no font flag, compress flag set
#[test]
fn test_xbin_no_palette_no_font_compress() {
    let buffer = create_test_buffer_default();

    let mut options = SaveOptions::new();
    options.compress = true;
    options.save_sauce = None;

    let xbin_data = FileFormat::XBin.to_bytes(&buffer, &options).unwrap();

    // Verify flags byte (position 10)
    let flags = xbin_data[10];
    assert_eq!(flags & 0b0001, 0, "Palette flag should not be set");
    assert_eq!(flags & 0b0010, 0, "Font flag should not be set");
    assert_eq!(flags & 0b0100, 0b0100, "Compress flag should be set");

    // Verify we can load it back
    let loaded = FileFormat::XBin.from_bytes(&xbin_data, None).unwrap();
    assert_eq!(buffer.width(), loaded.screen.buffer.width());
    assert_eq!(buffer.height(), loaded.screen.buffer.height());
}

/// Test XBin with custom palette, default font, with compression
/// Expected: Palette flag set, no font flag, compress flag set
#[test]
fn test_xbin_palette_no_font_compress() {
    let buffer = create_test_buffer_custom_palette();

    let mut options = SaveOptions::new();
    options.compress = true;
    options.save_sauce = None;

    let xbin_data = FileFormat::XBin.to_bytes(&buffer, &options).unwrap();

    // Verify flags byte (position 10)
    let flags = xbin_data[10];
    assert_eq!(flags & 0b0001, 0b0001, "Palette flag should be set");
    assert_eq!(flags & 0b0010, 0, "Font flag should not be set");
    assert_eq!(flags & 0b0100, 0b0100, "Compress flag should be set");

    // Verify we can load it back
    let loaded = FileFormat::XBin.from_bytes(&xbin_data, None).unwrap();
    assert_eq!(buffer.width(), loaded.screen.buffer.width());
    assert_eq!(buffer.height(), loaded.screen.buffer.height());
}

/// Test XBin with default palette, custom font, with compression
/// Expected: No palette flag, font flag set, compress flag set
#[test]
fn test_xbin_no_palette_font_compress() {
    let buffer = create_test_buffer_custom_font();

    let mut options = SaveOptions::new();
    options.compress = true;
    options.save_sauce = None;

    let xbin_data = FileFormat::XBin.to_bytes(&buffer, &options).unwrap();

    // Verify flags byte (position 10)
    let flags = xbin_data[10];
    assert_eq!(flags & 0b0001, 0, "Palette flag should not be set");
    assert_eq!(flags & 0b0010, 0b0010, "Font flag should be set");
    assert_eq!(flags & 0b0100, 0b0100, "Compress flag should be set");

    // Verify we can load it back
    let loaded = FileFormat::XBin.from_bytes(&xbin_data, None).unwrap();
    assert_eq!(buffer.width(), loaded.screen.buffer.width());
    assert_eq!(buffer.height(), loaded.screen.buffer.height());
}

/// Test XBin with all optional components (custom palette, custom font, compression)
/// Expected: All flags set
#[test]
fn test_xbin_palette_font_compress() {
    let mut buffer = create_test_buffer_custom_palette();

    // Also add custom font
    let data = vec![0u8; 256 * 14];
    let font = BitFont::from_basic(8, 14, &data);
    buffer.set_font(0, font);

    let mut options = SaveOptions::new();
    options.compress = true;
    options.save_sauce = None;

    let xbin_data = FileFormat::XBin.to_bytes(&buffer, &options).unwrap();

    // Verify flags byte (position 10)
    let flags = xbin_data[10];
    assert_eq!(flags & 0b0001, 0b0001, "Palette flag should be set");
    assert_eq!(flags & 0b0010, 0b0010, "Font flag should be set");
    assert_eq!(flags & 0b0100, 0b0100, "Compress flag should be set");

    // Verify we can load it back
    let loaded = FileFormat::XBin.from_bytes(&xbin_data, None).unwrap();
    assert_eq!(buffer.width(), loaded.screen.buffer.width());
    assert_eq!(buffer.height(), loaded.screen.buffer.height());
}
