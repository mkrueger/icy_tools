use icy_parser_core::{CommandParser, CommandSink, IgsCommand, IgsParser, TerminalCommand};

mod load;

mod roundtrip;

struct TestSink {
    pub igs_commands: Vec<IgsCommand>,
    text: Vec<String>,
    terminal_commands: Vec<TerminalCommand>,
}

impl TestSink {
    fn new() -> Self {
        Self {
            igs_commands: Vec::new(),
            text: Vec::new(),
            terminal_commands: Vec::new(),
        }
    }
}

impl CommandSink for TestSink {
    fn print(&mut self, text: &[u8]) {
        self.text.push(String::from_utf8_lossy(text).to_string());
    }

    fn emit(&mut self, cmd: TerminalCommand) {
        self.terminal_commands.push(cmd);
    }

    fn emit_igs(&mut self, cmd: IgsCommand) {
        self.igs_commands.push(cmd);
    }
}

#[test]
fn test_igs_box_command() {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    // G#B10,20,100,200,0:
    parser.parse(b"G#B10,20,100,200,0:", &mut sink);

    assert_eq!(sink.igs_commands.len(), 1);
    match &sink.igs_commands[0] {
        IgsCommand::Box { x1, y1, x2, y2, rounded } => {
            assert_eq!(*x1, 10);
            assert_eq!(*y1, 20);
            assert_eq!(*x2, 100);
            assert_eq!(*y2, 200);
            assert_eq!(*rounded, false);
        }
        _ => panic!("Expected Box command"),
    }
}

#[test]
fn test_igs_line_command() {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    // G#L0,0,100,100:
    parser.parse(b"G#L0,0,100,100:", &mut sink);

    assert_eq!(sink.igs_commands.len(), 1);
    match &sink.igs_commands[0] {
        IgsCommand::Line { x1, y1, x2, y2 } => {
            assert_eq!(*x1, 0);
            assert_eq!(*y1, 0);
            assert_eq!(*x2, 100);
            assert_eq!(*y2, 100);
        }
        _ => panic!("Expected Line command"),
    }
}

#[test]
fn test_igs_circle_command() {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    // G#O50,50,25:
    parser.parse(b"G#O50,50,25:", &mut sink);

    assert_eq!(sink.igs_commands.len(), 1);
    match &sink.igs_commands[0] {
        IgsCommand::Circle { x, y, radius } => {
            assert_eq!(*x, 50);
            assert_eq!(*y, 50);
            assert_eq!(*radius, 25);
        }
        _ => panic!("Expected Circle command"),
    }
}

#[test]
fn test_igs_color_set() {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    // G#C1,15:
    parser.parse(b"G#C1,15:", &mut sink);

    assert_eq!(sink.igs_commands.len(), 1);
    match &sink.igs_commands[0] {
        IgsCommand::ColorSet { pen, color } => {
            assert_eq!(*pen, 1);
            assert_eq!(*color, 15);
        }
        _ => panic!("Expected ColorSet command"),
    }
}

#[test]
fn test_igs_write_text() {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    // G#W10,20,0,Hello World@
    parser.parse(b"G#W10,20,0,Hello World@", &mut sink);

    assert_eq!(sink.igs_commands.len(), 1);
    match &sink.igs_commands[0] {
        IgsCommand::WriteText { x, y, text } => {
            assert_eq!(*x, 10);
            assert_eq!(*y, 20);
            assert_eq!(text, "Hello World");
        }
        _ => panic!("Expected WriteText command"),
    }
}

#[test]
fn test_igs_multiple_commands() {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    // Multiple commands in one sequence
    parser.parse(b"G#C1,15:B10,20,100,200,0:L0,0,50,50:", &mut sink);

    assert_eq!(sink.igs_commands.len(), 3);
    assert!(matches!(sink.igs_commands[0], IgsCommand::ColorSet { .. }));
    assert!(matches!(sink.igs_commands[1], IgsCommand::Box { .. }));
    assert!(matches!(sink.igs_commands[2], IgsCommand::Line { .. }));
}

#[test]
fn test_igs_vt52_cursor_up() {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1BA", &mut sink);

    assert_eq!(sink.igs_commands.len(), 1);
    assert!(matches!(sink.igs_commands[0], IgsCommand::CursorUp));
}

#[test]
fn test_igs_vt52_set_cursor_pos() {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    // ESC Y {row+32} {col+32}
    // Set cursor to (5, 10): ESC Y space+10 space+5
    parser.parse(b"\x1BY*%", &mut sink);

    assert_eq!(sink.igs_commands.len(), 1);
    match &sink.igs_commands[0] {
        IgsCommand::SetCursorPos { x, y } => {
            assert_eq!(*x, 10);
            assert_eq!(*y, 5);
        }
        _ => panic!("Expected SetCursorPos command"),
    }
}

#[test]
fn test_igs_vt52_set_foreground_color() {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x1Bb7", &mut sink);

    assert_eq!(sink.igs_commands.len(), 1);
    match &sink.igs_commands[0] {
        IgsCommand::SetForeground { color } => {
            assert_eq!(*color, 7);
        }
        _ => panic!("Expected SetForeground command"),
    }
}

#[test]
fn test_igs_passthrough_text() {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"Regular text before G#C1,15: and after", &mut sink);

    assert_eq!(sink.igs_commands.len(), 1);
    assert!(sink.text.len() > 0);
    assert!(sink.text.join("").contains("Regular text"));
}

#[test]
fn test_igs_flood_fill() {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"G#F100,100:", &mut sink);

    assert_eq!(sink.igs_commands.len(), 1);
    match &sink.igs_commands[0] {
        IgsCommand::FloodFill { x, y } => {
            assert_eq!(*x, 100);
            assert_eq!(*y, 100);
        }
        _ => panic!("Expected FloodFill command"),
    }
}

#[test]
fn test_igs_ellipse() {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    // Q command is Ellipse
    parser.parse(b"G#Q100,100,50,30:", &mut sink);

    assert_eq!(sink.igs_commands.len(), 1);
    match &sink.igs_commands[0] {
        IgsCommand::Ellipse { x, y, x_radius, y_radius } => {
            assert_eq!(*x, 100);
            assert_eq!(*y, 100);
            assert_eq!(*x_radius, 50);
            assert_eq!(*y_radius, 30);
        }
        _ => panic!("Expected Ellipse command"),
    }
}

#[test]
fn test_igs_arc() {
    let mut parser = IgsParser::new();
    let mut sink = TestSink::new();

    // K command: x, y, radius, start_angle, end_angle
    parser.parse(b"G#K100,100,50,0,90:", &mut sink);

    assert_eq!(sink.igs_commands.len(), 1);
    match &sink.igs_commands[0] {
        IgsCommand::Arc {
            x,
            y,
            start_angle,
            end_angle,
            radius,
        } => {
            assert_eq!(*x, 100);
            assert_eq!(*y, 100);
            assert_eq!(*radius, 50);
            assert_eq!(*start_angle, 0);
            assert_eq!(*end_angle, 90);
        }
        _ => panic!("Expected Arc command"),
    }
}
