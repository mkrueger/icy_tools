use std::{collections::HashSet, io, path::Path};

use super::super::{LoadData, SaveOptions};
use crate::{
    AttributedChar, BufferType, EditableScreen, IceMode, LoadingError, PaletteMode, Position, Result, SavingError, TextAttribute, TextBuffer, TextPane,
    TextScreen, analyze_font_usage,
};

// http://fileformats.archiveteam.org/wiki/TUNDRA
// ANSI code for 24 bit: ESC[(0|1);R;G;Bt
// 0 for background
// 1 for foreground

const TUNDRA_VER: u8 = 24;
const TUNDRA_HEADER: &[u8] = b"TUNDRA24";

const TUNDRA_POSITION: u8 = 1;
const TUNDRA_COLOR_FOREGROUND: u8 = 2;
const TUNDRA_COLOR_BACKGROUND: u8 = 4;

pub(crate) fn save_tundra(buf: &TextBuffer, options: &SaveOptions) -> Result<Vec<u8>> {
    let mut result = vec![TUNDRA_VER]; // version
    result.extend(TUNDRA_HEADER);
    let mut attr = TextAttribute::from_u8(0, buf.ice_mode);
    let mut skip_pos = None;
    let mut colors = HashSet::new();

    let fonts = analyze_font_usage(buf);
    if fonts.len() > 1 {
        return Err(crate::EngineError::OnlySingleFontSupported);
    }

    for y in 0..buf.height() {
        for x in 0..buf.width() {
            let pos = Position::new(x, y);
            let ch = buf.char_at(pos);
            let cur_attr = ch.attribute;
            if !ch.is_visible() {
                if skip_pos.is_none() {
                    skip_pos = Some(pos);
                }
                continue;
            }
            /*
            if ch.is_transparent() && attr.background() == 0 {
                if skip_pos.is_none() {
                    skip_pos = Some(pos);
                }
                continue;
            }

            if let Some(pos2) = skip_pos {
                let skip_len =
                    (pos.x + pos.y * buf.width()) - (pos2.x + pos2.y * buf.width());
                if skip_len <= TND_GOTO_BLOCK_LEN {
                    result.resize(result.len() + skip_len as usize, 0);
                } else {
                    result.push(TUNDRA_POSITION);
                    result.extend(i32::to_be_bytes(pos.y));
                    result.extend(i32::to_be_bytes(pos.x));
                }
                skip_pos = None;
            }*/
            let ch = ch.ch as u32;
            if ch > 255 {
                return Err(SavingError::Only8BitCharactersSupported.into());
            }

            if (1..=6).contains(&ch) {
                // fake color change to represent control characters
                result.push(TUNDRA_COLOR_FOREGROUND);
                result.push(ch as u8);

                let rgb = buf.palette.rgb(attr.foreground());
                result.push(0);
                result.push(rgb.0);
                result.push(rgb.1);
                result.push(rgb.2);
                continue;
            }

            let mut cmd = 0;
            let write_foreground =
                buf.palette.color(attr.foreground()).rgb() != buf.palette.color(cur_attr.foreground()).rgb() || attr.is_bold() != cur_attr.is_bold();
            if write_foreground {
                cmd |= TUNDRA_COLOR_FOREGROUND;
            }
            let write_background = buf.palette.color(attr.background()).rgb() != buf.palette.color(cur_attr.background()).rgb();
            if write_background {
                cmd |= TUNDRA_COLOR_BACKGROUND;
            }

            if cmd != 0 {
                result.push(cmd);
                result.push(ch as u8);
                if write_foreground {
                    let mut fg = cur_attr.foreground();
                    if cur_attr.is_bold() {
                        fg += 8;
                    }
                    colors.insert(fg);
                    let rgb = buf.palette.rgb(fg);
                    result.push(0);
                    result.push(rgb.0);
                    result.push(rgb.1);
                    result.push(rgb.2);
                }
                if write_background {
                    colors.insert(cur_attr.background());

                    let rgb = buf.palette.rgb(cur_attr.background());
                    result.push(0);
                    result.push(rgb.0);
                    result.push(rgb.1);
                    result.push(rgb.2);
                }
                attr = cur_attr;
                continue;
            }
            result.push(ch as u8);
        }
    }
    if let Some(pos2) = skip_pos {
        let pos = Position::new(buf.width().saturating_sub(1), buf.height().saturating_sub(1));

        let skip_len = (pos.x + pos.y * buf.width()) - (pos2.x + pos2.y * buf.width()) + 1;
        result.resize(result.len() + skip_len as usize, 0);
    }

    if let Some(sauce) = &options.save_sauce {
        sauce.write(&mut result)?;
    }
    Ok(result)
}

pub(crate) fn load_tundra(file_name: &Path, data: &[u8], load_data_opt: Option<LoadData>) -> Result<TextScreen> {
    let mut screen = TextScreen::new((80, 25));
    screen.buffer.terminal_state.is_terminal_buffer = false;
    screen.buffer.file_name = Some(file_name.into());
    let load_data = load_data_opt.unwrap_or_default();
    let max_height = load_data.max_height();
    if let Some(sauce) = &load_data.sauce_opt {
        screen.apply_sauce(sauce);
    }
    if data.len() < 1 + TUNDRA_HEADER.len() {
        return Err(LoadingError::FileTooShort.into());
    }
    let mut o = 1;

    let header = &data[1..=TUNDRA_HEADER.len()];

    if header != TUNDRA_HEADER {
        return Err(LoadingError::IDMismatch.into());
    }
    o += TUNDRA_HEADER.len();

    screen.buffer.palette.clear();
    screen.buffer.palette.insert_color_rgb(0, 0, 0);
    screen.buffer.buffer_type = BufferType::CP437;
    screen.buffer.palette_mode = PaletteMode::RGB;
    screen.buffer.ice_mode = IceMode::Ice;

    let mut pos = Position::default();
    let mut attr = TextAttribute::default();

    while o < data.len() {
        // Check height limit
        if let Some(max_h) = max_height {
            if pos.y >= max_h {
                break;
            }
        }

        let mut cmd = data[o];
        o += 1;
        if cmd == TUNDRA_POSITION {
            pos.y = to_u32(&data[o..]);
            // Check if jump position exceeds height limit
            if let Some(max_h) = max_height {
                if pos.y >= max_h {
                    break;
                }
            }
            if pos.y >= (u16::MAX) as i32 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "Invalid Tundra Draw file.\nJump y position {} out of bounds (height is {})",
                        pos.y,
                        screen.buffer.height()
                    ),
                )
                .into());
            }
            o += 4;
            pos.x = to_u32(&data[o..]);
            if pos.x >= screen.buffer.width() {
                return Err(crate::EngineError::InvalidBounds {
                    message: format!(
                        "Invalid Tundra Draw file. Jump x position {} out of bounds (width is {})",
                        pos.x,
                        screen.buffer.width()
                    ),
                });
            }
            o += 4;
            continue;
        }

        if cmd > 1 && cmd <= 6 {
            let ch = data[o];
            o += 1;
            if cmd & TUNDRA_COLOR_FOREGROUND != 0 {
                o += 1;
                let r = data[o];
                o += 1;
                let g = data[o];
                o += 1;
                let b = data[o];
                o += 1;
                attr.set_foreground(screen.buffer.palette.insert_color_rgb(r, g, b));
            }
            if cmd & TUNDRA_COLOR_BACKGROUND != 0 {
                o += 1;
                let r = data[o];
                o += 1;
                let g = data[o];
                o += 1;
                let b = data[o];
                o += 1;
                attr.set_background(screen.buffer.palette.insert_color_rgb(r, g, b));
            }
            cmd = ch;
        }
        screen.buffer.set_height(pos.y + 1);
        screen.buffer.layers[0].set_height(pos.y + 1);
        screen.buffer.layers[0].set_char(pos, AttributedChar::new(cmd as char, attr));
        advance_pos(&screen.buffer, &mut pos);
    }
    screen.buffer.set_size(screen.buffer.layers[0].size());

    Ok(screen)
}

fn advance_pos(result: &TextBuffer, pos: &mut Position) -> bool {
    pos.x += 1;
    if pos.x >= result.width() {
        pos.x = 0;
        pos.y += 1;
    }
    true
}

fn to_u32(bytes: &[u8]) -> i32 {
    bytes[3] as i32 | (bytes[2] as i32) << 8 | (bytes[1] as i32) << 16 | (bytes[0] as i32) << 24
}

// const TND_GOTO_BLOCK_LEN: i32 = 1 + 2 * 4;

pub fn _get_save_sauce_default_tnd(buf: &TextBuffer) -> (bool, String) {
    if buf.width() != 80 {
        return (true, "width != 80".to_string());
    }

    (false, String::new())
}
