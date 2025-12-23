use super::ansi2::{compare_buffers, CompareOptions};
use icy_engine::{AttributedChar, Color, FileFormat, Layer, Position, Role, SaveOptions, Sixel, TextAttribute, TextBuffer, TextPane};

const PNG_SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
const ICYD_CHUNK_TYPE: [u8; 4] = *b"icYD";
const ICYD_RECORD_VERSION: u8 = 1;
const ZSTD_FRAME_MAGIC: [u8; 4] = [0x28, 0xB5, 0x2F, 0xFD];

fn extract_png_chunks_by_type(png: &[u8], wanted: [u8; 4]) -> Vec<Vec<u8>> {
    assert!(png.len() >= PNG_SIGNATURE.len());
    assert_eq!(&png[..PNG_SIGNATURE.len()], &PNG_SIGNATURE);

    let mut res = Vec::new();
    let mut off = PNG_SIGNATURE.len();
    while off + 8 <= png.len() {
        let len = u32::from_be_bytes(png[off..off + 4].try_into().unwrap()) as usize;
        let chunk_type: [u8; 4] = png[off + 4..off + 8].try_into().unwrap();
        let data_start = off + 8;
        let data_end = data_start + len;
        let crc_end = data_end + 4;
        assert!(crc_end <= png.len(), "truncated PNG chunk");

        if chunk_type == wanted {
            res.push(png[data_start..data_end].to_vec());
        }
        if &chunk_type == b"IEND" {
            break;
        }
        off = crc_end;
    }
    res
}

fn parse_icyd_record(payload: &[u8]) -> (String, Vec<u8>) {
    assert!(payload.len() >= 1 + 2 + 4, "icYD record too small");
    assert_eq!(payload[0], ICYD_RECORD_VERSION, "unexpected icYD record version");

    let keyword_len = u16::from_le_bytes(payload[1..3].try_into().unwrap()) as usize;
    let keyword_start = 3;
    let keyword_end = keyword_start + keyword_len;
    assert!(payload.len() >= keyword_end + 4, "icYD keyword truncated");

    let keyword = std::str::from_utf8(&payload[keyword_start..keyword_end])
        .expect("icYD keyword utf8")
        .to_string();
    let data_len = u32::from_le_bytes(payload[keyword_end..keyword_end + 4].try_into().unwrap()) as usize;
    let data_start = keyword_end + 4;
    let data_end = data_start + data_len;
    assert!(payload.len() >= data_end, "icYD data truncated");

    (keyword, payload[data_start..data_end].to_vec())
}

fn is_zstd_frame(data: &[u8]) -> bool {
    data.len() >= 4 && data[..4] == ZSTD_FRAME_MAGIC
}

fn make_png_with_ztxt_chunks(chunks: &[(&str, String)]) -> Vec<u8> {
    let mut out = Vec::new();

    let mut encoder = png::Encoder::new(&mut out, 1, 1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);

    for (keyword, text) in chunks {
        encoder.add_ztxt_chunk((*keyword).to_string(), text.clone()).expect("add_ztxt_chunk");
    }

    let mut writer = encoder.write_header().expect("write_header");
    writer.write_image_data(&[0, 0, 0, 0]).expect("write_image_data");
    writer.finish().expect("finish");

    out
}
/*
    fn is_hidden(entry: &walkdir::DirEntry) -> bool {
        entry
            .file_name()
            .to_str()
            .map_or(false, |s| s.starts_with('.'))
    }

                #[test]
                fn test_roundtrip() {
                    let walker = walkdir::WalkDir::new("../sixteencolors-archive").into_iter();
                    let mut num = 0;

                    for entry in walker.filter_entry(|e| !is_hidden(e)) {
                        let entry = entry.unwrap();
                        let path = entry.path();

                        if path.is_dir() {
                            continue;
                        }
                        let extension = path.extension();
                        if extension.is_none() {
                            continue;
                        }
                        let extension = extension.unwrap().to_str();
                        if extension.is_none() {
                            continue;
                        }
                        let extension = extension.unwrap().to_lowercase();

                        let mut found = false;
                        for format in &*crate::FORMATS {
                            if format.get_file_extension() == extension
                                || format.get_alt_extensions().contains(&extension)
                            {
                                found = true;
                            }
                        }
                        if !found {
                            continue;
                        }
                        num += 1;/*
                        if num < 53430 {
                            continue;
                        }*/
                        if let Ok(mut buf) = Buffer::load_buffer(path, true) {
                            let draw = FileFormat::IcyDraw;
                            let bytes = draw.to_bytes(&buf, &AnsiSaveOptionsV2::default()).unwrap();
                            let buf2 = draw
                                .from_bytes(&bytes, None)
                                .unwrap();
                            compare_buffers(&buf, &buf2);
                        }
                    }
                }
*/
/*
    #[test]
    fn test_single() {
        // .into()
        let mut buf = Buffer::load_buffer(
            Path::new("../sixteencolors-archive/1996/moz9604a/SHD-SOFT.ANS"),
            true,
        )
        .unwrap();
        let draw = FileFormat::IcyDraw;
        let bytes = draw.to_bytes(&buf, &AnsiSaveOptionsV2::default()).unwrap();
        let buf2 = draw
            .from_bytes(&bytes, None)
            .unwrap();
        compare_buffers(&buf, &buf2);
    }
*/

#[test]
fn test_empty_buffer() {
    let mut buf = TextBuffer::default();
    buf.set_width(12);
    buf.set_height(23);

    let draw = FileFormat::IcyDraw;
    let bytes = draw.to_bytes(&mut buf, &SaveOptions::default()).unwrap();
    let buf2 = draw.from_bytes(&bytes, None).unwrap().screen.buffer;
    compare_buffers(&buf, &buf2, CompareOptions::ALL);
}

#[test]
fn test_icy_draw_load_syncs_terminal_state_size() {
    let mut buf = TextBuffer::new((300, 453));
    buf.layers[0].set_char((0, 0), AttributedChar::new('X', TextAttribute::default()));

    let draw = FileFormat::IcyDraw;
    let bytes = draw.to_bytes(&mut buf, &SaveOptions::default()).unwrap();
    let loaded = draw.from_bytes(&bytes, None).unwrap().screen.buffer;

    assert_eq!(loaded.width(), 300);
    assert_eq!(loaded.height(), 453);
    assert_eq!(loaded.terminal_state.width(), loaded.width());
    assert_eq!(loaded.terminal_state.height(), loaded.height());
}

#[test]
fn test_iced_v1_compression_off_writes_uncompressed_records() {
    let mut buf = TextBuffer::new((2, 2));
    buf.layers[0].set_char((0, 0), AttributedChar::new('A', TextAttribute::default()));

    let mut options = SaveOptions::default();
    options.format = icy_engine::FormatOptions::IcyDraw(icy_engine::IcyDrawFormatOptions {
        skip_thumbnail: false,
        compress: false,
    });

    let draw = FileFormat::IcyDraw;
    let bytes = draw.to_bytes(&mut buf, &options).unwrap();

    // Sanity: roundtrip must still work.
    let buf2 = draw.from_bytes(&bytes, None).unwrap().screen.buffer;
    compare_buffers(&buf, &buf2, CompareOptions::ALL);

    // Inspect ICED header compression flag.
    let raw_records = extract_png_chunks_by_type(&bytes, ICYD_CHUNK_TYPE);
    assert!(!raw_records.is_empty());

    let mut have_layer = false;
    for payload in raw_records {
        let (keyword, record_bytes) = parse_icyd_record(&payload);
        if keyword == "ICED" {
            assert!(record_bytes.len() >= 3, "ICED header too small");
            // bytes[2] is the compression byte (after u16 version)
            assert_eq!(record_bytes[2], 0, "expected compression NONE");
        }
        if keyword == "LAYER" {
            have_layer = true;
            assert!(!is_zstd_frame(&record_bytes), "LAYER record should be uncompressed");
        }
    }
    assert!(have_layer, "expected at least one LAYER record");
}

#[test]
fn test_iced_v1_compression_on_writes_zstd_records() {
    let mut buf = TextBuffer::new((2, 2));
    buf.layers[0].set_char((0, 0), AttributedChar::new('A', TextAttribute::default()));

    let mut options = SaveOptions::default();
    options.format = icy_engine::FormatOptions::IcyDraw(icy_engine::IcyDrawFormatOptions {
        skip_thumbnail: false,
        compress: true,
    });

    let draw = FileFormat::IcyDraw;
    let bytes = draw.to_bytes(&mut buf, &options).unwrap();

    // Sanity: roundtrip must still work.
    let buf2 = draw.from_bytes(&bytes, None).unwrap().screen.buffer;
    compare_buffers(&buf, &buf2, CompareOptions::ALL);

    let raw_records = extract_png_chunks_by_type(&bytes, ICYD_CHUNK_TYPE);
    assert!(!raw_records.is_empty());

    let mut have_layer = false;
    for payload in raw_records {
        let (keyword, record_bytes) = parse_icyd_record(&payload);
        if keyword == "ICED" {
            assert!(record_bytes.len() >= 3, "ICED header too small");
            assert_eq!(record_bytes[2], 2, "expected compression ZSTD");
        }
        if keyword == "LAYER" {
            have_layer = true;
            assert!(is_zstd_frame(&record_bytes), "LAYER record should be zstd-compressed");
        }
    }
    assert!(have_layer, "expected at least one LAYER record");
}

/// Load a PNG file and return RGBA pixel data + dimensions
fn load_png_as_rgba(path: &std::path::Path) -> (i32, i32, Vec<u8>) {
    use std::io::BufReader;
    let file = std::fs::File::open(path).expect("failed to open PNG");
    let decoder = png::Decoder::new(BufReader::new(file));
    let mut reader = decoder.read_info().expect("failed to read PNG info");

    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).expect("failed to decode PNG frame");
    buf.truncate(info.buffer_size());

    // Convert to RGBA if needed
    let rgba_data = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => {
            let mut rgba = Vec::with_capacity(buf.len() / 3 * 4);
            for chunk in buf.chunks(3) {
                rgba.push(chunk[0]);
                rgba.push(chunk[1]);
                rgba.push(chunk[2]);
                rgba.push(255);
            }
            rgba
        }
        other => panic!("unsupported color type: {:?}", other),
    };

    (info.width as i32, info.height as i32, rgba_data)
}

#[test]
fn test_sixel_layer_roundtrip() {
    // Load a real PNG image for testing
    let png_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../icy_draw/build/linux/128x128.png");

    if !png_path.exists() {
        eprintln!("Skipping test_sixel_layer_roundtrip: test PNG not found at {:?}", png_path);
        return;
    }

    let (width, height, rgba_data) = load_png_as_rgba(&png_path);
    assert_eq!(width, 128);
    assert_eq!(height, 128);
    assert_eq!(rgba_data.len(), (128 * 128 * 4) as usize);

    // Create a buffer with an image layer
    let mut buf = TextBuffer::new((80, 25));
    buf.set_font_dimensions((8, 16).into());

    // Create sixel from the PNG data
    let mut sixel = Sixel::from_data((width, height), 1, 1, rgba_data.clone());
    sixel.position = Position::new(0, 0);

    // Create image layer
    let font_dims = buf.font_dimensions();
    let layer_width = (width + font_dims.width - 1) / font_dims.width;
    let layer_height = (height + font_dims.height - 1) / font_dims.height;

    let mut image_layer = Layer::new("Test Image".to_string(), (layer_width, layer_height));
    image_layer.role = Role::Image;
    image_layer.properties.is_visible = true;
    image_layer.set_offset((5, 3));
    image_layer.sixels.push(sixel);

    buf.layers.push(image_layer);

    // Save with compression
    let mut options = SaveOptions::default();
    options.format = icy_engine::FormatOptions::IcyDraw(icy_engine::IcyDrawFormatOptions {
        skip_thumbnail: false,
        compress: true,
    });

    let draw = FileFormat::IcyDraw;
    let bytes = draw.to_bytes(&mut buf, &options).unwrap();

    // Verify SIXEL chunk is NOT zstd-compressed (PNG is already compressed)
    let raw_records = extract_png_chunks_by_type(&bytes, ICYD_CHUNK_TYPE);
    let mut have_sixel = false;
    for payload in &raw_records {
        let (keyword, record_bytes) = parse_icyd_record(payload);
        if keyword == "SIXEL" {
            have_sixel = true;
            assert!(!is_zstd_frame(&record_bytes), "SIXEL record should NOT be zstd-compressed");
        }
    }
    assert!(have_sixel, "expected a SIXEL chunk in the output");

    // Load back
    let loaded = draw.from_bytes(&bytes, None).unwrap();
    let buf2 = loaded.screen.buffer;

    // Verify we have the image layer
    assert_eq!(buf2.layers.len(), 2, "expected 2 layers (default + image)");

    let loaded_image_layer = buf2.layers.iter().find(|l| l.role == Role::Image);
    assert!(loaded_image_layer.is_some(), "expected an image layer");

    let loaded_layer = loaded_image_layer.unwrap();
    assert_eq!(loaded_layer.properties.title, "Test Image");
    assert_eq!(loaded_layer.offset().x, 5);
    assert_eq!(loaded_layer.offset().y, 3);
    assert!(loaded_layer.properties.is_visible);

    // Verify sixel data
    assert_eq!(loaded_layer.sixels.len(), 1);
    let loaded_sixel = &loaded_layer.sixels[0];
    assert_eq!(loaded_sixel.width(), 128);
    assert_eq!(loaded_sixel.height(), 128);
    assert_eq!(loaded_sixel.picture_data.len(), rgba_data.len());
    assert_eq!(loaded_sixel.picture_data, rgba_data, "sixel pixel data mismatch");
}

#[test]
fn test_sixel_layer_roundtrip_uncompressed() {
    // Same test but with compression OFF
    let png_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../icy_draw/build/linux/128x128.png");

    if !png_path.exists() {
        return;
    }

    let (width, height, rgba_data) = load_png_as_rgba(&png_path);

    let mut buf = TextBuffer::new((80, 25));
    buf.set_font_dimensions((8, 16).into());

    let mut sixel = Sixel::from_data((width, height), 1, 1, rgba_data.clone());
    sixel.position = Position::new(0, 0);

    let font_dims = buf.font_dimensions();
    let layer_width = (width + font_dims.width - 1) / font_dims.width;
    let layer_height = (height + font_dims.height - 1) / font_dims.height;

    let mut image_layer = Layer::new("Uncompressed Image".to_string(), (layer_width, layer_height));
    image_layer.role = Role::Image;
    image_layer.sixels.push(sixel);

    buf.layers.push(image_layer);

    // Save WITHOUT compression
    let mut options = SaveOptions::default();
    options.format = icy_engine::FormatOptions::IcyDraw(icy_engine::IcyDrawFormatOptions {
        skip_thumbnail: false,
        compress: false,
    });

    let draw = FileFormat::IcyDraw;
    let bytes = draw.to_bytes(&mut buf, &options).unwrap();

    // Load back and verify
    let loaded = draw.from_bytes(&bytes, None).unwrap();
    let buf2 = loaded.screen.buffer;

    let loaded_layer = buf2.layers.iter().find(|l| l.role == Role::Image).unwrap();
    let loaded_sixel = &loaded_layer.sixels[0];

    assert_eq!(loaded_sixel.width(), 128);
    assert_eq!(loaded_sixel.height(), 128);
    assert_eq!(loaded_sixel.picture_data, rgba_data, "sixel pixel data mismatch (uncompressed)");
}

#[test]
#[ignore = "ICY format changed - extended palette encoding incompatible with V1"]
fn test_rgb_serialization_bug() {
    let mut buf = TextBuffer::new((2, 2));
    let fg = buf.palette.insert_color(Color::new(82, 85, 82));
    buf.layers[0].set_char(
        (0, 0),
        AttributedChar {
            ch: '²',
            attribute: TextAttribute::new(fg, 0),
        },
    );
    let bg = buf.palette.insert_color(Color::new(182, 185, 82));
    buf.layers[0].set_char(
        (1, 0),
        AttributedChar {
            ch: '²',
            attribute: TextAttribute::new(fg, bg),
        },
    );

    let draw = FileFormat::IcyDraw;
    let bytes = draw.to_bytes(&mut buf, &SaveOptions::default()).unwrap();
    let buf2 = draw.from_bytes(&bytes, None).unwrap().screen.buffer;
    compare_buffers(&buf, &buf2, CompareOptions::ALL);
}

#[test]
#[ignore = "ICY format changed - extended palette encoding incompatible with V1"]
fn test_rgb_serialization_bug_2() {
    // was a bug in compare_buffers, but having more test doesn't hurt.
    let mut buf = TextBuffer::new((2, 2));

    let _ = buf.palette.insert_color(Color::new(1, 2, 3));
    let fg = buf.palette.insert_color(Color::new(4, 5, 6)); // 17
    let bg = buf.palette.insert_color(Color::new(7, 8, 9)); // 18
    buf.layers[0].set_char(
        (0, 0),
        AttributedChar {
            ch: 'A',
            attribute: TextAttribute::new(fg, bg),
        },
    );

    let draw = FileFormat::IcyDraw;
    let bytes = draw.to_bytes(&mut buf, &SaveOptions::default()).unwrap();
    let buf2 = draw.from_bytes(&bytes, None).unwrap().screen.buffer;
    compare_buffers(&buf, &buf2, CompareOptions::ALL);
}

#[test]
#[ignore = "ICY format changed - extended palette encoding incompatible with V1"]
fn test_nonstandard_palettes() {
    // was a bug in compare_buffers, but having more test doesn't hurt.
    let mut buf = TextBuffer::new((2, 2));
    buf.palette.set_color(9, Color::new(4, 5, 6));
    buf.palette.set_color(10, Color::new(7, 8, 9));

    buf.layers[0].set_char(
        (0, 0),
        AttributedChar {
            ch: 'A',
            attribute: TextAttribute::new(9, 10),
        },
    );

    let draw = FileFormat::IcyDraw;
    let bytes = draw.to_bytes(&mut buf, &SaveOptions::default()).unwrap();
    let buf2 = draw.from_bytes(&bytes, None).unwrap().screen.buffer;

    compare_buffers(&buf, &buf2, CompareOptions::ALL);
}

#[test]
fn test_fg_switch() {
    // was a bug in compare_buffers, but having more test doesn't hurt.
    let mut buf = TextBuffer::new((2, 1));
    let mut attribute = TextAttribute::new(1, 1);
    attribute.set_is_bold(true);
    buf.layers[0].set_char((0, 0), AttributedChar { ch: 'A', attribute });
    buf.layers[0].set_char(
        (1, 0),
        AttributedChar {
            ch: 'A',
            attribute: TextAttribute::new(2, 1),
        },
    );

    let draw = FileFormat::IcyDraw;
    let bytes = draw.to_bytes(&mut buf, &SaveOptions::default()).unwrap();
    let buf2 = draw.from_bytes(&bytes, None).unwrap().screen.buffer;

    compare_buffers(&buf, &buf2, CompareOptions::ALL);
}

#[test]
fn test_escape_char() {
    let mut buf = TextBuffer::new((2, 2));
    buf.layers[0].set_char(
        (0, 0),
        AttributedChar {
            ch: '\x1b',
            attribute: TextAttribute::default(),
        },
    );

    let draw = FileFormat::IcyDraw;
    let bytes = draw.to_bytes(&mut buf, &SaveOptions::default()).unwrap();
    let buf2 = draw.from_bytes(&bytes, None).unwrap().screen.buffer;
    compare_buffers(&buf, &buf2, CompareOptions::ALL);
}

#[test]
fn test_rejects_newer_iced_versions() {
    use base64::{engine::general_purpose, Engine};

    // ICED header (v2) with v1-sized payload (21 bytes) so only the version triggers the error.
    let mut header = Vec::new();
    header.extend(u16::to_le_bytes(2)); // version
    header.extend(u32::to_le_bytes(0)); // type
    header.extend(u16::to_le_bytes(0)); // buffer_type
    header.push(0); // ice_mode
    header.push(0); // palette_mode
    header.push(0); // font_mode
    header.extend(u32::to_le_bytes(80)); // width
    header.extend(u32::to_le_bytes(25)); // height
    header.push(8); // font_width
    header.push(16); // font_height

    assert_eq!(header.len(), 21);

    let iced_text = general_purpose::STANDARD.encode(&header);
    let png = make_png_with_ztxt_chunks(&[("ICED", iced_text), ("END", String::new())]);

    let draw = FileFormat::IcyDraw;
    assert!(draw.from_bytes(&png, None).is_err());
}

#[test]
fn test_tag_roundtrip_short_and_long() {
    use icy_engine::{attribute, AttributeColor, Position, Tag, TagPlacement, TagRole};

    let mut buf = TextBuffer::default();

    // Short tag (palette colors)
    let mut short_attr = TextAttribute::new(12, 3);
    short_attr.attr = attribute::UNDERLINE;
    short_attr.set_font_page(7);

    buf.tags.push(Tag {
        is_enabled: true,
        preview: "PREVIEW".to_string(),
        replacement_value: "REPL".to_string(),
        position: Position::new(1, 2),
        length: 0,
        alignment: std::fmt::Alignment::Left,
        tag_placement: TagPlacement::InText,
        tag_role: TagRole::Displaycode,
        attribute: short_attr,
    });

    // Long tag with RGB color (forces extended encoding)
    let mut long_attr = TextAttribute::default();
    long_attr.attr = attribute::BOLD;
    long_attr.set_foreground_color(AttributeColor::Rgb(0x11, 0x22, 0x33));
    long_attr.set_background_color(AttributeColor::Rgb(0x55, 0x66, 0x77));
    long_attr.set_font_page(3);

    buf.tags.push(Tag {
        is_enabled: false,
        preview: "X".repeat(10),
        replacement_value: "https://example.invalid".to_string(),
        position: Position::new(5, 6),
        length: 10,
        alignment: std::fmt::Alignment::Center,
        tag_placement: TagPlacement::WithGotoXY,
        tag_role: TagRole::Hyperlink,
        attribute: long_attr,
    });

    let draw = FileFormat::IcyDraw;
    let bytes = draw.to_bytes(&mut buf, &SaveOptions::default()).unwrap();
    let buf2 = draw.from_bytes(&bytes, None).unwrap().screen.buffer;

    assert_eq!(buf2.tags.len(), 2);

    // Compare fields explicitly (Tag doesn't implement PartialEq)
    for (a, b) in buf.tags.iter().zip(buf2.tags.iter()) {
        assert_eq!(a.is_enabled, b.is_enabled);
        assert_eq!(a.preview, b.preview);
        assert_eq!(a.replacement_value, b.replacement_value);
        assert_eq!(a.position, b.position);
        assert_eq!(a.length, b.length);
        assert_eq!(a.alignment, b.alignment);
        assert_eq!(a.tag_placement, b.tag_placement);
        assert_eq!(a.tag_role, b.tag_role);
        assert_eq!(a.attribute.attr, b.attribute.attr);
        assert_eq!(a.attribute.foreground_color(), b.attribute.foreground_color());
        assert_eq!(a.attribute.background_color(), b.attribute.background_color());
        assert_eq!(a.attribute.font_page(), b.attribute.font_page());
    }
}

#[test]
#[ignore = "ICY format changed - short version removed"]
fn test_layer_continuation_resume_is_y_based_not_line_count() {
    let mut buf = TextBuffer::default();
    buf.set_size((1000, 400));
    buf.layers[0].set_size((1000, 400));

    let attr = TextAttribute::new(7, 0);
    buf.layers[0].set_char((0, 0), AttributedChar { ch: 'A', attribute: attr });
    buf.layers[0].set_char((0, 300), AttributedChar { ch: 'B', attribute: attr });

    let draw = FileFormat::IcyDraw;
    let bytes = draw.to_bytes(&mut buf, &SaveOptions::default()).unwrap();
    let buf2 = draw.from_bytes(&bytes, None).unwrap().screen.buffer;

    assert_eq!(buf2.width(), 1000);
    assert_eq!(buf2.height(), 400);

    let mut found_y: Option<i32> = None;
    for y in 0..buf2.height() {
        if buf2.layers[0].char_at((0, y).into()).ch == 'B' {
            found_y = Some(y);
            break;
        }
    }

    assert_eq!(found_y, Some(300));
}

#[test]
fn test_fuzz_lite_no_panic_on_corrupt_icy_draw() {
    let mut buf = TextBuffer::default();
    buf.layers[0].set_char(
        (0, 0),
        AttributedChar {
            ch: 'Z',
            attribute: TextAttribute::new(7, 0),
        },
    );

    let draw = FileFormat::IcyDraw;
    let good = draw.to_bytes(&mut buf, &SaveOptions::default()).unwrap();

    let mut cases: Vec<Vec<u8>> = Vec::new();

    // Truncations
    for &cut in &[0usize, 1, 10, good.len() / 2, good.len().saturating_sub(1)] {
        cases.push(good[..cut.min(good.len())].to_vec());
    }

    // Bit flips
    for &idx in &[0usize, 1, good.len() / 2, good.len().saturating_sub(1)] {
        if good.is_empty() {
            continue;
        }
        let mut m = good.clone();
        let i = idx.min(m.len() - 1);
        m[i] ^= 0xFF;
        cases.push(m);
    }

    for data in cases {
        let res = std::panic::catch_unwind(|| draw.from_bytes(&data, None));
        assert!(res.is_ok(), "load panicked for input of len {}", data.len());
    }
}

#[test]
fn test_0_255_chars() {
    let mut buf = TextBuffer::new((2, 2));
    buf.layers[0].set_char(
        (0, 0),
        AttributedChar {
            ch: '\0',
            attribute: TextAttribute::default(),
        },
    );
    buf.layers[0].set_char(
        (0, 1),
        AttributedChar {
            ch: '\u{FF}',
            attribute: TextAttribute::default(),
        },
    );

    let draw = FileFormat::IcyDraw;
    let mut opt = SaveOptions::default();
    opt.preprocess.optimize_colors = false;
    let bytes = draw.to_bytes(&mut buf, &opt).unwrap();
    let buf2 = draw.from_bytes(&bytes, None).unwrap().screen.buffer;

    // Use ignore_invisible_chars since we only set 2 chars in a 2x2 buffer
    let options = CompareOptions {
        compare_palette: true,
        compare_fonts: true,
        ignore_invisible_chars: true,
    };
    compare_buffers(&buf, &buf2, options);
}

#[test]
#[ignore = "ICY format changed - short version removed"]
fn test_too_long_lines() {
    let mut buf = TextBuffer::new((2, 2));
    buf.layers[0].set_char(
        (0, 0),
        AttributedChar {
            ch: '1',
            attribute: TextAttribute::default(),
        },
    );
    buf.layers[0].set_char(
        (0, 1),
        AttributedChar {
            ch: '2',
            attribute: TextAttribute::default(),
        },
    );
    buf.layers[0].lines[0].chars.resize(
        80,
        AttributedChar {
            ch: ' ',
            attribute: TextAttribute::default(),
        },
    );

    let draw = FileFormat::IcyDraw;
    let bytes = draw.to_bytes(&mut buf, &SaveOptions::default()).unwrap();
    let buf2 = draw.from_bytes(&bytes, None).unwrap().screen.buffer;
    compare_buffers(&buf, &buf2, CompareOptions::ALL);
}

#[test]
fn test_space_persistance_buffer() {
    let mut buf = TextBuffer::default();
    buf.layers[0].set_char(
        (0, 0),
        AttributedChar {
            ch: ' ',
            attribute: TextAttribute::default(),
        },
    );

    let draw = FileFormat::IcyDraw;
    let bytes = draw.to_bytes(&mut buf, &SaveOptions::default()).unwrap();
    let buf2 = draw.from_bytes(&bytes, None).unwrap().screen.buffer;
    compare_buffers(&buf, &buf2, CompareOptions::ALL);
}

#[test]
fn test_invisible_layer_bug() {
    let mut buf = TextBuffer::new((1, 1));
    buf.layers.push(Layer::new("test", (1, 1)));
    buf.layers[1].set_char((0, 0), AttributedChar::new('a', TextAttribute::default()));
    buf.layers[0].properties.is_visible = false;
    buf.layers[1].properties.is_visible = false;

    let draw = FileFormat::IcyDraw;
    let bytes = draw.to_bytes(&mut buf, &SaveOptions::default()).unwrap();
    let mut buf2 = draw.from_bytes(&bytes, None).unwrap().screen.buffer;

    compare_buffers(&buf, &buf2, CompareOptions::ALL);
    buf2.layers[0].properties.is_visible = true;
    buf2.layers[1].properties.is_visible = true;
}

/// Test that layers with trailing invisible chars roundtrip correctly.
/// This was a bug where the saver would only write up to the last visible char,
/// but the loader expected data for the full width.
#[test]
fn test_trailing_invisible_chars_roundtrip() {
    let mut buf = TextBuffer::new((80, 25));

    // Only set a few characters at the start - the rest are invisible
    buf.layers[0].set_char((0, 0), AttributedChar::new('H', TextAttribute::default()));
    buf.layers[0].set_char((1, 0), AttributedChar::new('i', TextAttribute::default()));
    // Columns 2-79 are invisible

    // Second line has chars in the middle
    buf.layers[0].set_char((40, 1), AttributedChar::new('X', TextAttribute::default()));
    // Columns 41-79 are invisible

    let draw = FileFormat::IcyDraw;
    let bytes = draw.to_bytes(&mut buf, &SaveOptions::default()).unwrap();
    let buf2 = draw.from_bytes(&bytes, None).unwrap().screen.buffer;

    // Use ignore_invisible_chars since we only set a few chars in an 80x25 buffer
    let options = CompareOptions {
        compare_palette: true,
        compare_fonts: true,
        ignore_invisible_chars: true,
    };
    compare_buffers(&buf, &buf2, options);
}

#[test]
fn test_invisisible_persistance_bug() {
    let mut buf = TextBuffer::new((3, 1));
    buf.layers.push(Layer::new("test", (3, 1)));
    buf.layers[1].set_char((0, 0), AttributedChar::new('a', TextAttribute::default()));
    buf.layers[1].set_char((2, 0), AttributedChar::new('b', TextAttribute::default()));
    buf.layers[0].properties.is_visible = false;
    buf.layers[1].properties.is_visible = false;
    buf.layers[1].properties.has_alpha_channel = true;

    assert_eq!(AttributedChar::invisible(), buf.layers[1].char_at((1, 0).into()).into());

    let draw = FileFormat::IcyDraw;
    let bytes = draw.to_bytes(&mut buf, &SaveOptions::default()).unwrap();
    let mut buf2 = draw.from_bytes(&bytes, None).unwrap().screen.buffer;

    compare_buffers(&buf, &buf2, CompareOptions::ALL);
    buf2.layers[0].properties.is_visible = true;
    buf2.layers[1].properties.is_visible = true;
}
