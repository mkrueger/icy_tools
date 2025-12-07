use std::{fs, io::Read, path::Path};

use eframe::{
    egui::{self, Button, Response, Sense, TextEdit, WidgetText},
    epaint::{Color32, FontFamily, FontId, Pos2, Rect, Stroke, Vec2, ahash::HashMap},
};
use egui::{CornerRadius, Image, TextureHandle, load::SizedTexture};
use egui_modal::Modal;
use i18n_embed_fl::fl;
use icy_engine::{ANSI_FONTS, AttributedChar, BitFont, Buffer, SAUCE_FONT_NAMES, TextAttribute};
use walkdir::WalkDir;

use crate::{AnsiEditor, Message, Settings, TerminalResult, create_image, is_font_extensions};

#[derive(Default)]
struct BitfontSource {
    pub ansi_slot: Option<usize>,
    pub sauce: Option<String>,
    pub file_slot: Option<usize>,
    pub library: bool,
}

pub struct FontSelector {
    fonts: Vec<(BitFont, BitfontSource)>,
    selected_font: i32,

    filter: String,
    show_builtin: bool,
    show_library: bool,
    show_file: bool,
    has_file: bool,
    show_sauce: bool,
    should_add: bool,

    image_cache: HashMap<usize, TextureHandle>,
    do_select: bool,
    edit_selected_font: bool,
    only_sauce_fonts: bool,
}

impl FontSelector {
    pub fn new(editor: &AnsiEditor, should_add: bool) -> Self {
        let mut fonts = Vec::new();
        for f in SAUCE_FONT_NAMES {
            fonts.push((
                BitFont::from_sauce_name(f).unwrap(),
                BitfontSource {
                    sauce: Some(f.to_string()),
                    ..Default::default()
                },
            ));
        }

        let only_sauce_fonts = matches!(editor.buffer_view.lock().get_buffer().font_mode, icy_engine::FontMode::Sauce);

        if !only_sauce_fonts {
            for slot in 0..ANSI_FONTS {
                let ansi_font = BitFont::from_ansi_font_page(slot).unwrap();
                let mut found = false;
                for (existing_font, src) in &mut fonts {
                    if existing_font.get_checksum() == ansi_font.get_checksum() {
                        src.ansi_slot = Some(slot);
                        found = true;
                        break;
                    }
                }
                if found {
                    continue;
                }
                fonts.push((
                    ansi_font,
                    BitfontSource {
                        ansi_slot: Some(slot),
                        ..Default::default()
                    },
                ));
            }

            if let Ok(font_dir) = Settings::get_font_diretory() {
                for lib_font in FontSelector::load_fonts(font_dir.as_path()) {
                    let mut found = false;
                    for (existing_font, src) in &mut fonts {
                        if existing_font.get_checksum() == lib_font.get_checksum() {
                            src.library = true;
                            found = true;
                            break;
                        }
                    }
                    if found {
                        continue;
                    }

                    fonts.push((
                        lib_font,
                        BitfontSource {
                            library: true,
                            ..Default::default()
                        },
                    ));
                }
            }
        }

        let mut selected_font = 0;
        let cur_font = editor.buffer_view.lock().get_caret().get_font_page();

        for (id, file_font) in editor.buffer_view.lock().get_buffer().font_iter() {
            let mut found = false;
            for (index, (existing_font, src)) in fonts.iter_mut().enumerate() {
                if existing_font.get_checksum() == file_font.get_checksum() {
                    src.file_slot = Some(*id);
                    found = true;
                    if *id == cur_font {
                        selected_font = index as i32;
                    }
                    break;
                }
            }
            if !found {
                if *id == cur_font {
                    selected_font = fonts.len() as i32;
                }
                fonts.push((
                    file_font.clone(),
                    BitfontSource {
                        file_slot: Some(*id),
                        ..Default::default()
                    },
                ));
            }
        }

        Self {
            do_select: false,
            fonts,
            image_cache: HashMap::default(),
            selected_font,
            filter: String::new(),
            show_builtin: true,
            show_library: true,
            show_file: true,
            has_file: true,
            show_sauce: true,
            edit_selected_font: false,
            should_add,
            only_sauce_fonts,
        }
    }

    pub fn font_library() -> Self {
        let mut fonts = Vec::new();
        for f in SAUCE_FONT_NAMES {
            fonts.push((
                BitFont::from_sauce_name(f).unwrap(),
                BitfontSource {
                    sauce: Some(f.to_string()),
                    ..Default::default()
                },
            ));
        }

        for slot in 0..ANSI_FONTS {
            let ansi_font = BitFont::from_ansi_font_page(slot).unwrap();
            let mut found = false;
            for (existing_font, src) in &mut fonts {
                if existing_font.get_checksum() == ansi_font.get_checksum() {
                    src.ansi_slot = Some(slot);
                    found = true;
                    break;
                }
            }
            if found {
                continue;
            }
            fonts.push((
                ansi_font,
                BitfontSource {
                    ansi_slot: Some(slot),
                    ..Default::default()
                },
            ));
        }

        if let Ok(font_dir) = Settings::get_font_diretory() {
            for lib_font in FontSelector::load_fonts(font_dir.as_path()) {
                let mut found = false;
                for (existing_font, src) in &mut fonts {
                    if existing_font.get_checksum() == lib_font.get_checksum() {
                        src.library = true;
                        found = true;
                        break;
                    }
                }
                if found {
                    continue;
                }

                fonts.push((
                    lib_font,
                    BitfontSource {
                        library: true,
                        ..Default::default()
                    },
                ));
            }
        }
        Self {
            do_select: false,
            fonts,
            image_cache: HashMap::default(),
            selected_font: 0,
            filter: String::new(),
            show_builtin: true,
            show_library: true,
            show_file: true,
            has_file: false,
            show_sauce: true,
            edit_selected_font: false,
            should_add: false,
            only_sauce_fonts: false,
        }
    }

    pub fn load_fonts(tdf_dir: &Path) -> Vec<BitFont> {
        let mut fonts = Vec::new();
        let walker = WalkDir::new(tdf_dir).into_iter();
        for entry in walker.filter_entry(|e| !crate::model::font_imp::FontTool::is_hidden(e)) {
            if let Err(e) = entry {
                log::error!("Can't load font library: {e}");
                break;
            }
            let Ok(entry) = entry else {
                continue;
            };
            let path = entry.path();

            if path.is_dir() {
                continue;
            }
            let extension = path.extension();
            if extension.is_none() {
                continue;
            }
            let Some(extension) = extension else {
                continue;
            };
            let Some(extension) = extension.to_str() else {
                continue;
            };
            let ext = extension.to_lowercase().to_string();

            if is_font_extensions(&ext) {
                if let Ok(font) = BitFont::load(path) {
                    fonts.push(font);
                }
            }

            if ext == "zip" {
                match fs::File::open(path) {
                    Ok(mut file) => {
                        let mut data = Vec::new();
                        file.read_to_end(&mut data).unwrap_or_default();
                        FontSelector::read_zip_archive(data, &mut fonts);
                    }

                    Err(err) => {
                        log::error!("Failed to open zip file: {}", err);
                    }
                }
            }
        }
        fonts
    }

    fn read_zip_archive(data: Vec<u8>, fonts: &mut Vec<BitFont>) {
        let file = std::io::Cursor::new(data);
        match zip::ZipArchive::new(file) {
            Ok(mut archive) => {
                for i in 0..archive.len() {
                    match archive.by_index(i) {
                        Ok(mut file) => {
                            if let Some(path) = file.enclosed_name() {
                                let file_name = path.to_string_lossy().to_string();
                                if let Some(ext) = path.extension() {
                                    let ext = ext.to_string_lossy();
                                    if is_font_extensions(&ext.to_ascii_lowercase()) {
                                        let mut data = Vec::new();
                                        file.read_to_end(&mut data).unwrap_or_default();
                                        if let Ok(font) = BitFont::from_bytes(file_name, &data) {
                                            fonts.push(font)
                                        }
                                    } else if ext == "zip" {
                                        let mut data = Vec::new();
                                        file.read_to_end(&mut data).unwrap_or_default();
                                        FontSelector::read_zip_archive(data, fonts);
                                    }
                                }
                            }
                        }
                        Err(err) => {
                            log::error!("Error reading zip file: {}", err);
                        }
                    }
                }
            }
            Err(err) => {
                log::error!("Error reading zip archive: {}", err);
            }
        }
    }

    pub fn selected_font(&self) -> &BitFont {
        let font = &self.fonts[self.selected_font as usize];
        &font.0
    }

    pub fn draw_font_row(&mut self, ui: &mut egui::Ui, cur_font: usize, row_height: f32, is_selected: bool) -> Response {
        let font = &self.fonts[cur_font];
        let (id, rect) = ui.allocate_space([ui.available_width(), row_height].into());
        let response = ui.interact(rect, id, Sense::click());

        if response.hovered() {
            ui.painter()
                .rect_filled(rect.expand(1.0), CornerRadius::same(4), ui.style().visuals.widgets.active.bg_fill);
        } else if is_selected {
            ui.painter()
                .rect_filled(rect.expand(1.0), CornerRadius::same(4), ui.style().visuals.extreme_bg_color);
        }

        let text_color = if is_selected {
            ui.style().visuals.strong_text_color()
        } else {
            ui.style().visuals.text_color()
        };

        let font_id = FontId::new(14.0, FontFamily::Proportional);
        let text: WidgetText = font.0.name.clone().into();
        let galley = text.into_galley(ui, Some(egui::TextWrapMode::Truncate), f32::INFINITY, font_id);
        ui.painter().galley_with_override_text_color(
            egui::Align2::LEFT_TOP.align_size_within_rect(galley.size(), rect.shrink(4.0)).min,
            galley,
            text_color,
        );

        #[allow(clippy::map_entry)]
        if !self.image_cache.contains_key(&cur_font) {
            let mut buffer = Buffer::new((64, 4));
            buffer.set_font(0, font.0.clone());
            for ch in 0..256 {
                buffer.layers[0].set_char(
                    (ch % 64, ch / 64),
                    AttributedChar::new(unsafe { char::from_u32_unchecked(ch as u32) }, TextAttribute::default()),
                );
            }
            let img = create_image(ui.ctx(), &buffer);
            self.image_cache.insert(cur_font, img);
        }

        if let Some(image) = self.image_cache.get(&cur_font) {
            let sized_texture: SizedTexture = image.into();
            let w = sized_texture.size.x.floor();
            let h = sized_texture.size.y.floor();
            let r = Rect::from_min_size(Pos2::new((rect.left() + 4.0).floor(), (rect.top() + 24.0).floor()), Vec2::new(w, h));
            let image = Image::from_texture(sized_texture);
            image.paint_at(ui, r);

            let mut rect = rect;
            if font.1.library {
                let left = print_source(fl!(crate::LANGUAGE_LOADER, "font_selector-library_font"), ui, rect, text_color);
                rect.set_right(left);
            }

            if font.1.sauce.is_some() {
                let left = print_source(fl!(crate::LANGUAGE_LOADER, "font_selector-sauce_font"), ui, rect, text_color);
                rect.set_right(left);
            }

            if font.1.ansi_slot.is_some() {
                let left = print_source(fl!(crate::LANGUAGE_LOADER, "font_selector-ansi_font"), ui, rect, text_color);
                rect.set_right(left);
            }

            if font.1.file_slot.is_some() {
                let left = print_source(fl!(crate::LANGUAGE_LOADER, "font_selector-file_font"), ui, rect, text_color);
                rect.set_right(left);
            }
        }
        response
    }
}

fn print_source(font_type: String, ui: &egui::Ui, rect: Rect, text_color: Color32) -> f32 {
    let font_id = FontId::new(12.0, FontFamily::Proportional);
    let text: WidgetText = font_type.into();
    let galley = text.into_galley(ui, Some(egui::TextWrapMode::Truncate), f32::INFINITY, font_id);
    let galley_size = galley.size();

    let left_side = rect.right() - galley_size.x - 10.0;
    let rect = Rect::from_min_size(Pos2::new((left_side).floor(), (rect.top() + 8.0).floor()), galley_size);

    ui.painter()
        .rect_filled(rect.expand(2.0), CornerRadius::same(4), ui.style().visuals.widgets.active.bg_fill);

    ui.painter()
        .rect_stroke(rect.expand(2.0), 4.0, Stroke::new(1.0, text_color), egui::StrokeKind::Outside);

    ui.painter()
        .galley_with_override_text_color(egui::Align2::CENTER_CENTER.align_size_within_rect(galley_size, rect).min, galley, text_color);
    left_side
}

impl crate::ModalDialog for FontSelector {
    fn show(&mut self, ctx: &egui::Context) -> bool {
        let mut result = false;
        let modal = Modal::new(ctx, "select_font_dialog2");
        let font_count = self.fonts.len();
        modal.show(|ui| {
            modal.title(
                ui,
                if self.should_add {
                    fl!(crate::LANGUAGE_LOADER, "add-font-dialog-title", fontcount = font_count)
                } else {
                    fl!(crate::LANGUAGE_LOADER, "select-font-dialog-title", fontcount = font_count)
                },
            );
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
                    if !self.only_sauce_fonts {
                        let response = ui.selectable_label(self.show_library, fl!(crate::LANGUAGE_LOADER, "font_selector-library_font"));
                        if response.clicked() {
                            self.show_library = !self.show_library;
                        }

                        if self.has_file {
                            let response = ui.selectable_label(self.show_file, fl!(crate::LANGUAGE_LOADER, "font_selector-file_font"));
                            if response.clicked() {
                                self.show_file = !self.show_file;
                            }
                        }

                        let response = ui.selectable_label(self.show_builtin, fl!(crate::LANGUAGE_LOADER, "font_selector-ansi_font"));
                        if response.clicked() {
                            self.show_builtin = !self.show_builtin;
                        }
                        let response = ui.selectable_label(self.show_sauce, fl!(crate::LANGUAGE_LOADER, "font_selector-sauce_font"));
                        if response.clicked() {
                            self.show_sauce = !self.show_sauce;
                        }
                    }
                });
                ui.add_space(4.0);

                let mut filtered_fonts = Vec::new();

                for i in 0..font_count {
                    let font = &self.fonts[i];
                    let match_filter = self.show_builtin && font.1.ansi_slot.is_some()
                        || self.show_file && font.1.file_slot.is_some()
                        || self.show_library && font.1.library
                        || self.show_sauce && font.1.sauce.is_some();

                    if font.0.name.to_lowercase().contains(&self.filter.to_lowercase()) && match_filter {
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
                let text = if self.should_add {
                    fl!(crate::LANGUAGE_LOADER, "add-font-dialog-select")
                } else {
                    fl!(crate::LANGUAGE_LOADER, "select-font-dialog-select")
                };

                if ui.button(text).clicked() {
                    self.do_select = true;
                    result = true;
                }
                if ui.button(fl!(crate::LANGUAGE_LOADER, "new-file-cancel")).clicked() {
                    result = true;
                }

                let enabled = self.fonts[self.selected_font as usize].0.path_opt.is_some();
                if ui
                    .add_enabled(enabled, Button::new(fl!(crate::LANGUAGE_LOADER, "select-font-dialog-edit-button")))
                    .clicked()
                {
                    self.edit_selected_font = true;
                    result = true;
                }
            });
        });
        modal.open();
        result
    }

    fn should_commit(&self) -> bool {
        self.do_select || self.edit_selected_font
    }

    fn commit(&self, editor: &mut AnsiEditor) -> TerminalResult<Option<Message>> {
        if self.edit_selected_font {
            let font = &self.fonts[self.selected_font as usize];
            if let Some(path) = &font.0.path_opt {
                return Ok(Some(Message::TryLoadFile(path.clone())));
            }
            return Ok(Some(Message::ShowError("Invalid font.".to_string())));
        }

        if let Some((font, src)) = self.fonts.get(self.selected_font as usize) {
            if let Some(file_slot) = src.file_slot {
                return Ok(Some(Message::SwitchToFontPage(file_slot)));
            } else if let Some(ansi_slot) = src.ansi_slot {
                if self.should_add {
                    return Ok(Some(Message::AddAnsiFont(ansi_slot)));
                } else {
                    return Ok(Some(Message::SetAnsiFont(ansi_slot)));
                }
            } else if let Some(name) = &src.sauce {
                if self.should_add {
                    return Ok(Some(Message::AddFont(Box::new(font.clone()))));
                } else {
                    return Ok(Some(Message::SetSauceFont(name.clone())));
                }
            } else {
                let mut font_set = false;
                let mut font_slot = 0;
                editor.buffer_view.lock().get_buffer().font_iter().for_each(|(id, f)| {
                    if f == font {
                        font_slot = *id;
                        font_set = true;
                    }
                });
                if font_set {
                    return Ok(Some(Message::SwitchToFontPage(font_slot)));
                }

                if !font_set {
                    if self.should_add {
                        return Ok(Some(Message::AddFont(Box::new(font.clone()))));
                    } else {
                        return Ok(Some(Message::SetFont(Box::new(font.clone()))));
                    }
                }
            }
        }

        Ok(None)
    }
}
