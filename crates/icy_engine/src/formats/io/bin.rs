//! Binary format (.bin) I/O implementation.

use std::path::Path;

use crate::{AttributedChar, EditableScreen, Position, Result, TextAttribute, TextBuffer, TextPane, TextScreen};

use super::super::{LoadData, SaveOptions};

/// Load a binary file into a TextScreen.
pub(crate) fn load_bin(file_name: &Path, data: &[u8], load_data_opt: Option<LoadData>) -> Result<TextScreen> {
    let mut screen = TextScreen::new((160, 25));
    screen.buffer.terminal_state.is_terminal_buffer = false;
    screen.buffer.file_name = Some(file_name.into());
    let load_data = load_data_opt.unwrap_or_default();
    let max_height = load_data.max_height();
    if let Some(sauce) = &load_data.sauce_opt {
        screen.apply_sauce(sauce);
    }
    let mut o = 0;
    let mut pos = Position::default();
    loop {
        // Check height limit before processing a new row
        if let Some(max_h) = max_height {
            if pos.y >= max_h {
                screen.buffer.set_height(pos.y);
                return Ok(screen);
            }
        }

        for _ in 0..screen.buffer.width() {
            if o >= data.len() {
                screen.buffer.set_height(screen.buffer.layers[0].height());
                return Ok(screen);
            }

            if o + 1 >= data.len() {
                // last byte is not important enough to throw an error
                // there seem to be some invalid files out there.
                log::error!("Invalid Bin. Read char block beyond EOF.");
                screen.buffer.set_height(screen.buffer.layers[0].height());
                return Ok(screen);
            }

            screen.buffer.layers[0].set_height(pos.y + 1);
            let mut attribute = TextAttribute::from_u8(data[o + 1], screen.buffer.ice_mode);
            if attribute.is_bold() {
                attribute.set_foreground(attribute.foreground_color + 8);
                attribute.set_is_bold(false);
            }

            screen.buffer.layers[0].set_char(pos, AttributedChar::new(data[o] as char, attribute));
            pos.x += 1;
            o += 2;
        }
        pos.x = 0;
        pos.y += 1;
    }
}

/// Save a TextBuffer to binary format.
pub(crate) fn save_bin(buf: &TextBuffer, options: &SaveOptions) -> Result<Vec<u8>> {
    let mut result = Vec::new();

    for y in 0..buf.height() {
        for x in 0..buf.width() {
            let ch = buf.char_at((x, y).into());
            result.push(ch.ch as u8);
            result.push(ch.attribute.as_u8(buf.ice_mode));
        }
    }
    if let Some(sauce) = &options.save_sauce {
        sauce.write(&mut result)?;
    }
    Ok(result)
}

/// Check if SAUCE is required for saving (width != 160).
pub fn _get_save_sauce_default_binary(buf: &TextBuffer) -> (bool, String) {
    if buf.width() != 160 {
        return (true, "width != 160".to_string());
    }
    (false, String::new())
}
