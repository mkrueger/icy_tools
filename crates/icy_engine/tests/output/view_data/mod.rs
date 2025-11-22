use icy_engine::ScreenMode;
use icy_net::telnet::TerminalEmulation;
use std::fs::{self};

#[test]
pub fn test_viewdata() {
    for entry in fs::read_dir("tests/output/view_data/files").expect("Error reading test_data directory.") {
        let cur_entry = entry.unwrap().path();
        if cur_entry.extension().unwrap() != "vd" {
            continue;
        }

        let data = fs::read(&cur_entry).unwrap_or_else(|e| panic!("Error reading file {:?}: {}", cur_entry, e));
        let data = icy_sauce::strip_sauce(&data, icy_sauce::StripMode::All);

        let mut screen = ScreenMode::Videotex.create_screen(TerminalEmulation::ViewData, None);
        super::run_parser_compare(&mut screen, &cur_entry, &data);
    }
}
