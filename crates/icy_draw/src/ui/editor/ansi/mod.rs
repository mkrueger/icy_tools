use std::{
    cmp::{max, min},
    ffi::OsStr,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
};

use eframe::{
    egui::{self, Id, Key, Response},
    epaint::{mutex::Mutex, Vec2},
};
use i18n_embed_fl::fl;
use icy_engine::{
    attribute,
    editor::{AtomicUndoGuard, UndoState},
    util::{pop_data, pop_sixel_image, push_data, BUFFER_DATA},
    AttributedChar, Buffer, EngineResult, Line, Position, Rectangle, SaveOptions, TextAttribute, TextPane,
};

use icy_engine_gui::{show_terminal_area, BufferView, CaretShape, TerminalCalc};

use crate::{
    model::{DragPos, MKey, MModifiers, Tool},
    paint::ColorMode,
    ClipboardHandler, Commands, Document, DocumentOptions, Message, SavingError, TerminalResult, UndoHandler, SETTINGS,
};

pub enum Event {
    None,
    CursorPositionChange(Position, Position),
}

pub struct AnsiEditor {
    pub id: usize,
    pub drag_pos: DragPos,
    pub half_block_click_pos: Position,
    drag_started: bool,
    pub buffer_view: Arc<eframe::epaint::mutex::Mutex<BufferView>>,
    pub is_inactive: bool,

    pub outline_font_mode: bool,
    last_selected_tool: usize,

    pub reference_image: Option<PathBuf>,
    // pub outline_changed: std::boxed::Box<dyn Fn(&Editor)>,
    //pub request_refresh: Box<dyn Fn ()>,
    pub egui_id: Id,
    pub next_scroll_x_position: Option<f32>,
    pub next_scroll_y_position: Option<f32>,
    pub guide: Option<Vec2>,
    pub raster: Option<Vec2>, //pub pos_changed: std::boxed::Box<dyn Fn(&Editor, Position)>,
    //pub attr_changed: std::boxed::Box<dyn Fn(TextAttribute)>
    pub request_focus: bool,
    pub color_mode: ColorMode,
    pub tool_switch: bool,
}

impl UndoHandler for AnsiEditor {
    fn undo_description(&self) -> Option<String> {
        self.buffer_view.lock().get_edit_state().undo_description()
    }

    fn can_undo(&self) -> bool {
        self.buffer_view.lock().get_edit_state().can_undo()
    }

    fn undo(&mut self) -> EngineResult<Option<Message>> {
        self.buffer_view.lock().get_edit_state_mut().undo()?;
        Ok(None)
    }

    fn redo_description(&self) -> Option<String> {
        self.buffer_view.lock().get_edit_state().redo_description()
    }

    fn can_redo(&self) -> bool {
        self.buffer_view.lock().get_edit_state().can_redo()
    }

    fn redo(&mut self) -> EngineResult<Option<Message>> {
        self.buffer_view.lock().get_edit_state_mut().redo()?;
        Ok(None)
    }
}

impl ClipboardHandler for AnsiEditor {
    fn can_cut(&self) -> bool {
        self.buffer_view.lock().get_selection().is_some()
    }
    fn cut(&mut self) -> EngineResult<()> {
        let _cut = self.begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-cut"));
        self.copy()?;
        self.buffer_view.lock().get_edit_state_mut().erase_selection()?;
        Ok(())
    }

    fn can_copy(&self) -> bool {
        self.buffer_view.lock().get_selection().is_some()
    }

    fn copy(&mut self) -> EngineResult<()> {
        if let Some(data) = self.buffer_view.lock().get_edit_state_mut().get_clipboard_data() {
            push_data(BUFFER_DATA, &data)?;
        } else {
            log::error!("can't get clipboard data!");
        }
        Ok(())
    }

    fn can_paste(&self) -> bool {
        pop_data(BUFFER_DATA).is_some() || pop_sixel_image().is_some()
    }

    fn paste(&mut self) -> EngineResult<()> {
        if self.buffer_view.lock().get_edit_state_mut().has_floating_layer() {
            return Ok(());
        }

        if let Some(data) = pop_data(BUFFER_DATA) {
            self.buffer_view.lock().get_edit_state_mut().paste_clipboard_data(&data)?;
        } else if let Some(sixel) = pop_sixel_image() {
            self.buffer_view.lock().get_edit_state_mut().paste_sixel(sixel)?;
        }
        Ok(())
    }
}
const ICED_EXT: &str = "icy";

impl Document for AnsiEditor {
    fn default_extension(&self) -> &'static str {
        ICED_EXT
    }
    fn has_tool_switched(&self) -> bool {
        self.tool_switch
    }
    fn set_tool_switch(&mut self, switched: bool) {
        self.tool_switch = switched;
    }

    fn undo_stack_len(&self) -> usize {
        if let Ok(stack) = self.buffer_view.lock().get_edit_state().get_undo_stack().lock() {
            for i in (0..stack.len()).rev() {
                if stack[i].changes_data() {
                    return i + 1;
                }
            }
        }
        0
    }

    fn get_bytes(&mut self, path: &Path) -> TerminalResult<Vec<u8>> {
        let ext = if let Some(ext) = path.extension() {
            OsStr::to_str(ext).unwrap_or(ICED_EXT).to_lowercase()
        } else {
            ICED_EXT.to_string()
        };
        let mut options = SaveOptions::new();
        options.compress = false;
        options.lossles_output = true;
        let bytes = self.buffer_view.lock().get_buffer().to_bytes(&ext, &options)?;
        Ok(bytes)
    }

    fn show_ui(&mut self, ui: &mut egui::Ui, cur_tool: &mut Box<dyn Tool>, selected_tool: usize, options: &DocumentOptions) -> Option<Message> {
        let mut message = None;

        // clear tool overlays on tool change
        if self.last_selected_tool != selected_tool {
            self.last_selected_tool = selected_tool;
            self.buffer_view.lock().get_edit_state_mut().get_tool_overlay_mask_mut().clear();
            self.buffer_view.lock().get_edit_state_mut().set_is_buffer_dirty();
        }

        let mut scale = unsafe { SETTINGS.get_scale() };
        let is_visible = cur_tool.use_caret(self);
        self.buffer_view.lock().get_caret_mut().set_is_visible(is_visible);
        if self.buffer_view.lock().get_buffer().use_aspect_ratio() {
            if self.buffer_view.lock().get_buffer().use_letter_spacing() {
                scale.y *= 1.2;
            } else {
                scale.y *= 1.35;
            }
        }
        let opt = icy_engine_gui::TerminalOptions {
            stick_to_bottom: false,
            scale: Some(scale),
            fit_width: options.fit_width,
            monitor_settings: unsafe { SETTINGS.monitor_settings.clone() },
            marker_settings: unsafe { SETTINGS.marker_settings.clone() },
            id: Some(Id::new(self.id + 10000)),
            raster: self.raster,
            guide: self.guide,
            scroll_offset_y: self.next_scroll_y_position.take(),
            scroll_offset_x: self.next_scroll_x_position.take(),
            show_layer_borders: unsafe { SETTINGS.show_layer_borders },
            show_line_numbers: unsafe { SETTINGS.show_line_numbers },
            request_focus: self.request_focus,
            caret_shape: CaretShape::Block,
            ..Default::default()
        };
        let (mut response, calc) = show_terminal_area(ui, self.buffer_view.clone(), opt);

        if calc.has_focus {
            self.request_focus = false;
        }
        let response_opt = response.context_menu(|ui| {
            message = terminal_context_menu(self, &options.commands, ui);
        });
        if let Some(response_opt) = response_opt {
            response = response_opt.response;
        }
        self.handle_response(ui, response, calc, cur_tool, &mut message);

        message
    }

    fn get_ansi_editor_mut(&mut self) -> Option<&mut AnsiEditor> {
        Some(self)
    }

    fn get_ansi_editor(&self) -> Option<&AnsiEditor> {
        Some(self)
    }

    fn destroy(&self, gl: &glow::Context) -> Option<Message> {
        self.buffer_view.lock().destroy(gl);
        None
    }
}

impl AnsiEditor {
    pub fn new(gl: &Arc<glow::Context>, id: usize, buf: Buffer) -> Self {
        let buffer_view = Arc::new(Mutex::new(BufferView::from_buffer(gl, buf)));
        // let buffer_parser = ansi::Parser::default();
        AnsiEditor {
            id,
            buffer_view,
            is_inactive: false,
            reference_image: None,
            drag_started: false,
            drag_pos: DragPos::default(),
            egui_id: Id::new(id),
            guide: None,
            raster: None,
            outline_font_mode: false,
            next_scroll_x_position: Some(0.0),
            next_scroll_y_position: Some(0.0),
            half_block_click_pos: Position::default(),
            last_selected_tool: 0,
            request_focus: false,
            color_mode: ColorMode::Both,
            tool_switch: true,
        }
    }

    pub fn get_cur_layer_index(&self) -> TerminalResult<usize> {
        self.buffer_view.lock().get_edit_state_mut().get_current_layer()
    }

    pub fn set_cur_layer_index(&self, layer: usize) {
        self.buffer_view.lock().get_edit_state_mut().set_current_layer(layer);
    }

    pub fn output_string(&mut self, str: &str) {
        for ch in str.chars() {
            self.type_key(ch);
        }
    }

    pub fn get_caret_position(&self) -> Position {
        self.buffer_view.lock().get_caret().get_position()
    }

    pub fn set_caret_position(&mut self, pos: Position) {
        let buffer_view = &mut self.buffer_view.lock();
        let pos = Position::new(
            min(buffer_view.get_width() - 1, max(0, pos.x)),
            min(buffer_view.get_buffer().get_height() - 1, max(0, pos.y)),
        );
        buffer_view.get_caret_mut().set_position(pos);
        buffer_view.reset_caret_blink();
        //(self.pos_changed)(self, pos);
    }

    pub fn set_caret_attribute(&mut self, attr: TextAttribute) {
        if attr == self.buffer_view.lock().get_caret().get_attribute() {
            return;
        }

        self.buffer_view.lock().get_caret_mut().set_attr(attr);
        // (self.attr_changed)(attr);
    }

    pub fn join_overlay(&mut self, description: impl Into<String>) {
        let _undo = self.begin_atomic_undo(description.into());
        let opt_layer = self.buffer_view.lock().get_buffer_mut().remove_overlay();
        let use_selection = self.buffer_view.lock().get_edit_state().is_something_selected();

        if let Some(layer) = &opt_layer {
            for y in 0..layer.lines.len() {
                let line = &layer.lines[y];
                for x in 0..line.chars.len() {
                    let ch = line.chars[x];
                    let pos = Position::new(x as i32, y as i32);
                    if ch.is_visible() && (!use_selection || self.buffer_view.lock().get_edit_state().get_is_selected(pos + layer.get_offset())) {
                        self.set_char(pos, ch);
                    }
                }
            }
        }
    }

    pub fn delete_line(&mut self, line: i32) {
        // TODO: Undo
        let mut lock = self.buffer_view.lock();
        if let Ok(cur_layer) = self.get_cur_layer_index() {
            let layer = &mut lock.get_buffer_mut().layers[cur_layer];
            layer.remove_line(line);
        } else {
            log::error!("can't get current layer!");
        }
    }

    pub fn insert_line(&mut self, line: i32) {
        // TODO: Undo
        let mut binding = self.buffer_view.lock();
        if let Ok(cur_layer) = self.get_cur_layer_index() {
            let layer = &mut binding.get_buffer_mut().layers[cur_layer];
            layer.insert_line(line, Line::new());
        } else {
            log::error!("can't get current layer!");
        }
    }

    pub fn pickup_color(&mut self, pos: Position) {
        let ch = self.buffer_view.lock().get_buffer().get_char(pos);
        if ch.is_visible() {
            self.buffer_view.lock().get_caret_mut().set_attr(ch.attribute);
        }
    }

    pub fn set_caret(&mut self, x: i32, y: i32) -> Event {
        let old = self.buffer_view.lock().get_caret().get_position();
        let mut w = self.buffer_view.lock().get_buffer().get_width() - 1;
        let mut h = self.buffer_view.lock().get_buffer().get_height() - 1;
        let offset: Position = if let Some(layer) = self.buffer_view.lock().get_edit_state().get_cur_layer() {
            w = layer.get_width() - 1;
            h = layer.get_height() - 1;
            layer.get_offset()
        } else {
            Position::default()
        };

        let char_scroll_position = self.buffer_view.lock().calc.char_scroll_position;
        let terminal_rect = self.buffer_view.lock().calc.terminal_rect;
        let terminal_width = terminal_rect.width();
        let terminal_height = terminal_rect.height();
        let rect = self.buffer_view.lock().calc.buffer_rect;
        let buffer_width = rect.width().min(terminal_width);
        let buffer_height = rect.height().min(terminal_height);
        let buffer_char_width = buffer_width / self.buffer_view.lock().calc.scale.x;
        let buffer_char_height = buffer_height / self.buffer_view.lock().calc.scale.y;

        let y = min(max(0, y), h);
        self.set_caret_position(Position::new(min(max(0, x), w), y));

        let font_dim = self.buffer_view.lock().get_buffer().get_font_dimensions();

        let pos = self.buffer_view.lock().get_caret().get_position();
        let x = (offset.x + pos.x) as f32 * font_dim.width as f32;
        let y = (offset.y + pos.y) as f32 * font_dim.height as f32;
        let mut next_y = None;
        let mut next_x = None;
        if y < char_scroll_position.y {
            next_y = Some(y);
        }
        if x < char_scroll_position.x {
            next_x = Some(x);
        }
        if y > (char_scroll_position.y + buffer_char_height - font_dim.height as f32) {
            next_y = Some((y - buffer_char_height + font_dim.height as f32).max(0.0));
        }
        if x > (char_scroll_position.x + buffer_char_width - font_dim.width as f32) {
            next_x = Some((x - buffer_char_width + font_dim.width as f32).max(0.0));
        }
        self.next_scroll_x_position = next_x;
        self.next_scroll_y_position = next_y;
        Event::CursorPositionChange(old, self.buffer_view.lock().get_caret().get_position())
    }

    pub fn save_content(&self, file_name: &Path, options: &SaveOptions) -> EngineResult<bool> {
        match File::create(file_name) {
            Ok(mut f) => {
                let content = if let Some(ext) = file_name.extension() {
                    let ext = OsStr::to_string_lossy(ext).to_lowercase();
                    self.buffer_view.lock().get_buffer().to_bytes(ext.as_str(), options)?
                } else {
                    self.buffer_view.lock().get_buffer().to_bytes(ICED_EXT, options)?
                };
                if let Err(err) = f.write_all(&content) {
                    return Err(SavingError::ErrorWritingFile(format!("{err}")).into());
                }
            }
            Err(err) => {
                return Err(SavingError::ErrorCreatingFile(format!("{err}")).into());
            }
        }

        Ok(true)
    }

    pub fn get_char_set_key(&self, i: usize) -> char {
        unsafe {
            let lock = self.buffer_view.lock();
            let font_page = lock.get_caret().get_font_page();

            let checksum = if let Some(font) = lock.get_buffer().get_font(font_page) {
                font.get_checksum()
            } else {
                0
            };

            SETTINGS.get_character_set_char(checksum, i)
        }
    }

    pub fn type_char_set_key(&mut self, character_set: usize) {
        self.buffer_view.lock().clear_selection();
        let ch = self.get_char_set_key(character_set);
        self.type_key(unsafe { char::from_u32_unchecked(ch as u32) });
    }

    pub fn get_char(&self, pos: Position) -> AttributedChar {
        self.buffer_view.lock().get_buffer().get_char(pos)
    }

    pub fn get_char_from_cur_layer(&self, pos: Position) -> AttributedChar {
        if let Ok(cur_layer) = self.get_cur_layer_index() {
            if cur_layer >= self.buffer_view.lock().get_buffer().layers.len() {
                return AttributedChar::invisible();
            }
            self.buffer_view.lock().get_buffer().layers[cur_layer].get_char(pos)
        } else {
            log::error!("can't get current layer!");
            AttributedChar::invisible()
        }
    }

    pub fn set_char(&mut self, pos: impl Into<Position>, attributed_char: AttributedChar) {
        let _ = self.buffer_view.lock().get_edit_state_mut().set_char(pos, attributed_char);
    }

    #[must_use]
    pub fn begin_atomic_undo(&mut self, description: impl Into<String>) -> AtomicUndoGuard {
        self.buffer_view.lock().get_edit_state_mut().begin_atomic_undo(description.into())
    }

    pub fn fill(&mut self, rect: Rectangle, dos_char: AttributedChar) {
        let mut pos = rect.start;
        let _undo = self.begin_atomic_undo("Fill");
        for _ in 0..rect.size.height {
            for _ in 0..rect.size.width {
                self.set_char(pos, dos_char);
                pos.x += 1;
            }
            pos.y += 1;
            pos.x = rect.start.x;
        }
    }

    pub fn type_key(&mut self, char_code: char) {
        let pos = self.buffer_view.lock().get_caret().get_position();
        self.buffer_view.lock().clear_selection();
        if self.buffer_view.lock().get_caret().insert_mode {
            let end = self.buffer_view.lock().get_buffer().get_width();
            for i in (pos.x..end).rev() {
                let next = self.get_char_from_cur_layer(Position::new(i - 1, pos.y));
                self.set_char(Position::new(i, pos.y), next);
            }
        }
        let mut attr = self.get_char(pos).attribute;
        let caret_attr = self.buffer_view.lock().get_caret().get_attribute();
        attr.set_font_page(caret_attr.get_font_page());
        attr.attr = caret_attr.attr & !attribute::INVISIBLE;
        if self.color_mode.use_fore() {
            attr.set_foreground(caret_attr.get_foreground());
        }
        if self.color_mode.use_back() {
            attr.set_background(caret_attr.get_background());
        }

        self.set_char(pos, AttributedChar::new(char_code, attr));
        self.set_caret(pos.x + 1, pos.y);
        self.request_focus = true;
    }

    pub fn switch_fg_bg_color(&mut self) {
        let mut attr = self.buffer_view.lock().get_caret().get_attribute();
        let bg = attr.get_background();
        attr.set_background(attr.get_foreground());
        attr.set_foreground(bg);
        self.set_caret_attribute(attr);
    }

    pub fn erase_line(&mut self) {
        let _undo = self.begin_atomic_undo("Erase line");
        // TODO
    }

    pub fn erase_line_to_start(&mut self) {
        let _undo = self.begin_atomic_undo("Erase line to start");
        // TODO
    }

    pub fn erase_line_to_end(&mut self) {
        let _undo = self.begin_atomic_undo("Erase line to end");
        // TODO
    }

    pub fn erase_column(&mut self) {
        let _undo = self.begin_atomic_undo("Erase column");
        // TODO
    }

    pub fn erase_column_to_start(&mut self) {
        let _undo = self.begin_atomic_undo("Erase column to start");
        // TODO
    }

    pub fn erase_column_to_end(&mut self) {
        let _undo = self.begin_atomic_undo("Erase column to end");
        // TODO
    }

    pub fn delete_row(&mut self) {
        let _undo = self.begin_atomic_undo("Delete row");
        // TODO
    }

    pub fn insert_row(&mut self) {
        let _undo = self.begin_atomic_undo("Insert row");
        // TODO
    }

    pub fn delete_column(&mut self) {
        let _undo = self.begin_atomic_undo("Delete column");
        // TODO
    }

    pub fn insert_column(&mut self) {
        let _undo = self.begin_atomic_undo("Insert column");
        // TODO
    }

    fn handle_response(
        &mut self,
        ui: &egui::Ui,
        mut response: Response,
        calc: TerminalCalc,
        cur_tool: &mut Box<dyn Tool>,
        message: &mut Option<Message>,
    ) -> Response {
        if response.has_focus() {
            let events = ui.input(|i| i.events.clone());
            for e in &events {
                match e {
                    egui::Event::Copy => {}
                    egui::Event::Cut => {}
                    egui::Event::Paste(text) => {
                        self.output_string(text);
                    }

                    /* egui::Event::CompositionEnd(text) |*/
                    egui::Event::Text(text) => {
                        if !ui.input(|i| i.modifiers.ctrl || i.modifiers.command || i.modifiers.alt) {
                            for c in text.chars() {
                                cur_tool.handle_key(self, MKey::Character(c as u16), MModifiers::None);
                            }
                        }
                    }

                    egui::Event::Key {
                        key, pressed: true, modifiers, ..
                    } => {
                        let mut key_code = *key as u32;
                        if modifiers.shift {
                            key_code |= SHIFT_MOD;
                        }

                        let mut modifier: MModifiers = MModifiers::None;
                        if modifiers.ctrl || modifiers.command {
                            modifier = MModifiers::Control;
                        }

                        if modifiers.shift {
                            modifier = MModifiers::Shift;
                        }

                        if modifiers.alt {
                            modifier = MModifiers::Alt;
                        }
                        for (k, m) in EDITOR_KEY_MAP {
                            if *k == key_code {
                                cur_tool.handle_key(self, *m, modifier);
                                break;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        if response.clicked_by(egui::PointerButton::Primary) {
            if let Some(mouse_pos) = response.hover_pos() {
                if calc.buffer_rect.contains(mouse_pos) && !calc.vert_scrollbar_rect.contains(mouse_pos) && !calc.horiz_scrollbar_rect.contains(mouse_pos) {
                    let click_pos = calc.calc_click_pos(mouse_pos);
                    let cp_abs = Position::new(click_pos.x as i32, click_pos.y as i32);
                    let layer_offset = self.get_cur_click_offset();
                    let cp = cp_abs - layer_offset;
                    let click_pos2 = calc.calc_click_pos_half_block(mouse_pos);
                    self.half_block_click_pos = Position::new(click_pos2.x as i32 - layer_offset.x, click_pos2.y as i32 - layer_offset.y);

                    /*
                    let b: i32 = match responsee.b {
                                     PointerButton::Primary => 1,
                                     PointerButton::Secondary => 2,
                                     PointerButton::Middle => 3,
                                     PointerButton::Extra1 => 4,
                                     PointerButton::Extra2 => 5,
                                 }; */
                    let msg = cur_tool.handle_click(self, 1, cp, cp_abs, &response);
                    if message.is_none() {
                        *message = msg;
                    }
                }
            }
        }

        if response.drag_started_by(egui::PointerButton::Primary) {
            if let Some(mouse_pos) = response.hover_pos() {
                if calc.buffer_rect.contains(mouse_pos) && !calc.vert_scrollbar_rect.contains(mouse_pos) && !calc.horiz_scrollbar_rect.contains(mouse_pos) {
                    let click_pos = calc.calc_click_pos(mouse_pos);
                    let cp_abs = Position::new(click_pos.x as i32, click_pos.y as i32);

                    let layer_offset = self.get_cur_click_offset();
                    let cp = cp_abs - layer_offset;
                    let click_pos2 = calc.calc_click_pos_half_block(mouse_pos);
                    let click_pos2 = Position::new(click_pos2.x as i32, click_pos2.y as i32);
                    let half_block_layer_offset = Position::new(layer_offset.x, layer_offset.y * 2);
                    let half_block_click_pos = click_pos2 - half_block_layer_offset;
                    self.half_block_click_pos = half_block_click_pos;

                    self.drag_pos.start_abs = cp_abs;
                    self.drag_pos.start = cp;

                    self.drag_pos.cur_abs = cp_abs;
                    self.drag_pos.cur = cp;
                    self.drag_pos.start_half_block = half_block_click_pos;
                    self.drag_started = true;

                    cur_tool.handle_drag_begin(self, &response);
                }
            }
        }

        if response.dragged_by(egui::PointerButton::Primary) && self.drag_started {
            if let Some(mouse_pos) = response.hover_pos() {
                let layer_offset = self.get_cur_click_offset();
                let click_pos2 = calc.calc_click_pos_half_block(mouse_pos);
                let click_pos2 = Position::new(click_pos2.x as i32, click_pos2.y as i32);

                let half_block_layer_offset = Position::new(layer_offset.x, layer_offset.y * 2);
                let half_block_click_pos = click_pos2 - half_block_layer_offset;

                let mut c_abs = self.half_block_click_pos;
                while c_abs != half_block_click_pos {
                    let s = (half_block_click_pos - c_abs).signum();
                    c_abs += s;
                    self.half_block_click_pos = c_abs;

                    self.drag_pos.cur_abs = Position::new(c_abs.x, c_abs.y / 2) + layer_offset;
                    self.drag_pos.cur = self.drag_pos.cur_abs - layer_offset;
                    response = cur_tool.handle_drag(ui, response, self, &calc);
                }
            }
        }
        if response.hovered() {
            if let Some(mouse_pos) = response.hover_pos() {
                if calc.buffer_rect.contains(mouse_pos) && !calc.vert_scrollbar_rect.contains(mouse_pos) && !calc.horiz_scrollbar_rect.contains(mouse_pos) {
                    let click_pos = calc.calc_click_pos(mouse_pos);
                    let cp_abs = Position::new(click_pos.x as i32, click_pos.y as i32);
                    let cp = cp_abs - self.get_cur_click_offset();
                    response = cur_tool.handle_hover(ui, response, self, cp, cp_abs);
                } else {
                    cur_tool.handle_no_hover(self);
                }
            } else {
                cur_tool.handle_no_hover(self);
            }
        } else {
            cur_tool.handle_no_hover(self);
        }

        if response.drag_stopped_by(egui::PointerButton::Primary) {
            let msg = cur_tool.handle_drag_end(self);
            if msg.is_some() {
                *message = msg;
            }

            self.drag_started = false;
        }

        response
    }

    fn get_cur_click_offset(&mut self) -> Position {
        if let Some(layer) = self.buffer_view.lock().get_edit_state().get_cur_layer() {
            return layer.get_offset();
        }
        Position::default()
    }

    pub(crate) fn clear_overlay_layer(&self) {
        let cur_offset = self.buffer_view.lock().get_edit_state().get_cur_layer().unwrap().get_offset();

        if let Some(layer) = self.buffer_view.lock().get_edit_state_mut().get_overlay_layer() {
            layer.set_offset(cur_offset);
            layer.clear();
        }
        self.buffer_view.lock().get_edit_state_mut().set_is_buffer_dirty();
    }

    pub fn backspace(&mut self) {
        let _op: AtomicUndoGuard = self
            .buffer_view
            .lock()
            .get_edit_state_mut()
            .begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-backspace"));
        self.buffer_view.lock().clear_selection();
        let pos = self.get_caret_position();
        if pos.x > 0 {
            self.set_caret_position(pos + Position::new(-1, 0));
            if self.buffer_view.lock().get_caret().insert_mode {
                let end = self.buffer_view.lock().get_width() - 1;
                for i in pos.x..end {
                    let next = self.get_char_from_cur_layer(Position::new(i + 1, pos.y));
                    self.set_char(Position::new(i, pos.y), next);
                }
                let last_pos = Position::new(self.buffer_view.lock().get_width() - 1, pos.y);
                self.set_char(last_pos, AttributedChar::invisible());
            } else {
                let pos = self.get_caret_position();
                self.set_char(pos, AttributedChar::invisible());
            }
        }
    }

    pub fn delete(&mut self) {
        let _op: AtomicUndoGuard = self
            .buffer_view
            .lock()
            .get_edit_state_mut()
            .begin_atomic_undo(fl!(crate::LANGUAGE_LOADER, "undo-delete"));
        self.buffer_view.lock().clear_selection();
        let pos = self.get_caret_position();
        if pos.x >= 0 {
            let end = self.buffer_view.lock().get_width() - 1;
            for i in pos.x..end {
                let next = self.get_char_from_cur_layer(Position::new(i + 1, pos.y));
                self.set_char(Position::new(i, pos.y), next);
            }
            let last_pos = Position::new(self.buffer_view.lock().get_width() - 1, pos.y);
            self.set_char(last_pos, AttributedChar::invisible());
        }
    }
}

pub const DEFAULT_CHAR_SET_TABLE: [[u8; 10]; 15] = [
    [218, 191, 192, 217, 196, 179, 195, 180, 193, 194],
    [201, 187, 200, 188, 205, 186, 204, 185, 202, 203],
    [213, 184, 212, 190, 205, 179, 198, 181, 207, 209],
    [214, 183, 211, 189, 196, 186, 199, 182, 208, 210],
    [197, 206, 216, 215, 232, 233, 155, 156, 153, 239],
    [176, 177, 178, 219, 223, 220, 221, 222, 254, 250],
    [1, 2, 3, 4, 5, 6, 240, 127, 14, 15],
    [24, 25, 30, 31, 16, 17, 18, 29, 20, 21],
    [174, 175, 242, 243, 169, 170, 253, 246, 171, 172],
    [227, 241, 244, 245, 234, 157, 228, 248, 251, 252],
    [224, 225, 226, 229, 230, 231, 235, 236, 237, 238],
    [128, 135, 165, 164, 152, 159, 247, 249, 173, 168],
    [131, 132, 133, 160, 166, 134, 142, 143, 145, 146],
    [136, 137, 138, 130, 144, 140, 139, 141, 161, 158],
    [147, 148, 149, 162, 167, 150, 129, 151, 163, 154],
];

pub fn terminal_context_menu(editor: &AnsiEditor, commands: &Commands, ui: &mut egui::Ui) -> Option<Message> {
    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
    let mut result = None;
    ui.input_mut(|i| i.events.clear());

    commands.cut.ui(ui, &mut result);
    commands.copy.ui(ui, &mut result);
    commands.paste.ui(ui, &mut result);

    let sel = editor.buffer_view.lock().get_selection();

    if let Some(_sel) = sel {
        commands.erase_selection.ui(ui, &mut result);
        commands.flip_x.ui(ui, &mut result);
        commands.flip_y.ui(ui, &mut result);
        commands.justifycenter.ui(ui, &mut result);
        commands.justifyleft.ui(ui, &mut result);
        commands.justifyright.ui(ui, &mut result);
        commands.crop.ui(ui, &mut result);
    }
    result
}

pub const CTRL_MOD: u32 = 0b1000_0000_0000_0000_0000;
pub const SHIFT_MOD: u32 = 0b0100_0000_0000_0000_0000;

pub static EDITOR_KEY_MAP: &[(u32, MKey)] = &[
    (Key::Escape as u32, MKey::Escape),
    (Key::Home as u32, MKey::Home),
    (Key::End as u32, MKey::End),
    (Key::Home as u32 | CTRL_MOD, MKey::Home),
    (Key::End as u32 | CTRL_MOD, MKey::End),
    (Key::Insert as u32, MKey::Insert),
    (Key::Backspace as u32, MKey::Backspace),
    (Key::Enter as u32, MKey::Return),
    (Key::Tab as u32, MKey::Tab),
    (Key::Tab as u32 | SHIFT_MOD, MKey::Tab),
    (Key::Delete as u32, MKey::Delete),
    (Key::PageUp as u32, MKey::PageUp),
    (Key::PageDown as u32, MKey::PageDown),
    (Key::ArrowUp as u32, MKey::Up),
    (Key::ArrowDown as u32, MKey::Down),
    (Key::ArrowRight as u32, MKey::Right),
    (Key::ArrowLeft as u32, MKey::Left),
    (Key::ArrowUp as u32 | SHIFT_MOD, MKey::Up),
    (Key::ArrowDown as u32 | SHIFT_MOD, MKey::Down),
    (Key::ArrowRight as u32 | SHIFT_MOD, MKey::Right),
    (Key::ArrowLeft as u32 | SHIFT_MOD, MKey::Left),
    (Key::F1 as u32, MKey::F1),
    (Key::F2 as u32, MKey::F2),
    (Key::F3 as u32, MKey::F3),
    (Key::F4 as u32, MKey::F4),
    (Key::F5 as u32, MKey::F5),
    (Key::F6 as u32, MKey::F6),
    (Key::F7 as u32, MKey::F7),
    (Key::F8 as u32, MKey::F8),
    (Key::F9 as u32, MKey::F9),
    (Key::F10 as u32, MKey::F10),
    (Key::F11 as u32, MKey::F11),
    (Key::F12 as u32, MKey::F12),
];
