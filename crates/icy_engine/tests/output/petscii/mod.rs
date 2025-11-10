use icy_engine::{
    BitFont, BufferParser, C64_DEFAULT_PALETTE, C64_SHIFTED, C64_UNSHIFTED, EditableScreen, Palette, TextScreen,
    petscii::{self, C64_SCREEN_SIZE},
};
use std::fs::{self};

use crate::compare_output;

#[test]
pub fn test_petscii() {
    for entry in fs::read_dir("tests/output/petscii/files").expect("Error reading test_data directory.") {
        let cur_entry = entry.unwrap().path();
        if cur_entry.extension().unwrap() != "seq" {
            continue;
        }

        let data = fs::read(&cur_entry).unwrap_or_else(|e| panic!("Error reading file {:?}: {}", cur_entry, e));
        let data = icy_sauce::strip_sauce(&data, icy_sauce::StripMode::All);

        let mut screen = TextScreen::new(*C64_SCREEN_SIZE);
        screen.terminal_state_mut().is_terminal_buffer = true;
        screen.clear_font_table();
        screen.set_font(0, BitFont::from_bytes("", C64_UNSHIFTED).unwrap());
        screen.set_font(1, BitFont::from_bytes("", C64_SHIFTED).unwrap());
        *screen.palette_mut() = Palette::from_slice(&C64_DEFAULT_PALETTE);
        *screen.buffer_type_mut() = icy_engine::BufferType::Petscii;

        let mut parser = petscii::Parser::default();
        for c in data {
            if let Err(err) = parser.print_char(&mut screen, *c as char) {
                eprintln!("Error parsing char '{}' ({:02X}): {}", c, c, err);
            }
        }
        // Pass filenames for loading expected PNG and saving output
        compare_output(&screen, &cur_entry);
    }
}
