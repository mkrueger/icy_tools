use icy_parser_core::{AsciiParser, CommandParser, CommandSink, TerminalCommand};

struct CollectSink {
    pub text: Vec<u8>,
    pub cmds: Vec<TerminalCommand>,
}
impl CollectSink {
    fn new() -> Self {
        Self {
            text: Vec::new(),
            cmds: Vec::new(),
        }
    }
}

impl CommandSink for CollectSink {
    fn print(&mut self, text: &[u8]) {
        self.text.extend_from_slice(text);
    }

    fn emit(&mut self, cmd: TerminalCommand) {
        match cmd {
            TerminalCommand::CarriageReturn => self.cmds.push(TerminalCommand::CarriageReturn),
            TerminalCommand::LineFeed => self.cmds.push(TerminalCommand::LineFeed),
            TerminalCommand::Backspace => self.cmds.push(TerminalCommand::Backspace),
            TerminalCommand::Tab => self.cmds.push(TerminalCommand::Tab),
            TerminalCommand::FormFeed => self.cmds.push(TerminalCommand::FormFeed),
            TerminalCommand::Bell => self.cmds.push(TerminalCommand::Bell),
            TerminalCommand::Delete => self.cmds.push(TerminalCommand::Delete),
            // ASCII parser won't emit any ANSI commands, but we need to handle them for completeness
            _ => panic!("ASCII parser should not emit ANSI commands: {:?}", cmd),
        }
    }
}

#[test]
fn batches_printable_and_controls() {
    let mut p = AsciiParser::new();
    let data = b"Hello\nWorld\r!\x07\x08\x7F"; // includes LF, CR, Bell, Backspace, Delete
    let mut sink = CollectSink::new();
    p.parse(data, &mut sink);

    assert_eq!(sink.text, b"HelloWorld!");
    assert_eq!(sink.cmds.len(), 5);
    assert!(matches!(sink.cmds[0], TerminalCommand::LineFeed));
    assert!(matches!(sink.cmds[1], TerminalCommand::CarriageReturn));
    assert!(matches!(sink.cmds[2], TerminalCommand::Bell));
    assert!(matches!(sink.cmds[3], TerminalCommand::Backspace));
    assert!(matches!(sink.cmds[4], TerminalCommand::Delete));
}
