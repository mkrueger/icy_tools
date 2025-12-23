//! Binary format (.bin) I/O implementation.

use crate::{AttributedChar, Position, Result, TextAttribute, TextBuffer, TextPane, TextScreen};

use super::super::{LoadData, SauceBuilder, SaveOptions, apply_sauce_to_buffer};

/// Load a binary file into a TextScreen.
pub(crate) fn load_bin(data: &[u8], load_data_opt: Option<&LoadData>, sauce_opt: Option<&icy_sauce::SauceRecord>) -> Result<TextScreen> {
    let mut screen = TextScreen::new((160, 25));
    screen.buffer.terminal_state.is_terminal_buffer = false;

    // Apply SAUCE settings early to get correct dimensions
    if let Some(sauce) = sauce_opt {
        apply_sauce_to_buffer(&mut screen.buffer, sauce);
    }

    let max_height = load_data_opt.and_then(|ld| ld.max_height());

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
                attribute.set_foreground(attribute.foreground() + 8);
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
    if let Some(meta) = &options.sauce {
        let sauce = buf.build_binary_sauce(meta);
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
