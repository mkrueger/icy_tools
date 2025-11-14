use icy_parser_core::{
    AnsiMode, AnsiParser, Blink, CaretShape, Color, CommandParser, CommandSink, DecPrivateMode, DeviceControlString, Direction, EraseInDisplayMode,
    EraseInLineMode, Intensity, OperatingSystemCommand, ParseError, SgrAttribute, TerminalCommand, Underline,
};

struct CollectSink {
    pub text: Vec<u8>,
    pub cmds: Vec<TerminalCommand>,
    pub aps_data: Vec<Vec<u8>>,
    pub dcs_commands: Vec<DeviceControlString<'static>>,
    pub osc_commands: Vec<OperatingSystemCommand<'static>>,
}

impl CollectSink {
    fn new() -> Self {
        Self {
            text: Vec::new(),
            cmds: Vec::new(),
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

    fn device_control(&mut self, dcs: DeviceControlString<'_>) {
        match dcs {
            DeviceControlString::LoadFont(slot, data) => {
                self.dcs_commands.push(DeviceControlString::LoadFont(slot, data));
            }
            DeviceControlString::Sixel(scale, color, data) => {
                let owned = data.to_vec();
                let leaked: &'static [u8] = Box::leak(owned.into_boxed_slice());
                self.dcs_commands.push(DeviceControlString::Sixel(scale, color, leaked));
            }
        }
    }

    fn operating_system_command(&mut self, osc: OperatingSystemCommand<'_>) {
        match osc {
            OperatingSystemCommand::SetTitle(data) => {
                let owned = data.to_vec();
                let leaked: &'static [u8] = Box::leak(owned.into_boxed_slice());
                self.osc_commands.push(OperatingSystemCommand::SetTitle(leaked));
            }
            OperatingSystemCommand::SetIconName(data) => {
                let owned = data.to_vec();
                let leaked: &'static [u8] = Box::leak(owned.into_boxed_slice());
                self.osc_commands.push(OperatingSystemCommand::SetIconName(leaked));
            }
            OperatingSystemCommand::SetWindowTitle(data) => {
                let owned = data.to_vec();
                let leaked: &'static [u8] = Box::leak(owned.into_boxed_slice());
                self.osc_commands.push(OperatingSystemCommand::SetWindowTitle(leaked));
            }
            OperatingSystemCommand::Hyperlink { params, uri } => {
                let params_owned = params.to_vec();
                let uri_owned = uri.to_vec();
                let params_leaked: &'static [u8] = Box::leak(params_owned.into_boxed_slice());
                let uri_leaked: &'static [u8] = Box::leak(uri_owned.into_boxed_slice());
                self.osc_commands.push(OperatingSystemCommand::Hyperlink {
                    params: params_leaked,
                    uri: uri_leaked,
                });
            }
        }
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
fn test_csi_cursor_movement() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC[5A - Cursor Up 5
    parser.parse(b"\x1B[5A", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::CsiMoveCursor(Direction::Up, 5));

    sink.cmds.clear();

    // ESC[B - Cursor Down 1 (default)
    parser.parse(b"\x1B[B", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::CsiMoveCursor(Direction::Down, 1));

    sink.cmds.clear();

    // ESC[10;20H - Cursor Position row 10, col 20
    parser.parse(b"\x1B[10;20H", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::CsiCursorPosition(10, 20));
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
fn test_sgr_colors() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC[1;31m - Bold + Red foreground (emits 2 separate commands)
    parser.parse(b"\x1B[1;31m", &mut sink);
    assert_eq!(sink.cmds.len(), 2);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Intensity(Intensity::Bold))
    ));
    assert!(matches!(
        &sink.cmds[1],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(1)))
    ));

    sink.cmds.clear();

    // ESC[m - Reset (default)
    parser.parse(b"\x1B[m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(&sink.cmds[0], TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Reset)));
}

#[test]
fn test_sgr_extended_colors() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC[38;5;123m - 256-color foreground
    parser.parse(b"\x1B[38;5;123m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Extended(123)))
    ));

    sink.cmds.clear();

    // ESC[48;5;200m - 256-color background
    parser.parse(b"\x1B[48;5;200m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Background(Color::Extended(200)))
    ));

    sink.cmds.clear();

    // ESC[38;2;255;128;64m - RGB foreground
    parser.parse(b"\x1B[38;2;255;128;64m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Rgb(255, 128, 64)))
    ));

    sink.cmds.clear();

    // ESC[48;2;100;150;200m - RGB background
    parser.parse(b"\x1B[48;2;100;150;200m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Background(Color::Rgb(100, 150, 200)))
    ));
}

#[test]
fn test_sgr_styles() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC[3;4;9m - Italic + Underline + CrossedOut (emits 3 commands)
    parser.parse(b"\x1B[3;4;9m", &mut sink);
    assert_eq!(sink.cmds.len(), 3);
    assert!(matches!(&sink.cmds[0], TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Italic(true))));
    assert!(matches!(
        &sink.cmds[1],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Underline(Underline::Single))
    ));
    assert!(matches!(
        &sink.cmds[2],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::CrossedOut(true))
    ));

    sink.cmds.clear();

    // ESC[5;7m - SlowBlink + Inverse (emits 2 commands)
    parser.parse(b"\x1B[5;7m", &mut sink);
    assert_eq!(sink.cmds.len(), 2);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Blink(Blink::Slow))
    ));
    assert!(matches!(&sink.cmds[1], TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Inverse(true))));

    sink.cmds.clear();

    // ESC[21m - DoubleUnderline
    parser.parse(b"\x1B[21m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Underline(Underline::Double))
    ));
}

#[test]
fn test_sgr_bright_colors() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC[91;102m - Bright red foreground + Bright green background (emits 2 commands)
    parser.parse(b"\x1B[91;102m", &mut sink);
    assert_eq!(sink.cmds.len(), 2);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(9)))
    ));
    assert!(matches!(
        &sink.cmds[1],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Background(Color::Base(10)))
    ));

    sink.cmds.clear();

    // ESC[97;100m - Bright white foreground + Bright black background (emits 2 commands)
    parser.parse(b"\x1B[97;100m", &mut sink);
    assert_eq!(sink.cmds.len(), 2);
    assert!(matches!(
        &sink.cmds[0],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(15)))
    ));
    assert!(matches!(
        &sink.cmds[1],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Background(Color::Base(8)))
    ));
}

#[test]
fn test_sgr_fonts() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC[10m - Primary font
    parser.parse(b"\x1B[10m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(&sink.cmds[0], TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Font(0))));

    sink.cmds.clear();

    // ESC[15m - Alternative font 5
    parser.parse(b"\x1B[15m", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(&sink.cmds[0], TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Font(5))));
}

#[test]
fn test_esc_sequences() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC 7 - Save Cursor
    parser.parse(b"\x1B7", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::EscSaveCursor);

    sink.cmds.clear();

    // ESC 8 - Restore Cursor
    parser.parse(b"\x1B8", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::EscRestoreCursor);

    sink.cmds.clear();

    // ESC M - Reverse Index
    parser.parse(b"\x1BM", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::EscReverseIndex);
}

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
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(1)))
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
fn test_scrolling_region() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC[5;20r - Set scrolling region from line 5 to 20
    parser.parse(b"\x1B[5;20r", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::CsiSetScrollingRegion(5, 20));
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

        fn report_error(&mut self, error: ParseError) {
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
            value: 99,
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
            value: 5,
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
            value: 99,
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
            value: 9999,
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
        text: Vec<u8>,
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

        fn report_error(&mut self, error: ParseError) {
            self.errors.push(error);
        }
    }

    let mut parser = AnsiParser::new();
    let mut sink = ErrorCollectSink {
        text: Vec::new(),
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
            value: 99,
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
fn test_dcs_sequences() {
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

    // DCS for sixel graphics
    parser.parse(b"\x1BP0;0;8q\"1;1;80;80#0;2;0;0;0#1!80~-#1!80~-\x1B\\", &mut sink);
    assert_eq!(sink.dcs_commands.len(), 1);
    if let DeviceControlString::Sixel(scale, bg_color, data) = sink.dcs_commands[0] {
        assert_eq!(scale, 2); // Vertical scale for params 0
        assert_eq!(bg_color, (0, 0, 0));
        assert!(data.starts_with(b"\"1;1;80;80"));
    } else {
        panic!("Expected Sixel");
    }

    sink.dcs_commands.clear();

    // DCS for custom font loading: CTerm:Font:{slot}:{base64_data}
    // Base64 "dGVzdGRhdGE=" decodes to "testdata"
    parser.parse(b"\x1BPCTerm:Font:5:dGVzdGRhdGE=\x1B\\", &mut sink);
    assert_eq!(sink.dcs_commands.len(), 1);
    if let DeviceControlString::LoadFont(slot, data) = &sink.dcs_commands[0] {
        assert_eq!(*slot, 5);
        assert_eq!(data, b"testdata");
    } else {
        panic!("Expected LoadFont");
    }
}

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
fn test_csi_asterisk_sequences() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // CSI Pn*z - Invoke Macro (executed internally)
    // Invoking a non-existent macro should not error or emit anything
    parser.parse(b"\x1B[5*z", &mut sink);
    assert_eq!(sink.cmds.len(), 0, "Non-existent macro should not emit commands");

    sink.cmds.clear();

    // CSI Ps1;Ps2*r - Select Communication Speed
    parser.parse(b"\x1B[9600;9600*r", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSelectCommunicationSpeed(ps1, ps2) = sink.cmds[0] {
        assert_eq!(ps1, 9600);
        assert_eq!(ps2, 9600);
    } else {
        panic!("Expected CsiSelectCommunicationSpeed");
    }

    sink.cmds.clear();

    // CSI multiple params *y - Request Checksum of Rectangular Area
    // Format: ESC[{Pid};{Ppage};{Pt};{Pl};{Pb};{Pr}*y (Pid is ignored)
    parser.parse(b"\x1B[1;2;3;4;5;6*y", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiRequestChecksumRectangularArea(ppage, pt, pl, pb, pr) = sink.cmds[0] {
        assert_eq!(ppage, 2); // Pid (1) is ignored, this is Ppage
        assert_eq!(pt, 3);
        assert_eq!(pl, 4);
        assert_eq!(pb, 5);
        assert_eq!(pr, 6);
    } else {
        panic!("Expected CsiRequestChecksumRectangularArea");
    }
}

#[test]
fn test_csi_dollar_sequences() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // CSI Ps$w - Request Tab Stop Report
    parser.parse(b"\x1B[2$w", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiRequestTabStopReport(ps) = sink.cmds[0] {
        assert_eq!(ps, 2);
    } else {
        panic!("Expected CsiRequestTabStopReport");
    }

    sink.cmds.clear();

    // CSI Pchar;Pt;Pl;Pb;Pr$x - Fill Rectangular Area
    parser.parse(b"\x1B[65;1;1;10;10$x", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiFillRectangularArea(pchar, pt, pl, pb, pr) = sink.cmds[0] {
        assert_eq!(pchar, 65); // 'A'
        assert_eq!(pt, 1);
        assert_eq!(pl, 1);
        assert_eq!(pb, 10);
        assert_eq!(pr, 10);
    } else {
        panic!("Expected CsiFillRectangularArea");
    }

    sink.cmds.clear();

    // CSI Pt;Pl;Pb;Pr$z - Erase Rectangular Area
    parser.parse(b"\x1B[5;5;15;20$z", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiEraseRectangularArea(pt, pl, pb, pr) = sink.cmds[0] {
        assert_eq!(pt, 5);
        assert_eq!(pl, 5);
        assert_eq!(pb, 15);
        assert_eq!(pr, 20);
    } else {
        panic!("Expected CsiEraseRectangularArea");
    }

    sink.cmds.clear();

    // CSI Pt;Pl;Pb;Pr${ - Selective Erase Rectangular Area
    parser.parse(b"\x1B[2;3;12;18${", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSelectiveEraseRectangularArea(pt, pl, pb, pr) = sink.cmds[0] {
        assert_eq!(pt, 2);
        assert_eq!(pl, 3);
        assert_eq!(pb, 12);
        assert_eq!(pr, 18);
    } else {
        panic!("Expected CsiSelectiveEraseRectangularArea");
    }
}

#[test]
fn test_csi_space_sequences() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // CSI Ps q - Set Caret Style (DECSCUSR)
    parser.parse(b"\x1B[3 q", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSetCaretStyle(blinking, shape) = sink.cmds[0] {
        assert_eq!(blinking, true);
        assert_eq!(shape, CaretShape::Underline);
    } else {
        panic!("Expected CsiSetCaretStyle");
    }

    sink.cmds.clear();

    // CSI 0 q - default (blinking block)
    parser.parse(b"\x1B[0 q", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSetCaretStyle(blinking, shape) = sink.cmds[0] {
        assert_eq!(blinking, true);
        assert_eq!(shape, CaretShape::Block);
    } else {
        panic!("Expected CsiSetCaretStyle");
    }

    sink.cmds.clear();

    // CSI 6 q - steady bar
    parser.parse(b"\x1B[6 q", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSetCaretStyle(blinking, shape) = sink.cmds[0] {
        assert_eq!(blinking, false);
        assert_eq!(shape, CaretShape::Bar);
    } else {
        panic!("Expected CsiSetCaretStyle");
    }

    sink.cmds.clear();

    // CSI Ps1;Ps2 D - Font Selection
    parser.parse(b"\x1B[1;5 D", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiFontSelection(ps1, ps2) = sink.cmds[0] {
        assert_eq!(ps1, 1); // slot
        assert_eq!(ps2, 5); // font number
    } else {
        panic!("Expected CsiFontSelection");
    }

    sink.cmds.clear();

    // CSI Pn A - Scroll Right
    parser.parse(b"\x1B[4 A", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiScroll(Direction::Right, n) = sink.cmds[0] {
        assert_eq!(n, 4);
    } else {
        panic!("Expected CsiScroll Right");
    }

    sink.cmds.clear();

    // CSI Pn @ - Scroll Left
    parser.parse(b"\x1B[3 @", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiScroll(Direction::Left, n) = sink.cmds[0] {
        assert_eq!(n, 3);
    } else {
        panic!("Expected CsiScroll Left");
    }
}

#[test]
fn test_cursor_position_aliases() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // CSI Pn j - Character Position Backward (alias for D)
    parser.parse(b"\x1B[5j", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiMoveCursor(Direction::Left, n) = sink.cmds[0] {
        assert_eq!(n, 5);
    } else {
        panic!("Expected CsiMoveCursor Left");
    }

    sink.cmds.clear();

    // CSI Pn k - Line Position Backward (alias for A)
    parser.parse(b"\x1B[3k", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiMoveCursor(Direction::Up, n) = sink.cmds[0] {
        assert_eq!(n, 3);
    } else {
        panic!("Expected CsiMoveCursor(Up, 3)");
    }

    sink.cmds.clear();

    // CSI Pn d - VPA - Line Position Absolute
    parser.parse(b"\x1B[10d", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiLinePositionAbsolute(n) = sink.cmds[0] {
        assert_eq!(n, 10);
    } else {
        panic!("Expected CsiLinePositionAbsolute");
    }

    sink.cmds.clear();

    // CSI Pn e - VPR - Line Position Forward
    parser.parse(b"\x1B[4e", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiLinePositionForward(n) = sink.cmds[0] {
        assert_eq!(n, 4);
    } else {
        panic!("Expected CsiLinePositionForward");
    }

    sink.cmds.clear();

    // CSI Pn a - HPR - Character Position Forward
    parser.parse(b"\x1B[7a", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiCharacterPositionForward(n) = sink.cmds[0] {
        assert_eq!(n, 7);
    } else {
        panic!("Expected CsiCharacterPositionForward");
    }

    sink.cmds.clear();

    // CSI Pn ' - HPA - Horizontal Position Absolute
    parser.parse(b"\x1B[15'", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiHorizontalPositionAbsolute(n) = sink.cmds[0] {
        assert_eq!(n, 15);
    } else {
        panic!("Expected CsiHorizontalPositionAbsolute");
    }
}

#[test]
fn test_save_restore_cursor() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // CSI s - Save Cursor Position
    parser.parse(b"\x1B[s", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(sink.cmds[0], TerminalCommand::CsiSaveCursorPosition));

    sink.cmds.clear();

    // CSI u - Restore Cursor Position
    parser.parse(b"\x1B[u", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(sink.cmds[0], TerminalCommand::CsiRestoreCursorPosition));
}

#[test]
fn test_tab_operations() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // CSI Ps g - TBC - Tabulation Clear (clear tab at current position)
    parser.parse(b"\x1B[0g", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(sink.cmds[0], TerminalCommand::CsiClearTabulation));

    sink.cmds.clear();

    // CSI 3g - Clear all tabs
    parser.parse(b"\x1B[3g", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(sink.cmds[0], TerminalCommand::CsiClearAllTabs));

    sink.cmds.clear();

    // CSI Pn Y - CVT - Cursor Line Tabulation (forward to next tab)
    parser.parse(b"\x1B[2Y", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiCursorLineTabulationForward(n) = sink.cmds[0] {
        assert_eq!(n, 2);
    } else {
        panic!("Expected CsiCursorLineTabulationForward");
    }

    sink.cmds.clear();

    // CSI Pn Z - CBT - Cursor Backward Tabulation
    parser.parse(b"\x1B[3Z", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiCursorBackwardTabulation(n) = sink.cmds[0] {
        assert_eq!(n, 3);
    } else {
        panic!("Expected CsiCursorBackwardTabulation");
    }
}

#[test]
fn test_window_manipulation() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // CSI 8;{height};{width}t - Resize Terminal
    parser.parse(b"\x1B[8;24;80t", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiResizeTerminal(height, width) = sink.cmds[0] {
        assert_eq!(height, 24);
        assert_eq!(width, 80);
    } else {
        panic!("Expected CsiResizeTerminal");
    }

    sink.cmds.clear();

    // 24-bit color selection: ESC[0;{r};{g};{b}t (background) or ESC[1;{r};{g};{b}t (foreground)
    parser.parse(b"\x1B[1;255;128;64t", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Rgb(r, g, b))) = sink.cmds[0] {
        assert_eq!(r, 255);
        assert_eq!(g, 128);
        assert_eq!(b, 64);
    } else {
        panic!("Expected CsiSelectGraphicRendition with RGB foreground color");
    }

    sink.cmds.clear();

    // 24-bit background color
    parser.parse(b"\x1B[0;100;150;200t", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Background(Color::Rgb(r, g, b))) = sink.cmds[0] {
        assert_eq!(r, 100);
        assert_eq!(g, 150);
        assert_eq!(b, 200);
    } else {
        panic!("Expected CsiSelectGraphicRendition with RGB background color");
    }
}

#[test]
fn test_special_keys() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // CSI 1 ~ - Home
    parser.parse(b"\x1B[1~", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSpecialKey(n) = sink.cmds[0] {
        assert_eq!(n, 1);
    } else {
        panic!("Expected CsiSpecialKey");
    }

    sink.cmds.clear();

    // CSI 2 ~ - Insert
    parser.parse(b"\x1B[2~", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSpecialKey(n) = sink.cmds[0] {
        assert_eq!(n, 2);
    } else {
        panic!("Expected CsiSpecialKey");
    }

    sink.cmds.clear();

    // CSI 3 ~ - Delete
    parser.parse(b"\x1B[3~", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSpecialKey(n) = sink.cmds[0] {
        assert_eq!(n, 3);
    } else {
        panic!("Expected CsiSpecialKey");
    }

    sink.cmds.clear();

    // CSI 5 ~ - Page Up
    parser.parse(b"\x1B[5~", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSpecialKey(n) = sink.cmds[0] {
        assert_eq!(n, 5);
    } else {
        panic!("Expected CsiSpecialKey");
    }

    sink.cmds.clear();

    // CSI 6 ~ - Page Down
    parser.parse(b"\x1B[6~", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::CsiSpecialKey(n) = sink.cmds[0] {
        assert_eq!(n, 6);
    } else {
        panic!("Expected CsiSpecialKey");
    }
}
