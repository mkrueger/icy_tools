use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use icy_engine::{AddType, FileFormat, KeyModifiers, MouseButton, Position, Rectangle, Screen, Selection, Size, TextBuffer, TextPane};
use icy_engine_edit::{tools::Tool, AtomicUndoGuard, EditState, UndoState};
use parking_lot::Mutex;

use crate::{
    box_lines::{self, BoxStyle},
    brush::BrushPrimaryMode,
    paint::{apply_stamp_at_doc_pos, BrushSettings},
    selection_drag::{compute_dragged_selection, hit_test_selection, DragParameters, SelectionDrag},
    shape_points::shape_points,
};

pub type DrawResult<T> = Result<T, String>;

struct Stroke {
    start: Position,
    last: Position,
    tool: Tool,
    brush: BrushSettings,
    button: MouseButton,
    clear: bool,
    selection_drag: SelectionDrag,
    selection_rect: Option<Rectangle>,
    add_type: AddType,
    layer_offset: Option<Position>,
    tag_offsets: Vec<(usize, Position)>,
    /// A tag tool drag on empty canvas that creates a tag if it stays on one row.
    tag_create: bool,
    undo: AtomicUndoGuard,
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum SelectionMode {
    #[default]
    Rectangle,
    Character,
    Attribute,
    Foreground,
    Background,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasteAction {
    Move(Position),
    Stamp,
    Rotate,
    FlipX,
    FlipY,
    Transparent,
    /// Centers the paste horizontally on the canvas.
    Center,
    Anchor,
    Keep,
    Cancel,
}

struct PasteState {
    previous_tool: Tool,
    undo: AtomicUndoGuard,
    /// Every state of the paste before and after it was made transparent, so undoing back to a
    /// transparent state keeps the toggle working; see [`Document::paste_transparent`].
    transparent: Vec<(icy_engine_edit::CharGrid, icy_engine_edit::CharGrid)>,
    /// Tags lifted with a block: where the block started and the tags inside it, which move
    /// with a moved block and are copied with a copied one when it is anchored.
    carried: Option<(Position, Vec<usize>, bool)>,
}

/// All characters of the current layer.
fn layer_chars(state: &EditState) -> icy_engine_edit::CharGrid {
    state.get_cur_layer().map_or_else(Vec::new, |layer| {
        icy_engine_edit::chars_from_area(layer, icy_engine::Rectangle::from_min_size(Position::default(), layer.size()))
    })
}

/// Makes the paste transparent and returns the characters before and after.
fn make_transparent(state: &mut EditState) -> icy_engine::Result<(icy_engine_edit::CharGrid, icy_engine_edit::CharGrid)> {
    let opaque = layer_chars(state);
    state.make_layer_transparent()?;
    Ok((opaque, layer_chars(state)))
}

/// Outline font keys of TheDraw: F1–F10 type the placeholders `A`–`J`, the digits 1–8 type
/// `K`–`O`, the fill marker `@`, the end marker `&` and the hard blank.
pub const OUTLINE_KEYS: [(&str, char); 18] = [
    ("F1", 'A'),
    ("F2", 'B'),
    ("F3", 'C'),
    ("F4", 'D'),
    ("F5", 'E'),
    ("F6", 'F'),
    ("F7", 'G'),
    ("F8", 'H'),
    ("F9", 'I'),
    ("F10", 'J'),
    ("1", 'K'),
    ("2", 'L'),
    ("3", 'M'),
    ("4", 'N'),
    ("5", 'O'),
    ("6", '@'),
    ("7", '&'),
    ("8", '\u{00ff}'),
];

/// Outline code typed by the digit keys 1–8.
/// The character `font` draws for `character`: itself, a space as wide as the font's spacing,
/// or the other case in fonts that have only upper or only lower case letters.
fn art_glyph(font: &retrofont::Font, character: char) -> Option<char> {
    if font.has_char(character) || character == ' ' {
        return Some(character);
    }
    let other = if character.is_lowercase() {
        character.to_uppercase().next()
    } else {
        character.to_lowercase().next()
    };
    other.filter(|other| *other != character && font.has_char(*other))
}

fn outline_digit(character: char) -> Option<char> {
    OUTLINE_KEYS[10..].iter().find(|(key, _)| key.starts_with(character)).map(|(_, code)| *code)
}

pub struct Document {
    pub screen: Arc<Mutex<Box<dyn Screen>>>,
    pub path: Option<PathBuf>,
    pub brush: BrushSettings,
    /// The line tool draws box-drawing lines in this style, joined with the lines they meet,
    /// instead of using the brush.
    pub box_line: Option<BoxStyle>,
    /// The box-line style last chosen, kept while the line tool draws with the brush.
    pub box_style: BoxStyle,
    pub tool: Tool,
    pub selection_mode: SelectionMode,
    pub outline_font: bool,
    pub selected_tags: Vec<usize>,
    /// Where a tag tool drag along one row asks for a new tag, and its width.
    pub new_tag_request: Option<(Position, usize)>,
    pub preview: Vec<Position>,
    /// The characters a box line drag will draw, with the joins, for the canvas to show as they will look.
    pub box_preview: Vec<(Position, char, icy_engine::TextAttribute)>,
    pub metadata_dirty: bool,
    pub baseline: Option<Vec<u8>>,
    stroke: Option<Stroke>,
    paste: Option<PasteState>,
    /// The caret after the last font text and the column its line started at, so Enter returns
    /// there while typing continues from where the text left off.
    art_line: Option<(Position, i32)>,
    /// Tags copied in the tag tool and the clipboard text put there for them.
    tag_clipboard: Option<(String, Vec<icy_engine::Tag>)>,
    /// Whether text is typed in inverse video, on screens that keep it in the character (ATASCII).
    pub inverse: bool,
    /// Whether the pencil and shapes draw 2 × 2 pixels per character with quarter blocks
    /// (ATASCII); positions are then in pixels.
    pub quarter_blocks: bool,
}

impl Document {
    pub fn new(size: Size) -> Self {
        let mut buffer = TextBuffer::new(size);
        buffer.terminal_state.is_terminal_buffer = false;
        Self::from_state(EditState::from_buffer(buffer))
    }

    /// An empty ATASCII document in the Atari screen `mode`.
    pub fn new_atascii(mode: crate::screen_profile::AtasciiMode) -> Self {
        Self::from_state(EditState::from_buffer(crate::screen_profile::atascii_buffer(mode)))
    }

    /// An empty VT52 document: the Atari ST's text screen in `resolution`, typing in its text color.
    pub fn new_atari_st(resolution: icy_engine::TerminalResolution) -> Self {
        let mut state = EditState::from_buffer(crate::screen_profile::atari_st_buffer(resolution));
        state.set_caret_foreground(icy_engine::atari_st_text_color(resolution));
        state.set_caret_background(0);
        Self::from_state(state)
    }

    /// What the document's screen can hold.
    pub fn profile(&self) -> crate::screen_profile::ScreenProfile {
        self.with_state(|state| crate::screen_profile::ScreenProfile::of(state.get_buffer()))
    }

    pub fn from_state(state: EditState) -> Self {
        Self {
            screen: Arc::new(Mutex::new(Box::new(state))),
            path: None,
            baseline: None,
            brush: BrushSettings::default(),
            box_line: None,
            box_style: BoxStyle::default(),
            tool: Tool::Click,
            selection_mode: SelectionMode::default(),
            outline_font: false,
            selected_tags: Vec::new(),
            new_tag_request: None,
            preview: Vec::new(),
            box_preview: Vec::new(),
            metadata_dirty: false,
            stroke: None,
            paste: None,
            art_line: None,
            tag_clipboard: None,
            inverse: false,
            quarter_blocks: false,
        }
    }

    pub fn load(path: &Path) -> DrawResult<Self> {
        let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
        let format = FileFormat::from_path(path).ok_or_else(|| format!("Unknown file format: {}", path.display()))?;
        let load_data = crate::screen_profile::atascii_columns(path).map(|columns| icy_engine::LoadData::new(None, Some(columns)));
        let loaded = format.from_bytes(&bytes, load_data).map_err(|error| error.to_string())?;
        let mut state = EditState::from_buffer(loaded.screen.buffer);
        if let Some(sauce) = loaded.sauce_opt {
            state.set_sauce_meta(sauce.metadata());
        }
        let mut document = Self::from_state(state);
        document.path = Some(path.to_path_buf());
        document.baseline = Some(bytes);
        Ok(document)
    }

    pub fn with_state<T>(&self, action: impl FnOnce(&mut EditState) -> T) -> T {
        let mut screen = self.screen.lock();
        action(screen.as_any_mut().downcast_mut::<EditState>().expect("draw document edit state"))
    }

    pub fn modified(&self) -> bool {
        self.paste_active() || self.metadata_dirty || self.with_state(|state| state.get_undo_stack().lock().unwrap().is_modified())
    }

    pub fn bytes(&self, format: FileFormat, mut options: icy_engine::SaveOptions) -> DrawResult<Vec<u8>> {
        self.with_state(|state| {
            options.sauce = Some(state.get_sauce_meta().clone());
            format.to_bytes(state.get_buffer(), &options).map_err(|error| error.to_string())
        })
    }

    pub fn save(&mut self, path: &Path, overwrite: bool) -> DrawResult<()> {
        self.finish();
        self.paste_action(PasteAction::Keep)?;
        let format = FileFormat::from_path(path).ok_or("Unknown file extension")?;
        if format != FileFormat::IcyDraw {
            return Err("Save editable documents as .icy; use Export for other formats.".into());
        }
        let bytes = self.bytes(format, icy_engine::SaveOptions::icy_draw())?;
        crate::files::save_bytes(path, &bytes, self.path.as_deref().zip(self.baseline.as_deref()), overwrite)?;
        self.path = Some(path.to_path_buf());
        self.baseline = Some(bytes);
        self.metadata_dirty = false;
        self.with_state(|state| state.get_undo_stack().lock().unwrap().mark_saved());
        Ok(())
    }

    pub fn stroke_active(&self) -> bool {
        self.stroke.is_some()
    }

    /// The complete document for crash recovery, including an unanchored paste.
    pub fn recovery_snapshot(&self) -> DrawResult<crate::recovery::Snapshot> {
        let options = icy_engine::SaveOptions {
            format: icy_engine::FormatOptions::IcyDraw(icy_engine::IcyDrawFormatOptions {
                skip_thumbnail: true,
                compress: true,
            }),
            ..icy_engine::SaveOptions::icy_draw()
        };
        Ok(crate::recovery::Snapshot {
            kind: crate::recovery::RecoveryKind::Ansi,
            path: self.path.clone(),
            disk: self.baseline.as_deref().map(crate::recovery::Fingerprint::of),
            payload: self.bytes(FileFormat::IcyDraw, options)?,
        })
    }

    /// Restores a [`Self::recovery_snapshot`] as a modified document of its original file.
    ///
    /// When that file changed since the snapshot's edits began, saving to it requires
    /// confirmation, just as for files changed while the document is open.
    pub fn from_recovery(snapshot: &crate::recovery::Snapshot) -> DrawResult<Self> {
        let loaded = FileFormat::IcyDraw.from_bytes(&snapshot.payload, None).map_err(|error| error.to_string())?;
        let mut state = EditState::from_buffer(loaded.screen.buffer);
        if let Some(sauce) = loaded.sauce_opt {
            state.set_sauce_meta(sauce.metadata());
        }
        let mut document = Self::from_state(state);
        document.path = snapshot.path.clone();
        document.baseline = snapshot.path.as_deref().zip(snapshot.disk).and_then(|(path, disk)| disk.read_matching(path));
        document.metadata_dirty = true;
        Ok(document)
    }

    pub fn paste_active(&self) -> bool {
        self.paste.is_some()
    }

    pub fn start_paste(&mut self, text: &str, data: Option<&[u8]>) -> DrawResult<()> {
        if text.is_empty() && data.is_none() {
            return Ok(());
        }
        self.start_floating_paste(|state| {
            if let Some(data) = data {
                state.paste_clipboard_data(data)
            } else {
                state.paste_text(text)
            }
        })
    }

    /// Inserts an RGBA image as a floating image layer that can be positioned before anchoring.
    pub fn start_image_paste(&mut self, image: &image::RgbaImage) -> DrawResult<()> {
        let mut sixel = icy_engine::Sixel::new(Position::default());
        sixel.picture_data = image.as_raw().clone();
        sixel.set_width(image.width() as i32);
        sixel.set_height(image.height() as i32);
        self.start_floating_paste(|state| state.paste_sixel(sixel))
    }

    pub fn insert_buffer(&mut self, buffer: &icy_engine::TextBuffer, title: String) -> DrawResult<()> {
        if self.paste_active() || !self.can_paint() {
            return Err("Finish the current paste and select an editable layer before inserting artwork.".into());
        }
        self.start_floating_paste(|state| state.paste_buffer(buffer, title))?;
        self.paste_action(PasteAction::Keep)
    }

    /// Lifts the selected cells into a floating paste at the same place, like the Move Block and
    /// Copy Block of Moebius and PabloDraw; `cut` erases them from the layer below.
    pub fn float_selection(&mut self, cut: bool) -> DrawResult<()> {
        let Some((data, area)) = self.with_state(|state| Some((state.clipboard_data()?, state.selection()?.as_rectangle()))) else {
            return Ok(());
        };
        // Tags that lie completely inside the block go with it.
        let tags: Vec<usize> = self.with_state(|state| {
            state
                .get_buffer()
                .tags
                .iter()
                .enumerate()
                .filter(|(_, tag)| {
                    let end = tag.position + Position::new(tag.len().max(1) as i32 - 1, 0);
                    area.contains_pt(tag.position) && area.contains_pt(end)
                })
                .map(|(index, _)| index)
                .collect()
        });
        self.with_state(|state| state.set_caret_from_document_position(area.start));
        self.start_floating(cut, |state| state.paste_clipboard_data(&data))?;
        if let Some(paste) = &mut self.paste {
            if !tags.is_empty() {
                paste.carried = Some((area.start, tags, cut));
            }
        }
        Ok(())
    }

    /// Fills the selection with full blocks in the foreground color, or erases it for color 0,
    /// like Moebius.
    pub fn fill_selection(&mut self) -> DrawResult<()> {
        if !self.can_paint() {
            return Ok(());
        }
        self.finish();
        self.with_state(|state| {
            let attribute = state.get_caret().attribute;
            if attribute.foreground() == 0 {
                return state.erase_selection();
            }
            let mut block = icy_engine::TextAttribute::default();
            block.set_foreground(attribute.foreground());
            state.fill_selection(icy_engine::AttributedChar::new('\u{00DB}', block))
        })
        .map_err(|error| error.to_string())
    }

    fn start_floating_paste(&mut self, paste: impl FnOnce(&mut EditState) -> icy_engine::Result<()>) -> DrawResult<()> {
        self.start_floating(false, paste)
    }

    fn start_floating(&mut self, erase_selection: bool, paste: impl FnOnce(&mut EditState) -> icy_engine::Result<()>) -> DrawResult<()> {
        self.finish();
        if self.paste_active() || !self.can_paint() {
            return Ok(());
        }
        let mut undo = self.with_state(|state| state.begin_atomic_undo("Paste"));
        let result = self.with_state(|state| {
            if erase_selection {
                state.erase_selection()?;
            }
            let offset = state.get_cur_layer().map(|layer| layer.offset()).unwrap_or_default();
            let count = state.get_buffer().layers.len();
            // While the caret still sits on a corner of the selection (where the drag began or ended), the paste
            // starts at the selection's upper left corner instead, whichever direction it was dragged.
            if let Some(selection) = state.selection() {
                let caret = state.layer_to_document_position(state.get_caret().position());
                if caret == selection.anchor || caret == selection.lead {
                    state.set_caret_from_document_position(selection.as_rectangle().start);
                }
            }
            state.clear_selection()?;
            paste(state)?;
            if state.get_buffer().layers.len() == count {
                return Ok(false);
            }
            let position = state.get_cur_layer().unwrap().offset() + offset;
            state.move_layer(position)?;
            Ok::<bool, icy_engine::EngineError>(true)
        });
        match result {
            Ok(true) => {
                self.paste = Some(PasteState {
                    previous_tool: self.tool,
                    undo,
                    transparent: Vec::new(),
                    carried: None,
                });
                self.tool = Tool::Click;
                Ok(())
            }
            result => {
                self.with_state(|state| undo.discard_and_undo(state));
                result.map(|_| ()).map_err(|error| error.to_string())
            }
        }
    }

    /// Moves the tags lifted with a block to where it is placed, or copies them for a copied
    /// block, within the paste's undo step.
    fn place_carried_tags(&mut self) -> icy_engine::Result<()> {
        let Some((origin, tags, cut)) = self.paste.as_mut().and_then(|paste| paste.carried.take()) else {
            return Ok(());
        };
        self.with_state(|state| {
            let Some(offset) = state.get_cur_layer().map(|layer| layer.offset()) else {
                return Ok(());
            };
            let delta = offset - origin;
            for index in tags {
                let Some(tag) = state.get_buffer().tags.get(index).cloned() else {
                    continue;
                };
                if cut {
                    state.move_tag(index, tag.position + delta)?;
                } else {
                    state.add_new_tag(icy_engine::Tag {
                        position: tag.position + delta,
                        ..tag
                    })?;
                }
            }
            Ok(())
        })
    }

    /// Whether the floating paste is transparent. Other edits of the paste turn it off.
    pub fn paste_transparent(&self) -> bool {
        self.transparent_source().is_some()
    }

    /// The opaque paste of the current transparent one.
    fn transparent_source(&self) -> Option<icy_engine_edit::CharGrid> {
        let paste = self.paste.as_ref()?;
        if paste.transparent.is_empty() {
            return None;
        }
        let current = self.with_state(|state| layer_chars(state));
        paste
            .transparent
            .iter()
            .rev()
            .find(|(_, transparent)| *transparent == current)
            .map(|(opaque, _)| opaque.clone())
    }

    pub fn paste_action(&mut self, action: PasteAction) -> DrawResult<()> {
        if !self.paste_active() {
            return Ok(());
        }
        if action == PasteAction::Cancel {
            self.cancel();
            let mut paste = self.paste.take().unwrap();
            self.with_state(|state| paste.undo.discard_and_undo(state));
            self.tool = paste.previous_tool;
            return Ok(());
        }
        self.finish();
        if matches!(action, PasteAction::Anchor | PasteAction::Keep) {
            self.place_carried_tags().map_err(|error| error.to_string())?;
        }
        // Transparency is a toggle: the opaque characters are kept to turn it off again, and
        // rotating or flipping a transparent paste transforms the opaque one and reapplies it.
        let opaque = self.transparent_source();
        let mut transparent = None;
        self.with_state(|state| match action {
            PasteAction::Move(delta) => state.move_layer(state.get_cur_layer().unwrap().offset() + delta),
            PasteAction::Center => {
                let layer = state.get_cur_layer().unwrap();
                let (width, y) = (layer.width(), layer.offset().y);
                let x = ((state.get_buffer().width() - width) / 2).max(0);
                state.move_layer(Position::new(x, y))
            }
            PasteAction::Stamp => state.stamp_layer_down(),
            PasteAction::Rotate | PasteAction::FlipX | PasteAction::FlipY => {
                let name = match action {
                    PasteAction::Rotate => crate::fl!("paste-tool-rotate"),
                    PasteAction::FlipX => crate::fl!("paste-tool-flip-x"),
                    _ => crate::fl!("paste-tool-flip-y"),
                };
                let _undo = opaque.as_ref().map(|_| state.begin_atomic_undo(name));
                if let Some(chars) = &opaque {
                    state.replace_layer_chars(chars.clone())?;
                }
                match action {
                    PasteAction::Rotate => state.paste_rotate()?,
                    PasteAction::FlipX => state.paste_flip_x()?,
                    _ => state.paste_flip_y()?,
                }
                if opaque.is_some() {
                    transparent = Some(make_transparent(state)?);
                }
                Ok(())
            }
            PasteAction::Transparent => {
                if let Some(chars) = &opaque {
                    state.replace_layer_chars(chars.clone())
                } else {
                    transparent = Some(make_transparent(state)?);
                    Ok(())
                }
            }
            PasteAction::Anchor => state.paste_anchor(),
            PasteAction::Keep => state.add_floating_layer(),
            PasteAction::Cancel => Ok(()),
        })
        .map_err(|error| error.to_string())?;
        if let (Some(paste), Some(pair)) = (&mut self.paste, transparent) {
            if !paste.transparent.contains(&pair) {
                paste.transparent.push(pair);
            }
        }
        if matches!(action, PasteAction::Anchor | PasteAction::Keep) {
            self.tool = self.paste.take().unwrap().previous_tool;
        }
        Ok(())
    }

    pub fn begin(&mut self, position: Position, button: MouseButton) {
        self.begin_with_shift(position, button, false);
    }

    pub fn begin_with_shift(&mut self, position: Position, button: MouseButton, shift: bool) {
        self.begin_with_modifiers(position, button, KeyModifiers { shift, ..Default::default() });
    }

    pub fn begin_with_modifiers(&mut self, position: Position, button: MouseButton, modifiers: KeyModifiers) {
        self.finish();
        let shift = modifiers.shift;
        if self.tool == Tool::Pipette {
            self.with_state(|state| {
                let sampled = state.get_buffer().char_at(position).attribute;
                let mut attribute = state.get_caret().attribute;
                if button != MouseButton::Right && !modifiers.ctrl && !modifiers.meta {
                    attribute.set_foreground_color(sampled.foreground_color());
                }
                if button == MouseButton::Right || !shift {
                    attribute.set_background_color(sampled.background_color());
                }
                state.set_caret_attribute(attribute);
            });
            self.tool = Tool::Click;
            return;
        }
        if matches!(self.tool, Tool::Click | Tool::Font) && button != MouseButton::Left {
            return;
        }
        if !matches!(self.tool, Tool::Click | Tool::Select | Tool::Font | Tool::Tag) && !self.can_paint() {
            return;
        }
        if self.tool == Tool::Fill {
            let half = self.brush.primary == BrushPrimaryMode::HalfBlock;
            let cell = if half { Position::new(position.x, position.y / 2) } else { position };
            self.with_state(|state| crate::fill::fill(state, self.brush, cell, !half || position.y % 2 == 0, button, shift));
            return;
        }
        let undo = self.with_state(|state| state.begin_atomic_undo(self.tool.name()));
        let mut tag_offsets = Vec::new();
        let mut tag_create = false;
        if self.tool == Tool::Tag {
            let hit = self.with_state(|state| state.get_buffer().tags.iter().position(|tag| tag.contains(position)));
            if let Some(index) = hit {
                if modifiers.ctrl || modifiers.meta {
                    if self.selected_tags.contains(&index) {
                        self.selected_tags.retain(|selected| *selected != index);
                    } else {
                        self.selected_tags.push(index);
                    }
                    if let Some(index) = self.selected_tags.last().copied() {
                        self.with_state(|state| state.set_current_tag(index));
                    }
                    return;
                }
                if !self.selected_tags.contains(&index) {
                    self.selected_tags = vec![index];
                }
                self.with_state(|state| state.set_current_tag(index));
                tag_offsets = self.with_state(|state| {
                    self.selected_tags
                        .iter()
                        .filter_map(|index| state.get_buffer().tags.get(*index).map(|tag| (*index, tag.position)))
                        .collect()
                });
            } else {
                self.selected_tags.clear();
                self.with_state(|state| state.set_caret_from_document_position(position));
                // Shift+drag always selects; a plain drag along one row creates a tag.
                tag_create = !shift && !modifiers.ctrl && !modifiers.meta;
            }
            if button != MouseButton::Left {
                return;
            }
        }
        let add_type = if self.tool != Tool::Select {
            AddType::Default
        } else if shift {
            AddType::Add
        } else if modifiers.ctrl || modifiers.meta {
            AddType::Subtract
        } else {
            AddType::Default
        };
        let layer_offset = self.with_state(|state| {
            state
                .get_cur_layer()
                .filter(|layer| {
                    self.tool == Tool::Click
                        && button == MouseButton::Left
                        && (self.paste_active() || modifiers.ctrl || modifiers.meta || layer.role == icy_engine::Role::Image)
                        && !layer.properties.is_position_locked
                        && !layer.properties.is_locked
                })
                .map(|layer| layer.offset())
        });
        if self.tool == Tool::Select && self.selection_mode != SelectionMode::Rectangle {
            self.with_state(|state| {
                let sample = state.get_cur_layer().map(|layer| layer.char_at(position - layer.offset())).unwrap_or_default();
                state.enumerate_selections(|_, character, _| {
                    let matches = match self.selection_mode {
                        SelectionMode::Character => character.ch == sample.ch,
                        SelectionMode::Attribute => character.attribute == sample.attribute,
                        SelectionMode::Foreground => character.attribute.foreground_color() == sample.attribute.foreground_color(),
                        SelectionMode::Background => character.attribute.background_color() == sample.attribute.background_color(),
                        SelectionMode::Rectangle => false,
                    };
                    match add_type {
                        AddType::Default => Some(matches),
                        AddType::Add => matches.then_some(true),
                        AddType::Subtract => matches.then_some(false),
                    }
                });
            });
            return;
        }
        let mut selection_drag = SelectionDrag::None;
        let mut selection_rect = None;
        if layer_offset.is_none() && matches!(self.tool, Tool::Click | Tool::Font | Tool::Select) {
            let arbitrary_selection = self.tool == Tool::Select;
            self.with_state(|state| {
                if add_type == AddType::Default {
                    if arbitrary_selection {
                        let _ = state.clear_selection_mask();
                    }
                    selection_drag = hit_test_selection(state.selection(), position);
                    selection_rect = state.selection().map(|selection| selection.as_rectangle());
                    if selection_drag == SelectionDrag::None {
                        selection_drag = SelectionDrag::Create;
                        let _ = state.clear_selection();
                        if !arbitrary_selection {
                            state.set_caret_from_document_position(position);
                        }
                    }
                } else {
                    let _ = state.add_selection_to_mask();
                    let _ = state.deselect();
                    selection_drag = SelectionDrag::Create;
                }
            });
        }
        self.stroke = Some(Stroke {
            start: position,
            last: position,
            tool: self.tool,
            brush: self.brush,
            button,
            clear: shift && self.tool.is_shape_tool(),
            selection_drag,
            selection_rect,
            add_type,
            layer_offset,
            tag_offsets,
            tag_create,
            undo,
        });
        if self.tool == Tool::Pencil {
            self.stamp(position, self.brush, button);
        }
        self.update(position);
    }

    pub fn update(&mut self, position: Position) {
        let Some(stroke) = &self.stroke else {
            return;
        };
        let (start, last, tool, brush, button) = (stroke.start, stroke.last, stroke.tool, stroke.brush, stroke.button);
        let clear = stroke.clear;
        if tool == Tool::Tag {
            if stroke.tag_offsets.is_empty() {
                let area = Rectangle::from(
                    start.x.min(position.x),
                    start.y.min(position.y),
                    (start.x - position.x).abs() + 1,
                    (start.y - position.y).abs() + 1,
                );
                self.selected_tags = self.with_state(|state| {
                    state
                        .get_buffer()
                        .tags
                        .iter()
                        .enumerate()
                        .filter(|(_, tag)| {
                            tag.position.y >= area.top()
                                && tag.position.y <= area.bottom()
                                && tag.position.x <= area.right()
                                && tag.position.x + tag.len() as i32 - 1 >= area.left()
                        })
                        .map(|(index, _)| index)
                        .collect()
                });
            } else if last != position {
                let tag_offsets = stroke.tag_offsets.clone();
                let mut delta = position - start;
                let min_x = tag_offsets.iter().map(|(_, offset)| offset.x).min().unwrap_or(0);
                let min_y = tag_offsets.iter().map(|(_, offset)| offset.y).min().unwrap_or(0);
                delta.x = delta.x.max(-min_x);
                delta.y = delta.y.max(-min_y);
                self.with_state(|state| {
                    for (index, offset) in tag_offsets {
                        let _ = state.move_tag(index, offset + delta);
                    }
                });
            }
        } else if let Some(offset) = stroke.layer_offset {
            self.with_state(|state| state.set_layer_preview_offset(Some(offset + position - start)));
        } else if tool == Tool::Pencil && last != position {
            for point in icy_engine_edit::brushes::get_line_points(last, position).into_iter().skip(1) {
                self.stamp(point, brush, button);
            }
        } else if let Some(style) = self.box_line.filter(|_| tool == Tool::Line) {
            self.preview = box_lines::box_path(start, position);
            self.box_preview = if clear { Vec::new() } else { self.box_cells(start, position, style, button) };
        } else if tool.is_shape_tool() {
            self.preview = shape_points(tool, start, position);
        } else if matches!(tool, Tool::Select | Tool::Click | Tool::Font) {
            let selection = if stroke.selection_drag == SelectionDrag::Create && start == position {
                None
            } else if stroke.selection_drag == SelectionDrag::Create {
                // Mouse selections are always rectangular; Selection::new would default to terminal line mode.
                let mut selection = Selection::new(start);
                selection.lead = position;
                selection.shape = icy_engine::Shape::Rectangle;
                Some(selection)
            } else {
                stroke
                    .selection_rect
                    .and_then(|start_rect| {
                        compute_dragged_selection(
                            stroke.selection_drag,
                            DragParameters {
                                start_rect,
                                start_pos: start,
                                cur_pos: position,
                            },
                        )
                    })
                    .map(Selection::from)
            };
            if let Some(mut selection) = selection {
                selection.add_type = stroke.add_type;
                self.with_state(|state| state.set_selection(selection)).ok();
            }
        }
        self.stroke.as_mut().unwrap().last = position;
    }

    /// The characters of a box line (as Unicode) with their colors, joined with the box characters
    /// already on the layer, for the cells inside the layer and the selection. The colors are the
    /// caret's (swapped by the right button); the brush's Apply switches keep a cell's own
    /// foreground or background.
    pub fn box_cells(&self, start: Position, end: Position, style: BoxStyle, button: MouseButton) -> Vec<(Position, char, icy_engine::TextAttribute)> {
        let brush = self.brush;
        self.with_state(|state| {
            let Some(layer) = state.get_cur_layer() else {
                return Vec::new();
            };
            let (offset, width, height) = (layer.offset(), layer.width(), layer.height());
            let buffer_type = state.get_buffer().buffer_type;
            let inside = |point: Position| {
                let local = point - offset;
                local.x >= 0 && local.y >= 0 && local.x < width && local.y < height
            };
            let cells = box_lines::box_line(start, end, style, |point| {
                inside(point)
                    .then(|| layer.char_at(point - offset).ch)
                    .and_then(|ch| box_lines::arms_in(buffer_type, ch))
            });
            let mut caret = state.get_caret().attribute;
            if button == MouseButton::Right {
                let foreground = caret.foreground();
                caret.set_foreground(caret.background());
                caret.set_background(foreground);
            }
            let selected = state.is_something_selected();
            cells
                .into_iter()
                .filter(|(point, _)| inside(*point) && (!selected || state.is_selected(*point)))
                .map(|(point, ch)| {
                    // Start from the caret, so empty (transparent) cells become visible.
                    let own = layer.char_at(point - offset).attribute;
                    let mut attribute = caret;
                    if !brush.colorize_fg {
                        attribute.set_foreground(own.foreground());
                    }
                    if !brush.colorize_bg {
                        attribute.set_background(own.background());
                    }
                    (point, box_lines::code_for(buffer_type, ch), attribute)
                })
                .collect()
        })
    }

    /// Draws a box line, joining the box characters already on the layer; see [`Self::box_cells`].
    fn draw_box_line(&self, start: Position, end: Position, style: BoxStyle, button: MouseButton) {
        if !self.can_paint() {
            return;
        }
        let cells = self.box_cells(start, end, style, button);
        self.with_state(|state| {
            let Some(offset) = state.get_cur_layer().map(|layer| layer.offset()) else {
                return;
            };
            for (point, ch, attribute) in cells {
                let character = icy_engine::AttributedChar::new(ch, attribute);
                let _ = state.set_char_in_atomic(point - offset, character);
            }
        });
    }

    /// Whether positions of the current tool are quarter block pixels rather than characters.
    pub fn draws_pixels(&self) -> bool {
        self.quarter_blocks && self.tool == Tool::Pencil
            || self.quarter_blocks && self.tool.is_shape_tool() && !(self.tool == Tool::Line && self.box_line.is_some())
    }

    /// Sets (left button) or clears the quarter block pixel at `pixel`.
    fn stamp_pixel(&self, pixel: Position, button: MouseButton) {
        self.with_state(|state| {
            let cell = crate::quarter_blocks::cell_of(pixel);
            if state.is_something_selected() && !state.is_selected(cell) {
                return;
            }
            let attribute = state.get_caret().attribute;
            let Some(layer) = state.get_cur_layer() else {
                return;
            };
            let local = cell - layer.offset();
            if local.x < 0 || local.y < 0 || local.x >= layer.width() || local.y >= layer.height() {
                return;
            }
            let code = crate::quarter_blocks::with_pixel(layer.char_at(local).ch, pixel, button != MouseButton::Right);
            let _ = state.set_char_in_atomic(local, icy_engine::AttributedChar::new(code, attribute));
        });
    }

    fn stamp(&self, position: Position, brush: BrushSettings, button: MouseButton) {
        if !self.can_paint() {
            return;
        }
        if self.draws_pixels() {
            self.stamp_pixel(position, button);
            return;
        }
        self.with_state(|state| {
            if brush.primary == BrushPrimaryMode::HalfBlock {
                let size = brush.brush_size.max(1) as i32;
                for row in 0..size {
                    for column in 0..size {
                        let point = position + Position::new(column - size / 2, row - size / 2);
                        if point.y >= 0 {
                            apply_stamp_at_doc_pos(state, brush, Position::new(point.x, point.y / 2), point.y % 2 == 0, button);
                        }
                    }
                }
            } else {
                apply_stamp_at_doc_pos(state, brush, position, true, button);
            }
        });
    }

    pub fn finish(&mut self) {
        if let Some(stroke) = self.stroke.take() {
            if stroke.tool == Tool::Tag && stroke.tag_create && stroke.start != stroke.last && stroke.start.y == stroke.last.y {
                let left = stroke.start.x.min(stroke.last.x);
                let width = (stroke.start.x - stroke.last.x).unsigned_abs() as usize + 1;
                self.selected_tags.clear();
                self.new_tag_request = Some((Position::new(left, stroke.start.y), width));
            }
            if let Some(offset) = stroke.layer_offset {
                self.with_state(|state| {
                    state.set_layer_preview_offset(None);
                    let _ = state.move_layer(offset + stroke.last - stroke.start);
                });
            }
            if stroke.selection_drag == SelectionDrag::Create && stroke.start == stroke.last {
                self.with_state(|state| {
                    if stroke.tool == Tool::Select {
                        let _ = state.deselect();
                    } else {
                        let _ = state.clear_selection();
                    }
                });
            }
            if matches!(stroke.add_type, AddType::Add | AddType::Subtract) {
                self.with_state(|state| {
                    let _ = state.add_selection_to_mask();
                    let _ = state.deselect();
                });
            }
            if stroke.tool.is_shape_tool() {
                if let Some(style) = self.box_line.filter(|_| stroke.tool == Tool::Line && !stroke.clear) {
                    self.preview.clear();
                    self.box_preview.clear();
                    self.draw_box_line(stroke.start, stroke.last, style, stroke.button);
                }
                for point in std::mem::take(&mut self.preview) {
                    if stroke.clear {
                        if !self.can_paint() {
                            continue;
                        }
                        self.with_state(|state| {
                            use icy_engine::TextPane;
                            let point = if self.quarter_blocks && stroke.tool.is_shape_tool() {
                                crate::quarter_blocks::cell_of(point)
                            } else if stroke.brush.primary == BrushPrimaryMode::HalfBlock {
                                Position::new(point.x, point.y / 2)
                            } else {
                                point
                            };
                            if let Some(layer) = state.get_cur_layer() {
                                let local = point - layer.offset();
                                if local.x >= 0
                                    && local.y >= 0
                                    && local.x < layer.width()
                                    && local.y < layer.height()
                                    && (!state.is_something_selected() || state.is_selected(point))
                                {
                                    let _ = state.set_char_in_atomic(local, icy_engine::AttributedChar::invisible());
                                }
                            }
                        });
                    } else {
                        self.stamp(point, stroke.brush, stroke.button);
                    }
                }
            }
            drop(stroke);
        }
        self.preview.clear();
        self.box_preview.clear();
    }

    pub fn cancel(&mut self) {
        if let Some(mut stroke) = self.stroke.take() {
            self.with_state(|state| {
                state.set_layer_preview_offset(None);
                stroke.undo.discard_and_undo(state);
            });
        }
        self.preview.clear();
        self.box_preview.clear();
    }

    pub fn undo(&mut self) -> DrawResult<()> {
        if self.paste_active() {
            return self.paste_action(PasteAction::Cancel);
        }
        self.finish();
        self.edit_tags(|state| state.undo()).map_err(|error| error.to_string())
    }

    pub fn redo(&mut self) -> DrawResult<()> {
        if self.paste_active() {
            return Ok(());
        }
        self.finish();
        self.edit_tags(|state| state.redo()).map_err(|error| error.to_string())
    }

    /// Runs an edit that may add or remove tags. The selected tags are indices, which a removal
    /// shifts, so the tag selection is dropped when the number of tags changes.
    pub fn edit_tags<T>(&mut self, action: impl FnOnce(&mut EditState) -> T) -> T {
        let count = self.with_state(|state| state.get_buffer().tags.len());
        let result = self.with_state(action);
        if self.with_state(|state| state.get_buffer().tags.len()) != count {
            self.selected_tags.clear();
        }
        result
    }

    pub fn type_text(&mut self, text: &str) -> DrawResult<()> {
        self.finish();
        if !self.can_paint() {
            return Ok(());
        }
        self.with_state(|state| {
            let _undo = state.begin_atomic_undo("Type text");
            for character in text.chars() {
                match character {
                    '\r' => {}
                    '\n' => state.new_line()?,
                    '\t' => state.handle_tab(),
                    _ => {
                        let encoded = if self.outline_font {
                            let upper = outline_digit(character).unwrap_or(character.to_ascii_uppercase());
                            if !matches!(upper, 'A'..='Q' | '@' | '&' | ' ' | '\u{00ff}') {
                                continue;
                            }
                            upper
                        } else {
                            let buffer_type = state.get_buffer().buffer_type;
                            let encoded = buffer_type.convert_from_unicode(character);
                            if self.inverse && buffer_type == icy_engine::BufferType::Atascii {
                                char::from((encoded as u32 as u8) | 0x80)
                            } else {
                                encoded
                            }
                        };
                        state.type_key(encoded)?;
                    }
                }
            }
            Ok::<(), icy_engine::EngineError>(())
        })
        .map_err(|error| error.to_string())
    }

    /// Types the character with the code `code` of the document's character set, as it is.
    pub fn type_code(&mut self, code: char) -> DrawResult<()> {
        self.finish();
        if !self.can_paint() {
            return Ok(());
        }
        self.with_state(|state| {
            let _undo = state.begin_atomic_undo("Type text");
            state.type_key(code)
        })
        .map_err(|error| error.to_string())
    }

    pub fn can_paint(&self) -> bool {
        !self.paste_active()
            && self.with_state(|state| {
                state
                    .get_cur_layer()
                    .is_some_and(|layer| !layer.properties.is_locked && layer.is_visible() && layer.role != icy_engine::Role::Image)
            })
    }

    pub fn preview_color(&self) -> (u8, u8, u8) {
        self.with_state(|state| {
            let attribute = state.get_caret().attribute;
            let button = self.stroke.as_ref().map_or(MouseButton::Left, |stroke| stroke.button);
            crate::paint::compute_preview_color(&self.brush, attribute.foreground(), attribute.background(), &state.get_buffer().palette, button)
        })
    }

    pub fn type_art_text(&mut self, text: &str, font: &retrofont::Font, outline_style: usize) -> DrawResult<()> {
        self.finish();
        if !self.can_paint() {
            return Ok(());
        }
        let art_line = self.art_line;
        let (caret, line_start) = self.with_state(|state| {
            let _undo = state.begin_typed_atomic_undo("Render font character", icy_engine_edit::OperationType::RenderCharacter);
            state.undo_caret_position()?;
            let caret = state.get_caret().position();
            // Text continues its line unless the caret was moved elsewhere since.
            let line_start = art_line.filter(|(after, _)| *after == caret).map_or(caret.x, |(_, column)| column);
            for character in text.chars() {
                let start = state.get_caret().position();
                if character == '\n' {
                    state.set_caret_position(Position::new(line_start, start.y + font.max_height() as i32));
                } else if let Some(glyph) = art_glyph(font, character) {
                    let mut renderer = icy_engine_edit::TdfEditStateRenderer::new(state, start.x, start.y)?;
                    if matches!(font, retrofont::Font::Tdf(tdf) if matches!(tdf.font_type(), retrofont::tdf::TdfFontType::Block | retrofont::tdf::TdfFontType::Outline)) {
                        if let Some((width, height)) = font.glyph_size(glyph) {
                            renderer.fill_background(width, height)?;
                        }
                    }
                    font.render_glyph(
                        &mut renderer,
                        glyph,
                        &retrofont::RenderOptions {
                            outline_style,
                            ..Default::default()
                        },
                    )
                    .map_err(|error| icy_engine::EngineError::Generic(error.to_string()))?;
                    let end = renderer.max_x();
                    state.set_caret_position(Position::new(end, start.y));
                }
            }
            Ok::<_, icy_engine::EngineError>((state.get_caret().position(), line_start))
        })
        .map_err(|error| error.to_string())?;
        self.art_line = Some((caret, line_start));
        Ok(())
    }

    pub fn font_backspace(&mut self) -> DrawResult<()> {
        use icy_engine_edit::OperationType;
        self.finish();
        if !self.can_paint() {
            return Ok(());
        }
        self.with_state(|state| {
            let stack = state.get_undo_stack();
            let stack = stack.lock().unwrap();
            let mut reversed = 0;
            let mut operation = None;
            for index in (0..stack.len()).rev() {
                match stack[index].get_operation_type() {
                    OperationType::RenderCharacter if reversed == 0 => {
                        operation = stack[index].try_clone();
                        break;
                    }
                    OperationType::RenderCharacter => reversed -= 1,
                    OperationType::ReversedRenderCharacter => reversed += 1,
                    OperationType::Unknown => break,
                }
            }
            drop(stack);
            if let Some(operation) = operation {
                state.push_reverse_undo("Undo font character", operation, OperationType::ReversedRenderCharacter)
            } else if state.is_something_selected() {
                state.erase_selection()
            } else {
                state.backspace()
            }
        })
        .map_err(|error| error.to_string())?;
        // Backspace stays on the same text, so its line keeps its start column.
        let caret = self.with_state(|state| state.get_caret().position());
        if let Some((after, _)) = &mut self.art_line {
            *after = caret;
        }
        Ok(())
    }

    pub fn tag_preview_position(&self, index: usize, position: Position) -> Position {
        if let Some(stroke) = &self.stroke {
            if stroke.tool == Tool::Tag && stroke.tag_offsets.iter().any(|(tag, _)| *tag == index) {
                return self.with_state(|state| state.get_buffer().tags.get(index).map_or(position, |tag| tag.position));
            }
        }
        position
    }

    pub fn tag_drag_active(&self) -> bool {
        self.stroke
            .as_ref()
            .is_some_and(|stroke| stroke.tool == Tool::Tag && !stroke.tag_offsets.is_empty() && stroke.start != stroke.last)
    }

    /// The selection handle for the mouse cursor: the one being dragged, or else the one under `position`.
    pub fn selection_handle(&self, position: Option<Position>) -> SelectionDrag {
        if let Some(stroke) = &self.stroke {
            return if stroke.layer_offset.is_none() {
                stroke.selection_drag
            } else {
                SelectionDrag::None
            };
        }
        let handles = match self.tool {
            Tool::Select => self.selection_mode == SelectionMode::Rectangle,
            Tool::Click => !self.with_state(|state| state.get_cur_layer().is_some_and(|layer| layer.role == icy_engine::Role::Image)),
            Tool::Font => true,
            _ => false,
        };
        match position {
            Some(position) if handles && !self.paste_active() => self.with_state(|state| hit_test_selection(state.selection(), position)),
            _ => SelectionDrag::None,
        }
    }

    pub fn tag_selection_rectangle(&self) -> Option<Rectangle> {
        let stroke = self.stroke.as_ref()?;
        (stroke.tool == Tool::Tag && stroke.tag_offsets.is_empty()).then(|| {
            Rectangle::from(
                stroke.start.x.min(stroke.last.x),
                stroke.start.y.min(stroke.last.y),
                (stroke.start.x - stroke.last.x).abs() + 1,
                (stroke.start.y - stroke.last.y).abs() + 1,
            )
        })
    }

    fn change_caret_colors(
        &mut self,
        label: &str,
        change_caret: impl FnOnce(&mut EditState),
        change_tag: impl Fn(&mut icy_engine::TextAttribute, icy_engine::TextAttribute),
    ) -> DrawResult<()> {
        let selected_tags = (self.tool == Tool::Tag).then(|| self.selected_tags.clone()).unwrap_or_default();
        self.with_state(|state| {
            change_caret(state);
            if selected_tags.is_empty() {
                return Ok(());
            }
            let caret_attribute = state.get_caret().attribute;
            let updates: Vec<_> = selected_tags
                .into_iter()
                .filter_map(|index| {
                    state.get_buffer().tags.get(index).cloned().and_then(|mut tag| {
                        let mut attribute = tag.attribute;
                        change_tag(&mut attribute, caret_attribute);
                        (attribute != tag.attribute).then(|| {
                            tag.attribute = attribute;
                            (index, tag)
                        })
                    })
                })
                .collect();
            if updates.is_empty() {
                return Ok(());
            }
            let _undo = state.begin_atomic_undo(label);
            for (index, tag) in updates {
                state.update_tag(tag, index)?;
            }
            Ok(())
        })
        .map_err(|error: icy_engine::EngineError| error.to_string())
    }

    pub fn set_caret_foreground(&mut self, color: u32) -> DrawResult<()> {
        self.change_caret_colors(
            "Change tag foreground",
            |state| state.set_caret_foreground(color),
            |attribute, caret| attribute.set_foreground_color(caret.foreground_color()),
        )
    }

    pub fn set_caret_background(&mut self, color: u32) -> DrawResult<()> {
        self.change_caret_colors(
            "Change tag background",
            |state| state.set_caret_background(color),
            |attribute, caret| attribute.set_background_color(caret.background_color()),
        )
    }

    pub fn swap_caret_colors(&mut self) -> DrawResult<()> {
        self.change_caret_colors(
            "Swap tag colors",
            |state| {
                state.swap_caret_colors();
            },
            |attribute, _| {
                let foreground = attribute.foreground_color();
                attribute.set_foreground_color(attribute.background_color());
                attribute.set_background_color(foreground);
            },
        )
    }

    pub fn reset_caret_colors(&mut self) -> DrawResult<()> {
        self.change_caret_colors(
            "Reset tag colors",
            |state| {
                state.set_caret_foreground(7);
                state.set_caret_background(0);
            },
            |attribute, caret| {
                attribute.set_foreground_color(caret.foreground_color());
                attribute.set_background_color(caret.background_color());
            },
        )
    }

    pub fn delete_selected_tags(&mut self) -> DrawResult<()> {
        self.finish();
        let mut indices = std::mem::take(&mut self.selected_tags);
        indices.sort_unstable();
        indices.dedup();
        self.with_state(|state| {
            let _undo = state.begin_atomic_undo("Delete tags");
            for index in indices.into_iter().rev() {
                state.remove_tag(index)?;
            }
            Ok::<(), icy_engine::EngineError>(())
        })
        .map_err(|error| error.to_string())
    }

    /// The selected tags in reading order.
    fn selected_tag_copies(&mut self) -> Vec<icy_engine::Tag> {
        let selected = self.selected_tags.clone();
        let mut tags: Vec<_> = self.with_state(|state| selected.iter().filter_map(|index| state.get_buffer().tags.get(*index).cloned()).collect());
        tags.sort_by_key(|tag| (tag.position.y, tag.position.x));
        tags
    }

    /// Adds `tags` shifted by `delta` and kept on the canvas, and selects them.
    fn add_tag_copies(&mut self, label: &str, tags: Vec<icy_engine::Tag>, delta: Position) -> DrawResult<()> {
        self.finish();
        let first = self.with_state(|state| state.get_buffer().tags.len());
        let count = tags.len();
        self.with_state(|state| {
            let (width, height) = (state.get_buffer().width(), state.get_buffer().height());
            let _undo = state.begin_atomic_undo(label);
            for mut tag in tags {
                let position = tag.position + delta;
                tag.position = Position::new(position.x.clamp(0, (width - tag.len() as i32).max(0)), position.y.clamp(0, (height - 1).max(0)));
                state.add_new_tag(tag)?;
            }
            Ok::<(), icy_engine::EngineError>(())
        })
        .map_err(|error| error.to_string())?;
        self.selected_tags = (first..first + count).collect();
        Ok(())
    }

    /// Moves the selected tags by `delta`, as far as they stay on the canvas. Returns false when
    /// no tag is selected.
    pub fn nudge_selected_tags(&mut self, delta: Position) -> DrawResult<bool> {
        self.finish();
        let tags = self.selected_tag_copies();
        if tags.is_empty() {
            return Ok(false);
        }
        let selected = self.selected_tags.clone();
        self.with_state(|state| {
            let (width, height) = (state.get_buffer().width(), state.get_buffer().height());
            let left = tags.iter().map(|tag| tag.position.x).min().unwrap_or(0);
            let top = tags.iter().map(|tag| tag.position.y).min().unwrap_or(0);
            let right = tags.iter().map(|tag| tag.position.x + tag.len() as i32).max().unwrap_or(0);
            let bottom = tags.iter().map(|tag| tag.position.y + 1).max().unwrap_or(0);
            let delta = Position::new(
                delta.x.clamp(-left, (width - right).max(-left)),
                delta.y.clamp(-top, (height - bottom).max(-top)),
            );
            if delta == Position::default() {
                return Ok(());
            }
            let _undo = state.begin_atomic_undo("Move tags");
            for index in selected {
                if let Some(position) = state.get_buffer().tags.get(index).map(|tag| tag.position) {
                    state.move_tag(index, position + delta)?;
                }
            }
            Ok::<(), icy_engine::EngineError>(())
        })
        .map_err(|error| error.to_string())?;
        Ok(true)
    }

    /// Copies the selected tags for [`Self::paste_tags`] and returns the text to put on the
    /// clipboard for them: their replacements, which is what the BBS would show.
    pub fn copy_selected_tags(&mut self) -> Option<String> {
        let tags = self.selected_tag_copies();
        if tags.is_empty() {
            return None;
        }
        let text = tags
            .iter()
            .map(|tag| {
                if tag.replacement_value.is_empty() {
                    &tag.preview
                } else {
                    &tag.replacement_value
                }
            })
            .cloned()
            .collect::<Vec<_>>()
            .join(" ");
        // The clipboard drops empty text, which would lose the paste.
        let text = if text.trim().is_empty() { "TAG".to_string() } else { text };
        self.tag_clipboard = Some((text.clone(), tags));
        Some(text)
    }

    /// Pastes the copied tags with their upper left one at the caret when `text` is still what
    /// was copied for them. Returns false for any other clipboard content.
    pub fn paste_tags(&mut self, text: &str) -> DrawResult<bool> {
        let Some((copied, tags)) = self.tag_clipboard.clone() else {
            return Ok(false);
        };
        if copied.trim_end() != text.replace("\r\n", "\n").trim_end() {
            return Ok(false);
        }
        let left = tags.iter().map(|tag| tag.position.x).min().unwrap_or(0);
        let top = tags.iter().map(|tag| tag.position.y).min().unwrap_or(0);
        let caret = self.with_state(|state| state.layer_to_document_position(state.get_caret().position()));
        self.add_tag_copies("Paste tags", tags, caret - Position::new(left, top))?;
        Ok(true)
    }

    /// Copies the selected tags one row further down and selects the copies.
    pub fn duplicate_selected_tags(&mut self) -> DrawResult<bool> {
        let tags = self.selected_tag_copies();
        if tags.is_empty() {
            return Ok(false);
        }
        self.add_tag_copies("Duplicate tags", tags, Position::new(0, 1))?;
        Ok(true)
    }

    /// Selects the next tag in reading order, or the previous one when `backward`, and moves the
    /// caret there. Returns false when there are no tags.
    pub fn select_next_tag(&mut self, backward: bool) -> bool {
        self.finish();
        let mut order: Vec<(Position, usize)> =
            self.with_state(|state| state.get_buffer().tags.iter().enumerate().map(|(index, tag)| (tag.position, index)).collect());
        if order.is_empty() {
            return false;
        }
        order.sort_by_key(|(position, index)| (position.y, position.x, *index));
        let count = order.len();
        let current = self
            .selected_tags
            .last()
            .and_then(|selected| order.iter().position(|(_, index)| index == selected));
        let next = match (current, backward) {
            (Some(current), false) => (current + 1) % count,
            (Some(current), true) => (current + count - 1) % count,
            (None, false) => 0,
            (None, true) => count - 1,
        };
        let (position, index) = order[next];
        self.selected_tags = vec![index];
        self.with_state(|state| {
            state.set_current_tag(index);
            state.set_caret_from_document_position(position);
        });
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use icy_engine::TextPane;

    #[test]
    fn paste_starts_at_the_upper_left_corner_of_the_selection() {
        let mut document = Document::new(Size::new(30, 20));
        document.type_text("AB").unwrap();
        document.tool = Tool::Click;
        // Drag from the lower right to the upper left, as reported by users.
        document.begin(Position::new(12, 8), MouseButton::Left);
        document.update(Position::new(5, 3));
        document.finish();
        document.start_paste("XY", None).unwrap();
        let offset = document.with_state(|state| state.get_cur_layer().unwrap().offset());
        assert_eq!(offset, Position::new(5, 3));
        document.paste_action(PasteAction::Cancel).unwrap();

        // Once the caret was moved away from the selection, the paste follows the caret.
        document.begin(Position::new(12, 8), MouseButton::Left);
        document.update(Position::new(5, 3));
        document.finish();
        document.with_state(|state| state.set_caret_position(Position::new(20, 2)));
        document.start_paste("XY", None).unwrap();
        assert_eq!(document.with_state(|state| state.get_cur_layer().unwrap().offset()), Position::new(20, 2));
        document.paste_action(PasteAction::Cancel).unwrap();

        // Without a selection the paste starts at the caret.
        document.begin(Position::new(7, 9), MouseButton::Left);
        document.finish();
        document.start_paste("XY", None).unwrap();
        assert_eq!(document.with_state(|state| state.get_cur_layer().unwrap().offset()), Position::new(7, 9));
    }

    #[test]
    fn selection_handles_follow_the_hover_and_the_drag() {
        let mut document = Document::new(Size::new(30, 20));
        document.tool = Tool::Select;
        document
            .with_state(|state| state.set_selection(Selection::from(Rectangle::from(5, 3, 10, 8))))
            .unwrap();
        assert_eq!(document.selection_handle(Some(Position::new(9, 6))), SelectionDrag::Move);
        assert_eq!(document.selection_handle(Some(Position::new(5, 3))), SelectionDrag::TopLeft);
        assert_eq!(document.selection_handle(Some(Position::new(20, 15))), SelectionDrag::None);
        assert_eq!(document.selection_handle(None), SelectionDrag::None);

        // While resizing, the handle stays even when the pointer leaves the corner.
        document.begin(Position::new(5, 3), MouseButton::Left);
        document.update(Position::new(2, 1));
        assert_eq!(document.selection_handle(Some(Position::new(20, 15))), SelectionDrag::TopLeft);
        document.finish();

        document.selection_mode = SelectionMode::Character;
        assert_eq!(document.selection_handle(Some(Position::new(9, 6))), SelectionDrag::None);
        document.selection_mode = SelectionMode::Rectangle;
        document.tool = Tool::Pencil;
        assert_eq!(document.selection_handle(Some(Position::new(9, 6))), SelectionDrag::None);
    }

    #[test]
    fn click_and_select_tools_differ_in_mask_handling() {
        let mut document = Document::new(Size::new(30, 20));

        // Click mode drags plain rectangles and moves the caret, like the original click tool.
        document.tool = Tool::Click;
        document.begin(Position::new(10, 10), MouseButton::Left);
        document.update(Position::new(12, 12));
        document.finish();
        assert_eq!(document.with_state(|state| state.selection().unwrap().shape), icy_engine::Shape::Rectangle);
        assert_eq!(document.with_state(|state| state.get_caret().position()), Position::new(10, 10));
        document.begin(Position::new(20, 5), MouseButton::Left);
        document.finish();
        assert!(document.with_state(|state| !state.is_something_selected()));

        // The select tool builds arbitrary masks and leaves the caret alone.
        document.tool = Tool::Select;
        document.begin_with_modifiers(
            Position::new(1, 1),
            MouseButton::Left,
            KeyModifiers {
                shift: true,
                ..Default::default()
            },
        );
        document.update(Position::new(4, 4));
        document.finish();
        assert_eq!(document.with_state(|state| state.get_caret().position()), Position::new(20, 5));
        assert!(document.with_state(|state| state.selection().is_none() && state.get_is_mask_selected(Position::new(2, 2))));
        document.begin_with_modifiers(
            Position::new(3, 3),
            MouseButton::Left,
            KeyModifiers {
                ctrl: true,
                ..Default::default()
            },
        );
        document.update(Position::new(6, 6));
        document.finish();
        assert!(document.with_state(|state| !state.get_is_mask_selected(Position::new(3, 3)) && state.get_is_mask_selected(Position::new(1, 1))));

        // A replacing press drops the accumulated mask again.
        document.begin(Position::new(20, 5), MouseButton::Left);
        document.finish();
        assert!(document.with_state(|state| !state.is_something_selected()));
    }

    #[test]
    fn matching_selection_modes_build_arbitrary_masks() {
        let mut document = Document::new(Size::new(20, 6));
        document.type_text("AB\nBA").unwrap();
        document.tool = Tool::Select;
        document.selection_mode = SelectionMode::Character;
        document.begin(Position::new(0, 0), MouseButton::Left);
        assert!(!document.stroke_active());
        assert!(document.with_state(|state| state.get_is_mask_selected(Position::new(0, 0)) && state.get_is_mask_selected(Position::new(1, 1))));
        assert!(document.with_state(|state| !state.get_is_mask_selected(Position::new(1, 0))));
        document.begin_with_modifiers(
            Position::new(1, 0),
            MouseButton::Left,
            KeyModifiers {
                shift: true,
                ..Default::default()
            },
        );
        assert!(document.with_state(|state| state.get_is_mask_selected(Position::new(1, 0)) && state.get_is_mask_selected(Position::new(0, 0))));
        document.begin_with_modifiers(
            Position::new(0, 0),
            MouseButton::Left,
            KeyModifiers {
                ctrl: true,
                ..Default::default()
            },
        );
        assert!(document.with_state(|state| !state.get_is_mask_selected(Position::new(0, 0)) && state.get_is_mask_selected(Position::new(1, 0))));
        document.undo().unwrap();
        assert!(document.with_state(|state| state.get_is_mask_selected(Position::new(0, 0))));
    }

    #[test]
    fn selection_masks_add_subtract_match_and_resize() {
        let mut document = Document::new(Size::new(30, 20));
        document.tool = Tool::Select;
        document.begin(Position::new(1, 1), MouseButton::Left);
        document.update(Position::new(10, 10));
        document.finish();
        document.begin(Position::new(5, 5), MouseButton::Left);
        document.update(Position::new(7, 6));
        document.finish();
        assert_eq!(
            document.with_state(|state| state.selection().unwrap().as_rectangle().start),
            Position::new(3, 2)
        );
        document.begin_with_modifiers(
            Position::new(20, 1),
            MouseButton::Left,
            KeyModifiers {
                shift: true,
                ..Default::default()
            },
        );
        document.update(Position::new(22, 3));
        document.finish();
        assert!(document.with_state(|state| state.is_selected(Position::new(5, 5)) && state.is_selected(Position::new(21, 2))));
        document.begin_with_modifiers(
            Position::new(4, 4),
            MouseButton::Left,
            KeyModifiers {
                ctrl: true,
                ..Default::default()
            },
        );
        document.update(Position::new(6, 6));
        document.finish();
        assert!(!document.with_state(|state| state.is_selected(Position::new(5, 5))));
        assert!(document.with_state(|state| state.is_selected(Position::new(21, 2))));
        document.with_state(|state| {
            state
                .set_char(Position::new(2, 2), icy_engine::AttributedChar::new('X', icy_engine::TextAttribute::default()))
                .unwrap();
            state
                .set_char(Position::new(12, 4), icy_engine::AttributedChar::new('X', icy_engine::TextAttribute::default()))
                .unwrap();
        });
        document.selection_mode = SelectionMode::Character;
        document.begin(Position::new(2, 2), MouseButton::Left);
        assert!(document.with_state(|state| state.is_selected(Position::new(12, 4))));
        assert!(!document.with_state(|state| state.is_selected(Position::new(21, 2))));
        assert!(!document.stroke_active());
    }

    #[test]
    fn layer_drag_previews_cancel_and_commit_with_position_locks() {
        let mut document = Document::new(Size::new(20, 10));
        let modifiers = KeyModifiers {
            ctrl: true,
            ..Default::default()
        };
        document.begin_with_modifiers(Position::new(1, 1), MouseButton::Left, modifiers.clone());
        document.update(Position::new(4, 3));
        document.cancel();
        assert_eq!(document.with_state(|state| state.get_cur_layer().unwrap().offset()), Position::default());
        document.begin_with_modifiers(Position::new(1, 1), MouseButton::Left, modifiers.clone());
        document.update(Position::new(4, 3));
        document.finish();
        assert_eq!(document.with_state(|state| state.get_cur_layer().unwrap().offset()), Position::new(3, 2));
        document.undo().unwrap();
        assert_eq!(document.with_state(|state| state.get_cur_layer().unwrap().offset()), Position::default());
        document.with_state(|state| state.get_cur_layer_mut().unwrap().properties.is_position_locked = true);
        document.begin_with_modifiers(Position::new(1, 1), MouseButton::Left, modifiers);
        document.update(Position::new(4, 3));
        document.finish();
        assert_eq!(document.with_state(|state| state.get_cur_layer().unwrap().offset()), Position::default());
    }

    #[test]
    fn pipette_samples_only_requested_colors_and_returns_to_text() {
        for (button, modifiers, foreground, background) in [
            (MouseButton::Left, KeyModifiers::default(), true, true),
            (MouseButton::Right, KeyModifiers::default(), false, true),
            (
                MouseButton::Left,
                KeyModifiers {
                    shift: true,
                    ..Default::default()
                },
                true,
                false,
            ),
            (
                MouseButton::Left,
                KeyModifiers {
                    meta: true,
                    ..Default::default()
                },
                false,
                true,
            ),
        ] {
            let mut document = Document::new(Size::new(10, 5));
            let mut sample = icy_engine::TextAttribute::from_color(2, 3);
            sample.set_foreground_rgb(10, 20, 30);
            document.with_state(|state| state.set_char(Position::new(1, 1), icy_engine::AttributedChar::new('Q', sample)).unwrap());
            let before = document.with_state(|state| state.get_caret().attribute);
            document.brush.paint_char = '#';
            document.tool = Tool::Pipette;
            document.begin_with_modifiers(Position::new(1, 1), button, modifiers);
            let after = document.with_state(|state| state.get_caret().attribute);
            assert_eq!(
                after.foreground_color(),
                if foreground { sample.foreground_color() } else { before.foreground_color() }
            );
            assert_eq!(
                after.background_color(),
                if background { sample.background_color() } else { before.background_color() }
            );
            assert_eq!(after.attr, before.attr);
            assert_eq!(document.brush.paint_char, '#');
            assert_eq!(document.tool, Tool::Click);
        }
    }

    #[test]
    fn text_and_font_drag_select_in_document_coordinates() {
        for tool in [Tool::Click, Tool::Font] {
            let mut document = Document::new(Size::new(20, 10));
            document.tool = tool;
            document.with_state(|state| state.move_layer(Position::new(3, 2)).unwrap());
            document.begin(Position::new(4, 3), MouseButton::Left);
            assert_eq!(document.with_state(|state| state.get_caret().position()), Position::new(1, 1));
            document.update(Position::new(8, 5));
            document.finish();
            let selection = document.with_state(|state| state.selection().unwrap());
            assert_eq!(selection.anchor, Position::new(4, 3));
            assert_eq!(selection.lead, Position::new(8, 5));
            document.begin(Position::new(10, 6), MouseButton::Right);
            assert!(!document.stroke_active());
            assert_eq!(document.with_state(|state| state.selection()), Some(selection));
            document.begin(Position::new(11, 6), MouseButton::Left);
            document.finish();
            assert!(document.with_state(|state| state.selection().is_none()));
        }
    }

    #[test]
    fn pencil_stroke_is_one_undo_and_respects_offset() {
        let mut doc = Document::new(Size::new(20, 10));
        doc.tool = Tool::Pencil;
        doc.brush.primary = BrushPrimaryMode::Char;
        doc.brush.paint_char = '#';
        doc.with_state(|state| {
            state.add_new_layer(0).unwrap();
            state.move_layer(Position::new(3, 2)).unwrap();
        });
        let before = doc.with_state(|state| state.undo_stack_len());
        doc.begin(Position::new(4, 3), MouseButton::Left);
        doc.update(Position::new(8, 3));
        doc.finish();
        assert_eq!(doc.with_state(|state| state.undo_stack_len()), before + 1);
        assert_eq!(doc.with_state(|state| state.get_cur_layer().unwrap().char_at((1, 1).into()).ch), '#');
        doc.undo().unwrap();
        assert_ne!(doc.with_state(|state| state.get_cur_layer().unwrap().char_at((1, 1).into()).ch), '#');
        doc.redo().unwrap();
        assert_eq!(doc.with_state(|state| state.get_cur_layer().unwrap().char_at((5, 1).into()).ch), '#');
    }

    #[test]
    fn half_block_pencil_paints_brush_size_half_pixels() {
        let mut doc = Document::new(Size::new(20, 10));
        doc.tool = Tool::Pencil;
        doc.brush.primary = BrushPrimaryMode::HalfBlock;
        doc.brush.brush_size = 3;
        doc.with_state(|state| state.set_caret_attribute(icy_engine::TextAttribute::from_color(4, 0)));
        // Half-block position (10, 10) is the top half of cell (10, 5).
        doc.begin(Position::new(10, 10), MouseButton::Left);
        doc.finish();
        let cell = |x: i32, y: i32| doc.with_state(|state| state.get_buffer().char_at((x, y).into()));
        for x in 9..=11 {
            let (upper, middle, lower) = (cell(x, 4), cell(x, 5), cell(x, 6));
            assert_eq!(upper.ch, '\u{DC}', "the lower half of row 4 at column {x}");
            assert_eq!(upper.attribute.foreground(), 4);
            assert_eq!(middle.ch, '\u{DB}', "both halves of row 5 at column {x}");
            assert_eq!(lower.ch, ' ', "row 6 stays empty at column {x}");
        }
        for x in [8, 12] {
            assert_eq!(cell(x, 5).ch, ' ', "column {x} is outside the brush");
        }
    }

    #[test]
    fn box_lines_join_what_they_cross_in_one_undo_step() {
        let mut doc = Document::new(Size::new(20, 10));
        doc.tool = Tool::Line;
        doc.brush.primary = BrushPrimaryMode::Char;
        doc.box_line = Some(BoxStyle::Single);
        let line = |doc: &mut Document, start: (i32, i32), end: (i32, i32), button| {
            doc.begin(Position::new(start.0, start.1), button);
            doc.update(Position::new(end.0, end.1));
            doc.finish();
        };
        let row = |doc: &Document, y: i32| -> String {
            doc.with_state(|state| {
                let buffer = state.get_buffer();
                (0..8)
                    .map(|x| buffer.buffer_type.convert_to_unicode(buffer.char_at((x, y).into()).ch))
                    .collect()
            })
        };
        line(&mut doc, (3, 0), (3, 4), MouseButton::Left);
        line(&mut doc, (0, 2), (6, 2), MouseButton::Left);
        assert_eq!(row(&doc, 2), "───┼─── ", "the second line crosses the first");
        doc.box_line = Some(BoxStyle::Double);
        line(&mut doc, (3, 4), (6, 4), MouseButton::Left);
        assert_eq!(
            row(&doc, 4),
            "   ╘═══ ",
            "a double line from the end of a single one turns its end into a corner"
        );
        assert_eq!(row(&doc, 3), "   │    ");
        doc.undo().unwrap();
        assert_eq!(row(&doc, 4), "   │    ", "one undo step per line");

        // The right button swaps the colors, Shift erases along the line like other shapes.
        doc.with_state(|state| state.set_caret_foreground(14));
        line(&mut doc, (0, 6), (2, 6), MouseButton::Right);
        let attribute = doc.with_state(|state| state.get_buffer().char_at((1, 6).into()).attribute);
        assert_eq!((attribute.foreground(), attribute.background()), (0, 14));
        doc.box_line = None;
        doc.brush.paint_char = '#';
        line(&mut doc, (0, 8), (2, 8), MouseButton::Left);
        assert_eq!(row(&doc, 8), "###     ", "without the box style the brush draws");

        // While dragging, the preview holds the characters the line will draw, joins included.
        doc.box_line = Some(BoxStyle::Single);
        doc.begin(Position::new(0, 1), MouseButton::Left);
        doc.update(Position::new(5, 1));
        let preview: String = doc
            .box_preview
            .iter()
            .map(|(_, ch, _)| icy_engine::BufferType::CP437.convert_to_unicode(*ch))
            .collect();
        assert_eq!(preview, "───┼──", "the preview crosses the existing line");
        assert_eq!(row(&doc, 1), "   │    ", "the canvas only changes when the line is finished");
        doc.finish();
        assert!(doc.box_preview.is_empty());
        assert_eq!(row(&doc, 1), "───┼──  ");

        // The brush's Apply switches keep a cell's own colors, like for the other modes.
        doc.with_state(|state| {
            state.set_caret_foreground(4);
            state.set_caret_background(1);
        });
        doc.brush.colorize_bg = false;
        line(&mut doc, (0, 9), (2, 9), MouseButton::Left);
        let attribute = doc.with_state(|state| state.get_buffer().char_at((1, 9).into()).attribute);
        assert_eq!((attribute.foreground(), attribute.background()), (4, 0), "the background stays");
    }

    #[test]
    fn shape_preview_is_non_destructive_and_cancel_drops_it() {
        let mut doc = Document::new(Size::new(20, 10));
        doc.tool = Tool::RectangleOutline;
        doc.brush.primary = BrushPrimaryMode::Char;
        doc.brush.paint_char = '#';
        doc.begin(Position::new(1, 1), MouseButton::Left);
        doc.update(Position::new(6, 4));
        assert!(!doc.preview.is_empty());
        assert!(!doc.modified());
        doc.cancel();
        assert!(!doc.modified());
        assert!(doc.preview.is_empty());
        doc.begin(Position::new(1, 1), MouseButton::Left);
        doc.update(Position::new(6, 4));
        doc.finish();
        assert_eq!(doc.with_state(|state| state.get_buffer().char_at((6, 4).into()).ch), '#');
        doc.undo().unwrap();
        assert!(!doc.modified());
    }

    #[test]
    fn typing_uses_engine_encoding_and_undo() {
        let mut doc = Document::new(Size::new(20, 10));
        doc.type_text("Hello\nWorld").unwrap();
        assert_eq!(doc.with_state(|state| state.get_buffer().char_at((0, 1).into()).ch), 'W');
        doc.undo().unwrap();
        assert!(!doc.modified());
    }

    #[test]
    fn shift_shape_clears_cells_and_is_undoable() {
        let mut document = Document::new(Size::new(20, 10));
        document.type_text("ABC").unwrap();
        document.tool = Tool::Line;
        document.brush.primary = BrushPrimaryMode::Char;
        document.begin_with_shift(Position::new(0, 0), MouseButton::Left, true);
        document.update(Position::new(2, 0));
        document.finish();
        assert!(!document.with_state(|state| state.get_cur_layer().unwrap().char_at((1, 0).into()).is_visible()));
        document.undo().unwrap();
        assert_eq!(document.with_state(|state| state.get_buffer().char_at((1, 0).into()).ch), 'B');
    }

    #[test]
    fn plugin_writes_are_a_single_undo_action() {
        let mut document = Document::new(Size::new(20, 10));
        let output =
            crate::plugins::Plugin::run_script_string(&document.screen, "buf:set_char(0, 0, 'A'); buf:set_char(1, 0, 'B'); log('done')", "Plugin").unwrap();
        assert_eq!(output, "done");
        assert_eq!(document.with_state(|state| state.undo_stack_len()), 1);
        assert_eq!(document.with_state(|state| state.get_buffer().char_at((1, 0).into()).ch), 'B');
        document.undo().unwrap();
        assert!(!document.modified());
    }

    #[test]
    fn text_art_glyph_uses_colors_and_one_undo() {
        let mut font = crate::charfont::CharFontDocument::new(icy_engine_edit::charset::TdfFontType::Color);
        let mut glyph = font.document();
        glyph.with_state(|state| state.set_caret_attribute(icy_engine::TextAttribute::from_color(12, 0)));
        glyph.type_text("##").unwrap();
        font.commit(&mut glyph);
        let reopened = font.document();
        assert_eq!(
            reopened.with_state(|state| state.get_buffer().char_at((0, 0).into()).attribute.as_u8(icy_engine::IceMode::Blink) & 15),
            12
        );
        let fonts = retrofont::Font::load(&font.state.get_autosave_bytes().unwrap()).unwrap();
        let mut doc = Document::new(Size::new(30, 12));
        doc.type_art_text("A", &fonts[0], 0).unwrap();
        assert_eq!(
            doc.with_state(|state| state.get_buffer().char_at((0, 0).into()).attribute.as_u8(icy_engine::IceMode::Blink) & 15),
            12
        );
        assert_eq!(doc.with_state(|state| state.get_buffer().char_at((1, 0).into()).ch), '#');
        assert!(doc.with_state(|state| state.get_caret().position().x) >= 2);
        let first_end = doc.with_state(|state| state.get_caret().position());
        doc.type_art_text("A", &fonts[0], 0).unwrap();
        doc.font_backspace().unwrap();
        assert_eq!(doc.with_state(|state| state.get_caret().position()), first_end);
        doc.font_backspace().unwrap();
        assert_eq!(doc.with_state(|state| state.get_caret().position()), Position::default());
        assert_ne!(doc.with_state(|state| state.get_buffer().char_at((0, 0).into()).ch), '#');
        doc.undo().unwrap();
        assert_eq!(doc.with_state(|state| state.get_buffer().char_at((0, 0).into()).ch), '#');
        doc.undo().unwrap();
        doc.undo().unwrap();
        doc.undo().unwrap();
        assert!(!doc.modified());
        assert_eq!(doc.with_state(|state| state.get_caret().position()), Position::new(0, 0));
        doc.with_state(|state| state.get_cur_layer_mut().unwrap().properties.is_locked = true);
        doc.type_art_text("A", &fonts[0], 0).unwrap();
        assert!(!doc.modified());
    }

    #[test]
    fn block_and_outline_text_art_use_caret_colors() {
        for font_type in [icy_engine_edit::charset::TdfFontType::Block, icy_engine_edit::charset::TdfFontType::Outline] {
            let mut tdf = retrofont::tdf::TdfFont::new("test", font_type, 0);
            let mut glyph = retrofont::Glyph::new(4, 2);
            glyph.parts = vec![
                retrofont::GlyphPart::Char('A'),
                retrofont::GlyphPart::Skip,
                retrofont::GlyphPart::NewLine,
                retrofont::GlyphPart::Char('B'),
            ];
            tdf.add_glyph('A', glyph);
            let font = retrofont::Font::Tdf(Box::new(tdf));
            let mut doc = Document::new(Size::new(30, 12));
            doc.with_state(|state| state.set_caret_attribute(icy_engine::TextAttribute::from_color(4, 1)));
            doc.type_art_text("A", &font, 0).unwrap();
            for y in 0..2 {
                for x in 0..4 {
                    let attribute = doc.with_state(|state| state.get_buffer().char_at((x, y).into()).attribute);
                    assert_eq!(
                        (attribute.foreground(), attribute.background()),
                        (4, 1),
                        "{font_type:?} cell ({x}, {y}) ignores the caret colors"
                    );
                }
            }
            assert_eq!(doc.with_state(|state| state.get_buffer().char_at((1, 0).into()).ch), ' ');
            assert_eq!(doc.with_state(|state| state.get_buffer().char_at((3, 1).into()).ch), ' ');
            assert_eq!(doc.with_state(|state| state.get_buffer().char_at((4, 0).into()).attribute.background()), 0);
        }
    }

    /// A color font with only an upper case 'A', 3 × 2 cells, and a spacing of 2.
    fn upper_case_font() -> retrofont::Font {
        let mut tdf = retrofont::tdf::TdfFont::new("upper", icy_engine_edit::charset::TdfFontType::Color, 2);
        let mut glyph = retrofont::Glyph::new(3, 2);
        glyph.parts = vec![
            retrofont::GlyphPart::Char('A'),
            retrofont::GlyphPart::Char('A'),
            retrofont::GlyphPart::Char('A'),
            retrofont::GlyphPart::NewLine,
            retrofont::GlyphPart::Char('a'),
            retrofont::GlyphPart::Char('a'),
            retrofont::GlyphPart::Char('a'),
        ];
        tdf.add_glyph('A', glyph);
        retrofont::Font::Tdf(Box::new(tdf))
    }

    #[test]
    fn text_art_types_spaces_and_the_other_case() {
        let font = upper_case_font();
        let mut doc = Document::new(Size::new(30, 12));
        doc.type_art_text("a A", &font, 0).unwrap();
        let caret = doc.with_state(|state| state.get_caret().position());
        assert_eq!(caret, Position::new(3 + 2 + 3, 0), "a lower case letter and the space advance the caret");
        for x in [0, 5] {
            assert_eq!(
                doc.with_state(|state| state.get_buffer().char_at((x, 0).into()).ch),
                'A',
                "a glyph at column {x}"
            );
        }
        doc.type_art_text("?", &font, 0).unwrap();
        assert_eq!(doc.with_state(|state| state.get_caret().position()), caret, "missing glyphs are still skipped");
    }

    #[test]
    fn text_art_enter_returns_to_the_start_column() {
        let font = upper_case_font();
        let mut doc = Document::new(Size::new(40, 12));
        doc.with_state(|state| state.set_caret_position(Position::new(10, 1)));
        doc.type_art_text("AA", &font, 0).unwrap();
        // Each key is typed on its own, like in the editor.
        doc.type_art_text("\n", &font, 0).unwrap();
        doc.type_art_text("A", &font, 0).unwrap();
        assert_eq!(
            doc.with_state(|state| state.get_buffer().char_at((10, 3).into()).ch),
            'A',
            "the second line starts below the first"
        );
        doc.font_backspace().unwrap();
        assert_eq!(doc.with_state(|state| state.get_caret().position()), Position::new(10, 3));
        doc.type_art_text("A\n", &font, 0).unwrap();
        assert_eq!(
            doc.with_state(|state| state.get_caret().position()),
            Position::new(10, 5),
            "backspace keeps the line start"
        );

        doc.with_state(|state| state.set_caret_position(Position::new(20, 8)));
        doc.type_art_text("A\n", &font, 0).unwrap();
        assert_eq!(
            doc.with_state(|state| state.get_caret().position()),
            Position::new(20, 10),
            "moving the caret starts a new text"
        );
    }

    #[test]
    fn native_roundtrip_retains_layers_and_saved_undo_position() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("layers.icy");
        let mut doc = Document::new(Size::new(20, 10));
        doc.with_state(|state| state.add_new_layer(0)).unwrap();
        doc.type_text("HELLO").unwrap();
        doc.save(&path, false).unwrap();
        assert!(!doc.modified());
        let loaded = Document::load(&path).unwrap();
        assert_eq!(loaded.with_state(|state| state.get_buffer().layers.len()), 2);
        assert_eq!(loaded.with_state(|state| state.get_buffer().char_at(Position::new(0, 0)).ch), 'H');
        doc.type_text("!").unwrap();
        assert!(doc.modified());
        doc.undo().unwrap();
        assert!(!doc.modified());
        std::fs::write(&path, b"external change").unwrap();
        assert!(doc.save(&path, false).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"external change");
    }

    #[test]
    fn tag_selection_drag_cancel_and_delete_are_grouped() {
        let mut document = Document::new(Size::new(30, 12));
        for column in [2, 12] {
            document
                .with_state(|state| {
                    state.add_new_tag(icy_engine::Tag {
                        is_enabled: true,
                        preview: "TAG".into(),
                        replacement_value: String::new(),
                        position: Position::new(column, 2),
                        length: 3,
                        alignment: std::fmt::Alignment::Left,
                        tag_placement: icy_engine::TagPlacement::InText,
                        tag_role: icy_engine::TagRole::Displaycode,
                        attribute: icy_engine::TextAttribute::default(),
                    })
                })
                .unwrap();
        }
        document.tool = Tool::Tag;
        document.begin(Position::new(2, 2), MouseButton::Left);
        document.finish();
        document.begin_with_modifiers(
            Position::new(12, 2),
            MouseButton::Left,
            KeyModifiers {
                ctrl: true,
                ..Default::default()
            },
        );
        assert_eq!(document.selected_tags, vec![0, 1]);
        let undo_count = document.with_state(|state| state.undo_stack_len());
        document.begin(Position::new(2, 2), MouseButton::Left);
        document.update(Position::new(5, 4));
        assert_eq!(document.tag_preview_position(1, Position::new(12, 2)), Position::new(15, 4));
        assert_eq!(document.with_state(|state| state.get_buffer().tags[0].position), Position::new(5, 4));
        assert_eq!(document.with_state(|state| state.get_buffer().tags[1].position), Position::new(15, 4));
        document.cancel();
        assert_eq!(document.with_state(|state| state.get_buffer().tags[0].position), Position::new(2, 2));
        document.begin(Position::new(2, 2), MouseButton::Left);
        document.update(Position::new(5, 4));
        document.finish();
        assert_eq!(document.with_state(|state| state.undo_stack_len()), undo_count + 1);
        assert_eq!(document.with_state(|state| state.get_buffer().tags[1].position), Position::new(15, 4));
        document.undo().unwrap();
        assert_eq!(document.with_state(|state| state.get_buffer().tags[1].position), Position::new(12, 2));
        document.delete_selected_tags().unwrap();
        assert!(document.with_state(|state| state.get_buffer().tags.is_empty()));
        document.undo().unwrap();
        assert_eq!(document.with_state(|state| state.get_buffer().tags.len()), 2);
    }

    #[test]
    fn selected_tag_uses_and_tracks_caret_colors() {
        let mut document = Document::new(Size::new(30, 12));
        document
            .with_state(|state| {
                let mut attribute = icy_engine::TextAttribute::default();
                attribute.set_foreground(2);
                attribute.set_background(1);
                state.add_new_tag(icy_engine::Tag {
                    is_enabled: true,
                    preview: "TAG".into(),
                    replacement_value: String::new(),
                    position: Position::new(2, 2),
                    length: 3,
                    alignment: std::fmt::Alignment::Left,
                    tag_placement: icy_engine::TagPlacement::InText,
                    tag_role: icy_engine::TagRole::Displaycode,
                    attribute,
                })
            })
            .unwrap();
        document.tool = Tool::Tag;
        document.begin(Position::new(2, 2), MouseButton::Left);
        document.finish();

        assert_eq!(document.with_state(|state| state.get_caret().attribute.foreground()), 2);
        assert_eq!(document.with_state(|state| state.get_caret().attribute.background()), 1);

        document.set_caret_foreground(14).unwrap();
        assert_eq!(document.with_state(|state| state.get_buffer().tags[0].attribute.background()), 1);
        document.set_caret_background(4).unwrap();
        let attribute = document.with_state(|state| state.get_buffer().tags[0].attribute);
        assert_eq!(attribute.foreground(), 14);
        assert_eq!(attribute.background(), 4);
        assert_eq!(
            document.with_state(|state| state.get_buffer().char_at(Position::new(2, 2)).attribute),
            attribute
        );
    }

    #[test]
    fn paste_transparency_toggles_redraws_and_survives_rotation() {
        let mut document = Document::new(Size::new(30, 12));
        document.start_paste("A B", None).unwrap();
        let chars = |document: &Document| document.with_state(|state| super::layer_chars(state));
        let visible = |document: &Document| chars(document).iter().flatten().filter(|ch| ch.is_visible()).count();
        let opaque = chars(&document);
        assert!(opaque[0][1].is_visible(), "the pasted space starts out opaque");
        let all = visible(&document);

        document.with_state(|state| state.get_buffer().clear_dirty());
        document.paste_action(PasteAction::Transparent).unwrap();
        assert!(
            document.with_state(|state| state.get_buffer().is_dirty()),
            "turning transparency on must redraw"
        );
        assert!(document.paste_transparent());
        assert!(!chars(&document)[0][1].is_visible());
        let see_through = visible(&document);
        assert!(see_through < all);

        document.with_state(|state| state.get_buffer().clear_dirty());
        document.paste_action(PasteAction::Transparent).unwrap();
        assert!(document.with_state(|state| state.get_buffer().is_dirty()), "turning it off must redraw");
        assert!(!document.paste_transparent());
        assert_eq!(chars(&document), opaque, "a second click restores the paste");

        // Rotating a transparent paste keeps it transparent, and turning it off afterwards
        // restores the rotated opaque paste.
        document.paste_action(PasteAction::Transparent).unwrap();
        let before_rotation = chars(&document);
        document.paste_action(PasteAction::Rotate).unwrap();
        assert!(document.paste_transparent());
        assert_eq!(visible(&document), see_through);
        document.paste_action(PasteAction::Transparent).unwrap();
        assert_eq!(visible(&document), all);
        document.with_state(|state| state.undo()).unwrap();
        document.with_state(|state| state.undo()).unwrap();
        assert_eq!(chars(&document), before_rotation, "rotating a transparent paste is one undo step");
        assert!(document.paste_transparent());

        // Undo keeps the toggle in sync with what is shown.
        document.paste_action(PasteAction::Transparent).unwrap();
        assert!(!document.paste_transparent());
        document.with_state(|state| state.undo()).unwrap();
        assert!(document.paste_transparent(), "undoing \"off\" shows the transparent paste again");
        document.paste_action(PasteAction::Transparent).unwrap();
        assert_eq!(chars(&document), opaque);
        document.with_state(|state| state.undo()).unwrap();
        document.with_state(|state| state.undo()).unwrap();
        assert_eq!(chars(&document), opaque);
        assert!(!document.paste_transparent(), "undoing \"on\" turns the toggle off");
    }

    #[test]
    fn floating_paste_drag_transform_cancel_and_anchor() {
        let mut document = Document::new(Size::new(30, 12));
        document.tool = Tool::Pencil;
        document.start_paste("AB", None).unwrap();
        assert!(document.paste_active());
        assert!(document.modified());
        assert!(!document.can_paint());
        document.begin(Position::default(), MouseButton::Left);
        document.update(Position::new(3, 2));
        document.paste_action(PasteAction::Stamp).unwrap();
        assert_eq!(document.with_state(|state| state.get_buffer().layers[0].char_at(Position::new(3, 2)).ch), 'A');
        document.paste_action(PasteAction::Rotate).unwrap();
        document.paste_action(PasteAction::FlipX).unwrap();
        document.paste_action(PasteAction::Cancel).unwrap();
        assert!(!document.modified());
        assert_eq!(document.tool, Tool::Pencil);
        assert_eq!(document.with_state(|state| state.get_buffer().layers.len()), 1);
        assert_ne!(document.with_state(|state| state.get_buffer().char_at(Position::new(3, 2)).ch), 'A');
        document.start_paste("AB", None).unwrap();
        document.paste_action(PasteAction::Move(Position::new(3, 2))).unwrap();
        document.paste_action(PasteAction::Anchor).unwrap();
        assert_eq!(document.tool, Tool::Pencil);
        assert_eq!(document.with_state(|state| state.get_buffer().layers.len()), 1);
        assert_eq!(document.with_state(|state| state.get_buffer().char_at(Position::new(3, 2)).ch), 'A');
        assert_eq!(document.with_state(|state| state.undo_stack_len()), 1);
        document.undo().unwrap();
        assert!(!document.modified());
        document.redo().unwrap();
        assert_eq!(document.with_state(|state| state.get_buffer().char_at(Position::new(3, 2)).ch), 'A');
    }

    fn tag(x: i32, y: i32, preview: &str) -> icy_engine::Tag {
        icy_engine::Tag {
            is_enabled: true,
            preview: preview.into(),
            replacement_value: String::new(),
            position: Position::new(x, y),
            length: preview.len(),
            alignment: std::fmt::Alignment::Left,
            tag_placement: icy_engine::TagPlacement::InText,
            tag_role: icy_engine::TagRole::Displaycode,
            attribute: icy_engine::TextAttribute::default(),
        }
    }

    #[test]
    fn dragging_along_a_row_asks_for_a_tag_and_other_drags_select() {
        let mut document = Document::new(Size::new(30, 12));
        document.with_state(|state| state.add_new_tag(tag(20, 6, "OLD"))).unwrap();
        document.tool = Tool::Tag;
        document.begin(Position::new(8, 3), MouseButton::Left);
        document.update(Position::new(3, 3));
        document.finish();
        assert_eq!(document.new_tag_request.take(), Some((Position::new(3, 3), 6)), "dragged from either side");

        document.begin(Position::new(4, 4), MouseButton::Left);
        document.finish();
        assert_eq!(document.new_tag_request, None, "a click only places the caret");

        document.begin(Position::new(18, 5), MouseButton::Left);
        document.update(Position::new(24, 7));
        document.finish();
        assert_eq!(document.new_tag_request, None);
        assert_eq!(document.selected_tags, vec![0], "a drag over rows selects");

        document.begin_with_shift(Position::new(18, 6), MouseButton::Left, true);
        document.update(Position::new(24, 6));
        document.finish();
        assert_eq!(document.new_tag_request, None, "Shift+drag selects along a row too");
        assert_eq!(document.selected_tags, vec![0]);
    }

    #[test]
    fn moved_and_copied_blocks_take_their_tags_along() {
        let mut document = Document::new(Size::new(30, 12));
        document.type_text("ABCD").unwrap();
        document
            .with_state(|state| {
                state.add_new_tag(tag(1, 0, "TG"))?;
                state.add_new_tag(tag(3, 0, "OUT"))
            })
            .unwrap();
        let block = |document: &mut Document| {
            let mut selection = Selection::new(Position::new(0, 0));
            selection.lead = Position::new(2, 0);
            document.with_state(|state| state.set_selection(selection)).unwrap();
        };
        block(&mut document);
        document.float_selection(false).unwrap();
        document.paste_action(PasteAction::Move(Position::new(0, 2))).unwrap();
        document.paste_action(PasteAction::Anchor).unwrap();
        let positions = |document: &Document| document.with_state(|state| state.get_buffer().tags.iter().map(|tag| tag.position).collect::<Vec<_>>());
        assert_eq!(
            positions(&document),
            [Position::new(1, 0), Position::new(3, 0), Position::new(1, 2)],
            "a copied block copies the tag inside it, not the one sticking out"
        );

        block(&mut document);
        document.float_selection(true).unwrap();
        document.paste_action(PasteAction::Move(Position::new(5, 4))).unwrap();
        document.paste_action(PasteAction::Anchor).unwrap();
        assert_eq!(positions(&document)[0], Position::new(6, 4), "a moved block moves it");
        document.undo().unwrap();
        assert_eq!(positions(&document)[0], Position::new(1, 0), "one undo puts block and tag back");
    }

    #[test]
    fn tags_move_cycle_copy_and_duplicate_from_the_keyboard() {
        let mut document = Document::new(Size::new(20, 10));
        document.tool = Tool::Tag;
        document
            .with_state(|state| {
                state.add_new_tag(tag(5, 3, "B"))?;
                state.add_new_tag(tag(2, 1, "AAA"))?;
                state.add_new_tag(tag(9, 3, "C"))
            })
            .unwrap();
        let positions = |document: &Document| document.with_state(|state| state.get_buffer().tags.iter().map(|tag| tag.position).collect::<Vec<_>>());

        assert!(document.select_next_tag(false));
        assert_eq!(document.selected_tags, vec![1], "Tab starts at the first tag in reading order");
        assert!(document.select_next_tag(false));
        assert_eq!(document.selected_tags, vec![0]);
        assert!(document.select_next_tag(true));
        assert!(document.select_next_tag(true));
        assert_eq!(document.selected_tags, vec![2], "Shift+Tab wraps around");
        assert_eq!(document.with_state(|state| state.get_caret().position()), Position::new(9, 3));

        document.selected_tags = vec![1, 2];
        assert!(document.nudge_selected_tags(Position::new(1, 1)).unwrap());
        assert_eq!(positions(&document), [Position::new(5, 3), Position::new(3, 2), Position::new(10, 4)]);
        document.nudge_selected_tags(Position::new(-10, 0)).unwrap();
        assert_eq!(positions(&document)[1], Position::new(0, 2), "tags stay on the canvas and keep their layout");
        assert_eq!(positions(&document)[2], Position::new(7, 4));
        document.undo().unwrap();
        assert_eq!(positions(&document)[1], Position::new(3, 2), "a nudge is one undo step");

        let text = document.copy_selected_tags().unwrap();
        assert_eq!(text, "AAA C");
        document.with_state(|state| state.set_caret_from_document_position(Position::new(15, 8)));
        assert!(!document.paste_tags("something else").unwrap(), "other clipboard content pastes as usual");
        assert!(document.paste_tags(&text).unwrap());
        assert_eq!(
            positions(&document)[3..],
            [Position::new(15, 8), Position::new(19, 9)],
            "pasted at the caret, kept on the canvas"
        );
        assert_eq!(document.selected_tags, vec![3, 4]);

        document.selected_tags = vec![0];
        assert!(document.duplicate_selected_tags().unwrap());
        assert_eq!(positions(&document)[5], Position::new(5, 4));
        assert_eq!(document.selected_tags, vec![5], "the copy is selected");
        document.undo().unwrap();
        assert_eq!(positions(&document).len(), 5, "duplicating is one undo step");
    }
}
