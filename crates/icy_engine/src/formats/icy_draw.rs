use std::{fmt::Alignment, path::Path};

use base64::{Engine, engine::general_purpose};
use icy_sauce::SauceRecord;
use regex::Regex;

use crate::{BitFont, Color, Result, Layer, LoadingError, OutputFormat, Palette, Position, SaveOptions, Sixel, Size, TextBuffer, TextPane, attribute};

use super::LoadData;

mod constants {
    pub const ICED_VERSION: u16 = 0;
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

    fn to_bytes(&self, buf: &mut crate::TextBuffer, options: &SaveOptions) -> Result<Vec<u8>> {
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

        {
            let mut result = vec![constants::ICED_VERSION as u8, (constants::ICED_VERSION >> 8) as u8];
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

        if let Some(sauce) = &options.save_sauce {
            let mut sauce_vec: Vec<u8> = Vec::new();
            sauce.write(&mut sauce_vec)?;
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
            write_utf8_encoded_string(&mut font_data, &v.name());
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
                        let ch = layer.get_char((x, y).into());
                        let mut attr = ch.attribute.attr;

                        let is_short = if ch.is_visible() && ch.ch as u32 <= 255 && ch.attribute.foreground_color <= 255 && ch.attribute.background_color <= 255
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
                            let ch = layer.get_char((x, y).into());
                            let mut attr = ch.attribute.attr;

                            let is_short =
                                if ch.is_visible() && ch.ch as u32 <= 255 && ch.attribute.foreground_color <= 255 && ch.attribute.background_color <= 255 {
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

                let is_short = if tag.attribute.foreground_color <= 255 && tag.attribute.background_color <= 255 {
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
                &crate::Rectangle {
                    start: Position::new(0, first_line),
                    size: Size::new(buf.get_width(), last_line - first_line),
                }
                .into(),
                false,
            );
            writer.write_image_data(&data).unwrap();
        }
        writer.finish().unwrap();

        Ok(result)
    }

    fn load_buffer(&self, file_name: &Path, data: &[u8], _load_data_opt: Option<LoadData>) -> Result<crate::TextBuffer> {
        let mut result = TextBuffer::new((80, 25));
        result.terminal_state.is_terminal_buffer = false;
        result.file_name = Some(file_name.into());
        result.layers.clear();

        let mut decoder = png::StreamingDecoder::new();
        let mut len = 0;
        let mut last_info = 0;
        let mut is_running = true;
        while is_running {
            match decoder.update(&data[len..], None) {
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
                                        return Err(crate::EngineError::UnsupportedFormat { description: format!("unsupported header size {}", bytes.len()) });
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
                                    if let Some(sauce) = SauceRecord::from_bytes(&bytes)? {
                                        super::apply_sauce_to_buffer(&mut result, &sauce);
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
                                                return Err(crate::EngineError::UnsupportedFormat { description: "unsupported alignment".to_string() });
                                            }
                                        };
                                        bytes = &bytes[1..];
                                        let tag_placement = match bytes[0] {
                                            0 => crate::TagPlacement::InText,
                                            1 => crate::TagPlacement::WithGotoXY,
                                            _ => {
                                                return Err(crate::EngineError::UnsupportedFormat { description: "unsupported tag placement".to_string() });
                                            }
                                        };
                                        bytes = &bytes[1..];

                                        let tag_role = match bytes[0] {
                                            0 => crate::TagRole::Displaycode,
                                            1 => crate::TagRole::Hyperlink,
                                            _ => {
                                                return Err(crate::EngineError::UnsupportedFormat { description: "unsupported tag role".to_string() });
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
                                                font_page: font_page as u8,
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
                                                                    font_page: font_page as u8,
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
                                            return Err(crate::EngineError::OutOfBounds { offset: o + length });
                                        }
                                        for y in 0..height {
                                            if o >= bytes.len() {
                                                // will be continued in a later chunk.
                                                break;
                                            }
                                            for x in 0..width {
                                                if o + 2 > bytes.len() {
                                                    return Err(crate::EngineError::OutOfBounds { offset: o + 2 });
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
                                                        return Err(crate::EngineError::OutOfBounds { offset: o + 3 });
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
                                                        return Err(crate::EngineError::OutOfBounds { offset: o + 14 });
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
                                                            font_page: font_page as u8,
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
    while length > 0 && !layer.get_char((length - 1, y).into()).is_visible() {
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

impl TextBuffer {
    pub fn is_line_empty(&self, line: i32) -> bool {
        for i in 0..self.get_width() {
            if !self.get_char((i, line).into()).is_transparent() {
                return false;
            }
        }
        true
    }
}

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
