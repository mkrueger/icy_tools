use std::path::Path;

use icy_engine::{EditableScreen, ScreenSink};
use icy_parser_core::{CommandParser, ErrorLevel};

mod ansi;
mod atascii;
mod avatar;
mod igs;
mod petscii;
mod rip;
// mod skypix;
mod view_data;
mod vt52;

mod ar9px;

pub fn run_parser_compare(screen: &mut (Box<dyn EditableScreen>, Box<dyn CommandParser + Send>), src_file: &Path, data: &[u8]) {
    run_parser_compare_impl(screen, src_file, data, false);
}

pub fn run_parser_compare_no_errors(screen: &mut (Box<dyn EditableScreen>, Box<dyn CommandParser + Send>), src_file: &Path, data: &[u8]) {
    run_parser_compare_impl(screen, src_file, data, true);
}

fn run_parser_compare_impl(screen: &mut (Box<dyn EditableScreen>, Box<dyn CommandParser + Send>), src_file: &Path, data: &[u8], fail_on_parser_errors: bool) {
    let screen_ptr = &mut *screen.0;
    let mut sink = ScreenSink::new(screen_ptr);
    screen.1.parse(data, &mut sink);
    if fail_on_parser_errors {
        let errors: Vec<_> = sink
            .diagnostics()
            .iter()
            .filter(|(_, level)| *level == ErrorLevel::Error)
            .map(|(error, _)| error.to_string())
            .collect();
        assert!(errors.is_empty(), "Parser errors for {}:\n{}", src_file.display(), errors.join("\n"));
    }
    crate::compare_output(screen_ptr, src_file);
}
