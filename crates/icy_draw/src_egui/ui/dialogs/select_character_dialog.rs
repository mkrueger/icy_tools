use std::{cell::RefCell, rc::Rc, sync::Arc};

use eframe::{
    egui::{self, RichText},
    epaint::{Color32, Rect, Vec2, mutex::Mutex},
};
use egui::CornerRadius;
use egui_modal::Modal;
use i18n_embed_fl::fl;
use icy_engine_gui::BufferView;

use crate::{AnsiEditor, Message, ModalDialog, TerminalResult};

pub struct SelectCharacterDialog {
    should_commit: bool,
    buf: Arc<Mutex<BufferView>>,
    ch: Rc<RefCell<char>>,
    selected_ch: char,
}

impl SelectCharacterDialog {
    pub fn new(buf: Arc<Mutex<BufferView>>, ch: Rc<RefCell<char>>) -> Self {
        let selected_ch = *ch.borrow();
        SelectCharacterDialog {
            should_commit: false,
            buf,
            ch,
            selected_ch,
        }
    }
}

impl ModalDialog for SelectCharacterDialog {
    fn show(&mut self, ctx: &egui::Context) -> bool {
        let mut result = false;
        let modal = Modal::new(ctx, "select_character_dialog");
        modal.show(|ui| {
            modal.title(ui, fl!(crate::LANGUAGE_LOADER, "select-character-title"));
            let font_page = self.buf.lock().get_caret().font_page();
            let scale = 4.;

            //   ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
            modal.frame(ui, |ui| {
                if let Some(font) = self.buf.lock().get_buffer().font(font_page) {
                    let (_id, stroke_rect) = ui.allocate_space(Vec2::new(scale * font.size.width as f32, scale * font.size.height as f32));
                    let painter = ui.painter_at(stroke_rect);
                    painter.rect_filled(stroke_rect, CornerRadius::ZERO, Color32::BLACK);
                    let s = font.size;

                    let col = Color32::GRAY;
                    let ch = unsafe { char::from_u32_unchecked(self.selected_ch as u32) };
                    if let Some(glyph) = font.glyph(ch) {
                        for y in 0..s.height {
                            for x in 0..s.width {
                                if glyph.data[y as usize] & (128 >> x) != 0 {
                                    painter.rect_filled(
                                        Rect::from_min_size(
                                            egui::Pos2::new(stroke_rect.left() + x as f32 * scale, stroke_rect.top() + y as f32 * scale),
                                            Vec2::new(scale, scale),
                                        ),
                                        CornerRadius::ZERO,
                                        col,
                                    );
                                }
                            }
                        }
                    }
                }
            });

            ui.horizontal(|ui| {
                ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "glyph-char-label")).small());
                ui.label(RichText::new(format!("{0}/0x{0:02X}", self.selected_ch as i32)).small().color(Color32::WHITE));
            });
            let scale = 2.;

            modal.frame(ui, |ui| {
                if let Some(font) = self.buf.lock().get_buffer().font(font_page) {
                    let (_id, stroke_rect) = ui.allocate_space(Vec2::new(scale * font.size.width as f32 * 256. / 8., scale * font.size.height as f32 * 8.));

                    let painter = ui.painter_at(stroke_rect);
                    painter.rect_filled(stroke_rect, CornerRadius::ZERO, Color32::BLACK);
                    let s = font.size;

                    let mut hovered_char = -1;

                    if let Some(hover_pos) = ui.input(|i| i.pointer.hover_pos()) {
                        if stroke_rect.contains(hover_pos) {
                            let char_x = ((hover_pos.x - stroke_rect.left()) / scale / font.size.width as f32) as i32;
                            let char_y = ((hover_pos.y - stroke_rect.top()) / scale / font.size.height as f32) as i32;
                            hovered_char = char_x + 32 * char_y;
                        }
                    }
                    if hovered_char > 0 && ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary)) {
                        self.selected_ch = unsafe { char::from_u32_unchecked(hovered_char as u32) };
                    }

                    if ui.input(|i| i.pointer.button_double_clicked(egui::PointerButton::Primary)) {
                        self.should_commit = true;
                        result = true;
                    }

                    for i in 0..font.length {
                        let is_selected = i == self.selected_ch as i32;
                        let col = if hovered_char >= 0 && i == hovered_char {
                            Color32::WHITE
                        } else if is_selected {
                            Color32::GRAY
                        } else {
                            Color32::DARK_GRAY
                        };

                        let ch = unsafe { char::from_u32_unchecked(i as u32) };
                        let xs = ((i % 32) as f32) * scale * font.size.width as f32;
                        let ys = ((i / 32) as f32) * scale * font.size.height as f32;
                        if let Some(glyph) = font.glyph(ch) {
                            for y in 0..s.height {
                                for x in 0..s.width {
                                    if glyph.data[y as usize] & (128 >> x) != 0 {
                                        painter.rect_filled(
                                            Rect::from_min_size(
                                                egui::Pos2::new(xs + stroke_rect.left() + x as f32 * scale, ys + stroke_rect.top() + y as f32 * scale),
                                                Vec2::new(scale, scale),
                                            ),
                                            CornerRadius::ZERO,
                                            col,
                                        );
                                    }
                                }
                            }
                        }
                    }

                    let xs = ((self.selected_ch as i32 % 32) as f32) * scale * font.size.width as f32;
                    let ys = ((self.selected_ch as i32 / 32) as f32) * scale * font.size.height as f32;

                    let selected_rect = Rect::from_min_size(
                        egui::Pos2::new(stroke_rect.left() + xs, stroke_rect.top() + ys),
                        Vec2::new(scale * font.size.width as f32, scale * font.size.height as f32),
                    );

                    painter.rect(
                        selected_rect,
                        CornerRadius::ZERO,
                        Color32::TRANSPARENT,
                        (2.0, Color32::LIGHT_BLUE),
                        egui::StrokeKind::Outside,
                    );

                    ui.horizontal(|ui| {
                        if hovered_char >= 0 {
                            ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "glyph-char-label")).small());
                            ui.label(RichText::new(format!("{0}/0x{0:02X}", hovered_char)).small().color(Color32::WHITE));
                        } else {
                            ui.label("");
                        }
                    });
                }
            });

            modal.buttons(ui, |ui| {
                if ui.button(fl!(crate::LANGUAGE_LOADER, "new-file-ok")).clicked() {
                    self.should_commit = true;
                    result = true;
                }
                if ui.button(fl!(crate::LANGUAGE_LOADER, "new-file-cancel")).clicked() {
                    result = true;
                }
            });
        });
        modal.open();
        result
    }

    fn should_commit(&self) -> bool {
        self.should_commit
    }

    fn commit(&self, _editor: &mut AnsiEditor) -> TerminalResult<Option<Message>> {
        self.ch.swap(&RefCell::new(self.selected_ch));
        Ok(None)
    }
}
