use icy_parser_core::{Color, CommandParser, CommandSink, RenegadeParser, SgrAttribute, TerminalCommand};

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

    fn report_error(&mut self, _error: icy_parser_core::ParseError, _level: icy_parser_core::ErrorLevel) {}
}

#[test]
fn test_renegade_simple_text() {
    let mut parser = RenegadeParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"Hello World", &mut sink);

    assert_eq!(sink.commands.len(), 1);
    assert_eq!(sink.commands[0], "Text: \"Hello World\"");
}

#[test]
fn test_renegade_foreground_colors() {
    let mut parser = RenegadeParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"|00Black|04Red|15White", &mut sink);

    assert_eq!(sink.commands.len(), 6);
    assert_eq!(sink.commands[0], "FG: 0"); // |00 = Black
    assert_eq!(sink.commands[1], "Text: \"Black\"");
    assert_eq!(sink.commands[2], "FG: 4"); // |04 = Red
    assert_eq!(sink.commands[3], "Text: \"Red\"");
    assert_eq!(sink.commands[4], "FG: 15"); // |15 = White
    assert_eq!(sink.commands[5], "Text: \"White\"");
}

#[test]
fn test_renegade_background_colors() {
    let mut parser = RenegadeParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"|16Black|17Blue|23White", &mut sink);

    assert_eq!(sink.commands.len(), 6);
    assert_eq!(sink.commands[0], "BG: 0"); // |16 = Black BG
    assert_eq!(sink.commands[1], "Text: \"Black\"");
    assert_eq!(sink.commands[2], "BG: 1"); // |17 = Blue BG
    assert_eq!(sink.commands[3], "Text: \"Blue\"");
    assert_eq!(sink.commands[4], "BG: 7"); // |23 = White BG
    assert_eq!(sink.commands[5], "Text: \"White\"");
}

#[test]
fn test_renegade_mixed_colors() {
    let mut parser = RenegadeParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"|15|16Test", &mut sink);

    assert_eq!(sink.commands.len(), 3);
    assert_eq!(sink.commands[0], "FG: 15"); // White foreground
    assert_eq!(sink.commands[1], "BG: 0"); // Black background
    assert_eq!(sink.commands[2], "Text: \"Test\"");
}

#[test]
fn test_renegade_invalid_codes() {
    let mut parser = RenegadeParser::new();
    let mut sink = TestSink::new();

    // |24 and above are invalid (only 0-23 valid)
    parser.parse(b"|24Test", &mut sink);

    assert_eq!(sink.commands.len(), 2);
    assert_eq!(sink.commands[0], "Text: \"|24\"");
    assert_eq!(sink.commands[1], "Text: \"Test\"");
}

#[test]
fn test_renegade_incomplete_sequence() {
    let mut parser = RenegadeParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"|1Test", &mut sink);

    assert_eq!(sink.commands.len(), 2);
    assert_eq!(sink.commands[0], "Text: \"|1\"");
    assert_eq!(sink.commands[1], "Text: \"Test\"");
}

#[test]
fn test_renegade_literal_pipe() {
    let mut parser = RenegadeParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"|Hello", &mut sink);

    assert_eq!(sink.commands.len(), 2);
    assert_eq!(sink.commands[0], "Text: \"|\"");
    assert_eq!(sink.commands[1], "Text: \"Hello\"");
}

#[test]
fn test_renegade_all_16_foreground() {
    let mut parser = RenegadeParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"|00|01|02|03|04|05|06|07|08|09|10|11|12|13|14|15", &mut sink);

    assert_eq!(sink.commands.len(), 16);
    for i in 0..16 {
        assert_eq!(sink.commands[i], format!("FG: {}", i));
    }
}

#[test]
fn test_renegade_all_8_background() {
    let mut parser = RenegadeParser::new();
    let mut sink = TestSink::new();

    parser.parse(b"|16|17|18|19|20|21|22|23", &mut sink);

    assert_eq!(sink.commands.len(), 8);
    for i in 0..8 {
        assert_eq!(sink.commands[i], format!("BG: {}", i));
    }
}
