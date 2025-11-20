use icy_parser_core::{CommandParser, IgsParser};
use std::fs;

#[test]
pub fn test_igs_load() {
    for entry in fs::read_dir("benches/igs_data").expect("Error reading test_data directory.") {
        let cur_entry = entry.unwrap().path();
        if cur_entry.extension().unwrap() != "IG" {
            continue;
        }

        let filename = cur_entry.file_name().unwrap().to_string_lossy().to_string();

        let data = fs::read(&cur_entry).unwrap_or_else(|e| panic!("Error reading file {:?}: {}", cur_entry, e));

        let mut parser = IgsParser::new();
        let mut sink = super::TestSink::new();

        parser.parse(&data, &mut sink);

        // Build output string
        let mut output = String::new();
        for cmd in &sink.igs_commands {
            output.push_str(&format!("{}\n", cmd));
        }

        // Read expected output from reference file
        let reference_file = format!("tests/igs/out/{}.out", cur_entry.file_name().unwrap().to_string_lossy());
        let expected = fs::read_to_string(&reference_file).unwrap_or_else(|e| panic!("Error reading reference file {:?}: {}", reference_file, e));

        // Compare line by line for better error messages
        let output_lines: Vec<&str> = output.trim().lines().collect();
        let expected_lines: Vec<&str> = expected.trim().lines().collect();

        let mut errors = 0;
        if output_lines != expected_lines {
            eprintln!("\n=== Mismatch in file: {} ===", filename);
            eprintln!("Expected {} lines, got {} lines\n", expected_lines.len(), output_lines.len());

            let max_lines = output_lines.len().max(expected_lines.len());
            for i in 0..max_lines {
                let exp = expected_lines.get(i).unwrap_or(&"<MISSING>");
                let out = output_lines.get(i).unwrap_or(&"<MISSING>");

                if exp != out {
                    errors += 1;
                    if errors > 10 {
                        break;
                    }
                    eprintln!("Line {}: MISMATCH", i + 1);
                    eprintln!("  Expected: {}", exp);
                    eprintln!("  Got:      {}", out);
                } else if i < 10 || (i >= max_lines.saturating_sub(10)) {
                    // Show first and last 10 matching lines for context
                    eprintln!("Line {}: OK - {}", i + 1, exp);
                }
            }
            panic!("IGS command output mismatch for file: {:?}", cur_entry.file_name().unwrap());
        }
    }
}
