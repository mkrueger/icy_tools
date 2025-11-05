use crate::VERSION;
use icy_engine::editor::EditState;
use icy_engine::{AttributedChar, Buffer, Position, TextAttribute, TextPane};
use std::path::Path;

const MAIN_SCREEN_ANSI: &[u8] = include_bytes!("../../data/welcome_screen.1.icy");

pub fn create_weclome_screen() -> EditState {
    // Create a default EditState
    let mut edit_state = EditState::default();

    // Load the welcome screen from MAIN_SCREEN_ANSI
    let mut buffer = Buffer::from_bytes(&Path::new("a.icy"), true, MAIN_SCREEN_ANSI, None, None).unwrap();
    buffer.buffer_type = icy_engine::BufferType::CP437;
    buffer.is_terminal_buffer = true;
    buffer.terminal_state.fixed_size = true;

    // Find and replace special characters
    let mut ready_position = None;

    // Scan through the buffer to find and replace special characters
    for y in 0..buffer.get_height() {
        for x in 0..buffer.get_width() {
            let ch = buffer.get_char((x, y));

            if ch.ch == '@' {
                // Build version string with colors
                let mut version_chars = Vec::new();

                // 'v' in white (color 7)
                version_chars.push(AttributedChar::new('v', TextAttribute::from_u8(0x07, icy_engine::IceMode::Ice)));

                // Major version in yellow (color 14)
                let major_str = VERSION.major.to_string();
                for ch in major_str.chars() {
                    version_chars.push(AttributedChar::new(ch, TextAttribute::from_u8(0x0E, icy_engine::IceMode::Ice)));
                }

                // First dot in green (color 10)
                version_chars.push(AttributedChar::new('.', TextAttribute::from_u8(0x0A, icy_engine::IceMode::Ice)));

                // Minor version in light red (color 12)
                let minor_str = VERSION.minor.to_string();
                for ch in minor_str.chars() {
                    version_chars.push(AttributedChar::new(ch, TextAttribute::from_u8(0x0C, icy_engine::IceMode::Ice)));
                }

                // Second dot in green (color 10)
                version_chars.push(AttributedChar::new('.', TextAttribute::from_u8(0x0A, icy_engine::IceMode::Ice)));

                // Patch/build version in magenta (color 13)
                let patch_str = VERSION.patch.to_string();
                for ch in patch_str.chars() {
                    version_chars.push(AttributedChar::new(ch, TextAttribute::from_u8(0x0D, icy_engine::IceMode::Ice)));
                }

                // Place the colored version at the @ position
                for (i, new_ch) in version_chars.into_iter().enumerate() {
                    let new_x = x + i as i32;
                    if new_x < buffer.get_width() {
                        buffer.layers[0].set_char(Position::new(new_x, y), new_ch);
                    }
                }
            } else if ch.ch == '#' {
                // Mark position for ready message
                ready_position = Some((x, y));
            }
        }
    }

    // Write "IcyTerm ready." message at the marked position
    let mut caret_pos = Position::default();
    if let Some((x, y)) = ready_position {
        let ready_msg = "READY.";

        for (i, msg_char) in ready_msg.chars().enumerate() {
            let new_x = x + i as i32;
            if new_x < buffer.get_width() {
                let new_ch = AttributedChar::new(msg_char, TextAttribute::default());
                buffer.layers[0].set_char(Position::new(new_x, y), new_ch);
            }
        }

        // Set cursor position after the ready message
        caret_pos = Position::new(0, y + 1);
    }

    buffer.update_hyperlinks();
    edit_state.set_buffer(buffer);

    edit_state.get_caret_mut().set_position(caret_pos);
    edit_state
}
