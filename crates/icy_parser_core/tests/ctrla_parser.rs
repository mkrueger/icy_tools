use icy_parser_core::{CommandParser, CommandSink, CtrlAParser, DecMode, TerminalCommand};

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
            TerminalCommand::CsiSelectGraphicRendition(icy_parser_core::SgrAttribute::Foreground(icy_parser_core::Color::Base(c))) => {
                self.commands.push(format!("FG: {}", c));
            }
            TerminalCommand::CsiSelectGraphicRendition(icy_parser_core::SgrAttribute::Background(icy_parser_core::Color::Base(c))) => {
                self.commands.push(format!("BG: {}", c));
            }
            TerminalCommand::CsiSelectGraphicRendition(icy_parser_core::SgrAttribute::Reset) => {
                self.commands.push("Reset".to_string());
            }
            TerminalCommand::CsiDecSetMode(DecMode::IceColors, true) => {
                self.commands.push("IceColors: On".to_string());
            }
            TerminalCommand::CsiDecSetMode(DecMode::IceColors, false) => {
                self.commands.push("IceColors: Off".to_string());
            }
            _ => {
                self.commands.push(format!("Other: {:?}", cmd));
            }
        }
    }

    fn report_error(&mut self, _error: icy_parser_core::ParseError, _level: icy_parser_core::ErrorLevel) {}
}

#[test]
fn test_ctrla_simple_text() {
    let mut parser = CtrlAParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"Hello World", &mut sink);

    assert_eq!(sink.commands.len(), 1);
    assert_eq!(sink.commands[0], "Text: \"Hello World\"");
}

#[test]
fn test_ctrla_foreground_colors() {
    let mut parser = CtrlAParser::new();
    let mut sink = TestSink::new();

    // ^AK = Black, ^AR = Red, ^AW = White
    parser.parse(b"\x01KBlack\x01RRed\x01WWhite", &mut sink);

    assert_eq!(sink.commands.len(), 6);
    assert_eq!(sink.commands[0], "FG: 0"); // K = Black
    assert_eq!(sink.commands[1], "Text: \"Black\"");
    assert_eq!(sink.commands[2], "FG: 4"); // R = Red
    assert_eq!(sink.commands[3], "Text: \"Red\"");
    assert_eq!(sink.commands[4], "FG: 7"); // W = White
    assert_eq!(sink.commands[5], "Text: \"White\"");
}

#[test]
fn test_ctrla_background_colors() {
    let mut parser = CtrlAParser::new();
    let mut sink = TestSink::new();

    // ^A0 = Black BG, ^A4 = Blue BG
    parser.parse(b"\x010Test\x014More", &mut sink);

    assert_eq!(sink.commands.len(), 4);
    assert_eq!(sink.commands[0], "BG: 0"); // 0 = Black BG
    assert_eq!(sink.commands[1], "Text: \"Test\"");
    assert_eq!(sink.commands[2], "BG: 1"); // 4 = Blue BG (maps to index 1)
    assert_eq!(sink.commands[3], "Text: \"More\"");
}

#[test]
fn test_ctrla_bold() {
    let mut parser = CtrlAParser::new();
    let mut sink = TestSink::new();

    // ^AH = bold on, ^AK with bold = bright black (8)
    parser.parse(b"\x01H\x01KTest", &mut sink);

    assert_eq!(sink.commands.len(), 2);
    assert_eq!(sink.commands[0], "FG: 8"); // K + bold = 8
    assert_eq!(sink.commands[1], "Text: \"Test\"");
}

#[test]
fn test_ctrla_clear_screen() {
    let mut parser = CtrlAParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x01L", &mut sink);

    assert_eq!(sink.commands.len(), 1);
    assert!(sink.commands[0].contains("EraseInDisplay"));
}

#[test]
fn test_ctrla_literal() {
    let mut parser = CtrlAParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x01A", &mut sink);

    assert_eq!(sink.commands.len(), 1);
    assert_eq!(sink.commands[0], "Text: \"\\u{1}\"");
}

#[test]
fn test_ctrla_normal_reset() {
    let mut parser = CtrlAParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"\x01H\x01N", &mut sink);

    assert_eq!(sink.commands.len(), 2);
    assert_eq!(sink.commands[0], "IceColors: Off");
    assert_eq!(sink.commands[1], "Reset");
}
