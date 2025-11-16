/*
use icy_parser_core::{CommandParser, CommandSink, SkypixCommand, SkypixParser, TerminalCommand};

struct TestSink {
    skypix_commands: Vec<SkypixCommand>,
    ansi_commands: Vec<String>,
}

impl TestSink {
    fn new() -> Self {
        Self {
            skypix_commands: Vec::new(),
            ansi_commands: Vec::new(),
        }
    }
}

impl CommandSink for TestSink {
    fn print(&mut self, text: &[u8]) {
        self.ansi_commands.push(format!("Printable({:?})", String::from_utf8_lossy(text)));
    }

    fn emit(&mut self, cmd: TerminalCommand) {
        // Capture ANSI commands for testing
        self.ansi_commands.push(format!("{:?}", cmd));
    }

    fn emit_skypix(&mut self, cmd: SkypixCommand) {
        self.skypix_commands.push(cmd);
    }
}

#[test]
fn test_skypix_set_pixel() {
    let mut parser = SkypixParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1B[1;100;50!", &mut sink);

    assert_eq!(sink.skypix_commands.len(), 1);
    match &sink.skypix_commands[0] {
        SkypixCommand::SetPixel { x, y } => {
            assert_eq!(*x, 100);
            assert_eq!(*y, 50);
        }
        _ => panic!("Expected SetPixel command"),
    }
}

#[test]
fn test_skypix_draw_line() {
    let mut parser = SkypixParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1B[2;200;100!", &mut sink);

    assert_eq!(sink.skypix_commands.len(), 1);
    match &sink.skypix_commands[0] {
        SkypixCommand::DrawLine { x, y } => {
            assert_eq!(*x, 200);
            assert_eq!(*y, 100);
        }
        _ => panic!("Expected DrawLine command"),
    }
}

#[test]
fn test_skypix_area_fill() {
    let mut parser = SkypixParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1B[3;1;50;75!", &mut sink);

    assert_eq!(sink.skypix_commands.len(), 1);
    match &sink.skypix_commands[0] {
        SkypixCommand::AreaFill { mode, x, y } => {
            assert_eq!(*mode, 1);
            assert_eq!(*x, 50);
            assert_eq!(*y, 75);
        }
        _ => panic!("Expected AreaFill command"),
    }
}

#[test]
fn test_skypix_rectangle_fill() {
    let mut parser = SkypixParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1B[4;10;20;100;80!", &mut sink);

    assert_eq!(sink.skypix_commands.len(), 1);
    match &sink.skypix_commands[0] {
        SkypixCommand::RectangleFill { x1, y1, x2, y2 } => {
            assert_eq!(*x1, 10);
            assert_eq!(*y1, 20);
            assert_eq!(*x2, 100);
            assert_eq!(*y2, 80);
        }
        _ => panic!("Expected RectangleFill command"),
    }
}

#[test]
fn test_skypix_ellipse() {
    let mut parser = SkypixParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1B[5;100;50;40;30!", &mut sink);

    assert_eq!(sink.skypix_commands.len(), 1);
    match &sink.skypix_commands[0] {
        SkypixCommand::Ellipse { x, y, a, b } => {
            assert_eq!(*x, 100);
            assert_eq!(*y, 50);
            assert_eq!(*a, 40);
            assert_eq!(*b, 30);
        }
        _ => panic!("Expected Ellipse command"),
    }
}

#[test]
fn test_skypix_filled_ellipse() {
    let mut parser = SkypixParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1B[13;100;50;40;30!", &mut sink);

    assert_eq!(sink.skypix_commands.len(), 1);
    match &sink.skypix_commands[0] {
        SkypixCommand::FilledEllipse { x, y, a, b } => {
            assert_eq!(*x, 100);
            assert_eq!(*y, 50);
            assert_eq!(*a, 40);
            assert_eq!(*b, 30);
        }
        _ => panic!("Expected FilledEllipse command"),
    }
}

#[test]
fn test_skypix_grab_brush() {
    let mut parser = SkypixParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1B[6;10;20;50;40!", &mut sink);

    assert_eq!(sink.skypix_commands.len(), 1);
    match &sink.skypix_commands[0] {
        SkypixCommand::GrabBrush { x1, y1, width, height } => {
            assert_eq!(*x1, 10);
            assert_eq!(*y1, 20);
            assert_eq!(*width, 50);
            assert_eq!(*height, 40);
        }
        _ => panic!("Expected GrabBrush command"),
    }
}

#[test]
fn test_skypix_use_brush() {
    let mut parser = SkypixParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1B[7;0;0;100;50;32;16;192;255!", &mut sink);

    assert_eq!(sink.skypix_commands.len(), 1);
    match &sink.skypix_commands[0] {
        SkypixCommand::UseBrush {
            src_x,
            src_y,
            dst_x,
            dst_y,
            width,
            height,
            minterm,
            mask,
        } => {
            assert_eq!(*src_x, 0);
            assert_eq!(*src_y, 0);
            assert_eq!(*dst_x, 100);
            assert_eq!(*dst_y, 50);
            assert_eq!(*width, 32);
            assert_eq!(*height, 16);
            assert_eq!(*minterm, 192);
            assert_eq!(*mask, 255);
        }
        _ => panic!("Expected UseBrush command"),
    }
}

#[test]
fn test_skypix_move_pen() {
    let mut parser = SkypixParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1B[8;320;100!", &mut sink);

    assert_eq!(sink.skypix_commands.len(), 1);
    match &sink.skypix_commands[0] {
        SkypixCommand::MovePen { x, y } => {
            assert_eq!(*x, 320);
            assert_eq!(*y, 100);
        }
        _ => panic!("Expected MovePen command"),
    }
}

#[test]
fn test_skypix_set_pen_a() {
    let mut parser = SkypixParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1B[15;3!", &mut sink);

    assert_eq!(sink.skypix_commands.len(), 1);
    match &sink.skypix_commands[0] {
        SkypixCommand::SetPenA { color } => {
            assert_eq!(*color, 3);
        }
        _ => panic!("Expected SetPenA command"),
    }
}

#[test]
fn test_skypix_set_pen_b() {
    let mut parser = SkypixParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1B[18;1!", &mut sink);

    assert_eq!(sink.skypix_commands.len(), 1);
    match &sink.skypix_commands[0] {
        SkypixCommand::SetPenB { color } => {
            assert_eq!(*color, 1);
        }
        _ => panic!("Expected SetPenB command"),
    }
}

#[test]
fn test_skypix_delay() {
    let mut parser = SkypixParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1B[14;60!", &mut sink);

    assert_eq!(sink.skypix_commands.len(), 1);
    match &sink.skypix_commands[0] {
        SkypixCommand::Delay { jiffies } => {
            assert_eq!(*jiffies, 60);
        }
        _ => panic!("Expected Delay command"),
    }
}

#[test]
fn test_skypix_reset_palette() {
    let mut parser = SkypixParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1B[12!", &mut sink);

    assert_eq!(sink.skypix_commands.len(), 1);
    match &sink.skypix_commands[0] {
        SkypixCommand::ResetPalette => {}
        _ => panic!("Expected ResetPalette command"),
    }
}

#[test]
fn test_skypix_set_display_mode() {
    let mut parser = SkypixParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1B[17;2!", &mut sink);

    assert_eq!(sink.skypix_commands.len(), 1);
    match &sink.skypix_commands[0] {
        SkypixCommand::SetDisplayMode { mode } => {
            assert_eq!(*mode, 2);
        }
        _ => panic!("Expected SetDisplayMode command"),
    }
}

#[test]
fn test_skypix_position_cursor() {
    let mut parser = SkypixParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1B[19;320;100!", &mut sink);

    assert_eq!(sink.skypix_commands.len(), 1);
    match &sink.skypix_commands[0] {
        SkypixCommand::PositionCursor { x, y } => {
            assert_eq!(*x, 320);
            assert_eq!(*y, 100);
        }
        _ => panic!("Expected PositionCursor command"),
    }
}

#[test]
fn test_skypix_set_font() {
    let mut parser = SkypixParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1B[10;12!topaz.font!", &mut sink);

    assert_eq!(sink.skypix_commands.len(), 1);
    match &sink.skypix_commands[0] {
        SkypixCommand::SetFont { size, name } => {
            assert_eq!(*size, 12);
            assert_eq!(name, "topaz.font");
        }
        _ => panic!("Expected SetFont command"),
    }
}

#[test]
fn test_skypix_new_palette() {
    let mut parser = SkypixParser::new();
    let mut sink = TestSink::new();

    // 16 color values
    parser.parse(b"\x1B[11;0;287;3549;3840;241;943;4082;3086;182;221;175;124;15;1807;3086;3080!", &mut sink);

    assert_eq!(sink.skypix_commands.len(), 1);
    match &sink.skypix_commands[0] {
        SkypixCommand::NewPalette { colors } => {
            assert_eq!(colors.len(), 16);
            assert_eq!(colors[0], 0);
            assert_eq!(colors[1], 287);
            assert_eq!(colors[15], 3080);
        }
        _ => panic!("Expected NewPalette command"),
    }
}

#[test]
fn test_ansi_cursor_up() {
    let mut parser = SkypixParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1B[5A", &mut sink);

    assert!(sink.ansi_commands.len() > 0);
    assert!(sink.ansi_commands[0].contains("CursorUp"));
}

#[test]
fn test_ansi_cursor_position() {
    let mut parser = SkypixParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1B[10;20H", &mut sink);

    assert!(sink.ansi_commands.len() > 0);
    assert!(sink.ansi_commands[0].contains("CursorPosition"));
}

#[test]
fn test_ansi_color_change() {
    let mut parser = SkypixParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1B[31m", &mut sink);

    assert!(sink.ansi_commands.len() > 0);
    assert!(sink.ansi_commands[0].contains("Foreground"));
}

#[test]
fn test_ansi_erase_display() {
    let mut parser = SkypixParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1B[2J", &mut sink);

    assert!(sink.ansi_commands.len() > 0);
    assert!(sink.ansi_commands[0].contains("EraseInDisplay"));
}

#[test]
fn test_mixed_ansi_and_skypix() {
    let mut parser = SkypixParser::new();
    let mut sink = TestSink::new();

    // Color change + SkyPix command + more text
    parser.parse(b"\x1B[31mRed text\x1B[15;3!More text", &mut sink);

    assert!(sink.ansi_commands.len() > 0);
    assert_eq!(sink.skypix_commands.len(), 1);
    match &sink.skypix_commands[0] {
        SkypixCommand::SetPenA { color } => {
            assert_eq!(*color, 3);
        }
        _ => panic!("Expected SetPenA command"),
    }
}

#[test]
fn test_multiple_skypix_commands() {
    let mut parser = SkypixParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1B[15;5!\x1B[8;100;50!\x1B[2;200;100!", &mut sink);

    assert_eq!(sink.skypix_commands.len(), 3);
    assert!(matches!(sink.skypix_commands[0], SkypixCommand::SetPenA { .. }));
    assert!(matches!(sink.skypix_commands[1], SkypixCommand::MovePen { .. }));
    assert!(matches!(sink.skypix_commands[2], SkypixCommand::DrawLine { .. }));
}

#[test]
fn test_regular_text_passthrough() {
    let mut parser = SkypixParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"Hello, World!", &mut sink);

    // Should have emitted printable characters
    assert!(sink.ansi_commands.len() > 0);
}
*/
