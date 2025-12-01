use icy_parser_core::{AtasciiParser, CommandParser, CommandSink, Direction, TerminalCommand};

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
            TerminalCommand::CsiMoveCursor(Direction::Up, n, _) => {
                self.commands.push(format!("CursorUp: {}", n));
            }
            TerminalCommand::CsiMoveCursor(Direction::Down, n, _) => {
                self.commands.push(format!("CursorDown: {}", n));
            }
            TerminalCommand::CsiMoveCursor(Direction::Left, n, _) => {
                self.commands.push(format!("CursorBack: {}", n));
            }
            TerminalCommand::CsiMoveCursor(Direction::Right, n, _) => {
                self.commands.push(format!("CursorForward: {}", n));
            }
            TerminalCommand::CsiEraseInDisplay(_mode) => {
                self.commands.push("ClearScreen".to_string());
            }
            TerminalCommand::Backspace => {
                self.commands.push("Backspace".to_string());
            }
            TerminalCommand::LineFeed => {
                self.commands.push("LineFeed".to_string());
            }
            TerminalCommand::Bell => {
                self.commands.push("Bell".to_string());
            }
            TerminalCommand::Delete => {
                self.commands.push("Delete".to_string());
            }
            TerminalCommand::CsiInsertLine(n) => {
                self.commands.push(format!("InsertLine: {}", n));
            }
            TerminalCommand::CsiDeleteLine(n) => {
                self.commands.push(format!("DeleteLine: {}", n));
            }
            TerminalCommand::Tab => {
                self.commands.push("Tab".to_string());
            }
            TerminalCommand::CsiClearTabulation => {
                self.commands.push("ClearTab".to_string());
            }
            TerminalCommand::EscSetTab => {
                self.commands.push("SetTab".to_string());
            }
            _ => {
                self.commands.push(format!("Other: {:?}", cmd));
            }
        }
    }
}

#[test]
fn test_atascii_simple_text() {
    let mut parser = AtasciiParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"Hello World", &mut sink);

    assert_eq!(sink.commands.len(), 1);
    assert_eq!(sink.commands[0], "Text: \"Hello World\"");
}

#[test]
fn test_atascii_cursor_movement() {
    let mut parser = AtasciiParser::new();
    let mut sink = TestSink::new();

    // 0x1C = up, 0x1D = down, 0x1E = left, 0x1F = right
    parser.parse(b"\x1CUp\x1DDown\x1ELeft\x1FRight", &mut sink);

    assert_eq!(sink.commands.len(), 8);
    assert_eq!(sink.commands[0], "CursorUp: 1");
    assert_eq!(sink.commands[1], "Text: \"Up\"");
    assert_eq!(sink.commands[2], "CursorDown: 1");
    assert_eq!(sink.commands[3], "Text: \"Down\"");
    assert_eq!(sink.commands[4], "CursorBack: 1");
    assert_eq!(sink.commands[5], "Text: \"Left\"");
    assert_eq!(sink.commands[6], "CursorForward: 1");
    assert_eq!(sink.commands[7], "Text: \"Right\"");
}

#[test]
fn test_atascii_clear_screen() {
    let mut parser = AtasciiParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"Before\x7DAfter", &mut sink);

    assert_eq!(sink.commands.len(), 3);
    assert_eq!(sink.commands[0], "Text: \"Before\"");
    assert_eq!(sink.commands[1], "ClearScreen");
    assert_eq!(sink.commands[2], "Text: \"After\"");
}

#[test]
fn test_atascii_backspace() {
    let mut parser = AtasciiParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"Test\x7EBS", &mut sink);

    assert_eq!(sink.commands.len(), 3);
    assert_eq!(sink.commands[0], "Text: \"Test\"");
    assert_eq!(sink.commands[1], "Backspace");
    assert_eq!(sink.commands[2], "Text: \"BS\"");
}

#[test]
fn test_atascii_tab() {
    let mut parser = AtasciiParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"A\x7FB", &mut sink);

    assert_eq!(sink.commands.len(), 3);
    assert_eq!(sink.commands[0], "Text: \"A\"");
    assert_eq!(sink.commands[1], "Tab");
    assert_eq!(sink.commands[2], "Text: \"B\"");
}

#[test]
fn test_atascii_line_feed() {
    let mut parser = AtasciiParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"Line1\x9BLine2", &mut sink);

    assert_eq!(sink.commands.len(), 3);
    assert_eq!(sink.commands[0], "Text: \"Line1\"");
    assert_eq!(sink.commands[1], "LineFeed");
    assert_eq!(sink.commands[2], "Text: \"Line2\"");
}

#[test]
fn test_atascii_insert_delete_line() {
    let mut parser = AtasciiParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x9DInsert\x9CDelete", &mut sink);

    assert_eq!(sink.commands.len(), 4);
    assert_eq!(sink.commands[0], "InsertLine: 1");
    assert_eq!(sink.commands[1], "Text: \"Insert\"");
    assert_eq!(sink.commands[2], "DeleteLine: 1");
    assert_eq!(sink.commands[3], "Text: \"Delete\"");
}

#[test]
fn test_atascii_tab_stops() {
    let mut parser = AtasciiParser::new();
    let mut sink = TestSink::new();

    // Set tab, clear tab
    parser.parse(b"\x9FSet\x9EClear", &mut sink);

    assert_eq!(sink.commands.len(), 4);
    assert_eq!(sink.commands[0], "SetTab");
    assert_eq!(sink.commands[1], "Text: \"Set\"");
    assert_eq!(sink.commands[2], "ClearTab");
    assert_eq!(sink.commands[3], "Text: \"Clear\"");
}

#[test]
fn test_atascii_bell() {
    let mut parser = AtasciiParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"Beep\xFDNow", &mut sink);

    assert_eq!(sink.commands.len(), 3);
    assert_eq!(sink.commands[0], "Text: \"Beep\"");
    assert_eq!(sink.commands[1], "Bell");
    assert_eq!(sink.commands[2], "Text: \"Now\"");
}

#[test]
fn test_atascii_delete_insert_char() {
    let mut parser = AtasciiParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\xFEDelete\xFFInsert", &mut sink);

    assert_eq!(sink.commands.len(), 4);
    assert_eq!(sink.commands[0], "Delete");
    assert_eq!(sink.commands[1], "Text: \"Delete\"");
    assert_eq!(sink.commands[2], "Text: \" \""); // Space from 0xFF
    assert_eq!(sink.commands[3], "Text: \"Insert\"");
}

#[test]
fn test_atascii_escape_literal() {
    let mut parser = AtasciiParser::new();
    let mut sink = TestSink::new();

    // ESC followed by a character prints it literally
    parser.parse(b"Normal\x1B\x7DLiteral", &mut sink);

    assert_eq!(sink.commands.len(), 3);
    assert_eq!(sink.commands[0], "Text: \"Normal\"");
    assert_eq!(sink.commands[1], "Text: \"}\""); // 0x7D printed literally instead of clear screen
    assert_eq!(sink.commands[2], "Text: \"Literal\"");
}

#[test]
fn test_atascii_inverse_video() {
    let mut parser = AtasciiParser::new();
    let mut sink = TestSink::new();

    // Characters >= 0x80 are inverse video
    // They should be passed through in Printable for the consumer to handle
    parser.parse(b"Normal\x80\x81\x82Inverse", &mut sink);

    // All text is in one continuous printable run since high-bit characters
    // are not control codes in ATASCII - they're just inverse video variants
    assert_eq!(sink.commands.len(), 1);
    assert!(sink.commands[0].starts_with("Text:"));
    assert!(sink.commands[0].contains("Normal"));
    assert!(sink.commands[0].contains("Inverse"));
}

#[test]
fn test_atascii_tab_commands() {
    let mut parser = AtasciiParser::new();
    let mut sink = TestSink::new();

    // Test clear tab (0x9E)
    parser.parse(b"\x9E", &mut sink);
    assert_eq!(sink.commands.len(), 1);
    assert_eq!(sink.commands[0], "ClearTab");

    sink.commands.clear();

    // Test set tab (0x9F)
    parser.parse(b"\x9F", &mut sink);
    assert_eq!(sink.commands.len(), 1);
    assert_eq!(sink.commands[0], "SetTab");

    sink.commands.clear();

    // Test tab forward (0x7F)
    parser.parse(b"\x7F", &mut sink);
    assert_eq!(sink.commands.len(), 1);
    assert_eq!(sink.commands[0], "Tab");

    sink.commands.clear();

    // Test combined: set tab, move, then tab to it
    parser.parse(b"\x9F\x1EText\x7F", &mut sink);
    assert_eq!(sink.commands[0], "SetTab");
    assert_eq!(sink.commands[1], "CursorBack: 1");
    assert_eq!(sink.commands[2], "Text: \"Text\"");
    assert_eq!(sink.commands[3], "Tab");
}

#[test]
fn test_atascii_mixed_content() {
    let mut parser = AtasciiParser::new();
    let mut sink = TestSink::new();

    // Complex real-world example
    parser.parse(b"\x7DHello\x1F\x1F\x1FWorld\x9B\x1DTest\x1B\xFDBeep", &mut sink);

    assert_eq!(sink.commands[0], "ClearScreen");
    assert_eq!(sink.commands[1], "Text: \"Hello\"");
    assert_eq!(sink.commands[2], "CursorForward: 1");
    assert_eq!(sink.commands[3], "CursorForward: 1");
    assert_eq!(sink.commands[4], "CursorForward: 1");
    assert_eq!(sink.commands[5], "Text: \"World\"");
    assert_eq!(sink.commands[6], "LineFeed");
    assert_eq!(sink.commands[7], "CursorDown: 1");
    assert_eq!(sink.commands[8], "Text: \"Test\"");
    // ESC makes 0xFD literal - it will be printed as the actual byte
    assert!(sink.commands[9].starts_with("Text:"));
    assert_eq!(sink.commands[10], "Text: \"Beep\"");
}
