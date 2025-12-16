use std::collections::HashMap;
use std::fmt::Alignment;
use std::io::{Cursor, Read, Write};

use base64::{Engine, engine::general_purpose};
use bzip2::read::BzDecoder;
use bzip2::write::BzEncoder;
use bzip2::Compression as BzCompression;
use icy_sauce::SauceRecord;
use regex::Regex;

use crate::{BitFont, Color, Layer, LoadingError, Position, Result, Sixel, Size, TextBuffer, TextPane, TextScreen, attribute};

use super::super::{AnsiSaveOptionsV2, LoadData};

mod constants {
    pub const ICED_VERSION: u16 = 1;
    pub const ICED_HEADER_SIZEV0: usize = 19;
    pub const ICED_HEADER_SIZE: usize = 21;
    
    /// Compression methods for ICED format (stored in first byte of Type field)
    pub mod compression {
        pub const NONE: u8 = 0;
        pub const BZ2: u8 = 1;
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

lazy_static::lazy_static! {
    static ref LAYER_CONTINUE_REGEX: Regex = Regex::new(r"LAYER_(\d+)~(\d+)").unwrap();
}

const PNG_SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
const ICYD_CHUNK_TYPE: [u8; 4] = *b"icYD";
const ICYD_RECORD_VERSION: u8 = 1;

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

/// Compresses data with bz2 and writes it as an icYD chunk
fn write_compressed_chunk<W: std::io::Write>(
    writer: &mut png::Writer<W>,
    keyword: &str,
    data: &[u8],
) -> std::result::Result<(), IcedError> {
    // Compress data with bz2
    let mut encoder = BzEncoder::new(Vec::new(), BzCompression::best());
    encoder.write_all(data).map_err(|e| IcedError::ErrorEncodingZText(format!("bz2 compression failed: {e}")))?;
    let compressed = encoder.finish().map_err(|e| IcedError::ErrorEncodingZText(format!("bz2 finish failed: {e}")))?;
    
    let record = build_icyd_record(keyword, &compressed).map_err(|e| IcedError::ErrorEncodingZText(format!("{e}")))?;
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
        
        let mut writer = encoder.write_header().map_err(|e| IcedError::ErrorEncodingZText(format!("PNG header failed: {e}")))?;
        writer.write_image_data(&sixel.picture_data).map_err(|e| IcedError::ErrorEncodingZText(format!("PNG write failed: {e}")))?;
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

fn process_icy_draw_decoded_chunk(
    keyword: &str,
    bytes: &[u8],
    result: &mut TextBuffer,
    layer_resume_y: &mut HashMap<usize, i32>,
    compression: &mut u8,
    sixel_format: &mut u8,
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
            match version {
                0 => {
                    if bytes.len() != constants::ICED_HEADER_SIZEV0 {
                        return Err(crate::EngineError::UnsupportedFormat {
                            description: format!("unsupported ICED v0 header size {}", bytes.len()),
                        });
                    }
                }
                1 => {
                    if bytes.len() != constants::ICED_HEADER_SIZE {
                        return Err(crate::EngineError::UnsupportedFormat {
                            description: format!("unsupported ICED v1 header size {}", bytes.len()),
                        });
                    }
                }
                _ => {
                    return Err(crate::EngineError::UnsupportedFormat {
                        description: format!("unsupported ICED version {} (max supported: {})", version, constants::ICED_VERSION),
                    });
                }
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

            let palette_mode = bytes[o];
            o += 1;
            result.palette_mode = crate::PaletteMode::from_byte(palette_mode);

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

                if cur.len() < 4 + 4 + 2 + 1 + 1 + 1 + 2 {
                    return Err(crate::EngineError::OutOfBounds { offset: bytes.len() + 1 });
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

                if cur.len() < 2 {
                    return Err(crate::EngineError::OutOfBounds { offset: 2 });
                }
                let mut attr = u16::from_le_bytes(cur[..2].try_into().unwrap());
                cur = &cur[2..];

                let is_short = if attr & attribute::SHORT_DATA != 0 {
                    attr &= !attribute::SHORT_DATA;
                    true
                } else {
                    false
                };

                let (fg, bg, font_page, ext_attr) = if is_short {
                    if cur.len() < 3 {
                        return Err(crate::EngineError::OutOfBounds { offset: 3 });
                    }
                    let fg = cur[0] as u32;
                    let bg = cur[1] as u32;
                    let font_page = cur[2];
                    cur = &cur[3..];
                    (fg, bg, font_page, 0)
                } else {
                    if cur.len() < 10 {
                        return Err(crate::EngineError::OutOfBounds { offset: 10 });
                    }
                    let fg = u32::from_le_bytes(cur[..4].try_into().unwrap());
                    let bg = u32::from_le_bytes(cur[4..8].try_into().unwrap());
                    let font_page = cur[8];
                    let ext_attr = cur[9];
                    cur = &cur[10..];
                    (fg, bg, font_page, ext_attr)
                };

                if cur.len() < 16 {
                    return Err(crate::EngineError::OutOfBounds { offset: 16 });
                }
                cur = &cur[16..]; // unused data for future use

                result.tags.push(crate::Tag {
                    preview,
                    replacement_value,
                    position: Position::new(x, y),
                    length,
                    is_enabled,
                    alignment,
                    tag_placement,
                    tag_role,
                    attribute: crate::TextAttribute {
                        foreground_color: fg,
                        background_color: bg,
                        font_page,
                        ext_attr,
                        attr,
                    },
                });
            }
        }

        text => {
            if let Some(font_slot) = text.strip_prefix("FONT_") {
                match font_slot.parse() {
                    Ok(font_slot) => {
                        let mut o: usize = 0;
                        let (font_name, size) = read_utf8_encoded_string(&bytes[o..])?;
                        o += size;
                        let font = BitFont::from_bytes(font_name, &bytes[o..])?;
                        result.set_font(font_slot, font);
                    }
                    Err(e) => return Err(IcedError::ErrorParsingFontSlot(e.to_string()).into()),
                }
                return Ok(true);
            }

            if !text.starts_with("LAYER_") {
                log::warn!("unsupported chunk {text}");
                return Ok(true);
            }

            // Continuation chunk
            if let Some(m) = LAYER_CONTINUE_REGEX.captures(text) {
                let (_, [layer_num, _chunk]) = m.extract();
                let layer_num = layer_num.parse::<usize>()?;

                if layer_num >= result.layers.len() {
                    return Err(crate::EngineError::UnsupportedFormat {
                        description: format!("layer continuation refers to missing layer index {layer_num}"),
                    });
                }

                let layer = &mut result.layers[layer_num];
                match layer.role {
                    crate::Role::Normal => {
                        let mut o = 0;
                        let start_y = *layer_resume_y.get(&layer_num).unwrap_or(&layer.line_count());
                        let mut y = start_y;
                        while y < layer.height() {
                            if o >= bytes.len() {
                                break;
                            }
                            for x in 0..layer.width() {
                                if bytes.len() < o + 2 {
                                    return Err(crate::EngineError::OutOfBounds { offset: o + 2 });
                                }
                                let mut attr_raw = u16::from_le_bytes(bytes[o..(o + 2)].try_into().unwrap());
                                o += 2;
                                if attr_raw == attribute::INVISIBLE_SHORT {
                                    break;
                                }

                                let is_short = (attr_raw & attribute::SHORT_DATA) != 0;
                                if is_short {
                                    attr_raw &= !attribute::SHORT_DATA;
                                }
                                let attr = attr_raw;
                                if attr == attribute::INVISIBLE {
                                    continue;
                                }

                                let need = if is_short { 4 } else { 14 };
                                if bytes.len() < o + need {
                                    return Err(crate::EngineError::OutOfBounds { offset: o + need });
                                }

                                let (ch_u32, fg, bg, ext_attr, font_page) = if is_short {
                                    let ch = bytes[o] as u32;
                                    let fg = bytes[o + 1] as u32;
                                    let bg = bytes[o + 2] as u32;
                                    let font_page = bytes[o + 3];
                                    o += 4;
                                    (ch, fg, bg, 0, font_page)
                                } else {
                                    let ch = u32::from_le_bytes(bytes[o..(o + 4)].try_into().unwrap());
                                    let fg = u32::from_le_bytes(bytes[(o + 4)..(o + 8)].try_into().unwrap());
                                    let bg = u32::from_le_bytes(bytes[(o + 8)..(o + 12)].try_into().unwrap());
                                    let font_page = bytes[o + 12];
                                    let ext_attr = bytes[o + 13];
                                    o += 14;
                                    (ch, fg, bg, ext_attr, font_page)
                                };

                                let ch = char::from_u32(ch_u32).ok_or_else(|| crate::EngineError::UnsupportedFormat {
                                    description: format!("invalid unicode scalar value: {ch_u32}"),
                                })?;

                                layer.set_char(
                                    (x, y),
                                    crate::AttributedChar {
                                        ch,
                                        attribute: crate::TextAttribute {
                                            foreground_color: fg,
                                            background_color: bg,
                                            font_page,
                                            ext_attr,
                                            attr,
                                        },
                                    },
                                );
                            }
                            y += 1;
                        }

                        layer_resume_y.insert(layer_num, y);
                        if y >= layer.height() {
                            layer_resume_y.remove(&layer_num);
                        }
                        return Ok(true);
                    }
                    crate::Role::Image => {
                        layer.sixels[0].picture_data.extend(bytes);
                        return Ok(true);
                    }
                    crate::Role::PastePreview | crate::Role::PasteImage => {
                        return Ok(true);
                    }
                }
            }

            let layer_num = text
                .strip_prefix("LAYER_")
                .ok_or_else(|| crate::EngineError::UnsupportedFormat {
                    description: format!("invalid layer keyword {text}"),
                })?
                .parse::<usize>()
                .map_err(|_| crate::EngineError::UnsupportedFormat {
                    description: format!("invalid layer index in keyword {text}"),
                })?;

            if layer_num != result.layers.len() {
                return Err(crate::EngineError::UnsupportedFormat {
                    description: format!("unexpected layer index {layer_num}, expected {}", result.layers.len()),
                });
            }

            let mut o: usize = 0;
            let (title, used) = read_utf8_encoded_string(&bytes[o..])?;
            let mut layer = Layer::new(title, (0, 0));
            o += used;

            if bytes.len() < o + 1 {
                return Err(crate::EngineError::OutOfBounds { offset: o + 1 });
            }
            let role = bytes[o];
            o += 1;
            layer.role = if role == 1 { crate::Role::Image } else { crate::Role::Normal };

            if bytes.len() < o + 4 {
                return Err(crate::EngineError::OutOfBounds { offset: o + 4 });
            }
            o += 4; // unused

            if bytes.len() < o + 1 {
                return Err(crate::EngineError::OutOfBounds { offset: o + 1 });
            }
            let mode = bytes[o];
            o += 1;
            layer.properties.mode = match mode {
                0 => crate::Mode::Normal,
                1 => crate::Mode::Chars,
                2 => crate::Mode::Attributes,
                _ => return Err(LoadingError::IcyDrawUnsupportedLayerMode(mode).into()),
            };

            if bytes.len() < o + 4 {
                return Err(crate::EngineError::OutOfBounds { offset: o + 4 });
            }
            let red = bytes[o];
            let green = bytes[o + 1];
            let blue = bytes[o + 2];
            let alpha = bytes[o + 3];
            o += 4;
            if alpha != 0 {
                layer.properties.color = Some(Color::new(red, green, blue));
            }

            if bytes.len() < o + 4 + 1 {
                return Err(crate::EngineError::OutOfBounds { offset: o + 5 });
            }
            let flags = u32::from_le_bytes(bytes[o..(o + 4)].try_into().unwrap());
            o += 4;
            layer.transparency = bytes[o];
            o += 1;

            if bytes.len() < o + 4 + 4 {
                return Err(crate::EngineError::OutOfBounds { offset: o + 8 });
            }
            let x_offset = i32::from_le_bytes(bytes[o..(o + 4)].try_into().unwrap());
            o += 4;
            let y_offset = i32::from_le_bytes(bytes[o..(o + 4)].try_into().unwrap());
            o += 4;
            layer.set_offset((x_offset, y_offset));

            if bytes.len() < o + 4 + 4 + 2 {
                return Err(crate::EngineError::OutOfBounds { offset: o + 10 });
            }
            let width = i32::from_le_bytes(bytes[o..(o + 4)].try_into().unwrap());
            o += 4;
            let height = i32::from_le_bytes(bytes[o..(o + 4)].try_into().unwrap());
            o += 4;
            layer.set_size((width, height));
            let default_font_page = u16::from_le_bytes(bytes[o..(o + 2)].try_into().unwrap());
            o += 2;
            layer.default_font_page = default_font_page as usize;

            if bytes.len() < o + 8 {
                return Err(crate::EngineError::OutOfBounds { offset: o + 8 });
            }
            let length = u64::from_le_bytes(bytes[o..(o + 8)].try_into().unwrap()) as usize;
            o += 8;

            if role == 1 {
                if bytes.len() < o + 16 {
                    return Err(crate::EngineError::OutOfBounds { offset: o + 16 });
                }
                let sixel_width = i32::from_le_bytes(bytes[o..(o + 4)].try_into().unwrap());
                o += 4;
                let sixel_height = i32::from_le_bytes(bytes[o..(o + 4)].try_into().unwrap());
                o += 4;
                let vert_scale = i32::from_le_bytes(bytes[o..(o + 4)].try_into().unwrap());
                o += 4;
                let horiz_scale = i32::from_le_bytes(bytes[o..(o + 4)].try_into().unwrap());
                o += 4;
                
                // Check sixel format: PNG or raw RGBA
                let picture_data = if *sixel_format == constants::sixel_format::PNG {
                    // Decode PNG to RGBA
                    let png_data = &bytes[o..o + length];
                    let (_, _, rgba_data) = decode_png_to_rgba(png_data)?;
                    rgba_data
                } else {
                    // Raw RGBA data
                    bytes[o..].to_vec()
                };
                
                layer
                    .sixels
                    .push(Sixel::from_data((sixel_width, sixel_height), vert_scale, horiz_scale, picture_data));
                result.layers.push(layer);
            } else {
                if bytes.len() < o + length {
                    return Err(crate::EngineError::OutOfBounds { offset: o + length });
                }
                let mut y = 0;
                while y < height {
                    if o >= bytes.len() {
                        break;
                    }
                    for x in 0..width {
                        if bytes.len() < o + 2 {
                            return Err(crate::EngineError::OutOfBounds { offset: o + 2 });
                        }
                        let mut attr_raw = u16::from_le_bytes(bytes[o..(o + 2)].try_into().unwrap());
                        o += 2;
                        if attr_raw == attribute::INVISIBLE_SHORT {
                            break;
                        }

                        let is_short = (attr_raw & attribute::SHORT_DATA) != 0;
                        if is_short {
                            attr_raw &= !attribute::SHORT_DATA;
                        }
                        let attr = attr_raw;
                        if attr == attribute::INVISIBLE {
                            continue;
                        }

                        let need = if is_short { 4 } else { 14 };
                        if bytes.len() < o + need {
                            return Err(crate::EngineError::OutOfBounds { offset: o + need });
                        }

                        let (ch_u32, fg, bg, ext_attr, font_page) = if is_short {
                            let ch = bytes[o] as u32;
                            let fg = bytes[o + 1] as u32;
                            let bg = bytes[o + 2] as u32;
                            let font_page = bytes[o + 3];
                            o += 4;
                            (ch, fg, bg, 0u8, font_page)
                        } else {
                            let ch = u32::from_le_bytes(bytes[o..(o + 4)].try_into().unwrap());
                            let fg = u32::from_le_bytes(bytes[(o + 4)..(o + 8)].try_into().unwrap());
                            let bg = u32::from_le_bytes(bytes[(o + 8)..(o + 12)].try_into().unwrap());
                            let font_page = bytes[o + 12];
                            let ext_attr = bytes[o + 13];
                            o += 14;
                            (ch, fg, bg, ext_attr, font_page)
                        };

                        let ch = char::from_u32(ch_u32).ok_or_else(|| crate::EngineError::UnsupportedFormat {
                            description: format!("invalid unicode scalar value: {ch_u32}"),
                        })?;

                        layer.set_char(
                            (x, y),
                            crate::AttributedChar {
                                ch,
                                attribute: crate::TextAttribute {
                                    foreground_color: fg,
                                    background_color: bg,
                                    font_page,
                                    ext_attr,
                                    attr,
                                },
                            },
                        );
                    }
                    y += 1;
                }
                result.layers.push(layer);

                // Remember where to resume if this layer continues in `LAYER_{layer_num}~k`.
                if y < height {
                    layer_resume_y.insert(layer_num, y);
                }
            }

            // set attributes at the end because of the way the parser works
            if let Some(layer) = result.layers.last_mut() {
                layer.properties.is_visible = (flags & constants::layer::IS_VISIBLE) == constants::layer::IS_VISIBLE;
                layer.properties.is_locked = (flags & constants::layer::EDIT_LOCK) == constants::layer::EDIT_LOCK;
                layer.properties.is_position_locked = (flags & constants::layer::POS_LOCK) == constants::layer::POS_LOCK;
                layer.properties.has_alpha_channel = (flags & constants::layer::HAS_ALPHA) == constants::layer::HAS_ALPHA;
                layer.properties.is_alpha_channel_locked = (flags & constants::layer::ALPHA_LOCKED) == constants::layer::ALPHA_LOCKED;
            }
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
        result.push(constants::compression::BZ2);  // compression method
        result.push(constants::sixel_format::PNG); // sixel format
        result.extend([0, 0]); // reserved
        // Modes
        result.extend(u16::to_le_bytes(buf.buffer_type.to_byte() as u16));
        result.push(buf.ice_mode.to_byte());
        result.push(buf.palette_mode.to_byte());
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

    for (k, v) in buf.font_iter() {
        let mut font_data: Vec<u8> = Vec::new();
        write_utf8_encoded_string(&mut font_data, &v.name());
        font_data.extend(v.to_psf2_bytes().unwrap());

        write_compressed_chunk(&mut writer, &format!("FONT_{k}"), &font_data)?;
    }

    for (i, layer) in buf.layers.iter().enumerate() {
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
        layer_data.push(layer.transparency);

        layer_data.extend(i32::to_le_bytes(layer.offset().x));
        layer_data.extend(i32::to_le_bytes(layer.offset().y));

        layer_data.extend(i32::to_le_bytes(layer.width()));
        layer_data.extend(i32::to_le_bytes(layer.height()));
        layer_data.extend(u16::to_le_bytes(layer.default_font_page as u16));

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
            // Build char data
            let mut char_data = Vec::new();
            
            for y in 0..layer.height() {
                let real_length = get_invisible_line_length(layer, y);
                for x in 0..real_length {
                    let ch = layer.char_at((x, y).into());
                    let mut attr = ch.attribute.attr;

                    let is_short = if ch.is_visible()
                        && ch.ch as u32 <= 255
                        && ch.attribute.foreground_color <= 255
                        && ch.attribute.background_color <= 255
                        && ch.attribute.ext_attr == 0
                    {
                        attr |= attribute::SHORT_DATA;
                        true
                    } else {
                        false
                    };

                    char_data.extend(u16::to_le_bytes(attr));
                    if !ch.is_visible() {
                        continue;
                    }

                    if is_short {
                        char_data.push(ch.ch as u8);
                        char_data.push(ch.attribute.foreground_color as u8);
                        char_data.push(ch.attribute.background_color as u8);
                        char_data.push(ch.attribute.font_page);
                    } else {
                        char_data.extend(u32::to_le_bytes(ch.ch as u32));
                        char_data.extend(u32::to_le_bytes(ch.attribute.foreground_color));
                        char_data.extend(u32::to_le_bytes(ch.attribute.background_color));
                        char_data.push(ch.attribute.font_page);
                        char_data.push(ch.attribute.ext_attr);
                    }
                }
                if layer.width() > real_length {
                    char_data.extend(u16::to_le_bytes(attribute::INVISIBLE_SHORT));
                }
            }
            
            layer_data.extend(u64::to_le_bytes(char_data.len() as u64));
            layer_data.extend(char_data);
        }
        
        // Write the layer data with bz2 compression
        let keyword = format!("LAYER_{i}");
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
            let mut attr = tag.attribute.attr;

            let is_short = if tag.attribute.foreground_color <= 255 && tag.attribute.background_color <= 255 && tag.attribute.ext_attr == 0 {
                attr |= attribute::SHORT_DATA;
                true
            } else {
                false
            };
            data.extend(u16::to_le_bytes(attr));
            if is_short {
                data.push(tag.attribute.foreground_color as u8);
                data.push(tag.attribute.background_color as u8);
                data.push(tag.attribute.font_page as u8);
            } else {
                data.extend(u32::to_le_bytes(tag.attribute.foreground_color));
                data.extend(u32::to_le_bytes(tag.attribute.background_color));
                data.push(tag.attribute.font_page);
                data.push(tag.attribute.ext_attr);
            }
            // unused data for future use
            data.extend(&[0, 0, 0, 0]);
            data.extend(&[0, 0, 0, 0]);
            data.extend(&[0, 0, 0, 0]);
            data.extend(&[0, 0, 0, 0]);
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

pub(crate) fn load_icy_draw(data: &[u8], _load_data_opt: Option<LoadData>) -> Result<TextScreen> {
    if let Some(screen) = load_icy_draw_binary_chunks(data)? {
        return Ok(screen);
    }
    load_icy_draw_legacy_base64_text_chunks(data)
}

fn load_icy_draw_binary_chunks(data: &[u8]) -> Result<Option<TextScreen>> {
    let mut result = TextBuffer::new((80, 25));
    result.terminal_state.is_terminal_buffer = false;
    result.layers.clear();

    // Track how many lines were decoded per layer so `LAYER_i~k` continues at the correct y.
    let mut layer_resume_y: HashMap<usize, i32> = HashMap::new();
    
    // Compression and sixel format from ICED header
    let mut compression = constants::compression::NONE;
    let mut sixel_format = constants::sixel_format::RAW_RGBA;

    let records = extract_png_chunks_by_type(data, ICYD_CHUNK_TYPE)?;
    if records.is_empty() {
        return Ok(None);
    }

    let mut is_running = true;
    for payload in records {
        let (keyword, bytes) = parse_icyd_record(&payload)?;
        
        // Decompress data if needed (except for ICED header and END which are never compressed)
        let decompressed_data: Vec<u8>;
        let actual_bytes: &[u8] = if keyword != "ICED" && keyword != "END" && compression == constants::compression::BZ2 {
            let mut decoder = BzDecoder::new(bytes);
            let mut buf = Vec::new();
            decoder.read_to_end(&mut buf).map_err(|e| crate::EngineError::UnsupportedFormat {
                description: format!("bz2 decompression failed for '{}': {e}", keyword),
            })?;
            decompressed_data = buf;
            &decompressed_data
        } else {
            decompressed_data = Vec::new();
            let _ = &decompressed_data; // suppress unused warning
            bytes
        };
        
        let keep_running = process_icy_draw_decoded_chunk(&keyword, actual_bytes, &mut result, &mut layer_resume_y, &mut compression, &mut sixel_format)?;
        if !keep_running {
            is_running = false;
            break;
        }
    }

    if is_running {
        Ok(Some(TextScreen::from_buffer(result)))
    } else {
        Ok(Some(TextScreen::from_buffer(result)))
    }
}

// Legacy loader for older IcyDraw files that stored Base64 payloads in tEXt/zTXt chunks.
fn load_icy_draw_legacy_base64_text_chunks(data: &[u8]) -> Result<TextScreen> {
    let mut result = TextBuffer::new((80, 25));
    result.terminal_state.is_terminal_buffer = false;
    result.layers.clear();

    // Track how many lines were decoded per layer so `LAYER_i~k` continues at the correct y.
    let mut layer_resume_y: HashMap<usize, i32> = HashMap::new();
    
    // Legacy files have no compression
    let mut compression = constants::compression::NONE;
    let mut sixel_format = constants::sixel_format::RAW_RGBA;

    let mut decoder = png::StreamingDecoder::new();
    let mut len = 0;
    let mut last_uncompressed_info = 0usize;
    let mut last_compressed_info = 0usize;
    let mut is_running = true;

    while is_running {
        match decoder.update(&data[len..], None) {
            Ok((b, _)) => {
                len += b;
                if data.len() <= len {
                    break;
                }

                let Some(info) = decoder.info() else {
                    continue;
                };

                for i in last_uncompressed_info..info.uncompressed_latin1_text.len() {
                    let chunk = &info.uncompressed_latin1_text[i];
                    let text = chunk.text.as_str();

                    let decoded = match general_purpose::STANDARD.decode(text) {
                        Ok(data) => data,
                        Err(e) => {
                            log::warn!("error decoding iced chunk: {e}");
                            continue;
                        }
                    };

                    let keep_running = process_icy_draw_decoded_chunk(chunk.keyword.as_str(), &decoded, &mut result, &mut layer_resume_y, &mut compression, &mut sixel_format)?;
                    if !keep_running {
                        is_running = false;
                        break;
                    }
                }
                last_uncompressed_info = info.uncompressed_latin1_text.len();

                if !is_running {
                    break;
                }

                for i in last_compressed_info..info.compressed_latin1_text.len() {
                    let chunk = &info.compressed_latin1_text[i];
                    let Ok(text) = chunk.get_text() else {
                        log::error!("error decoding iced chunk: {}", chunk.keyword);
                        continue;
                    };

                    let decoded = match general_purpose::STANDARD.decode(text) {
                        Ok(data) => data,
                        Err(e) => {
                            log::warn!("error decoding iced chunk: {e}");
                            continue;
                        }
                    };

                    let keep_running = process_icy_draw_decoded_chunk(chunk.keyword.as_str(), &decoded, &mut result, &mut layer_resume_y, &mut compression, &mut sixel_format)?;
                    if !keep_running {
                        is_running = false;
                        break;
                    }
                }
                last_compressed_info = info.compressed_latin1_text.len();
            }
            Err(err) => {
                return Err(LoadingError::InvalidPng(format!("{err}")).into());
            }
        }
    }

    Ok(TextScreen::from_buffer(result))
}

fn get_invisible_line_length(layer: &Layer, y: i32) -> i32 {
    let mut length = layer.width();
    while length > 0 && !layer.char_at((length - 1, y).into()).is_visible() {
        length -= 1;
    }
    length
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
