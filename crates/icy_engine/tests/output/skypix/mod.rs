use icy_engine::{PaletteScreenBuffer, ScreenSink, SkypixParser};
use icy_parser_core::CommandParser;
use std::fs::{self};

use crate::compare_output;

#[test]
pub fn test_skypix() {
    crate::init_logging();
    for entry in fs::read_dir("tests/output/skypix/files").expect("Error reading test_data directory.") {
        let cur_entry = entry.unwrap().path();
        if !cur_entry.is_file() || cur_entry.extension().and_then(|e| e.to_str()) != Some("ans") {
            continue;
        }
        let data = fs::read(&cur_entry).unwrap_or_else(|e| panic!("Error reading file {:?}: {}", cur_entry, e));

        let mut buffer = PaletteScreenBuffer::new(icy_engine::GraphicsType::Skypix);

        let mut parser = SkypixParser::new();
        let mut sink = ScreenSink::new(&mut buffer);

        parser.parse(&data, &mut sink);

        // Pass filenames for loading expected PNG and saving output
        compare_output(&buffer, &cur_entry);
    }
}
