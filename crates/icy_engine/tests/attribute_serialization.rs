use icy_engine::{
    clipboard, AttributeColor, AttributedChar, BufferType, Color, FileFormat, FormatOptions, IcyDrawFormatOptions, Rectangle, SaveOptions, SelectionMask, Tag,
    TagPlacement, TagRole, TextAttribute, TextBuffer, TextPane,
};

fn colors() -> Vec<AttributeColor> {
    let mut colors = vec![AttributeColor::Transparent, AttributeColor::Rgb(0, 127, 255)];
    for index in 0..=255 {
        colors.push(AttributeColor::Palette(index));
        colors.push(AttributeColor::ExtendedPalette(index));
    }
    colors
}

#[test]
fn all_palette_indices_and_color_kinds_roundtrip() {
    for foreground in colors() {
        for background in colors() {
            let mut attribute = TextAttribute::from_colors(foreground, background);
            attribute.set_font_page(255);
            attribute.attr = 0xA55A;
            let mut bytes = Vec::new();
            TextAttribute::encode_attribute(&attribute, &mut bytes);
            bytes.push(0xFE);
            let (rest, decoded) = TextAttribute::decode_attribute(&bytes).unwrap();
            assert_eq!(decoded, attribute);
            assert_eq!(rest, &[0xFE]);
        }
    }
}

#[test]
fn every_truncated_attribute_prefix_is_rejected() {
    for color in colors() {
        let attribute = TextAttribute::from_colors(color, color);
        let mut bytes = Vec::new();
        TextAttribute::encode_attribute(&attribute, &mut bytes);
        for end in 0..bytes.len() {
            assert!(TextAttribute::decode_attribute(&bytes[..end]).is_err(), "accepted prefix {end} for {color:?}");
        }
    }
}

#[test]
fn unknown_color_tags_are_rejected() {
    for tag in 20..=255 {
        assert!(TextAttribute::decode_attribute(&[tag, 1, 0, 0, 0]).is_err());
        assert!(TextAttribute::decode_attribute(&[1, tag, 0, 0, 0]).is_err());
    }
}

#[test]
fn legacy_attribute_bytes_remain_unchanged() {
    for (bytes, fg, bg) in [
        (vec![16, 1, 7, 1, 128], AttributeColor::Palette(15), AttributeColor::Palette(0)),
        (vec![17, 255, 0, 7, 1, 128], AttributeColor::ExtendedPalette(255), AttributeColor::Transparent),
        (
            vec![18, 1, 2, 3, 17, 42, 7, 1, 128],
            AttributeColor::Rgb(1, 2, 3),
            AttributeColor::ExtendedPalette(42),
        ),
    ] {
        let (rest, decoded) = TextAttribute::decode_attribute(&bytes).unwrap();
        assert!(rest.is_empty());
        assert_eq!(decoded.foreground_color(), fg);
        assert_eq!(decoded.background_color(), bg);
        assert_eq!(decoded.font_page(), 7);
        assert_eq!(decoded.attr, 0x8001);
        let mut encoded = Vec::new();
        TextAttribute::encode_attribute(&decoded, &mut encoded);
        assert_eq!(encoded, bytes);
    }
}

fn clipboard_header(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = vec![0; 9];
    bytes.extend(width.to_le_bytes());
    bytes.extend(height.to_le_bytes());
    bytes
}

#[test]
fn clipboard_rejects_truncated_cells() {
    let mut bytes = clipboard_header(2, 1);
    for ch in ['A', 'B'] {
        bytes.extend(u32::from(ch).to_le_bytes());
        TextAttribute::encode_attribute(
            &TextAttribute::from_colors(AttributeColor::Rgb(1, 2, 3), AttributeColor::ExtendedPalette(255)),
            &mut bytes,
        );
    }
    for end in 0..bytes.len() {
        assert!(
            clipboard::from_clipboard_data(BufferType::Unicode, &bytes[..end]).is_none(),
            "accepted prefix {end}"
        );
    }
    assert!(clipboard::from_clipboard_data(BufferType::Unicode, &bytes).is_some());
}

#[test]
fn clipboard_rejects_invalid_unicode() {
    for codepoint in [0xD800u32, 0xDFFF, 0x110000, u32::MAX] {
        let mut bytes = clipboard_header(1, 1);
        bytes.extend(codepoint.to_le_bytes());
        TextAttribute::encode_attribute(&TextAttribute::default(), &mut bytes);
        assert!(clipboard::from_clipboard_data(BufferType::Unicode, &bytes).is_none());
    }
}

#[test]
fn clipboard_rejects_invalid_dimensions_and_version() {
    for (width, height) in [(0, 1), (1, 0), (u32::MAX, u32::MAX), (u32::MAX, 1), (1, u32::MAX), (1001, 1), (1, 20001)] {
        assert!(clipboard::from_clipboard_data(BufferType::Unicode, &clipboard_header(width, height)).is_none());
    }
    let mut bytes = clipboard_header(1, 1);
    bytes[0] = 2;
    bytes.extend(u32::from('A').to_le_bytes());
    TextAttribute::encode_attribute(&TextAttribute::default(), &mut bytes);
    assert!(clipboard::from_clipboard_data(BufferType::Unicode, &bytes).is_none());
}

#[test]
fn clipboard_writer_versions_and_colors_roundtrip() {
    for color in colors() {
        let mut buffer = TextBuffer::new((1, 1));
        buffer.buffer_type = BufferType::Unicode;
        let attribute = TextAttribute::from_colors(color, color);
        let ch = AttributedChar::new('🦀', attribute);
        buffer.layers[0].set_char((0, 0), ch);
        let mut mask = SelectionMask::default();
        mask.set_size((1, 1).into());
        mask.add_rectangle(Rectangle::from_min_size((0, 0), (1, 1)));
        let bytes = clipboard::clipboard_data(&buffer, 0, &mask, &None).unwrap();
        assert_eq!(bytes[0], u8::from(matches!(color, AttributeColor::Palette(16..=255))));
        let layer = clipboard::from_clipboard_data(BufferType::Unicode, &bytes).unwrap();
        assert_eq!(layer.char_at((0, 0).into()), ch);
    }
}

fn save_options(compress: bool) -> SaveOptions {
    SaveOptions {
        format: FormatOptions::IcyDraw(IcyDrawFormatOptions {
            skip_thumbnail: true,
            compress,
        }),
        ..Default::default()
    }
}

fn document_version(png: &[u8]) -> u16 {
    let mut offset = 8;
    while offset + 12 <= png.len() {
        let len = u32::from_be_bytes(png[offset..offset + 4].try_into().unwrap()) as usize;
        if &png[offset + 4..offset + 8] == b"icYD" {
            let payload = &png[offset + 8..offset + 8 + len];
            let keyword_len = u16::from_le_bytes(payload[1..3].try_into().unwrap()) as usize;
            if &payload[3..3 + keyword_len] == b"ICED" {
                return u16::from_le_bytes(payload[7 + keyword_len..9 + keyword_len].try_into().unwrap());
            }
        }
        offset += len + 12;
    }
    panic!("ICED header missing");
}

#[test]
fn icy_custom_and_xterm_palettes_remain_distinct() {
    for compress in [false, true] {
        let mut buffer = TextBuffer::new((256, 2));
        for index in 0..=255u8 {
            buffer.palette.set_color(u32::from(index), Color::new(index, 123, 45));
            for (y, color) in [(0, AttributeColor::Palette(index)), (1, AttributeColor::ExtendedPalette(index))] {
                buffer.layers[0].set_char((i32::from(index), y), AttributedChar::new('A', TextAttribute::from_colors(color, color)));
            }
        }
        let bytes = FileFormat::IcyDraw.to_bytes(&buffer, &save_options(compress)).unwrap();
        assert_eq!(document_version(&bytes), 2);
        let loaded = FileFormat::IcyDraw.from_bytes(&bytes, None).unwrap().screen.buffer;
        assert_eq!(loaded.palette, buffer.palette);
        for y in 0..2 {
            for x in 0..256 {
                assert_eq!(loaded.layers[0].char_at((x, y).into()), buffer.layers[0].char_at((x, y).into()));
            }
        }
    }
}

#[test]
fn ordinary_icy_documents_still_use_v1() {
    let mut buffer = TextBuffer::new((1, 1));
    buffer.layers[0].set_char(
        (0, 0),
        AttributedChar::new(
            'A',
            TextAttribute::from_colors(AttributeColor::ExtendedPalette(255), AttributeColor::Rgb(1, 2, 3)),
        ),
    );
    let bytes = FileFormat::IcyDraw.to_bytes(&buffer, &save_options(false)).unwrap();
    assert_eq!(document_version(&bytes), 1);
    let loaded = FileFormat::IcyDraw.from_bytes(&bytes, None).unwrap().screen.buffer;
    assert_eq!(loaded.layers[0].char_at((0, 0).into()), buffer.layers[0].char_at((0, 0).into()));
}

#[test]
fn tag_only_custom_palette_requires_v2() {
    let mut buffer = TextBuffer::new((1, 1));
    buffer.tags.push(Tag {
        is_enabled: true,
        preview: "A".into(),
        replacement_value: "B".into(),
        position: (0, 0).into(),
        length: 1,
        alignment: std::fmt::Alignment::Left,
        tag_placement: TagPlacement::InText,
        tag_role: TagRole::Displaycode,
        attribute: TextAttribute::new(42, 255),
    });
    for compress in [false, true] {
        let bytes = FileFormat::IcyDraw.to_bytes(&buffer, &save_options(compress)).unwrap();
        assert_eq!(document_version(&bytes), 2);
        let loaded = FileFormat::IcyDraw.from_bytes(&bytes, None).unwrap().screen.buffer;
        assert_eq!(loaded.tags, buffer.tags);
    }
}

fn png_with_record(version: u16, keyword: &str, payload: &[u8], compress: bool) -> Vec<u8> {
    let mut header = version.to_le_bytes().to_vec();
    header.extend([if compress { 2 } else { 0 }, 0, 0, 0, 0, 0, 0]);
    header.extend(1u32.to_le_bytes());
    header.extend(1u32.to_le_bytes());
    header.extend([8, 16]);
    assert_eq!(header.len(), 19);
    let payload = if compress {
        zstd::stream::encode_all(payload, 3).unwrap()
    } else {
        payload.to_vec()
    };
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, 1, 1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    for (keyword, data) in [("ICED", header.as_slice()), (keyword, payload.as_slice()), ("END", &[])] {
        let mut record = vec![1];
        record.extend((keyword.len() as u16).to_le_bytes());
        record.extend(keyword.as_bytes());
        record.extend((data.len() as u32).to_le_bytes());
        record.extend(data);
        writer.write_chunk(png::chunk::ChunkType(*b"icYD"), &record).unwrap();
    }
    writer.write_image_data(&[0, 0, 0, 0]).unwrap();
    writer.finish().unwrap();
    out
}

#[test]
fn icy_rejects_truncated_layer_attributes_even_when_minimum_cell_size_matches() {
    for compress in [false, true] {
        for attribute in [&[18, 1, 2, 3, 1][..], &[1, 18, 1, 2, 3], &[255, 1, 0, 0, 0]] {
            let mut layer = vec![0; 21]; // empty title, mode, color, flags, offsets
            layer.extend(1i32.to_le_bytes());
            layer.extend(1i32.to_le_bytes());
            layer.extend(u32::from('A').to_le_bytes());
            layer.extend(attribute);
            let bytes = png_with_record(1, "LAYER", &layer, compress);
            let error = FileFormat::IcyDraw
                .from_bytes(&bytes, None)
                .err()
                .expect("invalid attribute accepted")
                .to_string();
            assert!(error.contains("attribute at 0,0"), "unexpected error: {error}");
        }
    }
}

#[test]
fn icy_rejects_truncated_tag_attributes() {
    let mut tag = 1u16.to_le_bytes().to_vec();
    tag.extend([0; 22]); // two empty strings + 14 fixed bytes, no attribute
    for compress in [false, true] {
        let bytes = png_with_record(1, "TAG", &tag, compress);
        let error = FileFormat::IcyDraw
            .from_bytes(&bytes, None)
            .err()
            .expect("invalid attribute accepted")
            .to_string();
        assert!(error.contains("tag attribute"), "unexpected error: {error}");
    }
}

#[test]
fn icy_rejects_future_binary_version() {
    let bytes = png_with_record(3, "TAG", &[0, 0], false);
    assert!(FileFormat::IcyDraw.from_bytes(&bytes, None).is_err());
}
