use icy_parser_core::{Color, CommandParser, CommandSink, PcBoardParser, SgrAttribute, TerminalCommand};

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
            TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Foreground(Color::Base(c))) => {
                self.commands.push(format!("FG: {}", c));
            }
            TerminalCommand::CsiSelectGraphicRendition(SgrAttribute::Background(Color::Base(c))) => {
                self.commands.push(format!("BG: {}", c));
            }
            _ => {
                self.commands.push(format!("Other: {:?}", cmd));
            }
        }
    }

    fn report_errror(&mut self, _error: icy_parser_core::ParseError, _level: icy_parser_core::ErrorLevel) {}
}

#[test]
fn test_pcboard_simple_text() {
    let mut parser = PcBoardParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"Hello World", &mut sink);

    assert_eq!(sink.commands.len(), 1);
    assert_eq!(sink.commands[0], "Text: \"Hello World\"");
}

#[test]
fn test_pcboard_color_code() {
    let mut parser = PcBoardParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"@X0FHello", &mut sink);

    assert_eq!(sink.commands.len(), 3);
    assert_eq!(sink.commands[0], "FG: 15"); // 0x0F = 15
    assert_eq!(sink.commands[1], "BG: 0");
    assert_eq!(sink.commands[2], "Text: \"Hello\"");
}

#[test]
fn test_pcboard_escaped_at() {
    let mut parser = PcBoardParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"@@Test", &mut sink);

    assert_eq!(sink.commands.len(), 2);
    assert_eq!(sink.commands[0], "Text: \"@\"");
    assert_eq!(sink.commands[1], "Text: \"Test\"");
}

#[test]
fn test_pcboard_multiple_colors() {
    let mut parser = PcBoardParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"@X0FRed@X1FGreen@X2FBlue", &mut sink);

    assert_eq!(sink.commands.len(), 9);
    assert_eq!(sink.commands[0], "FG: 15");
    assert_eq!(sink.commands[1], "BG: 0");
    assert_eq!(sink.commands[2], "Text: \"Red\"");
    assert_eq!(sink.commands[3], "FG: 15");
    assert_eq!(sink.commands[4], "BG: 1");
    assert_eq!(sink.commands[5], "Text: \"Green\"");
    assert_eq!(sink.commands[6], "FG: 15");
    assert_eq!(sink.commands[7], "BG: 2");
    assert_eq!(sink.commands[8], "Text: \"Blue\"");
}

#[test]
fn test_pcboard_lowercase_x() {
    let mut parser = PcBoardParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"@x0fTest", &mut sink);

    assert_eq!(sink.commands.len(), 3);
    assert_eq!(sink.commands[0], "FG: 15");
    assert_eq!(sink.commands[1], "BG: 0");
    assert_eq!(sink.commands[2], "Text: \"Test\"");
}

#[test]
fn test_pcboard_macro_ignored() {
    let mut parser = PcBoardParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"@CLS@Hello", &mut sink);

    // Macros are currently ignored
    assert_eq!(sink.commands.len(), 1);
    assert_eq!(sink.commands[0], "Text: \"Hello\"");
}
