use icy_engine::{
    BitFont, BufferParser, EditableScreen, Palette, TextScreen, VIEWDATA, VIEWDATA_PALETTE,
    viewdata::{self, VIEWDATA_SCREEN_SIZE},
};
use std::fs::{self};

use crate::compare_output;

#[test]
pub fn test_viewdata() {
    for entry in fs::read_dir("tests/output/view_data/files").expect("Error reading test_data directory.") {
        let cur_entry = entry.unwrap().path();
        if cur_entry.extension().unwrap() != "vd" {
            continue;
        }

        let data = fs::read(&cur_entry).unwrap_or_else(|e| panic!("Error reading file {:?}: {}", cur_entry, e));
        let data = icy_sauce::strip_sauce(&data, icy_sauce::StripMode::All);
        let data = String::from_utf8_lossy(&data);

        let mut buffer = TextScreen::new(*VIEWDATA_SCREEN_SIZE);
        buffer.terminal_state_mut().is_terminal_buffer = true;
        buffer.clear_font_table();
        buffer.set_font(0, BitFont::from_bytes("", VIEWDATA).unwrap());
        *buffer.palette_mut() = Palette::from_slice(&VIEWDATA_PALETTE);
        *buffer.buffer_type_mut() = icy_engine::BufferType::Viewdata;

        let mut parser = viewdata::Parser::default();
        for c in data.chars() {
            parser.print_char(&mut buffer, c).unwrap();
        }

        // Pass filenames for loading expected PNG and saving output
        compare_output(&buffer, &cur_entry);
    }
}
