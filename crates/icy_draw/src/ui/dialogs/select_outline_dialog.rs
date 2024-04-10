use eframe::{
    egui::{self},
    epaint::{Color32, Rect, Rounding, Vec2},
};
use egui_modal::Modal;
use i18n_embed_fl::fl;
use icy_engine::{BitFont, TheDrawFont};

use crate::{AnsiEditor, Message, ModalDialog, Settings, TerminalResult};

pub struct SelectOutlineDialog {
    should_commit: bool,
    selected_outline: usize,
    font: BitFont,
}

impl Default for SelectOutlineDialog {
    fn default() -> Self {
        Self {
            should_commit: false,
            selected_outline: Settings::get_font_outline_style(),
            font: BitFont::default(),
        }
    }
}

impl SelectOutlineDialog {
    pub fn get_outline_style(&self) -> usize {
        self.selected_outline
    }

    fn draw_outline_glyph(&mut self, ui: &mut egui::Ui, outline_style: usize) {
        let scale = 1.;
        let border = 4.0;

        let (_id, stroke_rect) = ui.allocate_space(Vec2::new(
            2. * border + scale * self.font.size.width as f32 * OUTLINE_WIDTH as f32,
            2. * border + scale * self.font.size.height as f32 * OUTLINE_HEIGHT as f32,
        ));

        let painter = ui.painter_at(stroke_rect);
        let s = self.font.size;
        let mut col = if self.selected_outline == outline_style {
            Color32::GRAY
        } else {
            Color32::DARK_GRAY
        };
        let bg_color = if self.selected_outline == outline_style {
            Color32::DARK_BLUE
        } else {
            Color32::BLACK
        };

        if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
            if stroke_rect.contains(pos) {
                if ui.input(|i| i.pointer.primary_clicked()) {
                    self.selected_outline = outline_style;
                }
                if ui.input(|i| i.pointer.button_double_clicked(egui::PointerButton::Primary)) {
                    self.selected_outline = outline_style;
                    self.should_commit = true;
                }
                col = Color32::WHITE;
            }
        }
        painter.rect_filled(stroke_rect, Rounding::ZERO, bg_color);

        for h in 0..OUTLINE_HEIGHT {
            for w in 0..OUTLINE_WIDTH {
                let ch = TheDrawFont::transform_outline(outline_style, OUTLINE_FONT_CHAR[w + h * OUTLINE_WIDTH]);
                let ch = unsafe { char::from_u32_unchecked(ch as u32) };

                let xs = w as f32 * scale * self.font.size.width as f32;
                let ys = h as f32 * scale * self.font.size.height as f32;
                if let Some(glyph) = self.font.get_glyph(ch) {
                    for y in 0..s.height {
                        for x in 0..s.width {
                            if glyph.data[y as usize] & (128 >> x) != 0 {
                                painter.rect_filled(
                                    Rect::from_min_size(
                                        egui::Pos2::new(
                                            border + xs + stroke_rect.left() + x as f32 * scale,
                                            border + ys + stroke_rect.top() + y as f32 * scale,
                                        ),
                                        Vec2::new(scale, scale),
                                    ),
                                    Rounding::ZERO,
                                    col,
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn show_outline_ui(&mut self, ui: &mut egui::Ui, item_per_row: usize, spacing: Vec2) {
        for style in 0..TheDrawFont::OUTLINE_STYLES / item_per_row {
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                for i in 0..item_per_row {
                    self.draw_outline_glyph(ui, style * item_per_row + i);
                    if i < item_per_row - 1 {
                        ui.add_space(spacing.x);
                    }
                }
            });
            ui.end_row();
            ui.add_space(spacing.y);
        }
    }
}

const OUTLINE_WIDTH: usize = 8;
const OUTLINE_HEIGHT: usize = 6;
const OUTLINE_FONT_CHAR: [u8; 48] = [
    69, 65, 65, 65, 65, 65, 65, 70, 67, 79, 71, 66, 66, 72, 79, 68, 67, 79, 73, 65, 65, 74, 79, 68, 67, 79, 71, 66, 66, 72, 79, 68, 67, 79, 68, 64, 64, 67, 79,
    68, 75, 66, 76, 64, 64, 75, 66, 76,
];

impl ModalDialog for SelectOutlineDialog {
    fn show(&mut self, ctx: &egui::Context) -> bool {
        let mut result = false;
        let modal = Modal::new(ctx, "select_outline_dialog");

        modal.show(|ui| {
            modal.title(ui, fl!(crate::LANGUAGE_LOADER, "select-outline-style-title"));

            modal.frame(ui, |ui| {
                ui.add_space(8.0);
                self.show_outline_ui(ui, 4, Vec2::new(8.0, 8.0));
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
        result || self.should_commit
    }

    fn should_commit(&self) -> bool {
        self.should_commit
    }

    fn commit(&self, _editor: &mut AnsiEditor) -> TerminalResult<Option<Message>> {
        Settings::set_font_outline_style(self.selected_outline);
        Ok(None)
    }
}
