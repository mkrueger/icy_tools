use icy_engine::{AttributedChar, Buffer, FontGlyph, FontType, Position, TextAttribute};

#[test]
fn test_from_buffer_empty() {
    let buffer = Buffer::new((10, 10));

    let glyph = FontGlyph::from_buffer(&buffer, FontType::Block);
    assert_eq!(glyph.size.width, 0);
    assert_eq!(glyph.size.height, 0);
    assert!(glyph.data.is_empty());
}

#[test]
fn test_from_buffer_block_simple() {
    let mut buffer = Buffer::new((5, 3));
    buffer.layers[0].set_char(Position::new(0, 0), AttributedChar::from_char(b'A' as char));
    buffer.layers[0].set_char(Position::new(1, 0), AttributedChar::from_char(b'B' as char));
    buffer.layers[0].set_char(Position::new(2, 0), AttributedChar::from_char(b'C' as char));

    let glyph = FontGlyph::from_buffer(&buffer, FontType::Block);
    assert_eq!(glyph.size.width, 3);
    assert_eq!(glyph.size.height, 1);
    assert_eq!(glyph.data, vec![b'A', b'B', b'C']);
}

#[test]
fn test_from_buffer_block_multiline() {
    let mut buffer = Buffer::new((5, 5));
    buffer.layers[0].set_char(Position::new(0, 0), AttributedChar::from_char(b'A' as char));
    buffer.layers[0].set_char(Position::new(1, 0), AttributedChar::from_char(b'B' as char));
    buffer.layers[0].set_char(Position::new(0, 1), AttributedChar::from_char(b'C' as char));
    buffer.layers[0].set_char(Position::new(1, 1), AttributedChar::from_char(b'D' as char));
    buffer.layers[0].set_char(Position::new(2, 1), AttributedChar::from_char(b'E' as char));

    let glyph = FontGlyph::from_buffer(&buffer, FontType::Block);
    assert_eq!(glyph.size.width, 3); // Max width is 3 (second line)
    assert_eq!(glyph.size.height, 2);
    assert_eq!(
        glyph.data,
        vec![
            b'A', b'B', // First line
            13,   // CR
            b'C', b'D', b'E' // Second line
        ]
    );
}

#[test]
fn test_from_buffer_block_with_ampersand() {
    let mut buffer = Buffer::new((10, 3));
    buffer.layers[0].set_char(Position::new(0, 0), AttributedChar::from_char(b'A' as char));
    buffer.layers[0].set_char(Position::new(1, 0), AttributedChar::from_char(b'B' as char));
    buffer.layers[0].set_char(Position::new(2, 0), AttributedChar::from_char(b'&' as char));
    buffer.layers[0].set_char(Position::new(3, 0), AttributedChar::from_char(b'C' as char));
    buffer.layers[0].set_char(Position::new(0, 1), AttributedChar::from_char(b'D' as char));
    buffer.layers[0].set_char(Position::new(1, 1), AttributedChar::from_char(b'E' as char));

    let glyph = FontGlyph::from_buffer(&buffer, FontType::Block);
    assert_eq!(glyph.size.width, 3); // Width includes '&'
    assert_eq!(glyph.size.height, 2);
    assert_eq!(
        glyph.data,
        vec![
            b'A', b'B', b'&', // First line stops at '&'
            13,   // CR
            b'D', b'E' // Second line
        ]
    );
}

#[test]
fn test_from_buffer_color_simple() {
    let mut buffer = Buffer::new((5, 3));
    let ch = AttributedChar::new('A', TextAttribute::from_u8(0x1E, icy_engine::IceMode::Ice));
    buffer.layers[0].set_char(Position::new(0, 0), ch);

    let ch = AttributedChar::new('B', TextAttribute::from_u8(0x2F, icy_engine::IceMode::Ice));
    buffer.layers[0].set_char(Position::new(1, 0), ch);

    let glyph = FontGlyph::from_buffer(&buffer, FontType::Color);
    assert_eq!(glyph.size.width, 2);
    assert_eq!(glyph.size.height, 1);
    assert_eq!(
        glyph.data,
        vec![
            b'A', 0x1E, // 'A' with attribute
            b'B', 0x2F // 'B' with attribute
        ]
    );
}

#[test]
fn test_from_buffer_color_multiline() {
    let mut buffer = Buffer::new((5, 5));

    let ch = AttributedChar::new('X', TextAttribute::from_u8(0x14, icy_engine::IceMode::Ice));
    buffer.layers[0].set_char(Position::new(0, 0), ch);

    let ch = AttributedChar::new('Y', TextAttribute::from_u8(0x15, icy_engine::IceMode::Ice));
    buffer.layers[0].set_char(Position::new(1, 0), ch);

    let ch = AttributedChar::new('Z', TextAttribute::from_u8(0x16, icy_engine::IceMode::Ice));
    buffer.layers[0].set_char(Position::new(0, 1), ch);

    let glyph = FontGlyph::from_buffer(&buffer, FontType::Color);
    assert_eq!(glyph.size.width, 2);
    assert_eq!(glyph.size.height, 2);
    assert_eq!(
        glyph.data,
        vec![
            b'X', 0x14, // First line
            b'Y', 0x15, 13, // CR (no attribute)
            b'Z', 0x16 // Second line
        ]
    );
}

#[test]
fn test_from_buffer_color_with_ampersand() {
    let mut buffer = Buffer::new((10, 3));

    let ch = AttributedChar::new('A', TextAttribute::from_u8(0x1A, icy_engine::IceMode::Ice));
    buffer.layers[0].set_char(Position::new(0, 0), ch);

    buffer.layers[0].set_char(Position::new(1, 0), AttributedChar::from_char(b'&' as char));

    let ch = AttributedChar::new('B', TextAttribute::from_u8(0x1B, icy_engine::IceMode::Ice));
    buffer.layers[0].set_char(Position::new(2, 0), ch);

    let ch = AttributedChar::new('C', TextAttribute::from_u8(0x1C, icy_engine::IceMode::Ice));
    buffer.layers[0].set_char(Position::new(0, 1), ch);

    let glyph = FontGlyph::from_buffer(&buffer, FontType::Color);
    assert_eq!(glyph.size.width, 2);
    assert_eq!(glyph.size.height, 2);
    assert_eq!(
        glyph.data,
        vec![
            b'A', 0x1A, // 'A' with attribute
            b'&', // '&' without attribute
            13,   // CR
            b'C', 0x1C // 'C' with attribute
        ]
    );
}

#[test]
fn test_from_buffer_outline_valid_chars() {
    let mut buffer = Buffer::new((10, 3));
    buffer.layers[0].set_char(Position::new(0, 0), AttributedChar::from_char(b'A' as char));
    buffer.layers[0].set_char(Position::new(1, 0), AttributedChar::from_char(b'B' as char));
    buffer.layers[0].set_char(Position::new(2, 0), AttributedChar::from_char(b'@' as char));
    buffer.layers[0].set_char(Position::new(3, 0), AttributedChar::from_char(b'O' as char));
    buffer.layers[0].set_char(Position::new(4, 0), AttributedChar::from_char('♦'));
    buffer.layers[0].set_char(Position::new(5, 0), AttributedChar::from_char(b'!' as char));

    let glyph = FontGlyph::from_buffer(&buffer, FontType::Outline);
    assert_eq!(glyph.size.width, 6); // Width based on actual content
    assert_eq!(glyph.size.height, 1);
    // Invalid char (♦) is skipped
    assert_eq!(glyph.data, vec![b'A', b'B', b'@', b'O', b'!']);
}

#[test]
fn test_from_buffer_outline_with_spaces() {
    let mut buffer = Buffer::new((10, 3));
    buffer.layers[0].set_char(Position::new(0, 0), AttributedChar::from_char(b' ' as char));
    buffer.layers[0].set_char(Position::new(1, 0), AttributedChar::from_char(b'A' as char));
    buffer.layers[0].set_char(Position::new(2, 0), AttributedChar::from_char(b' ' as char));
    buffer.layers[0].set_char(Position::new(3, 0), AttributedChar::from_char(b'B' as char));

    let glyph = FontGlyph::from_buffer(&buffer, FontType::Outline);
    assert_eq!(glyph.size.width, 4);
    assert_eq!(glyph.size.height, 1);
    // Spaces are valid outline characters
    assert_eq!(glyph.data, vec![b' ', b'A', b' ', b'B']);
}

#[test]
fn test_from_buffer_outline_multiline() {
    let mut buffer = Buffer::new((5, 5));
    buffer.layers[0].set_char(Position::new(0, 0), AttributedChar::from_char(b'X' as char));
    buffer.layers[0].set_char(Position::new(1, 0), AttributedChar::from_char(b'Y' as char));
    buffer.layers[0].set_char(Position::new(0, 1), AttributedChar::from_char(b'Z' as char));
    buffer.layers[0].set_char(Position::new(1, 1), AttributedChar::from_char(b'W' as char));
    buffer.layers[0].set_char(Position::new(2, 1), AttributedChar::from_char(b'@' as char));

    let glyph = FontGlyph::from_buffer(&buffer, FontType::Outline);
    assert_eq!(glyph.size.width, 3);
    assert_eq!(glyph.size.height, 2);
    assert_eq!(glyph.data, vec![b'X', b'Y', 13, b'Z', b'W', b'@']);
}

#[test]
fn test_from_buffer_outline_with_ampersand() {
    let mut buffer = Buffer::new((10, 3));
    buffer.layers[0].set_char(Position::new(0, 0), AttributedChar::from_char(b'A' as char));
    buffer.layers[0].set_char(Position::new(1, 0), AttributedChar::from_char(b'B' as char));
    buffer.layers[0].set_char(Position::new(2, 0), AttributedChar::from_char(b'&' as char));
    buffer.layers[0].set_char(Position::new(3, 0), AttributedChar::from_char(b'C' as char));

    let glyph = FontGlyph::from_buffer(&buffer, FontType::Outline);
    assert_eq!(glyph.size.width, 3);
    assert_eq!(glyph.size.height, 1);
    assert_eq!(glyph.data, vec![b'A', b'B', b'&']);
}

#[test]
fn test_from_buffer_figlet() {
    let mut buffer = Buffer::new((5, 3));
    buffer.layers[0].set_char(Position::new(0, 0), AttributedChar::from_char(b'#' as char));
    buffer.layers[0].set_char(Position::new(1, 0), AttributedChar::from_char(b'#' as char));
    buffer.layers[0].set_char(Position::new(0, 1), AttributedChar::from_char(b'#' as char));
    buffer.layers[0].set_char(Position::new(1, 1), AttributedChar::from_char(b'#' as char));

    let glyph = FontGlyph::from_buffer(&buffer, FontType::Figlet);
    assert_eq!(glyph.size.width, 2);
    assert_eq!(glyph.size.height, 2);
    // Figlet uses newline instead of CR
    assert_eq!(glyph.data, vec![b'#', b'#', b'\n', b'#', b'#']);
}

#[test]
fn test_from_buffer_trailing_empty_lines() {
    let mut buffer = Buffer::new((5, 5));
    buffer.layers[0].set_char(Position::new(0, 0), AttributedChar::from_char(b'A' as char));
    buffer.layers[0].set_char(Position::new(1, 0), AttributedChar::from_char(b'B' as char));
    // Lines 1-4 are empty

    let glyph = FontGlyph::from_buffer(&buffer, FontType::Block);
    assert_eq!(glyph.size.width, 2);
    assert_eq!(glyph.size.height, 1); // Empty lines not included
    assert_eq!(glyph.data, vec![b'A', b'B']);
}

#[test]
fn test_from_buffer_sparse_content() {
    let mut buffer = Buffer::new((10, 10));
    buffer.layers[0].set_char(Position::new(0, 0), AttributedChar::from_char(b'A' as char));
    // Skip line 1
    buffer.layers[0].set_char(Position::new(0, 2), AttributedChar::from_char(b'B' as char));
    buffer.layers[0].set_char(Position::new(1, 2), AttributedChar::from_char(b'C' as char));
    // Skip line 3
    buffer.layers[0].set_char(Position::new(0, 4), AttributedChar::from_char(b'D' as char));

    let glyph = FontGlyph::from_buffer(&buffer, FontType::Block);
    assert_eq!(glyph.size.width, 2); // Max width
    assert_eq!(glyph.size.height, 5); // Up to line 4 (index 4)
    assert_eq!(
        glyph.data,
        vec![
            b'A', 13, 13, // Empty line 1
            b'B', b'C', 13, 13, // Empty line 3
            b'D'
        ]
    );
}

#[test]
fn test_from_buffer_color_0xff_character() {
    let mut buffer = Buffer::new((5, 3));

    let ch = AttributedChar::new(0xFF as char, TextAttribute::from_u8(0x07, icy_engine::IceMode::Ice));
    buffer.layers[0].set_char(Position::new(0, 0), ch);

    let ch = AttributedChar::new('A', TextAttribute::from_u8(0x1A, icy_engine::IceMode::Ice));
    buffer.layers[0].set_char(Position::new(1, 0), ch);

    let glyph = FontGlyph::from_buffer(&buffer, FontType::Color);
    assert_eq!(glyph.size.width, 2);
    assert_eq!(glyph.size.height, 1);
    assert_eq!(
        glyph.data,
        vec![
            0xFF, 0x07, // 0xFF with attribute
            b'A', 0x1A // 'A' with attribute
        ]
    );
}

#[test]
fn test_from_buffer_only_ampersand() {
    let mut buffer = Buffer::new((5, 3));
    buffer.layers[0].set_char(Position::new(0, 0), AttributedChar::from_char(b'&' as char));

    let glyph = FontGlyph::from_buffer(&buffer, FontType::Block);
    assert_eq!(glyph.size.width, 1);
    assert_eq!(glyph.size.height, 1);
    assert_eq!(glyph.data, vec![b'&']);
}

#[test]
fn test_from_buffer_ampersand_first_char() {
    let mut buffer = Buffer::new((5, 3));
    buffer.layers[0].set_char(Position::new(0, 0), AttributedChar::from_char(b'&' as char));
    buffer.layers[0].set_char(Position::new(1, 0), AttributedChar::from_char(b'A' as char));

    let glyph = FontGlyph::from_buffer(&buffer, FontType::Block);
    assert_eq!(glyph.size.width, 1);
    assert_eq!(glyph.size.height, 1);
    assert_eq!(glyph.data, vec![b'&']);

    let glyph = FontGlyph::from_buffer(&buffer, FontType::Color);
    assert_eq!(glyph.size.width, 1);
    assert_eq!(glyph.size.height, 1);
    assert_eq!(glyph.data, vec![b'&']);

    let glyph = FontGlyph::from_buffer(&buffer, FontType::Outline);
    assert_eq!(glyph.size.width, 1);
    assert_eq!(glyph.size.height, 1);
    assert_eq!(glyph.data, vec![b'&']);
}
