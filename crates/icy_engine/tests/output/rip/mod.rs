use icy_engine::{BufferParser as _, PaletteScreenBuffer, ScreenSink};
use icy_parser_core::{CommandParser, RipParser};
use std::{
    env::current_dir,
    fs::{self},
};

use crate::compare_output;

#[test]
pub fn test_rip() {
    for entry in fs::read_dir("tests/output/rip/files").expect("Error reading test_data directory.") {
        let cur_entry = entry.unwrap().path();
        if cur_entry.extension().unwrap() != "rip" {
            continue;
        }

        let data = fs::read(&cur_entry).unwrap_or_else(|e| panic!("Error reading file {:?}: {}", cur_entry, e));
        let data = icy_sauce::strip_sauce(&data, icy_sauce::StripMode::All);

        let mut buffer = PaletteScreenBuffer::new(icy_engine::GraphicsType::Rip);

        let mut parser = RipParser::new();
        let mut sink = ScreenSink::new(&mut buffer);

        parser.parse(&data, &mut sink);

        // Pass filenames for loading expected PNG and saving output
        compare_output(&buffer, &cur_entry);
    }
}
