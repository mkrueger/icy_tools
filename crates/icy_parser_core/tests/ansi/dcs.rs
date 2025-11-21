use super::*;
use icy_parser_core::{AnsiParser, CommandParser};

#[test]
fn test_dcs_error() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // DCS with unknown content now reports error instead of Unknown
    parser.parse(b"\x1BPHello\x1B\\World", &mut sink);
    assert_eq!(sink.cmds.len(), 0); // No commands emitted for malformed DCS
    assert_eq!(sink.text, b"World");

    sink.text.clear();

    // DCS with ESC in the middle (not a terminator) also reports error
    parser.parse(b"\x1BPTest\x1BData\x1B\\", &mut sink);
    assert_eq!(sink.cmds.len(), 0); // No commands emitted for malformed DCS

    sink.text.clear();
}

#[test]
fn test_dcs_sixel() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // DCS for sixel graphics
    parser.parse(b"\x1BP0;0;8q\"1;1;80;80#0;2;0;0;0#1!80~-#1!80~-\x1B\\", &mut sink);
    assert_eq!(sink.dcs_commands.len(), 1);
    if let DeviceControlString::Sixel {
        aspect_ratio: _,
        zero_color: _,
        grid_size: _,
        sixel_data,
    } = &sink.dcs_commands[0]
    {
        // TODO: Update these assertions based on actual parameter parsing
        assert!(sixel_data.starts_with(b"\"1;1;80;80"));
    } else {
        panic!("Expected Sixel");
    }
}

#[test]
fn test_dcs_font_loading() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // DCS for custom font loading: CTerm:Font:{slot}:{base64_data}
    // Base64 "dGVzdGRhdGE=" decodes to "testdata"
    parser.parse(b"\x1BPCTerm:Font:5:dGVzdGRhdGE=\x1B\\", &mut sink);
    assert_eq!(sink.dcs_commands.len(), 1);
    if let DeviceControlString::LoadFont(slot, data) = &sink.dcs_commands[0] {
        assert_eq!(slot, &5);
        assert_eq!(data, b"testdata");
    } else {
        panic!("Expected LoadFont");
    }
}
