use icy_engine::{ATARI, ATARI_DEFAULT_PALETTE, BitFont, BufferParser, EditableScreen, Palette, TextScreen, atascii};
use std::fs::{self};

use crate::compare_output;

#[test]
pub fn test_atascii_40() {
    for entry in fs::read_dir("tests/output/atascii/40col").expect("Error reading test_data directory.") {
        let cur_entry = entry.unwrap().path();
        if cur_entry.extension().unwrap() != "ata" {
            continue;
        }

        let data = fs::read(&cur_entry).unwrap_or_else(|e| panic!("Error reading file {:?}: {}", cur_entry, e));
        let data = icy_sauce::strip_sauce(&data, icy_sauce::StripMode::All);

        let mut screen = TextScreen::new((40, 25));
        screen.terminal_state_mut().is_terminal_buffer = true;
        screen.clear_font_table();
        screen.set_font(0, BitFont::from_bytes("", ATARI).unwrap());
        *screen.palette_mut() = Palette::from_slice(&ATARI_DEFAULT_PALETTE);
        *screen.buffer_type_mut() = icy_engine::BufferType::Atascii;

        let mut parser = atascii::Parser::default();
        for c in data {
            if let Err(err) = parser.print_char(&mut screen, *c as char) {
                eprintln!("Error parsing char '{}' ({:02X}): {}", c, c, err);
            }
        }
        // Pass filenames for loading expected PNG and saving output
        compare_output(&screen, &cur_entry);
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

        let mut screen = TextScreen::new((80, 25));
        screen.terminal_state_mut().is_terminal_buffer = true;
        screen.clear_font_table();
        screen.set_font(0, BitFont::from_bytes("", ATARI).unwrap());
        *screen.palette_mut() = Palette::from_slice(&ATARI_DEFAULT_PALETTE);
        *screen.buffer_type_mut() = icy_engine::BufferType::Atascii;

        let mut parser = atascii::Parser::default();
        for c in data {
            if let Err(err) = parser.print_char(&mut screen, *c as char) {
                eprintln!("Error parsing char '{}' ({:02X}): {}", c, c, err);
            }
        }
        // Pass filenames for loading expected PNG and saving output
        compare_output(&screen, &cur_entry);
    }
}
