use icy_engine::ScreenMode;
use icy_net::telnet::TerminalEmulation;
use std::fs::{self};

#[test]
pub fn test_ansi_output() {
    crate::init_logging();

    let mut fixtures: Vec<_> = fs::read_dir("tests/output/ansi/files")
        .expect("Error reading test_data directory.")
        .map(|entry| entry.expect("Error reading test_data entry.").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "ans"))
        .collect();
    fixtures.sort();

    for cur_entry in fixtures {
        /*
        if !cur_entry.file_name().unwrap().to_str().unwrap().starts_with("cpbug3.ans") {
        continue;
        }*/
        println!("Running test for file: {:?}", cur_entry);
        let data = fs::read(&cur_entry).unwrap_or_else(|e| panic!("Error reading file {:?}: {}", cur_entry, e));
        let data = icy_sauce::strip_sauce(&data, icy_sauce::StripMode::All);
        let mut screen = ScreenMode::Vga(80, 25).create_screen(TerminalEmulation::Ansi, None);
        super::run_parser_compare_no_errors(&mut screen, &cur_entry, data);
    }
}
