//! An editable RIPscrip scene independent of either editor UI. Mixed ANSI/RIP streams
//! retain their original bytes; commands within that stream are read-only.

use std::path::{Path, PathBuf};

use icy_engine::{GraphicsType, PaletteScreenBuffer, Screen, ScreenMode, ScreenSink};
use icy_parser_core::{CommandParser, CommandSink, ErrorLevel, ParseError, RipCommand, RipParser, TerminalCommand};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RipDocumentError {
    #[error("RIP stream contains non-command text")]
    NonCommandText,
    #[error("RIP parser dropped a command or encountered an incomplete command")]
    DroppedCommand,
    #[error("RIP parse error: {0}")]
    Parse(#[from] ParseError),
    #[error("RIP command {index} cannot be round-tripped through the parser")]
    UnrepresentableCommand { index: usize },
    #[error("command index {index} is out of bounds (length {len})")]
    InvalidIndex { index: usize, len: usize },
    #[error("command {index} belongs to the preserved RIP/ANSI stream and cannot be edited individually")]
    PreservedCommand { index: usize },
    #[error("no path is associated with this RIP document")]
    MissingPath,
    #[error("RIP drawing references an external icon, which cannot be safely previewed")]
    ExternalIcon,
    #[error("RIP file I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("Could not save RIP drawing: {0}")]
    Save(String),
}

pub type RipResult<T> = Result<T, RipDocumentError>;

#[derive(Default)]
struct CommandCollector {
    commands: Vec<RipCommand>,
    non_command_text: bool,
    error: Option<ParseError>,
}

impl CommandSink for CommandCollector {
    fn print(&mut self, bytes: &[u8]) {
        if bytes.iter().any(|byte| !matches!(byte, b'\r' | b'\n')) {
            self.non_command_text = true;
        }
    }

    fn emit(&mut self, _command: TerminalCommand) {
        self.non_command_text = true;
    }

    fn emit_rip(&mut self, command: RipCommand) {
        self.commands.push(command);
    }

    fn report_error(&mut self, error: ParseError, level: ErrorLevel) {
        if level >= ErrorLevel::Error && self.error.is_none() {
            self.error = Some(error);
        }
    }
}

struct PreviewSink<'a, 'b> {
    screen: &'a mut ScreenSink<'b>,
    remaining: usize,
}

impl CommandSink for PreviewSink<'_, '_> {
    fn print(&mut self, bytes: &[u8]) {
        if self.remaining > 0 {
            self.screen.print(bytes);
        }
    }

    fn emit(&mut self, command: TerminalCommand) {
        if self.remaining > 0 {
            self.screen.emit(command);
        }
    }

    fn emit_rip(&mut self, command: RipCommand) {
        if self.remaining > 0 {
            self.screen.emit_rip(command);
            self.remaining -= 1;
        }
    }
}

fn collect_commands(bytes: &[u8]) -> RipResult<CommandCollector> {
    if bytes.is_empty() {
        return Ok(CommandCollector::default());
    }
    let mut parser = RipParser::new();
    let mut sink = CommandCollector::default();
    parser.parse(bytes, &mut sink);
    // The parser emits a final parameterized command on newline, not at EOF.
    if !bytes.ends_with(b"\n") {
        parser.parse(b"\n", &mut sink);
    }
    if let Some(error) = sink.error {
        return Err(error.into());
    }
    if !bytes.is_empty() && sink.commands.is_empty() && !bytes.iter().all(u8::is_ascii_whitespace) {
        return Err(RipDocumentError::NonCommandText);
    }
    // RipParser currently skips some malformed parameters without reporting an
    // error to its sink. Count unescaped command delimiters to detect that loss.
    let mut delimiters = 0;
    let mut backslashes = 0;
    for &byte in bytes {
        if byte == b'|' && backslashes % 2 == 0 {
            delimiters += 1;
        }
        backslashes = if byte == b'\\' { backslashes + 1 } else { 0 };
    }
    if !sink.non_command_text && delimiters != sink.commands.len() {
        return Err(RipDocumentError::DroppedCommand);
    }
    Ok(sink)
}

fn parse_commands(bytes: &[u8]) -> RipResult<Vec<RipCommand>> {
    let sink = collect_commands(bytes)?;
    if sink.non_command_text {
        return Err(RipDocumentError::NonCommandText);
    }
    Ok(sink.commands)
}

fn encode_commands(commands: &[RipCommand]) -> RipResult<Vec<u8>> {
    let mut output = Vec::new();
    for (index, command) in commands.iter().enumerate() {
        let line = format!("!{command}\r\n");
        if parse_commands(line.as_bytes())?.as_slice() != std::slice::from_ref(command) {
            return Err(RipDocumentError::UnrepresentableCommand { index });
        }
        output.extend_from_slice(line.as_bytes());
    }
    // Some commands change parser mode. Check the complete scene as well.
    if parse_commands(&output)?.as_slice() != commands {
        return Err(RipDocumentError::UnrepresentableCommand {
            index: commands.len().saturating_sub(1),
        });
    }
    Ok(output)
}

/// A rendered 640 × 350 RIP scene. `indices` refer to the final palette;
/// `rgba` contains tightly packed, unscaled RGBA pixels.
pub struct RipPreview {
    screen: PaletteScreenBuffer,
    mode: ScreenMode,
}

impl RipPreview {
    /// Access the rendered RIP screen, including its palette and pixel indices.
    pub fn screen(&self) -> &PaletteScreenBuffer {
        &self.screen
    }

    pub fn screen_mode(&self) -> ScreenMode {
        self.mode
    }

    pub fn width(&self) -> usize {
        self.screen.pixel_size.width as usize
    }

    pub fn height(&self) -> usize {
        self.screen.pixel_size.height as usize
    }

    pub fn indices(&self) -> &[u8] {
        self.screen.screen()
    }

    pub fn pixel_index(&self, x: usize, y: usize) -> Option<u8> {
        (x < self.width() && y < self.height()).then(|| self.indices()[y * self.width() + x])
    }

    pub fn pixel_rgba(&self, x: usize, y: usize) -> Option<[u8; 4]> {
        self.pixel_index(x, y).map(|index| {
            let (r, g, b) = self.screen.palette().rgb(u32::from(index));
            [r, g, b, 255]
        })
    }

    pub fn rgba(&self) -> Vec<u8> {
        let mut rgba = Vec::with_capacity(self.indices().len() * 4);
        for &index in self.indices() {
            let (r, g, b) = self.screen.palette().rgb(u32::from(index));
            rgba.extend_from_slice(&[r, g, b, 255]);
        }
        rgba
    }
}

/// Ordered RIP commands with snapshot-based undo/redo and saved-state tracking.
pub struct RipDocument {
    commands: Vec<RipCommand>,
    saved_commands: Vec<RipCommand>,
    undo: Vec<Vec<RipCommand>>,
    redo: Vec<Vec<RipCommand>>,
    revision: u64,
    path: Option<PathBuf>,
    baseline: Vec<u8>,
    preserved: Option<Vec<u8>>,
    preserved_commands: usize,
}

impl Default for RipDocument {
    fn default() -> Self {
        Self::new()
    }
}

impl RipDocument {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            saved_commands: Vec::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            revision: 0,
            path: None,
            baseline: Vec::new(),
            preserved: None,
            preserved_commands: 0,
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> RipResult<Self> {
        let sink = collect_commands(bytes)?;
        let commands = sink.commands;
        let preserved = (sink.non_command_text || encode_commands(&commands).is_err()).then(|| bytes.to_vec());
        Ok(Self {
            saved_commands: commands.clone(),
            preserved_commands: if preserved.is_some() { commands.len() } else { 0 },
            preserved,
            commands,
            baseline: bytes.to_vec(),
            ..Self::new()
        })
    }

    pub fn open(path: impl AsRef<Path>) -> RipResult<Self> {
        let bytes = std::fs::read(path.as_ref())?;
        let mut document = Self::from_bytes(&bytes)?;
        document.path = Some(path.as_ref().to_path_buf());
        Ok(document)
    }

    pub fn load(path: impl AsRef<Path>) -> RipResult<Self> {
        Self::open(path)
    }

    pub fn commands(&self) -> &[RipCommand] {
        &self.commands
    }

    pub fn preserved_commands(&self) -> usize {
        self.preserved_commands
    }

    /// Original mixed or non-round-trippable stream, kept read-only before editable commands.
    pub fn preserved_source(&self) -> Option<&[u8]> {
        self.preserved.as_deref()
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn recovery_snapshot(&self) -> RipResult<crate::recovery::Snapshot> {
        Ok(crate::recovery::Snapshot {
            kind: crate::recovery::RecoveryKind::Rip,
            path: self.path.clone(),
            disk: self.path.as_ref().map(|_| crate::recovery::Fingerprint::of(&self.baseline)),
            payload: self.to_bytes()?,
        })
    }

    pub fn from_recovery(snapshot: &crate::recovery::Snapshot) -> RipResult<Self> {
        let mut document = Self::from_bytes(&snapshot.payload)?;
        document.path = snapshot.path.clone();
        document.baseline = snapshot
            .path
            .as_deref()
            .zip(snapshot.disk)
            .and_then(|(path, disk)| disk.read_matching(path))
            .unwrap_or_default();
        if let Ok(original) = parse_commands(&document.baseline) {
            document.saved_commands = original;
        }
        if document.saved_commands == document.commands {
            document.saved_commands = vec![RipCommand::Comment { text: String::new() }];
        }
        Ok(document)
    }

    pub fn is_dirty(&self) -> bool {
        self.commands != self.saved_commands
    }

    pub fn modified(&self) -> bool {
        self.is_dirty()
    }

    /// Increases whenever the command list changes, including undo and redo.
    /// This counter is local to this document instance; saving does not change it.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn to_bytes(&self) -> RipResult<Vec<u8>> {
        if let Some(prefix) = &self.preserved {
            if !self.is_dirty() {
                return Ok(self.baseline.clone());
            }
            let mut bytes = prefix.clone();
            if !bytes.ends_with(b"\n") {
                bytes.extend_from_slice(b"\r\n");
            }
            bytes.extend(encode_commands(&self.commands[self.preserved_commands..])?);
            return Ok(bytes);
        }
        encode_commands(&self.commands)
    }

    pub fn save_current(&mut self, overwrite: bool) -> RipResult<()> {
        let path = self.path.clone().ok_or(RipDocumentError::MissingPath)?;
        self.save(path, overwrite)
    }

    pub fn save(&mut self, path: impl AsRef<Path>, overwrite: bool) -> RipResult<()> {
        let bytes = self.to_bytes()?;
        crate::files::save_bytes(
            path.as_ref(),
            &bytes,
            self.path.as_deref().map(|path| (path, self.baseline.as_slice())),
            overwrite,
        )
        .map_err(|error| RipDocumentError::Save(error.to_string()))?;
        self.path = Some(path.as_ref().to_path_buf());
        self.baseline = bytes;
        self.saved_commands = self.commands.clone();
        Ok(())
    }

    pub fn save_as(&mut self, path: impl AsRef<Path>, overwrite: bool) -> RipResult<()> {
        self.save(path, overwrite)
    }

    pub fn preview(&self) -> RipResult<RipPreview> {
        self.preview_through(None)
    }

    /// Preview through the selected command (inclusive), without changing the scene.
    pub fn preview_through(&self, through: Option<usize>) -> RipResult<RipPreview> {
        Ok(RipPreview {
            screen: self.render_through(through)?,
            mode: ScreenMode::Rip,
        })
    }

    pub fn render_current(&self) -> RipResult<PaletteScreenBuffer> {
        self.render_through(None)
    }

    pub fn render_through(&self, through: Option<usize>) -> RipResult<PaletteScreenBuffer> {
        self.render_with(through, None, &[])
    }

    /// The scene with `extra` commands appended, e.g. a shape that is still being drawn.
    pub fn preview_with(&self, extra: &[RipCommand]) -> RipResult<RipPreview> {
        Ok(RipPreview {
            screen: self.render_with(None, None, extra)?,
            mode: ScreenMode::Rip,
        })
    }

    /// The scene with the command at `index` replaced, e.g. a shape that is being moved.
    pub fn preview_replacing(&self, index: usize, command: &RipCommand) -> RipResult<RipPreview> {
        if index >= self.commands.len() {
            return Err(RipDocumentError::InvalidIndex {
                index,
                len: self.commands.len(),
            });
        }
        if index < self.preserved_commands {
            return Err(RipDocumentError::PreservedCommand { index });
        }
        Ok(RipPreview {
            screen: self.render_with(None, Some((index, command)), &[])?,
            mode: ScreenMode::Rip,
        })
    }

    fn render_with(&self, through: Option<usize>, replace: Option<(usize, &RipCommand)>, extra: &[RipCommand]) -> RipResult<PaletteScreenBuffer> {
        if extra
            .iter()
            .chain(replace.map(|(_, command)| command))
            .any(|command| matches!(command, RipCommand::LoadIcon { .. }))
        {
            return Err(RipDocumentError::ExternalIcon);
        }
        if let Some(index) = through {
            if index >= self.commands.len() {
                return Err(RipDocumentError::InvalidIndex {
                    index,
                    len: self.commands.len(),
                });
            }
        }
        let end = through.map_or(self.commands.len(), |index| index + 1);
        if let Some(prefix) = &self.preserved {
            if self.commands[..end].iter().any(|command| matches!(command, RipCommand::LoadIcon { .. })) {
                return Err(RipDocumentError::ExternalIcon);
            }
            let mut screen = PaletteScreenBuffer::new(GraphicsType::Rip);
            let mut screen_sink = ScreenSink::new(&mut screen);
            let mut sink = PreviewSink {
                screen: &mut screen_sink,
                remaining: end,
            };
            let mut parser = RipParser::new();
            parser.parse(prefix, &mut sink);
            if !prefix.ends_with(b"\n") {
                parser.parse(b"\n", &mut sink);
            }
            if end > self.preserved_commands {
                for (index, command) in self.commands.iter().enumerate().take(end).skip(self.preserved_commands) {
                    let command = match replace {
                        Some((replaced, replacement)) if replaced == index => replacement,
                        _ => command,
                    };
                    sink.emit_rip(command.clone());
                }
            }
            for command in extra {
                screen_sink.emit_rip(command.clone());
            }
            return Ok(screen);
        }
        if extra.is_empty() && replace.is_none() {
            return Self::render(&self.commands[..end]);
        }
        let mut commands = self.commands[..end].to_vec();
        if let Some((index, command)) = replace.filter(|(index, _)| *index < end) {
            commands[index] = command.clone();
        }
        commands.extend_from_slice(extra);
        Self::render(&commands)
    }

    /// The scene with every editable command replaced by `editable`, e.g. text whose font changes.
    pub fn preview_editable(&self, editable: &[RipCommand]) -> RipResult<RipPreview> {
        if editable.iter().any(|command| matches!(command, RipCommand::LoadIcon { .. })) {
            return Err(RipDocumentError::ExternalIcon);
        }
        let screen = if let Some(prefix) = &self.preserved {
            if self.commands[..self.preserved_commands]
                .iter()
                .any(|command| matches!(command, RipCommand::LoadIcon { .. }))
            {
                return Err(RipDocumentError::ExternalIcon);
            }
            let mut screen = PaletteScreenBuffer::new(GraphicsType::Rip);
            let mut screen_sink = ScreenSink::new(&mut screen);
            let mut sink = PreviewSink {
                screen: &mut screen_sink,
                remaining: self.preserved_commands,
            };
            let mut parser = RipParser::new();
            parser.parse(prefix, &mut sink);
            if !prefix.ends_with(b"\n") {
                parser.parse(b"\n", &mut sink);
            }
            for command in editable {
                screen_sink.emit_rip(command.clone());
            }
            screen
        } else {
            Self::render(editable)?
        };
        Ok(RipPreview { screen, mode: ScreenMode::Rip })
    }

    /// Replay an arbitrary command sequence without changing the document.
    /// For hover previews, callers can clone `commands()`, modify the clone,
    /// and pass it here.
    pub fn render(commands: &[RipCommand]) -> RipResult<PaletteScreenBuffer> {
        let mut screen = PaletteScreenBuffer::new(GraphicsType::Rip);
        let mut sink = ScreenSink::new(&mut screen);
        for command in commands {
            if matches!(command, RipCommand::LoadIcon { .. }) {
                return Err(RipDocumentError::ExternalIcon);
            }
            sink.emit_rip(command.clone());
        }
        Ok(screen)
    }

    fn record_change(&mut self) {
        self.undo.push(self.commands.clone());
        self.redo.clear();
        self.revision = self.revision.wrapping_add(1);
    }

    pub fn append(&mut self, command: RipCommand) {
        self.record_change();
        self.commands.push(command);
    }

    pub fn add_command(&mut self, command: RipCommand) {
        self.append(command);
    }

    pub fn append_many(&mut self, commands: Vec<RipCommand>) {
        if !commands.is_empty() {
            self.record_change();
            self.commands.extend(commands);
        }
    }

    pub fn try_append_many(&mut self, commands: Vec<RipCommand>) -> RipResult<()> {
        if !commands.is_empty() {
            let mut appended = self.commands[self.preserved_commands..].to_vec();
            appended.extend(commands.iter().cloned());
            encode_commands(&appended).map_err(|error| match error {
                RipDocumentError::UnrepresentableCommand { index } => RipDocumentError::UnrepresentableCommand {
                    index: index + self.preserved_commands,
                },
                other => other,
            })?;
            self.append_many(commands);
        }
        Ok(())
    }

    /// Add a group of commands as a single undo step; an empty group changes nothing.
    pub fn add_commands(&mut self, commands: Vec<RipCommand>) {
        self.append_many(commands);
    }

    pub fn insert(&mut self, index: usize, command: RipCommand) -> RipResult<()> {
        if index > self.commands.len() {
            return Err(RipDocumentError::InvalidIndex {
                index,
                len: self.commands.len(),
            });
        }
        if index < self.preserved_commands {
            return Err(RipDocumentError::PreservedCommand { index });
        }
        self.record_change();
        self.commands.insert(index, command);
        Ok(())
    }

    pub fn replace(&mut self, index: usize, command: RipCommand) -> RipResult<()> {
        if index >= self.commands.len() {
            return Err(RipDocumentError::InvalidIndex {
                index,
                len: self.commands.len(),
            });
        }
        if index < self.preserved_commands {
            return Err(RipDocumentError::PreservedCommand { index });
        }
        if self.commands[index] != command {
            let mut replacement = self.commands[self.preserved_commands..].to_vec();
            replacement[index - self.preserved_commands] = command.clone();
            encode_commands(&replacement).map_err(|error| match error {
                RipDocumentError::UnrepresentableCommand { index } => RipDocumentError::UnrepresentableCommand {
                    index: index + self.preserved_commands,
                },
                other => other,
            })?;
            self.record_change();
            self.commands[index] = command;
        }
        Ok(())
    }

    /// Replaces all editable commands in one undo step, e.g. text together with the font and
    /// color commands inserted around it.
    pub fn replace_editable(&mut self, editable: Vec<RipCommand>) -> RipResult<()> {
        if self.commands[self.preserved_commands..] == editable[..] {
            return Ok(());
        }
        encode_commands(&editable).map_err(|error| match error {
            RipDocumentError::UnrepresentableCommand { index } => RipDocumentError::UnrepresentableCommand {
                index: index + self.preserved_commands,
            },
            other => other,
        })?;
        self.record_change();
        self.commands.truncate(self.preserved_commands);
        self.commands.extend(editable);
        Ok(())
    }

    /// Replaces several commands in one undo step, e.g. a button and its style.
    pub fn replace_many(&mut self, replacements: Vec<(usize, RipCommand)>) -> RipResult<()> {
        let mut replaced = self.commands.clone();
        for (index, command) in &replacements {
            if *index >= replaced.len() {
                return Err(RipDocumentError::InvalidIndex {
                    index: *index,
                    len: replaced.len(),
                });
            }
            if *index < self.preserved_commands {
                return Err(RipDocumentError::PreservedCommand { index: *index });
            }
            replaced[*index] = command.clone();
        }
        if replaced == self.commands {
            return Ok(());
        }
        encode_commands(&replaced[self.preserved_commands..]).map_err(|error| match error {
            RipDocumentError::UnrepresentableCommand { index } => RipDocumentError::UnrepresentableCommand {
                index: index + self.preserved_commands,
            },
            other => other,
        })?;
        self.record_change();
        self.commands = replaced;
        Ok(())
    }

    pub fn delete(&mut self, index: usize) -> RipResult<RipCommand> {
        if index >= self.commands.len() {
            return Err(RipDocumentError::InvalidIndex {
                index,
                len: self.commands.len(),
            });
        }
        if index < self.preserved_commands {
            return Err(RipDocumentError::PreservedCommand { index });
        }
        self.record_change();
        Ok(self.commands.remove(index))
    }

    pub fn remove_command(&mut self, index: usize) -> RipResult<RipCommand> {
        self.delete(index)
    }

    /// Move a command to its final index (both indices refer to the current list).
    pub fn move_command(&mut self, from: usize, to: usize) -> RipResult<()> {
        let len = self.commands.len();
        for index in [from, to] {
            if index >= len {
                return Err(RipDocumentError::InvalidIndex { index, len });
            }
            if index < self.preserved_commands {
                return Err(RipDocumentError::PreservedCommand { index });
            }
        }
        if from != to {
            self.record_change();
            let command = self.commands.remove(from);
            self.commands.insert(to, command);
        }
        Ok(())
    }

    pub fn undo(&mut self) -> bool {
        if let Some(previous) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.commands, previous));
            self.revision = self.revision.wrapping_add(1);
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.commands, next));
            self.revision = self.revision.wrapping_add(1);
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_edit_and_history() {
        let input = b"!|c0C|L0000000A0A\r\n!|!author\\!name\r\n";
        let mut doc = RipDocument::from_bytes(input).unwrap();
        assert_eq!(doc.commands().len(), 3);
        assert!(!doc.is_dirty());
        assert_eq!(RipDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap().commands(), doc.commands());

        doc.move_command(2, 0).unwrap();
        assert!(doc.is_dirty());
        assert_eq!(doc.delete(0).unwrap(), RipCommand::Comment { text: "author!name".into() });
        doc.insert(0, RipCommand::Pixel { x: 2, y: 3 }).unwrap();
        assert!(doc.undo());
        assert!(doc.undo());
        assert!(doc.undo());
        assert!(!doc.is_dirty());
        assert!(doc.redo());
        assert!(doc.is_dirty());
        doc.replace(0, RipCommand::Color { c: 12 }).unwrap();
        assert!(!doc.can_redo());
        assert_eq!(RipDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap().commands(), doc.commands());
    }

    #[test]
    fn scene_replay_changes_pixels_and_palette() {
        let mut doc = RipDocument::from_bytes(b"!|c0C|X0A0A\r\n").unwrap();
        assert_eq!(doc.revision(), 0);
        let preview = doc.preview().unwrap();
        assert_eq!((preview.width(), preview.height()), (640, 350));
        assert_eq!(preview.pixel_index(10, 10), Some(12));
        assert_eq!(preview.pixel_rgba(10, 10), Some([255, 85, 85, 255]));
        assert_eq!(preview.screen().screen()[10 * preview.width() + 10], 12);
        assert_eq!(preview.rgba().len(), 640 * 350 * 4);
        assert_eq!(preview.pixel_index(640, 0), None);
        let current = doc.render_current().unwrap();
        assert_eq!(current.screen()[10 * 640 + 10], 12);
        let mut hover_commands = doc.commands().to_vec();
        hover_commands[0] = RipCommand::Color { c: 4 };
        let hover = RipDocument::render(&hover_commands).unwrap();
        assert_eq!(hover.screen()[10 * 640 + 10], 4);
        assert_eq!(doc.revision(), 0);
        assert_eq!(doc.render_current().unwrap().screen()[10 * 640 + 10], 12);
        doc.move_command(1, 0).unwrap();
        assert_eq!(doc.revision(), 1);
        assert_ne!(doc.preview().unwrap().pixel_index(10, 10), Some(12));
        assert!(doc.undo());
        assert_eq!(doc.revision(), 2);
        assert!(!doc.modified());
        assert_eq!(doc.preview().unwrap().pixel_index(10, 10), Some(12));
        assert!(doc.redo());
        assert_eq!(doc.revision(), 3);
    }

    #[test]
    fn preview_through_is_inclusive_and_does_not_change_the_document() {
        let mut doc = RipDocument::new();
        doc.append_many(vec![
            RipCommand::Color { c: 12 },
            RipCommand::Pixel { x: 10, y: 10 },
            RipCommand::Color { c: 4 },
            RipCommand::Pixel { x: 20, y: 20 },
        ]);
        let revision = doc.revision();
        assert_ne!(doc.preview_through(Some(0)).unwrap().pixel_index(10, 10), Some(12));
        assert_eq!(doc.preview_through(Some(1)).unwrap().pixel_index(10, 10), Some(12));
        assert_ne!(doc.preview_through(Some(2)).unwrap().pixel_index(20, 20), Some(4));
        assert_eq!(doc.preview_through(Some(3)).unwrap().pixel_index(20, 20), Some(4));
        assert_eq!(doc.preview().unwrap().indices(), doc.preview_through(None).unwrap().indices());
        assert!(matches!(doc.preview_through(Some(4)), Err(RipDocumentError::InvalidIndex { .. })));
        assert_eq!(doc.revision(), revision);
    }

    #[test]
    fn preview_through_preserved_mixed_stream_keeps_ansi_and_stops_at_command() {
        let mut doc = RipDocument::from_bytes(b"!|c0C|X0A0A|c04|X0K0K\r\n\x1b[31m").unwrap();
        assert_eq!(doc.preserved_commands(), 4);
        assert_eq!(doc.preview_through(Some(1)).unwrap().pixel_index(10, 10), Some(12));
        assert_ne!(doc.preview_through(Some(1)).unwrap().pixel_index(20, 20), Some(4));
        assert_eq!(doc.preview_through(Some(3)).unwrap().pixel_index(20, 20), Some(4));
        doc.append(RipCommand::Pixel { x: 30, y: 30 });
        assert_ne!(doc.preview_through(Some(3)).unwrap().pixel_index(30, 30), Some(4));
        assert_eq!(doc.preview_through(Some(4)).unwrap().pixel_index(30, 30), Some(4));
    }

    #[test]
    fn invalid_parameter_edits_leave_the_command_and_history_unchanged() {
        let mut doc = RipDocument::new();
        doc.append(RipCommand::Pixel { x: 4, y: 5 });
        let revision = doc.revision();
        assert!(doc.replace(0, RipCommand::Pixel { x: 1296, y: 5 }).is_err());
        assert_eq!(doc.commands(), &[RipCommand::Pixel { x: 4, y: 5 }]);
        assert_eq!(doc.revision(), revision);
    }

    #[test]
    fn editing_a_command_replays_and_undoes_as_one_change() {
        let mut doc = RipDocument::new();
        doc.append_many(vec![RipCommand::Color { c: 4 }, RipCommand::Pixel { x: 10, y: 10 }]);
        let revision = doc.revision();
        doc.replace(1, RipCommand::Pixel { x: 20, y: 20 }).unwrap();
        assert_eq!(doc.revision(), revision + 1);
        assert_eq!(doc.preview().unwrap().pixel_index(20, 20), Some(4));
        assert_ne!(doc.preview().unwrap().pixel_index(10, 10), Some(4));
        assert!(doc.undo());
        assert_eq!(doc.preview().unwrap().pixel_index(10, 10), Some(4));
        assert!(doc.redo());
        assert_eq!(doc.preview().unwrap().pixel_index(20, 20), Some(4));
    }

    #[test]
    fn errors_and_save_state() {
        assert_eq!(RipDocument::new().to_bytes().unwrap(), b"");
        assert!(matches!(RipDocument::from_bytes(b"hello"), Err(RipDocumentError::NonCommandText)));
        assert!(matches!(RipDocument::from_bytes(b"!|c??|X0A0A\r\n"), Err(RipDocumentError::DroppedCommand)));
        let mixed = RipDocument::from_bytes(b"!|c04\r\n\x1b[31m").unwrap();
        assert_eq!(mixed.preserved_commands(), 1);
        let unknown = RipDocument::from_bytes(b"!|?opaque\r\n").unwrap();
        assert!(matches!(unknown.commands(), [RipCommand::Unsupported { data, .. }] if data == "opaque"));
        assert_eq!(RipDocument::from_bytes(&unknown.to_bytes().unwrap()).unwrap().commands(), unknown.commands());
        let mut doc = RipDocument::new();
        assert!(matches!(doc.save_current(false), Err(RipDocumentError::MissingPath)));
        assert!(matches!(doc.delete(0), Err(RipDocumentError::InvalidIndex { .. })));
        assert!(!doc.can_undo());
        doc.add_command(RipCommand::Pixel { x: 1, y: 2 });
        assert!(doc.modified());
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("scene.rip");
        doc.save(&path, false).unwrap();
        assert!(!doc.modified());
        assert_eq!(doc.path(), Some(path.as_path()));
        assert_eq!(RipDocument::load(&path).unwrap().commands(), doc.commands());
        assert_eq!(doc.remove_command(0).unwrap(), RipCommand::Pixel { x: 1, y: 2 });
        assert!(doc.modified());
        assert!(doc.undo());
        assert!(!doc.modified());
    }

    #[test]
    fn external_changes_are_not_overwritten_and_recovery_restores_commands() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("scene.rip");
        std::fs::write(&path, b"!|c04|X0A0K\r\n").unwrap();
        let mut doc = RipDocument::open(&path).unwrap();
        doc.append(RipCommand::Pixel { x: 15, y: 20 });
        let snapshot = doc.recovery_snapshot().unwrap();
        let recovered = RipDocument::from_recovery(&snapshot).unwrap();
        assert!(recovered.is_dirty());
        assert_eq!(recovered.commands(), doc.commands());
        std::fs::write(&path, b"!|c02|X0A0K\r\n").unwrap();
        assert!(doc.save_current(false).is_err(), "an externally modified scene needs overwrite confirmation");
        assert_eq!(std::fs::read(&path).unwrap(), b"!|c02|X0A0K\r\n");
        doc.save_current(true).unwrap();
        assert!(!doc.is_dirty());
        assert_eq!(RipDocument::open(&path).unwrap().commands(), doc.commands());
    }

    #[test]
    fn bundled_rip_drawing_can_be_edited_and_round_tripped() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../icy_engine/tests/output/rip/files/to-rip.rip");
        let mut drawing = RipDocument::open(&path).unwrap();
        assert!(!drawing.commands().is_empty());
        drawing.append(RipCommand::Color { c: 4 });
        drawing.append(RipCommand::Pixel { x: 42, y: 53 });
        let reloaded = RipDocument::from_bytes(&drawing.to_bytes().unwrap()).unwrap();
        assert_eq!(reloaded.commands(), drawing.commands());
    }

    #[test]
    fn commands_after_terminator_round_trip() {
        let mut document = RipDocument::from_bytes(b"!|c04|#\r\n").unwrap();
        document.append_many(vec![RipCommand::Color { c: 3 }, RipCommand::Pixel { x: 10, y: 20 }]);
        let bytes = document.to_bytes().unwrap();
        assert_eq!(RipDocument::from_bytes(&bytes).unwrap().commands(), document.commands());
    }

    #[test]
    fn mixed_rip_and_ansi_preserves_original_stream_when_drawing() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../icy_engine/tests/output/rip/files/paleo.rip");
        let bytes = std::fs::read(path).unwrap();
        let mut document = RipDocument::from_bytes(&bytes).unwrap();
        assert!(document.preserved_commands() > 0);
        assert_eq!(document.to_bytes().unwrap(), bytes);
        assert!(matches!(document.delete(0), Err(RipDocumentError::PreservedCommand { .. })));
        document.append_many(vec![RipCommand::Color { c: 4 }, RipCommand::Pixel { x: 10, y: 20 }]);
        assert_eq!(document.render_current().unwrap().screen()[20 * 640 + 10], 4);
        let saved = document.to_bytes().unwrap();
        assert!(saved.starts_with(&bytes));
        assert_eq!(RipDocument::from_bytes(&saved).unwrap().commands(), document.commands());
        let restored = RipDocument::from_recovery(&document.recovery_snapshot().unwrap()).unwrap();
        assert!(restored.is_dirty());
        assert_eq!(restored.to_bytes().unwrap(), saved);
        assert_eq!(restored.render_current().unwrap().screen()[20 * 640 + 10], 4);
    }

    #[test]
    fn grouped_add_is_one_undo_step() {
        let mut document = RipDocument::new();
        document.add_commands(Vec::new());
        assert_eq!(document.revision(), 0);
        assert!(!document.can_undo());

        let group = vec![RipCommand::Color { c: 4 }, RipCommand::Pixel { x: 10, y: 20 }];
        document.add_commands(group.clone());
        assert_eq!(document.commands(), group.as_slice());
        assert_eq!(document.revision(), 1);
        assert!(document.undo());
        assert!(document.commands().is_empty());
        assert!(!document.can_undo());
        assert!(document.redo());
        assert_eq!(document.commands(), group.as_slice());
    }
}
