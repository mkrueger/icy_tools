mod undo;

use std::{path::Path, sync::Arc};

use eframe::{
    egui::{self, Layout, RichText, Sense},
    emath::Align2,
    epaint::{Color32, FontFamily, FontId, Pos2, Rect, Vec2, mutex::Mutex},
};
use egui::CornerRadius;
use i18n_embed_fl::fl;
use icy_engine::{
    BitFont, Buffer, EngineResult, Glyph, Size, TextAttribute, TextPane,
    util::{BITFONT_GLYPH, pop_data, push_data},
};
use icy_engine_gui::{BufferView, show_terminal_area};

use crate::{AnsiEditor, ClipboardHandler, Document, DocumentOptions, Message, SETTINGS, TerminalResult, UndoHandler, model::Tool, to_message};

use self::undo::UndoOperation;

pub struct BitFontEditor {
    original_font: BitFont,
    last_updated_font: BitFont,
    font: BitFont,

    width: i32,
    height: i32,

    buffer_view: Arc<Mutex<BufferView>>,

    selected_char_opt: Option<char>,
    undo_stack: Arc<Mutex<Vec<Box<dyn UndoOperation>>>>,
    redo_stack: Vec<Box<dyn UndoOperation>>,
    old_data: Option<Vec<u32>>,

    send_update_message: bool,
    tool_switch: bool,
}

pub enum DrawGlyphStyle {
    Normal,
    Selected,
    GrayOut,
}

impl BitFontEditor {
    pub fn new(gl: &Arc<glow::Context>, font: BitFont) -> Self {
        let mut buffer = Buffer::new(Size::new(10, 10));
        buffer.is_terminal_buffer = false;
        let mut buffer_view = BufferView::from_buffer(gl, buffer);
        buffer_view.interactive = false;
        let buffer_view = Arc::new(Mutex::new(buffer_view));
        let size = font.size;
        let last_updated_font = font.clone();
        Self {
            buffer_view,
            original_font: font.clone(),
            last_updated_font,
            font,
            width: size.width,
            height: size.height,
            selected_char_opt: Some('A'),
            undo_stack: Arc::new(Mutex::new(Vec::new())),
            redo_stack: Vec::new(),
            old_data: None,
            send_update_message: false,
            tool_switch: true,
        }
    }

    pub fn draw_glyph(ui: &mut egui::Ui, font: &BitFont, style: DrawGlyphStyle, ch: char) -> egui::Response {
        let scale = 3.;
        let (id, stroke_rect) = ui.allocate_space(Vec2::new(scale * font.size.width as f32, scale * font.size.height as f32));
        let response = ui.interact(stroke_rect, id, Sense::click());
        let col = if response.hovered() {
            match style {
                DrawGlyphStyle::Normal => Color32::LIGHT_GRAY,
                DrawGlyphStyle::Selected => Color32::WHITE,
                DrawGlyphStyle::GrayOut => Color32::GRAY,
            }
        } else {
            match style {
                DrawGlyphStyle::Normal => Color32::GRAY,
                DrawGlyphStyle::Selected => Color32::YELLOW,
                DrawGlyphStyle::GrayOut => Color32::DARK_GRAY,
            }
        };

        let painter = ui.painter_at(stroke_rect);
        painter.rect_filled(stroke_rect, CornerRadius::ZERO, Color32::BLACK);
        let s = font.size;
        if let Some(glyph) = font.get_glyph(ch) {
            for y in 0..s.height {
                for x in 0..s.width {
                    if glyph.data[y as usize] & (128 >> x) != 0 {
                        painter.rect_filled(
                            Rect::from_min_size(
                                Pos2::new(stroke_rect.left() + x as f32 * scale, stroke_rect.top() + y as f32 * scale),
                                Vec2::new(scale, scale),
                            ),
                            CornerRadius::ZERO,
                            col,
                        );
                    }
                }
            }
        }
        response
    }

    pub fn update_tile_area(&mut self) {
        let lock = &mut self.buffer_view.lock();
        let buf = lock.get_buffer_mut();
        buf.set_font(0, self.font.clone());

        let ch = self.selected_char_opt.unwrap_or(' ');
        for y in 0..buf.get_width() {
            for x in 0..buf.get_height() {
                buf.layers[0].set_char(
                    (x, y),
                    icy_engine::AttributedChar {
                        ch,
                        attribute: TextAttribute::default(),
                    },
                );
            }
        }
        self.send_update_message = true;
        lock.redraw_view();
    }

    pub fn edit_glyph(&mut self) -> impl egui::Widget + '_ {
        move |ui: &mut egui::Ui| {
            let scale = 20.;
            let border = 2.;

            let left_ruler = 20.0;
            let top_ruler = 20.0;

            let (id, stroke_rect) = ui.allocate_space(Vec2::new(
                1. + (border + scale) * self.font.size.width as f32 + left_ruler,
                1. + (border + scale) * self.font.size.height as f32 + top_ruler,
            ));
            let mut response = ui.interact(stroke_rect, id, Sense::click_and_drag());

            let painter = ui.painter_at(stroke_rect);
            painter.rect_filled(stroke_rect, CornerRadius::ZERO, Color32::DARK_GRAY);

            let s = self.font.size;

            /*   if response.clicked() {
                if let Some(pos) = response.hover_pos() {
                    if let Some(number) = self.selected_char_opt {
                        if let Some(glyph) = self.font.get_glyph_mut(number) {
                            let y = ((pos.y - stroke_rect.top()) / (scale + border)) as usize;
                            let x = ((pos.x - stroke_rect.left()) / (scale + border)) as usize;
                            if glyph.data[y] & (128 >> x) != 0 {
                                glyph.data[y] &= !(128 >> x);
                            } else {
                                glyph.data[y] |= 128 >> x;
                            }
                            self.is_dirty = true;
                            response.mark_changed();
                        }
                    }
                }
            } else { */
            if response.drag_started_by(egui::PointerButton::Primary) || response.drag_started_by(egui::PointerButton::Secondary) {
                self.start_edit();
            }

            if response.drag_stopped_by(egui::PointerButton::Primary) || response.drag_stopped_by(egui::PointerButton::Secondary) {
                self.end_edit();
            }

            if response.dragged_by(egui::PointerButton::Primary) {
                if let Some(pos) = response.hover_pos() {
                    if let Some(number) = self.selected_char_opt {
                        if let Some(glyph) = self.font.get_glyph_mut(number) {
                            let y = ((pos.y - left_ruler - stroke_rect.top()) / (scale + border)) as usize;
                            let x = ((pos.x - top_ruler - stroke_rect.left()) / (scale + border)) as usize;
                            if y < glyph.data.len() && x < 8 {
                                glyph.data[y] |= 128 >> x;
                                self.update_tile_area();
                                response.mark_changed();
                            }
                        }
                    }
                }
            }

            if response.dragged_by(egui::PointerButton::Secondary) {
                if let Some(pos) = response.hover_pos() {
                    if let Some(number) = self.selected_char_opt {
                        if let Some(glyph) = self.font.get_glyph_mut(number) {
                            let y = ((pos.y - left_ruler - stroke_rect.top()) / (scale + border)) as usize;
                            let x = ((pos.x - top_ruler - stroke_rect.left()) / (scale + border)) as usize;
                            if y < glyph.data.len() && x < 8 {
                                glyph.data[y] &= !(128 >> x);
                                self.update_tile_area();
                                response.mark_changed();
                            }
                        }
                    }
                }
            }
            if let Some(number) = self.selected_char_opt {
                if let Some(glyph) = self.font.get_glyph_mut(number) {
                    painter.rect_filled(
                        Rect::from_min_size(
                            Pos2::new(stroke_rect.left(), stroke_rect.top()),
                            Vec2::new(
                                2. + left_ruler + s.width as f32 * (border + scale),
                                2. + top_ruler + s.height as f32 * (border + scale),
                            ),
                        ),
                        CornerRadius::ZERO,
                        ui.style().visuals.extreme_bg_color,
                    );

                    for x in 0..s.width {
                        let pos = Pos2::new(
                            2. + left_ruler + stroke_rect.left() + (x as f32 + 0.5) * (border + scale),
                            2. + top_ruler / 2. + stroke_rect.top(),
                        );
                        let col = if let Some(pos) = response.hover_pos() {
                            if x == ((pos.x - (2. + left_ruler + stroke_rect.left())) / (border + scale)) as i32 {
                                ui.style().visuals.strong_text_color()
                            } else {
                                ui.style().visuals.text_color()
                            }
                        } else {
                            ui.style().visuals.text_color()
                        };

                        painter.text(
                            pos,
                            Align2::CENTER_CENTER,
                            (x + 1).to_string(),
                            FontId::new(12.0, FontFamily::Proportional),
                            col,
                        );
                    }
                    for y in 0..s.height {
                        let pos = Pos2::new(
                            2. + left_ruler / 2. + stroke_rect.left(),
                            2. + top_ruler + stroke_rect.top() + (y as f32 + 0.5) * (border + scale),
                        );
                        let col = if let Some(pos) = response.hover_pos() {
                            if y == ((pos.y - (2. + top_ruler + stroke_rect.top())) / (border + scale)) as i32 {
                                ui.style().visuals.strong_text_color()
                            } else {
                                ui.style().visuals.text_color()
                            }
                        } else {
                            ui.style().visuals.text_color()
                        };

                        painter.text(
                            pos,
                            Align2::CENTER_CENTER,
                            (y + 1).to_string(),
                            FontId::new(12.0, FontFamily::Proportional),
                            col,
                        );
                    }

                    for y in 0..s.height {
                        for x in 0..s.width {
                            let rect = Rect::from_min_size(
                                Pos2::new(
                                    2. + left_ruler + stroke_rect.left() + x as f32 * (border + scale),
                                    2. + top_ruler + stroke_rect.top() + y as f32 * (border + scale),
                                ),
                                Vec2::new(scale, scale),
                            );
                            let col = if glyph.data[y as usize] & (128 >> x) != 0 {
                                if let Some(pos) = response.hover_pos() {
                                    if rect.contains(pos) { Color32::WHITE } else { Color32::GRAY }
                                } else {
                                    Color32::GRAY
                                }
                            } else if let Some(pos) = response.hover_pos() {
                                if rect.contains(pos) { Color32::DARK_GRAY } else { Color32::BLACK }
                            } else {
                                Color32::BLACK
                            };
                            painter.rect_filled(rect, CornerRadius::ZERO, col);
                        }
                    }
                }
            }
            response
        }
    }

    fn push_undo(&mut self, mut op: Box<dyn UndoOperation>) -> EngineResult<()> {
        op.redo(self)?;
        self.undo_stack.lock().push(op);
        self.redo_stack.clear();
        self.update_tile_area();
        Ok(())
    }

    fn clear_selected_glyph(&mut self) -> EngineResult<()> {
        if let Some(number) = self.selected_char_opt {
            let op = undo::ClearGlyph::new(number);
            self.push_undo(Box::new(op))?;
        }
        Ok(())
    }

    fn inverse_selected_glyph(&mut self) -> EngineResult<()> {
        if let Some(number) = self.selected_char_opt {
            let op = undo::InverseGlyph::new(number);
            self.push_undo(Box::new(op))?;
        }
        Ok(())
    }

    fn left_selected_glyph(&mut self) -> EngineResult<()> {
        if let Some(number) = self.selected_char_opt {
            let op = undo::LeftGlyph::new(number);
            self.push_undo(Box::new(op))?;
        }
        Ok(())
    }

    fn right_selected_glyph(&mut self) -> EngineResult<()> {
        if let Some(number) = self.selected_char_opt {
            let op = undo::RightGlyph::new(number);
            self.push_undo(Box::new(op))?;
        }
        Ok(())
    }

    fn up_selected_glyph(&mut self) -> EngineResult<()> {
        if let Some(number) = self.selected_char_opt {
            let op = undo::UpGlyph::new(number);
            self.push_undo(Box::new(op))?;
        }
        Ok(())
    }

    fn down_selected_glyph(&mut self) -> EngineResult<()> {
        if let Some(number) = self.selected_char_opt {
            let op = undo::DownGlyph::new(number);
            self.push_undo(Box::new(op))?;
        }
        Ok(())
    }

    fn flip_x_selected_glyph(&mut self) -> EngineResult<()> {
        if let Some(number) = self.selected_char_opt {
            let op = undo::FlipX::new(number);
            self.push_undo(Box::new(op))?;
        }
        Ok(())
    }

    fn flip_y_selected_glyph(&mut self) -> EngineResult<()> {
        if let Some(number) = self.selected_char_opt {
            let op = undo::FlipY::new(number);
            self.push_undo(Box::new(op))?;
        }
        Ok(())
    }

    fn resize_font(&mut self) -> EngineResult<()> {
        let old_font = self.font.clone();
        let mut new_font = self.font.clone();

        for glyph in new_font.glyphs.values_mut() {
            glyph.data.resize(self.height as usize, 0);
        }
        new_font.size = Size::new(self.width, self.height);

        let op = undo::ResizeFont::new(old_font, new_font);
        self.push_undo(Box::new(op))?;
        Ok(())
    }

    fn start_edit(&mut self) {
        if let Some(number) = self.selected_char_opt {
            if let Some(glyph) = self.font.get_glyph_mut(number) {
                self.old_data = Some(glyph.data.clone());
            }
        }
    }

    fn end_edit(&mut self) {
        if self.old_data.is_none() {
            return;
        }
        if let Some(number) = self.selected_char_opt {
            if let Some(glyph) = self.font.get_glyph(number) {
                if let Some(old_data) = self.old_data.take() {
                    let op = undo::Edit::new(number, glyph.data.clone(), old_data);
                    self.undo_stack.lock().push(Box::new(op));
                    self.redo_stack.clear();
                } else {
                    log::error!("no old_data for BitFontEditor:end_edit");
                }
            }
        }
    }
}

impl ClipboardHandler for BitFontEditor {
    fn can_copy(&self) -> bool {
        self.selected_char_opt.is_some()
    }

    fn copy(&mut self, _ctx: &egui::Context) -> EngineResult<()> {
        if let Some(ch) = self.selected_char_opt {
            if let Some(data) = self.font.get_clipboard_data(ch) {
                push_data(BITFONT_GLYPH, &data, None)?;
            }
        }
        Ok(())
    }

    fn can_paste(&self) -> bool {
        if self.selected_char_opt.is_none() {
            return false;
        }

        pop_data(BITFONT_GLYPH).is_some()
    }

    fn paste(&mut self, _ctx: &egui::Context) -> EngineResult<()> {
        if let Some(data) = pop_data(BITFONT_GLYPH) {
            let (_, g) = Glyph::from_clipbard_data(&data);
            if let Some(ch) = self.selected_char_opt {
                let op = undo::Paste::new(ch, g);
                self.push_undo(Box::new(op))?;
            }
        }
        Ok(())
    }
}

impl UndoHandler for BitFontEditor {
    fn undo_description(&self) -> Option<String> {
        self.undo_stack.lock().last().map(|op| op.get_description())
    }

    fn can_undo(&self) -> bool {
        !self.undo_stack.lock().is_empty()
    }

    fn undo(&mut self) -> EngineResult<Option<Message>> {
        let Some(mut op) = self.undo_stack.lock().pop() else {
            return Ok(None);
        };

        op.undo(self)?;
        self.redo_stack.push(op);
        self.update_tile_area();
        self.send_update_message = true;
        Ok(None)
    }

    fn redo_description(&self) -> Option<String> {
        self.redo_stack.last().map(|op| op.get_description())
    }

    fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    fn redo(&mut self) -> EngineResult<Option<Message>> {
        if let Some(mut op) = self.redo_stack.pop() {
            op.redo(self)?;
            self.undo_stack.lock().push(op);
            return Ok(None);
        }
        self.update_tile_area();
        self.send_update_message = true;
        Ok(None)
    }
}

impl Document for BitFontEditor {
    fn default_extension(&self) -> &'static str {
        "psf"
    }

    fn has_tool_switched(&self) -> bool {
        self.tool_switch
    }
    fn set_tool_switch(&mut self, switched: bool) {
        self.tool_switch = switched;
    }

    fn undo_stack_len(&self) -> usize {
        self.undo_stack.lock().len()
    }

    fn show_ui(&mut self, ui: &mut eframe::egui::Ui, _cur_tool: &mut Box<dyn Tool>, _selected_tool: usize, _options: &DocumentOptions) -> Option<Message> {
        let mut message = None;
        ui.add_space(16.);
        ui.vertical_centered(|ui| {
            ui.horizontal(|ui| {
                ui.add_space(120.);
                ui.add(self.edit_glyph());

                ui.vertical(|ui| {
                    ui.add_space(20.);
                    ui.horizontal(|ui| {
                        if ui.button(fl!(crate::LANGUAGE_LOADER, "font-editor-clear")).clicked() {
                            message = to_message(self.clear_selected_glyph());
                        }
                        if ui.button(fl!(crate::LANGUAGE_LOADER, "font-editor-inverse")).clicked() {
                            message = to_message(self.inverse_selected_glyph());
                        }
                    });
                    ui.add_space(8.);
                    ui.horizontal(|ui| {
                        ui.add_space(14.);

                        if ui.button("⬆").clicked() {
                            message = to_message(self.up_selected_glyph());
                        }
                    });

                    ui.horizontal(|ui| {
                        if ui.button("⬅").clicked() {
                            message = to_message(self.left_selected_glyph());
                        }

                        if ui.button("➡").clicked() {
                            message = to_message(self.right_selected_glyph());
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.add_space(14.);

                        if ui.button("⬇").clicked() {
                            message = to_message(self.down_selected_glyph());
                        }
                    });
                    ui.add_space(8.);

                    ui.horizontal(|ui| {
                        if ui.button(fl!(crate::LANGUAGE_LOADER, "font-editor-flip_x")).clicked() {
                            message = to_message(self.flip_x_selected_glyph());
                        }

                        if ui.button(fl!(crate::LANGUAGE_LOADER, "font-editor-flip_y")).clicked() {
                            message = to_message(self.flip_y_selected_glyph());
                        }
                    });

                    egui::Grid::new("some_unique_id").num_columns(2).spacing([4.0, 8.0]).show(ui, |ui| {
                        ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(fl!(crate::LANGUAGE_LOADER, "new-file-width"));
                        });
                        ui.add(egui::Slider::new(&mut self.width, 2..=8));
                        ui.end_row();

                        ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(fl!(crate::LANGUAGE_LOADER, "new-file-height"));
                        });
                        ui.add(egui::Slider::new(&mut self.height, 2..=19));
                        ui.end_row();
                    });

                    if (self.width != self.font.size.width || self.height != self.font.size.height) && ui.button("Resize").clicked() {
                        message = to_message(self.resize_font());
                    }
                });

                ui.vertical(|ui| {
                    ui.heading(fl!(crate::LANGUAGE_LOADER, "font-editor-tile_area"));
                    let mut scale = unsafe { SETTINGS.get_scale() };
                    if self.buffer_view.lock().get_buffer().use_aspect_ratio() {
                        scale.y *= 1.35;
                    }
                    let opt = icy_engine_gui::TerminalOptions {
                        stick_to_bottom: false,
                        scale: Some(Vec2::new(2.0, 2.0)),
                        monitor_settings: unsafe { SETTINGS.monitor_settings.clone() },
                        marker_settings: unsafe { SETTINGS.marker_settings.clone() },
                        ..Default::default()
                    };
                    self.buffer_view.lock().get_caret_mut().set_is_visible(false);
                    let (_, _) = show_terminal_area(ui, self.buffer_view.clone(), opt);
                });
            });
        });

        ui.label(fl!(crate::LANGUAGE_LOADER, "font-editor-table", length = (self.font.length - 1).to_string()));
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for i in 0..self.font.length {
                    let ch = unsafe { char::from_u32_unchecked(i as u32) };
                    let mut style = DrawGlyphStyle::Normal;
                    if let Some(ch2) = self.selected_char_opt {
                        if ch == ch2 {
                            style = DrawGlyphStyle::Selected
                        }
                    }
                    let response = BitFontEditor::draw_glyph(ui, &self.font, style, ch);
                    if response.clicked() {
                        self.selected_char_opt = Some(ch);
                        self.update_tile_area();
                    }

                    response.on_hover_ui(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "font-view-char_label")).small());
                            ui.label(RichText::new(format!("{0}/0x{0:02X}", i)).small().color(Color32::WHITE));
                        });
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "font-view-ascii_label")).small());
                            ui.label(
                                RichText::new(format!("'{0}'", unsafe { char::from_u32_unchecked(i as u32) }))
                                    .small()
                                    .color(Color32::WHITE),
                            );
                        });
                    });
                }
            })
        });

        if message.is_none() && self.send_update_message {
            message = Some(Message::UpdateFont(Box::new((self.last_updated_font.clone(), self.font.clone()))));
            self.last_updated_font = self.font.clone();
            self.send_update_message = false;
        }

        message
    }

    fn get_bytes(&mut self, _path: &Path) -> TerminalResult<Vec<u8>> {
        self.font.to_psf2_bytes()
    }

    fn get_ansi_editor_mut(&mut self) -> Option<&mut AnsiEditor> {
        None
    }

    fn get_ansi_editor(&self) -> Option<&AnsiEditor> {
        None
    }

    fn inform_save(&mut self) {
        self.original_font = self.font.clone();
    }

    fn destroy(&self, gl: &glow::Context) -> Option<Message> {
        self.buffer_view.lock().destroy(gl);
        Some(Message::UpdateFont(Box::new((self.last_updated_font.clone(), self.original_font.clone()))))
    }
}
