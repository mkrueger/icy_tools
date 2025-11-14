use icy_parser_core::{CommandParser, CommandSink, RipCommand, RipParser, TerminalCommand};

struct TestSink {
    rip_commands: Vec<RipCommand>,
    terminal_commands: Vec<String>,
}

impl TestSink {
    fn new() -> Self {
        Self {
            rip_commands: Vec::new(),
            terminal_commands: Vec::new(),
        }
    }
}

impl CommandSink for TestSink {
    fn print(&mut self, text: &[u8]) {
        self.terminal_commands.push(format!("Text: {:?}", String::from_utf8_lossy(text)));
    }

    fn emit(&mut self, _cmd: TerminalCommand) {
        // No terminal commands expected in RIP tests
    }

    fn emit_rip(&mut self, cmd: RipCommand) {
        self.rip_commands.push(cmd);
    }
}

#[test]
fn test_rip_text_window() {
    let mut parser = RipParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"!|w00000J0Z01\n", &mut sink);

    assert_eq!(sink.rip_commands.len(), 1);
    match &sink.rip_commands[0] {
        RipCommand::TextWindow { x0, y0, x1, y1, wrap, size } => {
            assert_eq!(*x0, 0);
            assert_eq!(*y0, 0);
            assert_eq!(*x1, 19);
            assert_eq!(*y1, 35);
            assert_eq!(*wrap, false);
            assert_eq!(*size, 1);
        }
        _ => panic!("Expected TextWindow command"),
    }
}

#[test]
fn test_rip_viewport() {
    let mut parser = RipParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"!|v00000M09\n", &mut sink);

    assert_eq!(sink.rip_commands.len(), 1);
    match &sink.rip_commands[0] {
        RipCommand::ViewPort { x0, y0, x1, y1 } => {
            assert_eq!(*x0, 0);
            assert_eq!(*y0, 0);
            assert_eq!(*x1, 22);
            assert_eq!(*y1, 9);
        }
        _ => panic!("Expected ViewPort command"),
    }
}

#[test]
fn test_rip_reset_windows() {
    let mut parser = RipParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"!|*\n", &mut sink);

    assert_eq!(sink.rip_commands.len(), 1);
    assert!(matches!(sink.rip_commands[0], RipCommand::ResetWindows));
}

#[test]
fn test_rip_color() {
    let mut parser = RipParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"!|c0F\n", &mut sink);

    assert_eq!(sink.rip_commands.len(), 1);
    match &sink.rip_commands[0] {
        RipCommand::Color { c } => {
            assert_eq!(*c, 15);
        }
        _ => panic!("Expected Color command"),
    }
}

#[test]
fn test_rip_move() {
    let mut parser = RipParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"!|m0A0A\n", &mut sink);

    assert_eq!(sink.rip_commands.len(), 1);
    match &sink.rip_commands[0] {
        RipCommand::Move { x, y } => {
            assert_eq!(*x, 10);
            assert_eq!(*y, 10);
        }
        _ => panic!("Expected Move command"),
    }
}

#[test]
fn test_rip_line() {
    let mut parser = RipParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"!|L00001010\n", &mut sink);

    assert_eq!(sink.rip_commands.len(), 1);
    match &sink.rip_commands[0] {
        RipCommand::Line { x0, y0, x1, y1 } => {
            assert_eq!(*x0, 0);
            assert_eq!(*y0, 0);
            assert_eq!(*x1, 36);
            assert_eq!(*y1, 36);
        }
        _ => panic!("Expected Line command"),
    }
}

#[test]
fn test_rip_rectangle() {
    let mut parser = RipParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"!|R05051515\n", &mut sink);

    assert_eq!(sink.rip_commands.len(), 1);
    match &sink.rip_commands[0] {
        RipCommand::Rectangle { x0, y0, x1, y1 } => {
            assert_eq!(*x0, 5);
            assert_eq!(*y0, 5);
            assert_eq!(*x1, 41);
            assert_eq!(*y1, 41);
        }
        _ => panic!("Expected Rectangle command"),
    }
}

#[test]
fn test_rip_bar() {
    let mut parser = RipParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"!|B03030C0C\n", &mut sink);

    assert_eq!(sink.rip_commands.len(), 1);
    match &sink.rip_commands[0] {
        RipCommand::Bar { x0, y0, x1, y1 } => {
            assert_eq!(*x0, 3);
            assert_eq!(*y0, 3);
            assert_eq!(*x1, 12);
            assert_eq!(*y1, 12);
        }
        _ => panic!("Expected Bar command"),
    }
}

#[test]
fn test_rip_circle() {
    let mut parser = RipParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"!|C0A0A05\n", &mut sink);

    assert_eq!(sink.rip_commands.len(), 1);
    match &sink.rip_commands[0] {
        RipCommand::Circle { x_center, y_center, radius } => {
            assert_eq!(*x_center, 10);
            assert_eq!(*y_center, 10);
            assert_eq!(*radius, 5);
        }
        _ => panic!("Expected Circle command"),
    }
}

#[test]
fn test_rip_text() {
    let mut parser = RipParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"!|THello World\n", &mut sink);

    assert_eq!(sink.rip_commands.len(), 1);
    match &sink.rip_commands[0] {
        RipCommand::Text { text } => {
            assert_eq!(text, "Hello World");
        }
        _ => panic!("Expected Text command"),
    }
}

#[test]
fn test_rip_text_xy() {
    let mut parser = RipParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"!|@0505Test\n", &mut sink);

    assert_eq!(sink.rip_commands.len(), 1);
    match &sink.rip_commands[0] {
        RipCommand::TextXY { x, y, text } => {
            assert_eq!(*x, 5);
            assert_eq!(*y, 5);
            assert_eq!(text, "Test");
        }
        _ => panic!("Expected TextXY command"),
    }
}

#[test]
fn test_rip_polygon() {
    let mut parser = RipParser::new();
    let mut sink = TestSink::new();

    // 3 points: (0,0), (10,0), (5,10)
    parser.parse(b"!|P0300000A00050A\n", &mut sink);

    assert_eq!(sink.rip_commands.len(), 1);
    match &sink.rip_commands[0] {
        RipCommand::Polygon { points } => {
            assert_eq!(points.len(), 6);
            assert_eq!(points[0], 0); // x1
            assert_eq!(points[1], 0); // y1
            assert_eq!(points[2], 10); // x2
            assert_eq!(points[3], 0); // y2
            assert_eq!(points[4], 5); // x3
            assert_eq!(points[5], 10); // y3
        }
        _ => panic!("Expected Polygon command"),
    }
}

#[test]
fn test_rip_fill_style() {
    let mut parser = RipParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"!|S010F\n", &mut sink);

    assert_eq!(sink.rip_commands.len(), 1);
    match &sink.rip_commands[0] {
        RipCommand::FillStyle { pattern, color } => {
            assert_eq!(*pattern, 1);
            assert_eq!(*color, 15);
        }
        _ => panic!("Expected FillStyle command"),
    }
}

#[test]
fn test_rip_mouse() {
    let mut parser = RipParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"!|1M01000010001000000GO\n", &mut sink);

    assert_eq!(sink.rip_commands.len(), 1);
    match &sink.rip_commands[0] {
        RipCommand::Mouse {
            num,
            x0,
            y0,
            x1,
            y1,
            clk,
            clr,
            res,
            text,
        } => {
            assert_eq!(*num, 1);
            assert_eq!(*x0, 0);
            assert_eq!(*y0, 0);
            assert_eq!(*x1, 36);
            assert_eq!(*y1, 0);
            assert_eq!(*clk, 1);
            assert_eq!(*clr, 0);
            assert_eq!(*res, 0);
            assert_eq!(text, "GO");
        }
        _ => panic!("Expected Mouse command"),
    }
}

#[test]
fn test_rip_button() {
    let mut parser = RipParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"!|1U05051015001Enter<>cmd<>label<>text\n", &mut sink);

    assert_eq!(sink.rip_commands.len(), 1);
    match &sink.rip_commands[0] {
        RipCommand::Button {
            x0,
            y0,
            x1,
            y1,
            hotkey,
            flags,
            res,
            text,
        } => {
            assert_eq!(*x0, 5);
            assert_eq!(*y0, 5);
            assert_eq!(*x1, 36);
            assert_eq!(*y1, 41);
            assert_eq!(*hotkey, 0);
            assert_eq!(*flags, 1);
            assert_eq!(*res, 0);
            assert_eq!(text, "Enter<>cmd<>label<>text");
        }
        _ => panic!("Expected Button command"),
    }
}

#[test]
fn test_rip_load_icon() {
    let mut parser = RipParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"!|1I0A0A00000test.icn\n", &mut sink);

    assert_eq!(sink.rip_commands.len(), 1);
    match &sink.rip_commands[0] {
        RipCommand::LoadIcon {
            x,
            y,
            mode,
            clipboard,
            res,
            file_name,
        } => {
            assert_eq!(*x, 10);
            assert_eq!(*y, 10);
            assert_eq!(*mode, 0);
            assert_eq!(*clipboard, 0);
            assert_eq!(*res, 0);
            assert_eq!(file_name, "test.icn");
        }
        _ => panic!("Expected LoadIcon command"),
    }
}

#[test]
fn test_rip_bezier() {
    let mut parser = RipParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"!|Z0000050005100510000A\n", &mut sink);

    assert_eq!(sink.rip_commands.len(), 1);
    match &sink.rip_commands[0] {
        RipCommand::Bezier {
            x1,
            y1,
            x2,
            y2,
            x3,
            y3,
            x4,
            y4,
            cnt,
        } => {
            assert_eq!(*x1, 0);
            assert_eq!(*y1, 0);
            assert_eq!(*x2, 5);
            assert_eq!(*y2, 0);
            assert_eq!(*x3, 5);
            assert_eq!(*y3, 36);
            assert_eq!(*x4, 5);
            assert_eq!(*y4, 36);
            assert_eq!(*cnt, 10);
        }
        _ => panic!("Expected Bezier command"),
    }
}

#[test]
fn test_rip_no_more() {
    let mut parser = RipParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"!|#\n", &mut sink);

    assert_eq!(sink.rip_commands.len(), 1);
    assert!(matches!(sink.rip_commands[0], RipCommand::NoMore));
}

#[test]
fn test_rip_multiple_commands() {
    let mut parser = RipParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"!|c0F!|m0A0A!|L00001010\n", &mut sink);

    assert_eq!(sink.rip_commands.len(), 3);
    assert!(matches!(sink.rip_commands[0], RipCommand::Color { .. }));
    assert!(matches!(sink.rip_commands[1], RipCommand::Move { .. }));
    assert!(matches!(sink.rip_commands[2], RipCommand::Line { .. }));
}

#[test]
fn test_rip_passthrough_text() {
    let mut parser = RipParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"Regular text before !|c0F RIP command\n", &mut sink);

    // Should have text before and after RIP command
    assert!(!sink.terminal_commands.is_empty());
    assert_eq!(sink.rip_commands.len(), 1);
}

#[test]
fn test_rip_line_continuation() {
    let mut parser = RipParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"!|L0000\\\n1010\n", &mut sink);

    assert_eq!(sink.rip_commands.len(), 1);
    match &sink.rip_commands[0] {
        RipCommand::Line { x0, y0, x1, y1 } => {
            assert_eq!(*x0, 0);
            assert_eq!(*y0, 0);
            assert_eq!(*x1, 36);
            assert_eq!(*y1, 36);
        }
        _ => panic!("Expected Line command"),
    }
}

#[test]
fn test_rip_fill() {
    let mut parser = RipParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"!|F050507\n", &mut sink);

    assert_eq!(sink.rip_commands.len(), 1);
    match &sink.rip_commands[0] {
        RipCommand::Fill { x, y, border } => {
            assert_eq!(*x, 5);
            assert_eq!(*y, 5);
            assert_eq!(*border, 7);
        }
        _ => panic!("Expected Fill command"),
    }
}

#[test]
fn test_rip_arc() {
    let mut parser = RipParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"!|A0A0A00Z10Z05\n", &mut sink);

    assert_eq!(sink.rip_commands.len(), 1);
    match &sink.rip_commands[0] {
        RipCommand::Arc { x, y, st_ang, end_ang, radius } => {
            assert_eq!(*x, 10);
            assert_eq!(*y, 10);
            assert_eq!(*st_ang, 0);
            assert_eq!(*end_ang, 35);
            assert_eq!(*end_ang, 35 * 36 + 35); // ZZ in base36
        }
        _ => panic!("Expected Arc command"),
    }
}

#[test]
fn test_rip_write_mode() {
    let mut parser = RipParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"!|W01\n", &mut sink);

    assert_eq!(sink.rip_commands.len(), 1);
    match &sink.rip_commands[0] {
        RipCommand::WriteMode { mode } => {
            assert_eq!(*mode, 1);
        }
        _ => panic!("Expected WriteMode command"),
    }
}

#[test]
fn test_rip_pixel() {
    let mut parser = RipParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"!|X0F0F\n", &mut sink);

    assert_eq!(sink.rip_commands.len(), 1);
    match &sink.rip_commands[0] {
        RipCommand::Pixel { x, y } => {
            assert_eq!(*x, 15);
            assert_eq!(*y, 15);
        }
        _ => panic!("Expected Pixel command"),
    }
}

#[test]
fn test_rip_font_style() {
    let mut parser = RipParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"!|Y00010200\n", &mut sink);

    assert_eq!(sink.rip_commands.len(), 1);
    match &sink.rip_commands[0] {
        RipCommand::FontStyle { font, direction, size, res } => {
            assert_eq!(*font, 0);
            assert_eq!(*direction, 1);
            assert_eq!(*size, 2);
            assert_eq!(*res, 0);
        }
        _ => panic!("Expected FontStyle command"),
    }
}

#[test]
fn test_rip_oval() {
    let mut parser = RipParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"!|O0A0A00Z10Z0505\n", &mut sink);

    assert_eq!(sink.rip_commands.len(), 1);
    match &sink.rip_commands[0] {
        RipCommand::Oval {
            x,
            y,
            st_ang,
            end_ang,
            x_rad,
            y_rad,
        } => {
            assert_eq!(*x, 10);
            assert_eq!(*y, 10);
            assert_eq!(*st_ang, 0);
            assert_eq!(*end_ang, 35 * 36 + 35);
            assert_eq!(*x_rad, 5);
            assert_eq!(*y_rad, 5);
        }
        _ => panic!("Expected Oval command"),
    }
}

#[test]
fn test_rip_set_palette() {
    let mut parser = RipParser::new();
    let mut sink = TestSink::new();

    // 16 colors, 2 digits each = 32 digits
    parser.parse(b"!|Q000102030405060708090A0B0C0D0E0F\n", &mut sink);

    assert_eq!(sink.rip_commands.len(), 1);
    match &sink.rip_commands[0] {
        RipCommand::SetPalette { colors } => {
            assert_eq!(colors.len(), 16);
            assert_eq!(colors[0], 0);
            assert_eq!(colors[15], 15);
        }
        _ => panic!("Expected SetPalette command"),
    }
}

#[test]
fn test_rip_one_palette() {
    let mut parser = RipParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"!|a0520\n", &mut sink);

    assert_eq!(sink.rip_commands.len(), 1);
    match &sink.rip_commands[0] {
        RipCommand::OnePalette { color, value } => {
            assert_eq!(*color, 5);
            assert_eq!(*value, 72); // 20 in base36 = 2*36 + 0
        }
        _ => panic!("Expected OnePalette command"),
    }
}

#[test]
fn test_rip_home() {
    let mut parser = RipParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"!|H\n", &mut sink);

    assert_eq!(sink.rip_commands.len(), 1);
    assert!(matches!(sink.rip_commands[0], RipCommand::Home));
}

#[test]
fn test_rip_erase_eol() {
    let mut parser = RipParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"!|>\n", &mut sink);

    assert_eq!(sink.rip_commands.len(), 1);
    assert!(matches!(sink.rip_commands[0], RipCommand::EraseEOL));
}

#[test]
fn test_rip_erase_window() {
    let mut parser = RipParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"!|e\n", &mut sink);

    assert_eq!(sink.rip_commands.len(), 1);
    assert!(matches!(sink.rip_commands[0], RipCommand::EraseWindow));
}

#[test]
fn test_rip_erase_view() {
    let mut parser = RipParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"!|E\n", &mut sink);

    assert_eq!(sink.rip_commands.len(), 1);
    assert!(matches!(sink.rip_commands[0], RipCommand::EraseView));
}

#[test]
fn test_rip_goto_xy() {
    let mut parser = RipParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"!|g0C0D\n", &mut sink);

    assert_eq!(sink.rip_commands.len(), 1);
    match &sink.rip_commands[0] {
        RipCommand::GotoXY { x, y } => {
            assert_eq!(*x, 12);
            assert_eq!(*y, 13);
        }
        _ => panic!("Expected GotoXY command"),
    }
}
