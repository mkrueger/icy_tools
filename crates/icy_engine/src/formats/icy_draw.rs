use std::{error::Error, fmt::Alignment, path::Path};

use base64::{Engine, engine::general_purpose};
use icy_sauce::SauceInformation;
use regex::Regex;

use crate::{BitFont, Buffer, Color, EngineResult, Layer, LoadingError, OutputFormat, Palette, Position, SaveOptions, Sixel, Size, TextPane, attribute};

use super::LoadData;

mod constants {
    pub const ICD_VERSION: u16 = 0;
    pub const ICED_HEADER_SIZE: usize = 19;
    pub mod layer {
        pub const IS_VISIBLE: u32 = 0b0000_0001;
        pub const POS_LOCK: u32 = 0b0000_0010;
        pub const EDIT_LOCK: u32 = 0b0000_0100;
        pub const HAS_ALPHA: u32 = 0b0000_1000;
        pub const ALPHA_LOCKED: u32 = 0b0001_0000;
    }
}

#[derive(Default)]
pub struct IcyDraw {}

/// maximum ztext chunk size from libpng source
const MAX: u64 = 3_000_000;
lazy_static::lazy_static! {
    static ref LAYER_CONTINUE_REGEX: Regex = Regex::new(r"LAYER_(\d+)~(\d+)").unwrap();
}

impl OutputFormat for IcyDraw {
    fn get_file_extension(&self) -> &str {
        "icy"
    }

    fn get_name(&self) -> &str {
        "Iced"
    }

    fn to_bytes(&self, buf: &mut crate::Buffer, _options: &SaveOptions) -> EngineResult<Vec<u8>> {
        let mut result = Vec::new();

        let font_dims = buf.get_font_dimensions();
        let mut width = buf.get_width() * font_dims.width;

        let mut first_line = 0;
        while first_line < buf.get_height() {
            if !buf.is_line_empty(first_line) {
                break;
            }
            first_line += 1;
        }

        let last_line = (first_line + MAX_LINES).min(buf.get_line_count().max(buf.get_height()));
        let mut height = (last_line - first_line) * font_dims.height;

        let image_empty = if width == 0 || height == 0 {
            width = 1;
            height = 1;
            true
        } else {
            false
        };

        let mut encoder: png::Encoder<'_, &mut Vec<u8>> = png::Encoder::new(&mut result, width as u32, height as u32); // Width is 2 pixels and height is 1.
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Best);

        {
            let mut result = vec![constants::ICD_VERSION as u8, (constants::ICD_VERSION >> 8) as u8];
            result.extend(u32::to_le_bytes(0)); // Type
            // Modes
            result.extend(u16::to_le_bytes(buf.buffer_type.to_byte() as u16));
            result.push(buf.ice_mode.to_byte());
            result.push(buf.palette_mode.to_byte());
            result.push(buf.font_mode.to_byte());

            result.extend(u32::to_le_bytes(buf.get_width() as u32));
            result.extend(u32::to_le_bytes(buf.get_height() as u32));
            let sauce_data = general_purpose::STANDARD.encode(&result);
            if let Err(err) = encoder.add_ztxt_chunk("ICED".to_string(), sauce_data) {
                return Err(IcedError::ErrorEncodingZText(format!("{err}")).into());
            }
        }

        if buf.has_sauce() {
            let mut sauce_vec: Vec<u8> = Vec::new();
            buf.write_sauce_info(icy_sauce::SauceDataType::Character, icy_sauce::char_caps::ContentType::Ansi, &mut sauce_vec)?;
            let sauce_data = general_purpose::STANDARD.encode(&sauce_vec);
            if let Err(err) = encoder.add_ztxt_chunk("SAUCE".to_string(), sauce_data) {
                return Err(IcedError::ErrorEncodingZText(format!("{err}")).into());
            }
        }

        if !buf.palette.is_default() {
            let pal_data = buf.palette.export_palette(&crate::PaletteFormat::Ice);
            let palette_data = general_purpose::STANDARD.encode(pal_data);
            if let Err(err) = encoder.add_ztxt_chunk("PALETTE".to_string(), palette_data) {
                return Err(IcedError::ErrorEncodingZText(format!("{err}")).into());
            }
        }

        for (k, v) in buf.font_iter() {
            let mut font_data: Vec<u8> = Vec::new();
            write_utf8_encoded_string(&mut font_data, &v.name);
            font_data.extend(v.to_psf2_bytes().unwrap());

            if let Err(err) = encoder.add_ztxt_chunk(format!("FONT_{k}"), general_purpose::STANDARD.encode(&font_data)) {
                return Err(IcedError::ErrorEncodingZText(format!("{err}")).into());
            }
        }

        for (i, layer) in buf.layers.iter().enumerate() {
            let mut result: Vec<u8> = Vec::new();
            write_utf8_encoded_string(&mut result, &layer.properties.title);

            match layer.role {
                crate::Role::Image => result.push(1),
                _ => result.push(0),
            }

            // Some extra bytes not yet used
            result.extend([0, 0, 0, 0]);

            let mode = match layer.properties.mode {
                crate::Mode::Normal => 0,
                crate::Mode::Chars => 1,
                crate::Mode::Attributes => 2,
            };
            result.push(mode);

            if let Some(color) = &layer.properties.color {
                let (r, g, b) = color.clone().get_rgb();
                result.push(r);
                result.push(g);
                result.push(b);
                result.push(0xFF);
            } else {
                result.extend([0, 0, 0, 0]);
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
            result.extend(u32::to_le_bytes(flags));
            result.push(layer.transparency);

            result.extend(i32::to_le_bytes(layer.get_offset().x));
            result.extend(i32::to_le_bytes(layer.get_offset().y));

            result.extend(i32::to_le_bytes(layer.get_width()));
            result.extend(i32::to_le_bytes(layer.get_height()));
            result.extend(u16::to_le_bytes(layer.default_font_page as u16));

            if matches!(layer.role, crate::Role::Image) {
                let sixel = &layer.sixels[0];
                let sixel_header_size = 16;
                let len = sixel_header_size + sixel.picture_data.len() as u64;

                let mut bytes_written = MAX.min(len);
                result.extend(u64::to_le_bytes(bytes_written));

                result.extend(i32::to_le_bytes(sixel.get_width()));
                result.extend(i32::to_le_bytes(sixel.get_height()));
                result.extend(i32::to_le_bytes(sixel.vertical_scale));
                result.extend(i32::to_le_bytes(sixel.horizontal_scale));
                bytes_written -= sixel_header_size;
                result.extend(&sixel.picture_data[0..bytes_written as usize]);
                let layer_data = general_purpose::STANDARD.encode(&result);
                if let Err(err) = encoder.add_ztxt_chunk(format!("LAYER_{i}"), layer_data) {
                    return Err(IcedError::ErrorEncodingZText(format!("{err}")).into());
                }

                let mut chunk = 1;
                let len = sixel.picture_data.len() as u64;
                while len > bytes_written {
                    let next_bytes = MAX.min(len - bytes_written);
                    let layer_data =
                        general_purpose::STANDARD.encode(&sixel.picture_data[bytes_written as usize..(bytes_written as usize + next_bytes as usize)]);
                    bytes_written += next_bytes;
                    if let Err(err) = encoder.add_ztxt_chunk(format!("LAYER_{i}~{chunk}"), layer_data) {
                        return Err(IcedError::ErrorEncodingZText(format!("{err}")).into());
                    }
                    chunk += 1;
                }
            } else {
                let offset = result.len();
                result.extend(u64::to_le_bytes(0));

                let mut y = 0;

                while y < layer.get_height() {
                    if result.len() as u64 + layer.get_width() as u64 * 16 > MAX {
                        break;
                    }
                    let real_length = get_invisible_line_length(layer, y);
                    for x in 0..real_length {
                        let ch = layer.get_char((x, y));
                        let mut attr = ch.attribute.attr;

                        let is_short = if ch.is_visible()
                            && ch.ch as u32 <= 255
                            && ch.attribute.foreground_color <= 255
                            && ch.attribute.background_color <= 255
                            && ch.attribute.font_page <= 255
                        {
                            attr |= attribute::SHORT_DATA;
                            true
                        } else {
                            false
                        };

                        result.extend(u16::to_le_bytes(attr));
                        if !ch.is_visible() {
                            continue;
                        }

                        if is_short {
                            result.push(ch.ch as u8);
                            result.push(ch.attribute.foreground_color as u8);
                            result.push(ch.attribute.background_color as u8);
                            result.push(ch.attribute.font_page as u8);
                        } else {
                            result.extend(u32::to_le_bytes(ch.ch as u32));
                            result.extend(u32::to_le_bytes(ch.attribute.foreground_color));
                            result.extend(u32::to_le_bytes(ch.attribute.background_color));
                            result.extend(u16::to_le_bytes(ch.attribute.font_page as u16));
                        }
                    }
                    if layer.get_width() > real_length {
                        result.extend(u16::to_le_bytes(attribute::INVISIBLE_SHORT));
                    }
                    y += 1;
                }
                let len = result.len();
                result[offset..(offset + 8)].copy_from_slice(&u64::to_le_bytes((len - offset - 8) as u64));
                let layer_data = general_purpose::STANDARD.encode(&result);
                if let Err(err) = encoder.add_ztxt_chunk(format!("LAYER_{i}"), layer_data) {
                    return Err(IcedError::ErrorEncodingZText(format!("{err}")).into());
                }
                let mut chunk = 1;
                while y < layer.get_height() {
                    result.clear();
                    while y < layer.get_height() {
                        if result.len() as u64 + layer.get_width() as u64 * 16 > MAX {
                            break;
                        }
                        let real_length = get_invisible_line_length(layer, y);

                        for x in 0..real_length {
                            let ch = layer.get_char((x, y));
                            let mut attr = ch.attribute.attr;

                            let is_short = if ch.is_visible()
                                && ch.ch as u32 <= 255
                                && ch.attribute.foreground_color <= 255
                                && ch.attribute.background_color <= 255
                                && ch.attribute.font_page <= 255
                            {
                                attr |= attribute::SHORT_DATA;
                                true
                            } else {
                                false
                            };

                            result.extend(u16::to_le_bytes(attr));
                            if !ch.is_visible() {
                                continue;
                            }
                            if is_short {
                                result.push(ch.ch as u8);
                                result.push(ch.attribute.foreground_color as u8);
                                result.push(ch.attribute.background_color as u8);
                                result.push(ch.attribute.font_page as u8);
                            } else {
                                result.extend(u32::to_le_bytes(ch.ch as u32));
                                result.extend(u32::to_le_bytes(ch.attribute.foreground_color));
                                result.extend(u32::to_le_bytes(ch.attribute.background_color));
                                result.extend(u16::to_le_bytes(ch.attribute.font_page as u16));
                            }
                        }
                        if layer.get_width() > real_length {
                            result.extend(u16::to_le_bytes(attribute::INVISIBLE_SHORT));
                        }
                        y += 1;
                    }
                    let layer_data = general_purpose::STANDARD.encode(&result);
                    if let Err(err) = encoder.add_ztxt_chunk(format!("LAYER_{i}~{chunk}"), layer_data) {
                        return Err(IcedError::ErrorEncodingZText(format!("{err}")).into());
                    }
                    chunk += 1;
                }
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
                let mut attr = tag.attribute.attr;

                let is_short = if tag.attribute.foreground_color <= 255 && tag.attribute.background_color <= 255 && tag.attribute.font_page <= 255 {
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
                    data.extend(u16::to_le_bytes(tag.attribute.font_page as u16));
                }
                // unused data for future use
                data.extend(&[0, 0, 0, 0]);
                data.extend(&[0, 0, 0, 0]);
                data.extend(&[0, 0, 0, 0]);
                data.extend(&[0, 0, 0, 0]);
            }

            let tag_data = general_purpose::STANDARD.encode(&data);
            if let Err(err) = encoder.add_ztxt_chunk("TAG".to_string(), tag_data) {
                return Err(IcedError::ErrorEncodingZText(format!("{err}")).into());
            }
        }

        if let Err(err) = encoder.add_ztxt_chunk("END".to_string(), String::new()) {
            return Err(IcedError::ErrorEncodingZText(format!("{err}")).into());
        }

        let mut writer = encoder.write_header().unwrap();

        if image_empty {
            writer.write_image_data(&[0, 0, 0, 0]).unwrap();
        } else {
            let (_, data) = buf.render_to_rgba(
                crate::Rectangle {
                    start: Position::new(0, first_line),
                    size: Size::new(buf.get_width(), last_line - first_line),
                },
                true,
            );
            writer.write_image_data(&data).unwrap();
        }
        writer.finish().unwrap();

        Ok(result)
    }

    fn load_buffer(&self, file_name: &Path, data: &[u8], _load_data_opt: Option<LoadData>) -> EngineResult<crate::Buffer> {
        let mut result = Buffer::new((80, 25));
        result.is_terminal_buffer = false;
        result.file_name = Some(file_name.into());
        result.layers.clear();

        let mut decoder = png::StreamingDecoder::new();
        let mut len = 0;
        let mut last_info = 0;
        let mut is_running = true;
        while is_running {
            match decoder.update(&data[len..], &mut Vec::new()) {
                Ok((b, _)) => {
                    len += b;
                    if data.len() <= len {
                        break;
                    }
                    if let Some(info) = decoder.info() {
                        for i in last_info..info.compressed_latin1_text.len() {
                            let chunk = &info.compressed_latin1_text[i];
                            let Ok(text) = chunk.get_text() else {
                                log::error!("error decoding iced chunk: {}", chunk.keyword);
                                continue;
                            };

                            let bytes = match general_purpose::STANDARD.decode(text) {
                                Ok(data) => data,
                                Err(e) => {
                                    log::warn!("error decoding iced chunk: {e}");
                                    continue;
                                }
                            };
                            match chunk.keyword.as_str() {
                                "END" => {
                                    is_running = false;
                                    break;
                                }
                                "ICED" => {
                                    let mut o: usize = 0;
                                    if bytes.len() != constants::ICED_HEADER_SIZE {
                                        return Err(anyhow::anyhow!("unsupported header size {}", bytes.len()));
                                    }
                                    o += 2; // skip version
                                    // TODO: read type ATM only 1 type is generated.
                                    o += 4; // skip type
                                    let buffer_type = u16::from_le_bytes(bytes[o..(o + 2)].try_into().unwrap());
                                    o += 2;
                                    result.buffer_type = crate::BufferType::from_byte(buffer_type as u8);
                                    let ice_mode = bytes[o];
                                    o += 1;
                                    result.ice_mode = crate::IceMode::from_byte(ice_mode);

                                    let palette_mode = bytes[o];
                                    o += 1;
                                    result.palette_mode = crate::PaletteMode::from_byte(palette_mode);

                                    let font_mode = bytes[o];
                                    o += 1;
                                    result.font_mode = crate::FontMode::from_byte(font_mode);

                                    let width: i32 = u32::from_le_bytes(bytes[o..(o + 4)].try_into().unwrap()) as i32;
                                    o += 4;
                                    let height: i32 = u32::from_le_bytes(bytes[o..(o + 4)].try_into().unwrap()) as i32;
                                    result.set_size((width, height));
                                }

                                "PALETTE" => {
                                    result.palette = Palette::load_palette(&crate::PaletteFormat::Ice, &bytes)?;
                                }

                                "SAUCE" => {
                                    if let Some(sauce) = SauceInformation::read(&bytes)? {
                                        result.load_sauce(sauce);
                                    }
                                }

                                "TAG" => {
                                    let mut bytes = &bytes[..];
                                    let tag_len = u16::from_le_bytes(bytes[..2].try_into().unwrap());
                                    bytes = &bytes[2..];
                                    for _ in 0..tag_len {
                                        let (preview, len) = read_utf8_encoded_string(&bytes);
                                        bytes = &bytes[len..];
                                        let (replacement_value, len) = read_utf8_encoded_string(&bytes);
                                        bytes = &bytes[len..];
                                        let x = i32::from_le_bytes(bytes[..4].try_into().unwrap());
                                        bytes = &bytes[4..];
                                        let y = i32::from_le_bytes(bytes[..4].try_into().unwrap());
                                        bytes = &bytes[4..];
                                        let length = u16::from_le_bytes(bytes[..2].try_into().unwrap());
                                        bytes = &bytes[2..];
                                        let is_enabled = bytes[0] == 1;
                                        bytes = &bytes[1..];
                                        let alignment = match bytes[0] {
                                            0 => Alignment::Left,
                                            1 => Alignment::Center,
                                            2 => Alignment::Right,
                                            _ => {
                                                return Err(anyhow::anyhow!("unsupported alignment"));
                                            }
                                        };
                                        bytes = &bytes[1..];
                                        let tag_placement = match bytes[0] {
                                            0 => crate::TagPlacement::InText,
                                            1 => crate::TagPlacement::WithGotoXY,
                                            _ => {
                                                return Err(anyhow::anyhow!("unsupported tag placement"));
                                            }
                                        };
                                        bytes = &bytes[1..];

                                        let tag_role = match bytes[0] {
                                            0 => crate::TagRole::Displaycode,
                                            1 => crate::TagRole::Hyperlink,
                                            _ => {
                                                return Err(anyhow::anyhow!("unsupported tag role"));
                                            }
                                        };

                                        bytes = &bytes[1..];

                                        let mut attr = u16::from_le_bytes(bytes[..2].try_into().unwrap());
                                        bytes = &bytes[2..];
                                        let is_short = if (attr & attribute::SHORT_DATA) == 0 {
                                            false
                                        } else {
                                            attr &= !attribute::SHORT_DATA;
                                            true
                                        };
                                        let (fg, bg, font_page) = if is_short {
                                            let r = (bytes[0] as u32, bytes[1] as u32, bytes[2] as u16);
                                            bytes = &bytes[3..];
                                            r
                                        } else {
                                            let r = (
                                                u32::from_le_bytes(bytes[..4].try_into().unwrap()),
                                                u32::from_le_bytes(bytes[4..8].try_into().unwrap()),
                                                u16::from_le_bytes(bytes[8..10].try_into().unwrap()),
                                            );
                                            bytes = &bytes[10..];
                                            r
                                        };
                                        bytes = &bytes[4..]; // skip unused data
                                        bytes = &bytes[4..]; // skip unused data
                                        bytes = &bytes[4..]; // skip unused data
                                        bytes = &bytes[4..]; // skip unused data

                                        result.tags.push(crate::Tag {
                                            preview,
                                            replacement_value,
                                            position: Position::new(x, y),
                                            length: length as usize,
                                            is_enabled,
                                            alignment,
                                            tag_placement,
                                            tag_role,
                                            attribute: crate::TextAttribute {
                                                foreground_color: fg,
                                                background_color: bg,
                                                font_page: font_page as usize,
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
                                                let (font_name, size) = read_utf8_encoded_string(&bytes[o..]);
                                                o += size;
                                                let font = BitFont::from_bytes(font_name, &bytes[o..])?;
                                                result.set_font(font_slot, font);
                                                continue;
                                            }
                                            Err(err) => {
                                                return Err(IcedError::ErrorParsingFontSlot(format!("{err}")).into());
                                            }
                                        }
                                    }
                                    if !text.starts_with("LAYER_") {
                                        log::warn!("unsupported chunk {text}");
                                        continue;
                                    }

                                    if let Some(m) = LAYER_CONTINUE_REGEX.captures(text) {
                                        let (_, [layer_num, _chunk]) = m.extract();
                                        let layer_num = layer_num.parse::<usize>()?;

                                        let layer = &mut result.layers[layer_num];
                                        match layer.role {
                                            crate::Role::Normal => {
                                                let mut o = 0;
                                                for y in layer.get_line_count()..layer.get_height() {
                                                    if o >= bytes.len() {
                                                        // will be continued in a later chunk.
                                                        break;
                                                    }
                                                    for x in 0..layer.get_width() {
                                                        let mut attr = u16::from_le_bytes(bytes[o..(o + 2)].try_into().unwrap());
                                                        o += 2;
                                                        if attr == crate::attribute::INVISIBLE_SHORT {
                                                            // end of line
                                                            break;
                                                        }
                                                        let is_short = if (attr & attribute::SHORT_DATA) == 0 {
                                                            false
                                                        } else {
                                                            attr &= !attribute::SHORT_DATA;
                                                            true
                                                        };
                                                        if attr == crate::attribute::INVISIBLE {
                                                            // default char
                                                            continue;
                                                        }

                                                        let (ch, fg, bg, font_page) = if is_short {
                                                            let ch = bytes[o] as u32;
                                                            o += 1;
                                                            let fg = bytes[o] as u32;
                                                            o += 1;
                                                            let bg = bytes[o] as u32;
                                                            o += 1;
                                                            let font_page = bytes[o] as u16;
                                                            o += 1;
                                                            (ch, fg, bg, font_page)
                                                        } else {
                                                            let ch = u32::from_le_bytes(bytes[o..(o + 4)].try_into().unwrap());
                                                            o += 4;
                                                            let fg = u32::from_le_bytes(bytes[o..(o + 4)].try_into().unwrap());
                                                            o += 4;
                                                            let bg = u32::from_le_bytes(bytes[o..(o + 4)].try_into().unwrap());
                                                            o += 4;
                                                            let font_page = u16::from_le_bytes(bytes[o..(o + 2)].try_into().unwrap());
                                                            o += 2;
                                                            (ch, fg, bg, font_page)
                                                        };

                                                        layer.set_char(
                                                            (x, y),
                                                            crate::AttributedChar {
                                                                ch: unsafe { char::from_u32_unchecked(ch) },
                                                                attribute: crate::TextAttribute {
                                                                    foreground_color: fg,
                                                                    background_color: bg,
                                                                    font_page: font_page as usize,
                                                                    attr,
                                                                },
                                                            },
                                                        );
                                                    }
                                                }
                                                continue;
                                            }
                                            crate::Role::PastePreview => todo!(),
                                            crate::Role::PasteImage => todo!(),
                                            crate::Role::Image => {
                                                layer.sixels[0].picture_data.extend(&bytes);
                                                continue;
                                            }
                                        }
                                    }
                                    let mut o: usize = 0;

                                    let (title, size) = read_utf8_encoded_string(&bytes[o..]);
                                    let mut layer = Layer::new(title, (0, 0));

                                    o += size;
                                    let role = bytes[o];
                                    o += 1;
                                    if role == 1 {
                                        layer.role = crate::Role::Image;
                                    } else {
                                        layer.role = crate::Role::Normal;
                                    }

                                    o += 4; // skip unused

                                    let mode = bytes[o];

                                    layer.properties.mode = match mode {
                                        0 => crate::Mode::Normal,
                                        1 => crate::Mode::Chars,
                                        2 => crate::Mode::Attributes,
                                        _ => {
                                            return Err(LoadingError::IcyDrawUnsupportedLayerMode(mode).into());
                                        }
                                    };
                                    o += 1;

                                    // read layer color
                                    let red = bytes[o];
                                    o += 1;
                                    let green = bytes[o];
                                    o += 1;
                                    let blue = bytes[o];
                                    o += 1;
                                    let alpha = bytes[o];
                                    o += 1;
                                    if alpha != 0 {
                                        layer.properties.color = Some(Color::new(red, green, blue));
                                    }

                                    let flags = u32::from_le_bytes(bytes[o..(o + 4)].try_into().unwrap());
                                    o += 4;

                                    layer.transparency = bytes[o];
                                    o += 1;

                                    let x_offset: i32 = u32::from_le_bytes(bytes[o..(o + 4)].try_into().unwrap()) as i32;
                                    o += 4;
                                    let y_offset: i32 = u32::from_le_bytes(bytes[o..(o + 4)].try_into().unwrap()) as i32;
                                    o += 4;
                                    layer.set_offset((x_offset, y_offset));

                                    let width: i32 = u32::from_le_bytes(bytes[o..(o + 4)].try_into().unwrap()) as i32;
                                    o += 4;
                                    let height: i32 = u32::from_le_bytes(bytes[o..(o + 4)].try_into().unwrap()) as i32;
                                    o += 4;
                                    layer.set_size((width, height));
                                    let default_font_page = u16::from_le_bytes(bytes[o..(o + 2)].try_into().unwrap());
                                    o += 2;
                                    layer.default_font_page = default_font_page as usize;

                                    let length = u64::from_le_bytes(bytes[o..(o + 8)].try_into().unwrap()) as usize;
                                    o += 8;

                                    if role == 1 {
                                        let width: i32 = u32::from_le_bytes(bytes[o..(o + 4)].try_into().unwrap()) as i32;
                                        o += 4;
                                        let height: i32 = u32::from_le_bytes(bytes[o..(o + 4)].try_into().unwrap()) as i32;
                                        o += 4;

                                        let vert_scale: i32 = u32::from_le_bytes(bytes[o..(o + 4)].try_into().unwrap()) as i32;
                                        o += 4;
                                        let horiz_scale: i32 = u32::from_le_bytes(bytes[o..(o + 4)].try_into().unwrap()) as i32;
                                        o += 4;
                                        layer
                                            .sixels
                                            .push(Sixel::from_data((width, height), vert_scale, horiz_scale, bytes[o..].to_vec()));
                                        result.layers.push(layer);
                                    } else {
                                        if bytes.len() < o + length {
                                            return Err(anyhow::anyhow!("data length out ouf bounds {} data lenth: {}", o + length, bytes.len()));
                                        }
                                        for y in 0..height {
                                            if o >= bytes.len() {
                                                // will be continued in a later chunk.
                                                break;
                                            }
                                            for x in 0..width {
                                                if o + 2 > bytes.len() {
                                                    return Err(anyhow::anyhow!("data length out ouf bounds"));
                                                }
                                                let mut attr = u16::from_le_bytes(bytes[o..(o + 2)].try_into().unwrap());
                                                o += 2;
                                                if attr == crate::attribute::INVISIBLE_SHORT {
                                                    // end of line
                                                    break;
                                                }

                                                let is_short = if (attr & attribute::SHORT_DATA) == 0 {
                                                    false
                                                } else {
                                                    attr &= !attribute::SHORT_DATA;
                                                    true
                                                };

                                                if attr == crate::attribute::INVISIBLE {
                                                    // default char
                                                    continue;
                                                }

                                                let (ch, fg, bg, font_page) = if is_short {
                                                    if o + 3 > bytes.len() {
                                                        return Err(anyhow::anyhow!("data length out ouf bounds"));
                                                    }

                                                    let ch = bytes[o] as u32;
                                                    o += 1;
                                                    let fg = bytes[o] as u32;
                                                    o += 1;
                                                    let bg = bytes[o] as u32;
                                                    o += 1;
                                                    let font_page = bytes[o] as u16;
                                                    o += 1;
                                                    (ch, fg, bg, font_page)
                                                } else {
                                                    if o + 14 > bytes.len() {
                                                        return Err(anyhow::anyhow!("data length out ouf bounds"));
                                                    }

                                                    let ch = u32::from_le_bytes(bytes[o..(o + 4)].try_into().unwrap());
                                                    o += 4;
                                                    let fg = u32::from_le_bytes(bytes[o..(o + 4)].try_into().unwrap());
                                                    o += 4;
                                                    let bg = u32::from_le_bytes(bytes[o..(o + 4)].try_into().unwrap());
                                                    o += 4;

                                                    let font_page = u16::from_le_bytes(bytes[o..(o + 2)].try_into().unwrap());
                                                    o += 2;
                                                    (ch, fg, bg, font_page)
                                                };

                                                layer.set_char(
                                                    (x, y),
                                                    crate::AttributedChar {
                                                        ch: unsafe { char::from_u32_unchecked(ch) },
                                                        attribute: crate::TextAttribute {
                                                            foreground_color: fg,
                                                            background_color: bg,
                                                            font_page: font_page as usize,
                                                            attr,
                                                        },
                                                    },
                                                );
                                            }
                                        }
                                        result.layers.push(layer);
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
                        }
                        last_info = info.compressed_latin1_text.len();
                    }
                }
                Err(err) => {
                    return Err(LoadingError::InvalidPng(format!("{err}")).into());
                }
            }
        }

        Ok(result)
    }
}

fn get_invisible_line_length(layer: &Layer, y: i32) -> i32 {
    let mut length = layer.get_width();
    while length > 0 && !layer.get_char((length - 1, y)).is_visible() {
        length -= 1;
    }
    length
}

fn read_utf8_encoded_string(data: &[u8]) -> (String, usize) {
    let size = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;
    (unsafe { String::from_utf8_unchecked(data[4..(4 + size)].to_vec()) }, size + 4)
}

fn write_utf8_encoded_string(data: &mut Vec<u8>, s: &str) {
    data.extend(u32::to_le_bytes(s.len() as u32));
    data.extend(s.as_bytes());
}

const MAX_LINES: i32 = 80;

impl Buffer {
    pub fn is_line_empty(&self, line: i32) -> bool {
        for i in 0..self.get_width() {
            if !self.get_char((i, line)).is_transparent() {
                return false;
            }
        }
        true
    }
}

#[derive(Debug, Clone)]
pub enum IcedError {
    ErrorEncodingZText(String),
    ErrorParsingFontSlot(String),
}

impl std::fmt::Display for IcedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IcedError::ErrorEncodingZText(err) => {
                write!(f, "Error while encoding ztext chunk: {err}")
            }
            IcedError::ErrorParsingFontSlot(err) => {
                write!(f, "Error while parsing font slot: {err}")
            }
        }
    }
}

impl Error for IcedError {
    fn description(&self) -> &str {
        "use std::display"
    }

    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }

    fn cause(&self) -> Option<&dyn Error> {
        self.source()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::{AttributedChar, Buffer, Color, Layer, OutputFormat, SaveOptions, TextAttribute, TextPane, compare_buffers};

    use super::IcyDraw;
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
                                let draw = IcyDraw::default();
                                let bytes = draw.to_bytes(&buf, &SaveOptions::default()).unwrap();
                                let buf2 = draw
                                    .load_buffer(Path::new("test.icy"), &bytes, None)
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
            let draw = IcyDraw::default();
            let bytes = draw.to_bytes(&buf, &SaveOptions::default()).unwrap();
            let buf2 = draw
                .load_buffer(Path::new("test.icy"), &bytes, None)
                .unwrap();
            compare_buffers(&buf, &buf2);
        }
    */

    #[test]
    fn test_default_font_page() {
        let mut buf = Buffer::default();
        buf.layers[0].default_font_page = 12;
        buf.layers.push(Layer::new("test", (80, 25)));
        buf.layers[1].default_font_page = 1;

        let draw = IcyDraw::default();
        let bytes = draw.to_bytes(&mut buf, &SaveOptions::default()).unwrap();
        let buf2 = draw.load_buffer(Path::new("test.icy"), &bytes, None).unwrap();
        compare_buffers(&buf, &buf2, crate::CompareOptions::ALL);
    }

    #[test]
    fn test_empty_buffer() {
        let mut buf = Buffer::default();
        buf.set_width(12);
        buf.set_height(23);

        let draw = IcyDraw::default();
        let bytes = draw.to_bytes(&mut buf, &SaveOptions::default()).unwrap();
        let buf2 = draw.load_buffer(Path::new("test.icy"), &bytes, None).unwrap();
        compare_buffers(&buf, &buf2, crate::CompareOptions::ALL);
    }

    #[test]
    fn test_rgb_serialization_bug() {
        let mut buf = Buffer::new((2, 2));
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

        let draw = IcyDraw::default();
        let bytes = draw.to_bytes(&mut buf, &SaveOptions::default()).unwrap();
        let buf2 = draw.load_buffer(Path::new("test.icy"), &bytes, None).unwrap();
        compare_buffers(&buf, &buf2, crate::CompareOptions::ALL);
    }

    #[test]
    fn test_rgb_serialization_bug_2() {
        // was a bug in compare_buffers, but having more test doesn't hurt.
        let mut buf = Buffer::new((2, 2));

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

        let draw = IcyDraw::default();
        let bytes = draw.to_bytes(&mut buf, &SaveOptions::default()).unwrap();
        let buf2 = draw.load_buffer(Path::new("test.icy"), &bytes, None).unwrap();
        compare_buffers(&buf, &buf2, crate::CompareOptions::ALL);
    }

    #[test]
    fn test_nonstandard_palettes() {
        // was a bug in compare_buffers, but having more test doesn't hurt.
        let mut buf = Buffer::new((2, 2));
        buf.palette.set_color(9, Color::new(4, 5, 6));
        buf.palette.set_color(10, Color::new(7, 8, 9));

        buf.layers[0].set_char(
            (0, 0),
            AttributedChar {
                ch: 'A',
                attribute: TextAttribute::new(9, 10),
            },
        );

        let draw = IcyDraw::default();
        let bytes = draw.to_bytes(&mut buf, &SaveOptions::default()).unwrap();
        let buf2 = draw.load_buffer(Path::new("test.icy"), &bytes, None).unwrap();

        compare_buffers(&buf, &buf2, crate::CompareOptions::ALL);
    }

    #[test]
    fn test_fg_switch() {
        // was a bug in compare_buffers, but having more test doesn't hurt.
        let mut buf = Buffer::new((2, 1));
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

        let draw = IcyDraw::default();
        let bytes = draw.to_bytes(&mut buf, &SaveOptions::default()).unwrap();
        let buf2 = draw.load_buffer(Path::new("test.icy"), &bytes, None).unwrap();

        compare_buffers(&buf, &buf2, crate::CompareOptions::ALL);
    }

    #[test]
    fn test_escape_char() {
        let mut buf = Buffer::new((2, 2));
        buf.layers[0].set_char(
            (0, 0),
            AttributedChar {
                ch: '\x1b',
                attribute: TextAttribute::default(),
            },
        );

        let draw = IcyDraw::default();
        let bytes = draw.to_bytes(&mut buf, &SaveOptions::default()).unwrap();
        let buf2 = draw.load_buffer(Path::new("test.icy"), &bytes, None).unwrap();
        compare_buffers(&buf, &buf2, crate::CompareOptions::ALL);
    }

    #[test]
    fn test_0_255_chars() {
        let mut buf = Buffer::new((2, 2));
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

        let draw = IcyDraw::default();
        let bytes = draw.to_bytes(&mut buf, &SaveOptions::default()).unwrap();
        let buf2 = draw.load_buffer(Path::new("test.icy"), &bytes, None).unwrap();
        compare_buffers(&buf, &buf2, crate::CompareOptions::ALL);
    }

    #[test]
    fn test_too_long_lines() {
        let mut buf = Buffer::new((2, 2));
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

        let draw = IcyDraw::default();
        let bytes = draw.to_bytes(&mut buf, &SaveOptions::default()).unwrap();
        let buf2 = draw.load_buffer(Path::new("test.icy"), &bytes, None).unwrap();
        compare_buffers(&buf, &buf2, crate::CompareOptions::ALL);
    }

    #[test]
    fn test_space_persistance_buffer() {
        let mut buf = Buffer::default();
        buf.layers[0].set_char(
            (0, 0),
            AttributedChar {
                ch: ' ',
                attribute: TextAttribute::default(),
            },
        );

        let draw = IcyDraw::default();
        let bytes = draw.to_bytes(&mut buf, &SaveOptions::default()).unwrap();
        let buf2 = draw.load_buffer(Path::new("test.icy"), &bytes, None).unwrap();
        compare_buffers(&buf, &buf2, crate::CompareOptions::ALL);
    }

    #[test]
    fn test_invisible_layer_bug() {
        let mut buf = Buffer::new((1, 1));
        buf.layers.push(Layer::new("test", (1, 1)));
        buf.layers[1].set_char((0, 0), AttributedChar::new('a', TextAttribute::default()));
        buf.layers[0].properties.is_visible = false;
        buf.layers[1].properties.is_visible = false;

        let draw = IcyDraw::default();
        let bytes = draw.to_bytes(&mut buf, &SaveOptions::default()).unwrap();
        let mut buf2 = draw.load_buffer(Path::new("test.icy"), &bytes, None).unwrap();

        compare_buffers(&buf, &buf2, crate::CompareOptions::ALL);
        buf2.layers[0].properties.is_visible = true;
        buf2.layers[1].properties.is_visible = true;
    }

    #[test]
    fn test_invisisible_persistance_bug() {
        let mut buf = Buffer::new((3, 1));
        buf.layers.push(Layer::new("test", (3, 1)));
        buf.layers[1].set_char((0, 0), AttributedChar::new('a', TextAttribute::default()));
        buf.layers[1].set_char((2, 0), AttributedChar::new('b', TextAttribute::default()));
        buf.layers[0].properties.is_visible = false;
        buf.layers[1].properties.is_visible = false;
        buf.layers[1].properties.has_alpha_channel = true;

        assert_eq!(AttributedChar::invisible(), buf.layers[1].get_char((1, 0)));

        let draw = IcyDraw::default();
        let bytes = draw.to_bytes(&mut buf, &SaveOptions::default()).unwrap();
        let mut buf2 = draw.load_buffer(Path::new("test.icy"), &bytes, None).unwrap();

        compare_buffers(&buf, &buf2, crate::CompareOptions::ALL);
        buf2.layers[0].properties.is_visible = true;
        buf2.layers[1].properties.is_visible = true;
    }
}
