use icy_parser_core::{AvatarParser, Color, CommandParser, CommandSink, DecPrivateMode, Direction, EraseInLineMode, SgrAttribute, TerminalCommand};

/// Test helper that collects all emitted commands
struct CollectSink {
    commands: Vec<OwnedCommand>,
}

/// Owned version of TerminalCommand for testing
#[derive(Debug, PartialEq, Clone)]
enum OwnedCommand {
    Printable(Vec<u8>),
    CarriageReturn,
    LineFeed,
    Backspace,
    Tab,
    FormFeed,
    Bell,
    Delete,
    CsiMoveCursor(Direction, u16),
    CsiCursorPosition(u16, u16),
    CsiEraseInLine(EraseInLineMode),
    CsiDecPrivateModeReset(DecPrivateMode),
    CsiSelectGraphicRendition(SgrAttribute),
}

impl CommandSink for CollectSink {
    fn print(&mut self, text: &[u8]) {
        self.commands.push(OwnedCommand::Printable(text.to_vec()));
    }

    fn emit(&mut self, cmd: TerminalCommand) {
        let owned = match cmd {
            TerminalCommand::CarriageReturn => OwnedCommand::CarriageReturn,
            TerminalCommand::LineFeed => OwnedCommand::LineFeed,
            TerminalCommand::Backspace => OwnedCommand::Backspace,
            TerminalCommand::Tab => OwnedCommand::Tab,
            TerminalCommand::FormFeed => OwnedCommand::FormFeed,
            TerminalCommand::Bell => OwnedCommand::Bell,
            TerminalCommand::Delete => OwnedCommand::Delete,
            TerminalCommand::CsiMoveCursor(dir, n) => OwnedCommand::CsiMoveCursor(dir, n),
            TerminalCommand::CsiCursorPosition(r, c) => OwnedCommand::CsiCursorPosition(r, c),
            TerminalCommand::CsiEraseInLine(mode) => OwnedCommand::CsiEraseInLine(mode),
            TerminalCommand::CsiDecPrivateModeReset(mode) => OwnedCommand::CsiDecPrivateModeReset(mode),
            TerminalCommand::CsiSelectGraphicRendition(attr) => OwnedCommand::CsiSelectGraphicRendition(attr),
            _ => panic!("Unexpected command type in Avatar test"),
        };
        self.commands.push(owned);
    }
}

impl CollectSink {
    fn new() -> Self {
        Self { commands: Vec::new() }
    }
}

#[test]
fn test_basic_text() {
    let mut parser = AvatarParser::new();
    let mut sink = CollectSink::new();

    parser.parse(b"Hello, World!", &mut sink);

    assert_eq!(sink.commands.len(), 1);
    assert_eq!(sink.commands[0], OwnedCommand::Printable(b"Hello, World!".to_vec()));
}

#[test]
fn test_clear_screen() {
    let mut parser = AvatarParser::new();
    let mut sink = CollectSink::new();

    // ^L (0x0C) is clear screen in Avatar
    parser.parse(b"X\x0CY", &mut sink);

    assert_eq!(sink.commands.len(), 3);
    assert_eq!(sink.commands[0], OwnedCommand::Printable(b"X".to_vec()));
    assert_eq!(sink.commands[1], OwnedCommand::FormFeed);
    assert_eq!(sink.commands[2], OwnedCommand::Printable(b"Y".to_vec()));
}

#[test]
fn test_repeat_character() {
    let mut parser = AvatarParser::new();
    let mut sink = CollectSink::new();

    // ^Y{char}{count} - repeat character
    // \x19 = ^Y, b'b' = character, 3 = count
    parser.parse(b"X\x19b\x03Y", &mut sink);

    assert_eq!(sink.commands.len(), 3);
    assert_eq!(sink.commands[0], OwnedCommand::Printable(b"X".to_vec()));
    assert_eq!(sink.commands[1], OwnedCommand::Printable(b"bbb".to_vec()));
    assert_eq!(sink.commands[2], OwnedCommand::Printable(b"Y".to_vec()));
}

#[test]
fn test_zero_repeat() {
    let mut parser = AvatarParser::new();
    let mut sink = CollectSink::new();

    // Zero repeat count
    parser.parse(b"\x19b\x00", &mut sink);

    assert_eq!(sink.commands.len(), 0);
}

#[test]
fn test_set_color() {
    let mut parser = AvatarParser::new();
    let mut sink = CollectSink::new();

    // ^V^A{color} - set color
    // \x16 = ^V, \x01 = ^A, 0x07 = white on black (fg=7, bg=0)
    parser.parse(b"\x16\x01\x07", &mut sink);

    // Should emit 2 SGR commands: foreground white + background black
    assert_eq!(sink.commands.len(), 2);
    assert_eq!(
        sink.commands[0],
        OwnedCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(7)))
    );
    assert_eq!(
        sink.commands[1],
        OwnedCommand::CsiSelectGraphicRendition(SgrAttribute::Background(Color::Base(0)))
    );
}

#[test]
fn test_blink_on() {
    let mut parser = AvatarParser::new();
    let mut sink = CollectSink::new();

    // ^V^B - blink on (maps to DECRST 12 - disable cursor blinking)
    parser.parse(b"\x16\x02", &mut sink);

    assert_eq!(sink.commands.len(), 1);
    assert_eq!(
        sink.commands[0],
        OwnedCommand::CsiSelectGraphicRendition(SgrAttribute::Blink(icy_parser_core::Blink::Slow))
    );
}

#[test]
fn test_cursor_movement() {
    let mut parser = AvatarParser::new();
    let mut sink = CollectSink::new();

    // Test all cursor movement commands
    // ^V^C = up, ^V^D = down, ^V^E = left, ^V^F = right
    // These should map to ANSI cursor movement commands
    parser.parse(b"\x16\x03\x16\x04\x16\x05\x16\x06", &mut sink);

    assert_eq!(sink.commands.len(), 4);
    assert_eq!(sink.commands[0], OwnedCommand::CsiMoveCursor(Direction::Up, 1));
    assert_eq!(sink.commands[1], OwnedCommand::CsiMoveCursor(Direction::Down, 1));
    assert_eq!(sink.commands[2], OwnedCommand::CsiMoveCursor(Direction::Left, 1));
    assert_eq!(sink.commands[3], OwnedCommand::CsiMoveCursor(Direction::Right, 1));
}

#[test]
fn test_clear_eol() {
    let mut parser = AvatarParser::new();
    let mut sink = CollectSink::new();

    // ^V^G - clear to end of line (maps to ANSI EL)
    parser.parse(b"Text\x16\x07", &mut sink);

    assert_eq!(sink.commands.len(), 2);
    assert_eq!(sink.commands[0], OwnedCommand::Printable(b"Text".to_vec()));
    assert_eq!(sink.commands[1], OwnedCommand::CsiEraseInLine(EraseInLineMode::CursorToEnd));
}

#[test]
fn test_goto_xy() {
    let mut parser = AvatarParser::new();
    let mut sink = CollectSink::new();

    // ^V^H{row}{col} - goto XY position (maps to ANSI CUP)
    // \x16 = ^V, \x08 = ^H, 10 = row, 20 = col
    parser.parse(b"\x16\x08\x0A\x14", &mut sink);

    assert_eq!(sink.commands.len(), 1);
    assert_eq!(sink.commands[0], OwnedCommand::CsiCursorPosition(10, 20));
}

#[test]
fn test_mixed_content() {
    let mut parser = AvatarParser::new();
    let mut sink = CollectSink::new();

    // Mix of text, commands, and repeats
    // 0x0F = bright white on black (fg=15, bg=0)
    parser.parse(b"Hello\x16\x01\x0F World\x19!\x05 End", &mut sink);

    // Should be: Printable + 2 SGR (fg+bg) + Printable + Repeat + Printable = 6 commands
    assert_eq!(sink.commands.len(), 6);
    assert_eq!(sink.commands[0], OwnedCommand::Printable(b"Hello".to_vec()));
    assert_eq!(
        sink.commands[1],
        OwnedCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(15)))
    );
    assert_eq!(
        sink.commands[2],
        OwnedCommand::CsiSelectGraphicRendition(SgrAttribute::Background(Color::Base(0)))
    );
    assert_eq!(sink.commands[3], OwnedCommand::Printable(b" World".to_vec()));
    assert_eq!(sink.commands[4], OwnedCommand::Printable(b"!!!!!".to_vec()));
    assert_eq!(sink.commands[5], OwnedCommand::Printable(b" End".to_vec()));
}

#[test]
fn test_ansi_controls_in_avatar() {
    let mut parser = AvatarParser::new();
    let mut sink = CollectSink::new();

    // Avatar should handle basic ANSI control characters
    parser.parse(b"Line1\r\nLine2\tTabbed", &mut sink);

    assert_eq!(sink.commands.len(), 6);
    assert_eq!(sink.commands[0], OwnedCommand::Printable(b"Line1".to_vec()));
    assert_eq!(sink.commands[1], OwnedCommand::CarriageReturn);
    assert_eq!(sink.commands[2], OwnedCommand::LineFeed);
    assert_eq!(sink.commands[3], OwnedCommand::Printable(b"Line2".to_vec()));
    assert_eq!(sink.commands[4], OwnedCommand::Tab);
    assert_eq!(sink.commands[5], OwnedCommand::Printable(b"Tabbed".to_vec()));
}

#[test]
fn test_unknown_command() {
    let mut parser = AvatarParser::new();
    let mut sink = CollectSink::new();

    // ^V followed by an unknown command byte (e.g., 0xFF) - now reports error instead of Unknown
    parser.parse(b"\x16\xFF", &mut sink);

    assert_eq!(sink.commands.len(), 0); // No commands emitted for malformed Avatar sequence
}

#[test]
fn test_linebreak_bug_from_engine() {
    // This test is based on the actual test from icy_engine
    let mut parser = AvatarParser::new();
    let mut sink = CollectSink::new();

    let data = [
        12, 22, 1, 8, 32, 88, 22, 1, 15, 88, 25, 32, 4, 88, 22, 1, 8, 88, 32, 32, 32, 22, 1, 3, 88, 88, 22, 1, 57, 88, 88, 88, 25, 88, 7, 22, 1, 9, 25, 88, 4,
        22, 1, 25, 88, 88, 88, 88, 88, 88, 22, 1, 1, 25, 88, 13,
    ];

    parser.parse(&data, &mut sink);

    // Just verify it parses without error
    assert!(!sink.commands.is_empty());
}

#[test]
fn test_char_compression() {
    let mut parser = AvatarParser::new();
    let mut sink = CollectSink::new();

    // From icy_engine test: \x16\x01\x07A-A--A---A\x19-\x04A\x19-\x05A\x19-\x06A\x19-\x07A
    // 0x07 = white on black
    let data = b"\x16\x01\x07A-A--A---A\x19-\x04A\x19-\x05A\x19-\x06A\x19-\x07A";
    parser.parse(data, &mut sink);

    // Verify structure - should have SGR commands first
    assert!(sink.commands.len() >= 2);
    // First two commands should be SGR for foreground and background
    assert_eq!(
        sink.commands[0],
        OwnedCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(7)))
    );
    assert_eq!(
        sink.commands[1],
        OwnedCommand::CsiSelectGraphicRendition(SgrAttribute::Background(Color::Base(0)))
    );
}
