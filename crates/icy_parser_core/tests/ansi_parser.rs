use icy_parser_core::{
    AnsiMode, AnsiParser, Blink, Color, CommandParser, CommandSink, DecPrivateMode, EraseInDisplayMode, EraseInLineMode, Intensity, ParseError, SgrAttribute,
    TerminalCommand, Underline,
};

struct CollectSink {
    pub cmds: Vec<TerminalCommand<'static>>,
}

impl CollectSink {
    fn new() -> Self {
        Self { cmds: Vec::new() }
    }
}

impl CommandSink for CollectSink {
    fn emit(&mut self, cmd: TerminalCommand<'_>) {
        match cmd {
            TerminalCommand::Printable(b) => {
                let owned = b.to_vec();
                let leaked: &'static [u8] = Box::leak(owned.into_boxed_slice());
                self.cmds.push(TerminalCommand::Printable(leaked));
            }
            TerminalCommand::CsiSelectGraphicRendition(params) => {
                self.cmds.push(TerminalCommand::CsiSelectGraphicRendition(params));
            }
            TerminalCommand::CsiDecPrivateModeSet(params) => {
                self.cmds.push(TerminalCommand::CsiDecPrivateModeSet(params));
            }
            TerminalCommand::CsiDecPrivateModeReset(params) => {
                self.cmds.push(TerminalCommand::CsiDecPrivateModeReset(params));
            }
            TerminalCommand::CsiSetMode(params) => {
                self.cmds.push(TerminalCommand::CsiSetMode(params));
            }
            TerminalCommand::CsiResetMode(params) => {
                self.cmds.push(TerminalCommand::CsiResetMode(params));
            }
            TerminalCommand::OscSetTitle(b) => {
                let owned = b.to_vec();
                let leaked: &'static [u8] = Box::leak(owned.into_boxed_slice());
                self.cmds.push(TerminalCommand::OscSetTitle(leaked));
            }
            TerminalCommand::OscSetIconName(b) => {
                let owned = b.to_vec();
                let leaked: &'static [u8] = Box::leak(owned.into_boxed_slice());
                self.cmds.push(TerminalCommand::OscSetIconName(leaked));
            }
            TerminalCommand::OscSetWindowTitle(b) => {
                let owned = b.to_vec();
                let leaked: &'static [u8] = Box::leak(owned.into_boxed_slice());
                self.cmds.push(TerminalCommand::OscSetWindowTitle(leaked));
            }
            TerminalCommand::OscHyperlink { params, uri } => {
                let params_owned = params.to_vec();
                let uri_owned = uri.to_vec();
                let params_leaked: &'static [u8] = Box::leak(params_owned.into_boxed_slice());
                let uri_leaked: &'static [u8] = Box::leak(uri_owned.into_boxed_slice());
                self.cmds.push(TerminalCommand::OscHyperlink {
                    params: params_leaked,
                    uri: uri_leaked,
                });
            }
            TerminalCommand::Unknown(b) => {
                let owned = b.to_vec();
                let leaked: &'static [u8] = Box::leak(owned.into_boxed_slice());
                self.cmds.push(TerminalCommand::Unknown(leaked));
            }
            // Handle all other variants
            TerminalCommand::CarriageReturn => self.cmds.push(TerminalCommand::CarriageReturn),
            TerminalCommand::LineFeed => self.cmds.push(TerminalCommand::LineFeed),
            TerminalCommand::Backspace => self.cmds.push(TerminalCommand::Backspace),
            TerminalCommand::Tab => self.cmds.push(TerminalCommand::Tab),
            TerminalCommand::FormFeed => self.cmds.push(TerminalCommand::FormFeed),
            TerminalCommand::Bell => self.cmds.push(TerminalCommand::Bell),
            TerminalCommand::Delete => self.cmds.push(TerminalCommand::Delete),
            TerminalCommand::CsiCursorUp(n) => self.cmds.push(TerminalCommand::CsiCursorUp(n)),
            TerminalCommand::CsiCursorDown(n) => self.cmds.push(TerminalCommand::CsiCursorDown(n)),
            TerminalCommand::CsiCursorForward(n) => self.cmds.push(TerminalCommand::CsiCursorForward(n)),
            TerminalCommand::CsiCursorBack(n) => self.cmds.push(TerminalCommand::CsiCursorBack(n)),
            TerminalCommand::CsiCursorNextLine(n) => self.cmds.push(TerminalCommand::CsiCursorNextLine(n)),
            TerminalCommand::CsiCursorPreviousLine(n) => self.cmds.push(TerminalCommand::CsiCursorPreviousLine(n)),
            TerminalCommand::CsiCursorHorizontalAbsolute(n) => self.cmds.push(TerminalCommand::CsiCursorHorizontalAbsolute(n)),
            TerminalCommand::CsiCursorPosition(row, col) => self.cmds.push(TerminalCommand::CsiCursorPosition(row, col)),
            TerminalCommand::CsiEraseInDisplay(n) => self.cmds.push(TerminalCommand::CsiEraseInDisplay(n)),
            TerminalCommand::CsiEraseInLine(n) => self.cmds.push(TerminalCommand::CsiEraseInLine(n)),
            TerminalCommand::CsiScrollUp(n) => self.cmds.push(TerminalCommand::CsiScrollUp(n)),
            TerminalCommand::CsiScrollDown(n) => self.cmds.push(TerminalCommand::CsiScrollDown(n)),
            TerminalCommand::CsiSetScrollingRegion(top, bottom) => self.cmds.push(TerminalCommand::CsiSetScrollingRegion(top, bottom)),
            TerminalCommand::CsiInsertCharacter(n) => self.cmds.push(TerminalCommand::CsiInsertCharacter(n)),
            TerminalCommand::CsiDeleteCharacter(n) => self.cmds.push(TerminalCommand::CsiDeleteCharacter(n)),
            TerminalCommand::CsiEraseCharacter(n) => self.cmds.push(TerminalCommand::CsiEraseCharacter(n)),
            TerminalCommand::CsiInsertLine(n) => self.cmds.push(TerminalCommand::CsiInsertLine(n)),
            TerminalCommand::CsiDeleteLine(n) => self.cmds.push(TerminalCommand::CsiDeleteLine(n)),
            TerminalCommand::CsiRepeatPrecedingCharacter(n) => self.cmds.push(TerminalCommand::CsiRepeatPrecedingCharacter(n)),
            TerminalCommand::CsiDeviceAttributes => self.cmds.push(TerminalCommand::CsiDeviceAttributes),
            TerminalCommand::CsiDeviceStatusReport(n) => self.cmds.push(TerminalCommand::CsiDeviceStatusReport(n)),
            TerminalCommand::EscIndex => self.cmds.push(TerminalCommand::EscIndex),
            TerminalCommand::EscNextLine => self.cmds.push(TerminalCommand::EscNextLine),
            TerminalCommand::EscSetTab => self.cmds.push(TerminalCommand::EscSetTab),
            TerminalCommand::EscReverseIndex => self.cmds.push(TerminalCommand::EscReverseIndex),
            TerminalCommand::EscSaveCursor => self.cmds.push(TerminalCommand::EscSaveCursor),
            TerminalCommand::EscRestoreCursor => self.cmds.push(TerminalCommand::EscRestoreCursor),
            TerminalCommand::EscReset => self.cmds.push(TerminalCommand::EscReset),
            // Avatar commands (should not appear in ANSI parser tests)
            TerminalCommand::AvtRepeatChar(_, _) => {
                panic!("Avatar commands should not appear in ANSI parser tests")
            }
        }
    }
}

#[test]
fn test_basic_text() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    parser.parse(b"Hello World", &mut sink);

    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(sink.cmds[0], TerminalCommand::Printable(_)));
    if let TerminalCommand::Printable(text) = &sink.cmds[0] {
        assert_eq!(text, b"Hello World");
    }
}

#[test]
fn test_control_characters() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    parser.parse(b"Hello\r\nWorld", &mut sink);

    assert_eq!(sink.cmds.len(), 4);
    assert!(matches!(sink.cmds[0], TerminalCommand::Printable(_)));
    assert!(matches!(sink.cmds[1], TerminalCommand::CarriageReturn));
    assert!(matches!(sink.cmds[2], TerminalCommand::LineFeed));
    assert!(matches!(sink.cmds[3], TerminalCommand::Printable(_)));
}

#[test]
fn test_csi_cursor_movement() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // ESC[5A - Cursor Up 5
    parser.parse(b"\x1B[5A", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::CsiCursorUp(5));

    sink.cmds.clear();

    // ESC[B - Cursor Down 1 (default)
    parser.parse(b"\x1B[B", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    assert_eq!(sink.cmds[0], TerminalCommand::CsiCursorDown(1));

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
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(sink.cmds[0], TerminalCommand::OscSetTitle(_)));
    if let TerminalCommand::OscSetTitle(title) = &sink.cmds[0] {
        assert_eq!(title, b"My Title");
    }

    sink.cmds.clear();

    // ESC]2;Another Title ESC\ - Set window title with ST terminator
    parser.parse(b"\x1B]2;Another Title\x1B\\", &mut sink);
    assert_eq!(sink.cmds.len(), 1);
    if let TerminalCommand::OscSetWindowTitle(title) = &sink.cmds[0] {
        assert_eq!(title, b"Another Title");
    }
}

#[test]
fn test_mixed_content() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // Text with embedded escape sequences
    parser.parse(b"Hello \x1B[1;31mRed\x1B[m World", &mut sink);

    // Should be: "Hello ", SGR(Bold), SGR(ForegroundRed), "Red", SGR(Reset), " World"
    assert_eq!(sink.cmds.len(), 6);
    assert!(matches!(sink.cmds[0], TerminalCommand::Printable(_)));
    assert!(matches!(
        sink.cmds[1],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Intensity(Intensity::Bold))
    ));
    assert!(matches!(
        sink.cmds[2],
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(1)))
    ));
    assert!(matches!(sink.cmds[3], TerminalCommand::Printable(_)));
    assert!(matches!(sink.cmds[4], TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Reset)));
    assert!(matches!(sink.cmds[5], TerminalCommand::Printable(_)));
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
        cmds: Vec<TerminalCommand<'static>>,
        errors: Vec<ParseError>,
    }

    impl CommandSink for ErrorCollectSink {
        fn emit(&mut self, cmd: TerminalCommand<'_>) {
            match cmd {
                TerminalCommand::Printable(b) => {
                    let owned = b.to_vec();
                    let leaked: &'static [u8] = Box::leak(owned.into_boxed_slice());
                    self.cmds.push(TerminalCommand::Printable(leaked));
                }
                TerminalCommand::CsiEraseInDisplay(mode) => {
                    self.cmds.push(TerminalCommand::CsiEraseInDisplay(mode));
                }
                TerminalCommand::CsiEraseInLine(mode) => {
                    self.cmds.push(TerminalCommand::CsiEraseInLine(mode));
                }
                TerminalCommand::CsiSetMode(mode) => {
                    self.cmds.push(TerminalCommand::CsiSetMode(mode));
                }
                TerminalCommand::CsiResetMode(mode) => {
                    self.cmds.push(TerminalCommand::CsiResetMode(mode));
                }
                TerminalCommand::CsiDecPrivateModeSet(mode) => {
                    self.cmds.push(TerminalCommand::CsiDecPrivateModeSet(mode));
                }
                TerminalCommand::CsiDecPrivateModeReset(mode) => {
                    self.cmds.push(TerminalCommand::CsiDecPrivateModeReset(mode));
                }
                TerminalCommand::Unknown(b) => {
                    let owned = b.to_vec();
                    let leaked: &'static [u8] = Box::leak(owned.into_boxed_slice());
                    self.cmds.push(TerminalCommand::Unknown(leaked));
                }
                _ => {}
            }
        }

        fn report_error(&mut self, error: ParseError) {
            self.errors.push(error);
        }
    }

    let mut parser = AnsiParser::new();
    let mut sink = ErrorCollectSink {
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
    // Should emit Unknown command
    assert_eq!(sink.cmds.len(), 1);
    assert!(matches!(sink.cmds[0], TerminalCommand::Unknown(_)));

    sink.cmds.clear();
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
        cmds: Vec<TerminalCommand<'static>>,
        errors: Vec<ParseError>,
    }

    impl CommandSink for ErrorCollectSink {
        fn emit(&mut self, cmd: TerminalCommand<'_>) {
            match cmd {
                TerminalCommand::CsiSetMode(mode) => {
                    self.cmds.push(TerminalCommand::CsiSetMode(mode));
                }
                TerminalCommand::CsiResetMode(mode) => {
                    self.cmds.push(TerminalCommand::CsiResetMode(mode));
                }
                _ => {}
            }
        }

        fn report_error(&mut self, error: ParseError) {
            self.errors.push(error);
        }
    }

    let mut parser = AnsiParser::new();
    let mut sink = ErrorCollectSink {
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
