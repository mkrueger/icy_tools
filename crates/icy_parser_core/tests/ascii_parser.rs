use icy_parser_core::{AsciiParser, CommandParser, CommandSink, TerminalCommand};

struct CollectSink {
    pub cmds: Vec<TerminalCommand<'static>>,
}
impl CollectSink {
    fn new() -> Self {
        Self { cmds: Vec::new() }
    }
}

impl CommandSink for CollectSink {
    fn emit(&mut self, cmd: TerminalCommand<'_>) {
        match cmd {
            TerminalCommand::Printable(b) => {
                let owned = b.to_vec();
                let leaked: &'static [u8] = Box::leak(owned.into_boxed_slice());
                self.cmds.push(TerminalCommand::Printable(leaked));
            }
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

    assert_eq!(sink.cmds.len(), 8);
    if let TerminalCommand::Printable(bs) = &sink.cmds[0] {
        assert_eq!(bs, b"Hello");
    }
    assert!(matches!(sink.cmds[1], TerminalCommand::LineFeed));
    if let TerminalCommand::Printable(bs) = &sink.cmds[2] {
        assert_eq!(bs, b"World");
    }
    assert!(matches!(sink.cmds[3], TerminalCommand::CarriageReturn));
    if let TerminalCommand::Printable(bs) = &sink.cmds[4] {
        assert_eq!(bs, b"!");
    }
    assert!(matches!(sink.cmds[5], TerminalCommand::Bell));
    assert!(matches!(sink.cmds[6], TerminalCommand::Backspace));
    assert!(matches!(sink.cmds[7], TerminalCommand::Delete));
}
