use icy_engine::{EditableScreen, TextScreen};
use icy_parser_core::AnsiParser;
use std::{
    fs::{self},
    thread,
    time::Duration,
};

use crate::compare_output;

#[test]
pub fn test_ansi() {
    for entry in fs::read_dir("tests/output/ansi/files").expect("Error reading test_data directory.") {
        let cur_entry = entry.unwrap().path();
        if cur_entry.extension().unwrap() != "ans" {
            continue;
        }

        let data = fs::read(&cur_entry).unwrap_or_else(|e| panic!("Error reading file {:?}: {}", cur_entry, e));
        let data = icy_sauce::strip_sauce(&data, icy_sauce::StripMode::All);

        let mut screen = TextScreen::new((80, 25));
        screen.terminal_state_mut().is_terminal_buffer = true;
        *screen.buffer_type_mut() = icy_engine::BufferType::CP437;

        super::parse_with_parser(&mut screen, &mut AnsiParser::default(), &data).expect("Error parsing file");
        while !screen.buffer.sixel_threads.is_empty() {
            thread::sleep(Duration::from_millis(50));
            let _ = screen.buffer.update_sixel_threads();
        }

        // Pass filenames for loading expected PNG and saving output
        compare_output(&screen, &cur_entry);
    }
}
