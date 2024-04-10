use std::{fs, sync::Arc};

use eframe::{
    egui::{self, Response, Sense, TextEdit, TextStyle, WidgetText},
    epaint::{ahash::HashMap, ColorImage, FontFamily, FontId, Pos2, Rect, Rounding, Stroke, Vec2},
};
use egui::{load::SizedTexture, mutex::Mutex, Image, TextureHandle, TextureOptions};
use egui_file::FileDialog;
use egui_modal::Modal;
use i18n_embed_fl::fl;
use icy_engine::{editor::EditState, Buffer, Rectangle, Size, TextPane, TheDrawFont};

use crate::{MainWindow, Message};

pub struct SelectFontDialog {
    fonts: Arc<Mutex<Vec<TheDrawFont>>>,
    selected_font_arc: Arc<Mutex<i32>>,
    selected_font: i32,
    pub do_select: bool,
    filter: String,
    show_outline: bool,
    show_color: bool,
    show_block: bool,

    export_data: Option<Vec<u8>>,
    export_dialog: Option<FileDialog>,

    image_cache: HashMap<usize, TextureHandle>,
}

impl SelectFontDialog {
    pub fn new(fonts: Arc<Mutex<Vec<TheDrawFont>>>, selected_font_arc: Arc<Mutex<i32>>) -> Self {
        let selected_font = *selected_font_arc.lock();

        Self {
            do_select: false,
            fonts,
            selected_font_arc,
            selected_font,
            filter: String::new(),
            show_outline: true,
            show_color: true,
            show_block: true,
            image_cache: HashMap::default(),
            export_dialog: None,
            export_data: None,
        }
    }

    pub fn draw_font_row(&mut self, ui: &mut egui::Ui, cur_font: usize, row_height: f32, is_selected: bool) -> Response {
        let font = &self.fonts.lock()[cur_font];
        let (id, rect) = ui.allocate_space([ui.available_width(), row_height].into());
        let response = ui.interact(rect, id, Sense::click());
        if response.hovered() {
            ui.painter()
                .rect_filled(rect.expand(1.0), Rounding::same(4.0), ui.style().visuals.widgets.active.bg_fill);
        } else if is_selected {
            ui.painter()
                .rect_filled(rect.expand(1.0), Rounding::same(4.0), ui.style().visuals.extreme_bg_color);
        }

        let text_color = if is_selected {
            ui.style().visuals.strong_text_color()
        } else {
            ui.style().visuals.text_color()
        };

        let font_id = TextStyle::Button.resolve(ui.style());
        let text: WidgetText = font.name.clone().into();
        let galley = text.into_galley(ui, Some(false), f32::INFINITY, font_id);
        ui.painter().galley_with_override_text_color(
            egui::Align2::LEFT_TOP.align_size_within_rect(galley.size(), rect.shrink(4.0)).min,
            galley,
            text_color,
        );

        let mut x = 0.;
        let mut y = 26.;
        let mut cnt = 0;

        for ch in '!'..='~' {
            let color = if font.has_char(ch as u8) {
                ui.style().visuals.strong_text_color()
            } else {
                ui.style().visuals.text_color()
            };
            let text: WidgetText = ch.to_string().into();
            let font_id = FontId::new(14.0, FontFamily::Monospace);
            let galley = text.into_galley(ui, Some(false), f32::INFINITY, font_id);

            let mut rect = rect.shrink(4.0);
            rect.set_top(rect.top() + y);
            rect.set_left(rect.left() + x);
            x += galley.size().x;
            cnt += 1;
            if cnt > 31 {
                y += galley.size().y;
                x = 0.;
                cnt = 0;
            }
            ui.painter()
                .galley_with_override_text_color(egui::Align2::LEFT_TOP.align_size_within_rect(galley.size(), rect).min, galley, color);
        }

        #[allow(clippy::map_entry)]
        if !self.image_cache.contains_key(&cur_font) {
            let buffer = Buffer::new((100, 12));
            let mut state = EditState::from_buffer(buffer);

            let text = fl!(crate::LANGUAGE_LOADER, "select-font-dialog-preview-text");
            let lowercase = text.to_ascii_lowercase();

            let b = if font.has_char(text.chars().next().unwrap() as u8) {
                text.bytes()
            } else {
                lowercase.bytes()
            };
            for ch in b {
                let opt_size: Option<Size> = font.render(&mut state, ch);
                if let Some(size) = opt_size {
                    let mut pos = state.get_caret().get_position();
                    pos.x += size.width + font.spaces;
                    state.get_caret_mut().set_position(pos);
                }
            }
            let img = create_image(ui.ctx(), state.get_buffer());
            self.image_cache.insert(cur_font, img);
        }

        if let Some(image) = self.image_cache.get(&cur_font) {
            let sized_texture: SizedTexture = image.into();
            let w = (sized_texture.size.x / 2.0).floor();
            let h = (sized_texture.size.y / 2.0).floor();
            let r = Rect::from_min_size(
                Pos2::new((rect.right() - w - 4.0).floor(), (rect.top() + ((rect.height() - h) / 2.0)).floor()),
                Vec2::new(w, h),
            );
            let image = Image::from_texture(sized_texture);
            image.paint_at(ui, r);
            /*
            ui.painter().image(
                image.texture_id(ui.ctx()),
                r,
                Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );*/

            let font_type = match font.font_type {
                icy_engine::FontType::Outline => {
                    fl!(crate::LANGUAGE_LOADER, "select-font-dialog-outline-font")
                }
                icy_engine::FontType::Block => {
                    fl!(crate::LANGUAGE_LOADER, "select-font-dialog-block-font")
                }
                icy_engine::FontType::Color => {
                    fl!(crate::LANGUAGE_LOADER, "select-font-dialog-color-font")
                }
            };

            let font_id = FontId::new(12.0, FontFamily::Proportional);
            let text: WidgetText = font_type.into();
            let galley = text.into_galley(ui, Some(false), f32::INFINITY, font_id);

            let rect = Rect::from_min_size(
                Pos2::new((rect.right() - galley.size().x - 10.0).floor(), (rect.top() + 8.0).floor()),
                galley.size(),
            );

            ui.painter()
                .rect_filled(rect.expand(2.0), Rounding::same(4.0), ui.style().visuals.widgets.active.bg_fill);

            ui.painter().rect_stroke(rect.expand(2.0), 4.0, Stroke::new(1.0, text_color));

            ui.painter()
                .galley_with_override_text_color(egui::Align2::CENTER_CENTER.align_size_within_rect(galley.size(), rect).min, galley, text_color);
        }

        response
    }
}

impl crate::ModalDialog for SelectFontDialog {
    fn show(&mut self, ctx: &egui::Context) -> bool {
        if let Some(ed) = &mut self.export_dialog {
            if ed.show(ctx).selected() {
                if let Some(res) = ed.path() {
                    let mut res = res.to_path_buf();
                    res.set_extension("tdf");
                    if let Some(data) = self.export_data.take() {
                        if let Err(err) = fs::write(res, data) {
                            log::error!("Failed to write font: {}", err);
                        }
                    } else {
                        log::error!("Export data == None");
                    }
                }
                self.export_dialog = None
            } else {
                return false;
            }
        }

        let mut result = false;
        let modal = Modal::new(ctx, "select_font_dialog2");
        let font_count = self.fonts.lock().len();
        modal.show(|ui| {
            ui.set_width(700.);
            modal.title(ui, fl!(crate::LANGUAGE_LOADER, "select-font-dialog-title", fontcount = font_count));
            modal.frame(ui, |ui| {
                let row_height = 200.0 / 2.0;
                ui.horizontal(|ui: &mut egui::Ui| {
                    ui.add_sized(
                        [250.0, 20.0],
                        TextEdit::singleline(&mut self.filter).hint_text(fl!(crate::LANGUAGE_LOADER, "select-font-dialog-filter-text")),
                    );
                    let response = ui.button("🗙");
                    if response.clicked() {
                        self.filter.clear();
                    }

                    let response = ui.selectable_label(self.show_color, fl!(crate::LANGUAGE_LOADER, "select-font-dialog-color-font"));
                    if response.clicked() {
                        self.show_color = !self.show_color;
                    }

                    let response = ui.selectable_label(self.show_block, fl!(crate::LANGUAGE_LOADER, "select-font-dialog-block-font"));
                    if response.clicked() {
                        self.show_block = !self.show_block;
                    }

                    let response = ui.selectable_label(self.show_outline, fl!(crate::LANGUAGE_LOADER, "select-font-dialog-outline-font"));
                    if response.clicked() {
                        self.show_outline = !self.show_outline;
                    }
                });
                ui.add_space(4.0);

                let mut filtered_fonts = Vec::new();

                for i in 0..font_count {
                    let font = &self.fonts.lock()[i];
                    if font.name.to_lowercase().contains(&self.filter.to_lowercase())
                        && (self.show_outline && matches!(font.font_type, icy_engine::FontType::Outline)
                            || self.show_block && matches!(font.font_type, icy_engine::FontType::Block)
                            || self.show_color && matches!(font.font_type, icy_engine::FontType::Color))
                    {
                        filtered_fonts.push(i);
                    }
                }
                if filtered_fonts.is_empty() {
                    if font_count == 0 {
                        ui.label(fl!(crate::LANGUAGE_LOADER, "select-font-dialog-no-fonts-installed"));
                    } else {
                        ui.label(fl!(crate::LANGUAGE_LOADER, "select-font-dialog-no-fonts"));
                    }
                } else {
                    egui::ScrollArea::vertical()
                        .max_height(300.)
                        .show_rows(ui, row_height, filtered_fonts.len(), |ui, range| {
                            for row in range {
                                let is_selected = self.selected_font == filtered_fonts[row] as i32;
                                let response = self.draw_font_row(ui, filtered_fonts[row], row_height, is_selected);

                                if response.clicked() {
                                    self.selected_font = filtered_fonts[row] as i32;
                                }
                                if response.double_clicked() {
                                    self.selected_font = filtered_fonts[row] as i32;
                                    self.do_select = true;
                                    result = true;
                                }
                            }
                        });
                }
            });

            modal.buttons(ui, |ui| {
                if ui.button(fl!(crate::LANGUAGE_LOADER, "select-font-dialog-select")).clicked() {
                    self.do_select = true;
                    result = true;
                }
                if ui.button(fl!(crate::LANGUAGE_LOADER, "new-file-cancel")).clicked() {
                    result = true;
                }

                if ui.button(fl!(crate::LANGUAGE_LOADER, "export-button-title")).clicked() {
                    match self.fonts.lock()[self.selected_font as usize].as_tdf_bytes() {
                        Ok(data) => {
                            let mut initial_path = None;
                            crate::set_default_initial_directory_opt(&mut initial_path);
                            let mut dialog = FileDialog::save_file(initial_path);
                            dialog.open();
                            self.export_data = Some(data);
                            self.export_dialog = Some(dialog);
                        }
                        Err(err) => {
                            log::error!("Failed to export font: {}", err);
                        }
                    }
                }
            });
        });
        modal.open();
        result
    }

    fn should_commit(&self) -> bool {
        self.do_select
    }

    fn commit_self(&self, _window: &mut MainWindow<'_>) -> crate::TerminalResult<Option<Message>> {
        *self.selected_font_arc.lock() = self.selected_font;
        Ok(None)
    }
}

pub fn create_image(ctx: &egui::Context, buf: &Buffer) -> TextureHandle {
    let (size, pixels) = buf.render_to_rgba(Rectangle::from(0, 0, buf.get_width(), buf.get_height()));
    let color_image: ColorImage = ColorImage::from_rgba_premultiplied([size.width as usize, size.height as usize], &pixels);
    ctx.load_texture("my_texture", color_image, TextureOptions::NEAREST)
}
