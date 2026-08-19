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

#[test]
fn test_unsupported_control_strings_are_ignored() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    parser.parse(b"Before\x1B^private message\x1B\\After", &mut sink);
    assert_eq!(sink.text, b"BeforeAfter");

    sink.text.clear();
    parser.parse(b"Before\x1BXsplit", &mut sink);
    parser.parse(b" string\x1B\\After", &mut sink);
    assert_eq!(sink.text, b"BeforeAfter");
}

#[test]
fn test_jxl_support_query() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    parser.parse(b"\x1B_SyncTERM:Q;JXL\x1B\\", &mut sink);

    assert_eq!(sink.requests, [icy_parser_core::TerminalRequest::JxlSupportReport]);
    assert!(sink.aps_data.is_empty());
}

#[test]
fn test_audio_channel_state_query() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    parser.parse(b"\x1B[?7n", &mut sink);
    parser.parse(b"\x1B[?7;3n", &mut sink);

    assert_eq!(
        sink.requests,
        [
            icy_parser_core::TerminalRequest::AudioChannelStateReport(None),
            icy_parser_core::TerminalRequest::AudioChannelStateReport(Some(3)),
        ]
    );
}

#[test]
fn test_libsndfile_query_is_delivered_as_aps() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    parser.parse(b"\x1B_SyncTERM:Q;libsndfile\x1B\\", &mut sink);

    assert!(sink.requests.is_empty());
    assert_eq!(sink.aps_data, [b"SyncTERM:Q;libsndfile".to_vec()]);
}
