use icy_engine::{
    BufferParser, EditableScreen, PaletteScreenBuffer, ansi,
    rip::{self, RIP_SCREEN_SIZE},
};
use std::fs::{self};

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
        let data = String::from_utf8_lossy(&data);

        let mut buffer: PaletteScreenBuffer = PaletteScreenBuffer::new(RIP_SCREEN_SIZE.width, RIP_SCREEN_SIZE.height, rip::bgi::DEFAULT_BITFONT.clone());
        rip::setup_rip_text_fonts(&mut buffer);

        let mut ansi_parser = ansi::Parser::default();
        ansi_parser.bs_is_ctrl_char = true;
        let mut parser = rip::Parser::new(Box::new(ansi_parser), ".".into(), RIP_SCREEN_SIZE);
        for c in data.chars() {
            if c == '\x1A' {
                break;
            }
            parser.print_char(&mut buffer, c).unwrap();
        }

        // Pass filenames for loading expected PNG and saving output
        compare_output(&buffer, &cur_entry);
    }
}
