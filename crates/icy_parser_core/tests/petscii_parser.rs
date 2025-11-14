use icy_parser_core::{CommandParser, CommandSink, Direction, PetsciiParser, TerminalCommand};

struct TestSink {
    commands: Vec<String>,
}

impl TestSink {
    fn new() -> Self {
        Self { commands: Vec::new() }
    }
}

impl CommandSink for TestSink {
    fn print(&mut self, text: &[u8]) {
        self.commands.push(format!("Text: {:?}", String::from_utf8_lossy(text)));
    }

    fn emit(&mut self, cmd: TerminalCommand) {
        match cmd {
            TerminalCommand::CsiMoveCursor(Direction::Up, n) => {
                self.commands.push(format!("CursorUp: {}", n));
            }
            TerminalCommand::CsiMoveCursor(Direction::Down, n) => {
                self.commands.push(format!("CursorDown: {}", n));
            }
            TerminalCommand::CsiMoveCursor(Direction::Left, n) => {
                self.commands.push(format!("CursorBack: {}", n));
            }
            TerminalCommand::CsiMoveCursor(Direction::Right, n) => {
                self.commands.push(format!("CursorForward: {}", n));
            }
            TerminalCommand::CsiCursorPosition(row, col) => {
                self.commands.push(format!("CursorPosition: {},{}", row, col));
            }
            TerminalCommand::CsiEraseInDisplay(mode) => {
                self.commands.push(format!("EraseInDisplay: {:?}", mode));
            }
            TerminalCommand::CsiEraseInLine(mode) => {
                self.commands.push(format!("EraseInLine: {:?}", mode));
            }
            TerminalCommand::Backspace => {
                self.commands.push("Backspace".to_string());
            }
            TerminalCommand::LineFeed => {
                self.commands.push("LineFeed".to_string());
            }
            TerminalCommand::CarriageReturn => {
                self.commands.push("CarriageReturn".to_string());
            }
            TerminalCommand::Bell => {
                self.commands.push("Bell".to_string());
            }
            TerminalCommand::CsiInsertLine(n) => {
                self.commands.push(format!("InsertLine: {}", n));
            }
            TerminalCommand::CsiDeleteLine(n) => {
                self.commands.push(format!("DeleteLine: {}", n));
            }
            TerminalCommand::CsiSelectGraphicRendition(attr) => {
                self.commands.push(format!("SGR: {:?}", attr));
            }
            TerminalCommand::CsiClearAllTabs => {
                self.commands.push("ClearAllTabs".to_string());
            }
            _ => {
                self.commands.push(format!("Other: {:?}", cmd));
            }
        }
    }
}

#[test]
fn test_petscii_simple_text() {
    let mut parser = PetsciiParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"HELLO", &mut sink);

    assert_eq!(sink.commands.len(), 5);
    assert!(sink.commands[0].contains("Text"));
}

#[test]
fn test_petscii_colors() {
    let mut parser = PetsciiParser::new();
    let mut sink = TestSink::new();

    // Test color codes: RED (0x1C), WHITE (0x05), BLUE (0x1F)
    parser.parse(b"\x1CRed\x05White\x1FBlue", &mut sink);

    // Should have: RED color command, "Red" text, WHITE color command, "White" text, BLUE color command, "Blue" text
    assert!(sink.commands.iter().any(|c| c.contains("Base(2)"))); // RED
    assert!(sink.commands.iter().any(|c| c.contains("Base(1)"))); // WHITE
    assert!(sink.commands.iter().any(|c| c.contains("Base(6)"))); // BLUE
}

#[test]
fn test_petscii_cursor_movement() {
    let mut parser = PetsciiParser::new();
    let mut sink = TestSink::new();

    // Cursor down (0x11), up (0x91), right (0x1D), left (0x9D)
    parser.parse(b"\x11\x91\x1D\x9D", &mut sink);

    assert_eq!(sink.commands.len(), 4);
    assert_eq!(sink.commands[0], "CursorDown: 1");
    assert_eq!(sink.commands[1], "CursorUp: 1");
    assert_eq!(sink.commands[2], "CursorForward: 1");
    assert_eq!(sink.commands[3], "CursorBack: 1");
}

#[test]
fn test_petscii_home() {
    let mut parser = PetsciiParser::new();
    let mut sink = TestSink::new();

    // Home cursor (0x13)
    parser.parse(b"\x13", &mut sink);

    assert_eq!(sink.commands.len(), 1);
    assert_eq!(sink.commands[0], "CursorPosition: 1,1");
}

#[test]
fn test_petscii_clear_screen() {
    let mut parser = PetsciiParser::new();
    let mut sink = TestSink::new();

    // Clear screen (0x93)
    parser.parse(b"\x93", &mut sink);

    assert_eq!(sink.commands.len(), 2);
    assert!(sink.commands[0].contains("EraseInDisplay"));
    assert_eq!(sink.commands[1], "CursorPosition: 1,1");
}

#[test]
fn test_petscii_reverse_mode() {
    let mut parser = PetsciiParser::new();
    let mut sink = TestSink::new();

    // Reverse on (0x12), text, reverse off (0x92)
    parser.parse(b"\x12RVS\x92OFF", &mut sink);

    // Should have: Reverse ON command, "RVS" text, Reverse OFF command, "OFF" text
    assert!(sink.commands.iter().any(|c| c.contains("Inverse(true)")));
    assert!(sink.commands.iter().any(|c| c.contains("Inverse(false)")));
}

#[test]
fn test_petscii_line_feed() {
    let mut parser = PetsciiParser::new();
    let mut sink = TestSink::new();

    // Line feed (0x0D) - should reset reverse mode
    parser.parse(b"\x0D", &mut sink);

    assert_eq!(sink.commands.len(), 2);
    assert_eq!(sink.commands[0], "LineFeed");
    assert!(sink.commands[1].contains("Inverse(false)"));
}

#[test]
fn test_petscii_backspace() {
    let mut parser = PetsciiParser::new();
    let mut sink = TestSink::new();

    // Backspace (0x14)
    parser.parse(b"AB\x14C", &mut sink);

    assert!(sink.commands.iter().any(|c| c == "Backspace"));
}

#[test]
fn test_petscii_bell() {
    let mut parser = PetsciiParser::new();
    let mut sink = TestSink::new();

    // Bell (0x07)
    parser.parse(b"\x07", &mut sink);

    assert_eq!(sink.commands.len(), 1);
    assert_eq!(sink.commands[0], "Bell");
}

#[test]
fn test_petscii_c128_escape_sequences() {
    let mut parser = PetsciiParser::new();
    let mut sink = TestSink::new();

    // ESC Q (clear to end of line)
    parser.parse(b"\x1BQ", &mut sink);
    assert!(sink.commands.iter().any(|c| c.contains("EraseInLine")));

    sink.commands.clear();

    // ESC D (delete line)
    parser.parse(b"\x1BD", &mut sink);
    assert!(sink.commands.iter().any(|c| c.contains("DeleteLine")));

    sink.commands.clear();

    // ESC I (insert line)
    parser.parse(b"\x1BI", &mut sink);
    assert!(sink.commands.iter().any(|c| c.contains("InsertLine")));
}

#[test]
fn test_petscii_shift_modes() {
    let mut parser = PetsciiParser::new();
    let mut sink = TestSink::new();

    // Shift mode changes don't emit commands but affect character rendering
    // 0x0E = unshifted (uppercase + graphics)
    // 0x0F or 0x8E = shifted (uppercase + lowercase)
    parser.parse(b"\x0ETest\x0FTest", &mut sink);

    // Should have text output (shift mode is internal state)
    assert!(sink.commands.iter().any(|c| c.contains("Text")));
}

#[test]
fn test_petscii_underline() {
    let mut parser = PetsciiParser::new();
    let mut sink = TestSink::new();

    // Enable underline (0x02), disable underline (0x03)
    parser.parse(b"\x02Test\x03", &mut sink);

    assert!(sink.commands.iter().any(|c| c.contains("Underline(Single)")));
    assert!(sink.commands.iter().any(|c| c.contains("Underline(Off)")));
}

#[test]
fn test_petscii_insert_char() {
    let mut parser = PetsciiParser::new();
    let mut sink = TestSink::new();

    // Insert character (0x94) - now outputs a space
    parser.parse(b"\x94", &mut sink);

    assert_eq!(sink.commands.len(), 1);
    assert_eq!(sink.commands[0], "Text: \" \""); // Space character
}

#[test]
fn test_petscii_carriage_return() {
    let mut parser = PetsciiParser::new();
    let mut sink = TestSink::new();

    // Carriage return (0x0A)
    parser.parse(b"\x0A", &mut sink);

    assert_eq!(sink.commands.len(), 1);
    assert_eq!(sink.commands[0], "CarriageReturn");
}

#[test]
fn test_petscii_mixed_content() {
    let mut parser = PetsciiParser::new();
    let mut sink = TestSink::new();

    // Mix of text, cursor movement, and colors
    parser.parse(b"HI\x11\x1CRED\x13", &mut sink);

    // Should have: "HI" text, cursor down, RED color, "RED" text, home
    assert!(sink.commands.iter().any(|c| c.contains("Text")));
    assert!(sink.commands.iter().any(|c| c == "CursorDown: 1"));
    assert!(sink.commands.iter().any(|c| c.contains("Base(2)")));
    assert!(sink.commands.iter().any(|c| c == "CursorPosition: 1,1"));
}

#[test]
fn test_petscii_pi_character() {
    let mut parser = PetsciiParser::new();
    let mut sink = TestSink::new();

    // PI character (0xFF)
    parser.parse(b"\xFF", &mut sink);

    assert_eq!(sink.commands.len(), 1);
    assert!(sink.commands[0].contains("Text"));
}
