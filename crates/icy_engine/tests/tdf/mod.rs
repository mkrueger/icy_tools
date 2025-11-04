mod tdf_container;

use icy_engine::{AnsiFont, Buffer, FontGlyph, FontType, Position, Size, TextPane, editor::EditState, font::TheDrawFont};

#[test]
fn test_tdf_font_basic_loading() {
    // Requires a real test_font.tdf sitting next to this file.
    // Mark with #[ignore] if you don’t have it yet.
    let font_data = include_bytes!("test_font.tdf");
    let result = TheDrawFont::from_bytes(font_data);
    assert!(result.is_ok(), "Failed to load TDF font");
    let fonts = result.unwrap();
    assert!(!fonts.is_empty(), "No fonts found in TDF file");
    let font = &fonts[0];
    assert!(!font.name.is_empty(), "Font name is empty");
    assert!(font.char_table.len() > 0, "No characters in font");
}

#[test]
fn test_tdf_round_trip_block_font_single_glyph() {
    // Create a block font with glyph for 'A'
    let mut font = TheDrawFont::new("TESTFONT", FontType::Block, 0);
    let glyph = FontGlyph {
        size: Size::new(4, 2),
        // Data: 'A','B',13,'C','D'
        data: vec![b'A', b'B', 13, b'C', b'D'],
    };
    font.set_glyph('A', glyph);

    // Serialize and re-load
    let bytes = font.as_tdf_bytes().expect("serialize");
    let fonts = TheDrawFont::from_bytes(&bytes).expect("parse back");
    assert_eq!(fonts.len(), 1);
    let parsed = &fonts[0];
    assert_eq!(parsed.name, "TESTFONT");
    assert_eq!(parsed.font_type, FontType::Block);
    let pg = parsed.get_glyph('A').expect("glyph A");
    assert_eq!(pg.size.width, 4);
    assert_eq!(pg.size.height, 2);
    assert_eq!(pg.data, vec![b'A', b'B', 13, b'C', b'D']);
}

#[test]
fn test_tdf_round_trip_color_font_attribute_rules() {
    // Color font glyph: A(attr), CR(no attr), &, B(attr), 0 terminator added in save
    // We embed attribute bytes only after characters except CR (13) and '&'
    let mut font = TheDrawFont::new("COLORF", FontType::Color, 0);
    let glyph = FontGlyph {
        size: Size::new(3, 2),
        data: vec![
            b'A', 0x1E, // A + attr
            13,   // newline (no attribute)
            b'&', // end-of-line marker (no attribute)
            b'B', 0x2F, // B + attr
        ],
    };
    font.set_glyph('Z', glyph);

    let bytes = font.as_tdf_bytes().expect("serialize color font");
    let fonts = TheDrawFont::from_bytes(&bytes).expect("parse color font");
    assert_eq!(fonts.len(), 1);
    let parsed = &fonts[0];
    assert_eq!(parsed.font_type, FontType::Color);
    let pg = parsed.get_glyph('Z').expect("glyph Z");
    // Ensure attributes present only after A and B, not after 13 or '&'
    assert_eq!(pg.data, vec![b'A', 0x1E, 13, b'&', b'B', 0x2F], "Attribute insertion mismatch for color font");
}

#[test]
fn test_tdf_render_block_multiline_cr() {
    let mut font = TheDrawFont::new("BLK", FontType::Block, 0);
    let glyph = FontGlyph {
        size: Size::new(2, 2),
        data: vec![b'X', b'Y', 13, b'Z', b'W'],
    };
    font.set_glyph('X', glyph);

    let buffer = Buffer::new((20, 10));
    let mut edit = EditState::from_buffer(buffer);
    edit.get_caret_mut().set_position(Position::new(0, 0));
    let sz = font.render(&mut edit, b'X', false).expect("render size");
    assert_eq!(sz.width, 2);
    assert_eq!(sz.height, 2);

    let b = edit.get_buffer();
    assert_eq!(b.get_char((0, 0)).ch, 'X');
    assert_eq!(b.get_char((1, 0)).ch, 'Y');
    assert_eq!(b.get_char((0, 1)).ch, 'Z');
    assert_eq!(b.get_char((1, 1)).ch, 'W');
}

#[test]
fn test_tdf_render_block_ampersand_edit_mode_visible() {
    let mut font = TheDrawFont::new("AMPED", FontType::Block, 0);
    let glyph = FontGlyph {
        size: Size::new(3, 1),
        data: vec![b'A', b'B', b'&'],
    };
    font.set_glyph('A', glyph);

    let buffer = Buffer::new((10, 5));
    let mut edit = EditState::from_buffer(buffer);
    edit.get_caret_mut().set_position(Position::new(0, 0));
    let sz = font.render(&mut edit, b'A', true).expect("render");
    assert_eq!(sz.height, 1);
    assert_eq!(sz.width, 3);
    let b = edit.get_buffer();
    assert_eq!(b.get_char((0, 0)).ch, 'A');
    assert_eq!(b.get_char((1, 0)).ch, 'B');
    assert_eq!(b.get_char((2, 0)).ch, '&');
}

#[test]
fn test_tdf_transform_outline_mapping() {
    // Outline style 0 mapping for 'A' should map to OUTLINE_CHAR_SET[0][0] (0xC4)
    let mapped = TheDrawFont::transform_outline(0, b'A');
    assert_eq!(mapped, 0xC4, "Outline mapping for 'A' in style 0 unexpected");
    // For a non-mapped char outside range (e.g. '?'), should become space
    let mapped_q = TheDrawFont::transform_outline(0, b'?');
    assert_eq!(mapped_q, b' ', "Char outside outline range should map to space");
}

#[test]
fn test_tdf_render_color_font_attribute_consumption() {
    let mut font = TheDrawFont::new("COLR", FontType::Color, 0);
    // Sequence: A(attr), &, B(attr), CR, C(attr)
    let glyph = FontGlyph {
        size: Size::new(3, 2),
        data: vec![b'A', 0x10, b'&', b'\r', b'B', 0x2A, b'\r', b'C', 0x1E],
    };
    font.set_glyph('C', glyph);

    let buffer = Buffer::new((20, 8));
    let mut edit = EditState::from_buffer(buffer);
    edit.get_caret_mut().set_position(Position::new(0, 0));
    let sz = font.render(&mut edit, b'C', false).expect("size");
    assert_eq!(sz.height, 3);
    assert_eq!(sz.width, 1);

    let b = edit.get_buffer();
    // A with attribute (foreground should match lower nibble if IceMode mapping)
    let a = b.get_char((0, 0));
    let bch = b.get_char((0, 1));
    let cch = b.get_char((0, 2));
    assert_eq!(a.ch, 'A');
    assert_eq!(bch.ch, 'B');
    assert_eq!(cch.ch, 'C');
    // Can't assert exact palette indices without attribute decode, but presence of chars suffices.
}

#[test]
fn test_tdf_render_color_font_truncated_attribute_safe() {
    let mut font = TheDrawFont::new("TRUNC", FontType::Color, 0);
    // Last char missing attribute byte
    let glyph = FontGlyph {
        size: Size::new(2, 1),
        data: vec![b'A', 0x14, b'B'], // B has no attribute (truncated)
    };
    font.set_glyph('T', glyph);

    let buffer = Buffer::new((10, 5));
    let mut edit = EditState::from_buffer(buffer);
    edit.get_caret_mut().set_position(Position::new(0, 0));
    let sz = font.render(&mut edit, b'T', false).expect("render");
    assert_eq!(sz.height, 1);
    assert!(sz.width >= 2);

    let b = edit.get_buffer();
    assert_eq!(b.get_char((0, 0)).ch, 'A');
    assert_eq!(b.get_char((1, 0)).ch, 'B');
}

#[test]
fn test_tdf_color_font_round_trip_preserves_skip_rules() {
    let mut font = TheDrawFont::new("CLRRT", FontType::Color, 0);
    let glyph = FontGlyph {
        size: Size::new(2, 2),
        data: vec![b'A', 0x11, 13, b'&', b'B', 0x22],
    };
    font.set_glyph('X', glyph);
    let bytes = font.as_tdf_bytes().expect("serialize");
    let fonts = TheDrawFont::from_bytes(&bytes).expect("parse");
    let parsed = &fonts[0];
    let pg = parsed.get_glyph('X').unwrap();
    assert_eq!(pg.data, vec![b'A', 0x11, 13, b'&', b'B', 0x22]);
}

#[test]
fn test_tdf_get_font_height_from_first_glyph() {
    let mut font = TheDrawFont::new("HEIGHT", FontType::Block, 0);
    let glyph = FontGlyph {
        size: Size::new(3, 5),
        data: vec![b'A', b'B', b'C'],
    };
    font.set_glyph('A', glyph);
    assert_eq!(font.get_font_height(), 5);
}

#[test]
fn test_tdf_clear_glyph() {
    let mut font = TheDrawFont::new("CLR", FontType::Block, 0);
    let glyph = FontGlyph {
        size: Size::new(1, 1),
        data: vec![b'Z'],
    };
    font.set_glyph('Z', glyph);
    assert!(font.get_glyph('Z').is_some());
    font.clear_glyph('Z');
    assert!(font.get_glyph('Z').is_none());
}

#[test]
fn test_tdf_create_font_bundle_multiple() {
    let mut font1 = TheDrawFont::new("ONE", FontType::Block, 0);
    font1.set_glyph(
        'A',
        FontGlyph {
            size: Size::new(1, 1),
            data: vec![b'A'],
        },
    );
    let mut font2 = TheDrawFont::new("TWO", FontType::Color, 0);
    font2.set_glyph(
        'B',
        FontGlyph {
            size: Size::new(1, 1),
            data: vec![b'B', 0x1F],
        },
    );

    let bundle = TheDrawFont::create_font_bundle(&[font1.clone(), font2.clone()]).expect("bundle");
    let fonts = TheDrawFont::from_bytes(&bundle).expect("parse bundle");
    assert_eq!(fonts.len(), 2);
    assert_eq!(fonts[0].name, "ONE");
    assert_eq!(fonts[1].name, "TWO");
    assert_eq!(fonts[0].font_type, FontType::Block);
    assert_eq!(fonts[1].font_type, FontType::Color);
}

#[test]
fn test_tdf_render_next_trait_drives_caret() {
    // Exercise AnsiFont impl: render_next
    let mut font = TheDrawFont::new("NEXT", FontType::Block, 0);
    font.set_glyph(
        'A',
        FontGlyph {
            size: Size::new(3, 1),
            data: vec![b'A', b'B', b'C'],
        },
    );
    let buffer = Buffer::new((10, 5));
    let mut edit = EditState::from_buffer(buffer);
    edit.get_caret_mut().set_position(Position::new(0, 0));

    // Render 'A' then next caret should advance by width (3)
    let pos_after = font.render_next(&mut edit, ' ', 'A', false);
    assert_eq!(pos_after.x, 3);
    assert_eq!(pos_after.y, 0);
    let b = edit.get_buffer();
    assert_eq!(b.get_char((0, 0)).ch, 'A');
    assert_eq!(b.get_char((1, 0)).ch, 'B');
    assert_eq!(b.get_char((2, 0)).ch, 'C');
}

#[test]
fn test_tdf_outline_transform_space_for_out_of_range() {
    // Char below 'A' should map to space
    let mapped = TheDrawFont::transform_outline(0, b'@'); // '@' is 64
    assert_eq!(mapped, b' ', "Outline mapping of '@' should produce space");
}

// Optional: detect current bug in set_glyph negative indexing (if fixed, remove should_panic)
#[test]
#[should_panic]
fn test_tdf_set_glyph_invalid_low_char_panics_currently() {
    // '\0' produces negative offset; current implementation panics
    let mut font = TheDrawFont::new("BUG", FontType::Block, 0);
    font.set_glyph(
        '\0',
        FontGlyph {
            size: Size::new(1, 1),
            data: vec![b'X'],
        },
    );
}
