use icy_engine::ScreenMode;
use icy_net::telnet::TerminalEmulation;
use std::fs::{self};

#[test]
pub fn test_skypix() {
    crate::init_logging();
    for entry in fs::read_dir("tests/output/skypix/files").expect("Error reading test_data directory.") {
        let cur_entry = entry.unwrap().path();
        if !cur_entry.is_file() || cur_entry.extension().and_then(|e| e.to_str()) != Some("ans") {
            continue;
        }
        let data = fs::read(&cur_entry).unwrap_or_else(|e| panic!("Error reading file {:?}: {}", cur_entry, e));

        let mut screen = ScreenMode::SkyPix.create_screen(TerminalEmulation::Skypix, None);
        super::run_parser_compare(&mut screen, &cur_entry, &data);
    }
}
