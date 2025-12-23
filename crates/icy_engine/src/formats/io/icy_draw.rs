use std::fmt::Alignment;
use std::io::Cursor;

use icy_sauce::{CharacterFormat, SauceRecord};
use zstd::stream::encode_all as zstd_encode_all;

use crate::{BitFont, Color, Layer, LayerProperties, Position, Result, Sixel, Size, TextAttribute, TextBuffer, TextPane, TextScreen};

use super::super::{LoadData, SauceBuilder, SaveOptions};

mod constants {
    pub const ICED_VERSION: u16 = 1;
    pub const ICED_HEADER_SIZE: usize = 19; // Version(2) + Type(3) + Modes(4) + Size(8) + FontDims(2)

    /// Compression methods for ICED format (stored in first byte of Type field)
    pub mod compression {
        pub const NONE: u8 = 0;
        pub const ZSTD: u8 = 2;
    }

    pub mod layer {
        pub const IS_VISIBLE: u32 = 0b0000_0001;
        pub const POS_LOCK: u32 = 0b0000_0010;
        pub const EDIT_LOCK: u32 = 0b0000_0100;
        pub const HAS_ALPHA: u32 = 0b0000_1000;
        pub const ALPHA_LOCKED: u32 = 0b0001_0000;
    }
}

// ICED v1 supports optional record compression; ZSTD is the preferred codec.
const PNG_SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
const ICYD_CHUNK_TYPE: [u8; 4] = *b"icYD";
const ICYD_RECORD_VERSION: u8 = 1;

// Safety/robustness limit for a single decompressed record payload.
// This prevents malicious files from causing unbounded allocations.
const MAX_DECOMPRESSED_RECORD_SIZE: usize = 256 * 1024 * 1024;

#[derive(Debug, Clone, thiserror::Error)]
pub enum IcedError {
    #[error("Chunk encoding failed: {0}")]
    ChunkEncodingFailed(String),
    #[error("PNG encoding failed: {0}")]
    PngEncodingFailed(String),
    #[error("PNG decoding failed: {0}")]
    PngDecodingFailed(String),
    #[error("Compression failed: {0}")]
    CompressionFailed(String),
    #[error("Decompression failed: {0}")]
    DecompressionFailed(String),
    #[error("Invalid header: {0}")]
    InvalidHeader(String),
    #[error("Invalid record: {0}")]
    InvalidRecord(String),
    #[error("Unsupported version: {0}")]
    UnsupportedVersion(u16),
    #[error("Unsupported compression: {0}")]
    UnsupportedCompression(u8),
    #[error("Data truncated at offset {0}")]
    DataTruncated(usize),
    #[error("Invalid UTF-8 string: {0}")]
    InvalidUtf8(String),
    #[error("Font parsing failed: {0}")]
    FontParsingFailed(String),
    #[error("Invalid PNG: {0}")]
    InvalidPng(String),
}

impl From<IcedError> for crate::EngineError {
    fn from(err: IcedError) -> Self {
        crate::EngineError::Generic(err.to_string())
    }
}

/// Encode layer flags from Properties
fn encode_layer_flags(props: &LayerProperties) -> u32 {
    let mut flags = 0u32;
    if props.is_visible {
        flags |= constants::layer::IS_VISIBLE;
    }
    if props.is_locked {
        flags |= constants::layer::EDIT_LOCK;
    }
    if props.is_position_locked {
        flags |= constants::layer::POS_LOCK;
    }
    if props.has_alpha_channel {
        flags |= constants::layer::HAS_ALPHA;
    }
    if props.is_alpha_channel_locked {
        flags |= constants::layer::ALPHA_LOCKED;
    }
    flags
}

/// Decode layer flags to Properties
fn decode_layer_flags(flags: u32, props: &mut LayerProperties) {
    props.is_visible = (flags & constants::layer::IS_VISIBLE) != 0;
    props.is_locked = (flags & constants::layer::EDIT_LOCK) != 0;
    props.is_position_locked = (flags & constants::layer::POS_LOCK) != 0;
    props.has_alpha_channel = (flags & constants::layer::HAS_ALPHA) != 0;
    props.is_alpha_channel_locked = (flags & constants::layer::ALPHA_LOCKED) != 0;
}

/// Encode layer color (RGBA where A=0xFF means color is set)
fn encode_layer_color(color: &Option<Color>, data: &mut Vec<u8>) {
    if let Some(c) = color {
        let (r, g, b) = c.clone().rgb();
        data.push(r);
        data.push(g);
        data.push(b);
        data.push(0xFF);
    } else {
        data.extend([0, 0, 0, 0]);
    }
}

/// Decode layer color (RGBA where A=0xFF means color is set)
fn decode_layer_color(bytes: &[u8]) -> Option<Color> {
    if bytes.len() >= 4 && bytes[3] == 0xFF {
        Some(Color::new(bytes[0], bytes[1], bytes[2]))
    } else {
        None
    }
}

fn build_icyd_record(keyword: &str, data: &[u8]) -> std::result::Result<Vec<u8>, IcedError> {
    let keyword_bytes = keyword.as_bytes();
    let keyword_len: u16 = keyword_bytes
        .len()
        .try_into()
        .map_err(|_| IcedError::ChunkEncodingFailed(format!("keyword too long: {}", keyword_bytes.len())))?;
    let data_len: u32 = data
        .len()
        .try_into()
        .map_err(|_| IcedError::ChunkEncodingFailed(format!("payload too large: {}", data.len())))?;

    let mut out = Vec::with_capacity(1 + 2 + keyword_bytes.len() + 4 + data.len());
    out.push(ICYD_RECORD_VERSION);
    out.extend(u16::to_le_bytes(keyword_len));
    out.extend(keyword_bytes);
    out.extend(u32::to_le_bytes(data_len));
    out.extend(data);
    Ok(out)
}

fn write_icyd_record<W: std::io::Write>(writer: &mut png::Writer<W>, keyword: &str, data: &[u8]) -> std::result::Result<(), IcedError> {
    let record = build_icyd_record(keyword, data)?;
    writer
        .write_chunk(png::chunk::ChunkType(ICYD_CHUNK_TYPE), &record)
        .map_err(|e| IcedError::ChunkEncodingFailed(format!("{e}")))?;
    Ok(())
}

/// Compresses data and writes it as an icYD chunk
fn write_compressed_chunk<W: std::io::Write>(writer: &mut png::Writer<W>, keyword: &str, compression: u8, data: &[u8]) -> std::result::Result<(), IcedError> {
    let compressed = match compression {
        constants::compression::NONE => data.to_vec(),
        constants::compression::ZSTD => zstd_encode_all(Cursor::new(data), 3).map_err(|e| IcedError::CompressionFailed(format!("zstd: {e}")))?,
        other => return Err(IcedError::UnsupportedCompression(other)),
    };

    let record = build_icyd_record(keyword, &compressed)?;
    writer
        .write_chunk(png::chunk::ChunkType(ICYD_CHUNK_TYPE), &record)
        .map_err(|e| IcedError::ChunkEncodingFailed(format!("{e}")))?;
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

        let mut writer = encoder.write_header().map_err(|e| IcedError::PngEncodingFailed(format!("header: {e}")))?;
        writer
            .write_image_data(&sixel.picture_data)
            .map_err(|e| IcedError::PngEncodingFailed(format!("data: {e}")))?;
    }

    Ok(png_data)
}

/// Decode PNG data back to RGBA picture data
fn decode_png_to_rgba(png_data: &[u8]) -> std::result::Result<(i32, i32, Vec<u8>), IcedError> {
    let cursor = Cursor::new(png_data);
    let decoder = png::Decoder::new(cursor);
    let mut reader = decoder.read_info().map_err(|e| IcedError::PngDecodingFailed(format!("{e}")))?;

    // The embedded PNG is expected to be RGBA8.
    let png_info = reader.info();
    if png_info.color_type != png::ColorType::Rgba || png_info.bit_depth != png::BitDepth::Eight {
        return Err(IcedError::PngDecodingFailed(format!(
            "unsupported format: {:?}/{:?} (expected RGBA/8-bit)",
            png_info.color_type, png_info.bit_depth
        )));
    }

    let buf_size = reader
        .output_buffer_size()
        .ok_or_else(|| IcedError::PngDecodingFailed("output buffer size unknown".to_string()))?;
    let mut buf = vec![0; buf_size];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| IcedError::PngDecodingFailed(format!("frame read: {e}")))?;

    buf.truncate(info.buffer_size());
    Ok((info.width as i32, info.height as i32, buf))
}

fn zstd_decode_all_limited(bytes: &[u8], limit: usize, context: &str) -> std::result::Result<Vec<u8>, IcedError> {
    use std::io::Read;

    let mut decoder = zstd::stream::read::Decoder::new(Cursor::new(bytes)).map_err(|e| IcedError::DecompressionFailed(format!("init for '{context}': {e}")))?;

    let mut out: Vec<u8> = Vec::new();
    let mut buf = [0u8; 8 * 1024];
    loop {
        let read = decoder
            .read(&mut buf)
            .map_err(|e| IcedError::DecompressionFailed(format!("'{context}': {e}")))?;
        if read == 0 {
            break;
        }
        if out.len().saturating_add(read) > limit {
            return Err(IcedError::DecompressionFailed(format!("'{context}' too large (limit: {limit} bytes)")));
        }
        out.extend_from_slice(&buf[..read]);
    }
    Ok(out)
}

fn extract_png_chunks_by_type(png: &[u8], wanted: [u8; 4]) -> std::result::Result<Vec<Vec<u8>>, IcedError> {
    if png.len() < PNG_SIGNATURE.len() || png[..PNG_SIGNATURE.len()] != PNG_SIGNATURE {
        return Err(IcedError::InvalidPng("invalid signature".to_string()));
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
            return Err(IcedError::InvalidPng("truncated chunk".to_string()));
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

fn parse_icyd_record(payload: &[u8]) -> std::result::Result<(String, &[u8]), IcedError> {
    if payload.len() < 1 + 2 + 4 {
        return Err(IcedError::InvalidRecord("too small".to_string()));
    }
    if payload[0] != ICYD_RECORD_VERSION {
        return Err(IcedError::InvalidRecord(format!("unsupported version {}", payload[0])));
    }
    let keyword_len = u16::from_le_bytes(payload[1..3].try_into().unwrap()) as usize;
    let keyword_start = 3;
    let keyword_end = keyword_start + keyword_len;
    if payload.len() < keyword_end + 4 {
        return Err(IcedError::InvalidRecord("truncated keyword".to_string()));
    }
    let keyword = std::str::from_utf8(&payload[keyword_start..keyword_end]).map_err(|e| IcedError::InvalidUtf8(format!("keyword: {e}")))?;
    let data_len = u32::from_le_bytes(payload[keyword_end..keyword_end + 4].try_into().unwrap()) as usize;
    let data_start = keyword_end + 4;
    let data_end = data_start + data_len;
    if payload.len() < data_end {
        return Err(IcedError::InvalidRecord("truncated data".to_string()));
    }
    Ok((keyword.to_string(), &payload[data_start..data_end]))
}

fn process_icy_draw_v1_decoded_chunk(
    keyword: &str,
    bytes: &[u8],
    result: &mut TextBuffer,
    compression: &mut u8,
    sauce_opt: &mut Option<SauceRecord>,
) -> std::result::Result<bool, IcedError> {
    match keyword {
        "END" => return Ok(false),
        "ICED" => {
            if bytes.len() < 2 {
                return Err(IcedError::InvalidHeader("too small".to_string()));
            }

            let version = u16::from_le_bytes([bytes[0], bytes[1]]);
            if version != 1 {
                return Err(IcedError::UnsupportedVersion(version));
            }
            if bytes.len() < constants::ICED_HEADER_SIZE {
                return Err(IcedError::InvalidHeader(format!("size {} < {}", bytes.len(), constants::ICED_HEADER_SIZE)));
            }

            let mut o: usize = 2; // skip version

            // Read Type field: [compression: u8][reserved: u16]
            *compression = bytes[o];
            o += 3; // skip compression + reserved

            if bytes.len() < o + 2 {
                return Err(IcedError::DataTruncated(o + 2));
            }
            let buffer_type = u16::from_le_bytes(bytes[o..(o + 2)].try_into().unwrap());
            o += 2;
            result.buffer_type = crate::BufferType::from_byte(buffer_type as u8);

            if bytes.len() < o + 2 {
                return Err(IcedError::DataTruncated(o + 2));
            }
            let ice_mode = bytes[o];
            o += 1;
            result.ice_mode = crate::IceMode::from_byte(ice_mode);

            let font_mode = bytes[o];
            o += 1;
            result.font_mode = crate::FontMode::from_byte(font_mode);

            if bytes.len() < o + 8 {
                return Err(IcedError::DataTruncated(o + 8));
            }
            let width_u32 = u32::from_le_bytes(bytes[o..(o + 4)].try_into().unwrap());
            o += 4;
            let height_u32 = u32::from_le_bytes(bytes[o..(o + 4)].try_into().unwrap());
            o += 4;

            let width: i32 = i32::try_from(width_u32).map_err(|_| IcedError::InvalidHeader(format!("width out of range: {width_u32}")))?;
            let height: i32 = i32::try_from(height_u32).map_err(|_| IcedError::InvalidHeader(format!("height out of range: {height_u32}")))?;
            result.set_size((width, height));
            // Keep terminal state dimensions in sync with the buffer size.
            // Some UI/layout code uses TerminalState sizes even for non-terminal buffers.
            result.terminal_state.set_width(width);
            result.terminal_state.set_height(height);
            let font_width = bytes[o] as i32;
            o += 1;
            let font_height = bytes[o] as i32;
            result.set_font_dimensions((font_width, font_height).into());
        }

        "PALETTE" => {
            result.palette = crate::FileFormat::Palette(crate::PaletteFormat::Ice)
                .load_palette(bytes)
                .map_err(|e| IcedError::InvalidRecord(format!("palette: {e}")))?;
        }

        "SAUCE" => {
            if let Some(sauce) = SauceRecord::from_bytes(bytes).map_err(|e| IcedError::InvalidRecord(format!("sauce: {e}")))? {
                super::super::apply_sauce_to_buffer(result, &sauce);
                *sauce_opt = Some(sauce);
            }
        }

        "TAG" => {
            if bytes.len() < 2 {
                return Err(IcedError::DataTruncated(2));
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
                    return Err(IcedError::DataTruncated(14));
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
                    _ => return Err(IcedError::InvalidRecord("unsupported alignment".to_string())),
                };
                cur = &cur[1..];

                let tag_placement = match cur[0] {
                    0 => crate::TagPlacement::InText,
                    1 => crate::TagPlacement::WithGotoXY,
                    _ => return Err(IcedError::InvalidRecord("unsupported tag placement".to_string())),
                };
                cur = &cur[1..];

                let tag_role = match cur[0] {
                    0 => crate::TagRole::Displaycode,
                    1 => crate::TagRole::Hyperlink,
                    _ => return Err(IcedError::InvalidRecord("unsupported tag role".to_string())),
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
                return Err(IcedError::DataTruncated(1));
            }
            let mut o: usize = 0;
            let font_slot = bytes[o];
            o += 1;
            let (font_name, size) = read_utf8_encoded_string(&bytes[o..])?;
            o += size;
            let font = BitFont::from_bytes(font_name, &bytes[o..]).map_err(|e| IcedError::FontParsingFailed(format!("{e}")))?;
            result.set_font(font_slot, font);
        }

        "LAYER" => {
            let mut o: usize = 0;

            // Read layer title
            let (title, size) = read_utf8_encoded_string(&bytes[o..])?;
            o += size;
            // Bounds check for fixed fields: mode(1) + color(4) + flags(4) + offset(8) + width(4) + height(4) = 25 bytes
            if bytes.len() < o + 25 {
                return Err(IcedError::DataTruncated(o + 25));
            }

            // Read mode
            let mode = match bytes[o] {
                1 => crate::Mode::Chars,
                2 => crate::Mode::Attributes,
                _ => crate::Mode::Normal,
                };
            o += 1;

            // Read color (RGBA, where A=0xFF means color is set)
            let color = decode_layer_color(&bytes[o..o + 4]);
            o += 4;

            // Read flags
            let flags = u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
            o += 4;

            // Read offset
            let offset_x = i32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
            o += 4;
            let offset_y = i32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
            o += 4;

            // Read layer dimensions
            let width = i32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
            o += 4;
            let height = i32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
            o += 4;

            // Sanity check: validate layer dimensions against available char data
            // Each char is: char(4) + attribute(5..11 bytes)
            // - Attribute min: fg(1) + bg(1) + font_page(1) + attr(2) = 5 bytes
            // - Attribute max: fg(4) + bg(4) + font_page(1) + attr(2) = 11 bytes
            // So per char: min 9 bytes, max 15 bytes
            let char_data_size = bytes.len().saturating_sub(o);
            let cell_count = (width as i64) * (height as i64);

            const MIN_BYTES_PER_CHAR: i64 = 9; // char(4) + attr_min(5)
            const MAX_BYTES_PER_CHAR: i64 = 15; // char(4) + attr_max(11)

            let min_expected = cell_count * MIN_BYTES_PER_CHAR;
            let max_expected = cell_count * MAX_BYTES_PER_CHAR;

            if (char_data_size as i64) < min_expected || (char_data_size as i64) > max_expected {
                return Err(IcedError::InvalidRecord(format!(
                    "layer '{}' char data size {} doesn't match dimensions {}x{} (expected {}..{} bytes)",
                    title, char_data_size, width, height, min_expected, max_expected
                )));
            }
            let mut layer = Layer::new(title.clone(), (width, height));
            let mut cur = &bytes[o..];
            for y in 0..height {
                for x in 0..width {
                    if cur.len() < 4 {
                        return Err(IcedError::DataTruncated(bytes.len()));
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

            // Set layer properties
            layer.properties.title = title;
            layer.properties.mode = mode;
            layer.properties.color = color;
            decode_layer_flags(flags, &mut layer.properties);
            layer.set_offset((offset_x, offset_y));

            result.layers.push(layer);
        }

        "SIXEL" => {
            let mut o: usize = 0;

            // Read layer title
            let (title, size) = read_utf8_encoded_string(&bytes[o..])?;
            o += size;

            // Bounds check for fixed fields: color(4) + flags(4) + offset(8) + dimensions(8) + scales(8) + png_len(8) = 40 bytes
            if bytes.len() < o + 40 {
                return Err(IcedError::DataTruncated(o + 40));
            }

            // Read color
            let color = decode_layer_color(&bytes[o..o + 4]);
            o += 4;

            // Read flags
            let flags = u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
            o += 4;

            // Read combined position (layer offset + sixel position)
            let pos_x = i32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
            o += 4;
            let pos_y = i32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
            o += 4;

            // Read sixel dimensions
            let width = i32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
            o += 4;
            let height = i32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
            o += 4;
            let vertical_scale = i32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
            o += 4;
            let horizontal_scale = i32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
            o += 4;

            // Read PNG data
            let png_len = u64::from_le_bytes(bytes[o..o + 8].try_into().unwrap()) as usize;
            o += 8;

            if bytes.len() < o + png_len {
                return Err(IcedError::DataTruncated(o + png_len));
            }

            let png_data = &bytes[o..o + png_len];
            let (_, _, picture_data) = decode_png_to_rgba(png_data)?;

            let mut sixel = Sixel::from_data((width, height), vertical_scale, horizontal_scale, picture_data);
            sixel.position = Position::new(0, 0);

            // Calculate layer size from sixel dimensions and font dimensions
            let font_dims = result.font_dimensions();
            let layer_width = (width + font_dims.width - 1) / font_dims.width;
            let layer_height = (height + font_dims.height - 1) / font_dims.height;

            let mut layer = Layer::new(title.clone(), (layer_width, layer_height));
            layer.role = crate::Role::Image;
            layer.properties.title = title;
            layer.properties.color = color;
            decode_layer_flags(flags, &mut layer.properties);
            layer.sixels.push(sixel);
            layer.set_offset((pos_x, pos_y));

            result.layers.push(layer);
        }

        text => {
            log::warn!("unsupported chunk {text}");
            return Ok(true);
        }
    }

    Ok(true)
}

pub(crate) fn save_icy_draw(buf: &TextBuffer, options: &SaveOptions) -> Result<Vec<u8>> {
    let mut png_bytes = Vec::new();

    let mut first_line = 0;
    let mut last_line = 0;

    let font_dims = buf.font_dimensions();
    let icy_opts = options.icy_draw_options();
    // Absolute fast path for IcyDraw autosave: no thumbnail rendering.
    let fast_save = icy_opts.skip_thumbnail;

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

    let mut encoder: png::Encoder<'_, &mut Vec<u8>> = png::Encoder::new(&mut png_bytes, width as u32, height as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);

    // The PNG preview is not the bottleneck for the fast-save path (it's 1x1), but this keeps
    // encoding overhead minimal and predictable.
    if fast_save {
        encoder.set_compression(png::Compression::Fastest);
    }

    let mut writer = encoder.write_header()?;

    // ICED v1: compression is a file-level choice recorded in the ICED header.
    // For autosave and debug scenarios we support fully uncompressed records.
    let file_compression = if icy_opts.compress {
        constants::compression::ZSTD
    } else {
        constants::compression::NONE
    };

    {
        let mut result = vec![constants::ICED_VERSION as u8, (constants::ICED_VERSION >> 8) as u8];
        // Type field: [compression: u8][reserved: u16]
        result.push(file_compression); // compression method
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

    if let Some(meta) = &options.sauce {
        let sauce = buf.build_character_sauce(meta, CharacterFormat::Ansi);
        let mut sauce_vec: Vec<u8> = Vec::new();
        sauce.write(&mut sauce_vec)?;
        write_compressed_chunk(&mut writer, "SAUCE", file_compression, &sauce_vec)?;
    }

    if !buf.palette.is_default() {
        let pal_data = buf.palette.export_palette(&crate::FileFormat::Palette(crate::PaletteFormat::Ice)).unwrap();
        write_compressed_chunk(&mut writer, "PALETTE", file_compression, &pal_data)?;
    }

    for (slot, v) in buf.font_iter() {
        let mut font_data: Vec<u8> = Vec::new();
        font_data.push(*slot as u8);
        write_utf8_encoded_string(&mut font_data, &v.name());
        font_data.extend(v.to_psf2_bytes().unwrap());

        write_compressed_chunk(&mut writer, "FONT", file_compression, &font_data)?;
    }

    for layer in &buf.layers {
        if layer.role == crate::Role::Image {
            // SIXEL chunk - separate format for image layers
            let sixel = &layer.sixels[0];
            let mut sixel_data = Vec::new();
            write_utf8_encoded_string(&mut sixel_data, &layer.properties.title);

            // Layer color
            encode_layer_color(&layer.properties.color, &mut sixel_data);

            // Layer flags
            let flags = encode_layer_flags(&layer.properties);
            sixel_data.extend(u32::to_le_bytes(flags));

            // Combine layer offset with sixel position for foolproof storage
            let combined_x = layer.offset().x + sixel.position.x;
            let combined_y = layer.offset().y + sixel.position.y;
            sixel_data.extend(i32::to_le_bytes(combined_x));
            sixel_data.extend(i32::to_le_bytes(combined_y));

            // Sixel dimensions and scaling
            sixel_data.extend(i32::to_le_bytes(sixel.width()));
            sixel_data.extend(i32::to_le_bytes(sixel.height()));
            sixel_data.extend(i32::to_le_bytes(sixel.vertical_scale));
            sixel_data.extend(i32::to_le_bytes(sixel.horizontal_scale));

            // PNG-encoded image data (already compressed, skip ZSTD)
            let png_data = encode_sixel_as_png(sixel)?;
            sixel_data.extend(u64::to_le_bytes(png_data.len() as u64));
            sixel_data.extend(&png_data);

            // SIXEL chunks are always uncompressed - PNG data is already compressed
            write_compressed_chunk(&mut writer, "SIXEL", constants::compression::NONE, &sixel_data)?;
        } else {
            // LAYER chunk - only for char layers
            let mut layer_data = Vec::new();
            write_utf8_encoded_string(&mut layer_data, &layer.properties.title);

            let mode = match layer.properties.mode {
                crate::Mode::Normal => 0,
                crate::Mode::Chars => 1,
                crate::Mode::Attributes => 2,
            };
            layer_data.push(mode);

            encode_layer_color(&layer.properties.color, &mut layer_data);

            let flags = encode_layer_flags(&layer.properties);
            layer_data.extend(u32::to_le_bytes(flags));

            layer_data.extend(i32::to_le_bytes(layer.offset().x));
            layer_data.extend(i32::to_le_bytes(layer.offset().y));

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

            write_compressed_chunk(&mut writer, "LAYER", file_compression, &layer_data)?;
        }
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

        write_compressed_chunk(&mut writer, "TAG", file_compression, &data)?;
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

    // Compression from ICED header
    let mut compression = constants::compression::NONE;
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
            return Err(IcedError::InvalidRecord(format!("ICED must be first (found '{first_keyword}')")).into());
        }
        None => return Ok(None),
    }

    // Must have ICED to be considered a v1 file.
    let mut iced_bytes_opt: Option<Vec<u8>> = None;
    for (keyword, bytes) in &records {
        if keyword == "ICED" {
            if iced_bytes_opt.is_some() {
                return Err(IcedError::InvalidRecord("multiple ICED headers".to_string()).into());
            }
            iced_bytes_opt = Some(bytes.clone());
        }
    }
    let Some(iced_bytes) = iced_bytes_opt else {
        return Err(IcedError::InvalidRecord("ICED header missing".to_string()).into());
    };

    // Process ICED first to initialize buffer metadata and the compression setting.
    let _ = process_icy_draw_v1_decoded_chunk("ICED", &iced_bytes, &mut result, &mut compression, &mut sauce_opt)?;

    for (keyword, bytes) in records {
        if keyword == "ICED" {
            continue;
        }

        // Decompress data if needed (END and SIXEL are never compressed - SIXEL contains PNG which is already compressed)
        let decompressed_data: Vec<u8>;
        let actual_bytes: &[u8] = if keyword != "END" && keyword != "SIXEL" {
            match compression {
                constants::compression::NONE => &bytes,
                constants::compression::ZSTD => {
                    decompressed_data = zstd_decode_all_limited(&bytes, MAX_DECOMPRESSED_RECORD_SIZE, &keyword)?;
                    &decompressed_data
                }
                other => {
                    return Err(IcedError::UnsupportedCompression(other).into());
                }
            }
        } else {
            &bytes
        };

        let keep_running = process_icy_draw_v1_decoded_chunk(&keyword, actual_bytes, &mut result, &mut compression, &mut sauce_opt)?;
        if !keep_running {
            break;
        }
    }

    Ok(Some((TextScreen::from_buffer(result), sauce_opt)))
}

fn read_utf8_encoded_string(data: &[u8]) -> std::result::Result<(String, usize), IcedError> {
    if data.len() < 4 {
        return Err(IcedError::DataTruncated(4));
    }

    let size = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;
    let end = 4usize.saturating_add(size);
    if data.len() < end {
        return Err(IcedError::DataTruncated(end));
    }

    let s = std::str::from_utf8(&data[4..end])
        .map_err(|e| IcedError::InvalidUtf8(format!("{e}")))?
        .to_string();

    Ok((s, size + 4))
}

fn write_utf8_encoded_string(data: &mut Vec<u8>, s: &str) {
    data.extend(u32::to_le_bytes(s.len() as u32));
    data.extend(s.as_bytes());
}

const MAX_LINES: i32 = 80;
