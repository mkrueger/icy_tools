//! Lossless, editable view of an IGS byte stream.
//!
//! Every input byte belongs to exactly one [`IgsItem`]. Unedited commands keep their
//! original bytes, so an unedited stream encodes back to the identical byte sequence.
//! Edited or new commands are written in canonical `G#` form and verified by parsing.

use std::fmt;

use super::{IgsCommand, IgsParser, LoopCommandData, LoopParamToken, LoopTarget, State};
use crate::{CommandParser, CommandSink, ErrorLevel, ParseError, TerminalCommand};

/// The parser position an IGS item starts in. It decides whether a command needs a `G#` prefix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IgsStreamState {
    /// Outside of any command; a command needs a `G#` prefix.
    Idle,
    /// Directly after a command terminator; the next command omits `G#`.
    Chained,
    /// Inside an unfinished command or escape sequence.
    Busy,
}

fn stream_state(parser: &IgsParser) -> IgsStreamState {
    match parser.state {
        State::Default if parser.vt52_parser.is_default_state() => IgsStreamState::Idle,
        State::GotIgsStart => IgsStreamState::Chained,
        _ => IgsStreamState::Busy,
    }
}

/// An IGS command together with the exact bytes it was read from.
#[derive(Clone, Debug, PartialEq)]
pub struct IgsCommandItem {
    command: IgsCommand,
    source: Option<(IgsStreamState, Vec<u8>)>,
    trailing: Vec<u8>,
}

impl IgsCommandItem {
    /// A new command, written as one `G#` line.
    pub fn new(command: IgsCommand) -> Self {
        Self {
            command,
            source: None,
            trailing: b"\n".to_vec(),
        }
    }

    pub fn command(&self) -> &IgsCommand {
        &self.command
    }

    /// Replaces the command. The original bytes are dropped unless the command is unchanged.
    pub fn set_command(&mut self, command: IgsCommand) {
        if self.command != command {
            self.command = command;
            self.source = None;
        }
    }

    /// The bytes this command was read from, `None` once edited.
    pub fn source(&self) -> Option<&[u8]> {
        self.source.as_ref().map(|(_, bytes)| bytes.as_slice())
    }

    /// Line breaks following the command.
    pub fn trailing(&self) -> &[u8] {
        &self.trailing
    }
}

/// Bytes that are not an IGS command: VT52 text, escape sequences and unparsable input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IgsText {
    pub bytes: Vec<u8>,
    /// The parser reported an error for these bytes, or they form an unfinished command.
    pub invalid: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum IgsItem {
    Command(IgsCommandItem),
    Text(IgsText),
}

impl IgsItem {
    pub fn command(&self) -> Option<&IgsCommand> {
        match self {
            IgsItem::Command(item) => Some(item.command()),
            IgsItem::Text(_) => None,
        }
    }
}

impl From<IgsCommand> for IgsItem {
    fn from(command: IgsCommand) -> Self {
        IgsItem::Command(IgsCommandItem::new(command))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IgsEncodeError {
    /// The command at `index` cannot be written so that it parses back identically.
    Unrepresentable { index: usize },
    /// The encoded stream does not parse back into the same commands.
    Mismatch,
}

impl fmt::Display for IgsEncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IgsEncodeError::Unrepresentable { index } => write!(f, "IGS command {index} cannot be written without changing its meaning"),
            IgsEncodeError::Mismatch => write!(f, "IGS stream does not parse back into the same commands"),
        }
    }
}

impl std::error::Error for IgsEncodeError {}

#[derive(Default)]
struct UnitSink {
    commands: Vec<IgsCommand>,
    output: bool,
    line_breaks_only: bool,
    error: bool,
}

impl UnitSink {
    fn new() -> Self {
        Self {
            line_breaks_only: true,
            ..Default::default()
        }
    }

    fn is_empty(&self) -> bool {
        self.commands.is_empty() && !self.output && !self.error
    }
}

impl CommandSink for UnitSink {
    fn print(&mut self, text: &[u8]) {
        if !text.is_empty() {
            self.output = true;
            self.line_breaks_only = false;
        }
    }

    fn emit(&mut self, cmd: TerminalCommand) {
        self.output = true;
        if !matches!(cmd, TerminalCommand::CarriageReturn | TerminalCommand::LineFeed) {
            self.line_breaks_only = false;
        }
    }

    fn emit_igs(&mut self, cmd: IgsCommand) {
        self.commands.push(cmd);
    }

    fn report_error(&mut self, _error: ParseError, level: ErrorLevel) {
        if level >= ErrorLevel::Error {
            self.error = true;
        }
    }
}

struct NullSink;

impl CommandSink for NullSink {
    fn print(&mut self, _text: &[u8]) {}
    fn emit(&mut self, _cmd: TerminalCommand) {}
}

#[derive(Default)]
struct CommandCollector(Vec<IgsCommand>);

impl CommandSink for CommandCollector {
    fn print(&mut self, _text: &[u8]) {}
    fn emit(&mut self, _cmd: TerminalCommand) {}
    fn emit_igs(&mut self, cmd: IgsCommand) {
        self.0.push(cmd);
    }
}

fn push_text(items: &mut Vec<IgsItem>, bytes: Vec<u8>, invalid: bool) {
    if let Some(IgsItem::Text(text)) = items.last_mut()
        && text.invalid == invalid
    {
        text.bytes.extend(bytes);
        return;
    }
    items.push(IgsItem::Text(IgsText { bytes, invalid }));
}

fn is_line_break(bytes: &[u8]) -> bool {
    bytes.iter().all(|byte| matches!(byte, b'\r' | b'\n'))
}

/// All IGS commands of a byte stream, parsed in one piece without executing loops.
pub fn parse_igs_commands(bytes: &[u8]) -> Vec<IgsCommand> {
    let mut parser = IgsParser::new();
    let mut sink = CommandCollector::default();
    parser.parse(bytes, &mut sink);
    sink.0
}

/// Splits an IGS byte stream into editable items without losing any byte.
///
/// If the stream cannot be split consistently, it is returned as one text item.
pub fn parse_igs_stream(bytes: &[u8]) -> Vec<IgsItem> {
    let mut parser = IgsParser::new();
    let mut items = Vec::new();
    let mut pending = Vec::new();
    let mut unit_state = stream_state(&parser);
    for &byte in bytes {
        if pending.is_empty() {
            unit_state = stream_state(&parser);
        }
        pending.push(byte);
        let mut sink = UnitSink::new();
        parser.parse(&[byte], &mut sink);
        if sink.is_empty() {
            continue;
        }
        let unit = std::mem::take(&mut pending);
        if !sink.error && !sink.output && sink.commands.len() == 1 {
            items.push(IgsItem::Command(IgsCommandItem {
                command: sink.commands.pop().unwrap(),
                source: Some((unit_state, unit)),
                trailing: Vec::new(),
            }));
            continue;
        }
        if !sink.error
            && sink.commands.is_empty()
            && sink.line_breaks_only
            && is_line_break(&unit)
            && let Some(IgsItem::Command(item)) = items.last_mut()
        {
            item.trailing.extend(unit);
            continue;
        }
        push_text(&mut items, unit, sink.error || !sink.commands.is_empty());
    }
    if !pending.is_empty() {
        match items.last_mut() {
            Some(IgsItem::Command(item)) if is_line_break(&pending) => item.trailing.extend(pending),
            _ => {
                let invalid = !pending.iter().all(u8::is_ascii_whitespace);
                push_text(&mut items, pending, invalid);
            }
        }
    }

    let segmented: Vec<&IgsCommand> = items.iter().filter_map(IgsItem::command).collect();
    let whole = parse_igs_commands(bytes);
    if segmented.len() != whole.len() || segmented.iter().zip(&whole).any(|(a, b)| *a != b) {
        return if bytes.is_empty() {
            Vec::new()
        } else {
            vec![IgsItem::Text(IgsText {
                bytes: bytes.to_vec(),
                invalid: true,
            })]
        };
    }
    items
}

fn write_loop(out: &mut Vec<u8>, data: &LoopCommandData) {
    out.extend(format!("G#&>{},{},{},{}", data.from, data.to, data.step, data.delay).bytes());
    match &data.target {
        LoopTarget::Single(cmd_type) => {
            out.push(b',');
            out.push(cmd_type.to_char() as u8);
        }
        LoopTarget::ChainGang { commands } => {
            out.extend_from_slice(b",>");
            out.extend(commands.iter().map(|cmd| cmd.to_char() as u8));
            out.push(b'@');
        }
    }
    if data.modifiers.xor_stepping {
        out.push(b'|');
    }
    if data.modifiers.refresh_text_each_iteration {
        out.push(b'@');
    }
    if !data.modifiers.xor_stepping && !data.modifiers.refresh_text_each_iteration {
        out.push(b',');
    }
    out.extend(data.param_count.to_string().bytes());
    let mut last_was_colon = false;
    let mut last_was_text = false;
    for token in &data.params {
        match token {
            LoopParamToken::GroupSeparator => {
                out.push(b':');
                last_was_colon = true;
                last_was_text = false;
            }
            LoopParamToken::Number(value) => {
                if !last_was_colon {
                    out.push(b',');
                }
                out.extend(value.to_string().bytes());
                last_was_colon = false;
                last_was_text = false;
            }
            LoopParamToken::Expr(op, value) => {
                if !last_was_colon {
                    out.push(b',');
                }
                out.push(*op as u8);
                out.extend(value.to_string().bytes());
                last_was_colon = false;
                last_was_text = false;
            }
            LoopParamToken::Text(bytes) => {
                if !last_was_colon && !last_was_text {
                    out.push(b',');
                }
                out.extend_from_slice(bytes);
                out.push(b'@');
                last_was_colon = false;
                last_was_text = true;
            }
        }
    }
    out.push(b':');
}

/// The canonical `G#` form of a command, byte-exact for text parameters.
///
/// The result is not verified; use [`encode_igs_command`] for a checked encoding.
pub fn write_igs_command(command: &IgsCommand) -> Vec<u8> {
    match command {
        IgsCommand::WriteText { x, y, text } => {
            let mut out = format!("G#W>{x},{y},").into_bytes();
            out.extend_from_slice(text);
            out.push(b'@');
            out
        }
        IgsCommand::DefineZone {
            zone_id,
            x1,
            y1,
            x2,
            y2,
            length,
            string,
        } if !(9997..=9999).contains(zone_id) => {
            let mut out = format!("G#X>4,{zone_id},{x1},{y1},{x2},{y2},{length},").into_bytes();
            out.extend_from_slice(string);
            out.push(b':');
            out
        }
        IgsCommand::Loop(data) => {
            let mut out = Vec::new();
            write_loop(&mut out, data);
            out
        }
        IgsCommand::SetTextColor { layer, color } => format!("G#c>{layer},{color}:").into_bytes(),
        IgsCommand::DeleteLine { count } => format!("G#d>{count}:").into_bytes(),
        IgsCommand::InsertLine { mode, count } => format!("G#i>{mode},{count}:").into_bytes(),
        IgsCommand::ClearLine { mode } => format!("G#l>{mode}:").into_bytes(),
        IgsCommand::CursorMotion { direction, count } => {
            let direction = match direction {
                crate::Direction::Up => 0,
                crate::Direction::Down => 1,
                crate::Direction::Left => 2,
                crate::Direction::Right => 3,
            };
            format!("G#m>{direction},{count}:").into_bytes()
        }
        IgsCommand::RememberCursor { value } => format!("G#r>{value}:").into_bytes(),
        _ => command.to_string().into_bytes(),
    }
}

/// The canonical `G#` form of a command, or `None` if it does not parse back identically.
pub fn encode_igs_command(command: &IgsCommand) -> Option<Vec<u8>> {
    let bytes = write_igs_command(command);
    let mut parser = IgsParser::new();
    let mut sink = UnitSink::new();
    // Commands with random parameters may panic in the parser's value accessors.
    let parsed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        parser.parse(&bytes, &mut sink);
        stream_state(&parser)
    }))
    .ok()?;
    (parsed == IgsStreamState::Chained && !sink.error && !sink.output && sink.commands.as_slice() == std::slice::from_ref(command)).then_some(bytes)
}

/// Writes items to bytes. Unedited commands keep their original bytes when they start in
/// the same parser state as before; all others use their canonical form.
pub fn encode_igs_stream(items: &[IgsItem]) -> Result<Vec<u8>, IgsEncodeError> {
    let mut out = Vec::new();
    let mut parser = IgsParser::new();
    for (index, item) in items.iter().enumerate() {
        let start = out.len();
        match item {
            IgsItem::Text(text) => out.extend_from_slice(&text.bytes),
            IgsItem::Command(item) => {
                let state = stream_state(&parser);
                match &item.source {
                    Some((source_state, bytes)) if *source_state == state => out.extend_from_slice(bytes),
                    _ => {
                        let bytes = encode_igs_command(&item.command).ok_or(IgsEncodeError::Unrepresentable { index })?;
                        match state {
                            IgsStreamState::Idle => out.extend(bytes),
                            IgsStreamState::Chained => out.extend_from_slice(&bytes[2..]),
                            IgsStreamState::Busy => return Err(IgsEncodeError::Unrepresentable { index }),
                        }
                    }
                }
                out.extend_from_slice(&item.trailing);
            }
        }
        parser.parse(&out[start..], &mut NullSink);
    }
    Ok(out)
}

/// Like [`encode_igs_stream`], but also verifies that the bytes parse back into the same commands.
pub fn encode_igs_stream_checked(items: &[IgsItem]) -> Result<Vec<u8>, IgsEncodeError> {
    let bytes = encode_igs_stream(items)?;
    let parsed = parse_igs_commands(&bytes);
    let expected: Vec<&IgsCommand> = items.iter().filter_map(IgsItem::command).collect();
    if parsed.len() != expected.len() || parsed.iter().zip(expected).any(|(a, b)| a != b) {
        return Err(IgsEncodeError::Mismatch);
    }
    Ok(bytes)
}
