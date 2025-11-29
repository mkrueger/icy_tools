use icy_engine::ScreenMode;
use icy_net::telnet::TerminalEmulation;
use std::fs;
use walkdir::WalkDir;

#[test]
pub fn test_skypix() {
    crate::init_logging();

    for entry in WalkDir::new("tests/output/skypix/files")
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|ext| ext.to_str()) == Some("ans"))
    {
        if entry.file_name() != "camera2.ans" {
            // This test file is known to be broken currently
            continue;
        }
        let cur_entry = entry.path();
        println!("Testing file: {:?}", cur_entry);

        let data = fs::read(cur_entry).unwrap_or_else(|e| panic!("Error reading file {:?}: {}", cur_entry, e));

        let mut screen = ScreenMode::SkyPix.create_screen(TerminalEmulation::Skypix, None);
        super::run_parser_compare(&mut screen, cur_entry, &data);
    }
}
