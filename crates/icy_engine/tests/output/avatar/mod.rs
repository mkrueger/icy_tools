use icy_engine::{EditableScreen, TextScreen};
use icy_parser_core::AvatarParser;
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

        super::parse_with_parser(&mut screen, &mut AvatarParser::default(), &data).expect("Error parsing file");

        // Pass filenames for loading expected PNG and saving output
        compare_output(&screen, &cur_entry);
    }
}
