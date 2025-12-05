use icy_engine::{ScreenMode, TextBuffer};
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

#[test]
pub fn test_seq() {
    for entry in fs::read_dir("tests/output/petscii/files").expect("Error reading test_data directory.") {
        let cur_entry = entry.unwrap().path();
        if cur_entry.extension().unwrap() != "seq" {
            continue;
        }
        // Load SEQ file using the SEQ format loader directly
        let buffer = TextBuffer::load_buffer(&cur_entry, true, None).unwrap_or_else(|e| panic!("Error loading SEQ file {:?}: {}", cur_entry, e));
        crate::compare_buffer_output(&buffer, &cur_entry);
    }
}
