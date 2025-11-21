use super::*;
use icy_parser_core::{AnsiParser, CommandParser};

#[test]
fn test_osc_sequences() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC]0;My Title BEL - Set window title
    parser.parse(b"\x1B]0;My Title\x07", &mut sink);
    assert_eq!(sink.osc_commands.len(), 1);
    if let OperatingSystemCommand::SetTitle(title) = &sink.osc_commands[0] {
        assert_eq!(title, b"My Title");
    }

    sink.osc_commands.clear();

    // ESC]2;Another Title ESC\ - Set window title with ST terminator
    parser.parse(b"\x1B]2;Another Title\x1B\\", &mut sink);
    assert_eq!(sink.osc_commands.len(), 1);
    if let OperatingSystemCommand::SetWindowTitle(title) = &sink.osc_commands[0] {
        assert_eq!(title, b"Another Title");
    }
}

#[test]
fn test_osc_palette() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // OSC 4 - Set palette color 0 to black
    parser.parse(b"\x1B]4;0;rgb:00/00/00\x07", &mut sink);

    assert_eq!(sink.osc_commands.len(), 1);
    if let OperatingSystemCommand::SetPaletteColor(index, r, g, b) = sink.osc_commands[0] {
        assert_eq!(index, 0);
        assert_eq!(r, 0x00);
        assert_eq!(g, 0x00);
        assert_eq!(b, 0x00);
    } else {
        panic!("Expected SetPaletteColor");
    }

    sink.osc_commands.clear();

    // OSC 4 - Set palette color 15 to white (using ST terminator)
    parser.parse(b"\x1B]4;15;rgb:ff/ff/ff\x1B\\", &mut sink);

    assert_eq!(sink.osc_commands.len(), 1);
    if let OperatingSystemCommand::SetPaletteColor(index, r, g, b) = sink.osc_commands[0] {
        assert_eq!(index, 15);
        assert_eq!(r, 0xff);
        assert_eq!(g, 0xff);
        assert_eq!(b, 0xff);
    } else {
        panic!("Expected SetPaletteColor");
    }

    sink.osc_commands.clear();

    // OSC 4 - Multiple palette entries
    parser.parse(b"\x1B]4;1;rgb:80/00/00;2;rgb:00/80/00\x07", &mut sink);

    assert_eq!(sink.osc_commands.len(), 2);
    if let OperatingSystemCommand::SetPaletteColor(index, r, g, b) = sink.osc_commands[0] {
        assert_eq!(index, 1);
        assert_eq!(r, 0x80);
        assert_eq!(g, 0x00);
        assert_eq!(b, 0x00);
    } else {
        panic!("Expected SetPaletteColor");
    }

    if let OperatingSystemCommand::SetPaletteColor(index, r, g, b) = sink.osc_commands[1] {
        assert_eq!(index, 2);
        assert_eq!(r, 0x00);
        assert_eq!(g, 0x80);
        assert_eq!(b, 0x00);
    } else {
        panic!("Expected SetPaletteColor");
    }
}

#[test]
fn test_osc8_hyperlinks() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // OSC 8 - Start hyperlink with URL
    parser.parse(b"\x1B]8;;http://example.com\x1B\\", &mut sink);
    assert_eq!(sink.osc_commands.len(), 1);
    if let OperatingSystemCommand::Hyperlink { params, uri } = &sink.osc_commands[0] {
        assert_eq!(params, b"");
        assert_eq!(uri, b"http://example.com");
    } else {
        panic!("Expected Hyperlink");
    }

    sink.osc_commands.clear();

    // OSC 8 - End hyperlink (empty URL)
    parser.parse(b"\x1B]8;;\x1B\\", &mut sink);
    assert_eq!(sink.osc_commands.len(), 1);
    if let OperatingSystemCommand::Hyperlink { params, uri } = &sink.osc_commands[0] {
        assert_eq!(params, b"");
        assert_eq!(uri, b"");
    } else {
        panic!("Expected Hyperlink");
    }

    sink.osc_commands.clear();

    // OSC 8 - Hyperlink with parameters (id)
    parser.parse(b"\x1B]8;id=123;http://example.com\x1B\\", &mut sink);
    assert_eq!(sink.osc_commands.len(), 1);
    if let OperatingSystemCommand::Hyperlink { params, uri } = &sink.osc_commands[0] {
        assert_eq!(params, b"id=123");
        assert_eq!(uri, b"http://example.com");
    } else {
        panic!("Expected Hyperlink");
    }
}
