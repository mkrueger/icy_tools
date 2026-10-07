//! Lossless, ordered SkyPix documents, independent of the editor frontend.
//!
//! Imported items retain their original wire spelling. ANSI, unknown commands and
//! malformed sequences are preserved as raw bytes, not silently normalized away.

use std::{
    fmt,
    path::{Path, PathBuf},
};

use icy_engine::amiga_screen_buffer::AmigaScreenBuffer;
use icy_engine::{BufferType, EditableScreen, GraphicsType, Palette, Screen, ScreenSink, SKYPIX_PALETTE};
use icy_parser_core::{CommandParser, CommandSink, ErrorLevel, ParseError, SkypixCommand, SkypixParser, TerminalCommand};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SkypixDocumentError {
    #[error("item index {index} is out of bounds (length {len})")]
    InvalidIndex { index: usize, len: usize },
    #[error("SkyPix command cannot be represented safely: {0}")]
    InvalidCommand(String),
    #[error("character {0:?} cannot be represented as SkyPix CP437 text")]
    InvalidText(char),
    #[error("text items cannot contain escape sequences; use Raw for ANSI bytes")]
    EscapeInText,
    #[error("raw items cannot contain recognized SkyPix commands")]
    CommandInRaw,
    #[error("edit would join or complete an escape sequence across item boundaries")]
    JoinedSequence,
    #[error("SkyPix preview failed on malformed input")]
    PreviewPanic,
    #[error("no path is associated with this SkyPix document")]
    MissingPath,
    #[error("SkyPix file I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("Could not save SkyPix drawing: {0}")]
    Save(String),
}

pub type SkypixResult<T> = Result<T, SkypixDocumentError>;

#[derive(Clone, Debug, PartialEq)]
pub enum SkypixItem {
    Command(SkypixCommand),
    Text(Vec<u8>),
    Raw(Vec<u8>),
}

impl SkypixItem {
    /// Validation is performed atomically when the item is added to a document.
    pub fn command(command: SkypixCommand) -> Self {
        Self::Command(command)
    }

    pub fn text(text: &str) -> SkypixResult<Self> {
        let mut bytes = Vec::new();
        for ch in text.chars() {
            if ch == '\x1b' {
                return Err(SkypixDocumentError::EscapeInText);
            }
            let code = if ch.is_ascii() {
                ch as u8
            } else {
                let code = BufferType::CP437.try_convert_from_unicode(ch).ok_or(SkypixDocumentError::InvalidText(ch))? as u8;
                // These graphical CP437 glyphs would execute parser controls,
                // rather than print the requested character.
                if matches!(code, 7..=13 | 27 | 127) {
                    return Err(SkypixDocumentError::InvalidText(ch));
                }
                code
            };
            bytes.push(code);
        }
        Ok(Self::Text(bytes))
    }

    pub fn as_command(&self) -> Option<&SkypixCommand> {
        match self {
            Self::Command(command) => Some(command),
            _ => None,
        }
    }

    pub fn as_text(&self) -> Option<&[u8]> {
        match self {
            Self::Text(bytes) => Some(bytes),
            _ => None,
        }
    }

    /// Decode CP437 glyphs while preserving bytes interpreted as terminal controls.
    pub fn decoded_text(&self) -> Option<String> {
        self.as_text().map(|bytes| {
            bytes
                .iter()
                .map(|&byte| {
                    if matches!(byte, 7..=13 | 27 | 127) {
                        char::from(byte)
                    } else {
                        BufferType::CP437.convert_to_unicode(char::from(byte))
                    }
                })
                .collect()
        })
    }
}

impl fmt::Display for SkypixItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Command(command) => write!(f, "{command:?}"),
            Self::Text(bytes) => write!(f, "Text ({} bytes)", bytes.len()),
            Self::Raw(bytes) => write!(f, "Preserved ANSI / raw ({} bytes)", bytes.len()),
        }
    }
}

#[derive(Default)]
struct Collector {
    commands: Vec<SkypixCommand>,
    errors: bool,
    output: bool,
}

impl CommandSink for Collector {
    fn print(&mut self, bytes: &[u8]) {
        self.output |= !bytes.is_empty();
    }
    fn emit(&mut self, _: TerminalCommand) {
        self.output = true;
    }
    fn emit_skypix(&mut self, command: SkypixCommand) {
        self.commands.push(command);
    }
    fn report_error(&mut self, _: ParseError, _: ErrorLevel) {
        self.errors = true;
    }
}

fn collect(bytes: &[u8]) -> Option<Collector> {
    std::panic::catch_unwind(|| {
        let mut collector = Collector::default();
        SkypixParser::new().parse(bytes, &mut collector);
        collector
    })
    .ok()
}

/// A token is either ordinary text or one complete (or EOF-truncated) escape
/// sequence. Follow the parser's grammar, including its string terminators.
fn tokens(bytes: &[u8]) -> Vec<(usize, usize, bool)> {
    let mut result = Vec::new();
    let mut start = 0;
    while start < bytes.len() {
        if bytes[start] != 27 {
            let end = bytes[start..].iter().position(|&b| b == 27).map_or(bytes.len(), |n| start + n);
            result.push((start, end, false));
            start = end;
            continue;
        }
        let mut end = (start + 2).min(bytes.len());
        if bytes.get(start + 1) == Some(&b'[') {
            end = (start + 3).min(bytes.len());
            if let Some(&first) = bytes.get(start + 2) {
                if first.is_ascii_digit() || first == b'-' {
                    let mut cursor = start + 2;
                    while cursor < bytes.len() && (bytes[cursor].is_ascii_digit() || matches!(bytes[cursor], b'-' | b';')) {
                        cursor += 1;
                    }
                    end = (cursor + 1).min(bytes.len());
                    if bytes.get(cursor) == Some(&b'!') {
                        // Parse the header numerically just as the parser does:
                        // omitted parameters are ignored, and arithmetic wraps.
                        let mut params = Vec::new();
                        let mut value = 0i32;
                        let mut negative = false;
                        let mut present = false;
                        for &byte in &bytes[start + 2..=cursor] {
                            match byte {
                                b'0'..=b'9' => {
                                    value = value.wrapping_mul(10).wrapping_add(i32::from(byte - b'0'));
                                    present = true;
                                }
                                b'-' => {
                                    negative = true;
                                    present = true;
                                }
                                _ => {
                                    if present {
                                        params.push(if negative { value.wrapping_neg() } else { value });
                                    }
                                    value = 0;
                                    negative = false;
                                    present = false;
                                }
                            }
                        }
                        let string = matches!(params.first(), Some(0 | 16)) || (params.first() == Some(&10) && params.get(1) != Some(&0));
                        if string {
                            end = bytes[end..].iter().position(|&b| b == b'!').map_or(bytes.len(), |n| end + n + 1);
                        }
                    }
                }
            }
        }
        result.push((start, end, true));
        start = end;
    }
    result
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Snapshot {
    items: Vec<SkypixItem>,
    original: Vec<Option<Vec<u8>>>,
}

fn import(bytes: &[u8]) -> Snapshot {
    let mut snapshot = Snapshot::default();
    for (start, end, escape) in tokens(bytes) {
        let source = &bytes[start..end];
        let item = if escape {
            match collect(source) {
                Some(mut collector) if !collector.errors && collector.commands.len() == 1 => SkypixItem::Command(collector.commands.remove(0)),
                _ => SkypixItem::Raw(source.to_vec()),
            }
        } else {
            SkypixItem::Text(source.to_vec())
        };
        snapshot.items.push(item);
        snapshot.original.push(Some(source.to_vec()));
    }
    snapshot
}

/// Detect a complete, recognized SkyPix command in a mixed byte stream.
/// Ordinary ANSI, unknown commands and malformed or incomplete sequences alone
/// do not select the SkyPix editor. Parameters must satisfy the checked encoder;
/// extra parameters silently discarded by the terminal parser do not qualify.
pub fn contains_skypix_commands(bytes: &[u8]) -> bool {
    tokens(bytes).into_iter().any(|(start, end, escape)| {
        if !escape {
            return false;
        }
        let source = &bytes[start..end];
        let Some(collector) = collect(source).filter(|collector| !collector.errors && collector.commands.len() == 1) else {
            return false;
        };
        let command = &collector.commands[0];
        let Ok(canonical) = encode_command(command) else {
            return false;
        };
        let header = |bytes: &[u8]| {
            let end = bytes.iter().position(|&byte| byte == b'!')?;
            let header = std::str::from_utf8(bytes.get(2..end)?).ok()?;
            let mut count = 0;
            for parameter in header.split(';') {
                parameter.parse::<i32>().ok()?;
                count += 1;
            }
            Some(count)
        };
        match (header(source), header(&canonical)) {
            (Some(source), Some(canonical)) => {
                source == canonical || (source == 1 && matches!(command, SkypixCommand::SetPenA { .. } | SkypixCommand::SetPenB { .. }))
            }
            _ => false,
        }
    })
}

/// Conservative bounds prevent overflow, excessive raster loops and brush
/// indexing faults in the existing graphics engine. Imported commands outside
/// these bounds remain lossless, but are not executed by the preview.
fn validate_command(command: &SkypixCommand) -> SkypixResult<()> {
    use SkypixCommand::*;
    let point = |x: i32, y: i32| (0..640).contains(&x) && (0..200).contains(&y);
    let extent = |width: i32, height: i32| (0..=640).contains(&width) && (0..=200).contains(&height);
    let string = |text: &str| text.chars().all(|ch| ch != '!' && u32::from(ch) <= 255);
    let valid = match command {
        Comment { text } => string(text),
        SetPixel { x, y } | DrawLine { x, y } | MovePen { x, y } | PositionCursor { x, y } | AreaFill { x, y, .. } => point(*x, *y),
        RectangleFill { x1, y1, x2, y2 } => point(*x1, *y1) && point(*x2, *y2),
        Ellipse { x, y, a, b } | FilledEllipse { x, y, a, b } => point(*x, *y) && extent(*a, *b),
        GrabBrush { x1, y1, width, height } => point(*x1, *y1) && extent(*width, *height) && x1 + width <= 640 && y1 + height <= 200,
        UseBrush {
            src_x,
            src_y,
            dst_x,
            dst_y,
            width,
            height,
            minterm,
            mask,
        } => point(*src_x, *src_y) && point(*dst_x, *dst_y) && extent(*width, *height) && (0..=255).contains(minterm) && (0..=255).contains(mask),
        PlaySample { speed, start, end, loops } => *speed >= 0 && *start >= 0 && *end >= *start && *loops >= 0,
        SetFont { size, name } => (1..=200).contains(size) && name.is_ascii() && string(name),
        NewPalette { colors } => colors.len() == 16 && colors.iter().all(|c| (0..=0xfff).contains(c)),
        Delay { jiffies } => *jiffies >= 0,
        SetPenA { color } | SetPenB { color } => (0..16).contains(color),
        CrcTransfer { width, height, filename, .. } => extent(*width, *height) && string(filename),
        ControllerReturn { c, x, y } => (1..=4).contains(c) && *x >= 0 && *y >= 0,
        DefineGadget { num, cmd, x1, y1, x2, y2 } => (1..=20).contains(num) && *cmd >= 0 && point(*x1, *y1) && point(*x2, *y2),
        ResetFont | ResetPalette | SetDisplayMode { .. } | EndSkypix => true,
    };
    if valid {
        Ok(())
    } else {
        Err(SkypixDocumentError::InvalidCommand(format!("{command:?}")))
    }
}

fn encode_command(command: &SkypixCommand) -> SkypixResult<Vec<u8>> {
    validate_command(command)?;
    use SkypixCommand::*;
    let mut bytes = match command {
        Comment { .. } => b"\x1b[0!".to_vec(),
        SetPixel { x, y } => format!("\x1b[1;{x};{y}!").into_bytes(),
        DrawLine { x, y } => format!("\x1b[2;{x};{y}!").into_bytes(),
        AreaFill { mode, x, y } => format!("\x1b[3;{};{x};{y}!", i32::from(*mode)).into_bytes(),
        RectangleFill { x1, y1, x2, y2 } => format!("\x1b[4;{x1};{y1};{x2};{y2}!").into_bytes(),
        Ellipse { x, y, a, b } => format!("\x1b[5;{x};{y};{a};{b}!").into_bytes(),
        GrabBrush { x1, y1, width, height } => format!("\x1b[6;{x1};{y1};{width};{height}!").into_bytes(),
        UseBrush {
            src_x,
            src_y,
            dst_x,
            dst_y,
            width,
            height,
            minterm,
            mask,
        } => format!("\x1b[7;{src_x};{src_y};{dst_x};{dst_y};{width};{height};{minterm};{mask}!").into_bytes(),
        MovePen { x, y } => format!("\x1b[8;{x};{y}!").into_bytes(),
        PlaySample { speed, start, end, loops } => format!("\x1b[9;{speed};{start};{end};{loops}!").into_bytes(),
        SetFont { size, .. } => format!("\x1b[10;{size}!").into_bytes(),
        ResetFont => b"\x1b[10;0!".to_vec(),
        NewPalette { colors } => format!("\x1b[11;{}!", colors.iter().map(i32::to_string).collect::<Vec<_>>().join(";")).into_bytes(),
        ResetPalette => b"\x1b[12!".to_vec(),
        FilledEllipse { x, y, a, b } => format!("\x1b[13;{x};{y};{a};{b}!").into_bytes(),
        Delay { jiffies } => format!("\x1b[14;{jiffies}!").into_bytes(),
        SetPenA { color } => format!("\x1b[15;{color}!").into_bytes(),
        CrcTransfer { mode, width, height, .. } => format!("\x1b[16;{};{width};{height}!", i32::from(*mode)).into_bytes(),
        SetDisplayMode { mode } => format!("\x1b[17;{}!", i32::from(*mode)).into_bytes(),
        SetPenB { color } => format!("\x1b[18;{color}!").into_bytes(),
        PositionCursor { x, y } => format!("\x1b[19;{x};{y}!").into_bytes(),
        ControllerReturn { c, x, y } => format!("\x1b[21;{c};{x};{y}!").into_bytes(),
        DefineGadget { num, cmd, x1, y1, x2, y2 } => format!("\x1b[22;{num};{cmd};{x1};{y1};{x2};{y2}!").into_bytes(),
        EndSkypix => b"\x1b[99!".to_vec(),
    };
    let string = match command {
        Comment { text } => Some(text),
        SetFont { name, .. } => Some(name),
        CrcTransfer { filename, .. } => Some(filename),
        _ => None,
    };
    if let Some(string) = string {
        bytes.extend(string.chars().map(|ch| ch as u8));
        bytes.push(b'!');
    }
    if !collect(&bytes).is_some_and(|sink| !sink.errors && sink.commands.as_slice() == std::slice::from_ref(command)) {
        return Err(SkypixDocumentError::InvalidCommand(format!("{command:?}")));
    }
    Ok(bytes)
}

fn encode(snapshot: &Snapshot) -> SkypixResult<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut boundaries = Vec::new();
    for (item, original) in snapshot.items.iter().zip(&snapshot.original) {
        if let Some(original) = original {
            bytes.extend_from_slice(original);
        } else {
            match item {
                SkypixItem::Command(command) => bytes.extend(encode_command(command)?),
                SkypixItem::Text(text) => {
                    if text.contains(&27) {
                        return Err(SkypixDocumentError::EscapeInText);
                    }
                    bytes.extend_from_slice(text);
                }
                SkypixItem::Raw(raw) => {
                    if !import(raw).items.iter().all(|item| item.as_command().is_none()) {
                        return Err(SkypixDocumentError::CommandInRaw);
                    }
                    bytes.extend_from_slice(raw);
                }
            }
        }
        boundaries.push(bytes.len());
    }
    // An incomplete escape/string may exist at EOF, but no edit can make the
    // next item become part of that sequence (even if it emits no command).
    for (start, end, escape) in tokens(&bytes) {
        let next = boundaries.partition_point(|&boundary| boundary <= start);
        if escape && boundaries.get(next).is_some_and(|&boundary| boundary < end) {
            return Err(SkypixDocumentError::JoinedSequence);
        }
    }
    Ok(bytes)
}

pub struct SkypixPreview {
    screen: AmigaScreenBuffer,
    warnings: Vec<String>,
}

impl SkypixPreview {
    pub fn screen(&self) -> &AmigaScreenBuffer {
        &self.screen
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
        self.indices()
            .iter()
            .flat_map(|&index| {
                let (r, g, b) = self.screen.palette().rgb(u32::from(index));
                [r, g, b, 255]
            })
            .collect()
    }
    /// Skipped unsafe/external operations and unsupported preview approximations.
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }
}

struct PreviewSink<'a, 'b> {
    inner: &'a mut ScreenSink<'b>,
    warnings: &'a mut Vec<String>,
}

impl CommandSink for PreviewSink<'_, '_> {
    fn print(&mut self, bytes: &[u8]) {
        self.inner.print(bytes);
    }
    fn emit(&mut self, command: TerminalCommand) {
        // A preview never rings the terminal bell or transmits user input.
        if !matches!(command, TerminalCommand::Bell) {
            self.inner.emit(command);
        }
    }
    fn emit_skypix(&mut self, command: SkypixCommand) {
        if validate_command(&command).is_err()
            || matches!(
                command,
                SkypixCommand::PlaySample { .. }
                    | SkypixCommand::CrcTransfer { .. }
                    | SkypixCommand::ControllerReturn { .. }
                    | SkypixCommand::DefineGadget { .. }
                    | SkypixCommand::Delay { .. }
            )
        {
            self.warnings.push(format!("Not previewed: {command:?}"));
        } else {
            if let SkypixCommand::UseBrush { minterm, mask, .. } = &command {
                if *minterm != 192 || *mask != 255 {
                    self.warnings.push(format!(
                        "Brush preview uses copy/all planes; minterm {minterm} and mask {mask} are not implemented"
                    ));
                }
            }
            if let SkypixCommand::SetFont { size, name } = &command {
                if icy_engine::get_amiga_font_by_name(name, *size).is_none() {
                    self.warnings.push(format!("Font unavailable; using default font: {name} ({size})"));
                }
            }
            self.inner.emit_skypix(command);
        }
    }
    fn report_error(&mut self, error: ParseError, _: ErrorLevel) {
        self.warnings.push(error.to_string());
    }
}

/// Snapshot undo/redo includes original source bytes, not just parsed commands.
pub struct SkypixDocument {
    current: Snapshot,
    saved: Snapshot,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    revision: u64,
    path: Option<PathBuf>,
    baseline: Option<Vec<u8>>,
    recovered_disk: Option<crate::recovery::Fingerprint>,
    recovered_unsaved: bool,
}

impl Default for SkypixDocument {
    fn default() -> Self {
        Self::new()
    }
}

impl SkypixDocument {
    pub fn new() -> Self {
        Self {
            current: Snapshot::default(),
            saved: Snapshot::default(),
            undo: Vec::new(),
            redo: Vec::new(),
            revision: 0,
            path: None,
            baseline: Some(Vec::new()),
            recovered_disk: None,
            recovered_unsaved: false,
        }
    }
    pub fn from_bytes(bytes: &[u8]) -> SkypixResult<Self> {
        let current = import(bytes);
        Ok(Self {
            saved: current.clone(),
            current,
            baseline: Some(bytes.to_vec()),
            ..Self::new()
        })
    }
    pub fn load(path: &Path) -> SkypixResult<Self> {
        let mut document = Self::from_bytes(&std::fs::read(path)?)?;
        document.path = Some(path.to_path_buf());
        Ok(document)
    }
    pub fn items(&self) -> &[SkypixItem] {
        &self.current.items
    }
    /// An isolated draft retaining source spelling but no file path or undo history.
    pub fn draft_copy(&self) -> Self {
        Self {
            current: self.current.clone(),
            saved: self.current.clone(),
            revision: self.revision,
            baseline: self.baseline.clone(),
            ..Self::new()
        }
    }

    /// Exact imported bytes or checked canonical bytes for one item.
    pub fn item_source(&self, index: usize) -> SkypixResult<Vec<u8>> {
        let item = self.current.items.get(index).ok_or(SkypixDocumentError::InvalidIndex {
            index,
            len: self.items().len(),
        })?;
        if let Some(source) = &self.current.original[index] {
            return Ok(source.clone());
        }
        match item {
            SkypixItem::Command(command) => encode_command(command),
            SkypixItem::Text(bytes) | SkypixItem::Raw(bytes) => Ok(bytes.clone()),
        }
    }
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
    pub fn recovery_snapshot(&self) -> Result<crate::recovery::Snapshot, String> {
        Ok(crate::recovery::Snapshot {
            kind: crate::recovery::RecoveryKind::Skypix,
            path: self.path.clone(),
            disk: self
                .path
                .as_ref()
                .and_then(|_| self.baseline.as_deref().map(crate::recovery::Fingerprint::of).or(self.recovered_disk)),
            payload: self.to_bytes().map_err(|error| error.to_string())?,
        })
    }
    pub fn from_recovery(snapshot: &crate::recovery::Snapshot) -> Result<Self, String> {
        if snapshot.kind != crate::recovery::RecoveryKind::Skypix {
            return Err("Recovery snapshot is not a SkyPix document".into());
        }
        let mut document = Self::from_bytes(&snapshot.payload).map_err(|error| error.to_string())?;
        document.path = snapshot.path.clone();
        document.baseline = snapshot.path.as_deref().zip(snapshot.disk).and_then(|(path, disk)| disk.read_matching(path));
        document.saved = document.baseline.as_deref().map(import).unwrap_or_default();
        document.recovered_disk = snapshot.disk;
        document.recovered_unsaved = true;
        Ok(document)
    }
    pub fn modified(&self) -> bool {
        self.recovered_unsaved || self.current != self.saved
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn undo(&mut self) -> bool {
        if let Some(previous) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.current, previous));
            self.revision = self.revision.wrapping_add(1);
            true
        } else {
            false
        }
    }
    pub fn redo(&mut self) -> bool {
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.current, next));
            self.revision = self.revision.wrapping_add(1);
            true
        } else {
            false
        }
    }
    pub fn to_bytes(&self) -> SkypixResult<Vec<u8>> {
        encode(&self.current)
    }
    pub fn save(&mut self) -> SkypixResult<()> {
        let path = self.path.clone().ok_or(SkypixDocumentError::MissingPath)?;
        self.save_as(&path, false)
    }
    pub fn save_as(&mut self, path: &Path, overwrite: bool) -> SkypixResult<()> {
        if !overwrite && self.baseline.is_none() && self.path.as_deref().is_some_and(|source| crate::files::same_file(source, path)) {
            return Err(SkypixDocumentError::Save(
                "The original file no longer matches this recovery snapshot. Use Save As with a different file or confirm replacement.".into(),
            ));
        }
        let bytes = self.to_bytes()?;
        crate::files::save_bytes(path, &bytes, self.path.as_deref().zip(self.baseline.as_deref()), overwrite).map_err(SkypixDocumentError::Save)?;
        self.path = Some(path.to_path_buf());
        self.baseline = Some(bytes);
        self.recovered_disk = None;
        self.recovered_unsaved = false;
        self.saved = self.current.clone();
        Ok(())
    }
    fn index(&self, index: usize) -> SkypixResult<()> {
        if index >= self.items().len() {
            Err(SkypixDocumentError::InvalidIndex {
                index,
                len: self.items().len(),
            })
        } else {
            Ok(())
        }
    }
    fn commit(&mut self, next: Snapshot) -> SkypixResult<()> {
        encode(&next)?;
        if next != self.current {
            self.undo.push(std::mem::replace(&mut self.current, next));
            self.redo.clear();
            self.revision = self.revision.wrapping_add(1);
        }
        Ok(())
    }
    pub fn append(&mut self, items: Vec<SkypixItem>) -> SkypixResult<()> {
        let mut next = self.current.clone();
        next.original.extend(std::iter::repeat_n(None, items.len()));
        next.items.extend(items);
        self.commit(next)
    }
    /// Replace the ordered scene as one validated undo step. Retained or moved
    /// items keep their imported wire spelling; newly introduced items are checked.
    pub fn set_items(&mut self, items: Vec<SkypixItem>) -> SkypixResult<()> {
        if items == self.current.items {
            return Ok(());
        }
        self.commit(self.snapshot_with_items(items))
    }
    fn snapshot_with_items(&self, items: Vec<SkypixItem>) -> Snapshot {
        let mut next = Snapshot {
            original: vec![None; items.len()],
            items,
        };
        let mut retained = vec![false; self.items().len()];
        let mut matched = vec![false; next.items.len()];
        for (index, item) in next.items.iter().enumerate() {
            if self.current.items.get(index) == Some(item) {
                next.original[index] = self.current.original[index].clone();
                retained[index] = true;
                matched[index] = true;
            }
        }
        for (index, item) in next.items.iter().enumerate() {
            if matched[index] {
                continue;
            }
            if let Some(previous) = self
                .current
                .items
                .iter()
                .enumerate()
                .position(|(previous, candidate)| !retained[previous] && candidate == item)
            {
                next.original[index] = self.current.original[previous].clone();
                retained[previous] = true;
            }
        }
        next
    }
    pub fn replace(&mut self, index: usize, item: SkypixItem) -> SkypixResult<()> {
        self.index(index)?;
        if self.current.items[index] == item {
            return Ok(());
        }
        let mut next = self.current.clone();
        next.items[index] = item;
        next.original[index] = None;
        self.commit(next)
    }
    pub fn delete(&mut self, index: usize) -> SkypixResult<()> {
        self.index(index)?;
        let mut next = self.current.clone();
        next.items.remove(index);
        next.original.remove(index);
        self.commit(next)
    }
    /// Move to an existing final index (after removal), keeping source spelling.
    pub fn move_item(&mut self, from: usize, to: usize) -> SkypixResult<()> {
        self.index(from)?;
        self.index(to)?;
        let mut next = self.current.clone();
        let item = next.items.remove(from);
        let original = next.original.remove(from);
        next.items.insert(to, item);
        next.original.insert(to, original);
        self.commit(next)
    }
    pub fn preview(&self) -> SkypixResult<SkypixPreview> {
        self.render(self.items().len())
    }
    /// Preview an edited scene without changing document bytes or undo history.
    pub fn preview_with_items(&self, items: Vec<SkypixItem>) -> SkypixResult<SkypixPreview> {
        let snapshot = self.snapshot_with_items(items);
        encode(&snapshot)?;
        Self::render_snapshot(&snapshot, snapshot.items.len())
    }
    /// Render the first `count` items. Zero is an empty scene; the item count is
    /// the full scene. This is a prefix length, not an inclusive item index.
    pub fn preview_through(&self, count: usize) -> SkypixResult<SkypixPreview> {
        if count > self.items().len() {
            return Err(SkypixDocumentError::InvalidIndex {
                index: count,
                len: self.items().len(),
            });
        }
        self.render(count)
    }
    fn render(&self, count: usize) -> SkypixResult<SkypixPreview> {
        Self::render_snapshot(&self.current, count)
    }
    fn render_snapshot(snapshot: &Snapshot, count: usize) -> SkypixResult<SkypixPreview> {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut screen = AmigaScreenBuffer::new(GraphicsType::Skypix).with_palette(Palette::from_slice(&SKYPIX_PALETTE));
            screen.caret_mut().set_foreground(2);
            let mut warnings = Vec::new();
            {
                let mut inner = ScreenSink::new(&mut screen);
                let mut sink = PreviewSink {
                    inner: &mut inner,
                    warnings: &mut warnings,
                };
                for index in 0..count {
                    let item = &snapshot.items[index];
                    let bytes = if let Some(original) = &snapshot.original[index] {
                        original.clone()
                    } else {
                        match item {
                            SkypixItem::Command(command) => encode_command(command)?,
                            SkypixItem::Text(bytes) | SkypixItem::Raw(bytes) => bytes.clone(),
                        }
                    };
                    if matches!(item, SkypixItem::Raw(_)) && !collect(&bytes).is_some_and(|collector| !collector.errors && collector.output) {
                        sink.warnings
                            .push(format!("Preserved malformed / unsupported / incomplete sequence at item {index}"));
                        continue;
                    }
                    SkypixParser::new().parse(&bytes, &mut sink);
                }
            }
            Ok(SkypixPreview { screen, warnings })
        }))
        .map_err(|_| SkypixDocumentError::PreviewPanic)?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use icy_parser_core::{CrcTransferMode, DisplayMode, FillMode};

    fn command(command: SkypixCommand) -> SkypixItem {
        SkypixItem::command(command)
    }

    #[test]
    fn mixed_stream_preserves_every_byte_and_source_spelling() {
        let bytes = b"hi\xdb\r\n\x1b[31mred\x1b[015;03!\x1b[1;12;13!\x1b[777;4!unknown\x1b[0!comment\xff!\x1b[10;8!topaz.font!\x1b[10;0!\x1b[1;";
        let mut doc = SkypixDocument::from_bytes(bytes).unwrap();
        assert_eq!(doc.to_bytes().unwrap(), bytes);
        assert!(!doc.modified());
        assert!(doc.items().iter().any(|item| matches!(item, SkypixItem::Raw(bytes) if bytes == b"\x1b[777;4!")));
        assert!(doc
            .items()
            .iter()
            .any(|item| matches!(item, SkypixItem::Command(SkypixCommand::SetFont { name, .. }) if name == "topaz.font")));
        assert!(doc.append(vec![command(SkypixCommand::ResetPalette)]).is_err());
        assert_eq!(doc.to_bytes().unwrap(), bytes);
        assert_eq!(doc.revision(), 0);
        assert!(!doc.can_undo());
        assert!(doc.preview().unwrap().warnings().len() >= 2);
    }

    #[test]
    fn all_command_variants_have_checked_identical_wire_roundtrips() {
        use SkypixCommand::*;
        let commands = vec![
            Comment { text: "hello \u{ff}".into() },
            SetPixel { x: 639, y: 199 },
            DrawLine { x: 0, y: 0 },
            AreaFill {
                mode: FillMode::Color,
                x: 12,
                y: 13,
            },
            RectangleFill { x1: 1, y1: 2, x2: 3, y2: 4 },
            Ellipse { x: 40, y: 40, a: 10, b: 20 },
            GrabBrush {
                x1: 1,
                y1: 2,
                width: 8,
                height: 9,
            },
            UseBrush {
                src_x: 0,
                src_y: 0,
                dst_x: 40,
                dst_y: 50,
                width: 8,
                height: 9,
                minterm: 192,
                mask: 255,
            },
            MovePen { x: 10, y: 20 },
            PlaySample {
                speed: 1000,
                start: 0,
                end: 100,
                loops: 1,
            },
            SetFont {
                size: 8,
                name: "topaz.font".into(),
            },
            ResetFont,
            NewPalette {
                colors: (0..16).map(|n| n * 0x111).collect(),
            },
            ResetPalette,
            FilledEllipse { x: 40, y: 50, a: 10, b: 20 },
            Delay { jiffies: 60 },
            SetPenA { color: 15 },
            CrcTransfer {
                mode: CrcTransferMode::IffBrush,
                width: 8,
                height: 9,
                filename: "brush.iff".into(),
            },
            SetDisplayMode {
                mode: DisplayMode::EightColors,
            },
            SetDisplayMode {
                mode: DisplayMode::SixteenColors,
            },
            SetPenB { color: 0 },
            PositionCursor { x: 10, y: 20 },
            ControllerReturn { c: 1, x: 10, y: 20 },
            DefineGadget {
                num: 1,
                cmd: 8,
                x1: 10,
                y1: 20,
                x2: 30,
                y2: 40,
            },
            EndSkypix,
        ];
        let mut doc = SkypixDocument::new();
        doc.append(commands.iter().cloned().map(command).collect()).unwrap();
        let reloaded = SkypixDocument::from_bytes(&doc.to_bytes().unwrap()).unwrap();
        assert_eq!(reloaded.items(), doc.items());
        assert_eq!(collect(&doc.to_bytes().unwrap()).unwrap().commands, commands);
        assert_eq!(doc.preview().unwrap().warnings().len(), 5);
    }

    #[test]
    fn text_is_cp437_not_utf8_and_renders_with_ansi() {
        let text = SkypixItem::text("Aé█\r\n").unwrap();
        assert_eq!(text, SkypixItem::Text(b"A\x82\xdb\r\n".to_vec()));
        assert_eq!(SkypixItem::text("☺").unwrap(), SkypixItem::Text(vec![1]));
        assert!(SkypixItem::text("😀").is_err());
        assert!(SkypixItem::text("←").is_err());
        assert!(SkypixItem::text("\x1b[1!").is_err());
        let mut doc = SkypixDocument::new();
        doc.append(vec![SkypixItem::Raw(b"\x1b[31m".to_vec()), text]).unwrap();
        let bytes = doc.to_bytes().unwrap();
        assert_eq!(bytes, b"\x1b[31mA\x82\xdb\r\n");
        assert_eq!(SkypixDocument::from_bytes(&bytes).unwrap().to_bytes().unwrap(), bytes);
        let preview = doc.preview().unwrap();
        assert!(preview.indices().contains(&3));
        assert_eq!(preview.width(), 640);
        assert_eq!(preview.height(), 200);
        assert_eq!(preview.rgba().len(), 640 * 200 * 4);
        assert_eq!(preview.pixel_index(640, 0), None);
    }

    #[test]
    fn transient_preview_preserves_source_revision_and_redo() {
        let bytes = b"\x1b[015;07!\x1b[2;10;10!\x1b[1;20;10!\x1b[777;4!";
        let mut doc = SkypixDocument::from_bytes(bytes).unwrap();
        doc.append(vec![command(SkypixCommand::SetPixel { x: 50, y: 50 })]).unwrap();
        doc.undo();
        let revision = doc.revision();
        let mut items = doc.items().to_vec();
        items[2] = command(SkypixCommand::DrawLine { x: 25, y: 20 });
        let preview = doc.preview_with_items(items.clone()).unwrap();
        assert_eq!(preview.pixel_index(25, 20), Some(7));
        assert_eq!(preview.pixel_index(15, 10), Some(0));
        assert!(!preview.warnings().is_empty());
        items.push(command(SkypixCommand::SetPixel { x: -1, y: 0 }));
        assert!(doc.preview_with_items(items).is_err());
        assert_eq!(doc.to_bytes().unwrap(), bytes);
        assert_eq!(doc.revision(), revision);
        assert!(!doc.modified());
        assert!(!doc.can_undo());
        assert!(doc.can_redo());
        doc.redo();
        assert_eq!(doc.preview().unwrap().pixel_index(50, 50), Some(7));
    }

    #[test]
    fn preview_prefix_pixels_and_palette_use_the_existing_engine() {
        let mut doc = SkypixDocument::new();
        let mut colors = vec![0; 16];
        colors[3] = 0x123;
        doc.append(vec![
            command(SkypixCommand::NewPalette { colors }),
            command(SkypixCommand::SetPenA { color: 3 }),
            command(SkypixCommand::SetPixel { x: 12, y: 13 }),
            command(SkypixCommand::RectangleFill {
                x1: 20,
                y1: 20,
                x2: 21,
                y2: 21,
            }),
        ])
        .unwrap();
        assert_eq!(doc.preview_through(0).unwrap().pixel_index(12, 13), Some(0));
        assert_eq!(doc.preview_through(2).unwrap().pixel_index(12, 13), Some(0));
        assert_eq!(doc.preview_through(3).unwrap().pixel_index(12, 13), Some(3));
        let preview = doc.preview().unwrap();
        assert_eq!(preview.pixel_rgba(12, 13), Some([51, 34, 17, 255]));
        assert_eq!(preview.pixel_index(20, 20), Some(3));
        assert_eq!(doc.preview_through(4).unwrap().indices(), preview.indices());
        assert!(doc.preview_through(5).is_err());
        assert!(SkypixDocument::new().preview_through(0).is_ok());
    }

    #[test]
    fn invalid_edits_are_atomic_and_imported_unsafe_commands_are_preserved() {
        let mut doc = SkypixDocument::from_bytes(b"\x1b[15;999!\x1b[5;1;1;2147483647;2!").unwrap();
        let initial = doc.to_bytes().unwrap();
        assert_eq!(doc.preview().unwrap().warnings().len(), 2);
        for cmd in [
            SkypixCommand::SetPenA { color: 16 },
            SkypixCommand::SetPixel { x: -1, y: 0 },
            SkypixCommand::NewPalette { colors: vec![0; 15] },
            SkypixCommand::GrabBrush {
                x1: 639,
                y1: 0,
                width: 8,
                height: 8,
            },
            SkypixCommand::SetFont {
                size: 0,
                name: "topaz.font".into(),
            },
            SkypixCommand::Comment { text: "bad!text".into() },
            SkypixCommand::SetFont {
                size: 8,
                name: "é.font".into(),
            },
        ] {
            assert!(doc.append(vec![SkypixItem::text("hello").unwrap(), command(cmd)]).is_err());
            assert_eq!(doc.to_bytes().unwrap(), initial);
            assert!(!doc.modified());
            assert!(!doc.can_undo());
            assert_eq!(doc.revision(), 0);
        }
        assert!(doc.replace(10, command(SkypixCommand::ResetFont)).is_err());
        assert!(doc.delete(10).is_err());
        assert!(doc.move_item(0, 10).is_err());
    }

    #[test]
    fn incomplete_strings_and_escapes_never_consume_appended_items() {
        for bytes in [
            b"hello\x1b".as_slice(),
            b"\x1b[",
            b"\x1b[0!open",
            b"\x1b[10;8!topaz.font",
            b"\x1b[16;1;8;8!brush",
        ] {
            let mut doc = SkypixDocument::from_bytes(bytes).unwrap();
            assert_eq!(doc.to_bytes().unwrap(), bytes);
            assert!(doc.append(vec![SkypixItem::text("!").unwrap()]).is_err());
            assert!(doc.append(vec![command(SkypixCommand::ResetFont)]).is_err());
            assert_eq!(doc.to_bytes().unwrap(), bytes);
            assert!(!doc.preview().unwrap().warnings().is_empty());
        }
        let mut doc = SkypixDocument::from_bytes(b"\x1b[1;2!text").unwrap();
        doc.append(vec![command(SkypixCommand::ResetFont)]).unwrap();
        assert_eq!(doc.to_bytes().unwrap(), b"\x1b[1;2!text\x1b[10;0!");
        assert!(doc.append(vec![SkypixItem::Raw(b"\x1b[12!".to_vec())]).is_err());
    }

    #[test]
    fn incomplete_raw_cannot_be_moved_before_another_item_and_errors_keep_redo() {
        let bytes = b"text\x1b[0!unfinished";
        let mut doc = SkypixDocument::from_bytes(bytes).unwrap();
        assert!(doc.move_item(1, 0).is_err());
        assert_eq!(doc.to_bytes().unwrap(), bytes);
        assert_eq!(doc.revision(), 0);
        doc.delete(1).unwrap();
        doc.append(vec![command(SkypixCommand::ResetPalette)]).unwrap();
        assert!(doc.undo());
        assert!(doc.can_redo());
        let revision = doc.revision();
        assert!(doc.replace(0, SkypixItem::Text(b"\x1b[12!".to_vec())).is_err());
        assert!(doc.can_redo());
        assert_eq!(doc.revision(), revision);
        assert_eq!(doc.to_bytes().unwrap(), b"text");
        assert!(doc.redo());
        assert_eq!(doc.to_bytes().unwrap(), b"text\x1b[12!");
    }

    #[test]
    fn binary_and_malformed_inputs_are_lossless_and_safe_to_preview() {
        for bytes in [
            (0..=255).collect::<Vec<u8>>(),
            b"\x1b[!tail\x1b[;bad\x1bXtext".to_vec(),
            b"\x1b[65536H\x1b[15;-2147483648!\x1b[1;999999999999999999999999999999999;4!".to_vec(),
            b"\x1b[10;8!incomplete\x1b[1;1;1".to_vec(),
        ] {
            let doc = SkypixDocument::from_bytes(&bytes).unwrap();
            assert_eq!(doc.to_bytes().unwrap(), bytes);
            doc.preview().unwrap();
        }
    }

    #[test]
    fn malformed_numeric_input_reports_errors_without_parser_panics() {
        for bytes in [b"\x1b[65536H".as_slice(), b"\x1b[1;65536H", b"\x1b[65536f", b"\x1b[65536G"] {
            let mut collector = Collector::default();
            SkypixParser::new().parse(bytes, &mut collector);
            assert!(collector.errors);
            assert!(!collector.output);
            let doc = SkypixDocument::from_bytes(bytes).unwrap();
            assert_eq!(doc.to_bytes().unwrap(), bytes);
            assert!(!doc.preview().unwrap().warnings().is_empty());
        }
        let bytes = b"\x1b[1;-2147483648;2!";
        let mut collector = Collector::default();
        SkypixParser::new().parse(bytes, &mut collector);
        assert!(!collector.errors);
        assert_eq!(collector.commands, vec![SkypixCommand::SetPixel { x: i32::MIN, y: 2 }]);
        let doc = SkypixDocument::from_bytes(bytes).unwrap();
        assert_eq!(doc.to_bytes().unwrap(), bytes);
        assert!(!doc.preview().unwrap().warnings().is_empty());
        let mut collector = Collector::default();
        SkypixParser::new().parse(b"\x1b[65535;65535H\x1b[65535G", &mut collector);
        assert!(!collector.errors);
        assert!(collector.output);
    }

    #[test]
    fn undo_redo_saved_state_and_editable_save_reload() {
        let directory = tempfile::Builder::new().prefix(".skypix-test-").tempdir_in(".").unwrap();
        let path = directory.path().join("art.sky");
        let original = b"text\x1b[015;03!\x1b[1;2;3!";
        std::fs::write(&path, original).unwrap();
        let mut doc = SkypixDocument::load(&path).unwrap();
        assert_eq!(doc.path(), Some(path.as_path()));
        doc.replace(2, command(SkypixCommand::SetPixel { x: 10, y: 11 })).unwrap();
        doc.move_item(0, 2).unwrap();
        doc.append(vec![SkypixItem::text("é").unwrap()]).unwrap();
        assert_eq!(doc.revision(), 3);
        assert!(doc.modified());
        doc.save().unwrap();
        let saved = doc.to_bytes().unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), saved);
        assert_eq!(SkypixDocument::load(&path).unwrap().to_bytes().unwrap(), saved);
        assert!(!doc.modified());
        assert_eq!(doc.revision(), 3);
        assert!(doc.undo());
        assert!(doc.modified());
        assert!(doc.redo());
        assert!(!doc.modified());
        assert!(doc.undo());
        assert!(doc.undo());
        assert!(doc.undo());
        assert_eq!(doc.to_bytes().unwrap(), original);
        assert!(!doc.undo());
        doc.delete(0).unwrap();
        assert!(!doc.can_redo());
        let other = directory.path().join("other.sky");
        doc.save_as(&other, false).unwrap();
        assert!(!doc.modified());
        assert_eq!(doc.path(), Some(other.as_path()));
        let unchanged_revision = doc.revision();
        doc.append(Vec::new()).unwrap();
        doc.move_item(0, 0).unwrap();
        assert_eq!(doc.revision(), unchanged_revision);
    }

    #[test]
    fn saving_does_not_overwrite_external_changes_or_mark_failed_saves_clean() {
        let directory = tempfile::Builder::new().prefix(".skypix-test-").tempdir_in(".").unwrap();
        let path = directory.path().join("art.sky");
        let mut doc = SkypixDocument::new();
        assert!(doc.save().is_err());
        doc.save_as(&path, false).unwrap();
        doc.append(vec![SkypixItem::text("change").unwrap()]).unwrap();
        std::fs::write(&path, b"external").unwrap();
        assert!(doc.save().is_err());
        assert!(doc.modified());
        assert_eq!(std::fs::read(&path).unwrap(), b"external");
        doc.save_as(&path, true).unwrap();
        assert!(!doc.modified());
        assert_eq!(std::fs::read(&path).unwrap(), b"change");
        doc.append(vec![SkypixItem::text(" next").unwrap()]).unwrap();
        doc.save().unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"change next");
        let other = directory.path().join("existing.sky");
        std::fs::write(&other, b"existing").unwrap();
        assert!(doc.save_as(&other, false).is_err());
        assert_eq!(doc.path(), Some(path.as_path()));
        assert_eq!(std::fs::read(&other).unwrap(), b"existing");
        doc.save_as(&other, true).unwrap();
        assert_eq!(doc.path(), Some(other.as_path()));
        assert_eq!(std::fs::read(&other).unwrap(), b"change next");
    }

    #[test]
    fn recovery_preserves_lossless_payload_path_and_matching_disk_baseline() {
        let directory = tempfile::Builder::new().prefix(".skypix-test-").tempdir_in(".").unwrap();
        let path = directory.path().join("art.sky");
        std::fs::write(&path, b"\x1b[015;03!original").unwrap();
        let mut doc = SkypixDocument::load(&path).unwrap();
        doc.append(vec![command(SkypixCommand::SetPixel { x: 12, y: 13 })]).unwrap();
        let snapshot = doc.recovery_snapshot().unwrap();
        assert_eq!(snapshot.kind, crate::recovery::RecoveryKind::Skypix);
        assert_eq!(snapshot.disk, Some(crate::recovery::Fingerprint::of(b"\x1b[015;03!original")));
        let mut recovered = SkypixDocument::from_recovery(&snapshot).unwrap();
        assert!(recovered.modified());
        assert_eq!(recovered.path(), Some(path.as_path()));
        assert_eq!(recovered.to_bytes().unwrap(), snapshot.payload);
        assert_eq!(recovered.recovery_snapshot().unwrap(), snapshot);
        assert!(!recovered.can_undo());
        recovered.save().unwrap();
        assert!(!recovered.modified());
        assert_eq!(std::fs::read(&path).unwrap(), snapshot.payload);
    }

    #[test]
    fn recovery_never_treats_a_changed_empty_disk_file_as_a_matching_baseline() {
        let directory = tempfile::Builder::new().prefix(".skypix-test-").tempdir_in(".").unwrap();
        let path = directory.path().join("art.sky");
        std::fs::write(&path, b"original").unwrap();
        let mut doc = SkypixDocument::load(&path).unwrap();
        doc.append(vec![SkypixItem::text("edited").unwrap()]).unwrap();
        let snapshot = doc.recovery_snapshot().unwrap();
        std::fs::write(&path, b"").unwrap();
        let mut recovered = SkypixDocument::from_recovery(&snapshot).unwrap();
        assert!(recovered.modified());
        assert_eq!(recovered.recovery_snapshot().unwrap(), snapshot);
        assert!(recovered.save().is_err());
        assert!(recovered.modified());
        assert_eq!(std::fs::read(&path).unwrap(), b"");
        recovered.save_as(&directory.path().join("recovered.sky"), false).unwrap();
        assert!(!recovered.modified());
    }

    #[test]
    fn empty_untitled_recovery_is_dirty_and_wrong_recovery_kind_is_rejected() {
        let mut snapshot = SkypixDocument::new().recovery_snapshot().unwrap();
        let recovered = SkypixDocument::from_recovery(&snapshot).unwrap();
        assert!(recovered.modified());
        assert!(recovered.path().is_none());
        assert!(recovered.to_bytes().unwrap().is_empty());
        snapshot.kind = crate::recovery::RecoveryKind::Ansi;
        assert!(SkypixDocument::from_recovery(&snapshot).is_err());
    }

    #[test]
    fn set_items_edits_groups_atomically_and_keeps_retained_wire_spelling() {
        let bytes = b"\x1b[015;03!\x1b[8;10;20!\x1b[2;30;40!text\x1b[777!";
        let mut doc = SkypixDocument::from_bytes(bytes).unwrap();
        let mut items = doc.items().to_vec();
        items[1] = command(SkypixCommand::MovePen { x: 20, y: 30 });
        items[2] = command(SkypixCommand::DrawLine { x: 40, y: 50 });
        doc.set_items(items.clone()).unwrap();
        assert_eq!(doc.revision(), 1);
        assert_eq!(doc.to_bytes().unwrap(), b"\x1b[015;03!\x1b[8;20;30!\x1b[2;40;50!text\x1b[777!");
        doc.set_items(items.clone()).unwrap();
        assert_eq!(doc.revision(), 1);
        assert!(doc.undo());
        assert_eq!(doc.to_bytes().unwrap(), bytes);
        assert!(!doc.can_undo());
        assert!(doc.can_redo());
        let revision = doc.revision();
        items[0] = command(SkypixCommand::SetPenA { color: 16 });
        assert!(doc.set_items(items).is_err());
        assert_eq!(doc.revision(), revision);
        assert_eq!(doc.to_bytes().unwrap(), bytes);
        assert!(doc.can_redo());
        let mut items = doc.items().to_vec();
        items.swap(0, 3);
        doc.set_items(items).unwrap();
        assert_eq!(doc.to_bytes().unwrap(), b"text\x1b[8;10;20!\x1b[2;30;40!\x1b[015;03!\x1b[777!");
    }

    #[test]
    fn text_accessors_decode_cp437_without_changing_terminal_controls() {
        let item = SkypixItem::Text(b"A\x82\xdb\x01\t\r\n\x7f".to_vec());
        assert_eq!(item.as_text(), Some(b"A\x82\xdb\x01\t\r\n\x7f".as_slice()));
        assert_eq!(item.decoded_text().as_deref(), Some("Aé█☺\t\r\n\x7f"));
        assert_eq!(SkypixItem::text(&item.decoded_text().unwrap()).unwrap(), item);
        assert!(command(SkypixCommand::ResetFont).as_text().is_none());
        assert!(SkypixItem::Raw(vec![1, 2, 3]).decoded_text().is_none());
    }

    #[test]
    fn fixture_corpus_import_export_is_lossless_and_renders_safely() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../icy_engine/tests/output/skypix/files");
        let mut directories = vec![root];
        let mut fixtures = Vec::new();
        while let Some(directory) = directories.pop() {
            for entry in std::fs::read_dir(&directory).unwrap() {
                let entry = entry.unwrap();
                let kind = entry.file_type().unwrap();
                if kind.is_dir() {
                    directories.push(entry.path());
                } else if kind.is_file()
                    && entry
                        .path()
                        .extension()
                        .and_then(|extension| extension.to_str())
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("ans"))
                {
                    fixtures.push(entry.path());
                }
            }
        }
        fixtures.sort();
        assert!(fixtures.len() >= 11, "the complete SkyPix fixture corpus must be present");
        for path in fixtures {
            let bytes = std::fs::read(&path).unwrap();
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| matches!(name, "line_test.ans" | "camera.ans"))
            {
                assert!(contains_skypix_commands(&bytes), "SkyPix detection: {}", path.display());
            }
            let doc = SkypixDocument::from_bytes(&bytes).unwrap();
            assert_eq!(doc.to_bytes().unwrap(), bytes, "lossless import: {}", path.display());
            let preview = doc.preview().unwrap_or_else(|error| panic!("preview {}: {error}", path.display()));
            assert_eq!((preview.width(), preview.height()), (640, 200), "{}", path.display());
            assert_eq!(preview.indices().len(), 640 * 200, "{}", path.display());
            assert_eq!(preview.rgba().len(), 640 * 200 * 4, "{}", path.display());
            assert_eq!(doc.to_bytes().unwrap(), bytes, "preview must not change source: {}", path.display());
            assert!(!doc.modified(), "{}", path.display());
        }
    }

    #[test]
    fn brush_dimensions_are_pixel_counts_including_a_single_edge_pixel() {
        let mut doc = SkypixDocument::new();
        doc.append(vec![
            command(SkypixCommand::SetPenA { color: 3 }),
            command(SkypixCommand::SetPixel { x: 639, y: 199 }),
            command(SkypixCommand::GrabBrush {
                x1: 639,
                y1: 199,
                width: 1,
                height: 1,
            }),
            command(SkypixCommand::UseBrush {
                src_x: 0,
                src_y: 0,
                dst_x: 10,
                dst_y: 20,
                width: 1,
                height: 1,
                minterm: 192,
                mask: 255,
            }),
        ])
        .unwrap();
        let preview = doc.preview().unwrap();
        assert_eq!(preview.pixel_index(639, 199), Some(3));
        assert_eq!(preview.pixel_index(10, 20), Some(3));
        assert_eq!(preview.pixel_index(11, 20), Some(0));
        assert_eq!(preview.pixel_index(10, 21), Some(0));
    }

    #[test]
    fn content_detection_requires_a_complete_recognized_skypix_escape() {
        for bytes in [
            b"ordinary ! text [1;2;3!".as_slice(),
            b"\x1b[31mANSI\x1b[0m\x1b[2J\x1b[1;1H",
            b"\x1b[777;1;2!",
            b"\x1b[99;123!",
            b"\x1b[!p",
            b"\x1b[31mANSI!\x1b[0m\r\n\x1b[!p",
            b"\x1b[777!\x1b[!p",
            b"\x1b[99;123!\x1b[!p",
            b"\x1b[1;2;3;4!",
            b"\x1b[1;2!",
            b"\x1b[3;7;10;20!",
            b"\x1b[10;8!topaz.font",
            b"\x1b[0!unfinished comment",
            b"\x1b[1;2;3",
            b"\x1b[65536H\x1b[1;-2147483648;2!",
        ] {
            assert!(!contains_skypix_commands(bytes), "{bytes:?}");
        }
        for bytes in [
            b"\x1b[1;2;3!".as_slice(),
            b"ANSI\x1b[31m\x1b[015;03!text",
            b"\x1b[777!unknown\x1b[12!",
            b"\x1b[0!a comment!",
            b"\x1b[10;8!topaz.font!",
            b"\x1b[10;0!",
            b"\x1b[9;100;0;10;0!",
            b"\x1b[15!",
            b"\x1b[18!",
            b"\x1b[99!",
        ] {
            assert!(contains_skypix_commands(bytes), "{bytes:?}");
        }
    }

    #[test]
    fn preview_warns_for_unsupported_blit_logic_without_changing_source() {
        let bytes = b"\x1b[7;0;0;10;20;1;1;90;15!";
        let doc = SkypixDocument::from_bytes(bytes).unwrap();
        let preview = doc.preview().unwrap();
        assert_eq!(preview.warnings().len(), 1);
        assert!(preview.warnings()[0].contains("minterm 90"));
        assert!(preview.warnings()[0].contains("mask 15"));
        assert_eq!(doc.to_bytes().unwrap(), bytes);
        assert!(SkypixDocument::new().preview().unwrap().warnings().is_empty());
    }
}
