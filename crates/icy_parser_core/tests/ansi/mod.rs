use std::u16;

use icy_parser_core::{
    AnsiMode, AnsiParser, Blink, Color, CommandParser, CommandSink, DecMode, DeviceControlString, Direction, EraseInDisplayMode, EraseInLineMode, ErrorLevel,
    Frame, Intensity, OperatingSystemCommand, ParseError, SgrAttribute, TerminalCommand, TerminalRequest, Underline,
};

mod requests;

mod dcs;

mod aps;

mod osc;

mod control_codes;

mod margins;

mod cursor;

mod terminal;

mod rectangular;

mod sgr;

pub mod music;

pub struct CollectSink {
    pub text: Vec<u8>,
    pub cmds: Vec<TerminalCommand>,
    pub requests: Vec<TerminalRequest>,
    pub aps_data: Vec<Vec<u8>>,
    pub dcs_commands: Vec<DeviceControlString>,
    pub osc_commands: Vec<OperatingSystemCommand>,
}

impl CollectSink {
    fn new() -> Self {
        Self {
            text: Vec::new(),
            cmds: Vec::new(),
            requests: Vec::new(),
            aps_data: Vec::new(),
            dcs_commands: Vec::new(),
            osc_commands: Vec::new(),
        }
    }
}

impl CommandSink for CollectSink {
    fn print(&mut self, text: &[u8]) {
        self.text.extend_from_slice(text);
    }

    fn emit(&mut self, cmd: TerminalCommand) {
        self.cmds.push(cmd);
    }

    fn request(&mut self, request: TerminalRequest) {
        self.requests.push(request);
    }

    fn device_control(&mut self, dcs: DeviceControlString) {
        self.dcs_commands.push(dcs);
    }

    fn operating_system_command(&mut self, osc: OperatingSystemCommand) {
        self.osc_commands.push(osc);
    }

    fn aps(&mut self, data: &[u8]) {
        self.aps_data.push(data.to_vec());
    }
}

#[test]
fn test_basic_text() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    parser.parse(b"Hello World", &mut sink);

    assert_eq!(sink.cmds.len(), 0);
    assert_eq!(sink.text, b"Hello World");
}

#[test]
fn test_control_characters() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    parser.parse(b"Hello\r\nWorld", &mut sink);

    assert_eq!(sink.cmds.len(), 2);
    assert_eq!(sink.text, b"HelloWorld");
    assert!(matches!(sink.cmds[0], TerminalCommand::CarriageReturn));
    assert!(matches!(sink.cmds[1], TerminalCommand::LineFeed));
}

#[test]
fn test_csi_erase() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC[2J - Erase entire display
    parser.parse(b"\x1B[2J", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::CsiEraseInDisplay(EraseInDisplayMode::All));

    sink.cmds.clear();

    // ESC[K - Erase from cursor to end of line (default)
    parser.parse(b"\x1B[K", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::CsiEraseInLine(EraseInLineMode::CursorToEnd));
}

#[test]
fn test_esc_sequences() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC M - Reverse Index
    parser.parse(b"\x1BM", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::EscReverseIndex);
}

#[test]
fn test_mixed_content() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // Text with embedded escape sequences
    parser.parse(b"Hello \x1B[1;31mRed\x1B[m World", &mut sink);

    // Should be: SGR(Bold), SGR(ForegroundRed), SGR(Reset), with text "Hello Red World"
    assert_eq!(sink.cmds.len(), 3);
    assert_eq!(sink.text, b"Hello Red World");
    assert!(matches!(
        sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Intensity(Intensity::Bold))
    ));
    assert!(matches!(
        sink.cmds[1],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(4)))
    ));
    assert!(matches!(sink.cmds[2], TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Reset)));
}

#[test]
fn test_dec_private_modes() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC[?25h - Show cursor (DECSET)
    parser.parse(b"\x1B[?25h", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiDecSetMode(mode, true) = sink.cmds[0] {
        assert_eq!(mode, DecMode::CursorVisible);
    } else {
        panic!("Expected CsiDecSetMode with true");
    }

    sink.cmds.clear();

    // ESC[?7l - Disable auto wrap (DECRST)
    parser.parse(b"\x1B[?7l", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiDecSetMode(mode, false) = sink.cmds[0] {
        assert_eq!(mode, DecMode::AutoWrap);
    } else {
        panic!("Expected CsiDecSetMode with false");
    }

    sink.cmds.clear();

    // ESC[?25;1000h - Multiple modes (cursor visible + VT200 mouse) - emits 2 commands
    parser.parse(b"\x1B[?25;1000h", &mut sink);
    assert_eq!(sink.cmds.len(), 2);
    if let TerminalCommand::CsiDecSetMode(mode, true) = sink.cmds[0] {
        assert_eq!(mode, DecMode::CursorVisible);
    } else {
        panic!("Expected CsiDecSetMode for first command");
    }
    if let TerminalCommand::CsiDecSetMode(mode, true) = sink.cmds[1] {
        assert_eq!(mode, DecMode::VT200Mouse);
    } else {
        panic!("Expected CsiDecSetMode for second command");
    }
}

#[test]
fn test_character_operations() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC[5@ - Insert 5 characters
    parser.parse(b"\x1B[5@", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::CsiInsertCharacter(5));

    sink.cmds.clear();

    // ESC[3P - Delete 3 characters
    parser.parse(b"\x1B[3P", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::CsiDeleteCharacter(3));

    sink.cmds.clear();

    // ESC[10X - Erase 10 characters
    parser.parse(b"\x1B[10X", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::CsiEraseCharacter(10));
}

#[test]
fn test_error_reporting() {
    struct ErrorCollectSink {
        text: Vec<u8>,
        cmds: Vec<TerminalCommand>,
        errors: Vec<ParseError>,
    }

    impl CommandSink for ErrorCollectSink {
        fn print(&mut self, text: &[u8]) {
            self.text.extend_from_slice(text);
        }

        fn emit(&mut self, cmd: TerminalCommand) {
            self.cmds.push(cmd);
        }

        fn report_error(&mut self, error: ParseError, _level: ErrorLevel) {
            self.errors.push(error);
        }
    }

    let mut parser = AnsiParser::new();
    let mut sink = ErrorCollectSink {
        text: Vec::new(),
        cmds: Vec::new(),
        errors: Vec::new(),
    };

    // ESC[99J - Invalid erase in display parameter (valid: 0-3)
    parser.parse(b"\x1B[99J", &mut sink);
    assert_eq!(sink.errors.len(), 1);
    assert_eq!(
        sink.errors[0],
        ParseError::InvalidParameter {
            command: "CsiEraseInDisplay",
            value: 99.to_string(),
            expected: None,
        }
    );
    // Should still emit a default command
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::CsiEraseInDisplay(EraseInDisplayMode::CursorToEnd));

    sink.cmds.clear();
    sink.errors.clear();

    // ESC[5K - Invalid erase in line parameter (valid: 0-2)
    parser.parse(b"\x1B[5K", &mut sink);
    assert_eq!(sink.errors.len(), 1);
    assert_eq!(
        sink.errors[0],
        ParseError::InvalidParameter {
            command: "CsiEraseInLine",
            value: 5.to_string(),
            expected: None,
        }
    );
    // Should still emit a default command
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::CsiEraseInLine(EraseInLineMode::CursorToEnd));

    sink.cmds.clear();
    sink.errors.clear();

    // ESC[99n - Invalid device status report parameter (valid: 5, 6)
    parser.parse(b"\x1B[99n", &mut sink);
    assert_eq!(sink.errors.len(), 1);
    assert_eq!(
        sink.errors[0],
        ParseError::InvalidParameter {
            command: "CsiDeviceStatusReport",
            value: 99.to_string(),
            expected: None,
        }
    );
    // Should not emit any commands for invalid parameter
    assert_eq!(sink.cmds.len(), 0);

    sink.errors.clear();

    // ESC[?9999h - Invalid DEC private mode
    parser.parse(b"\x1B[?9999h", &mut sink);
    assert_eq!(sink.errors.len(), 1);
    assert_eq!(
        sink.errors[0],
        ParseError::InvalidParameter {
            command: "CsiDecSetMode",
            value: 9999.to_string(),
            expected: None,
        }
    );
    // Should not emit command for invalid modes
    assert_eq!(sink.cmds.len(), 0);

    sink.cmds.clear();
    sink.errors.clear();

    // ESC[?25;9999;1000h - Mix of valid and invalid DEC private modes - emits 2 valid commands + error
    parser.parse(b"\x1B[?25;9999;1000h", &mut sink);
    assert_eq!(sink.errors.len(), 1); // Error for mode 9999
    assert_eq!(sink.cmds.len(), 2); // Two valid mode commands
    if let TerminalCommand::CsiDecSetMode(mode, true) = sink.cmds[0] {
        assert_eq!(mode, DecMode::CursorVisible);
    } else {
        panic!("Expected CsiDecSetMode for first command");
    }
    if let TerminalCommand::CsiDecSetMode(mode, true) = sink.cmds[1] {
        assert_eq!(mode, DecMode::VT200Mouse);
    } else {
        panic!("Expected CsiDecSetMode for second command");
    }
}

#[test]
fn test_ansi_modes() {
    struct ErrorCollectSink {
        _text: Vec<u8>,
        cmds: Vec<TerminalCommand>,
        errors: Vec<ParseError>,
    }

    impl CommandSink for ErrorCollectSink {
        fn print(&mut self, _text: &[u8]) {
            // Ignore print in error test
        }

        fn emit(&mut self, cmd: TerminalCommand) {
            self.cmds.push(cmd);
        }

        fn report_error(&mut self, error: ParseError, _level: ErrorLevel) {
            self.errors.push(error);
        }
    }

    let mut parser = AnsiParser::new();
    let mut sink = ErrorCollectSink {
        _text: Vec::new(),
        cmds: Vec::new(),
        errors: Vec::new(),
    };

    // ESC[4h - Set Insert/Replace Mode
    parser.parse(b"\x1B[4h", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSetMode(mode, enabled) = sink.cmds[0] {
        assert_eq!(mode, AnsiMode::InsertReplace);
        assert!(enabled);
    } else {
        panic!("Expected CsiSetMode");
    }

    sink.cmds.clear();

    // ESC[4l - Reset Insert/Replace Mode
    parser.parse(b"\x1B[4l", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSetMode(mode, enabled) = sink.cmds[0] {
        assert_eq!(mode, AnsiMode::InsertReplace);
        assert!(!enabled);
    } else {
        panic!("Expected CsiSetMode");
    }

    sink.cmds.clear();

    // ESC[4;4h - Set mode twice (duplicate) - emits 2 individual commands
    parser.parse(b"\x1B[4;4h", &mut sink);
    assert_eq!(sink.cmds.len(), 2);
    if let TerminalCommand::CsiSetMode(mode, enabled) = sink.cmds[0] {
        assert_eq!(mode, AnsiMode::InsertReplace);
        assert!(enabled);
    } else {
        panic!("Expected CsiSetMode for first command");
    }
    if let TerminalCommand::CsiSetMode(mode, enabled) = sink.cmds[1] {
        assert_eq!(mode, AnsiMode::InsertReplace);
        assert!(enabled);
    } else {
        panic!("Expected CsiSetMode for second command");
    }

    sink.cmds.clear();
    sink.errors.clear();

    // ESC[99h - Invalid mode (valid: 4 only)
    parser.parse(b"\x1B[99h", &mut sink);
    assert_eq!(sink.errors.len(), 1);
    assert_eq!(
        sink.errors[0],
        ParseError::InvalidParameter {
            command: "CsiSetMode",
            value: 99.to_string(),
            expected: None,
        }
    );
    // Should not emit command for invalid modes
    assert_eq!(sink.cmds.len(), 0);

    sink.cmds.clear();
    sink.errors.clear();

    // ESC[4;99;4h - Mix of valid and invalid modes - emits 2 valid commands + 1 error
    parser.parse(b"\x1B[4;99;4h", &mut sink);
    assert_eq!(sink.errors.len(), 1); // Error for mode 99
    assert_eq!(sink.cmds.len(), 2); // Two valid mode 4 commands
    if let TerminalCommand::CsiSetMode(mode, enabled) = sink.cmds[0] {
        assert_eq!(mode, AnsiMode::InsertReplace);
        assert!(enabled);
    } else {
        panic!("Expected CsiSetMode for first command");
    }
    if let TerminalCommand::CsiSetMode(mode, enabled) = sink.cmds[1] {
        assert_eq!(mode, AnsiMode::InsertReplace);
        assert!(enabled);
    } else {
        panic!("Expected CsiSetMode for second command");
    }
}

#[test]
fn test_csi_asterisk_sequences() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // CSI Pn*z - Invoke Macro (executed internally)
    // Invoking a non-existent macro should not error or emit anything
    parser.parse(b"\x1B[5*z", &mut sink);
    assert_eq!(sink.cmds.len(), 0, "Non-existent macro should not emit commands");

    sink.cmds.clear();

    // CSI multiple params *y - Request Checksum of Rectangular Area
    // Format: ESC[{Pid};{Ppage};{Pt};{Pl};{Pb};{Pr}*y
    parser.parse(b"\x1B[1;2;3;4;5;6*y", &mut sink);
    assert_eq!(sink.requests.len(), 1);
    if let TerminalRequest::RequestChecksumRectangularArea {
        id,
        page,
        top,
        left,
        bottom,
        right,
    } = sink.requests[0]
    {
        assert_eq!(id, 1);
        assert_eq!(page, 2);
        assert_eq!(top, 3);
        assert_eq!(left, 4);
        assert_eq!(bottom, 5);
        assert_eq!(right, 6);
    } else {
        panic!("Expected RequestChecksumRectangularArea");
    }
}

#[test]
fn test_csi_incomplete_sequence_recovery() {
    // Test that CSI ! (incomplete sequence) followed by another CSI sequence
    // correctly falls back to ESC state and parses the next sequence.
    // Bug: CSI ! CSI 6n should parse the CSI 6n correctly after abandoning CSI !
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // CSI ! is incomplete (expects a final character like 'p' for DECSTR)
    // When followed by ESC (0x1B), the parser should abandon the incomplete
    // sequence and start parsing the new ESC sequence.
    // CSI 6n is Device Status Report - Cursor Position Report
    parser.parse(b"\x1B[!\x1B[6n", &mut sink);

    // Should have exactly one request: CursorPositionReport (CSI 6n)
    assert_eq!(sink.requests.len(), 1, "Should have parsed CSI 6n after incomplete CSI !");
    assert_eq!(sink.requests[0], TerminalRequest::CursorPositionReport);

    // No text should have been printed
    assert!(sink.text.is_empty(), "No text should be printed");

    sink.requests.clear();

    // Test with CSI > (another intermediate byte) followed by ESC
    // CSI > without proper sequence should be abandoned on ESC
    parser.parse(b"\x1B[>\x1B[6n", &mut sink);
    assert_eq!(sink.requests.len(), 1, "Should have parsed CSI 6n after incomplete CSI >");
    assert_eq!(sink.requests[0], TerminalRequest::CursorPositionReport);

    sink.requests.clear();

    // Test multiple incomplete sequences in a row with !
    parser.parse(b"\x1B[!\x1B[!\x1B[6n", &mut sink);
    assert_eq!(sink.requests.len(), 1, "Should have parsed CSI 6n after multiple CSI ! sequences");
    assert_eq!(sink.requests[0], TerminalRequest::CursorPositionReport);

    sink.requests.clear();

    // Test CSI with parameters but interrupted by ESC before final byte
    // CSI 1 ESC [ 6 n - the CSI 1 is incomplete (no final byte), should parse CSI 6n
    parser.parse(b"\x1B[1\x1B[6n", &mut sink);
    assert_eq!(sink.requests.len(), 1, "Should have parsed CSI 6n after incomplete CSI with parameter");
    assert_eq!(sink.requests[0], TerminalRequest::CursorPositionReport);
}

#[test]
fn test_csi_esc_fallback_all_intermediate_states() {
    // Test ESC fallback in all CSI intermediate states
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // CSI ? (DEC Private Mode) - incomplete, followed by new sequence
    parser.parse(b"\x1B[?25\x1B[6n", &mut sink);
    assert_eq!(sink.requests.len(), 1, "CSI ? ESC fallback failed");
    assert_eq!(sink.requests[0], TerminalRequest::CursorPositionReport);
    sink.requests.clear();
    sink.cmds.clear();

    // CSI $ (Dollar sequences) - incomplete, followed by new sequence
    parser.parse(b"\x1B[1$\x1B[6n", &mut sink);
    assert_eq!(sink.requests.len(), 1, "CSI $ ESC fallback failed");
    assert_eq!(sink.requests[0], TerminalRequest::CursorPositionReport);
    sink.requests.clear();

    // CSI * (Asterisk sequences) - incomplete, followed by new sequence
    parser.parse(b"\x1B[5*\x1B[6n", &mut sink);
    assert_eq!(sink.requests.len(), 1, "CSI * ESC fallback failed");
    assert_eq!(sink.requests[0], TerminalRequest::CursorPositionReport);
    sink.requests.clear();

    // CSI SP (Space sequences) - incomplete, followed by new sequence
    parser.parse(b"\x1B[1 \x1B[6n", &mut sink);
    assert_eq!(sink.requests.len(), 1, "CSI SP ESC fallback failed");
    assert_eq!(sink.requests[0], TerminalRequest::CursorPositionReport);
    sink.requests.clear();

    // CSI = (Equals sequences) - incomplete, followed by new sequence
    parser.parse(b"\x1B[=1\x1B[6n", &mut sink);
    assert_eq!(sink.requests.len(), 1, "CSI = ESC fallback failed");
    assert_eq!(sink.requests[0], TerminalRequest::CursorPositionReport);
    sink.requests.clear();

    // CSI < (Less sequences) - incomplete, followed by new sequence
    parser.parse(b"\x1B[<1\x1B[6n", &mut sink);
    assert_eq!(sink.requests.len(), 1, "CSI < ESC fallback failed");
    assert_eq!(sink.requests[0], TerminalRequest::CursorPositionReport);
    sink.requests.clear();

    // No text should have been printed for any of these
    assert!(sink.text.is_empty(), "No text should be printed during CSI ESC recovery");
}

#[test]
fn test_esc_fallback_preserves_next_sequence() {
    // Test that various escape sequences are correctly parsed after malformed ones
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // Test: malformed CSI followed by cursor movement
    parser.parse(b"\x1B[!\x1B[5A", &mut sink);
    assert_eq!(sink.cmds.len(), 1, "Should parse cursor up after malformed CSI");
    assert!(matches!(sink.cmds[0], TerminalCommand::CsiMoveCursor(Direction::Up, 5)));
    sink.cmds.clear();

    // Test: malformed CSI followed by SGR (color)
    parser.parse(b"\x1B[>\x1B[31m", &mut sink);
    assert_eq!(sink.cmds.len(), 1, "Should parse SGR after malformed CSI >");
    sink.cmds.clear();

    // Test: malformed CSI followed by erase
    parser.parse(b"\x1B[=\x1B[2J", &mut sink);
    assert_eq!(sink.cmds.len(), 1, "Should parse erase after malformed CSI =");
    assert_eq!(sink.cmds[0], TerminalCommand::CsiEraseInDisplay(EraseInDisplayMode::All));
    sink.cmds.clear();

    // Test: malformed CSI followed by ESC command (not CSI)
    parser.parse(b"\x1B[!\x1BM", &mut sink);
    assert_eq!(sink.cmds.len(), 1, "Should parse ESC M after malformed CSI");
    assert_eq!(sink.cmds[0], TerminalCommand::EscReverseIndex);
    sink.cmds.clear();

    // Test: multiple malformed sequences followed by valid one
    parser.parse(b"\x1B[!\x1B[>\x1B[<\x1B[H", &mut sink);
    assert_eq!(sink.cmds.len(), 1, "Should parse cursor position after multiple malformed CSIs");
    assert!(matches!(sink.cmds[0], TerminalCommand::CsiCursorPosition(1, 1)));
}
