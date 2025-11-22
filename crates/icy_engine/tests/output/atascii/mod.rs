use icy_engine::ScreenMode;
use icy_net::telnet::TerminalEmulation;
use std::fs::{self};

#[test]
pub fn test_atascii_40() {
    for entry in fs::read_dir("tests/output/atascii/40col").expect("Error reading test_data directory.") {
        let cur_entry = entry.unwrap().path();
        if cur_entry.extension().unwrap() != "ata" {
            continue;
        }

        let data = fs::read(&cur_entry).unwrap_or_else(|e| panic!("Error reading file {:?}: {}", cur_entry, e));
        let data = icy_sauce::strip_sauce(&data, icy_sauce::StripMode::All);

        let mut screen = ScreenMode::Atascii(40).create_screen(TerminalEmulation::ATAscii, None);
        super::run_parser_compare(&mut screen, &cur_entry, &data);
    }
}

#[test]
pub fn test_atascii_80() {
    for entry in fs::read_dir("tests/output/atascii/80col").expect("Error reading test_data directory.") {
        let cur_entry = entry.unwrap().path();
        if cur_entry.extension().unwrap() != "ata" {
            continue;
        }

        let data = fs::read(&cur_entry).unwrap_or_else(|e| panic!("Error reading file {:?}: {}", cur_entry, e));
        let data = icy_sauce::strip_sauce(&data, icy_sauce::StripMode::All);

        let mut screen = ScreenMode::Atascii(80).create_screen(TerminalEmulation::ATAscii, None);
        super::run_parser_compare(&mut screen, &cur_entry, &data);
    }
}
