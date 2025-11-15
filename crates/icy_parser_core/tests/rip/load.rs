use icy_parser_core::{CommandParser, RipParser};
use std::fs;

#[test]
pub fn test_rip_load() {
    for entry in fs::read_dir("benches/rip_data").expect("Error reading test_data directory.") {
        let cur_entry = entry.unwrap().path();
        if cur_entry.extension().unwrap() != "rip" {
            continue;
        }

        let filename = cur_entry.file_name().unwrap().to_string_lossy().to_string();

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

        // Compare line by line for better error messages
        let output_lines: Vec<&str> = output.trim().lines().collect();
        let expected_lines: Vec<&str> = expected.trim().lines().collect();

        if output_lines != expected_lines {
            println!("\n=== Mismatch in file: {} ===", filename);
            println!("Expected {} lines, got {} lines\n", expected_lines.len(), output_lines.len());

            let max_lines = output_lines.len().max(expected_lines.len());
            for i in 0..max_lines {
                let exp = expected_lines.get(i).unwrap_or(&"<MISSING>");
                let out = output_lines.get(i).unwrap_or(&"<MISSING>");

                if exp != out {
                    println!("Line {}: MISMATCH", i + 1);
                    println!("  Expected: {}", exp);
                    println!("  Got:      {}", out);
                } else if i < 10 || (i >= max_lines.saturating_sub(10)) {
                    // Show first and last 10 matching lines for context
                    println!("Line {}: OK - {}", i + 1, exp);
                }
            }
            panic!("RIP command output mismatch for file: {:?}", cur_entry.file_name().unwrap());
        }
    }
}
