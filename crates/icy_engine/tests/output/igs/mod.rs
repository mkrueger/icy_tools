use std::fs::{self};
use icy_engine::{ATARI, BitFont, BufferParser, EditableScreen, IGS_SYSTEM_PALETTE, Palette, PaletteScreenBuffer};

use crate::compare_output;

#[test]
pub fn test_igs_lowres() {
    for entry in fs::read_dir("tests/output/igs/lowres").expect("Error reading test_data directory.") {
        let cur_entry = entry.unwrap().path();
        if cur_entry.extension().unwrap() != "ig" {
            continue;
        }

        let data = fs::read_to_string(&cur_entry).expect("Error reading file.");
        let font = BitFont::from_bytes("", ATARI).unwrap();
        let res = icy_engine::igs::TerminalResolution::Low.get_resolution();
        let mut buffer = PaletteScreenBuffer::new(res.width, res.height, font);
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
        let font = BitFont::from_bytes("", ATARI).unwrap();
        let res = icy_engine::igs::TerminalResolution::High.get_resolution();
        let mut buffer = PaletteScreenBuffer::new(res.width, res.height, font);
        *buffer.palette_mut() = Palette::from_slice(&IGS_SYSTEM_PALETTE);

        let mut parser = icy_engine::parsers::igs::Parser::new(icy_engine::igs::TerminalResolution::Low);
        for c in data.chars() {
            parser.print_char(&mut buffer, c).unwrap();
        }

        // Pass filenames for loading expected PNG and saving output
        compare_output(&buffer, &cur_entry);
    }
}
