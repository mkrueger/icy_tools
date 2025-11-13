use icy_parser_core::{CommandParser, CommandSink, TerminalCommand, ViewdataParser};

struct TestSink {
    commands: Vec<String>,
}

impl TestSink {
    fn new() -> Self {
        Self { commands: Vec::new() }
    }
}

impl CommandSink for TestSink {
    fn emit(&mut self, cmd: TerminalCommand<'_>) {
        match cmd {
            TerminalCommand::Printable(s) => {
                self.commands.push(format!("Text: {:?}", String::from_utf8_lossy(s)));
            }
            TerminalCommand::CsiCursorUp(n) => {
                self.commands.push(format!("CursorUp: {}", n));
            }
            TerminalCommand::CsiCursorDown(n) => {
                self.commands.push(format!("CursorDown: {}", n));
            }
            TerminalCommand::CsiCursorBack(n) => {
                self.commands.push(format!("CursorBack: {}", n));
            }
            TerminalCommand::CsiCursorForward(n) => {
                self.commands.push(format!("CursorForward: {}", n));
            }
            TerminalCommand::CsiCursorPosition(row, col) => {
                self.commands.push(format!("CursorPosition: {},{}", row, col));
            }
            TerminalCommand::CsiEraseInDisplay(mode) => {
                self.commands.push(format!("EraseInDisplay: {:?}", mode));
            }
            TerminalCommand::CarriageReturn => {
                self.commands.push("CarriageReturn".to_string());
            }
            TerminalCommand::CsiSelectGraphicRendition(attr) => {
                self.commands.push(format!("SGR: {:?}", attr));
            }
            _ => {
                self.commands.push(format!("Other: {:?}", cmd));
            }
        }
    }
}

#[test]
fn test_viewdata_simple_text() {
    let mut parser = ViewdataParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"HELLO", &mut sink);

    assert_eq!(sink.commands.len(), 5);
    assert!(sink.commands.iter().all(|c| c.starts_with("Text")));
}

#[test]
fn test_viewdata_cursor_movement() {
    let mut parser = ViewdataParser::new();
    let mut sink = TestSink::new();

    // Left (0x08), right (0x09), down (0x0A), up (0x0B)
    parser.parse(b"\x08\x09\x0A\x0B", &mut sink);

    assert_eq!(sink.commands.len(), 4);
    assert_eq!(sink.commands[0], "CursorBack: 1");
    assert_eq!(sink.commands[1], "CursorForward: 1");
    assert_eq!(sink.commands[2], "CursorDown: 1");
    assert_eq!(sink.commands[3], "CursorUp: 1");
}

#[test]
fn test_viewdata_clear_screen() {
    let mut parser = ViewdataParser::new();
    let mut sink = TestSink::new();

    // Form feed / clear screen (0x0C)
    parser.parse(b"\x0C", &mut sink);

    assert_eq!(sink.commands.len(), 2);
    assert!(sink.commands[0].contains("EraseInDisplay"));
    assert_eq!(sink.commands[1], "CursorPosition: 1,1");
}

#[test]
fn test_viewdata_home() {
    let mut parser = ViewdataParser::new();
    let mut sink = TestSink::new();

    // Home (0x1E)
    parser.parse(b"\x1E", &mut sink);

    assert_eq!(sink.commands.len(), 1);
    assert_eq!(sink.commands[0], "CursorPosition: 1,1");
}

#[test]
fn test_viewdata_carriage_return() {
    let mut parser = ViewdataParser::new();
    let mut sink = TestSink::new();

    // CR (0x0D)
    parser.parse(b"\x0D", &mut sink);

    assert_eq!(sink.commands.len(), 1);
    assert_eq!(sink.commands[0], "CarriageReturn");
}

#[test]
fn test_viewdata_alpha_colors() {
    let mut parser = ViewdataParser::new();
    let mut sink = TestSink::new();

    // ESC A (Red), ESC B (Green), ESC C (Yellow)
    parser.parse(b"\x1BAText\x1BBMore", &mut sink);

    // Should have: Red color, concealed off, "Text" (4 chars), Green color, concealed off, "More" (4 chars)
    assert!(sink.commands.iter().any(|c| c.contains("Foreground") && c.contains("Base(1)"))); // Red
    assert!(sink.commands.iter().any(|c| c.contains("Foreground") && c.contains("Base(2)"))); // Green
    assert!(sink.commands.iter().any(|c| c.contains("Concealed(false)")));
}

#[test]
fn test_viewdata_graphics_colors() {
    let mut parser = ViewdataParser::new();
    let mut sink = TestSink::new();

    // ESC Q (Graphics Red), ESC R (Graphics Green)
    parser.parse(b"\x1BQ\x1BR", &mut sink);

    // Should have color changes
    assert!(sink.commands.iter().any(|c| c.contains("Foreground") && c.contains("Base(1)"))); // Red
    assert!(sink.commands.iter().any(|c| c.contains("Foreground") && c.contains("Base(2)"))); // Green
}

#[test]
fn test_viewdata_blink() {
    let mut parser = ViewdataParser::new();
    let mut sink = TestSink::new();

    // ESC H (blink on), ESC I (steady/blink off)
    parser.parse(b"\x1BH\x1BI", &mut sink);

    assert!(sink.commands.iter().any(|c| c.contains("Blink(Slow)")));
    assert!(sink.commands.iter().any(|c| c.contains("Blink(Off)")));
}

#[test]
fn test_viewdata_concealed() {
    let mut parser = ViewdataParser::new();
    let mut sink = TestSink::new();

    // Switch to alpha mode first, then ESC X (conceal)
    parser.parse(b"\x1BA\x1BX", &mut sink);

    assert!(sink.commands.iter().any(|c| c.contains("Concealed(true)")));
}

#[test]
fn test_viewdata_black_background() {
    let mut parser = ViewdataParser::new();
    let mut sink = TestSink::new();

    // ESC \ (black background)
    parser.parse(b"\x1B\\", &mut sink);

    assert!(sink.commands.iter().any(|c| c.contains("Background") && c.contains("Base(0)")));
    assert!(sink.commands.iter().any(|c| c.contains("Concealed(false)")));
}

#[test]
fn test_viewdata_graphics_mode() {
    let mut parser = ViewdataParser::new();
    let mut sink = TestSink::new();

    // Enter graphics mode with ESC Q (graphics red), then printable chars
    parser.parse(b"\x1BQ!", &mut sink);

    // Graphics mode should remap characters
    assert!(sink.commands.iter().any(|c| c.contains("Foreground")));
    assert!(sink.commands.iter().any(|c| c.starts_with("Text")));
}

#[test]
fn test_viewdata_contiguous_separated() {
    let mut parser = ViewdataParser::new();
    let mut sink = TestSink::new();

    // ESC Y (contiguous graphics), ESC Z (separated graphics)
    parser.parse(b"\x1BY\x1BZ", &mut sink);

    // These affect internal state but don't emit specific commands
    // Just verify it doesn't crash
    assert!(!sink.commands.is_empty());
}

#[test]
fn test_viewdata_hold_release_graphics() {
    let mut parser = ViewdataParser::new();
    let mut sink = TestSink::new();

    // ESC ^ (hold graphics), ESC _ (release graphics)
    parser.parse(b"\x1B^\x1B_", &mut sink);

    // These affect internal state
    assert!(!sink.commands.is_empty());
}

#[test]
fn test_viewdata_mixed_content() {
    let mut parser = ViewdataParser::new();
    let mut sink = TestSink::new();

    // Mix of text, cursor movement, and color changes
    parser.parse(b"HI\x09\x1BARED\x1E", &mut sink);

    // Should have: "HI" text, cursor forward, Red color, concealed off, "RED" text (3 chars), home
    assert!(sink.commands.iter().any(|c| c.contains("Text")));
    assert!(sink.commands.iter().any(|c| c == "CursorForward: 1"));
    assert!(sink.commands.iter().any(|c| c.contains("Foreground") && c.contains("Base(1)")));
    assert!(sink.commands.iter().any(|c| c == "CursorPosition: 1,1"));
}

#[test]
fn test_viewdata_control_code_handling() {
    let mut parser = ViewdataParser::new();
    let mut sink = TestSink::new();

    // Various control codes that should be ignored
    parser.parse(b"\x00\x01\x02\x03\x04TEST", &mut sink);

    // Should have text output
    assert!(sink.commands.iter().any(|c| c.contains("Text")));
}

#[test]
fn test_viewdata_esc_without_valid_command() {
    let mut parser = ViewdataParser::new();
    let mut sink = TestSink::new();

    // ESC with an undefined command character
    parser.parse(b"\x1B\x7F", &mut sink);

    // Should handle gracefully
    assert!(!sink.commands.is_empty());
}

#[test]
fn test_viewdata_state_reset_on_down() {
    let mut parser = ViewdataParser::new();
    let mut sink = TestSink::new();

    // Enter graphics mode, then cursor down should reset state
    parser.parse(b"\x1BQ", &mut sink); // Graphics Red
    sink.commands.clear();

    parser.parse(b"\x0A", &mut sink); // Cursor down (resets state)

    assert_eq!(sink.commands.len(), 1);
    assert_eq!(sink.commands[0], "CursorDown: 1");
}

#[test]
fn test_viewdata_all_alpha_colors() {
    let mut parser = ViewdataParser::new();
    let mut sink = TestSink::new();

    // Test all alpha colors A-G (Red, Green, Yellow, Blue, Magenta, Cyan, White)
    parser.parse(b"\x1BA\x1BB\x1BC\x1BD\x1BE\x1BF\x1BG", &mut sink);

    // Should have 7 color changes and 7 concealed off commands, plus 7 spaces
    let color_count = sink.commands.iter().filter(|c| c.contains("Foreground")).count();
    assert_eq!(color_count, 7);
}

#[test]
fn test_viewdata_all_graphics_colors() {
    let mut parser = ViewdataParser::new();
    let mut sink = TestSink::new();

    // Test all graphics colors Q-W
    parser.parse(b"\x1BQ\x1BR\x1BS\x1BT\x1BU\x1BV\x1BW", &mut sink);

    let color_count = sink.commands.iter().filter(|c| c.contains("Foreground")).count();
    assert_eq!(color_count, 7);
}
