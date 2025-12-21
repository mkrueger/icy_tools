use std::fmt::Alignment;
use std::io::Cursor;

use icy_sauce::SauceRecord;
use regex::Regex;
use zstd::stream::encode_all as zstd_encode_all;

use crate::{BitFont, Color, Layer, LoadingError, Position, Result, Sixel, Size, TextAttribute, TextBuffer, TextPane, TextScreen};

use super::super::{AnsiSaveOptionsV2, LoadData};

mod constants {
    pub const ICED_VERSION: u16 = 1;
    pub const ICED_HEADER_SIZE: usize = 20;

    /// Compression methods for ICED format (stored in first byte of Type field)
    pub mod compression {
        pub const NONE: u8 = 0;
        pub const ZSTD: u8 = 2;
    }

    /// Sixel image format (stored in second byte of Type field)
    pub mod sixel_format {
        pub const RAW_RGBA: u8 = 0;
        pub const PNG: u8 = 1;
    }

    pub mod layer {
        pub const IS_VISIBLE: u32 = 0b0000_0001;
        pub const POS_LOCK: u32 = 0b0000_0010;
        pub const EDIT_LOCK: u32 = 0b0000_0100;
        pub const HAS_ALPHA: u32 = 0b0000_1000;
        pub const ALPHA_LOCKED: u32 = 0b0001_0000;
    }
}

// Default for new ICED v1 saves.
const DEFAULT_V1_COMPRESSION: u8 = constants::compression::ZSTD;

lazy_static::lazy_static! {
    static ref LAYER_CONTINUE_REGEX: Regex = Regex::new(r"LAYER_(\d+)~(\d+)").unwrap();
}

const PNG_SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
const ICYD_CHUNK_TYPE: [u8; 4] = *b"icYD";
const ICYD_RECORD_VERSION: u8 = 1;

// Safety/robustness limit for a single decompressed record payload.
// This prevents malicious files from causing unbounded allocations.
const MAX_DECOMPRESSED_RECORD_SIZE: usize = 256 * 1024 * 1024;

fn build_icyd_record(keyword: &str, data: &[u8]) -> Result<Vec<u8>> {
    let keyword_bytes = keyword.as_bytes();
    let keyword_len: u16 = keyword_bytes.len().try_into().map_err(|_| crate::EngineError::UnsupportedFormat {
        description: format!("icYD keyword too long: {}", keyword_bytes.len()),
    })?;
    let data_len: u32 = data.len().try_into().map_err(|_| crate::EngineError::UnsupportedFormat {
        description: format!("icYD payload too large: {}", data.len()),
    })?;

    let mut out = Vec::with_capacity(1 + 2 + keyword_bytes.len() + 4 + data.len());
    out.push(ICYD_RECORD_VERSION);
    out.extend(u16::to_le_bytes(keyword_len));
    out.extend(keyword_bytes);
    out.extend(u32::to_le_bytes(data_len));
    out.extend(data);
    Ok(out)
}

fn write_icyd_record<W: std::io::Write>(writer: &mut png::Writer<W>, keyword: &str, data: &[u8]) -> std::result::Result<(), IcedError> {
    let record = build_icyd_record(keyword, data).map_err(|e| IcedError::ErrorEncodingZText(format!("{e}")))?;
    writer
        .write_chunk(png::chunk::ChunkType(ICYD_CHUNK_TYPE), &record)
        .map_err(|e| IcedError::ErrorEncodingZText(format!("{e}")))?;
    Ok(())
}

/// Compresses data and writes it as an icYD chunk
fn write_compressed_chunk<W: std::io::Write>(writer: &mut png::Writer<W>, keyword: &str, data: &[u8]) -> std::result::Result<(), IcedError> {
    let compressed = match DEFAULT_V1_COMPRESSION {
        constants::compression::NONE => data.to_vec(),
        constants::compression::ZSTD => {
            zstd_encode_all(Cursor::new(data), 3).map_err(|e| IcedError::ErrorEncodingZText(format!("zstd compression failed: {e}")))?
        }
        other => return Err(IcedError::ErrorEncodingZText(format!("unsupported compression id {other}"))),
    };

    let record: Vec<u8> = build_icyd_record(keyword, &compressed).map_err(|e| IcedError::ErrorEncodingZText(format!("{e}")))?;
    writer
        .write_chunk(png::chunk::ChunkType(ICYD_CHUNK_TYPE), &record)
        .map_err(|e| IcedError::ErrorEncodingZText(format!("{e}")))?;
    Ok(())
}

/// Encode sixel picture data as PNG
fn encode_sixel_as_png(sixel: &Sixel) -> std::result::Result<Vec<u8>, IcedError> {
    let width = sixel.width() as u32;
    let height = sixel.height() as u32;

    let mut png_data = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png_data, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);

        let mut writer = encoder
            .write_header()
            .map_err(|e| IcedError::ErrorEncodingZText(format!("PNG header failed: {e}")))?;
        writer
            .write_image_data(&sixel.picture_data)
            .map_err(|e| IcedError::ErrorEncodingZText(format!("PNG write failed: {e}")))?;
    }

    Ok(png_data)
}

/// Decode PNG data back to RGBA picture data
fn decode_png_to_rgba(png_data: &[u8]) -> Result<(i32, i32, Vec<u8>)> {
    let cursor = Cursor::new(png_data);
    let decoder = png::Decoder::new(cursor);
    let mut reader = decoder.read_info().map_err(|e| crate::EngineError::UnsupportedFormat {
        description: format!("PNG decode failed: {e}"),
    })?;

    // The embedded PNG is expected to be RGBA8.
    let png_info = reader.info();
    if png_info.color_type != png::ColorType::Rgba || png_info.bit_depth != png::BitDepth::Eight {
        return Err(crate::EngineError::UnsupportedFormat {
            description: format!(
                "unsupported embedded PNG format: {:?}/{:?} (expected RGBA/8-bit)",
                png_info.color_type, png_info.bit_depth
            ),
        });
    }

    let buf_size = reader.output_buffer_size().ok_or_else(|| crate::EngineError::UnsupportedFormat {
        description: "PNG output buffer size unknown".to_string(),
    })?;
    let mut buf = vec![0; buf_size];
    let info = reader.next_frame(&mut buf).map_err(|e| crate::EngineError::UnsupportedFormat {
        description: format!("PNG frame read failed: {e}"),
    })?;

    buf.truncate(info.buffer_size());
    Ok((info.width as i32, info.height as i32, buf))
}

fn zstd_decode_all_limited(bytes: &[u8], limit: usize, context: &str) -> Result<Vec<u8>> {
    use std::io::Read;

    let mut decoder = zstd::stream::read::Decoder::new(Cursor::new(bytes)).map_err(|e| crate::EngineError::UnsupportedFormat {
        description: format!("zstd decompression init failed for '{context}': {e}"),
    })?;

    let mut out: Vec<u8> = Vec::new();
    let mut buf = [0u8; 8 * 1024];
    loop {
        let read = decoder.read(&mut buf).map_err(|e| crate::EngineError::UnsupportedFormat {
            description: format!("zstd decompression failed for '{context}': {e}"),
        })?;
        if read == 0 {
            break;
        }
        if out.len().saturating_add(read) > limit {
            return Err(crate::EngineError::UnsupportedFormat {
                description: format!("decompressed record too large for '{context}' (limit: {limit} bytes)"),
            });
        }
        out.extend_from_slice(&buf[..read]);
    }
    Ok(out)
}

fn extract_png_chunks_by_type(png: &[u8], wanted: [u8; 4]) -> Result<Vec<Vec<u8>>> {
    if png.len() < PNG_SIGNATURE.len() || png[..PNG_SIGNATURE.len()] != PNG_SIGNATURE {
        return Err(LoadingError::InvalidPng("invalid PNG signature".to_string()).into());
    }

    let mut res = Vec::new();
    let mut off = PNG_SIGNATURE.len();
    while off + 8 <= png.len() {
        let len = u32::from_be_bytes(png[off..off + 4].try_into().unwrap()) as usize;
        let chunk_type: [u8; 4] = png[off + 4..off + 8].try_into().unwrap();
        let data_start = off + 8;
        let data_end = data_start + len;
        let crc_end = data_end + 4;
        if crc_end > png.len() {
            return Err(LoadingError::InvalidPng("truncated PNG chunk".to_string()).into());
        }

        if chunk_type == wanted {
            res.push(png[data_start..data_end].to_vec());
        }
        if &chunk_type == b"IEND" {
            break;
        }
        off = crc_end;
    }
    Ok(res)
}

fn parse_icyd_record(payload: &[u8]) -> Result<(String, &[u8])> {
    if payload.len() < 1 + 2 + 4 {
        return Err(crate::EngineError::UnsupportedFormat {
            description: "icYD record too small".to_string(),
        });
    }
    if payload[0] != ICYD_RECORD_VERSION {
        return Err(crate::EngineError::UnsupportedFormat {
            description: format!("unsupported icYD record version {}", payload[0]),
        });
    }
    let keyword_len = u16::from_le_bytes(payload[1..3].try_into().unwrap()) as usize;
    let keyword_start = 3;
    let keyword_end = keyword_start + keyword_len;
    if payload.len() < keyword_end + 4 {
        return Err(crate::EngineError::UnsupportedFormat {
            description: "icYD record truncated (keyword)".to_string(),
        });
    }
    let keyword = std::str::from_utf8(&payload[keyword_start..keyword_end]).map_err(|e| crate::EngineError::UnsupportedFormat {
        description: format!("icYD keyword not UTF-8: {e}"),
    })?;
    let data_len = u32::from_le_bytes(payload[keyword_end..keyword_end + 4].try_into().unwrap()) as usize;
    let data_start = keyword_end + 4;
    let data_end = data_start + data_len;
    if payload.len() < data_end {
        return Err(crate::EngineError::UnsupportedFormat {
            description: "icYD record truncated (data)".to_string(),
        });
    }
    Ok((keyword.to_string(), &payload[data_start..data_end]))
}

fn process_icy_draw_v1_decoded_chunk(
    keyword: &str,
    bytes: &[u8],
    result: &mut TextBuffer,
    compression: &mut u8,
    sixel_format: &mut u8,
    sauce_opt: &mut Option<SauceRecord>,
) -> Result<bool> {
    match keyword {
        "END" => return Ok(false),
        "ICED" => {
            if bytes.len() < 2 {
                return Err(crate::EngineError::UnsupportedFormat {
                    description: "ICED header too small".to_string(),
                });
            }

            let version = u16::from_le_bytes([bytes[0], bytes[1]]);
            if version != 1 {
                return Err(crate::EngineError::UnsupportedFormat {
                    description: format!("unsupported ICED version {} (expected: 1)", version),
                });
            }
            if bytes.len() != constants::ICED_HEADER_SIZE {
                return Err(crate::EngineError::UnsupportedFormat {
                    description: format!("unsupported ICED v1 header size {}", bytes.len()),
                });
            }

            let mut o: usize = 2; // skip version

            // Read Type field: [compression: u8][sixel_format: u8][reserved: u16]
            *compression = bytes[o];
            *sixel_format = bytes[o + 1];
            o += 4; // skip full type field
            if bytes.len() < o + 2 {
                return Err(crate::EngineError::OutOfBounds { offset: o + 2 });
            }
            let buffer_type = u16::from_le_bytes(bytes[o..(o + 2)].try_into().unwrap());
            o += 2;
            result.buffer_type = crate::BufferType::from_byte(buffer_type as u8);
            if bytes.len() < o + 3 {
                return Err(crate::EngineError::OutOfBounds { offset: o + 3 });
            }
            let ice_mode = bytes[o];
            o += 1;
            result.ice_mode = crate::IceMode::from_byte(ice_mode);

            let font_mode = bytes[o];
            o += 1;
            result.font_mode = crate::FontMode::from_byte(font_mode);

            if bytes.len() < o + 8 {
                return Err(crate::EngineError::OutOfBounds { offset: o + 8 });
            }
            let width_u32 = u32::from_le_bytes(bytes[o..(o + 4)].try_into().unwrap());
            o += 4;
            let height_u32 = u32::from_le_bytes(bytes[o..(o + 4)].try_into().unwrap());
            o += 4;

            let width: i32 = i32::try_from(width_u32).map_err(|_| crate::EngineError::UnsupportedFormat {
                description: format!("ICED width out of range: {width_u32}"),
            })?;
            let height: i32 = i32::try_from(height_u32).map_err(|_| crate::EngineError::UnsupportedFormat {
                description: format!("ICED height out of range: {height_u32}"),
            })?;
            result.set_size((width, height));

            if bytes.len() >= constants::ICED_HEADER_SIZE {
                if bytes.len() < o + 2 {
                    return Err(crate::EngineError::OutOfBounds { offset: o + 2 });
                }
                let font_width = bytes[o] as i32;
                o += 1;
                let font_height = bytes[o] as i32;
                result.set_font_dimensions((font_width, font_height).into());
            }
        }

        "PALETTE" => {
            result.palette = crate::FileFormat::Palette(crate::PaletteFormat::Ice).load_palette(bytes)?;
        }

        "SAUCE" => {
            if let Some(sauce) = SauceRecord::from_bytes(bytes)? {
                super::super::apply_sauce_to_buffer(result, &sauce);
                *sauce_opt = Some(sauce);
            }
        }

        "TAG" => {
            if bytes.len() < 2 {
                return Err(crate::EngineError::OutOfBounds { offset: 2 });
            }
            let mut cur = &bytes[..];
            let tag_len = u16::from_le_bytes(cur[..2].try_into().unwrap());
            cur = &cur[2..];
            for _ in 0..tag_len {
                let (preview, used) = read_utf8_encoded_string(cur)?;
                cur = &cur[used..];
                let (replacement_value, used) = read_utf8_encoded_string(cur)?;
                cur = &cur[used..];

                // x:4 + y:4 + length:2 + is_enabled:1 + alignment:1 + tag_placement:1 + tag_role:1 = 14 bytes
                if cur.len() < 14 {
                    return Err(crate::EngineError::OutOfBounds { offset: 14 });
                }
                let x = i32::from_le_bytes(cur[..4].try_into().unwrap());
                cur = &cur[4..];
                let y = i32::from_le_bytes(cur[..4].try_into().unwrap());
                cur = &cur[4..];
                let length = u16::from_le_bytes(cur[..2].try_into().unwrap()) as usize;
                cur = &cur[2..];
                let is_enabled = cur[0] == 1;
                cur = &cur[1..];

                let alignment = match cur[0] {
                    0 => Alignment::Left,
                    1 => Alignment::Center,
                    2 => Alignment::Right,
                    _ => {
                        return Err(crate::EngineError::UnsupportedFormat {
                            description: "unsupported alignment".to_string(),
                        });
                    }
                };
                cur = &cur[1..];

                let tag_placement = match cur[0] {
                    0 => crate::TagPlacement::InText,
                    1 => crate::TagPlacement::WithGotoXY,
                    _ => {
                        return Err(crate::EngineError::UnsupportedFormat {
                            description: "unsupported tag placement".to_string(),
                        });
                    }
                };
                cur = &cur[1..];

                let tag_role = match cur[0] {
                    0 => crate::TagRole::Displaycode,
                    1 => crate::TagRole::Hyperlink,
                    _ => {
                        return Err(crate::EngineError::UnsupportedFormat {
                            description: "unsupported tag role".to_string(),
                        });
                    }
                };
                cur = &cur[1..];

                let (rest, text_attr) = TextAttribute::decode_attribute(cur);
                cur = rest;
                result.tags.push(crate::Tag {
                    preview,
                    replacement_value,
                    position: Position::new(x, y),
                    length,
                    is_enabled,
                    alignment,
                    tag_placement,
                    tag_role,
                    attribute: text_attr,
                });
            }
        }

        "FONT" => {
            if bytes.is_empty() {
                return Err(crate::EngineError::OutOfBounds { offset: 1 });
            }
            let mut o: usize = 0;
            let font_slot = bytes[o];
            o += 1;
            let (font_name, size) = read_utf8_encoded_string(&bytes[o..])?;
            o += size;
            let font = BitFont::from_bytes(font_name, &bytes[o..])?;
            result.set_font(font_slot, font);
        }

        "LAYER" => {
            let mut o: usize = 0;

            // Read layer title
            let (title, size) = read_utf8_encoded_string(&bytes[o..])?;
            o += size;

            // Bounds check for fixed fields: role(1) + reserved(4) + mode(1) + color(4) + flags(4) + offset(8) = 22 bytes
            if bytes.len() < o + 22 {
                return Err(crate::EngineError::OutOfBounds { offset: o + 22 });
            }

            // Read role (0 = Normal, 1 = Image)
            let role_byte = bytes[o];
            o += 1;
            let role = if role_byte == 1 { crate::Role::Image } else { crate::Role::Normal };

            // Skip reserved bytes (4 bytes)
            o += 4;

            // Read mode
            let mode = match bytes[o] {
                1 => crate::Mode::Chars,
                2 => crate::Mode::Attributes,
                _ => crate::Mode::Normal,
            };
            o += 1;

            // Read color (RGBA, where A=0xFF means color is set)
            let r = bytes[o];
            let g = bytes[o + 1];
            let b = bytes[o + 2];
            let a = bytes[o + 3];
            o += 4;
            let color = if a == 0xFF { Some(Color::new(r, g, b)) } else { None };

            // Read flags
            let flags = u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
            o += 4;

            // Read offset
            let offset_x = i32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
            o += 4;
            let offset_y = i32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
            o += 4;

            let mut layer = if matches!(role, crate::Role::Image) {
                // Bounds check for sixel header: png_len(8) + width(4) + height(4) + v_scale(4) + h_scale(4) = 24 bytes
                if bytes.len() < o + 24 {
                    return Err(crate::EngineError::OutOfBounds { offset: o + 24 });
                }

                // Read sixel data
                let png_len = u64::from_le_bytes(bytes[o..o + 8].try_into().unwrap()) as usize;
                o += 8;
                let width = i32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
                o += 4;
                let height = i32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
                o += 4;
                let vertical_scale = i32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
                o += 4;
                let horizontal_scale = i32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
                o += 4;

                // Bounds check for PNG data
                if bytes.len() < o + png_len {
                    return Err(crate::EngineError::OutOfBounds { offset: o + png_len });
                }

                let png_data = &bytes[o..o + png_len];
                let (_, _, picture_data) = decode_png_to_rgba(png_data)?;

                let mut sixel = Sixel::from_data((width, height), vertical_scale, horizontal_scale, picture_data);
                sixel.position = Position::new(0, 0);

                let mut layer = Layer::new(title.clone(), (0, 0));
                layer.role = role;
                layer.sixels.push(sixel);
                layer
            } else {
                // Bounds check for char layer header: transparency(1) + width(4) + height(4) = 9 bytes
                if bytes.len() < o + 9 {
                    return Err(crate::EngineError::OutOfBounds { offset: o + 9 });
                }

                // Read char layer data
                let transparency = bytes[o];
                o += 1;
                let width = i32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
                o += 4;
                let height = i32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
                o += 4;

                let mut layer = Layer::new(title.clone(), (width, height));
                layer.transparency = transparency;

                let mut cur = &bytes[o..];
                for y in 0..height {
                    for x in 0..width {
                        if cur.len() < 4 {
                            return Err(crate::EngineError::OutOfBounds { offset: bytes.len() });
                        }
                        let ch = u32::from_le_bytes(cur[0..4].try_into().unwrap());
                        cur = &cur[4..];
                        let (rest, attribute) = TextAttribute::decode_attribute(cur);
                        cur = rest;

                        layer.set_char(
                            Position::new(x, y),
                            crate::AttributedChar::new(char::from_u32(ch).unwrap_or('\u{FFFD}'), attribute),
                        );
                    }
                }
                layer
            };

            // Set layer properties
            layer.properties.title = title;
            layer.properties.mode = mode;
            layer.properties.color = color;
            layer.properties.is_visible = (flags & constants::layer::IS_VISIBLE) != 0;
            layer.properties.is_locked = (flags & constants::layer::EDIT_LOCK) != 0;
            layer.properties.is_position_locked = (flags & constants::layer::POS_LOCK) != 0;
            layer.properties.has_alpha_channel = (flags & constants::layer::HAS_ALPHA) != 0;
            layer.properties.is_alpha_channel_locked = (flags & constants::layer::ALPHA_LOCKED) != 0;
            layer.set_offset((offset_x, offset_y));

            result.layers.push(layer);
        }

        text => {
            log::warn!("unsupported chunk {text}");
            return Ok(true);
        }
    }

    Ok(true)
}

pub(crate) fn save_icy_draw(buf: &TextBuffer, options: &AnsiSaveOptionsV2) -> Result<Vec<u8>> {
    let mut png_bytes = Vec::new();

    let mut first_line = 0;
    let mut last_line = 0;

    let font_dims = buf.font_dimensions();
    // Absolute fast path for IcyDraw autosave: no thumbnail rendering.
    let fast_save = options.skip_thumbnail;

    let (width, height, image_empty) = if fast_save {
        (1, 1, true)
    } else {
        let mut width = buf.width() * font_dims.width;

        while first_line < buf.height() {
            if !buf.is_line_empty(first_line) {
                break;
            }
            first_line += 1;
        }

        last_line = (first_line + MAX_LINES).min(buf.line_count().max(buf.height()));
        let mut height = (last_line - first_line) * font_dims.height;

        let image_empty = width == 0 || height == 0;
        if image_empty {
            width = 1;
            height = 1;
        }

        (width, height, image_empty)
    };

    let mut encoder: png::Encoder<'_, &mut Vec<u8>> = png::Encoder::new(&mut png_bytes, width as u32, height as u32); // Width is 2 pixels and height is 1.
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);

    // The PNG preview is not the bottleneck for the fast-save path (it's 1x1), but this keeps
    // encoding overhead minimal and predictable.
    if fast_save {
        encoder.set_compression(png::Compression::Fastest);
    }

    let mut writer = encoder.write_header()?;

    {
        let mut result = vec![constants::ICED_VERSION as u8, (constants::ICED_VERSION >> 8) as u8];
        // Type field: [compression: u8][sixel_format: u8][reserved: u16]
        result.push(DEFAULT_V1_COMPRESSION); // compression method
        result.push(constants::sixel_format::PNG); // sixel format
        result.extend([0, 0]); // reserved
        // Modes
        result.extend(u16::to_le_bytes(buf.buffer_type.to_byte() as u16));
        result.push(buf.ice_mode.to_byte());
        result.push(buf.font_mode.to_byte());

        result.extend(u32::to_le_bytes(buf.width() as u32));
        result.extend(u32::to_le_bytes(buf.height() as u32));

        result.push(buf.font_dimensions().width as u8);
        result.push(buf.font_dimensions().height as u8);

        write_icyd_record(&mut writer, "ICED", &result)?;
    }

    if let Some(sauce) = &options.save_sauce {
        let mut sauce_vec: Vec<u8> = Vec::new();
        sauce.write(&mut sauce_vec)?;
        write_compressed_chunk(&mut writer, "SAUCE", &sauce_vec)?;
    }

    if !buf.palette.is_default() {
        let pal_data = buf.palette.export_palette(&crate::FileFormat::Palette(crate::PaletteFormat::Ice)).unwrap();
        write_compressed_chunk(&mut writer, "PALETTE", &pal_data)?;
    }

    for (slot, v) in buf.font_iter() {
        let mut font_data: Vec<u8> = Vec::new();
        font_data.push(*slot as u8);
        write_utf8_encoded_string(&mut font_data, &v.name());
        font_data.extend(v.to_psf2_bytes().unwrap());

        write_compressed_chunk(&mut writer, &format!("FONT"), &font_data)?;
    }

    for layer in &buf.layers {
        // Build layer data into a buffer first
        let mut layer_data = Vec::new();
        write_utf8_encoded_string(&mut layer_data, &layer.properties.title);

        match layer.role {
            crate::Role::Image => layer_data.push(1),
            _ => layer_data.push(0),
        }

        // Some extra bytes not yet used
        layer_data.extend([0, 0, 0, 0]);

        let mode = match layer.properties.mode {
            crate::Mode::Normal => 0,
            crate::Mode::Chars => 1,
            crate::Mode::Attributes => 2,
        };
        layer_data.push(mode);

        if let Some(color) = &layer.properties.color {
            let (r, g, b) = color.clone().rgb();
            layer_data.push(r);
            layer_data.push(g);
            layer_data.push(b);
            layer_data.push(0xFF);
        } else {
            layer_data.extend([0, 0, 0, 0]);
        }

        let mut flags = 0;
        if layer.properties.is_visible {
            flags |= constants::layer::IS_VISIBLE;
        }
        if layer.properties.is_locked {
            flags |= constants::layer::EDIT_LOCK;
        }
        if layer.properties.is_position_locked {
            flags |= constants::layer::POS_LOCK;
        }
        if layer.properties.has_alpha_channel {
            flags |= constants::layer::HAS_ALPHA;
        }
        if layer.properties.is_alpha_channel_locked {
            flags |= constants::layer::ALPHA_LOCKED;
        }
        layer_data.extend(u32::to_le_bytes(flags));

        layer_data.extend(i32::to_le_bytes(layer.offset().x));
        layer_data.extend(i32::to_le_bytes(layer.offset().y));

        if matches!(layer.role, crate::Role::Image) {
            let sixel = &layer.sixels[0];
            // Encode sixel as PNG for better compression
            let png_data = encode_sixel_as_png(sixel)?;
            layer_data.extend(u64::to_le_bytes(png_data.len() as u64));
            layer_data.extend(i32::to_le_bytes(sixel.width()));
            layer_data.extend(i32::to_le_bytes(sixel.height()));
            layer_data.extend(i32::to_le_bytes(sixel.vertical_scale));
            layer_data.extend(i32::to_le_bytes(sixel.horizontal_scale));
            layer_data.extend(&png_data);
        } else {
            layer_data.push(layer.transparency);
            layer_data.extend(i32::to_le_bytes(layer.width()));
            layer_data.extend(i32::to_le_bytes(layer.height()));
            // Build char data
            for y in 0..layer.height() {
                for x in 0..layer.width() {
                    let ch = layer.char_at((x, y).into());
                    layer_data.extend((ch.ch as u32).to_le_bytes());
                    TextAttribute::encode_attribute(&ch.attribute, &mut layer_data);
                }
            }
        }
        // Write the layer data with the configured ICED compression
        let keyword = format!("LAYER");
        write_compressed_chunk(&mut writer, &keyword, &layer_data)?;
    }

    if !buf.tags.is_empty() {
        let mut data = Vec::new();
        data.extend(u16::to_le_bytes(buf.tags.len() as u16));
        for tag in &buf.tags {
            write_utf8_encoded_string(&mut data, &tag.preview);
            write_utf8_encoded_string(&mut data, &tag.replacement_value);
            data.extend(i32::to_le_bytes(tag.position.x as i32));
            data.extend(i32::to_le_bytes(tag.position.y as i32));
            data.extend(u16::to_le_bytes(tag.length as u16));
            if tag.is_enabled {
                data.push(1);
            } else {
                data.push(0);
            }
            match tag.alignment {
                Alignment::Left => data.push(0),
                Alignment::Center => data.push(1),
                Alignment::Right => data.push(2),
            }
            match tag.tag_placement {
                crate::TagPlacement::InText => data.push(0),
                crate::TagPlacement::WithGotoXY => data.push(1),
            }
            match tag.tag_role {
                crate::TagRole::Displaycode => data.push(0),
                crate::TagRole::Hyperlink => data.push(1),
            }
            TextAttribute::encode_attribute(&tag.attribute, &mut data);
        }

        write_compressed_chunk(&mut writer, "TAG", &data)?;
    }

    write_icyd_record(&mut writer, "END", &[])?;

    if image_empty {
        writer.write_image_data(&[0, 0, 0, 0])?;
    } else {
        let (_, data) = buf.render_to_rgba(
            &crate::Rectangle {
                start: Position::new(0, first_line),
                size: Size::new(buf.width(), last_line - first_line),
            }
            .into(),
            false,
        );
        writer.write_image_data(&data)?;
    }
    writer.finish()?;

    Ok(png_bytes)
}

pub(crate) fn load_icy_draw(data: &[u8], _load_data_opt: Option<&LoadData>) -> Result<(TextScreen, Option<SauceRecord>)> {
    // Try v1 binary chunks first
    if let Some((screen, sauce_opt)) = load_icy_draw_v1_binary_chunks(data)? {
        return Ok((screen, sauce_opt));
    }

    // Fall back to v0 loader
    super::icy_draw_v0::load_icy_draw_v0(data)
}

fn load_icy_draw_v1_binary_chunks(data: &[u8]) -> Result<Option<(TextScreen, Option<SauceRecord>)>> {
    let mut result = TextBuffer::new((80, 25));
    result.terminal_state.is_terminal_buffer = false;
    result.layers.clear();

    // Compression and sixel format from ICED header
    let mut compression = constants::compression::NONE;
    let mut sixel_format = constants::sixel_format::RAW_RGBA;
    let mut sauce_opt: Option<SauceRecord> = None;

    let raw_records = extract_png_chunks_by_type(data, ICYD_CHUNK_TYPE)?;
    if raw_records.is_empty() {
        return Ok(None);
    }

    // Parse all records first so we can locate ICED and determine compression.
    let mut records: Vec<(String, Vec<u8>)> = Vec::with_capacity(raw_records.len());
    for payload in raw_records {
        let (keyword, bytes) = parse_icyd_record(&payload)?;
        records.push((keyword, bytes.to_vec()));
    }

    // Strict v1 requirement: ICED must be the first record.
    match records.first() {
        Some((first_keyword, _)) if first_keyword == "ICED" => {}
        Some((first_keyword, _)) => {
            return Err(crate::EngineError::UnsupportedFormat {
                description: format!("ICED must be the first icYD record (found '{first_keyword}' first)"),
            });
        }
        None => return Ok(None),
    }

    // Must have ICED to be considered a v1 file.
    let mut iced_bytes_opt: Option<Vec<u8>> = None;
    for (keyword, bytes) in &records {
        if keyword == "ICED" {
            if iced_bytes_opt.is_some() {
                return Err(crate::EngineError::UnsupportedFormat {
                    description: "multiple ICED headers found".to_string(),
                });
            }
            iced_bytes_opt = Some(bytes.clone());
        }
    }
    let Some(iced_bytes) = iced_bytes_opt else {
        return Err(crate::EngineError::UnsupportedFormat {
            description: "icYD records present but ICED header missing".to_string(),
        });
    };

    // Process ICED first to initialize buffer metadata and the compression setting.
    let _ = process_icy_draw_v1_decoded_chunk("ICED", &iced_bytes, &mut result, &mut compression, &mut sixel_format, &mut sauce_opt)?;

    for (keyword, bytes) in records {
        if keyword == "ICED" {
            continue;
        }

        // Decompress data if needed (END is never compressed)
        let decompressed_data: Vec<u8>;
        let actual_bytes: &[u8] = if keyword != "END" {
            match compression {
                constants::compression::NONE => &bytes,
                constants::compression::ZSTD => {
                    decompressed_data = zstd_decode_all_limited(&bytes, MAX_DECOMPRESSED_RECORD_SIZE, &keyword)?;
                    &decompressed_data
                }
                other => {
                    return Err(crate::EngineError::UnsupportedFormat {
                        description: format!("unsupported ICED compression id {other}"),
                    });
                }
            }
        } else {
            &bytes
        };

        let keep_running = process_icy_draw_v1_decoded_chunk(&keyword, actual_bytes, &mut result, &mut compression, &mut sixel_format, &mut sauce_opt)?;
        if !keep_running {
            break;
        }
    }

    Ok(Some((TextScreen::from_buffer(result), sauce_opt)))
}

fn read_utf8_encoded_string(data: &[u8]) -> Result<(String, usize)> {
    if data.len() < 4 {
        return Err(crate::EngineError::OutOfBounds { offset: 4 });
    }

    let size = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;
    let end = 4usize.saturating_add(size);
    if data.len() < end {
        return Err(crate::EngineError::OutOfBounds { offset: end });
    }

    let s = std::str::from_utf8(&data[4..end])
        .map_err(|e| crate::EngineError::UnsupportedFormat {
            description: format!("invalid UTF-8 string: {e}"),
        })?
        .to_string();

    Ok((s, size + 4))
}

fn write_utf8_encoded_string(data: &mut Vec<u8>, s: &str) {
    data.extend(u32::to_le_bytes(s.len() as u32));
    data.extend(s.as_bytes());
}

const MAX_LINES: i32 = 80;

#[derive(Debug, Clone, thiserror::Error)]
pub enum IcedError {
    #[error("Error while encoding ztext chunk: {0}")]
    ErrorEncodingZText(String),
    #[error("Error while parsing font slot: {0}")]
    ErrorParsingFontSlot(String),
}

impl From<IcedError> for crate::EngineError {
    fn from(err: IcedError) -> Self {
        crate::EngineError::Generic(err.to_string())
    }
}
