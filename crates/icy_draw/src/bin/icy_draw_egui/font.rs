use super::widgets::{self, Icons};
use eframe::egui::{self, Color32};
use icy_draw::fl;
use icy_engine::BitFont;
use icy_engine_edit::bitfont::{BitFontAtomicUndoGuard, BitFontClipboardData, BitFontEditState, BitFontFocusedPanel, BitFontUndoState, BITFONT_CLIPBOARD_TYPE};
use icy_engine_edit::tools::Tool;
use icy_engine_gui::egui::{
    appearance::{self, labels, DialogButton, MessageBox, MessageKind},
    screen::ScreenView,
};
use icy_engine_gui::system_clipboard;
use std::path::{Path, PathBuf};

pub enum Action {
    Apply(Box<BitFont>),
    Close,
}

pub struct FontEditor {
    pub state: BitFontEditState,
    pub path: Option<PathBuf>,
    /// Glyph hash when the font was loaded, saved or applied; see [`Self::modified`].
    baseline: u64,
    disk_bytes: Option<Vec<u8>>,
    stroke: Option<BitFontAtomicUndoGuard>,
    drawing: bool,
    last: Option<icy_engine::Position>,
    tool: Tool,
    drag_tool: Tool,
    drag_start: Option<icy_engine::Position>,
    icons: Icons,
    clipboard: Option<BitFontClipboardData>,
    /// Shows the question whether unsaved changes should be discarded.
    pub confirm_close: bool,
    /// The font was taken from the open ANSI document and can be applied back to it.
    pub apply_target: bool,
    dimensions: [i32; 2],
    preview: Option<ScreenView>,
    error: Option<String>,
    /// DOS palette indices of the glyph and background color.
    colors: (u32, u32),
    textures: Textures,
}

impl FontEditor {
    pub fn new(font: BitFont) -> Self {
        let dimensions = [font.size().width, font.size().height];
        let mut editor = Self {
            state: BitFontEditState::from_font(font),
            path: None,
            baseline: 0,
            disk_bytes: None,
            stroke: None,
            drawing: false,
            last: None,
            tool: Tool::Click,
            drag_tool: Tool::Click,
            drag_start: None,
            icons: Icons::default(),
            clipboard: None,
            confirm_close: false,
            apply_target: false,
            dimensions,
            preview: None,
            error: None,
            colors: (7, 0),
            textures: Textures::default(),
        };
        editor.baseline = editor.content_hash();
        editor
    }

    fn content_hash(&self) -> u64 {
        let glyphs = self.state.get_all_glyph_data();
        hash_glyphs(glyphs.iter(), self.state.font_width().max(0) as usize, self.state.font_height().max(0) as usize)
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let data = std::fs::read(path).map_err(|error| error.to_string())?;
        let font = BitFont::from_bytes(path.file_name().unwrap_or_default().to_string_lossy().to_string(), &data).map_err(|error| error.to_string())?;
        if font.size().width > 8 {
            return Err(fl!("error-bitmap-font-width"));
        }
        let mut editor = Self::new(font);
        editor.path = Some(path.to_path_buf());
        editor.disk_bytes = Some(data);
        Ok(editor)
    }

    pub fn modified(&self) -> bool {
        // Called every frame for the window title, so this compares a hash instead of encoding the font.
        self.content_hash() != self.baseline
    }

    /// The font for crash recovery.
    pub fn recovery_snapshot(&self) -> Result<icy_draw::recovery::Snapshot, String> {
        Ok(icy_draw::recovery::Snapshot {
            kind: icy_draw::recovery::RecoveryKind::BitFont,
            path: self.path.clone(),
            disk: self.disk_bytes.as_deref().map(icy_draw::recovery::Fingerprint::of),
            payload: self.state.build_font().to_psf2_bytes().map_err(|error| error.to_string())?,
        })
    }

    /// Restores a [`Self::recovery_snapshot`]; it counts as modified until saved.
    pub fn from_recovery(snapshot: &icy_draw::recovery::Snapshot) -> Result<Self, String> {
        let name = snapshot
            .path
            .as_ref()
            .and_then(|path| path.file_name())
            .map_or_else(|| fl!("unsaved-title"), |name| name.to_string_lossy().into_owned());
        let font = BitFont::from_bytes(name, &snapshot.payload).map_err(|error| error.to_string())?;
        let mut editor = Self::new(font);
        editor.path = snapshot.path.clone();
        editor.disk_bytes = snapshot.path.as_deref().zip(snapshot.disk).and_then(|(path, disk)| disk.read_matching(path));
        // No saved state matches the recovered glyphs until they are saved.
        editor.baseline = !editor.content_hash();
        Ok(editor)
    }

    pub fn mcp_status(&self) -> icy_draw::mcp::types::BitFontStatus {
        let count = self.state.get_all_glyph_data().len();
        icy_draw::mcp::types::BitFontStatus {
            glyph_width: self.state.font_width(),
            glyph_height: self.state.font_height(),
            glyph_count: count,
            first_char: 0,
            last_char: count.saturating_sub(1) as u32,
            selected_char: self.state.selected_char() as u32,
        }
    }

    pub fn mcp_get(&self, code: u32) -> Result<icy_draw::mcp::types::GlyphData, String> {
        use base64::Engine;
        let pixels = self.state.get_all_glyph_data().get(code as usize).ok_or("Character out of bounds")?;
        let bits: Vec<_> = pixels.iter().flatten().copied().collect();
        let mut bytes = vec![0u8; bits.len().div_ceil(8)];
        for (index, enabled) in bits.into_iter().enumerate() {
            if enabled {
                bytes[index / 8] |= 1 << (7 - index % 8);
            }
        }
        Ok(icy_draw::mcp::types::GlyphData {
            code,
            char: char::from_u32(code).map(|character| character.to_string()),
            width: self.state.font_width(),
            height: self.state.font_height(),
            bitmap: base64::engine::general_purpose::STANDARD.encode(bytes),
        })
    }

    pub fn mcp_set(&mut self, code: u32, data: &icy_draw::mcp::types::GlyphData) -> Result<(), String> {
        use base64::Engine;
        if code as usize >= self.state.get_all_glyph_data().len()
            || code != data.code
            || data.width != self.state.font_width()
            || data.height != self.state.font_height()
        {
            return Err("Glyph code or dimensions mismatch".into());
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&data.bitmap)
            .map_err(|error| error.to_string())?;
        let count = (data.width * data.height) as usize;
        if bytes.len() != count.div_ceil(8) {
            return Err("Glyph bitmap size mismatch".into());
        }
        let pixels = (0..data.height as usize)
            .map(|row| {
                (0..data.width as usize)
                    .map(|column| {
                        let index = row * data.width as usize + column;
                        bytes[index / 8] & (1 << (7 - index % 8)) != 0
                    })
                    .collect()
            })
            .collect();
        self.finish();
        self.preview = None;
        self.state
            .set_glyph_pixels(char::from_u32(code).ok_or("Invalid character")?, pixels)
            .map_err(|error| error.to_string())
    }

    pub fn finish(&mut self) {
        if let Some(start) = self.drag_start.take().filter(|_| self.drag_tool.is_shape_tool()) {
            let (column, row) = self.state.cursor_pos();
            let character = self.state.selected_char();
            let result = if self.drag_tool == Tool::Line {
                self.state.draw_line(character, start.x, start.y, column, row, true)
            } else {
                self.state
                    .draw_rectangle(character, start.x, start.y, column, row, self.drag_tool == Tool::RectangleFilled, true)
            };
            if let Err(error) = result {
                self.error = Some(error.to_string());
            }
        }
        if let Some(mut stroke) = self.stroke.take() {
            if let Some((count, description, operation)) = stroke.end_params() {
                self.state.end_atomic_undo(count, description, operation);
            }
        }
        self.last = None;
    }

    fn begin_grid(&mut self, point: icy_engine::Position, erase: bool) {
        self.finish();
        self.preview = None;
        self.state.set_focused_panel(BitFontFocusedPanel::EditGrid);
        self.state.set_cursor_pos(point.x, point.y);
        self.drag_tool = if erase && self.tool != Tool::Fill { Tool::Click } else { self.tool };
        let character = self.state.selected_char();
        if self.drag_tool == Tool::Click && !erase && self.state.edit_selection().is_some() {
            self.state.clear_edit_selection();
            return;
        }
        if self.drag_tool == Tool::Fill {
            self.operation(|state| state.flood_fill(character, point.x, point.y, !erase));
            return;
        }
        self.stroke = Some(self.state.begin_atomic_undo("Draw glyph"));
        self.drag_start = Some(point);
        if self.drag_tool == Tool::Select {
            self.state.start_edit_selection();
        } else if self.drag_tool == Tool::Click {
            self.drawing = !erase && !self.state.get_glyph_pixels(character)[point.y as usize][point.x as usize];
        }
        self.update_grid(point);
    }

    fn update_grid(&mut self, point: icy_engine::Position) {
        if self.stroke.is_none() {
            return;
        }
        self.state.set_cursor_pos(point.x, point.y);
        if self.drag_tool == Tool::Select {
            self.state.extend_edit_selection();
        } else if self.drag_tool == Tool::Click {
            let character = self.state.selected_char();
            for point in icy_engine_edit::brushes::get_line_points(self.last.unwrap_or(point), point) {
                if let Err(error) = self.state.set_pixel(character, point.x, point.y, self.drawing) {
                    self.error = Some(error.to_string());
                    break;
                }
            }
        }
        self.last = Some(point);
    }

    fn shape_preview(&self) -> Vec<(i32, i32)> {
        let Some(start) = self.drag_start else {
            return Vec::new();
        };
        let (column, row) = self.state.cursor_pos();
        match self.drag_tool {
            Tool::Line => icy_engine_edit::bitfont::brushes::bresenham_line(start.x, start.y, column, row),
            Tool::RectangleOutline | Tool::RectangleFilled => {
                icy_engine_edit::bitfont::brushes::rectangle_points(start.x, start.y, column, row, self.drag_tool == Tool::RectangleFilled)
            }
            _ => Vec::new(),
        }
    }

    pub fn save(&mut self, path: &Path, overwrite: bool) -> Result<(), String> {
        self.finish();
        if !path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("psf")) {
            return Err(fl!("error-font-psf-extension"));
        }
        let bytes = self.state.build_font().to_psf2_bytes().map_err(|error| error.to_string())?;
        icy_draw::files::save_bytes(path, &bytes, self.path.as_deref().zip(self.disk_bytes.as_deref()), overwrite)?;
        self.disk_bytes = Some(bytes);
        self.baseline = self.content_hash();
        self.path = Some(path.to_path_buf());
        self.state.mark_saved();
        Ok(())
    }

    fn operation(&mut self, operation: impl FnOnce(&mut BitFontEditState) -> icy_engine::Result<()>) {
        self.finish();
        self.preview = None;
        if let Err(error) = operation(&mut self.state) {
            self.error = Some(error.to_string());
        }
    }

    fn key(&mut self, key: egui::Key, modifiers: egui::Modifiers) -> Option<Action> {
        use egui::Key;
        let character = self.state.selected_char();
        let grid = self.state.focused_panel() == BitFontFocusedPanel::EditGrid;
        if modifiers.command {
            match key {
                Key::Z if modifiers.shift => self.operation(|state| state.redo()),
                Key::Z => self.operation(|state| state.undo()),
                Key::Y => self.operation(|state| state.redo()),
                Key::A => {
                    if grid {
                        self.state
                            .set_selection(Some((0, 0, self.state.font_width() - 1, self.state.font_height() - 1)));
                    } else {
                        self.state
                            .set_charset_selection(Some((icy_engine::Position::new(0, 0), icy_engine::Position::new(15, 15), true)));
                    }
                }
                _ => {}
            }
        }
        match key {
            Key::Tab => {
                self.finish();
                self.state
                    .set_focused_panel(if grid { BitFontFocusedPanel::CharSet } else { BitFontFocusedPanel::EditGrid });
            }
            Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown => {
                self.finish();
                let (horizontal, vertical) = match key {
                    Key::ArrowLeft => (-1, 0),
                    Key::ArrowRight => (1, 0),
                    Key::ArrowUp => (0, -1),
                    _ => (0, 1),
                };
                if modifiers.ctrl || modifiers.command {
                    self.operation(|state| state.slide_glyph(horizontal, vertical));
                } else if grid && modifiers.alt {
                    match key {
                        Key::ArrowLeft => self.operation(|state| state.delete_column()),
                        Key::ArrowRight => self.operation(|state| state.insert_column()),
                        Key::ArrowUp => self.operation(|state| state.delete_line()),
                        _ => self.operation(|state| state.insert_line()),
                    }
                    self.dimensions = [self.state.font_width(), self.state.font_height()];
                } else if grid && modifiers.shift {
                    self.state.move_cursor_and_extend_selection(horizontal, vertical);
                } else if !grid && modifiers.shift {
                    self.state.move_charset_cursor_and_extend_selection(horizontal, vertical, modifiers.alt);
                } else if grid {
                    self.state.move_cursor(horizontal, vertical);
                } else {
                    self.state.clear_charset_selection();
                    self.state.move_charset_cursor(horizontal, vertical);
                }
            }
            Key::Space | Key::Enter if !modifiers.command && !modifiers.alt => {
                if grid {
                    let (column, row) = self.state.cursor_pos();
                    self.operation(|state| state.toggle_pixel(character, column, row));
                } else {
                    self.finish();
                    self.state.select_char_at_cursor();
                    self.state.set_focused_panel(BitFontFocusedPanel::EditGrid);
                }
            }
            Key::Home | Key::End | Key::PageUp | Key::PageDown => {
                let (mut column, mut row) = if grid { self.state.cursor_pos() } else { self.state.charset_cursor() };
                match key {
                    Key::Home => column = 0,
                    Key::End => column = if grid { self.state.font_width() - 1 } else { 15 },
                    Key::PageUp => row = 0,
                    _ => row = if grid { self.state.font_height() - 1 } else { 15 },
                }
                if grid {
                    self.state.set_cursor_pos(column, row);
                } else {
                    self.state.set_charset_cursor(column, row);
                }
            }
            Key::Plus | Key::Equals | Key::Minus if !modifiers.command => {
                self.finish();
                let code = if key == Key::Minus {
                    (character as u32).saturating_sub(1)
                } else {
                    (character as u32 + 1).min(255)
                };
                self.state.set_selected_char(char::from_u32(code).unwrap());
            }
            Key::Delete | Key::Backspace if !modifiers.command => self.operation(|state| state.erase_selection()),
            Key::Escape => {
                self.drag_start = None;
                self.finish();
                self.state.clear_selection();
            }
            _ => {}
        }
        None
    }

    pub fn undo(&mut self, redo: bool) {
        self.operation(|state| if redo { state.redo() } else { state.undo() });
    }

    pub fn can_undo(&self) -> bool {
        self.state.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.state.can_redo()
    }

    /// Copies the selected pixels to the system clipboard, with the pixels as text art so egui
    /// notices a paste and other programs get something readable.
    pub fn copy(&mut self) {
        let data = BitFontClipboardData::new(self.state.get_copy_data());
        let art: String = data
            .pixels
            .iter()
            .map(|row| row.iter().map(|&set| if set { '#' } else { '.' }).chain(['\n']).collect::<String>())
            .collect();
        if let Err(error) = system_clipboard::copy_format(BITFONT_CLIPBOARD_TYPE, data.to_bytes(), &art) {
            log::warn!("could not copy glyph pixels to the system clipboard: {error}");
        }
        self.clipboard = Some(data);
    }

    pub fn cut(&mut self) {
        self.copy();
        self.operation(|state| state.erase_selection());
    }

    /// Pastes pixels from the system clipboard, or the pixels copied last in this window.
    pub fn paste(&mut self) {
        let copied = system_clipboard::read_format(BITFONT_CLIPBOARD_TYPE).and_then(|bytes| BitFontClipboardData::from_bytes(&bytes).ok());
        if let Some(data) = copied.or_else(|| self.clipboard.clone()) {
            self.operation(|state| state.paste_data(data));
        }
    }

    pub fn has_clipboard(&self) -> bool {
        self.clipboard.is_some() || system_clipboard::read_format(BITFONT_CLIPBOARD_TYPE).is_some()
    }

    pub fn select_all(&mut self) {
        self.key(egui::Key::A, egui::Modifiers::COMMAND);
    }

    pub fn glyph_operation(&mut self, operation: GlyphOperation) {
        let character = self.state.selected_char();
        match operation {
            GlyphOperation::FlipX => self.operation(|state| state.flip_glyph_x(character)),
            GlyphOperation::FlipY => self.operation(|state| state.flip_glyph_y(character)),
            GlyphOperation::Inverse => self.operation(|state| state.inverse_glyph(character)),
            GlyphOperation::Clear => self.operation(|state| state.clear_glyph(character)),
        }
    }

    /// Asks before leaving the editor with changes that were neither saved nor applied.
    pub fn request_close(&mut self) -> Option<Action> {
        self.finish();
        if self.modified() {
            self.confirm_close = true;
            None
        } else {
            Some(Action::Close)
        }
    }

    fn handle_events(&mut self, context: &egui::Context) {
        // Keys belong to another widget (e.g. a size field) unless the editor areas hold focus.
        let focus = context.memory(|memory| memory.focused());
        if focus.is_some_and(|id| id != editor_focus_id()) {
            return;
        }
        for event in context.input(|input| input.events.clone()) {
            match event {
                egui::Event::Key {
                    key, pressed: true, modifiers, ..
                } => {
                    // Application commands (open, save, new window, …) are handled by the main window.
                    if modifiers.command && !matches!(key, egui::Key::Z | egui::Key::Y | egui::Key::A) && !key_is_arrow(key) {
                        continue;
                    }
                    // Tab cycles the editor areas only while they hold focus; otherwise it moves egui focus.
                    if key == egui::Key::Tab && focus.is_none() {
                        continue;
                    }
                    self.key(key, modifiers);
                }
                egui::Event::Copy => self.copy(),
                egui::Event::Cut => self.cut(),
                egui::Event::Paste(_) => self.paste(),
                _ => {}
            }
        }
    }

    /// Shows the editor as the window's editing mode: tool rail, tool bar, glyph editor with the
    /// character set, tile preview and status bar, like the classic Icy Draw bitmap font editor.
    pub fn show(&mut self, context: &egui::Context, blocked: bool, layout: Layout) -> Option<Action> {
        let mut action = None;
        let interactive = !blocked && !self.confirm_close;
        if interactive {
            self.handle_events(context);
        }
        let panel_fill = context.style().visuals.panel_fill;
        egui::TopBottomPanel::top("font-toolbar")
            .exact_height(layout.toolbar_height)
            .frame(egui::Frame::new().fill(panel_fill))
            .show(context, |ui| {
                if !interactive {
                    ui.disable();
                }
                action = self.toolbar(ui).or(action.take());
            });
        egui::TopBottomPanel::bottom("font-status")
            .exact_height(layout.status_height)
            .frame(egui::Frame::new().fill(panel_fill))
            .show(context, |ui| self.status_bar(ui));
        egui::SidePanel::left("font-tools")
            .exact_width(layout.rail_width)
            .resizable(false)
            .frame(egui::Frame::new().fill(panel_fill))
            .show(context, |ui| {
                if !interactive {
                    ui.disable();
                }
                self.tool_rail(ui);
            });
        if context.content_rect().width() >= 760.0 {
            egui::SidePanel::right("font-tiles")
                .exact_width(200.0)
                .resizable(false)
                .frame(egui::Frame::new().fill(panel_fill).inner_margin(egui::Margin::symmetric(12, 8)))
                .show(context, |ui| self.tile_view(ui));
        }
        let well = if context.style().visuals.dark_mode {
            Color32::from_gray(22)
        } else {
            Color32::from_gray(212)
        };
        egui::CentralPanel::default().frame(egui::Frame::new().fill(well)).show(context, |ui| {
            if !interactive {
                ui.disable();
            }
            if let Some(preview) = &mut self.preview {
                let settings = icy_engine_gui::MonitorSettings {
                    scaling_mode: icy_engine_gui::ScalingMode::Auto,
                    ..Default::default()
                };
                preview.show(ui, &settings);
            } else {
                self.editor_area(ui);
            }
        });
        if self.confirm_close {
            #[derive(Clone, Copy)]
            enum Choice {
                Discard,
                KeepEditing,
            }
            let response = MessageBox::new("font-discard", MessageKind::Question, fl!("font-editor-discard-question"), "")
                .buttons([
                    DialogButton::destructive(fl!("ask_close_file_dialog-dont_save_button"), Choice::Discard).leading(),
                    DialogButton::primary(fl!("font-editor-keep-editing"), Choice::KeepEditing).cancels(),
                ])
                .show(context);
            match response.action {
                Some(Choice::Discard) => {
                    self.confirm_close = false;
                    action = Some(Action::Close);
                }
                Some(Choice::KeepEditing) => self.confirm_close = false,
                None if response.dismissed => self.confirm_close = false,
                None => {}
            }
        }
        action
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) -> Option<Action> {
        let mut action = None;
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.add_space(8.0);
            self.color_switcher(ui);
            ui.add_space(8.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                if self.apply_target {
                    if ui
                        .add(appearance::primary_button(fl!("font-editor-apply")))
                        .on_hover_text(fl!("font-editor-apply-tooltip"))
                        .clicked()
                    {
                        self.finish();
                        action = Some(Action::Apply(Box::new(self.state.build_font())));
                    }
                    if ui.button(labels::close()).on_hover_text(fl!("font-editor-close-tooltip")).clicked() {
                        action = self.request_close();
                    }
                    widgets::divider(ui);
                }
                let mut previewing = self.preview.is_some();
                if widgets::toggle(ui, &fl!("font-import-preview"), &mut previewing, &fl!("font-editor-preview-tooltip")).clicked() {
                    self.finish();
                    let character = self.state.selected_char();
                    self.preview =
                        previewing.then(|| ScreenView::new(self.state.build_preview_content_for(character, self.colors.0 as u8, self.colors.1 as u8)));
                }
                widgets::divider(ui);
                let size = [self.state.font_width(), self.state.font_height()];
                if ui
                    .add_enabled(self.dimensions != size, egui::Button::new(fl!("edit-canvas-size-resize")))
                    .clicked()
                {
                    let dimensions = self.dimensions;
                    self.operation(|state| state.resize_font(dimensions[0], dimensions[1]));
                }
                ui.add(egui::DragValue::new(&mut self.dimensions[1]).range(1..=32).suffix(" px"))
                    .on_hover_text(fl!("font-size-height"));
                ui.weak("×");
                ui.add(egui::DragValue::new(&mut self.dimensions[0]).range(1..=8).suffix(" px"))
                    .on_hover_text(fl!("font-size-width"));
                ui.weak(fl!("font-editor-size"));
                widgets::divider(ui);
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.add(egui::Label::new(hints_job(ui, &fl!("font-editor-hints"))).truncate());
                });
            });
        });
        action
    }

    /// Foreground over background swatch; a click swaps them, like the classic color switcher.
    fn color_switcher(&mut self, ui: &mut egui::Ui) {
        let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(32.0), egui::Sense::click());
        let swatch = egui::Vec2::splat(20.0);
        let back = egui::Rect::from_min_size(rect.max - swatch, swatch);
        let front = egui::Rect::from_min_size(rect.min, swatch);
        let painter = ui.painter();
        let stroke = egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.fg_stroke.color);
        painter.rect_filled(back, 2, palette_color(self.colors.1));
        painter.rect_stroke(back, 2, stroke, egui::StrokeKind::Inside);
        painter.rect_filled(front, 2, palette_color(self.colors.0));
        painter.rect_stroke(front, 2, stroke, egui::StrokeKind::Inside);
        if response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            self.colors = (self.colors.1, self.colors.0);
            self.preview = None;
        }
    }

    fn tool_rail(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.add_space(8.0);
            self.palette(ui);
            rail_divider(ui);
            for (index, tool) in TOOLS.into_iter().enumerate() {
                if index == 2 {
                    rail_divider(ui);
                }
                let (icon, name) = tool_entry(tool);
                if self.icons.button_sized(ui, icon, &name, self.tool == tool, 36.0).clicked() {
                    self.finish();
                    self.tool = tool;
                }
            }
        });
    }

    /// The 16 DOS colors in two columns: left click picks the glyph color, right click the background.
    fn palette(&mut self, ui: &mut egui::Ui) {
        let size = ((ui.available_width() - 8.0) / 2.0).floor().clamp(12.0, 22.0);
        let (rect, response) = ui.allocate_exact_size(egui::vec2(size * 2.0, size * 8.0), egui::Sense::click());
        let painter = ui.painter();
        let swatch = |index: u32| {
            egui::Rect::from_min_size(
                rect.min + egui::vec2((index / 8) as f32 * size, (index % 8) as f32 * size),
                egui::Vec2::splat(size),
            )
        };
        for index in 0..16 {
            let area = swatch(index);
            let color = palette_color(index);
            painter.rect_filled(area, 0, color);
            let marker = if color.r() as u32 + color.g() as u32 + color.b() as u32 > 384 {
                Color32::BLACK
            } else {
                Color32::WHITE
            };
            let corner = size * 0.4;
            if index == self.colors.0 {
                painter.add(egui::Shape::convex_polygon(
                    vec![
                        area.left_top(),
                        area.left_top() + egui::vec2(corner, 0.0),
                        area.left_top() + egui::vec2(0.0, corner),
                    ],
                    marker,
                    egui::Stroke::NONE,
                ));
            }
            if index == self.colors.1 {
                painter.add(egui::Shape::convex_polygon(
                    vec![
                        area.right_bottom(),
                        area.right_bottom() - egui::vec2(corner, 0.0),
                        area.right_bottom() - egui::vec2(0.0, corner),
                    ],
                    marker,
                    egui::Stroke::NONE,
                ));
            }
        }
        let picked = response
            .interact_pointer_pos()
            .filter(|_| response.clicked() || response.secondary_clicked())
            .map(|position| {
                let column = ((position.x - rect.left()) / size).clamp(0.0, 1.0) as u32;
                let row = ((position.y - rect.top()) / size).clamp(0.0, 7.0) as u32;
                column * 8 + row
            });
        if let Some(index) = picked {
            if response.secondary_clicked() {
                self.colors.1 = index;
            } else {
                self.colors.0 = index;
            }
            self.preview = None;
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.add_space(8.0);
            let small = |text: String| egui::RichText::new(text).size(12.0);
            let code = self.state.selected_char() as u32;
            ui.label(small(fl!(
                "font-editor-status-char",
                code = format!("{code:02X}"),
                char = cp437(code).to_string()
            )));
            ui.label(small("·".into()).weak());
            ui.label(small(format!("{} × {}", self.state.font_width(), self.state.font_height())).weak())
                .on_hover_text(fl!("font-editor-status-size"));
            if self.state.focused_panel() == BitFontFocusedPanel::EditGrid {
                let (column, row) = self.state.cursor_pos();
                ui.label(small("·".into()).weak());
                ui.label(small(format!("{column}, {row}")).weak());
            }
            if let Some(error) = &self.error {
                ui.label(small("·".into()).weak());
                ui.colored_label(ui.visuals().error_fg_color, small(error.clone()));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                let (undo, redo) = (self.state.undo_stack_len(), self.state.redo_stack_len());
                ui.label(small(format!("{} {undo}  ·  {} {redo}", fl!("menu-undo"), fl!("menu-redo"))).weak());
            });
        });
    }

    fn tile_view(&mut self, ui: &mut egui::Ui) {
        widgets::section_header(ui, &fl!("font-editor-tile_area"), |_| {});
        let width = self.state.font_width().max(1) as usize;
        let height = self.state.font_height().max(1) as usize;
        let pixels = self.state.get_glyph_pixels(self.state.selected_char());
        let key = hash_glyphs([pixels].into_iter(), width, height) ^ color_key(self.colors);
        if self.textures.tiles.as_ref().is_none_or(|(cached, _)| *cached != key) {
            let (foreground, background) = (palette_color(self.colors.0), palette_color(self.colors.1));
            let mut image = egui::ColorImage::filled([width * 8, height * 8], background);
            for tile_y in 0..8 {
                for tile_x in 0..8 {
                    for (row, line) in pixels.iter().enumerate().take(height) {
                        for (column, set) in line.iter().enumerate().take(width) {
                            if *set {
                                image[(tile_x * width + column, tile_y * height + row)] = foreground;
                            }
                        }
                    }
                }
            }
            self.textures.tiles = Some((key, ui.ctx().load_texture("font-tile-view", image, egui::TextureOptions::NEAREST)));
        }
        let Some((_, texture)) = &self.textures.tiles else {
            return;
        };
        let scale = (ui.available_width() / (width * 8) as f32).floor().max(1.0);
        ui.add(egui::Image::new(texture).fit_to_exact_size(egui::vec2((width * 8) as f32, (height * 8) as f32) * scale));
    }

    /// Glyph editor and character set side by side at the classic sizes (30 px pixels,
    /// characters at twice their size), scaled down together when the window is too small.
    fn editor_area(&mut self, ui: &mut egui::Ui) {
        let height = self.state.font_height().max(1);
        let columns = self.display_width();
        let available = ui.available_size();
        let title_height = 32.0;
        let gap = 32.0;
        let ideal_grid = egui::vec2(RULER + columns as f32 * (CELL + CELL_GAP), RULER + height as f32 * (CELL + CELL_GAP));
        let ideal_charset = egui::vec2(RULER + 16.0 * columns as f32 * 2.0, RULER + 16.0 * height as f32 * 2.0);
        let room = available - egui::vec2(gap + 32.0, title_height + 32.0);
        let scale = ((room.y - RULER) / (ideal_grid.y.max(ideal_charset.y) - RULER))
            .min((room.x - 2.0 * RULER) / (ideal_grid.x + ideal_charset.x - 2.0 * RULER))
            .clamp(MIN_SCALE, 1.0);
        let pitch = (CELL + CELL_GAP) * scale;
        let cell = egui::vec2(columns as f32 * 2.0, height as f32 * 2.0) * scale;
        let grid_size = egui::vec2(RULER + columns as f32 * pitch, RULER + height as f32 * pitch);
        let charset_size = egui::vec2(RULER, RULER) + cell * 16.0;
        let total = egui::vec2(grid_size.x + gap + charset_size.x, title_height + grid_size.y.max(charset_size.y));
        let origin = ui.max_rect().min + ((available - total) / 2.0).max(egui::Vec2::splat(8.0));
        // Registered beneath the grids; without click or drag sense it never takes their hover or clicks.
        let focus = ui.interact(
            egui::Rect::from_min_size(origin, total),
            editor_focus_id(),
            egui::Sense::focusable_noninteractive(),
        );
        let code = self.state.selected_char() as u32;
        let title_font = egui::FontId::proportional(16.0);
        let title_color = ui.visuals().strong_text_color();
        ui.painter().text(
            origin + egui::vec2(grid_size.x / 2.0, title_height / 2.0),
            egui::Align2::CENTER_CENTER,
            format!("0x{code:02X}: {}", cp437(code)),
            title_font.clone(),
            title_color,
        );
        let font_name = self.state.font_name();
        let charset_title = if font_name.is_empty() {
            fl!("font-editor-character-set")
        } else {
            font_name.to_owned()
        };
        let charset_origin = origin + egui::vec2(grid_size.x + gap, 0.0);
        ui.painter().text(
            charset_origin + egui::vec2(charset_size.x / 2.0, title_height / 2.0),
            egui::Align2::CENTER_CENTER,
            charset_title,
            title_font,
            title_color,
        );
        self.edit_grid(ui, egui::Rect::from_min_size(origin + egui::vec2(0.0, title_height), grid_size), pitch);
        self.charset(
            ui,
            egui::Rect::from_min_size(charset_origin + egui::vec2(0.0, title_height), charset_size),
            cell,
        );
        hold_keyboard_focus(ui, &focus);
    }

    /// Columns shown per glyph: the 9th column of 9-dot fonts is shown but not editable.
    fn display_width(&self) -> i32 {
        let width = self.state.font_width().max(1);
        if self.state.use_letter_spacing() && width == 8 {
            9
        } else {
            width
        }
    }

    fn edit_grid(&mut self, ui: &mut egui::Ui, rect: egui::Rect, pitch: f32) {
        let width = self.state.font_width();
        let height = self.state.font_height();
        let columns = self.display_width();
        let character = self.state.selected_char();
        let grid = egui::Rect::from_min_max(rect.min + egui::Vec2::splat(RULER), rect.max);
        let response = ui.interact(grid, ui.id().with("font-edit-grid"), egui::Sense::click_and_drag());
        let painter = ui.painter_at(rect.expand(2.0));
        let focused = self.state.focused_panel() == BitFontFocusedPanel::EditGrid;
        let (cursor_column, cursor_row) = self.state.cursor_pos();
        rulers(
            ui,
            &painter,
            rect,
            focused,
            (cursor_column, cursor_row),
            (columns, height),
            egui::Vec2::splat(pitch),
        );
        let gap = (CELL_GAP * pitch / (CELL + CELL_GAP)).max(1.0);
        let cell_rect =
            |column: i32, row: i32| egui::Rect::from_min_size(grid.min + egui::vec2(column as f32, row as f32) * pitch, egui::Vec2::splat(pitch - gap));
        let (foreground, background) = (palette_color(self.colors.0), palette_color(self.colors.1));
        let (cursor_foreground, cursor_background) = cursor_colors(foreground, background);
        let pixels = self.state.get_glyph_pixels(character);
        let box_drawing = (0xC0..=0xDF).contains(&(character as u32));
        for row in 0..height {
            for column in 0..columns {
                let set = if column < width {
                    pixels[row as usize][column as usize]
                } else {
                    box_drawing && pixels[row as usize][7]
                };
                let color = if column >= width {
                    if set {
                        foreground.gamma_multiply(0.85)
                    } else {
                        NINE_DOT_COLUMN
                    }
                } else if focused && column == cursor_column && row == cursor_row {
                    if set {
                        cursor_foreground
                    } else {
                        cursor_background
                    }
                } else if set {
                    foreground
                } else {
                    background
                };
                painter.rect_filled(cell_rect(column, row), 0, color);
            }
        }
        if columns > width {
            let x = grid.left() + width as f32 * pitch - gap / 2.0;
            painter.vline(x, grid.y_range(), egui::Stroke::new(2.0, NINE_DOT_SEPARATOR));
        }
        if let Some((x1, y1, x2, y2)) = self.state.selection() {
            let area = cell_rect(x1.min(x2), y1.min(y2))
                .union(cell_rect(x1.max(x2), y1.max(y2)))
                .expand(gap / 2.0 + 1.0);
            painter.rect_filled(area, 0, SELECTION_FILL);
            painter.rect_stroke(area, 0, egui::Stroke::new(2.0, SELECTION_BORDER), egui::StrokeKind::Middle);
        }
        for (column, row) in self.shape_preview() {
            if (0..width).contains(&column) && (0..height).contains(&row) {
                painter.rect_filled(cell_rect(column, row), 0, SHAPE_PREVIEW);
            }
        }
        let cell_at = |position: egui::Pos2| {
            icy_engine::Position::new(
                (((position.x - grid.left()) / pitch) as i32).clamp(0, width - 1),
                (((position.y - grid.top()) / pitch) as i32).clamp(0, height - 1),
            )
        };
        if let Some(position) = response.hover_pos() {
            let cell = cell_at(position);
            if !(focused && cell.x == cursor_column && cell.y == cursor_row) {
                corner_brackets(&painter, cell_rect(cell.x, cell.y), HOVER, 2.0);
            }
        }
        if !ui.is_enabled() {
            return;
        }
        let pointer = ui.input(|input| input.pointer.clone());
        if response.hovered() && pointer.button_pressed(egui::PointerButton::Middle) {
            self.finish();
            self.tool = if self.tool == Tool::Select { Tool::Click } else { Tool::Select };
            return;
        }
        if response.hovered() && (pointer.button_pressed(egui::PointerButton::Primary) || pointer.button_pressed(egui::PointerButton::Secondary)) {
            if let Some(position) = pointer.interact_pos() {
                self.preview = None;
                self.begin_grid(cell_at(position), pointer.secondary_down());
            }
        }
        if self.stroke.is_some() {
            if let Some(position) = pointer.interact_pos() {
                self.update_grid(cell_at(position));
            }
            if !pointer.any_down() {
                self.finish();
            }
        }
    }

    fn charset(&mut self, ui: &mut egui::Ui, rect: egui::Rect, cell: egui::Vec2) {
        let width = self.state.font_width().max(1) as usize;
        let height = self.state.font_height().max(1) as usize;
        let columns = self.display_width() as usize;
        let grid = egui::Rect::from_min_size(rect.min + egui::Vec2::splat(RULER), cell * 16.0);
        let response = ui.interact(grid, ui.id().with("font-charset"), egui::Sense::click_and_drag());
        let painter = ui.painter_at(rect.expand(2.0));
        let focused = self.state.focused_panel() == BitFontFocusedPanel::CharSet;
        let (cursor_column, cursor_row) = self.state.charset_cursor();
        rulers(ui, &painter, rect, focused, (cursor_column, cursor_row), (16, 16), cell);
        let (foreground, background) = (palette_color(self.colors.0), palette_color(self.colors.1));
        let cell_rect = |code: i32| egui::Rect::from_min_size(grid.min + egui::vec2((code % 16) as f32 * cell.x, (code / 16) as f32 * cell.y), cell);
        let selected = self.state.selected_char() as i32;
        painter.rect_filled(grid, 0, background);
        painter.rect_filled(cell_rect(selected), 0, CHAR_HIGHLIGHT);

        let glyphs = self.state.get_all_glyph_data();
        let key = hash_glyphs(glyphs.iter().take(256), width, height) ^ color_key(self.colors) ^ columns as u64;
        if self.textures.charset.as_ref().is_none_or(|(cached, _)| *cached != key) {
            // Glyph pixels on a transparent ground, so cell highlights show through.
            let mut image = egui::ColorImage::filled([columns * 16, height * 16], Color32::TRANSPARENT);
            for (code, pixels) in glyphs.iter().enumerate().take(256) {
                let (base_x, base_y) = ((code % 16) * columns, (code / 16) * height);
                let box_drawing = (0xC0..=0xDF).contains(&code);
                for (row, line) in pixels.iter().enumerate().take(height) {
                    for column in 0..columns {
                        let set = if column < width {
                            line.get(column).copied().unwrap_or(false)
                        } else {
                            box_drawing && line.get(7).copied().unwrap_or(false)
                        };
                        if set {
                            image[(base_x + column, base_y + row)] = foreground;
                        }
                    }
                }
            }
            self.textures.charset = Some((key, ui.ctx().load_texture("font-charset", image, egui::TextureOptions::NEAREST)));
        }
        if let Some((_, texture)) = &self.textures.charset {
            painter.image(
                texture.id(),
                grid,
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }

        if let Some((anchor, lead, rectangle)) = self.state.charset_selection() {
            let (anchor, lead) = (anchor.y * 16 + anchor.x, lead.y * 16 + lead.x);
            if rectangle {
                let (left, right) = ((anchor % 16).min(lead % 16), (anchor % 16).max(lead % 16));
                let (top, bottom) = ((anchor / 16).min(lead / 16), (anchor / 16).max(lead / 16));
                let area = cell_rect(top * 16 + left).union(cell_rect(bottom * 16 + right));
                painter.rect_filled(area, 0, RECT_SELECTION_FILL);
                painter.rect_stroke(area.expand(1.0), 0, egui::Stroke::new(2.0, RECT_SELECTION_BORDER), egui::StrokeKind::Middle);
            } else {
                let (start, end) = (anchor.min(lead), anchor.max(lead));
                let inside = |column: i32, row: i32| (0..16).contains(&column) && (0..16).contains(&row) && (start..=end).contains(&(row * 16 + column));
                let stroke = egui::Stroke::new(1.5, SELECTION_BORDER);
                for code in start..=end {
                    let area = cell_rect(code);
                    painter.rect_filled(area, 0, SELECTION_FILL);
                    let (column, row) = (code % 16, code / 16);
                    // Outline only the outer edges, so a wrapped range reads as one shape.
                    if !inside(column, row - 1) {
                        painter.hline(area.x_range(), area.top(), stroke);
                    }
                    if !inside(column, row + 1) {
                        painter.hline(area.x_range(), area.bottom(), stroke);
                    }
                    if !inside(column - 1, row) {
                        painter.vline(area.left(), area.y_range(), stroke);
                    }
                    if !inside(column + 1, row) {
                        painter.vline(area.right(), area.y_range(), stroke);
                    }
                }
            }
        }
        if focused {
            let code = (cursor_row * 16 + cursor_column).clamp(0, 255);
            let area = cell_rect(code);
            let (cursor_foreground, cursor_background) = cursor_colors(foreground, background);
            painter.rect_filled(area, 0, cursor_background);
            let pixel = egui::vec2(area.width() / columns as f32, area.height() / height as f32);
            if let Some(pixels) = glyphs.get(code as usize) {
                for (row, line) in pixels.iter().enumerate().take(height) {
                    for (column, set) in line.iter().enumerate().take(width) {
                        if *set {
                            painter.rect_filled(
                                egui::Rect::from_min_size(area.min + egui::vec2(column as f32 * pixel.x, row as f32 * pixel.y), pixel),
                                0,
                                cursor_foreground,
                            );
                        }
                    }
                }
            }
        }
        let code_at = |position: egui::Pos2| {
            let column = (((position.x - grid.left()) / cell.x) as i32).clamp(0, 15);
            let row = (((position.y - grid.top()) / cell.y) as i32).clamp(0, 15);
            (column, row)
        };
        if let Some(position) = response.hover_pos() {
            let (column, row) = code_at(position);
            if !(focused && column == cursor_column && row == cursor_row) {
                corner_brackets(&painter, cell_rect(row * 16 + column), HOVER, 1.5);
            }
        }
        if !ui.is_enabled() {
            return;
        }
        let pressed = response.hovered() && ui.input(|input| input.pointer.primary_pressed());
        if pressed {
            if let Some(position) = ui.input(|input| input.pointer.interact_pos()) {
                let (column, row) = code_at(position);
                self.finish();
                self.preview = None;
                self.state.set_selected_char(char::from_u32((row * 16 + column) as u32).unwrap_or(' '));
                self.state.set_charset_cursor(column, row);
                self.state.clear_charset_selection();
                self.state.set_focused_panel(BitFontFocusedPanel::CharSet);
            }
        } else if response.dragged() {
            if let Some(position) = response.interact_pointer_pos() {
                let (column, row) = code_at(position);
                if (column, row) != self.state.charset_cursor() || self.state.charset_selection().is_some() {
                    let rectangle = ui.input(|input| input.modifiers.alt);
                    if self.state.charset_selection().is_none() {
                        self.state.start_charset_selection_with_mode(rectangle);
                    }
                    self.state.set_charset_cursor(column, row);
                    self.state.extend_charset_selection_with_mode(rectangle);
                }
            }
        }
        if let Some(position) = response.hover_pos() {
            let (column, row) = code_at(position);
            let code = (row * 16 + column) as u32;
            response.on_hover_text(format!("0x{code:02X}  {}", cp437(code)));
        }
    }
}

/// Layout shared with the ANSI editor chrome, so switching modes keeps the frame in place.
#[derive(Clone, Copy)]
pub struct Layout {
    pub toolbar_height: f32,
    pub status_height: f32,
    pub rail_width: f32,
}

#[derive(Clone, Copy)]
pub enum GlyphOperation {
    FlipX,
    FlipY,
    Inverse,
    Clear,
}

/// Size of a glyph pixel and the gap between pixels at full size, as in the classic editor.
const CELL: f32 = 30.0;
const CELL_GAP: f32 = 2.0;
const RULER: f32 = 24.0;
const MIN_SCALE: f32 = 0.25;
const SELECTION_FILL: Color32 = Color32::from_rgba_premultiplied(18, 45, 89, 89);
const SELECTION_BORDER: Color32 = Color32::from_rgb(77, 153, 255);
const RECT_SELECTION_FILL: Color32 = Color32::from_rgba_premultiplied(46, 31, 77, 77);
const RECT_SELECTION_BORDER: Color32 = Color32::from_rgb(153, 102, 255);
const SHAPE_PREVIEW: Color32 = Color32::from_rgba_premultiplied(153, 138, 31, 153);
const CHAR_HIGHLIGHT: Color32 = Color32::from_rgb(38, 89, 140);
const NINE_DOT_COLUMN: Color32 = Color32::from_rgb(38, 38, 56);
const NINE_DOT_SEPARATOR: Color32 = Color32::from_rgb(89, 89, 140);
const HOVER: Color32 = Color32::from_rgb(93, 160, 232);
const TOOLS: [Tool; 6] = [Tool::Click, Tool::Select, Tool::Line, Tool::RectangleOutline, Tool::RectangleFilled, Tool::Fill];
/// Keys the glyph and character areas handle themselves instead of letting egui move focus.
const EDITOR_KEYS: egui::EventFilter = egui::EventFilter {
    tab: true,
    horizontal_arrows: true,
    vertical_arrows: true,
    escape: true,
};

/// One focus target for both editor areas: egui applies a lock filter only to a widget that kept
/// focus since the previous frame, so switching between two widgets would leak repeated Tabs.
fn editor_focus_id() -> egui::Id {
    egui::Id::new("font-editor-focus")
}

/// Keeps keyboard focus on the editor areas while no other widget uses it, like the ANSI canvas.
fn hold_keyboard_focus(ui: &egui::Ui, focus: &egui::Response) {
    if !ui.is_enabled() {
        focus.surrender_focus();
        return;
    }
    let pressed = focus.contains_pointer() && ui.input(|input| input.pointer.any_pressed());
    if !focus.has_focus() && (pressed || ui.memory(|memory| memory.focused().is_none())) {
        focus.request_focus();
        ui.ctx().request_repaint();
    }
    ui.memory_mut(|memory| memory.set_focus_lock_filter(focus.id, EDITOR_KEYS));
}

/// Textures that only change with the glyphs or colors, so they are not rebuilt every frame.
#[derive(Default)]
struct Textures {
    charset: Option<(u64, egui::TextureHandle)>,
    tiles: Option<(u64, egui::TextureHandle)>,
}

fn palette_color(index: u32) -> Color32 {
    let (red, green, blue) = icy_engine::DOS_DEFAULT_PALETTE[(index as usize) % 16].rgb();
    Color32::from_rgb(red, green, blue)
}

fn color_key(colors: (u32, u32)) -> u64 {
    (u64::from(colors.0) << 8 | u64::from(colors.1)).wrapping_mul(0x9e37_79b9_7f4a_7c15)
}

/// Hash of the glyph bitmaps, packing each row into one word so it stays cheap per frame.
fn hash_glyphs<'a>(glyphs: impl Iterator<Item = &'a Vec<Vec<bool>>>, width: usize, height: usize) -> u64 {
    use std::hash::Hasher;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    hasher.write_usize(width);
    hasher.write_usize(height);
    for glyph in glyphs {
        for row in glyph {
            hasher.write_u64(row.iter().enumerate().fold(0u64, |bits, (index, set)| bits | (u64::from(*set) << (index % 64))));
        }
    }
    hasher.finish()
}

/// Cursor colors that stand out against both the glyph and the background color.
fn cursor_colors(foreground: Color32, background: Color32) -> (Color32, Color32) {
    let luminance = |color: Color32| (0.2126 * f32::from(color.r()) + 0.7152 * f32::from(color.g()) + 0.0722 * f32::from(color.b())) / 255.0;
    let (foreground, background) = (luminance(foreground), luminance(background));
    if (foreground - background).abs() < 0.2 {
        return (Color32::from_rgb(255, 230, 0), Color32::from_rgb(0, 128, 255));
    }
    let set = if background > 0.5 {
        Color32::from_rgb(0, 102, 204)
    } else {
        Color32::from_rgb(255, 204, 0)
    };
    let unset = if foreground > 0.5 {
        Color32::from_rgb(51, 102, 179)
    } else {
        Color32::from_rgb(230, 153, 0)
    };
    (set, unset)
}

/// Hex rulers above and left of a grid; the focused grid gets the accent background.
#[allow(clippy::too_many_arguments)]
fn rulers(ui: &egui::Ui, painter: &egui::Painter, rect: egui::Rect, focused: bool, cursor: (i32, i32), count: (i32, i32), pitch: egui::Vec2) {
    let visuals = ui.visuals();
    let (fill, normal, highlight) = if focused {
        (HOVER, Color32::from_gray(24), Color32::WHITE)
    } else {
        (
            visuals.widgets.inactive.weak_bg_fill,
            visuals.weak_text_color().gamma_multiply(0.6),
            visuals.strong_text_color(),
        )
    };
    painter.rect_filled(egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), RULER)), 0, fill);
    painter.rect_filled(egui::Rect::from_min_size(rect.min, egui::vec2(RULER, rect.height())), 0, fill);
    let font = egui::FontId::proportional(pitch.min_elem().clamp(9.0, 16.0));
    for column in 0..count.0 {
        painter.text(
            egui::pos2(rect.left() + RULER + (column as f32 + 0.5) * pitch.x, rect.top() + RULER / 2.0),
            egui::Align2::CENTER_CENTER,
            format!("{column:X}"),
            font.clone(),
            if column == cursor.0 { highlight } else { normal },
        );
    }
    for row in 0..count.1 {
        painter.text(
            egui::pos2(rect.left() + RULER / 2.0, rect.top() + RULER + (row as f32 + 0.5) * pitch.y),
            egui::Align2::CENTER_CENTER,
            format!("{row:X}"),
            font.clone(),
            if row == cursor.1 { highlight } else { normal },
        );
    }
}

/// L-shaped marks at the corners of a hovered cell, leaving its content visible.
fn corner_brackets(painter: &egui::Painter, rect: egui::Rect, color: Color32, width: f32) {
    let length = (rect.width().min(rect.height()) * 0.3).max(3.0);
    let stroke = egui::Stroke::new(width, color);
    for (corner, horizontal, vertical) in [
        (rect.left_top(), 1.0, 1.0),
        (rect.right_top(), -1.0, 1.0),
        (rect.left_bottom(), 1.0, -1.0),
        (rect.right_bottom(), -1.0, -1.0),
    ] {
        painter.line_segment([corner, corner + egui::vec2(horizontal * length, 0.0)], stroke);
        painter.line_segment([corner, corner + egui::vec2(0.0, vertical * length)], stroke);
    }
}

/// "Key action · Key action" hints with the keys emphasized, like the classic tool bar.
fn hints_job(ui: &egui::Ui, hints: &str) -> egui::text::LayoutJob {
    let visuals = ui.visuals();
    let format = |color: Color32| egui::TextFormat {
        font_id: egui::FontId::proportional(13.0),
        color,
        ..Default::default()
    };
    let mut job = egui::text::LayoutJob::default();
    for (index, hint) in hints.split('·').map(str::trim).filter(|hint| !hint.is_empty()).enumerate() {
        if index > 0 {
            job.append("  |  ", 0.0, format(visuals.weak_text_color().gamma_multiply(0.5)));
        }
        let (key, action) = hint.split_once(' ').unwrap_or((hint, ""));
        job.append(key, 0.0, format(visuals.strong_text_color()));
        job.append(&format!(" {action}"), 0.0, format(visuals.weak_text_color()));
    }
    job
}

fn tool_entry(tool: Tool) -> (&'static str, String) {
    match tool {
        Tool::Select => ("select", fl!("font-editor-select-pixels")),
        Tool::Line => ("line", fl!("tool-line_name")),
        Tool::RectangleOutline => ("rectangle_outline", fl!("tool-rectangle_name")),
        Tool::RectangleFilled => ("rectangle_filled", fl!("tool-filled_rectangle_name")),
        Tool::Fill => ("fill", fl!("tool-fill_name")),
        _ => ("pencil", fl!("font-editor-pixels")),
    }
}

fn key_is_arrow(key: egui::Key) -> bool {
    matches!(key, egui::Key::ArrowLeft | egui::Key::ArrowRight | egui::Key::ArrowUp | egui::Key::ArrowDown)
}

/// The glyph of a code point in the DOS code page, for titles and tool tips.
fn cp437(code: u32) -> char {
    codepages::tables::CP437_TO_UNICODE.get(code as usize).copied().unwrap_or(' ')
}

fn rail_divider(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 9.0), egui::Sense::hover());
    ui.painter().hline(
        egui::Rangef::new(rect.center().x - 12.0, rect.center().x + 12.0),
        rect.center().y,
        ui.visuals().widgets.noninteractive.bg_stroke,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitmap_mouse_modes_preview_toggle_select_and_fill() {
        use icy_engine::Position;
        let mut editor = FontEditor::new(BitFont::from_ansi_font_page(0, 16).unwrap().clone());
        editor.state.set_selected_char('A');
        editor.state.clear_glyph('A').unwrap();
        editor.begin_grid(Position::new(1, 1), false);
        editor.update_grid(Position::new(3, 1));
        editor.finish();
        assert!(editor.state.get_glyph_pixels('A')[1][2]);
        editor.begin_grid(Position::new(1, 1), false);
        editor.update_grid(Position::new(3, 1));
        editor.finish();
        assert!(!editor.state.get_glyph_pixels('A')[1][2]);
        editor.tool = Tool::RectangleOutline;
        editor.begin_grid(Position::new(1, 1), false);
        editor.update_grid(Position::new(5, 5));
        assert!(!editor.shape_preview().is_empty());
        assert!(!editor.state.get_glyph_pixels('A')[1][1]);
        editor.finish();
        assert!(editor.state.get_glyph_pixels('A')[1][1]);
        editor.tool = Tool::Fill;
        editor.begin_grid(Position::new(3, 3), false);
        assert!(editor.state.get_glyph_pixels('A')[3][3]);
        assert!(!editor.state.get_glyph_pixels('A')[0][0]);
        editor.operation(|state| state.undo());
        assert!(!editor.state.get_glyph_pixels('A')[3][3]);
        editor.tool = Tool::Select;
        editor.begin_grid(Position::new(1, 1), false);
        editor.update_grid(Position::new(3, 3));
        editor.finish();
        assert!(editor.state.edit_selection().unwrap().is_inside(Position::new(2, 2)));
        editor.key(egui::Key::Delete, egui::Modifiers::NONE);
        assert!(!editor.state.get_glyph_pixels('A')[1][1]);
    }

    #[test]
    fn keyboard_routes_between_bitmap_grid_and_charset() {
        use egui::{Key, Modifiers};
        let mut editor = FontEditor::new(BitFont::from_ansi_font_page(0, 16).unwrap().clone());
        editor.state.set_selected_char('A');
        editor.state.clear_glyph('A').unwrap();
        editor.state.set_focused_panel(BitFontFocusedPanel::EditGrid);
        editor.state.set_cursor_pos(0, 0);
        editor.key(Key::ArrowRight, Modifiers::NONE);
        editor.key(Key::Space, Modifiers::NONE);
        assert!(editor.state.get_glyph_pixels('A')[0][1]);
        editor.key(
            Key::ArrowRight,
            Modifiers {
                ctrl: true,
                command: true,
                ..Default::default()
            },
        );
        assert!(editor.state.get_glyph_pixels('A')[0][2]);
        editor.key(Key::Z, Modifiers::COMMAND);
        assert!(editor.state.get_glyph_pixels('A')[0][1]);
        editor.key(Key::ArrowDown, Modifiers::SHIFT);
        assert!(editor.state.edit_selection().is_some());
        editor.key(Key::Escape, Modifiers::NONE);
        assert!(editor.state.edit_selection().is_none());
        editor.key(Key::Tab, Modifiers::NONE);
        assert_eq!(editor.state.focused_panel(), BitFontFocusedPanel::CharSet);
        editor.state.set_charset_cursor(2, 4);
        editor.key(Key::Enter, Modifiers::NONE);
        assert_eq!(editor.state.selected_char(), 'B');
        assert_eq!(editor.state.focused_panel(), BitFontFocusedPanel::EditGrid);
        editor.key(Key::PageDown, Modifiers::NONE);
        assert_eq!(editor.state.cursor_pos().1, 15);
    }

    #[test]
    fn tab_cycles_editor_areas_without_moving_widget_focus() {
        use egui::{Event, Key, Modifiers, RawInput};

        let context = egui::Context::default();
        let mut editor = FontEditor::new(BitFont::from_ansi_font_page(0, 16).unwrap().clone());
        editor.state.set_focused_panel(BitFontFocusedPanel::EditGrid);
        let layout = Layout {
            toolbar_height: 40.0,
            status_height: 24.0,
            rail_width: 48.0,
        };
        let key = |key, modifiers| Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        };
        let run = |editor: &mut FontEditor, events: Vec<Event>| {
            let _ = context.run(
                RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 820.0))),
                    events,
                    ..Default::default()
                },
                |context| {
                    editor.show(context, false, layout);
                },
            );
        };
        run(&mut editor, vec![]);
        run(&mut editor, vec![]);
        let focused = || context.memory(|memory| memory.focused());
        assert_eq!(focused(), Some(editor_focus_id()), "the editor areas should own keyboard focus");

        // Repeated Tabs in consecutive frames must never leak into egui's widget traversal.
        for (modifiers, expected) in [
            (Modifiers::NONE, BitFontFocusedPanel::CharSet),
            (Modifiers::NONE, BitFontFocusedPanel::EditGrid),
            (Modifiers::SHIFT, BitFontFocusedPanel::CharSet),
            (Modifiers::SHIFT, BitFontFocusedPanel::EditGrid),
        ] {
            run(&mut editor, vec![key(Key::Tab, modifiers)]);
            assert_eq!(editor.state.focused_panel(), expected);
            assert_eq!(focused(), Some(editor_focus_id()), "{modifiers:?}+Tab moved focus to another widget");
        }
        for editor_key in [Key::ArrowRight, Key::ArrowDown, Key::Escape] {
            run(&mut editor, vec![key(editor_key, Modifiers::NONE)]);
            assert_eq!(focused(), Some(editor_focus_id()), "{editor_key:?} moved focus away from the editor");
        }

        // A blocking dialog must get Tab back for its own widgets.
        let _ = context.run(Default::default(), |context| {
            editor.show(context, true, layout);
        });
        assert_eq!(focused(), None);
    }

    #[test]
    fn pixel_stroke_is_one_undo_and_psf_roundtrips() {
        let font = BitFont::from_ansi_font_page(0, 16).unwrap().clone();
        let mut editor = FontEditor::new(font);
        editor.state.clear_glyph('A').unwrap();
        editor.state.mark_saved();
        editor.baseline = editor.content_hash();
        let before = editor.state.undo_stack_len();
        editor.stroke = Some(editor.state.begin_atomic_undo("Draw glyph"));
        editor.state.set_pixel('A', 1, 1, true).unwrap();
        editor.state.set_pixel('A', 2, 1, true).unwrap();
        editor.finish();
        assert_eq!(editor.state.undo_stack_len(), before + 1);
        assert!(editor.modified());
        editor.state.undo().unwrap();
        assert!(!editor.modified());
        editor.state.redo().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("test.psf");
        editor.save(&path, false).unwrap();
        assert!(!editor.modified());
        let loaded = FontEditor::load(&path).unwrap();
        assert!(loaded.state.get_glyph_pixels('A')[1][1]);
        assert!(loaded.state.get_glyph_pixels('A')[1][2]);
        std::fs::write(&path, b"changed externally").unwrap();
        assert!(editor.save(&path, false).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"changed externally");
    }
}
