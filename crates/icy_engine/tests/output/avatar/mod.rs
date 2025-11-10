use icy_engine::{BufferParser, EditableScreen, TextScreen, avatar};
use std::fs::{self};

use crate::compare_output;

#[test]
pub fn test_avatar() {
    for entry in fs::read_dir("tests/output/avatar/files").expect("Error reading test_data directory.") {
        let cur_entry = entry.unwrap().path();
        if cur_entry.extension().unwrap() != "avt" {
            continue;
        }

        let data = fs::read(&cur_entry).unwrap_or_else(|e| panic!("Error reading file {:?}: {}", cur_entry, e));
        let data = icy_sauce::strip_sauce(&data, icy_sauce::StripMode::All);

        let mut screen = TextScreen::new((80, 25));
        screen.terminal_state_mut().is_terminal_buffer = true;
        *screen.buffer_type_mut() = icy_engine::BufferType::CP437;

        let mut parser = avatar::Parser::default();
        for c in data {
            if let Err(err) = parser.print_char(&mut screen, *c as char) {
                eprintln!("Error parsing char '{}' ({:02X}): {}", c, c, err);
            }
        }
        // Pass filenames for loading expected PNG and saving output
        compare_output(&screen, &cur_entry);
    }
}
