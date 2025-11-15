use icy_parser_core::{CommandParser, RipParser};
use pretty_assertions::{assert_eq, assert_ne};
use std::fs;

#[test]
pub fn test_rip_load() {
    for entry in fs::read_dir("benches/rip_data").expect("Error reading test_data directory.") {
        let cur_entry = entry.unwrap().path();
        if cur_entry.extension().unwrap() != "rip" {
            continue;
        }

        let data = fs::read(&cur_entry).unwrap_or_else(|e| panic!("Error reading file {:?}: {}", cur_entry, e));

        let mut parser = RipParser::new();
        let mut sink = super::TestSink::new();

        parser.parse(&data, &mut sink);

        // Build output string
        let mut output = String::new();
        for cmd in &sink.rip_commands {
            output.push_str(&format!("{}\n", cmd));
        }

        // Read expected output from reference file
        let reference_file = format!("tests/rip/out/{}.ripout.txt", cur_entry.file_name().unwrap().to_string_lossy());
        let expected = fs::read_to_string(&reference_file).unwrap_or_else(|e| panic!("Error reading reference file {:?}: {}", reference_file, e));

        // Compare
        assert_eq!(
            output.trim(),
            expected.trim(),
            "RIP command output mismatch for file: {:?}",
            cur_entry.file_name().unwrap()
        );
    }
}
