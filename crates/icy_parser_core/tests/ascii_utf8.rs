use icy_parser_core::{AsciiParser, CommandParser, CommandSink, TerminalCommand};

// Sink that owns emitted commands by copying printable slices.
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
            TerminalCommand::Printable(bytes) => {
                let owned = bytes.to_vec();
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
            _ => panic!("ASCII parser should not emit ANSI commands: {:?}", cmd),
        }
    }
}

#[test]
fn ascii_parser_handles_utf8_multibyte_as_printable_run() {
    // Characters: A (1 byte), ä (2 bytes), α (2 bytes), 😀 (4 bytes)
    let input = "Aäα😀"; // Rust source UTF-8
    let bytes = input.as_bytes();
    assert_eq!(bytes, &[0x41, 0xC3, 0xA4, 0xCE, 0xB1, 0xF0, 0x9F, 0x98, 0x80]);

    let mut parser = AsciiParser::new();
    let mut sink = CollectSink::new();
    parser.parse(bytes, &mut sink);

    // Expect single Printable spanning entire UTF-8 sequence (no controls inside).
    assert_eq!(sink.cmds.len(), 1);
    match &sink.cmds[0] {
        TerminalCommand::Printable(p) => assert_eq!(*p, bytes),
        _ => panic!("Expected one printable run"),
    }
}
