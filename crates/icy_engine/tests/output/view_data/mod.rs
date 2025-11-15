use icy_engine::{BitFont, EditableScreen, Palette, TextScreen, VIEWDATA, VIEWDATA_PALETTE};
use icy_parser_core::ViewdataParser;
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

        let mut screen = TextScreen::new((40, 24)); // Viewdata standard screen size
        screen.terminal_state_mut().is_terminal_buffer = true;
        screen.clear_font_table();
        screen.set_font(0, BitFont::from_bytes("", VIEWDATA).unwrap());
        *screen.palette_mut() = Palette::from_slice(&VIEWDATA_PALETTE);
        *screen.buffer_type_mut() = icy_engine::BufferType::Viewdata;

        super::parse_with_parser(&mut screen, &mut ViewdataParser::default(), &data).expect("Error parsing file");

        // Pass filenames for loading expected PNG and saving output
        compare_output(&screen, &cur_entry);
    }
}
