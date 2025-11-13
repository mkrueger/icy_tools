use icy_parser_core::{CommandParser, CommandSink, Mode7Parser, TerminalCommand};

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
            TerminalCommand::Bell => {
                self.commands.push("Bell".to_string());
            }
            TerminalCommand::Backspace => {
                self.commands.push("Backspace".to_string());
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
fn test_mode7_simple_text() {
    let mut parser = Mode7Parser::new();
    let mut sink = TestSink::new();

    parser.parse(b"HELLO", &mut sink);

    assert_eq!(sink.commands.len(), 5);
    assert!(sink.commands.iter().all(|c| c.starts_with("Text")));
}

#[test]
fn test_mode7_cursor_movement() {
    let mut parser = Mode7Parser::new();
    let mut sink = TestSink::new();

    // Left (8), right (9), down (10), up (11)
    parser.parse(b"\x08\x09\x0A\x0B", &mut sink);

    assert!(sink.commands.iter().any(|c| c == "CursorBack: 1"));
    assert!(sink.commands.iter().any(|c| c == "CursorForward: 1"));
    assert!(sink.commands.iter().any(|c| c == "CursorDown: 1"));
    assert!(sink.commands.iter().any(|c| c == "CursorUp: 1"));
}

#[test]
fn test_mode7_clear_screen() {
    let mut parser = Mode7Parser::new();
    let mut sink = TestSink::new();

    // VDU 12 - clear screen
    parser.parse(b"\x0C", &mut sink);

    assert!(sink.commands.iter().any(|c| c.contains("EraseInDisplay")));
    assert!(sink.commands.iter().any(|c| c == "CursorPosition: 1,1"));
}

#[test]
fn test_mode7_home() {
    let mut parser = Mode7Parser::new();
    let mut sink = TestSink::new();

    // VDU 30 - home cursor
    parser.parse(b"\x1E", &mut sink);

    assert!(sink.commands.iter().any(|c| c == "CursorPosition: 1,1"));
}

#[test]
fn test_mode7_carriage_return() {
    let mut parser = Mode7Parser::new();
    let mut sink = TestSink::new();

    // VDU 13 - carriage return
    parser.parse(b"\x0D", &mut sink);

    assert!(sink.commands.iter().any(|c| c == "CarriageReturn"));
}

#[test]
fn test_mode7_bell() {
    let mut parser = Mode7Parser::new();
    let mut sink = TestSink::new();

    // VDU 7 - bell
    parser.parse(b"\x07", &mut sink);

    assert_eq!(sink.commands.len(), 1);
    assert_eq!(sink.commands[0], "Bell");
}

#[test]
fn test_mode7_destructive_backspace() {
    let mut parser = Mode7Parser::new();
    let mut sink = TestSink::new();

    // VDU 127 - destructive backspace
    parser.parse(b"\x7F", &mut sink);

    // Should emit: backspace, space, backspace
    assert_eq!(sink.commands.len(), 3);
    assert_eq!(sink.commands[0], "Backspace");
    assert!(sink.commands[1].contains("Text"));
    assert_eq!(sink.commands[2], "Backspace");
}

#[test]
fn test_mode7_alpha_colors() {
    let mut parser = Mode7Parser::new();
    let mut sink = TestSink::new();

    // Alpha colors: 129-135 (Red, Green, Yellow, Blue, Magenta, Cyan, White)
    parser.parse(b"\x81\x82\x83", &mut sink);

    // Should have color changes and spaces
    let color_count = sink.commands.iter().filter(|c| c.contains("Foreground")).count();
    assert!(color_count >= 3);
}

#[test]
fn test_mode7_graphics_colors() {
    let mut parser = Mode7Parser::new();
    let mut sink = TestSink::new();

    // Graphics colors: 145-151 (Red, Green, Yellow, Blue, Magenta, Cyan, White)
    parser.parse(b"\x91\x92\x93", &mut sink);

    let color_count = sink.commands.iter().filter(|c| c.contains("Foreground")).count();
    assert!(color_count >= 3);
}

#[test]
fn test_mode7_flash_steady() {
    let mut parser = Mode7Parser::new();
    let mut sink = TestSink::new();

    // 136 = flash, 137 = steady
    parser.parse(b"\x88\x89", &mut sink);

    assert!(sink.commands.iter().any(|c| c.contains("Blink(Slow)")));
    assert!(sink.commands.iter().any(|c| c.contains("Blink(Off)")));
}

#[test]
fn test_mode7_concealed() {
    let mut parser = Mode7Parser::new();
    let mut sink = TestSink::new();

    // 152 = conceal
    parser.parse(b"\x98", &mut sink);

    assert!(sink.commands.iter().any(|c| c.contains("Concealed(true)")));
}

#[test]
fn test_mode7_contiguous_separated() {
    let mut parser = Mode7Parser::new();
    let mut sink = TestSink::new();

    // 153 = contiguous, 154 = separated
    parser.parse(b"\x99\x9A", &mut sink);

    // These affect internal state but still emit spaces
    assert!(sink.commands.iter().any(|c| c.contains("Text")));
}

#[test]
fn test_mode7_hold_release_graphics() {
    let mut parser = Mode7Parser::new();
    let mut sink = TestSink::new();

    // 158 = hold graphics, 159 = release graphics
    parser.parse(b"\x9E\x9F", &mut sink);

    assert!(sink.commands.iter().any(|c| c.contains("Text")));
}

#[test]
fn test_mode7_black_background() {
    let mut parser = Mode7Parser::new();
    let mut sink = TestSink::new();

    // 156 = black background
    parser.parse(b"\x9C", &mut sink);

    assert!(sink.commands.iter().any(|c| c.contains("Background") && c.contains("Base(0)")));
}

#[test]
fn test_mode7_new_background() {
    let mut parser = Mode7Parser::new();
    let mut sink = TestSink::new();

    // Set foreground first, then use as background (157)
    parser.parse(b"\x81\x9D", &mut sink);

    // Should have foreground and background changes
    assert!(sink.commands.iter().any(|c| c.contains("Foreground")));
    assert!(sink.commands.iter().any(|c| c.contains("Background")));
}

#[test]
fn test_mode7_vdu_colour() {
    let mut parser = Mode7Parser::new();
    let mut sink = TestSink::new();

    // VDU 17,n - COLOUR n (17 + color value)
    parser.parse(b"\x11\x03", &mut sink);

    assert!(sink.commands.iter().any(|c| c.contains("Foreground")));
}

#[test]
fn test_mode7_vdu_tab() {
    let mut parser = Mode7Parser::new();
    let mut sink = TestSink::new();

    // VDU 31,x,y - TAB(x,y) (31 + x + y)
    parser.parse(b"\x1F\x05\x0A", &mut sink);

    // Should position cursor at (5, 10) -> (row 11, col 6)
    assert!(sink.commands.iter().any(|c| c.contains("CursorPosition")));
}

#[test]
fn test_mode7_vdu_mode() {
    let mut parser = Mode7Parser::new();
    let mut sink = TestSink::new();

    // VDU 22,n - MODE n
    parser.parse(b"\x16\x07", &mut sink);

    // Should clear screen and reset
    assert!(sink.commands.iter().any(|c| c.contains("EraseInDisplay")));
    assert!(sink.commands.iter().any(|c| c.contains("CursorPosition")));
}

#[test]
fn test_mode7_default_colors() {
    let mut parser = Mode7Parser::new();
    let mut sink = TestSink::new();

    // VDU 20 - restore default colors
    parser.parse(b"\x14", &mut sink);

    // Should set white foreground and black background
    assert!(sink.commands.iter().any(|c| c.contains("Foreground") && c.contains("Base(7)")));
    assert!(sink.commands.iter().any(|c| c.contains("Background") && c.contains("Base(0)")));
}

#[test]
fn test_mode7_esc_sequence() {
    let mut parser = Mode7Parser::new();
    let mut sink = TestSink::new();

    // VDU 27 (ESC) - next char goes directly to screen
    parser.parse(b"\x1BX", &mut sink);

    assert_eq!(sink.commands.len(), 1);
    assert!(sink.commands[0].contains("Text"));
}

#[test]
fn test_mode7_vdu_disable_enable() {
    let mut parser = Mode7Parser::new();
    let mut sink = TestSink::new();

    // VDU 21 - disable output
    parser.parse(b"\x15ABC", &mut sink);
    sink.commands.clear();

    // VDU 6 - enable output
    parser.parse(b"\x06DEF", &mut sink);

    // Should only see DEF
    let text_count = sink.commands.iter().filter(|c| c.contains("Text")).count();
    assert_eq!(text_count, 3);
}

#[test]
fn test_mode7_graphics_characters() {
    let mut parser = Mode7Parser::new();
    let mut sink = TestSink::new();

    // Enter graphics mode, then send graphics character
    parser.parse(b"\x91\xA0", &mut sink);

    // Should have color change and graphics character
    assert!(sink.commands.iter().any(|c| c.contains("Foreground")));
    assert!(sink.commands.iter().any(|c| c.contains("Text")));
}

#[test]
fn test_mode7_mixed_content() {
    let mut parser = Mode7Parser::new();
    let mut sink = TestSink::new();

    // Mix of text, cursor movement, and colors
    parser.parse(b"HI\x09\x81RED\x1E", &mut sink);

    assert!(sink.commands.iter().any(|c| c.contains("Text")));
    assert!(sink.commands.iter().any(|c| c == "CursorForward: 1"));
    assert!(sink.commands.iter().any(|c| c.contains("Foreground")));
    assert!(sink.commands.iter().any(|c| c == "CursorPosition: 1,1"));
}

#[test]
fn test_mode7_state_reset_on_cr() {
    let mut parser = Mode7Parser::new();
    let mut sink = TestSink::new();

    // Enter graphics mode, then CR should reset
    parser.parse(b"\x91", &mut sink);
    sink.commands.clear();

    parser.parse(b"\x0D", &mut sink);

    // Should see CR
    assert!(sink.commands.iter().any(|c| c == "CarriageReturn"));
}

#[test]
fn test_mode7_all_alpha_colors() {
    let mut parser = Mode7Parser::new();
    let mut sink = TestSink::new();

    // Test all alpha colors 129-135
    parser.parse(b"\x81\x82\x83\x84\x85\x86\x87", &mut sink);

    let color_count = sink.commands.iter().filter(|c| c.contains("Foreground")).count();
    assert_eq!(color_count, 7);
}

#[test]
fn test_mode7_all_graphics_colors() {
    let mut parser = Mode7Parser::new();
    let mut sink = TestSink::new();

    // Test all graphics colors 145-151
    parser.parse(b"\x91\x92\x93\x94\x95\x96\x97", &mut sink);

    let color_count = sink.commands.iter().filter(|c| c.contains("Foreground")).count();
    assert_eq!(color_count, 7);
}
