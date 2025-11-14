use crate::{CommandParser, CommandSink, TerminalCommand};

#[derive(Default)]
pub struct AsciiParser {}

impl AsciiParser {
    pub fn new() -> Self {
        Self::default()
    }
}

#[repr(u8)]
#[derive(Copy, Clone)]
enum ControlKind {
    Bell = 1,
    Backspace = 2,
    Tab = 3,
    LineFeed = 4,
    FormFeed = 5,
    CarriageReturn = 6,
    Delete = 7,
}

const fn build_control_lut() -> [u8; 256] {
    let mut lut = [0u8; 256];
    lut[0x07] = ControlKind::Bell as u8;
    lut[0x08] = ControlKind::Backspace as u8;
    lut[0x09] = ControlKind::Tab as u8;
    lut[0x0A] = ControlKind::LineFeed as u8;
    lut[0x0C] = ControlKind::FormFeed as u8;
    lut[0x0D] = ControlKind::CarriageReturn as u8;
    lut[0x7F] = ControlKind::Delete as u8;
    lut
}

const CONTROL_LUT: [u8; 256] = build_control_lut();

impl CommandParser for AsciiParser {
    fn parse(&mut self, input: &[u8], sink: &mut dyn CommandSink) {
        let mut i = 0;
        while i < input.len() {
            let b = unsafe { *input.get_unchecked(i) }; // unchecked; guarded by outer bound
            let code = unsafe { *CONTROL_LUT.get_unchecked(b as usize) };
            if code != 0 {
                match code {
                    c if c == ControlKind::Bell as u8 => sink.emit(TerminalCommand::Bell),
                    c if c == ControlKind::Backspace as u8 => sink.emit(TerminalCommand::Backspace),
                    c if c == ControlKind::Tab as u8 => sink.emit(TerminalCommand::Tab),
                    c if c == ControlKind::LineFeed as u8 => sink.emit(TerminalCommand::LineFeed),
                    c if c == ControlKind::FormFeed as u8 => sink.emit(TerminalCommand::FormFeed),
                    c if c == ControlKind::CarriageReturn as u8 => sink.emit(TerminalCommand::CarriageReturn),
                    c if c == ControlKind::Delete as u8 => sink.emit(TerminalCommand::Delete),
                    _ => unreachable!(),
                }
                i += 1;
            } else {
                let start = i;
                i += 1;
                while i < input.len() {
                    let nb = unsafe { *input.get_unchecked(i) };
                    if unsafe { *CONTROL_LUT.get_unchecked(nb as usize) } != 0 {
                        break;
                    }
                    i += 1;
                }
                sink.print(&input[start..i]);
            }
        }
    }
}
