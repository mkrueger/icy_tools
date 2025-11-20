use icy_parser_core::{
    Color, CommandParser, CommandSink, DecPrivateMode, Direction, EraseInDisplayMode, EraseInLineMode, ErrorLevel, IgsParser, ParseError, SgrAttribute,
    TerminalCommand,
};

struct TestSink {
    text: Vec<String>,
    commands: Vec<TerminalCommand>,
    errors: Vec<(ParseError, ErrorLevel)>,
}

impl TestSink {
    fn new() -> Self {
        Self {
            text: Vec::new(),
            commands: Vec::new(),
            errors: Vec::new(),
        }
    }
}

impl CommandSink for TestSink {
    fn print(&mut self, text: &[u8]) {
        self.text.push(String::from_utf8_lossy(text).to_string());
    }

    fn emit(&mut self, cmd: TerminalCommand) {
        self.commands.push(cmd);
    }

    fn report_error(&mut self, error: ParseError, level: ErrorLevel) {
        self.errors.push((error, level));
    }
}

#[test]
fn test_vt52_linefeed() {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x0A", &mut sink);

    assert_eq!(sink.commands.len(), 1);
    assert!(matches!(sink.commands[0], TerminalCommand::LineFeed));
}

// Cursor movement
#[test]
fn test_vt52_cursor_up() {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1BA", &mut sink);

    assert_eq!(sink.commands.len(), 1);
    assert!(matches!(sink.commands[0], TerminalCommand::CsiMoveCursor(Direction::Up, 1)));
}

#[test]
fn test_vt52_cursor_down() {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1BB", &mut sink);

    assert_eq!(sink.commands.len(), 1);
    assert!(matches!(sink.commands[0], TerminalCommand::CsiMoveCursor(Direction::Down, 1)));
}

#[test]
fn test_vt52_cursor_home() {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1BH", &mut sink);

    assert_eq!(sink.commands.len(), 1);
    assert!(matches!(sink.commands[0], TerminalCommand::CsiCursorPosition(1, 1)));
}

#[test]
fn test_vt52_set_cursor_position() {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    // ESC Y {row+32} {col+32}
    // Set cursor to (10, 5): ESC Y space+5 space+10 = ESC Y % *
    parser.parse(b"\x1BY%*", &mut sink);

    assert_eq!(sink.commands.len(), 1);
    match &sink.commands[0] {
        TerminalCommand::CsiCursorPosition(row, col) => {
            assert_eq!(*row, 6); // 5 + 1 (1-based)
            assert_eq!(*col, 11); // 10 + 1 (1-based)
        }
        _ => panic!("Expected CsiCursorPosition command"),
    }
}

// Screen clearing
#[test]
fn test_vt52_clear_screen() {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1BE", &mut sink);

    assert_eq!(sink.commands.len(), 1);
    assert!(matches!(sink.commands[0], TerminalCommand::CsiEraseInDisplay(EraseInDisplayMode::All)));
}

#[test]
fn test_vt52_clear_down() {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1BJ", &mut sink);

    assert_eq!(sink.commands.len(), 1);
    assert!(matches!(sink.commands[0], TerminalCommand::CsiEraseInDisplay(EraseInDisplayMode::CursorToEnd)));
}

#[test]
fn test_vt52_clear_end_of_line() {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1BK", &mut sink);

    assert_eq!(sink.commands.len(), 1);
    assert!(matches!(sink.commands[0], TerminalCommand::CsiEraseInLine(EraseInLineMode::CursorToEnd)));
}

#[test]
fn test_vt52_clear_line() {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1Bl", &mut sink);

    assert_eq!(sink.commands.len(), 1);
    assert!(matches!(sink.commands[0], TerminalCommand::CsiEraseInLine(EraseInLineMode::All)));
}

// Line operations
#[test]
fn test_vt52_insert_line() {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1BL", &mut sink);

    assert_eq!(sink.commands.len(), 1);
    assert!(matches!(sink.commands[0], TerminalCommand::CsiInsertLine(1)));
}

#[test]
fn test_vt52_delete_line() {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1BM", &mut sink);

    assert_eq!(sink.commands.len(), 1);
    assert!(matches!(sink.commands[0], TerminalCommand::CsiDeleteLine(1)));
}

// Colors
#[test]
fn test_vt52_set_foreground() {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1Bb\x07", &mut sink);

    assert_eq!(sink.commands.len(), 1);
    match &sink.commands[0] {
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(c))) => {
            assert_eq!(*c, 7);
        }
        _ => panic!("Expected foreground color command"),
    }
}

#[test]
fn test_vt52_set_background() {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1Bc\x01", &mut sink);

    assert_eq!(sink.commands.len(), 1);
    match &sink.commands[0] {
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Background(Color::Base(c))) => {
            assert_eq!(*c, 1);
        }
        _ => panic!("Expected background color command"),
    }
}

// Cursor save/restore
#[test]
fn test_vt52_save_cursor() {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1Bj", &mut sink);

    assert_eq!(sink.commands.len(), 1);
    assert!(matches!(sink.commands[0], TerminalCommand::CsiSaveCursorPosition));
}

#[test]
fn test_vt52_restore_cursor() {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1Bk", &mut sink);

    assert_eq!(sink.commands.len(), 1);
    assert!(matches!(sink.commands[0], TerminalCommand::CsiRestoreCursorPosition));
}

// Wrapping
#[test]
fn test_vt52_wrap_on() {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1Bv", &mut sink);

    assert_eq!(sink.commands.len(), 1);
    assert!(matches!(sink.commands[0], TerminalCommand::CsiDecPrivateModeSet(DecPrivateMode::AutoWrap)));
}

#[test]
fn test_vt52_wrap_off() {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1Bw", &mut sink);

    assert_eq!(sink.commands.len(), 1);
    assert!(matches!(sink.commands[0], TerminalCommand::CsiDecPrivateModeReset(DecPrivateMode::AutoWrap)));
}

// Cursor visibility
#[test]
fn test_vt52_show_cursor() {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1Be", &mut sink);

    assert_eq!(sink.commands.len(), 1);
    assert!(matches!(sink.commands[0], TerminalCommand::CsiDecPrivateModeSet(DecPrivateMode::CursorVisible)));
}

#[test]
fn test_vt52_hide_cursor() {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1Bf", &mut sink);

    assert_eq!(sink.commands.len(), 1);
    assert!(matches!(
        sink.commands[0],
        TerminalCommand::CsiDecPrivateModeReset(DecPrivateMode::CursorVisible)
    ));
}

// TosWin2 extensions
#[test]
fn test_vt52_ansi_foreground() {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1B3\x0F", &mut sink);

    assert_eq!(sink.commands.len(), 1);
    match &sink.commands[0] {
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(c))) => {
            assert_eq!(*c, 15);
        }
        _ => panic!("Expected foreground color command"),
    }
}

#[test]
fn test_vt52_ansi_background() {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1B4\x00", &mut sink);

    assert_eq!(sink.commands.len(), 1);
    match &sink.commands[0] {
        TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Background(Color::Base(c))) => {
            assert_eq!(*c, 0);
        }
        _ => panic!("Expected background color command"),
    }
}

// Multiple commands
#[test]
fn test_vt52_multiple_commands() {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1BH\x1BE\x1Bb\x07", &mut sink);

    // ESC H = home (1 command)
    // ESC E = clear screen + home (1 command)
    // ESC b 7 = set foreground (1 command)
    assert_eq!(sink.commands.len(), 3);
}

// Mixed text and commands
#[test]
fn test_vt52_text_and_commands() {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"Hello \x1BE World", &mut sink);

    assert_eq!(sink.commands.len(), 1); // Clear screen
    assert!(sink.text.len() > 0);
    let combined_text = sink.text.join("");
    assert!(combined_text.contains("Hello"));
    assert!(combined_text.contains("World"));
}

#[test]
fn test_vt52_invalid_cursor_position_warning() {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    // Invalid cursor position: line=0xFF (too high), row=0xFF (too high)
    // ESC Y {line} {row} - both bytes outside valid range
    parser.parse(b"\x1BY\xFF\xFF", &mut sink);

    // Should emit a warning but not crash
    assert_eq!(sink.errors.len(), 1);
    match &sink.errors[0] {
        (ParseError::InvalidParameter { command, .. }, ErrorLevel::Warning) => {
            assert_eq!(*command, "VT52 cursor position");
        }
        _ => panic!("Expected InvalidParameter warning"),
    }
    // Should not emit cursor position command due to invalid input
    assert_eq!(sink.commands.len(), 0);
}

#[test]
fn test_vt52_invalid_color_warning() {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    // Invalid foreground color: ESC b {invalid_color}
    // Using byte 0xFF which is outside valid range
    parser.parse(b"\x1Bb\xFF", &mut sink);

    // Should emit a warning
    assert_eq!(sink.errors.len(), 1);
    match &sink.errors[0] {
        (ParseError::InvalidParameter { command, .. }, ErrorLevel::Warning) => {
            assert_eq!(*command, "VT52 color");
        }
        _ => panic!("Expected InvalidParameter warning for color"),
    }
}

#[test]
fn test_vt52_unknown_escape_sequence_warning() {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    // Unknown escape sequence: ESC Z (not a valid VT52 command)
    parser.parse(b"\x1BZ", &mut sink);

    // Should emit a warning
    assert_eq!(sink.errors.len(), 1);
    match &sink.errors[0] {
        (ParseError::InvalidParameter { command, .. }, ErrorLevel::Warning) => {
            assert_eq!(*command, "VT52 escape sequence");
        }
        _ => panic!("Expected InvalidParameter warning for unknown escape"),
    }
}
