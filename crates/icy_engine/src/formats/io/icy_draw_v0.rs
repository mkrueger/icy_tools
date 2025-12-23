use std::collections::HashMap;
use std::fmt::Alignment;

use base64::{engine::general_purpose, Engine};
use icy_sauce::SauceRecord;
use regex::Regex;

use crate::{AttributeColor, BitFont, Color, Layer, LoadingError, Position, Result, Sixel, TextAttribute, TextBuffer, TextPane, TextScreen};

/// Decode legacy wire format (ext_attr + u32 fg/bg) into AttributeColor.
fn decode_legacy_color(raw_color: u32, ext_attr: u8, is_foreground: bool) -> AttributeColor {
    if raw_color == 0x8000_0000 {
        return AttributeColor::Transparent;
    }
    let (rgb_flag, ext_flag) = if is_foreground {
        (0b0000_0001, 0b0000_0100)
    } else {
        (0b0000_0010, 0b0000_1000)
    };
    if (ext_attr & rgb_flag) != 0 {
        let r = ((raw_color >> 16) & 0xFF) as u8;
        let g = ((raw_color >> 8) & 0xFF) as u8;
        let b = (raw_color & 0xFF) as u8;
        AttributeColor::Rgb(r, g, b)
    } else if (ext_attr & ext_flag) != 0 {
        AttributeColor::ExtendedPalette((raw_color & 0xFF) as u8)
    } else {
        AttributeColor::Palette((raw_color & 0xFF) as u8)
    }
}

lazy_static::lazy_static! {
    static ref LAYER_CONTINUE_REGEX: Regex = Regex::new(r"LAYER_(\d+)~(\d+)").unwrap();
}

// V0 wire flags (kept local so runtime flags can evolve independently)
const V0_EOL: u16 = 0xC000;
// Historical versions used different bits for SHORT_DATA; accept both.
const V0_SHORT_DATA_MASK: u16 = 0x0800 | 0x4000;
// V0 stored "invisible cell" as a special attr-only marker.
const V0_INVISIBLE_CELL: u16 = 0x8000;

/// Load an IcyDraw v0 file (Base64 encoded tEXt/zTXt PNG chunks).
/// Returns an error if the file is not a valid v0 file.
pub(crate) fn load_icy_draw_v0(data: &[u8]) -> Result<(TextScreen, Option<SauceRecord>)> {
    match load_icy_draw_v0_base64_text_chunks(data)? {
        Some((screen, sauce_opt)) => Ok((screen, sauce_opt)),
        None => Err(crate::EngineError::UnsupportedFormat {
            description: "Not a valid IcyDraw v0 file".to_string(),
        }),
    }
}

pub(crate) fn load_icy_draw_v0_base64_text_chunks(data: &[u8]) -> Result<Option<(TextScreen, Option<SauceRecord>)>> {
    let mut result = TextBuffer::new((80, 25));
    result.terminal_state.is_terminal_buffer = false;
    result.layers.clear();

    // Track how many lines were decoded per layer so `LAYER_i~k` continues at the correct y.
    let mut layer_resume_y: HashMap<usize, i32> = HashMap::new();
    let mut sauce_opt: Option<SauceRecord> = None;

    let mut decoder = png::StreamingDecoder::new();
    let mut len = 0;
    let mut last_uncompressed_info = 0usize;
    let mut last_compressed_info = 0usize;
    let mut is_running = true;

    // We only claim the file if we actually see an ICED v0 header.
    let mut saw_iced_v0 = false;

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

                    let keep_running = process_icy_draw_v0_decoded_chunk(
                        chunk.keyword.as_str(),
                        &decoded,
                        &mut result,
                        &mut layer_resume_y,
                        &mut saw_iced_v0,
                        &mut sauce_opt,
                    )?;
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

                    let keep_running = process_icy_draw_v0_decoded_chunk(
                        chunk.keyword.as_str(),
                        &decoded,
                        &mut result,
                        &mut layer_resume_y,
                        &mut saw_iced_v0,
                        &mut sauce_opt,
                    )?;
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

    if !saw_iced_v0 {
        return Ok(None);
    }

    Ok(Some((TextScreen::from_buffer(result), sauce_opt)))
}

fn is_short_attr(mut attr_raw: u16) -> (bool, u16) {
    let is_short = (attr_raw & V0_SHORT_DATA_MASK) != 0;
    if is_short {
        attr_raw &= !V0_SHORT_DATA_MASK;
    }
    (is_short, attr_raw)
}

fn process_icy_draw_v0_decoded_chunk(
    keyword: &str,
    bytes: &[u8],
    result: &mut TextBuffer,
    layer_resume_y: &mut HashMap<usize, i32>,
    saw_iced_v0: &mut bool,
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
            if version != 0 {
                // Not our file: let the caller fall back.
                return Ok(true);
            }
            if bytes.len() != 19 {
                return Err(crate::EngineError::UnsupportedFormat {
                    description: format!("unsupported ICED v0 header size {}", bytes.len()),
                });
            }

            *saw_iced_v0 = true;

            let mut o: usize = 2; // skip version

            // Read Type field: [compression: u8][sixel_format: u8][reserved: u16]
            // V0 had no compression/chunk versions, but keep parsing for compatibility.
            if bytes.len() < o + 4 {
                return Err(crate::EngineError::OutOfBounds { offset: o + 4 });
            }
            o += 4;

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

            // legacey
            let _palette_mode = bytes[o];
            o += 1;

            let font_mode = bytes[o];
            o += 1;
            result.font_mode = crate::FontMode::from_byte(font_mode);

            if bytes.len() < o + 8 {
                return Err(crate::EngineError::OutOfBounds { offset: o + 8 });
            }
            let width_u32 = u32::from_le_bytes(bytes[o..(o + 4)].try_into().unwrap());
            o += 4;
            let height_u32 = u32::from_le_bytes(bytes[o..(o + 4)].try_into().unwrap());

            let width: i32 = i32::try_from(width_u32).map_err(|_| crate::EngineError::UnsupportedFormat {
                description: format!("ICED width out of range: {width_u32}"),
            })?;
            let height: i32 = i32::try_from(height_u32).map_err(|_| crate::EngineError::UnsupportedFormat {
                description: format!("ICED height out of range: {height_u32}"),
            })?;
            result.set_size((width, height));
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
                let attr_raw = u16::from_le_bytes(cur[..2].try_into().unwrap());
                cur = &cur[2..];

                let (is_short, attr) = is_short_attr(attr_raw);

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

                let mut text_attr = TextAttribute::default();
                text_attr.attr = attr;
                text_attr.set_font_page(font_page);
                text_attr.set_foreground_color(decode_legacy_color(fg, ext_attr, true));
                text_attr.set_background_color(decode_legacy_color(bg, ext_attr, false));

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

        text => {
            if let Some(font_slot) = text.strip_prefix("FONT_") {
                let font_slot: usize = font_slot.parse().map_err(|e| crate::EngineError::UnsupportedFormat {
                    description: format!("invalid font slot '{font_slot}': {e}"),
                })?;

                let mut o: usize = 0;
                let (font_name, size) = read_utf8_encoded_string(&bytes[o..])?;
                o += size;
                let font = BitFont::from_bytes(font_name, &bytes[o..])?;

                // Sync font_cell_size with the actual font dimensions for slot 0
                // This ensures buffer.font_dimensions() matches the embedded font size
                if font_slot == 0 {
                    result.set_font_dimensions(font.size());
                }

                result.set_font(font_slot as u8, font);
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
                                let attr_raw = u16::from_le_bytes(bytes[o..(o + 2)].try_into().unwrap());
                                o += 2;
                                if attr_raw == V0_EOL {
                                    break;
                                }

                                let (is_short, attr) = is_short_attr(attr_raw);
                                if attr == V0_INVISIBLE_CELL {
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

                                let mut text_attr = TextAttribute::default();
                                text_attr.attr = attr;
                                text_attr.set_font_page(font_page);
                                text_attr.set_foreground_color(decode_legacy_color(fg, ext_attr, true));
                                text_attr.set_background_color(decode_legacy_color(bg, ext_attr, false));

                                layer.set_char((x, y), crate::AttributedChar { ch, attribute: text_attr });
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
            let _transparency = bytes[o];
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
            // default_font_page was removed - skip 2 bytes for compatibility with old files
            let _default_font_page = u16::from_le_bytes(bytes[o..(o + 2)].try_into().unwrap());
            o += 2;

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

                if bytes.len() < o + length {
                    return Err(crate::EngineError::OutOfBounds { offset: o + length });
                }

                // V0 stored raw RGBA.
                let picture_data = bytes[o..(o + length)].to_vec();

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
                        let attr_raw = u16::from_le_bytes(bytes[o..(o + 2)].try_into().unwrap());
                        o += 2;
                        if attr_raw == V0_EOL {
                            break;
                        }

                        let (is_short, attr) = is_short_attr(attr_raw);
                        if attr == V0_INVISIBLE_CELL {
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

                        let mut text_attr = TextAttribute::default();
                        text_attr.attr = attr;
                        text_attr.set_font_page(font_page);
                        text_attr.set_foreground_color(decode_legacy_color(fg, ext_attr, true));
                        text_attr.set_background_color(decode_legacy_color(bg, ext_attr, false));

                        layer.set_char((x, y), crate::AttributedChar { ch, attribute: text_attr });
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
                layer.properties.is_visible = (flags & 0b0000_0001) == 0b0000_0001;
                layer.properties.is_locked = (flags & 0b0000_0100) == 0b0000_0100;
                layer.properties.is_position_locked = (flags & 0b0000_0010) == 0b0000_0010;
                layer.properties.has_alpha_channel = (flags & 0b0000_1000) == 0b0000_1000;
                layer.properties.is_alpha_channel_locked = (flags & 0b0001_0000) == 0b0001_0000;
            }
        }
    }

    Ok(true)
}

fn read_utf8_encoded_string(data: &[u8]) -> Result<(String, usize)> {
    if data.len() < 4 {
        return Err(crate::EngineError::OutOfBounds { offset: 4 });
    }

    let size: usize = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;
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
