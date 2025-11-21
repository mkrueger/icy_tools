use super::*;
use icy_parser_core::{AnsiParser, CommandParser};

#[test]
fn test_aps_sequences() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // APS with string terminator ESC \
    parser.parse(b"\x1B_AppCommand\x1B\\Text", &mut sink);
    assert_eq!(sink.aps_data.len(), 1);
    assert_eq!(sink.text, b"Text");
    assert_eq!(sink.aps_data[0], b"AppCommand");

    sink.text.clear();
    sink.aps_data.clear();

    // APS with ESC in the middle
    parser.parse(b"\x1B_Test\x1BData\x1B\\", &mut sink);
    assert_eq!(sink.aps_data.len(), 1);
    assert_eq!(sink.aps_data[0], b"Test\x1BData");
}
