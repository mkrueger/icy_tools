use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use icy_engine::{FileFormat, MouseButton, Position, Screen, Selection, Size, TextBuffer};
use icy_engine_edit::{tools::Tool, AtomicUndoGuard, EditState, UndoState};
use parking_lot::Mutex;

use crate::{
    brush::BrushPrimaryMode,
    paint::{apply_stamp_at_doc_pos, BrushSettings},
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
    undo: AtomicUndoGuard,
}

pub struct Document {
    pub screen: Arc<Mutex<Box<dyn Screen>>>,
    pub path: Option<PathBuf>,
    pub brush: BrushSettings,
    pub tool: Tool,
    pub preview: Vec<Position>,
    pub metadata_dirty: bool,
    pub baseline: Option<Vec<u8>>,
    stroke: Option<Stroke>,
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
            preview: Vec::new(),
            metadata_dirty: false,
            stroke: None,
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
        self.metadata_dirty || self.with_state(|state| state.get_undo_stack().lock().unwrap().is_modified())
    }

    pub fn bytes(&self, format: FileFormat, mut options: icy_engine::SaveOptions) -> DrawResult<Vec<u8>> {
        self.with_state(|state| {
            options.sauce = Some(state.get_sauce_meta().clone());
            format.to_bytes(state.get_buffer(), &options).map_err(|error| error.to_string())
        })
    }

    pub fn save(&mut self, path: &Path, overwrite: bool) -> DrawResult<()> {
        self.finish();
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

    pub fn begin(&mut self, position: Position, button: MouseButton) {
        self.begin_with_shift(position, button, false);
    }

    pub fn begin_with_shift(&mut self, position: Position, button: MouseButton, shift: bool) {
        self.finish();
        if self.tool != Tool::Click && self.tool != Tool::Select && !self.can_paint() {
            return;
        }
        if self.tool == Tool::Fill {
            let half = self.brush.primary == BrushPrimaryMode::HalfBlock;
            let cell = if half { Position::new(position.x, position.y / 2) } else { position };
            self.with_state(|state| crate::fill::fill(state, self.brush, cell, !half || position.y % 2 == 0, button, shift));
            return;
        }
        let undo = self.with_state(|state| state.begin_atomic_undo(self.tool.name()));
        self.stroke = Some(Stroke {
            start: position,
            last: position,
            tool: self.tool,
            brush: self.brush,
            button,
            clear: shift && self.tool.is_shape_tool(),
            undo,
        });
        if self.tool == Tool::Pencil {
            self.stamp(position, self.brush, button);
        } else if self.tool == Tool::Click || self.tool == Tool::Font {
            self.with_state(|state| state.set_caret_from_document_position(position));
        } else if self.tool == Tool::Select {
            self.with_state(|state| state.set_selection(Selection::new(position))).ok();
        }
        self.update(position);
    }

    pub fn update(&mut self, position: Position) {
        let Some(stroke) = &self.stroke else {
            return;
        };
        let (start, last, tool, brush, button) = (stroke.start, stroke.last, stroke.tool, stroke.brush, stroke.button);
        if tool == Tool::Pencil && last != position {
            for point in icy_engine_edit::brushes::get_line_points(last, position).into_iter().skip(1) {
                self.stamp(point, brush, button);
            }
        } else if tool.is_shape_tool() {
            self.preview = shape_points(tool, start, position);
        } else if tool == Tool::Select {
            let mut selection = Selection::new(start);
            selection.lead = position;
            selection.shape = icy_engine::Shape::Rectangle;
            self.with_state(|state| state.set_selection(selection)).ok();
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
            self.with_state(|state| stroke.undo.discard_and_undo(state));
        }
        self.preview.clear();
    }

    pub fn undo(&mut self) -> DrawResult<()> {
        self.finish();
        self.with_state(|state| state.undo()).map_err(|error| error.to_string())
    }

    pub fn redo(&mut self) -> DrawResult<()> {
        self.finish();
        self.with_state(|state| state.redo()).map_err(|error| error.to_string())
    }

    pub fn type_text(&mut self, text: &str) -> DrawResult<()> {
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
                        let encoded = state.get_buffer().buffer_type.convert_from_unicode(character);
                        state.type_key(encoded)?;
                    }
                }
            }
            Ok::<(), icy_engine::EngineError>(())
        })
        .map_err(|error| error.to_string())
    }

    pub fn can_paint(&self) -> bool {
        self.with_state(|state| state.get_cur_layer().is_some_and(|layer| !layer.properties.is_locked && layer.is_visible()))
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use icy_engine::TextPane;

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
}
