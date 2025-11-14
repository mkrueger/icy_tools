use icy_engine::{BufferParser, EditableScreen, IGS_SYSTEM_PALETTE, Palette, PaletteScreenBuffer};
use std::fs::{self};

use crate::compare_output;

#[test]
pub fn test_igs_lowres() {
    for entry in fs::read_dir("tests/output/igs/lowres").expect("Error reading test_data directory.") {
        let cur_entry = entry.unwrap().path();
        if cur_entry.extension().unwrap() != "ig" {
            continue;
        }

        let data = fs::read_to_string(&cur_entry).expect("Error reading file.");
        let mut buffer = PaletteScreenBuffer::new(icy_engine::GraphicsType::IGS(icy_engine::igs::TerminalResolution::Low));
        *buffer.palette_mut() = Palette::from_slice(&IGS_SYSTEM_PALETTE);

        let mut parser = icy_engine::parsers::igs::Parser::new(icy_engine::igs::TerminalResolution::Low);
        for c in data.chars() {
            parser.print_char(&mut buffer, c).unwrap();
        }

        // Pass filenames for loading expected PNG and saving output
        compare_output(&buffer, &cur_entry);
    }
}

#[test]
pub fn test_igs_highres() {
    for entry in fs::read_dir("tests/output/igs/highres").expect("Error reading test_data directory.") {
        let cur_entry = entry.unwrap().path();
        if cur_entry.extension().unwrap() != "ig" {
            continue;
        }

        let data = fs::read_to_string(&cur_entry).expect("Error reading file.");
        let mut buffer = PaletteScreenBuffer::new(icy_engine::GraphicsType::IGS(icy_engine::igs::TerminalResolution::High));
        *buffer.palette_mut() = Palette::from_slice(&IGS_SYSTEM_PALETTE);

        let mut parser = icy_engine::parsers::igs::Parser::new(icy_engine::igs::TerminalResolution::Low);
        for c in data.chars() {
            parser.print_char(&mut buffer, c).unwrap();
        }

        // Pass filenames for loading expected PNG and saving output
        compare_output(&buffer, &cur_entry);
    }
}
