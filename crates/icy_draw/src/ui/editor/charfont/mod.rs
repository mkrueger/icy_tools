use std::{path::Path, sync::Arc};

use eframe::{
    egui::{self, Button, ScrollArea, SidePanel, TextEdit, TopBottomPanel},
    epaint::{mutex::Mutex, Vec2},
};
use egui::{load::SizedTexture, Image, Rect, TextureHandle};
use i18n_embed_fl::fl;
use icy_engine::{AttributedChar, BitFont, Buffer, EngineResult, FontGlyph, Layer, Size, TextAttribute, TextPane, TheDrawFont};
use icy_engine_gui::{show_terminal_area, BufferView};

use crate::{
    model::{click_imp::VALID_OUTLINE_CHARS, Tool},
    AnsiEditor, BitFontEditor, ClipboardHandler, Document, DocumentOptions, DrawGlyphStyle, Message, SelectOutlineDialog, TerminalResult, UndoHandler,
    SETTINGS,
};

pub struct CharFontEditor {
    id: usize,
    font: BitFont,
    selected_char_opt: Option<char>,
    old_selected_char_opt: Option<char>,

    outline_previewbuffer_view: Arc<Mutex<BufferView>>,

    ansi_editor: AnsiEditor,
    selected_font: usize,
    fonts: Vec<TheDrawFont>,
    undostack_len: usize,
    last_update_preview: usize,
    last_update_preview_attr: TextAttribute,
    outline_selection: crate::SelectOutlineDialog,
    draw_outline_bg: bool,
    opt_cheat_sheet: Option<TextureHandle>,
    tool_switch: bool,
}

impl ClipboardHandler for CharFontEditor {
    fn can_cut(&self) -> bool {
        self.ansi_editor.can_cut()
    }
    fn cut(&mut self) -> EngineResult<()> {
        self.ansi_editor.cut()
    }

    fn can_copy(&self) -> bool {
        self.ansi_editor.can_copy()
    }

    fn copy(&mut self) -> EngineResult<()> {
        self.ansi_editor.copy()
    }

    fn can_paste(&self) -> bool {
        self.ansi_editor.can_paste()
    }

    fn paste(&mut self) -> EngineResult<()> {
        self.ansi_editor.paste()
    }
}

impl UndoHandler for CharFontEditor {
    fn undo_description(&self) -> Option<String> {
        self.ansi_editor.undo_description()
    }

    fn can_undo(&self) -> bool {
        self.ansi_editor.can_undo()
    }

    fn undo(&mut self) -> EngineResult<Option<Message>> {
        self.ansi_editor.undo()?;
        Ok(None)
    }

    fn redo_description(&self) -> Option<String> {
        self.ansi_editor.redo_description()
    }

    fn can_redo(&self) -> bool {
        self.ansi_editor.can_redo()
    }

    fn redo(&mut self) -> EngineResult<Option<Message>> {
        self.ansi_editor.redo()?;
        Ok(None)
    }
}

impl Document for CharFontEditor {
    fn default_extension(&self) -> &'static str {
        "tdf"
    }
    fn has_tool_switched(&self) -> bool {
        self.tool_switch
    }
    fn set_tool_switch(&mut self, switched: bool) {
        self.tool_switch = switched;
    }

    fn undo_stack_len(&self) -> usize {
        self.undostack_len
    }

    fn get_bytes(&mut self, _path: &Path) -> TerminalResult<Vec<u8>> {
        self.undostack_len += 1;
        self.save_old_selected_char();
        TheDrawFont::create_font_bundle(&self.fonts)
    }

    fn show_ui(&mut self, ui: &mut egui::Ui, cur_tool: &mut Box<dyn Tool>, selected_tool: usize, options: &DocumentOptions) -> Option<Message> {
        SidePanel::left("side_panel").default_width(200.0).show_inside(ui, |ui| {
            ui.add_space(4.0);

            if self.selected_font < self.fonts.len() {
                ScrollArea::vertical().show(ui, |ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);

                    for i in 0..self.fonts.len() {
                        if ui.selectable_value(&mut self.selected_font, i, &self.fonts[i].name).clicked() {
                            self.save_old_selected_char();
                            self.selected_font = i;
                            self.old_selected_char_opt = None;
                            self.selected_char_opt = None;
                            self.show_selected_char(ui);
                        }
                    }
                });
            }
            ui.separator();

            ui.horizontal(|ui| {
                /*if ui.button("+").clicked() {
                    self.fonts.push(TheDrawFont::new(
                        "New Font",
                        icy_engine::FontType::Color,
                        1,
                    ));
                    self.selected_font = self.fonts.len() - 1;
                    self.selected_char_opt = None;
                    self.old_selected_char_opt = None;
                    self.show_selected_char();
                    self.undostack_len += 1;
                }*/

                if ui.add_enabled(self.fonts.len() > 1, Button::new("🗑")).clicked() {
                    self.fonts.remove(self.selected_font);
                    self.selected_font = 0;
                    self.selected_char_opt = None;
                    self.old_selected_char_opt = None;
                    self.show_selected_char(ui);
                    self.undostack_len += 1;
                }

                if ui.button(fl!(crate::LANGUAGE_LOADER, "tdf-editor-clone_button")).clicked() {
                    self.fonts.push(self.fonts[self.selected_font].clone());
                    self.selected_font = self.fonts.len() - 1;
                    self.selected_char_opt = None;
                    self.old_selected_char_opt = None;
                    self.show_selected_char(ui);
                    self.undostack_len += 1;
                }
            });
        });

        TopBottomPanel::top("char_top_panel").exact_height(60.).show_inside(ui, |ui| {
            ui.add_space(4.0);
            if self.selected_font < self.fonts.len() {
                egui::Grid::new(
                    "font_grid
                    ",
                )
                .num_columns(4)
                .spacing([4.0, 4.0])
                .show(ui, |ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(fl!(crate::LANGUAGE_LOADER, "tdf-editor-font_name_label"));
                    });
                    if ui
                        .add(
                            TextEdit::singleline(&mut self.fonts[self.selected_font].name)
                                .min_size(Vec2::new(200.0, 22.))
                                .char_limit(12),
                        )
                        .changed()
                    {
                        self.undostack_len += 1;
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(fl!(crate::LANGUAGE_LOADER, "tdf-editor-font_type_label"));
                    });

                    let text = match self.fonts[self.selected_font].font_type {
                        icy_engine::FontType::Outline => {
                            fl!(crate::LANGUAGE_LOADER, "tdf-editor-font_type_outline")
                        }
                        icy_engine::FontType::Block => {
                            fl!(crate::LANGUAGE_LOADER, "tdf-editor-font_type_block")
                        }
                        icy_engine::FontType::Color => {
                            fl!(crate::LANGUAGE_LOADER, "tdf-editor-font_type_color")
                        }
                    };
                    ui.label(text);

                    ui.end_row();
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(fl!(crate::LANGUAGE_LOADER, "tdf-editor-spacing_label"));
                    });
                    if ui
                        .add(egui::DragValue::new(&mut self.fonts[self.selected_font].spaces).range(0.0..=40.0))
                        .changed()
                    {
                        self.undostack_len += 1;
                    }
                    ui.label("");
                    ui.label("");
                    ui.end_row();
                });
            } else {
                ui.heading(fl!(crate::LANGUAGE_LOADER, "tdf-editor-no_font_selected_label"));
            }
        });

        TopBottomPanel::bottom("char_bottom_panel").exact_height(150.).show_inside(ui, |ui| {
            if self.selected_font < self.fonts.len() {
                self.show_char_selector(ui);
                ui.add_space(4.0);
                if self.selected_char_opt.is_some() && ui.button(fl!(crate::LANGUAGE_LOADER, "tdf-editor-clear_char_button")).clicked() {
                    self.fonts[self.selected_font].clear_glyph(self.selected_char_opt.unwrap());
                    self.selected_char_opt = None;
                    self.old_selected_char_opt = None;
                    self.show_selected_char(ui);
                    self.undostack_len += 1;
                }
            }
        });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            if self.selected_font < self.fonts.len() {
                let attr = self
                    .ansi_editor
                    .buffer_view
                    .lock()
                    .get_edit_state()
                    .get_caret()
                    .get_attribute();

                let mut is_outline = false;
                for layer in &mut self
                .ansi_editor
                    .buffer_view
                    .lock()
                    .get_edit_state_mut()
                    .get_buffer_mut()
                    .layers
                {
                    match self.fonts[self.selected_font].font_type {
                        icy_engine::FontType::Outline => {
                            is_outline = true;
                            set_attribute(layer, attr);
                        }
                        icy_engine::FontType::Block => {
                            set_attribute(layer, attr);
                        }
                        icy_engine::FontType::Color => {
                        }
                    }
                }

                if is_outline {
                    SidePanel::right("outline…_side_panel")
                        .default_width(290.)
                        .show_inside(ui, |ui| {
                            TopBottomPanel::bottom("outline_style_bottom_panel")
                                .exact_height(220.)
                                .show_inside(ui, |ui| {
                                    self.outline_selection.show_outline_ui(ui, 8, Vec2::new(4.0, 4.0));
                                    let outline_style = self.outline_selection.get_outline_style();
                                    let old_style = self.outline_previewbuffer_view.lock().get_edit_state_mut().get_outline_style();
                                    self.outline_previewbuffer_view.lock().get_edit_state_mut().set_outline_style(outline_style);
                                    if outline_style != old_style {
                                        self.show_selected_char(ui);
                                    }
                                });

                                let opt = icy_engine_gui::TerminalOptions {
                                    stick_to_bottom: false,
                                    scale: Some(Vec2::new(2.0, 2.0)),
                                    monitor_settings: unsafe { SETTINGS.monitor_settings.clone() },
                                    marker_settings: unsafe { SETTINGS.marker_settings.clone() },
                                    id: Some(egui::Id::new(self.id + 20000)),
                                    ..Default::default()
                                };

                            self.outline_previewbuffer_view
                                .lock()
                                .get_caret_mut()
                                .set_is_visible(false);
                            ui.horizontal(|ui|  {
                                ui.label(fl!(crate::LANGUAGE_LOADER, "tdf-editor-outline_preview_label"));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.checkbox(&mut self.draw_outline_bg, fl!(crate::LANGUAGE_LOADER, "tdf-editor-draw_bg_checkbox")).changed() {
                                        self.render_outline_preview();
                                    }
                                });
                            });
                            let (_, _) = show_terminal_area(
                                ui,
                                self.outline_previewbuffer_view.clone(),
                                opt,
                            );
                        });


                    TopBottomPanel::top("cheat_sheet_top_panel")
                    .exact_height(50.)
                    .show_inside(ui, |ui| {
                        if self.opt_cheat_sheet.is_none() {

                            let mut key = fl!(crate::LANGUAGE_LOADER, "tdf-editor-cheat_sheet_key");
                            let mut code = fl!(crate::LANGUAGE_LOADER, "tdf-editor-cheat_sheet_code");
                            let mut res = fl!(crate::LANGUAGE_LOADER, "tdf-editor-cheat_sheet_res");

                             let m = key.len().max(code.len()).max(res.len());
                             let mut buffer = Buffer::new((56 + m, 3));
                             while key.len() < m {
                                key.insert(0, ' ');
                             }
                             while code.len() < m {
                                code.insert(0, ' ');
                             }
                             while res.len() < m {
                                res.insert(0, ' ');
                             }

                            let s  = format!("{key}: F1 F2 F3 F4 F5 F6 F7 F8 F9 F10 1  2  3  4  5  6  7  8 ");
                            let s2 = format!("{code}: A  B  C  D  E  F  G  H  I  J   K  L  M  N  O  @  &  \u{F7} ");
                            let s3 = format!("{res}: \u{CD}  \u{C4}  \u{B3}  \u{BA}  \u{D5}  \u{BB}  \u{D5}  \u{BF}  \u{C8}  \u{BE}   \u{C0}  \u{BD}  \u{B5}  \u{C7}  SP    &  \u{F7}");

                            let mut attr  = TextAttribute::default();
                            attr.set_foreground(0);
                            attr.set_background(4);

                            for (i, c) in s.chars().enumerate() {
                                buffer.layers[0].set_char((i, 0), AttributedChar::new(c, attr));
                            }

                            attr.set_foreground(15);
                            attr.set_background(4);

                            for (i, c) in s2.chars().enumerate() {
                                buffer.layers[0].set_char((i, 1), AttributedChar::new(c, attr));
                            }
                            attr.set_foreground(14);
                            attr.set_background(0);

                            for (i, c) in s3.chars().enumerate() {
                                buffer.layers[0].set_char((i, 2), AttributedChar::new(c, attr));
                            }
                            self.opt_cheat_sheet = Some(crate::create_image(ui.ctx(),&buffer));
                        }

                        if let Some(image) = & self.opt_cheat_sheet {
                            ui.vertical_centered(|ui| {
                                let sized_texture:SizedTexture = (image).into();
                                let image = Image::from_texture(sized_texture);
                                let mut size = sized_texture.size;
                                let width = ui.available_width();
                                if width < size.x {
                                    size.y *= width / size.x;
                                    size.x = width;
                                }
                                let r = Rect::from_min_size(
                                    ui.min_rect().min,
                                    size,
                                );
                                image.paint_at(ui, r);

                            });
                        }
                    });

                    egui::CentralPanel::default().show_inside(ui, |ui| {
                        self.ansi_editor
                            .show_ui(ui, cur_tool, selected_tool, options);
                    });
                } else {
                    self.ansi_editor
                        .show_ui(ui, cur_tool, selected_tool, options);
                }
            }
        });
        let u = self.ansi_editor.buffer_view.lock().get_edit_state().undo_stack_len();
        let attr = self.ansi_editor.buffer_view.lock().get_edit_state().get_caret().get_attribute();
        if self.last_update_preview != u || self.last_update_preview_attr != attr {
            self.last_update_preview = u;
            self.last_update_preview_attr = attr;
            self.save_old_selected_char();
            self.render_outline_preview();
        }

        None
    }

    fn get_ansi_editor_mut(&mut self) -> Option<&mut AnsiEditor> {
        self.ansi_editor.get_ansi_editor_mut()
    }

    fn get_ansi_editor(&self) -> Option<&AnsiEditor> {
        self.ansi_editor.get_ansi_editor()
    }

    fn destroy(&self, gl: &glow::Context) -> Option<Message> {
        self.ansi_editor.destroy(gl);
        None
    }
}

fn set_attribute(layer: &mut Layer, attr: TextAttribute) {
    for y in 0..layer.get_size().height {
        for x in 0..layer.get_size().width {
            let mut c = layer.get_char((x, y));
            if !c.is_visible() {
                continue;
            }
            c.attribute = attr;
            layer.set_char((x, y), c);
        }
    }
}

impl CharFontEditor {
    pub fn new(gl: &Arc<glow::Context>, id: usize, fonts: Vec<TheDrawFont>) -> Self {
        let mut buffer = Buffer::new(Size::new(30, 12));
        set_up_layers(&mut buffer);
        let ansi_editor = AnsiEditor::new(gl, id, buffer);

        let mut buffer = Buffer::new(Size::new(30, 12));
        buffer.is_terminal_buffer = false;
        let mut buffer_view = BufferView::from_buffer(gl, buffer);
        buffer_view.interactive = false;
        let outline_previewbuffer_view = Arc::new(Mutex::new(buffer_view));

        let mut res = Self {
            id,
            font: BitFont::default(),
            ansi_editor,
            selected_char_opt: Some('A'),
            old_selected_char_opt: None,
            fonts,
            selected_font: 0,
            undostack_len: 0,
            outline_previewbuffer_view,
            outline_selection: SelectOutlineDialog::default(),
            last_update_preview: 0,
            opt_cheat_sheet: None,
            draw_outline_bg: true,
            last_update_preview_attr: TextAttribute::default(),
            tool_switch: true,
        };
        res.update_selected_char();
        res
    }

    pub fn show_char_selector(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::Vec2::new(0., 0.);
                for i in b'!'..=b'~' {
                    let ch = unsafe { char::from_u32_unchecked(i as u32) };
                    let mut style = DrawGlyphStyle::Normal;
                    if !self.fonts[self.selected_font].has_char(i) {
                        style = DrawGlyphStyle::GrayOut
                    }
                    if let Some(ch2) = self.selected_char_opt {
                        if ch == ch2 {
                            style = DrawGlyphStyle::Selected
                        }
                    }
                    let response = BitFontEditor::draw_glyph(ui, &self.font, style, ch);
                    if response.clicked() {
                        self.selected_char_opt = Some(ch);
                        self.show_selected_char(ui);
                    }
                }
            });
        });
    }

    fn render_outline_preview(&mut self) {
        let font = &self.fonts[self.selected_font];
        if matches!(font.font_type, icy_engine::FontType::Outline) {
            let lock = &mut self.ansi_editor.buffer_view.lock();
            let mut attr = lock.get_caret().get_attribute();

            let _ = self.outline_previewbuffer_view.lock().get_edit_state_mut().clear_layer(0);
            self.outline_previewbuffer_view.lock().get_caret_mut().set_attr(attr);
            self.outline_previewbuffer_view.lock().get_edit_state_mut().set_is_buffer_dirty();
            if let Some(ch) = self.selected_char_opt {
                let size = self.outline_previewbuffer_view.lock().get_edit_state_mut().get_buffer().get_size();

                if self.draw_outline_bg {
                    attr.set_foreground(8);
                    attr.set_background(0);
                    for y in 0..size.height {
                        for x in 0..size.width {
                            self.outline_previewbuffer_view.lock().get_edit_state_mut().get_buffer_mut().layers[0]
                                .set_char((x, y), AttributedChar::new('\u{B1}', attr));
                        }
                    }
                }

                font.render(self.outline_previewbuffer_view.lock().get_edit_state_mut(), ch as u8);
            }
        }
    }

    fn update_selected_char(&mut self) {
        {
            self.save_old_selected_char();
            let font = &self.fonts[self.selected_font];
            self.ansi_editor.outline_font_mode = matches!(font.font_type, icy_engine::FontType::Outline);
            let lock = &mut self.ansi_editor.buffer_view.lock();

            let edit_state = &mut lock.get_edit_state_mut();
            set_up_layers(edit_state.get_buffer_mut());
            edit_state.set_current_layer(1);
            edit_state.get_caret_mut().set_position((0, 0).into());
            edit_state.set_outline_style(usize::MAX);
            edit_state.set_is_buffer_dirty();
            if let Some(ch) = self.selected_char_opt {
                font.render(edit_state, ch as u8);
            }
            edit_state.get_undo_stack().lock().unwrap().clear();
            self.old_selected_char_opt = self.selected_char_opt;
        }
        self.render_outline_preview();
    }

    fn show_selected_char(&mut self, ui: &mut egui::Ui) {
        self.update_selected_char();
        ui.ctx().request_repaint();
    }

    fn save_old_selected_char(&mut self) {
        if self.ansi_editor.buffer_view.lock().get_edit_state().undo_stack_len() == 0 {
            return;
        }
        self.undostack_len += 1;
        if let Some(font) = self.fonts.get_mut(self.selected_font) {
            if let Some(ch) = self.old_selected_char_opt {
                match font.font_type {
                    icy_engine::FontType::Outline => {
                        let lock = &mut self.ansi_editor.buffer_view.lock();
                        let buf = lock.get_buffer();
                        let mut data = Vec::new();
                        let mut w = 0;
                        let mut h = 0;
                        for y in 0..buf.get_line_count() {
                            if y > 0 {
                                data.push(13);
                            }
                            let lw = buf.get_line_length(y);
                            for x in 0..lw {
                                let ch = buf.get_char((x, y));
                                if VALID_OUTLINE_CHARS.contains(ch.ch) {
                                    data.push(ch.ch as u8);
                                }
                            }
                            w = w.max(lw);
                            h = y;
                        }

                        font.set_glyph(ch, FontGlyph { size: Size::new(w, h), data });
                    }
                    icy_engine::FontType::Block => {
                        let lock = &mut self.ansi_editor.buffer_view.lock();
                        let buf = lock.get_buffer();
                        let mut data = Vec::new();
                        let mut w = 0;
                        let mut h = 0;
                        for y in 0..buf.get_line_count() {
                            if y > 0 {
                                data.push(13);
                            }
                            let lw = buf.get_line_length(y);
                            for x in 0..lw {
                                let ch = buf.get_char((x, y));
                                data.push(ch.ch as u8);
                            }
                            w = w.max(lw);
                            h = y;
                        }

                        font.set_glyph(ch, FontGlyph { size: Size::new(w, h), data });
                    }
                    icy_engine::FontType::Color => {
                        let lock = &mut self.ansi_editor.buffer_view.lock();
                        let buf = lock.get_buffer();
                        let mut data = Vec::new();
                        let mut w = 0;
                        let mut h = 0;
                        for y in 0..buf.get_line_count() {
                            if y > 0 {
                                data.push(13);
                            }
                            let lw = buf.get_line_length(y);
                            for x in 0..lw {
                                let ch = buf.get_char((x, y));
                                data.push(ch.ch as u8);
                                data.push(ch.attribute.as_u8(icy_engine::IceMode::Ice));
                            }
                            w = w.max(lw);
                            h = y;
                        }

                        font.set_glyph(ch, FontGlyph { size: Size::new(w, h), data });
                    }
                }
            }
        }
    }
}

fn set_up_layers(buffer: &mut Buffer) {
    buffer.layers.clear();
    let char_size = Size::new(30, 12);
    let mut new_layer = Layer::new("background", char_size);
    new_layer.properties.has_alpha_channel = false;
    new_layer.properties.is_locked = true;
    new_layer.properties.is_position_locked = true;
    for y in 0..char_size.height {
        for x in 0..char_size.width {
            new_layer.set_char((x, y), AttributedChar::new(' ', TextAttribute::default()));
        }
    }
    buffer.layers.push(new_layer);

    let mut new_layer = Layer::new("edit layer", char_size);
    new_layer.properties.has_alpha_channel = true;
    new_layer.properties.is_position_locked = true;
    for y in 0..char_size.height {
        for x in 0..char_size.width {
            new_layer.set_char((x, y), AttributedChar::new(' ', TextAttribute::default()));
        }
    }
    buffer.layers.push(new_layer);
}
