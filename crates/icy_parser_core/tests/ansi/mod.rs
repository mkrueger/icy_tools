use std::u16;

use icy_parser_core::{
    AnsiMode, AnsiParser, Blink, Color, CommandParser, CommandSink, DecPrivateMode, DeviceControlString, EraseInDisplayMode, EraseInLineMode, ErrorLevel,
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
    if let TerminalCommand::CsiDecPrivateModeSet(mode) = sink.cmds[0] {
        assert_eq!(mode, DecPrivateMode::CursorVisible);
    } else {
        panic!("Expected CsiDecPrivateModeSet");
    }

    sink.cmds.clear();

    // ESC[?7l - Disable auto wrap (DECRST)
    parser.parse(b"\x1B[?7l", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiDecPrivateModeReset(mode) = sink.cmds[0] {
        assert_eq!(mode, DecPrivateMode::AutoWrap);
    } else {
        panic!("Expected CsiDecPrivateModeReset");
    }

    sink.cmds.clear();

    // ESC[?25;1000h - Multiple modes (cursor visible + VT200 mouse) - emits 2 commands
    parser.parse(b"\x1B[?25;1000h", &mut sink);
    assert_eq!(sink.cmds.len(), 2);
    if let TerminalCommand::CsiDecPrivateModeSet(mode) = sink.cmds[0] {
        assert_eq!(mode, DecPrivateMode::CursorVisible);
    } else {
        panic!("Expected CsiDecPrivateModeSet for first command");
    }
    if let TerminalCommand::CsiDecPrivateModeSet(mode) = sink.cmds[1] {
        assert_eq!(mode, DecPrivateMode::VT200Mouse);
    } else {
        panic!("Expected CsiDecPrivateModeSet for second command");
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
            command: "CsiDecPrivateModeSet",
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
    if let TerminalCommand::CsiDecPrivateModeSet(mode) = sink.cmds[0] {
        assert_eq!(mode, DecPrivateMode::CursorVisible);
    } else {
        panic!("Expected CsiDecPrivateModeSet for first command");
    }
    if let TerminalCommand::CsiDecPrivateModeSet(mode) = sink.cmds[1] {
        assert_eq!(mode, DecPrivateMode::VT200Mouse);
    } else {
        panic!("Expected CsiDecPrivateModeSet for second command");
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
    if let TerminalCommand::CsiSetMode(mode) = sink.cmds[0] {
        assert_eq!(mode, AnsiMode::InsertReplace);
    } else {
        panic!("Expected CsiSetMode");
    }

    sink.cmds.clear();

    // ESC[4l - Reset Insert/Replace Mode
    parser.parse(b"\x1B[4l", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiResetMode(mode) = sink.cmds[0] {
        assert_eq!(mode, AnsiMode::InsertReplace);
    } else {
        panic!("Expected CsiResetMode");
    }

    sink.cmds.clear();

    // ESC[4;4h - Set mode twice (duplicate) - emits 2 individual commands
    parser.parse(b"\x1B[4;4h", &mut sink);
    assert_eq!(sink.cmds.len(), 2);
    if let TerminalCommand::CsiSetMode(mode) = sink.cmds[0] {
        assert_eq!(mode, AnsiMode::InsertReplace);
    } else {
        panic!("Expected CsiSetMode for first command");
    }
    if let TerminalCommand::CsiSetMode(mode) = sink.cmds[1] {
        assert_eq!(mode, AnsiMode::InsertReplace);
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
    if let TerminalCommand::CsiSetMode(mode) = sink.cmds[0] {
        assert_eq!(mode, AnsiMode::InsertReplace);
    } else {
        panic!("Expected CsiSetMode for first command");
    }
    if let TerminalCommand::CsiSetMode(mode) = sink.cmds[1] {
        assert_eq!(mode, AnsiMode::InsertReplace);
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
