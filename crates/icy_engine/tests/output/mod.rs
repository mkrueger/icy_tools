use std::path::Path;

use icy_engine::{EditableScreen, ScreenSink};
use icy_parser_core::CommandParser;

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
    let screen_ptr = &mut *screen.0;
    let mut sink = ScreenSink::new(screen_ptr);
    screen.1.parse(data, &mut sink);
    crate::compare_output(screen_ptr, src_file);
}
