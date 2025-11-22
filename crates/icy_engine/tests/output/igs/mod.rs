use icy_engine::ScreenMode;
use icy_net::telnet::TerminalEmulation;
use std::fs::{self};

#[test]
pub fn test_igs_lowres() {
    crate::init_logging();
    for entry in fs::read_dir("tests/output/igs/lowres").expect("Error reading test_data directory.") {
        let cur_entry = entry.unwrap().path();
        if !cur_entry.is_file() || cur_entry.extension().and_then(|e| e.to_str()) != Some("ig") {
            continue;
        }
        log::info!("Testing IGS file: {:?}", cur_entry);
        let data = fs::read(&cur_entry).unwrap_or_else(|e| panic!("Error reading file {:?}: {}", cur_entry, e));

        let mut screen = ScreenMode::AtariST(icy_engine::TerminalResolution::Low, true).create_screen(TerminalEmulation::AtariST, None);
        super::run_parser_compare(&mut screen, &cur_entry, &data);
    }
}

#[test]
pub fn test_igs_midres() {
    crate::init_logging();
    for entry in fs::read_dir("tests/output/igs/midres").expect("Error reading test_data directory.") {
        let cur_entry = entry.unwrap().path();
        if !cur_entry.is_file() || cur_entry.extension().and_then(|e| e.to_str()) != Some("ig") {
            continue;
        }
        println!("-------------");
        let data = fs::read(&cur_entry).unwrap_or_else(|e| panic!("Error reading file {:?}: {}", cur_entry, e));

        let mut screen = ScreenMode::AtariST(icy_engine::TerminalResolution::Medium, true).create_screen(TerminalEmulation::AtariST, None);
        super::run_parser_compare(&mut screen, &cur_entry, &data);
    }
}

#[test]
pub fn test_igs_palette() {
    crate::init_logging();
    for entry in fs::read_dir("tests/output/igs/palette").expect("Error reading test_data directory.") {
        let cur_entry = entry.unwrap().path();
        if !cur_entry.is_file() || cur_entry.extension().and_then(|e| e.to_str()) != Some("ig") {
            continue;
        }
        log::info!("Testing IGS file: {:?}", cur_entry);
        let data = fs::read(&cur_entry).unwrap_or_else(|e| panic!("Error reading file {:?}: {}", cur_entry, e));

        let mut screen = ScreenMode::AtariST(icy_engine::TerminalResolution::Low, true).create_screen(TerminalEmulation::AtariST, None);
        super::run_parser_compare(&mut screen, &cur_entry, &data);
    }
}

#[test]
pub fn test_igs_text_effects() {
    crate::init_logging();
    for entry in fs::read_dir("tests/output/igs/text_effect").expect("Error reading test_data directory.") {
        let cur_entry = entry.unwrap().path();
        if !cur_entry.is_file() || cur_entry.extension().and_then(|e| e.to_str()) != Some("ig") {
            continue;
        }
        log::info!("Testing IGS file: {:?}", cur_entry);
        let data = fs::read(&cur_entry).unwrap_or_else(|e| panic!("Error reading file {:?}: {}", cur_entry, e));

        let mut screen = ScreenMode::AtariST(icy_engine::TerminalResolution::Low, true).create_screen(TerminalEmulation::AtariST, None);
        super::run_parser_compare(&mut screen, &cur_entry, &data);
    }
}
