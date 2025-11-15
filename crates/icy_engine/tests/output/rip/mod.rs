use icy_engine::{BufferParser as _, PaletteScreenBuffer, ScreenSink};
use icy_parser_core::{CommandParser, RipParser};
use std::{
    env::current_dir,
    fs::{self},
};

use crate::compare_output;

#[test]
pub fn test_rip() {
    for entry in fs::read_dir("tests/output/rip/files").expect("Error reading test_data directory.") {
        let cur_entry = entry.unwrap().path();
        if cur_entry.extension().unwrap() != "rip" {
            continue;
        }

        let data = fs::read(&cur_entry).unwrap_or_else(|e| panic!("Error reading file {:?}: {}", cur_entry, e));
        let data = icy_sauce::strip_sauce(&data, icy_sauce::StripMode::All);

        let mut buffer = PaletteScreenBuffer::new(icy_engine::GraphicsType::Rip);
        icy_engine::rip::setup_rip_text_fonts(&mut buffer);

        let mut parser = RipParser::new();
        let mut sink = ScreenSink::new(&mut buffer);

        parser.parse(&data, &mut sink);

        // Pass filenames for loading expected PNG and saving output
        compare_output(&buffer, &cur_entry);
    }
}

#[test]
pub fn test_rip2() {
    let mut files = Vec::new();
    for entry in fs::read_dir("tests/output/rip/files").expect("Error reading test_data directory.") {
        let cur_entry = entry.unwrap().path();
        if cur_entry.extension().unwrap() != "rip" {
            continue;
        }
        files.push(cur_entry);
    }

    files.sort();

    for cur_entry in files {
        let data = fs::read(&cur_entry).unwrap_or_else(|e| panic!("Error reading file {:?}: {}", cur_entry, e));
        let data = icy_sauce::strip_sauce(&data, icy_sauce::StripMode::All);
        let data = String::from_utf8_lossy(&data);

        let mut buffer: PaletteScreenBuffer = PaletteScreenBuffer::new(icy_engine::GraphicsType::Rip);
        icy_engine::rip::setup_rip_text_fonts(&mut buffer);

        println!("file : {}", cur_entry.display());
        let mut ansi_parser = icy_engine::ansi::Parser::default();
        ansi_parser.bs_is_ctrl_char = true;
        let mut parser = icy_engine::rip::Parser::new(Box::new(ansi_parser), ".".into(), icy_engine::rip::RIP_SCREEN_SIZE);
        parser.record_rip_commands = true;
        for c in data.chars() {
            if c == '\x1A' {
                break;
            }
            parser.print_char(&mut buffer, c).unwrap();
        }

        use std::io::Write;
        let mut file = std::fs::File::create(format!("/tmp/{}.ripout.txt", cur_entry.file_name().unwrap().to_string_lossy())).unwrap();
        for cmd in parser.rip_commands {
            writeln!(file, "{}", cmd.to_rip_string()).unwrap();
        }
        writeln!(file, "---- DONE").unwrap();

        // Pass filenames for loading expected PNG and saving output
        //compare_output(&buffer, &cur_entry);
    }
}
