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
fn test_osc_palette_reset() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    parser.parse(b"\x1B]104\x07", &mut sink);
    assert_eq!(sink.osc_commands, [OperatingSystemCommand::ResetPaletteColors(Vec::new())]);

    sink.osc_commands.clear();
    parser.parse(b"\x1B]104;1;15;255\x1B\\", &mut sink);
    assert_eq!(sink.osc_commands, [OperatingSystemCommand::ResetPaletteColors(vec![1, 15, 255])]);
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

#[test]
fn test_unknown_osc_is_ignored() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    parser.parse(b"Before\x1B]999;unsupported payload\x07After", &mut sink);

    assert_eq!(sink.text, b"BeforeAfter");
    assert!(sink.osc_commands.is_empty());
}

#[test]
fn test_osc_default_color_queries() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    parser.parse(b"\x1B]10;?\x1B\\\x1B]11;?\x07", &mut sink);

    assert_eq!(
        sink.requests,
        [
            TerminalRequest::OscColorReport { foreground: true },
            TerminalRequest::OscColorReport { foreground: false },
        ]
    );
}

#[test]
fn test_osc_palette_color_queries() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    parser.parse(b"\x1B]4;1;?;15;?\x1B\\", &mut sink);

    assert_eq!(
        sink.requests,
        [
            TerminalRequest::OscPaletteColorReport { index: 1 },
            TerminalRequest::OscPaletteColorReport { index: 15 }
        ]
    );
}
