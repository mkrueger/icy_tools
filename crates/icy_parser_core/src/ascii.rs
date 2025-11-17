use crate::{CommandParser, CommandSink, TerminalCommand};

// ASCII control character codes
const BELL: u8 = 0x07;
const BACKSPACE: u8 = 0x08;
const TAB: u8 = 0x09;
const LINE_FEED: u8 = 0x0A;
const FORM_FEED: u8 = 0x0C;
const CARRIAGE_RETURN: u8 = 0x0D;
const DELETE: u8 = 0x7F;

// Direct lookup table mapping ASCII bytes to TerminalCommand
const CONTROL_COMMAND: [Option<TerminalCommand>; 256] = {
    let mut table = [None; 256];
    table[BELL as usize] = Some(TerminalCommand::Bell);
    table[BACKSPACE as usize] = Some(TerminalCommand::Backspace);
    table[TAB as usize] = Some(TerminalCommand::Tab);
    table[LINE_FEED as usize] = Some(TerminalCommand::LineFeed);
    table[FORM_FEED as usize] = Some(TerminalCommand::FormFeed);
    table[CARRIAGE_RETURN as usize] = Some(TerminalCommand::CarriageReturn);
    table[DELETE as usize] = Some(TerminalCommand::Delete);
    table
};

#[derive(Default)]
pub struct AsciiParser {}

impl AsciiParser {
    pub fn new() -> Self {
        Self::default()
    }
}

impl CommandParser for AsciiParser {
    fn parse(&mut self, input: &[u8], sink: &mut dyn CommandSink) {
        let mut i = 0;
        let len = input.len();

        let mut start = i;
        while i < len {
            let nb = unsafe { *input.get_unchecked(i) };
            if let Some(ctrl) = unsafe { CONTROL_COMMAND.get_unchecked(nb as usize) } {
                flush_input(input, sink, i, start);
                sink.emit(*ctrl);
                i += 1;
                start = i;
                continue;
            }
            i += 1;
        }

        flush_input(input, sink, i, start);
    }
}

#[inline(always)]
fn flush_input(input: &[u8], sink: &mut dyn CommandSink, i: usize, start: usize) {
    if i > start {
        sink.print(&input[start..i]);
    }
}
