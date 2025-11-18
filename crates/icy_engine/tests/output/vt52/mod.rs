use icy_engine::{PaletteScreenBuffer, ScreenSink};
use icy_parser_core::{CommandParser, IgsParser};
use std::{
    fs::{self},
    path::Path,
};

use crate::compare_output;

#[test]
pub fn test_vt52() {
    crate::init_logging();
    for entry in fs::read_dir("tests/output/vt52/files").expect("Error reading test_data directory.") {
        let cur_entry = entry.unwrap().path();
        if cur_entry.extension().unwrap_or_default() != "vt52" {
            continue;
        }
        log::info!("Testing VT52 file: {:?}", cur_entry);
        let data = fs::read(&cur_entry).unwrap_or_else(|e| panic!("Error reading file {:?}: {}", cur_entry, e));

        // Use Low resolution (40x25, 16 colors) for VT52 files as they were designed for Atari ST Low mode
        let mut buffer = PaletteScreenBuffer::new(icy_engine::GraphicsType::IGS(icy_engine::TerminalResolution::Medium));

        let mut parser: IgsParser = IgsParser::new();
        let mut sink = ScreenSink::new(&mut buffer);

        parser.parse(&data, &mut sink);

        // Pass filenames for loading expected PNG and saving output
        compare_output(&buffer, &cur_entry);
    }
}

#[test]
pub fn color_test() {
    crate::init_logging();

    let data = fs::read("tests/output/vt52/2st.st").unwrap();

    // Test Low resolution
    {
        let mut buffer = PaletteScreenBuffer::new(icy_engine::GraphicsType::IGS(icy_engine::TerminalResolution::Low));
        let mut parser: IgsParser = IgsParser::new();
        let mut sink = ScreenSink::new(&mut buffer);
        parser.parse(&data, &mut sink);
        compare_output(&buffer, Path::new("tests/output/vt52/2st.low.png"));
    }

    // Test Medium resolution
    {
        let mut buffer = PaletteScreenBuffer::new(icy_engine::GraphicsType::IGS(icy_engine::TerminalResolution::Medium));
        let mut parser: IgsParser = IgsParser::new();
        let mut sink = ScreenSink::new(&mut buffer);
        parser.parse(&data, &mut sink);
        compare_output(&buffer, Path::new("tests/output/vt52/2st.medium.png"));
    }

    // Test High resolution
    {
        let mut buffer = PaletteScreenBuffer::new(icy_engine::GraphicsType::IGS(icy_engine::TerminalResolution::High));
        let mut parser: IgsParser = IgsParser::new();
        let mut sink = ScreenSink::new(&mut buffer);
        parser.parse(&data, &mut sink);
        compare_output(&buffer, Path::new("tests/output/vt52/2st.high.png"));
    }
}
