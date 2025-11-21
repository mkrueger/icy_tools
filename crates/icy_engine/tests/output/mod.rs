use icy_engine::{ScreenSink, TextScreen};
use icy_parser_core::CommandParser;

mod ansi;
mod atascii;
mod avatar;
mod igs;
mod petscii;
mod rip;
mod skypix;
mod view_data;
mod vt52;

pub fn parse_with_parser(result: &mut TextScreen, interpreter: &mut dyn CommandParser, data: &[u8]) -> icy_engine::EngineResult<()> {
    let mut sink = ScreenSink::new(result);
    interpreter.parse(data, &mut sink);
    Ok(())
}
