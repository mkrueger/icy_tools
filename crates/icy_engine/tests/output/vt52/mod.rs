use icy_engine::ScreenMode;
use icy_net::telnet::TerminalEmulation;
use std::{
    fs::{self},
    path::Path,
};

#[test]
pub fn test_vt52_igs() {
    crate::init_logging();
    for entry in fs::read_dir("tests/output/vt52/files").expect("Error reading test_data directory.") {
        let cur_entry = entry.unwrap().path();
        if cur_entry.extension().unwrap_or_default() != "vt52" {
            continue;
        }
        log::info!("Testing VT52 file: {:?}", cur_entry);
        let data = fs::read(&cur_entry).unwrap_or_else(|e| panic!("Error reading file {:?}: {}", cur_entry, e));

        let mut screen = ScreenMode::AtariST(icy_engine::TerminalResolution::Medium, true).create_screen(TerminalEmulation::AtariST, None);
        super::run_parser_compare(&mut screen, &cur_entry, &data);
    }
}

#[test]
pub fn test_vt52_mixed() {
    crate::init_logging();
    for entry in fs::read_dir("tests/output/vt52/files").expect("Error reading test_data directory.") {
        let cur_entry = entry.unwrap().path();
        if cur_entry.extension().unwrap_or_default() != "vt52" {
            continue;
        }
        log::info!("Testing VT52 file: {:?}", cur_entry);
        let data = fs::read(&cur_entry).unwrap_or_else(|e| panic!("Error reading file {:?}: {}", cur_entry, e));

        let mut screen = ScreenMode::AtariST(icy_engine::TerminalResolution::Medium, false).create_screen(TerminalEmulation::AtariST, None);
        super::run_parser_compare(&mut screen, &cur_entry, &data);
    }
}

#[test]
pub fn color_test() {
    crate::init_logging();

    let data = fs::read("tests/output/vt52/2st.st").unwrap();

    // Test Low resolution
    let mut screen = ScreenMode::AtariST(icy_engine::TerminalResolution::Low, false).create_screen(TerminalEmulation::AtariST, None);
    super::run_parser_compare(&mut screen, &Path::new("tests/output/vt52/2st.low.png"), &data);

    // Test Medium resolution
    let mut screen = ScreenMode::AtariST(icy_engine::TerminalResolution::Medium, false).create_screen(TerminalEmulation::AtariST, None);
    super::run_parser_compare(&mut screen, &Path::new("tests/output/vt52/2st.medium.png"), &data);

    // Test High resolution
    let mut screen = ScreenMode::AtariST(icy_engine::TerminalResolution::High, false).create_screen(TerminalEmulation::AtariST, None);
    super::run_parser_compare(&mut screen, &Path::new("tests/output/vt52/2st.high.png"), &data);
}
