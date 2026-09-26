use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use icy_engine::{AddType, FileFormat, KeyModifiers, MouseButton, Position, Rectangle, Screen, Selection, Size, TextBuffer, TextPane};
use icy_engine_edit::{tools::Tool, AtomicUndoGuard, EditState, UndoState};
use parking_lot::Mutex;

use crate::{
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
    Anchor,
    Keep,
    Cancel,
}

struct PasteState {
    previous_tool: Tool,
    undo: AtomicUndoGuard,
}

pub struct Document {
    pub screen: Arc<Mutex<Box<dyn Screen>>>,
    pub path: Option<PathBuf>,
    pub brush: BrushSettings,
    pub tool: Tool,
    pub selection_mode: SelectionMode,
    pub outline_font: bool,
    pub selected_tags: Vec<usize>,
    pub preview: Vec<Position>,
    pub metadata_dirty: bool,
    pub baseline: Option<Vec<u8>>,
    stroke: Option<Stroke>,
    paste: Option<PasteState>,
}

impl Document {
    pub fn new(size: Size) -> Self {
        let mut buffer = TextBuffer::new(size);
        buffer.terminal_state.is_terminal_buffer = false;
        Self::from_state(EditState::from_buffer(buffer))
    }

    pub fn from_state(state: EditState) -> Self {
        Self {
            screen: Arc::new(Mutex::new(Box::new(state))),
            path: None,
            baseline: None,
            brush: BrushSettings::default(),
            tool: Tool::Click,
            selection_mode: SelectionMode::default(),
            outline_font: false,
            selected_tags: Vec::new(),
            preview: Vec::new(),
            metadata_dirty: false,
            stroke: None,
            paste: None,
        }
    }

    pub fn load(path: &Path) -> DrawResult<Self> {
        let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
        let format = FileFormat::from_path(path).ok_or_else(|| format!("Unknown file format: {}", path.display()))?;
        let loaded = format.from_bytes(&bytes, None).map_err(|error| error.to_string())?;
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

    fn start_floating_paste(&mut self, paste: impl FnOnce(&mut EditState) -> icy_engine::Result<()>) -> DrawResult<()> {
        self.finish();
        if self.paste_active() || !self.can_paint() {
            return Ok(());
        }
        let mut undo = self.with_state(|state| state.begin_atomic_undo("Paste"));
        let result = self.with_state(|state| {
            let offset = state.get_cur_layer().map(|layer| layer.offset()).unwrap_or_default();
            let count = state.get_buffer().layers.len();
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
        self.with_state(|state| match action {
            PasteAction::Move(delta) => state.move_layer(state.get_cur_layer().unwrap().offset() + delta),
            PasteAction::Stamp => state.stamp_layer_down(),
            PasteAction::Rotate => state.paste_rotate(),
            PasteAction::FlipX => state.paste_flip_x(),
            PasteAction::FlipY => state.paste_flip_y(),
            PasteAction::Transparent => state.make_layer_transparent(),
            PasteAction::Anchor => state.paste_anchor(),
            PasteAction::Keep => state.add_floating_layer(),
            PasteAction::Cancel => Ok(()),
        })
        .map_err(|error| error.to_string())?;
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

    fn stamp(&self, position: Position, brush: BrushSettings, button: MouseButton) {
        if !self.can_paint() {
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
                for point in std::mem::take(&mut self.preview) {
                    if stroke.clear {
                        if !self.can_paint() {
                            continue;
                        }
                        self.with_state(|state| {
                            use icy_engine::TextPane;
                            let point = if stroke.brush.primary == BrushPrimaryMode::HalfBlock {
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
    }

    pub fn cancel(&mut self) {
        if let Some(mut stroke) = self.stroke.take() {
            self.with_state(|state| {
                state.set_layer_preview_offset(None);
                stroke.undo.discard_and_undo(state);
            });
        }
        self.preview.clear();
    }

    pub fn undo(&mut self) -> DrawResult<()> {
        if self.paste_active() {
            return self.paste_action(PasteAction::Cancel);
        }
        self.finish();
        self.with_state(|state| state.undo()).map_err(|error| error.to_string())
    }

    pub fn redo(&mut self) -> DrawResult<()> {
        if self.paste_active() {
            return Ok(());
        }
        self.finish();
        self.with_state(|state| state.redo()).map_err(|error| error.to_string())
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
                            let upper = character.to_ascii_uppercase();
                            if !matches!(upper, 'A'..='Q' | '@' | '&' | ' ' | '\u{00ff}') {
                                continue;
                            }
                            upper
                        } else {
                            state.get_buffer().buffer_type.convert_from_unicode(character)
                        };
                        state.type_key(encoded)?;
                    }
                }
            }
            Ok::<(), icy_engine::EngineError>(())
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
        self.with_state(|state| {
            let _undo = state.begin_typed_atomic_undo("Render font character", icy_engine_edit::OperationType::RenderCharacter);
            state.undo_caret_position()?;
            for character in text.chars() {
                let start = state.get_caret().position();
                if character == '\n' {
                    state.set_caret_position(Position::new(0, start.y + font.max_height() as i32));
                } else if font.has_char(character) {
                    let mut renderer = icy_engine_edit::TdfEditStateRenderer::new(state, start.x, start.y)?;
                    font.render_glyph(
                        &mut renderer,
                        character,
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
            Ok::<(), icy_engine::EngineError>(())
        })
        .map_err(|error| error.to_string())
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
        .map_err(|error| error.to_string())
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use icy_engine::TextPane;

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
}
