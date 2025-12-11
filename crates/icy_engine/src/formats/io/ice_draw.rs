use std::path::Path;

use super::super::{LoadData, SaveOptions};
use crate::{
    AttributedChar, BitFont, IceMode, LoadingError, Palette, Position, Result, SavingError, Size, TextAttribute, TextBuffer, TextPane, TextScreen,
    analyze_font_usage, guess_font_name,
};

// http://fileformats.archiveteam.org/wiki/ICEDraw

const HEADER_SIZE: usize = 4 + 4 * 2;

const IDF_V1_3_HEADER: &[u8] = b"\x041.3";
// TODO: Find source for 1.4 - 1.3 is the only one I could find now, has been too long since I wrote this code.
//       and I cant see differences from 1.3 in my implementation.
const IDF_V1_4_HEADER: &[u8] = b"\x041.4";

const FONT_SIZE: usize = 4096;
const PALETTE_SIZE: usize = 3 * 16;

pub(crate) fn save_ice_draw(buf: &TextBuffer, options: &SaveOptions) -> Result<Vec<u8>> {
    if buf.ice_mode != IceMode::Ice {
        return Err(crate::EngineError::OnlyIceModeSupported);
    }

    if buf.height() > 200 {
        return Err(crate::EngineError::TooManyLines { max_lines: 200 });
    }
    let fonts = analyze_font_usage(buf);
    if fonts.len() > 1 {
        return Err(crate::EngineError::OnlySingleFontSupported);
    }
    if buf.palette.len() != 16 {
        return Err(crate::EngineError::Only16ColorPalettesSupported);
    }

    let mut result = IDF_V1_4_HEADER.to_vec();
    // x1
    result.push(0);
    result.push(0);

    // y1
    result.push(0);
    result.push(0);

    let w = buf.width().saturating_sub(1);
    result.push(w as u8);
    result.push((w >> 8) as u8);

    let h = buf.height().saturating_sub(1);
    result.push(h as u8);
    result.push((h >> 8) as u8);

    for y in 0..buf.height() {
        let mut x = 0;
        while x < buf.width() {
            let ch = buf.char_at((x, y).into());
            let mut rle_count = 1;
            if options.compress {
                while x + rle_count < buf.width() && rle_count < (u16::MAX) as i32 {
                    if ch != buf.char_at((x + rle_count, y).into()) {
                        break;
                    }
                    rle_count += 1;
                }
                if rle_count > 3 || ch.ch == '\x01' {
                    result.push(1);
                    result.push(0);

                    result.push(rle_count as u8);
                    result.push((rle_count >> 8) as u8);
                } else {
                    rle_count = 1;
                }
            }
            let attr = ch.attribute.as_u8(buf.ice_mode);
            let ch = ch.ch as u32;
            if ch > 255 {
                return Err(SavingError::Only8BitCharactersSupported.into());
            }

            // fake repeat
            if ch == 1 && attr == 0 && rle_count == 1 {
                result.extend([1, 0, 1, 0]);
            }
            result.push(ch as u8);
            result.push(attr);

            x += rle_count;
        }
    }

    // font
    if buf.font_dimensions() != Size::new(8, 16) {
        return Err(SavingError::Only8x16FontsSupported.into());
    }
    if let Some(font) = buf.font(fonts[0]) {
        result.extend(font.convert_to_u8_data());
    } else {
        return Err(SavingError::NoFontFound.into());
    }

    // palette
    result.extend(buf.palette.as_vec_63());
    if let Some(sauce) = &options.save_sauce {
        sauce.write(&mut result)?;
    }
    Ok(result)
}

pub(crate) fn load_ice_draw(file_name: &Path, data: &[u8], load_data_opt: Option<LoadData>) -> Result<TextScreen> {
    let mut result = TextBuffer::new((80, 25));
    result.ice_mode = IceMode::Ice;
    result.terminal_state.is_terminal_buffer = false;
    result.file_name = Some(file_name.into());
    let load_data = load_data_opt.unwrap_or_default();
    let max_height = load_data.max_height();

    if data.len() < HEADER_SIZE + FONT_SIZE + PALETTE_SIZE {
        return Err(LoadingError::FileTooShort.into());
    }
    let version = &data[0..4];

    if version != IDF_V1_3_HEADER && version != IDF_V1_4_HEADER {
        return Err(LoadingError::IDMismatch.into());
    }

    let mut o = 4;
    let x1 = (data[o] as u16 + ((data[o + 1] as u16) << 8)) as i32;
    o += 2;
    let y1 = (data[o] as u16 + ((data[o + 1] as u16) << 8)) as i32;
    o += 2;
    let x2 = (data[o] as u16 + ((data[o + 1] as u16) << 8)) as i32;
    o += 2;
    // skip y2
    o += 2;

    if x2 < x1 {
        return Err(crate::EngineError::InvalidBounds {
            message: "IDF width needs to be >= 0".to_string(),
        });
    }

    result.set_width(x2 - x1 + 1);
    let data_size = data.len() - FONT_SIZE - PALETTE_SIZE;
    let mut pos = Position::new(x1, y1);

    while o + 1 < data_size {
        // Check height limit
        if let Some(max_h) = max_height {
            if pos.y >= max_h {
                break;
            }
        }

        let mut rle_count = 1;
        let mut char_code = data[o];
        o += 1;
        let mut attr = data[o];
        o += 1;

        if char_code == 1 && attr == 0 {
            rle_count = data[o] as i32 + ((data[o + 1] as i32) << 8);

            if o + 3 >= data_size {
                break;
            }
            o += 2;
            char_code = data[o];
            o += 1;
            attr = data[o];
            o += 1;
        }
        while rle_count > 0 {
            // Check height limit inside RLE loop
            if let Some(max_h) = max_height {
                if pos.y >= max_h {
                    break;
                }
            }
            result.layers[0].set_height(pos.y + 1);
            result.set_height(pos.y + 1);
            let attribute = TextAttribute::from_u8(attr, result.ice_mode);
            result.layers[0].set_char(pos, AttributedChar::new(char_code as char, attribute));
            advance_pos(x1, x2, &mut pos);
            rle_count -= 1;
        }
    }
    let mut font = BitFont::from_basic(8, 16, &data[o..(o + FONT_SIZE)]);
    font.yaff_font.name = Some(guess_font_name(&font));
    result.set_font(0, font);
    o += FONT_SIZE;

    result.palette = Palette::from_63(&data[o..(o + PALETTE_SIZE)]);
    Ok(TextScreen::from_buffer(result))
}

pub fn _get_save_sauce_default_idf(buf: &TextBuffer) -> (bool, String) {
    if buf.width() != 80 {
        return (true, "width != 80".to_string());
    }

    (false, String::new())
}

fn advance_pos(x1: i32, x2: i32, pos: &mut Position) -> bool {
    pos.x += 1;
    if pos.x > x2 {
        pos.x = x1;
        pos.y += 1;
    }
    true
}
