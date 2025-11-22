use icy_engine::ScreenMode;
use icy_net::telnet::TerminalEmulation;
use std::fs::{self};

#[test]
pub fn test_petscii() {
    for entry in fs::read_dir("tests/output/petscii/files").expect("Error reading test_data directory.") {
        let cur_entry = entry.unwrap().path();
        if cur_entry.extension().unwrap() != "seq" {
            continue;
        }

        let data: Vec<u8> = fs::read(&cur_entry).unwrap_or_else(|e| panic!("Error reading file {:?}: {}", cur_entry, e));
        let data = icy_sauce::strip_sauce(&data, icy_sauce::StripMode::All);

        let mut screen = ScreenMode::Vic.create_screen(TerminalEmulation::PETscii, None);
        super::run_parser_compare(&mut screen, &cur_entry, &data);
    }
}
