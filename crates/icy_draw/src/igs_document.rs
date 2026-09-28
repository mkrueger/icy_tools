//! An editable IGS (Atari ST Instant Graphics and Sound) drawing independent of either editor UI.
//!
//! The document is a list of [`IgsItem`]s that covers every byte of the file. Unedited items
//! keep their original bytes, so opening and saving a file without changes is lossless.

use std::path::{Path, PathBuf};

use icy_engine::{AutoWrapMode, EditableScreen, GraphicsType, PaletteScreenBuffer, Screen, ScreenSink, Size};
use icy_parser_core::{
    encode_igs_stream, encode_igs_stream_checked, parse_igs_stream, CommandParser, DrawingMode, IgsCommand, IgsEncodeError, IgsItem, IgsParser, IgsText,
    LineMarkerStyle, PaletteMode, PatternType, PenType, ScreenClearMode, TerminalResolution, TextEffects, TextRotation,
};
use thiserror::Error;

/// Resolution `.ig` files are shown in until they select one themselves.
pub const DEFAULT_RESOLUTION: TerminalResolution = TerminalResolution::Medium;
const PREVIEW_SEED: u64 = 0x1C5_D12A;

#[derive(Debug, Error)]
pub enum IgsDocumentError {
    #[error("{0}")]
    Encode(#[from] IgsEncodeError),
    #[error("item index {index} is out of bounds (length {len})")]
    InvalidIndex { index: usize, len: usize },
    #[error("item {index} is not an IGS command")]
    NotACommand { index: usize },
    #[error("IGS text could not be parsed")]
    InvalidSource,
    #[error("IGS parser failed on this input")]
    ParserPanic,
    #[error("no path is associated with this IGS document")]
    MissingPath,
    #[error("IGS file I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("Could not save IGS drawing: {0}")]
    Save(String),
}

pub type IgsResult<T> = Result<T, IgsDocumentError>;

fn parse_items(bytes: &[u8]) -> IgsResult<Vec<IgsItem>> {
    std::panic::catch_unwind(|| parse_igs_stream(bytes)).map_err(|_| IgsDocumentError::ParserPanic)
}

/// A rendered IGS scene with the palette and resolution it ended in.
pub struct IgsPreview {
    screen: PaletteScreenBuffer,
}

impl IgsPreview {
    pub fn screen(&self) -> &PaletteScreenBuffer {
        &self.screen
    }

    pub fn resolution(&self) -> TerminalResolution {
        match self.screen.graphics_type() {
            GraphicsType::IGS(resolution) => resolution,
            _ => DEFAULT_RESOLUTION,
        }
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

/// Drawing attributes in effect at a position of the command list.
/// `None` means the value is unknown, e.g. after a loop or an initialization.
#[derive(Clone, Debug, PartialEq)]
pub struct IgsDrawState {
    pub resolution: TerminalResolution,
    pub line_color: Option<u8>,
    pub fill_color: Option<u8>,
    pub text_color: Option<u8>,
    pub marker_color: Option<u8>,
    pub fill: Option<(PatternType, bool)>,
    pub line: Option<LineMarkerStyle>,
    pub marker: Option<LineMarkerStyle>,
    pub drawing_mode: Option<DrawingMode>,
    pub text: Option<(TextEffects, u8, TextRotation)>,
}

impl Default for IgsDrawState {
    fn default() -> Self {
        Self {
            resolution: DEFAULT_RESOLUTION,
            line_color: None,
            fill_color: None,
            text_color: None,
            marker_color: None,
            fill: None,
            line: None,
            marker: None,
            drawing_mode: None,
            text: None,
        }
    }
}

impl IgsDrawState {
    fn forget_attributes(&mut self) {
        *self = Self {
            resolution: self.resolution,
            ..Self::default()
        };
    }

    pub fn apply(&mut self, command: &IgsCommand) {
        match command {
            IgsCommand::ColorSet { pen, color } => {
                let slot = match pen {
                    PenType::Polymarker => &mut self.marker_color,
                    PenType::Line => &mut self.line_color,
                    PenType::Fill => &mut self.fill_color,
                    PenType::Text => &mut self.text_color,
                };
                *slot = Some(*color);
            }
            IgsCommand::AttributeForFills { pattern_type, border } => self.fill = Some((*pattern_type, *border)),
            IgsCommand::SetLineOrMarkerStyle { style } => match style {
                LineMarkerStyle::PolyMarkerSize(..) => self.marker = Some(*style),
                LineMarkerStyle::LineThickness(..) | LineMarkerStyle::LineEndpoints(..) => self.line = Some(*style),
            },
            IgsCommand::DrawingMode { mode } => self.drawing_mode = Some(*mode),
            IgsCommand::HollowSet { enabled } => {
                if *enabled {
                    self.fill = Some((PatternType::Hollow, true));
                    self.drawing_mode = Some(DrawingMode::Transparent);
                } else {
                    self.fill = Some((PatternType::Solid, false));
                    self.drawing_mode = Some(DrawingMode::Replace);
                }
            }
            IgsCommand::TextEffects { effects, size, rotation } => self.text = Some((*effects, *size, *rotation)),
            IgsCommand::SetResolution { resolution, .. } => {
                self.resolution = *resolution;
                self.forget_attributes();
            }
            IgsCommand::Initialize { .. } | IgsCommand::Loop(_) => self.forget_attributes(),
            _ => {}
        }
    }
}

/// Ordered IGS items with snapshot-based undo/redo and saved-state tracking.
pub struct IgsDocument {
    items: Vec<IgsItem>,
    saved_items: Vec<IgsItem>,
    undo: Vec<Vec<IgsItem>>,
    redo: Vec<Vec<IgsItem>>,
    revision: u64,
    path: Option<PathBuf>,
    baseline: Vec<u8>,
}

impl Default for IgsDocument {
    fn default() -> Self {
        Self::new(DEFAULT_RESOLUTION)
    }
}

impl IgsDocument {
    /// An untitled drawing that selects `resolution` and clears the screen.
    pub fn new(resolution: TerminalResolution) -> Self {
        let items = vec![
            IgsCommand::SetResolution {
                resolution,
                palette: PaletteMode::IgDefault,
            }
            .into(),
            IgsCommand::ScreenClear {
                mode: ScreenClearMode::ClearWholeScreenAndHome,
            }
            .into(),
        ];
        Self {
            saved_items: items.clone(),
            items,
            undo: Vec::new(),
            redo: Vec::new(),
            revision: 0,
            path: None,
            baseline: Vec::new(),
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> IgsResult<Self> {
        let items = parse_items(bytes)?;
        Ok(Self {
            saved_items: items.clone(),
            items,
            baseline: bytes.to_vec(),
            undo: Vec::new(),
            redo: Vec::new(),
            revision: 0,
            path: None,
        })
    }

    pub fn open(path: impl AsRef<Path>) -> IgsResult<Self> {
        let bytes = std::fs::read(path.as_ref())?;
        let mut document = Self::from_bytes(&bytes)?;
        document.path = Some(path.as_ref().to_path_buf());
        Ok(document)
    }

    pub fn items(&self) -> &[IgsItem] {
        &self.items
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn command(&self, index: usize) -> Option<&IgsCommand> {
        self.items.get(index).and_then(IgsItem::command)
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn is_dirty(&self) -> bool {
        self.items != self.saved_items
    }

    pub fn modified(&self) -> bool {
        self.is_dirty()
    }

    /// Increases whenever the item list changes, including undo and redo.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Drawing attributes in effect before the item at `index`; `len()` gives the final state.
    pub fn state_before(&self, index: usize) -> IgsDrawState {
        let mut state = IgsDrawState::default();
        for command in self.items[..index.min(self.items.len())].iter().filter_map(IgsItem::command) {
            state.apply(command);
        }
        state
    }

    /// The resolution the drawing ends in, which is the canvas size used for editing.
    pub fn resolution(&self) -> TerminalResolution {
        self.state_before(self.items.len()).resolution
    }

    pub fn to_bytes(&self) -> IgsResult<Vec<u8>> {
        if !self.is_dirty() && self.path.is_some() {
            return Ok(self.baseline.clone());
        }
        Ok(encode_igs_stream_checked(&self.items)?)
    }

    /// The editable text of an item: its original bytes, or its canonical encoding once edited.
    pub fn item_source(&self, index: usize) -> IgsResult<Vec<u8>> {
        let item = self.items.get(index).ok_or(IgsDocumentError::InvalidIndex { index, len: self.items.len() })?;
        Ok(match item {
            IgsItem::Text(text) => text.bytes.clone(),
            IgsItem::Command(command) => match command.source() {
                Some(source) => source.to_vec(),
                None => icy_parser_core::write_igs_command(command.command()),
            },
        })
    }

    pub fn recovery_snapshot(&self) -> IgsResult<crate::recovery::Snapshot> {
        Ok(crate::recovery::Snapshot {
            kind: crate::recovery::RecoveryKind::Igs,
            path: self.path.clone(),
            disk: self.path.as_ref().map(|_| crate::recovery::Fingerprint::of(&self.baseline)),
            payload: encode_igs_stream_checked(&self.items)?,
        })
    }

    pub fn from_recovery(snapshot: &crate::recovery::Snapshot) -> IgsResult<Self> {
        let mut document = Self::from_bytes(&snapshot.payload)?;
        document.path = snapshot.path.clone();
        document.baseline = snapshot
            .path
            .as_deref()
            .zip(snapshot.disk)
            .and_then(|(path, disk)| disk.read_matching(path))
            .unwrap_or_default();
        document.saved_items = parse_items(&document.baseline).unwrap_or_default();
        if document.saved_items == document.items {
            // A recovered document always differs from disk, otherwise it would not have been saved.
            document.saved_items.push(IgsItem::Text(IgsText {
                bytes: Vec::new(),
                invalid: false,
            }));
        }
        Ok(document)
    }

    pub fn save_current(&mut self, overwrite: bool) -> IgsResult<()> {
        let path = self.path.clone().ok_or(IgsDocumentError::MissingPath)?;
        self.save(path, overwrite)
    }

    pub fn save(&mut self, path: impl AsRef<Path>, overwrite: bool) -> IgsResult<()> {
        let bytes = self.to_bytes()?;
        crate::files::save_bytes(
            path.as_ref(),
            &bytes,
            self.path.as_deref().map(|path| (path, self.baseline.as_slice())),
            overwrite,
        )
        .map_err(IgsDocumentError::Save)?;
        self.path = Some(path.as_ref().to_path_buf());
        self.baseline = bytes;
        self.saved_items = self.items.clone();
        Ok(())
    }

    pub fn save_as(&mut self, path: impl AsRef<Path>, overwrite: bool) -> IgsResult<()> {
        self.save(path, overwrite)
    }

    /// Replay items without changing the document. Random parameters use a fixed seed so
    /// previews do not flicker between frames.
    pub fn render(items: &[IgsItem]) -> IgsResult<IgsPreview> {
        let bytes = encode_igs_stream(items)?;
        let mut screen = PaletteScreenBuffer::new(GraphicsType::IGS(DEFAULT_RESOLUTION));
        screen.terminal_state_mut().auto_wrap_mode = AutoWrapMode::AutoWrap;
        *screen.buffer_type_mut() = icy_engine::BufferType::Atascii;
        screen.set_font_dimensions(Size::new(8, 8));
        screen.caret_default_colors();
        screen.caret_mut().visible = false;
        let rendered = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            fastrand::seed(PREVIEW_SEED);
            let mut parser = IgsParser::new();
            parser.run_loop = true;
            let mut sink = ScreenSink::new(&mut screen);
            parser.parse(&bytes, &mut sink);
        }));
        rendered.map_err(|_| IgsDocumentError::ParserPanic)?;
        Ok(IgsPreview { screen })
    }

    pub fn preview(&self) -> IgsResult<IgsPreview> {
        Self::render(&self.items)
    }

    /// Preview through the item at `through` (inclusive), or everything for `None`.
    pub fn preview_through(&self, through: Option<usize>) -> IgsResult<IgsPreview> {
        match through {
            Some(index) => {
                self.check_index(index)?;
                Self::render(&self.items[..=index])
            }
            None => self.preview(),
        }
    }

    /// The drawing with `extra` commands inserted at `index`, e.g. a shape that is being drawn.
    pub fn preview_with(&self, index: usize, extra: &[IgsCommand]) -> IgsResult<IgsPreview> {
        let mut items = self.items.clone();
        let index = index.min(items.len());
        items.splice(index..index, extra.iter().cloned().map(IgsItem::from));
        Self::render(&items)
    }

    /// The drawing with the command at `index` replaced, e.g. a shape that is being moved.
    pub fn preview_replacing(&self, index: usize, command: &IgsCommand) -> IgsResult<IgsPreview> {
        let mut items = self.items.clone();
        Self::set_command(&mut items, index, command.clone())?;
        Self::render(&items)
    }

    fn check_index(&self, index: usize) -> IgsResult<()> {
        if index >= self.items.len() {
            return Err(IgsDocumentError::InvalidIndex { index, len: self.items.len() });
        }
        Ok(())
    }

    fn set_command(items: &mut [IgsItem], index: usize, command: IgsCommand) -> IgsResult<()> {
        let len = items.len();
        match items.get_mut(index) {
            Some(IgsItem::Command(item)) => {
                item.set_command(command);
                Ok(())
            }
            Some(IgsItem::Text(_)) => Err(IgsDocumentError::NotACommand { index }),
            None => Err(IgsDocumentError::InvalidIndex { index, len }),
        }
    }

    /// Applies a candidate item list as one undo step if it can be written.
    fn commit(&mut self, items: Vec<IgsItem>) -> IgsResult<()> {
        if items == self.items {
            return Ok(());
        }
        encode_igs_stream(&items)?;
        self.undo.push(std::mem::replace(&mut self.items, items));
        self.redo.clear();
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }

    pub fn append(&mut self, command: IgsCommand) -> IgsResult<()> {
        self.append_many(vec![command])
    }

    pub fn append_many(&mut self, commands: Vec<IgsCommand>) -> IgsResult<()> {
        let index = self.items.len();
        self.insert_many(index, commands)
    }

    /// Inserts commands as a single undo step.
    pub fn insert_many(&mut self, index: usize, commands: Vec<IgsCommand>) -> IgsResult<()> {
        if index > self.items.len() {
            return Err(IgsDocumentError::InvalidIndex { index, len: self.items.len() });
        }
        if commands.is_empty() {
            return Ok(());
        }
        let mut items = self.items.clone();
        items.splice(index..index, commands.into_iter().map(IgsItem::from));
        self.commit(items)
    }

    pub fn insert(&mut self, index: usize, command: IgsCommand) -> IgsResult<()> {
        self.insert_many(index, vec![command])
    }

    pub fn replace(&mut self, index: usize, command: IgsCommand) -> IgsResult<()> {
        self.replace_many(vec![(index, command)])
    }

    /// Replaces several commands in one undo step.
    pub fn replace_many(&mut self, replacements: Vec<(usize, IgsCommand)>) -> IgsResult<()> {
        let mut items = self.items.clone();
        for (index, command) in replacements {
            Self::set_command(&mut items, index, command)?;
        }
        self.commit(items)
    }

    /// Replaces the item at `index` with whatever `source` parses to, e.g. a hand-edited loop.
    /// Returns the number of items that replaced it.
    pub fn replace_source(&mut self, index: usize, source: &[u8]) -> IgsResult<usize> {
        self.check_index(index)?;
        let parsed = parse_items(source)?;
        if parsed.is_empty() || parsed.iter().any(|item| matches!(item, IgsItem::Text(text) if text.invalid)) {
            return Err(IgsDocumentError::InvalidSource);
        }
        let count = parsed.len();
        let mut items = self.items.clone();
        // Keep the line breaks that separated the old item from the next one.
        if let (Some(IgsItem::Command(old)), Some(IgsItem::Command(new))) = (items.get(index), parsed.last()) {
            if new.trailing().is_empty() && !old.trailing().is_empty() {
                let mut bytes = source.to_vec();
                bytes.extend_from_slice(old.trailing());
                let reparsed = parse_items(&bytes)?;
                if reparsed.len() == count {
                    items.splice(index..=index, reparsed);
                    self.commit(items)?;
                    return Ok(count);
                }
            }
        }
        items.splice(index..=index, parsed);
        self.commit(items)?;
        Ok(count)
    }

    pub fn delete(&mut self, index: usize) -> IgsResult<IgsItem> {
        self.check_index(index)?;
        let mut items = self.items.clone();
        let removed = items.remove(index);
        self.commit(items)?;
        Ok(removed)
    }

    /// Deletes several items in one undo step.
    pub fn delete_many(&mut self, indices: &[usize]) -> IgsResult<()> {
        let mut indices = indices.to_vec();
        indices.sort_unstable();
        indices.dedup();
        for &index in &indices {
            self.check_index(index)?;
        }
        let mut items = self.items.clone();
        for index in indices.into_iter().rev() {
            items.remove(index);
        }
        self.commit(items)
    }

    /// Move an item to its final index (both indices refer to the current list).
    pub fn move_item(&mut self, from: usize, to: usize) -> IgsResult<()> {
        self.check_index(from)?;
        self.check_index(to)?;
        if from == to {
            return Ok(());
        }
        let mut items = self.items.clone();
        let item = items.remove(from);
        items.insert(to, item);
        self.commit(items)
    }

    pub fn undo(&mut self) -> bool {
        if let Some(previous) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.items, previous));
            self.revision = self.revision.wrapping_add(1);
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.items, next));
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
    use icy_parser_core::IgsParameter;

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../icy_parser_core/benches/igs_data").join(name)).unwrap()
    }

    fn line(x1: i32, y1: i32, x2: i32, y2: i32) -> IgsCommand {
        IgsCommand::Line {
            x1: x1.into(),
            y1: y1.into(),
            x2: x2.into(),
            y2: y2.into(),
        }
    }

    #[test]
    fn new_document_selects_resolution() {
        for resolution in [TerminalResolution::Low, TerminalResolution::Medium, TerminalResolution::High] {
            let document = IgsDocument::new(resolution);
            assert!(!document.is_dirty());
            assert_eq!(document.resolution(), resolution);
            let preview = document.preview().unwrap();
            assert_eq!(preview.resolution(), resolution);
            let size = match resolution {
                TerminalResolution::Low => (320, 200),
                TerminalResolution::Medium => (640, 200),
                TerminalResolution::High => (640, 400),
            };
            assert_eq!((preview.width(), preview.height()), size);
        }
    }

    #[test]
    fn edit_history_and_round_trip() {
        let mut document = IgsDocument::new(TerminalResolution::Low);
        document
            .append_many(vec![IgsCommand::ColorSet { pen: PenType::Line, color: 1 }, line(0, 0, 100, 100)])
            .unwrap();
        assert!(document.is_dirty());
        assert_eq!(document.len(), 4);
        document.replace(3, line(10, 10, 50, 50)).unwrap();
        assert!(document.replace(1, line(0, 0, 1, 1)).is_ok());
        assert!(matches!(document.command(1), Some(IgsCommand::Line { .. })));
        assert!(document.undo());
        assert_eq!(document.command(3), Some(&line(10, 10, 50, 50)));
        assert!(document.undo());
        assert_eq!(document.command(3), Some(&line(0, 0, 100, 100)));
        assert!(document.redo());

        let bytes = document.to_bytes().unwrap();
        let reopened = IgsDocument::from_bytes(&bytes).unwrap();
        let commands = |document: &IgsDocument| document.items().iter().filter_map(IgsItem::command).cloned().collect::<Vec<_>>();
        assert_eq!(commands(&reopened), commands(&document));

        document.move_item(3, 2).unwrap();
        assert_eq!(document.command(2), Some(&line(10, 10, 50, 50)));
        document.delete_many(&[2, 3]).unwrap();
        assert_eq!(document.len(), 2);
    }

    #[test]
    fn fixtures_open_unchanged_and_render() {
        for name in ["KM-7LARR.IG", "KM-1FED.IG", "CARD.IG", "MENU2.IG"] {
            let bytes = fixture(name);
            let document = IgsDocument::from_bytes(&bytes).unwrap();
            assert!(!document.is_dirty());
            assert_eq!(encode_igs_stream_checked(document.items()).unwrap(), bytes, "{name}");
            // Animations may end by clearing the screen, so sample several prefixes.
            let renders = (1..=4).any(|quarter| {
                let preview = document.preview_through(Some(document.len() * quarter / 4 - 1)).unwrap();
                preview.indices().iter().any(|&index| index != preview.indices()[0])
            });
            assert!(renders, "{name} renders nothing");
        }
    }

    #[test]
    fn previews_are_deterministic_and_match_prefixes() {
        let mut document = IgsDocument::new(TerminalResolution::Low);
        document
            .append_many(vec![
                IgsCommand::ColorSet { pen: PenType::Line, color: 1 },
                IgsCommand::Line {
                    x1: IgsParameter::Random,
                    y1: IgsParameter::Random,
                    x2: IgsParameter::Random,
                    y2: IgsParameter::Random,
                },
            ])
            .unwrap();
        assert_eq!(document.preview().unwrap().indices(), document.preview().unwrap().indices());
        let blank = document.preview_through(Some(1)).unwrap();
        assert!(blank.indices().iter().all(|&index| index == blank.indices()[0]));

        let with = document.preview_with(document.len(), &[line(0, 0, 319, 0)]).unwrap();
        assert!(with.pixel_index(100, 0) != blank.pixel_index(100, 0));
        let replaced = document.preview_replacing(3, &line(0, 199, 319, 199)).unwrap();
        assert!(replaced.pixel_index(100, 199) != blank.pixel_index(100, 199));
    }

    #[test]
    fn edited_fixture_keeps_other_bytes() {
        let bytes = fixture("CIRTEST1.IG");
        let mut document = IgsDocument::from_bytes(&bytes).unwrap();
        let index = document
            .items()
            .iter()
            .position(|item| matches!(item.command(), Some(IgsCommand::Circle { .. })))
            .unwrap();
        let before = String::from_utf8(document.item_source(index).unwrap()).unwrap();
        assert!(before.starts_with("G#O>"));
        document
            .replace(
                index,
                IgsCommand::Circle {
                    x: 10.into(),
                    y: 20.into(),
                    radius: 5.into(),
                },
            )
            .unwrap();
        let expected = String::from_utf8_lossy(&bytes).replacen(&before, "G#O>10,20,5:", 1);
        assert_eq!(String::from_utf8_lossy(&document.to_bytes().unwrap()), expected);
    }

    #[test]
    fn state_tracks_attributes() {
        let mut document = IgsDocument::new(TerminalResolution::Low);
        document
            .append_many(vec![
                IgsCommand::ColorSet { pen: PenType::Fill, color: 3 },
                IgsCommand::HollowSet { enabled: true },
            ])
            .unwrap();
        let state = document.state_before(document.len());
        assert_eq!(state.resolution, TerminalResolution::Low);
        assert_eq!(state.fill_color, Some(3));
        assert_eq!(state.fill, Some((PatternType::Hollow, true)));
        assert_eq!(document.state_before(2).fill_color, None);
    }

    #[test]
    fn raw_source_edits_loops() {
        let mut document = IgsDocument::from_bytes(b"G#L>0,0,10,10:\r\nG#&>0,10,2,0,L,4,x,0,x,50:\r\nG#O>5,5,5:\r\n").unwrap();
        assert_eq!(document.len(), 3);
        assert!(matches!(document.command(1), Some(IgsCommand::Loop(_))));
        assert_eq!(document.item_source(1).unwrap(), b"G#&>0,10,2,0,L,4,x,0,x,50:");
        let count = document.replace_source(1, b"G#&>0,20,5,0,L,4,x,0,x,50:").unwrap();
        assert_eq!(count, 1);
        let IgsCommand::Loop(data) = document.command(1).unwrap() else {
            panic!("expected loop")
        };
        assert_eq!(data.to, 20);
        assert_eq!(document.to_bytes().unwrap(), b"G#L>0,0,10,10:\r\nG#&>0,20,5,0,L,4,x,0,x,50:\r\nG#O>5,5,5:\r\n");
        assert!(document.replace_source(1, b"G#~:").is_err());
        assert!(document.undo());
        assert!(!document.is_dirty());
    }

    #[test]
    fn recovery_round_trip() {
        let mut document = IgsDocument::new(TerminalResolution::High);
        document.append(line(0, 0, 10, 10)).unwrap();
        let snapshot = document.recovery_snapshot().unwrap();
        let recovered = IgsDocument::from_recovery(&snapshot).unwrap();
        assert!(recovered.is_dirty());
        assert_eq!(recovered.resolution(), TerminalResolution::High);
        assert_eq!(recovered.command(2), Some(&line(0, 0, 10, 10)));
    }
}
