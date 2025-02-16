use std::path::Path;

use eframe::{
    egui::{self, Layout, Sense, SidePanel, Ui, WidgetText},
    emath::Align,
    epaint::{FontId, Pos2, Rect},
};
use egui::{CornerRadius, Image, Vec2};
use egui_modal::Modal;
use i18n_embed_fl::fl;
use icy_engine::{font::TheDrawFont, BitFont, Buffer, FontType, Palette, ATARI, ATARI_DEFAULT_PALETTE};

use crate::{add_child, AnsiEditor, MainWindow, Message};

trait Template {
    fn image(&self) -> &Image<'static>;
    fn title(&self) -> String;
    fn description(&self) -> String;
    fn create_file(&self, window: &mut MainWindow<'_>) -> crate::TerminalResult<Option<Message>>;

    fn show_ui(&mut self, ui: &mut Ui);
}

pub struct NewFileDialog {
    pub create: bool,

    selected: usize,

    templates: Vec<Box<dyn Template>>,
}

struct Dos16Template {
    pub width: i32,
    pub height: i32,
}

impl Template for Dos16Template {
    fn image(&self) -> &Image<'static> {
        &crate::ANSI_TEMPLATE_IMG
    }

    fn title(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "new-file-template-cp437-title")
    }

    fn description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "new-file-template-cp437-description")
    }

    fn show_ui(&mut self, ui: &mut Ui) {
        show_file_ui(ui, &mut self.width, &mut self.height);
    }

    fn create_file(&self, window: &mut MainWindow<'_>) -> crate::TerminalResult<Option<Message>> {
        let mut buf = Buffer::create((self.width, self.height));
        buf.ice_mode = icy_engine::IceMode::Blink;
        buf.palette_mode = icy_engine::PaletteMode::Fixed16;
        buf.font_mode = icy_engine::FontMode::Sauce;

        let editor = AnsiEditor::new(&window.gl, buf);
        add_child(&mut window.document_tree, None, Box::new(editor));
        Ok(None)
    }
}

struct Ice16Template {
    pub width: i32,
    pub height: i32,
}

impl Template for Ice16Template {
    fn image(&self) -> &Image<'static> {
        &crate::ANSI_TEMPLATE_IMG
    }

    fn title(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "new-file-template-ice-title")
    }

    fn description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "new-file-template-ice-description")
    }

    fn show_ui(&mut self, ui: &mut Ui) {
        show_file_ui(ui, &mut self.width, &mut self.height);
    }

    fn create_file(&self, window: &mut MainWindow<'_>) -> crate::TerminalResult<Option<Message>> {
        let mut buf = Buffer::create((self.width, self.height));
        buf.ice_mode = icy_engine::IceMode::Ice;
        buf.palette_mode = icy_engine::PaletteMode::Fixed16;
        buf.font_mode = icy_engine::FontMode::Sauce;

        let editor = AnsiEditor::new(&window.gl, buf);
        add_child(&mut window.document_tree, None, Box::new(editor));
        Ok(None)
    }
}

struct XB16Template {
    pub width: i32,
    pub height: i32,
}

impl Template for XB16Template {
    fn image(&self) -> &Image<'static> {
        &crate::ANSI_TEMPLATE_IMG
    }

    fn title(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "new-file-template-xb-title")
    }

    fn description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "new-file-template-xb-description")
    }

    fn show_ui(&mut self, ui: &mut Ui) {
        show_file_ui(ui, &mut self.width, &mut self.height);
    }

    fn create_file(&self, window: &mut MainWindow<'_>) -> crate::TerminalResult<Option<Message>> {
        let mut buf = Buffer::create((self.width, self.height));
        buf.ice_mode = icy_engine::IceMode::Ice;
        buf.palette_mode = icy_engine::PaletteMode::Free16;
        buf.font_mode = icy_engine::FontMode::Single;

        let editor = AnsiEditor::new(&window.gl, buf);
        add_child(&mut window.document_tree, None, Box::new(editor));
        Ok(None)
    }
}

struct XBExtTemplate {
    pub width: i32,
    pub height: i32,
}

impl Template for XBExtTemplate {
    fn image(&self) -> &Image<'static> {
        &crate::ANSI_TEMPLATE_IMG
    }

    fn title(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "new-file-template-xb-ext-title")
    }

    fn description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "new-file-template-xb-ext-description")
    }

    fn show_ui(&mut self, ui: &mut Ui) {
        show_file_ui(ui, &mut self.width, &mut self.height);
    }

    fn create_file(&self, window: &mut MainWindow<'_>) -> crate::TerminalResult<Option<Message>> {
        let mut buf = Buffer::create((self.width, self.height));
        buf.ice_mode = icy_engine::IceMode::Ice;
        buf.palette_mode = icy_engine::PaletteMode::Free16;
        buf.font_mode = icy_engine::FontMode::FixedSize;
        buf.set_font(1, BitFont::default());
        let editor = AnsiEditor::new(&window.gl, buf);
        add_child(&mut window.document_tree, None, Box::new(editor));
        Ok(None)
    }
}

struct AnsiTemplate {
    pub width: i32,
    pub height: i32,
}

impl Template for AnsiTemplate {
    fn image(&self) -> &Image<'static> {
        &crate::ANSI_TEMPLATE_IMG
    }

    fn title(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "new-file-template-ansi-title")
    }

    fn description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "new-file-template-ansi-description")
    }

    fn show_ui(&mut self, ui: &mut Ui) {
        show_file_ui(ui, &mut self.width, &mut self.height);
    }

    fn create_file(&self, window: &mut MainWindow<'_>) -> crate::TerminalResult<Option<Message>> {
        let mut buf = Buffer::create((self.width, self.height));
        buf.ice_mode = icy_engine::IceMode::Unlimited;
        buf.palette_mode = icy_engine::PaletteMode::RGB;
        buf.font_mode = icy_engine::FontMode::Unlimited;

        let editor = AnsiEditor::new(&window.gl, buf);
        add_child(&mut window.document_tree, None, Box::new(editor));
        Ok(None)
    }
}

struct FileIdTemplate {
    pub width: i32,
    pub height: i32,
}

impl Template for FileIdTemplate {
    fn image(&self) -> &Image<'static> {
        &crate::ANSI_TEMPLATE_IMG
    }

    fn title(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "new-file-template-file_id-title")
    }

    fn description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "new-file-template-file_id-description")
    }

    fn show_ui(&mut self, ui: &mut Ui) {
        show_file_ui(ui, &mut self.width, &mut self.height);
    }

    fn create_file(&self, window: &mut MainWindow<'_>) -> crate::TerminalResult<Option<Message>> {
        let mut buf = Buffer::create((self.width, self.height));
        buf.ice_mode = icy_engine::IceMode::Blink;
        buf.palette_mode = icy_engine::PaletteMode::Fixed16;
        buf.font_mode = icy_engine::FontMode::Sauce;
        let editor = AnsiEditor::new(&window.gl, buf);
        add_child(&mut window.document_tree, None, Box::new(editor));
        Ok(None)
    }
}

struct AtasciiTemplate {
    pub width: i32,
    pub height: i32,
}

impl Template for AtasciiTemplate {
    fn image(&self) -> &Image<'static> {
        &crate::ANSI_TEMPLATE_IMG
    }

    fn title(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "new-file-template-atascii-title")
    }

    fn description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "new-file-template-atascii-description")
    }

    fn show_ui(&mut self, ui: &mut Ui) {
        show_file_ui(ui, &mut self.width, &mut self.height);
    }

    fn create_file(&self, window: &mut MainWindow<'_>) -> crate::TerminalResult<Option<Message>> {
        let mut buf = Buffer::create((self.width, self.height));
        buf.buffer_type = icy_engine::BufferType::Atascii;
        buf.ice_mode = icy_engine::IceMode::Ice;
        buf.palette_mode = icy_engine::PaletteMode::Fixed16;
        buf.font_mode = icy_engine::FontMode::Single;
        buf.clear_font_table();
        let mut font = BitFont::from_bytes("", ATARI).unwrap();
        font.length = 128;
        for k in 0..128u8 {
            font.glyphs.remove(&((128 + k) as char));
        }

        buf.set_font(0, font);
        buf.palette = Palette::from_slice(&ATARI_DEFAULT_PALETTE);

        let editor = AnsiEditor::new(&window.gl, buf);
        add_child(&mut window.document_tree, None, Box::new(editor));
        Ok(None)
    }
}

fn show_file_ui(ui: &mut Ui, width: &mut i32, height: &mut i32) {
    egui::Grid::new("some_unique_id").num_columns(2).spacing([4.0, 8.0]).show(ui, |ui| {
        ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(fl!(crate::LANGUAGE_LOADER, "new-file-width"));
        });
        let mut tmp_str = width.to_string();
        ui.add(egui::TextEdit::singleline(&mut tmp_str).char_limit(35));
        if let Ok(new_width) = tmp_str.parse::<i32>() {
            *width = new_width;
        }
        ui.end_row();

        ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(fl!(crate::LANGUAGE_LOADER, "new-file-height"));
        });
        let mut tmp_str = height.to_string();
        ui.add(egui::TextEdit::singleline(&mut tmp_str).char_limit(35));
        if let Ok(new_height) = tmp_str.parse::<i32>() {
            *height = new_height;
        }
        ui.end_row();
    });
}

struct AnsiMationTemplate {}

impl Template for AnsiMationTemplate {
    fn image(&self) -> &Image<'static> {
        &crate::ANSIMATION_TEMPLATE_IMG
    }

    fn title(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "new-file-template-ansimation-title")
    }

    fn description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "new-file-template-ansimation-description")
    }

    fn create_file(&self, window: &mut MainWindow<'_>) -> crate::TerminalResult<Option<Message>> {
        let txt = r#"local buf = new_buffer(80, 25)

for i=0,9,1 do
    buf:clear()
    buf.x = 10 + i * 5
    buf.y = 10
    buf:print("Hello World " .. cur_frame)
    next_frame(buf)
end"#;
        let editor = crate::AnimationEditor::new(window.gl.clone(), Path::new("."), txt.into());
        add_child(&mut window.document_tree, None, Box::new(editor));
        Ok(None)
    }

    fn show_ui(&mut self, ui: &mut Ui) {
        ui.label(fl!(crate::LANGUAGE_LOADER, "new-file-template-ansimation-ui-label"));
        ui.hyperlink("https://github.com/mkrueger/icy_tools/blob/master/crates/icy_draw/doc/lua_api.md");
    }
}

struct BitFontTemplate {
    width: i32,
    height: i32,
}

impl Template for BitFontTemplate {
    fn image(&self) -> &Image<'static> {
        &crate::BITFONT_TEMPLATE_IMG
    }

    fn title(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "new-file-template-bit_font-title")
    }

    fn description(&self) -> String {
        fl!(crate::LANGUAGE_LOADER, "new-file-template-bit_font-description")
    }

    fn create_file(&self, window: &mut MainWindow<'_>) -> crate::TerminalResult<Option<Message>> {
        let font = if self.width == 8 && self.height == 16 {
            BitFont::default()
        } else if self.width == 8 && self.height == 8 {
            BitFont::from_sauce_name("IBM VGA50")?
        } else if self.width == 8 && self.height == 14 {
            BitFont::from_sauce_name("IBM EGA")?
        } else {
            BitFont::create_8(
                format!("Empty {}x{}", self.width, self.height),
                self.width as u8,
                self.height as u8,
                &vec![0; 256 * self.height as usize],
            )
        };
        let editor = crate::BitFontEditor::new(&window.gl, font);
        add_child(&mut window.document_tree, None, Box::new(editor));
        Ok(None)
    }
    fn show_ui(&mut self, ui: &mut Ui) {
        ui.label(fl!(crate::LANGUAGE_LOADER, "new-file-template-bitfont-ui-label"));
        ui.add_space(8.0);
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
    }
}

struct TdfFontTemplate {
    pub font_type: FontType,
}

impl Template for TdfFontTemplate {
    fn image(&self) -> &Image<'static> {
        match self.font_type {
            FontType::Outline => &crate::OUTLINEFONT_TEMPLATE_IMG,
            FontType::Block => &crate::BLOCKFONT_TEMPLATE_IMG,
            FontType::Color => &crate::COLORFONT_TEMPLATE_IMG,
            FontType::Figlet => &crate::BLOCKFONT_TEMPLATE_IMG,
        }
    }

    fn title(&self) -> String {
        match self.font_type {
            FontType::Outline => fl!(crate::LANGUAGE_LOADER, "new-file-template-outline_font-title"),
            FontType::Block => fl!(crate::LANGUAGE_LOADER, "new-file-template-block_font-title"),
            FontType::Color => fl!(crate::LANGUAGE_LOADER, "new-file-template-color_font-title"),
            _ => String::new(),
        }
    }

    fn description(&self) -> String {
        match self.font_type {
            FontType::Outline => fl!(crate::LANGUAGE_LOADER, "new-file-template-outline_font-description"),
            FontType::Block => fl!(crate::LANGUAGE_LOADER, "new-file-template-block_font-description"),
            FontType::Color => fl!(crate::LANGUAGE_LOADER, "new-file-template-color_font-description"),
            _ => String::new(),
        }
    }

    fn create_file(&self, window: &mut MainWindow<'_>) -> crate::TerminalResult<Option<Message>> {
        let fonts = vec![TheDrawFont::new(self.title(), self.font_type, 1)];
        let editor = crate::CharFontEditor::new(&window.gl, fonts);
        add_child(&mut window.document_tree, None, Box::new(editor));
        Ok(None)
    }

    fn show_ui(&mut self, ui: &mut Ui) {
        ui.label(fl!(crate::LANGUAGE_LOADER, "new-file-template-thedraw-ui-label"));
        ui.hyperlink("http://www.roysac.com/thedrawfonts-tdf.html");
    }
}

impl Default for NewFileDialog {
    fn default() -> Self {
        let templates: Vec<Box<dyn Template>> = vec![
            Box::new(AnsiTemplate { width: 80, height: 25 }),
            Box::new(XB16Template { width: 80, height: 25 }),
            Box::new(Dos16Template { width: 80, height: 25 }),
            Box::new(Ice16Template { width: 80, height: 25 }),
            Box::new(XBExtTemplate { width: 80, height: 25 }),
            Box::new(FileIdTemplate { width: 44, height: 25 }),
            Box::new(AnsiMationTemplate {}),
            Box::new(AtasciiTemplate { width: 40, height: 24 }),
            Box::new(BitFontTemplate { width: 8, height: 16 }),
            Box::new(TdfFontTemplate { font_type: FontType::Color }),
            Box::new(TdfFontTemplate { font_type: FontType::Block }),
            Box::new(TdfFontTemplate { font_type: FontType::Outline }),
        ];

        Self {
            create: false,
            templates,
            selected: 0,
        }
    }
}

impl crate::ModalDialog for NewFileDialog {
    fn show(&mut self, ctx: &egui::Context) -> bool {
        let mut result = false;
        let modal = Modal::new(ctx, "new_file_dialog");

        modal.show(|ui| {
            ui.set_height(420.);
            ui.set_width(800.);

            modal.title(ui, fl!(crate::LANGUAGE_LOADER, "new-file-title"));

            modal.frame(ui, |ui| {
                SidePanel::left("new_file_side_panel")
                    .exact_width(280.0)
                    .resizable(false)
                    .show_inside(ui, |ui| {
                        let row_height = 58.0;
                        egui::ScrollArea::vertical().id_salt("bitfont_scroll_area").show(ui, |ui| {
                            for (i, template) in self.templates.iter().enumerate() {
                                let is_selected = i == self.selected;

                                let (id, rect) = ui.allocate_space([ui.available_width(), row_height].into());
                                let response = ui.interact(rect, id, Sense::click());
                                if response.hovered() {
                                    ui.painter()
                                        .rect_filled(rect.expand(1.0), CornerRadius::same(4), ui.style().visuals.widgets.active.bg_fill);
                                } else if is_selected {
                                    ui.painter()
                                        .rect_filled(rect.expand(1.0), CornerRadius::same(4), ui.style().visuals.extreme_bg_color);
                                }
                                let image = template.image();
                                let r = Rect::from_min_size(Pos2::new(rect.left() + 4.0, rect.top() + 4.0), Vec2::new(32.0, 32.0));
                                image.paint_at(ui, r);

                                let font_id = FontId::new(20.0, eframe::epaint::FontFamily::Proportional);
                                let text: WidgetText = template.title().into();
                                let galley = text.into_galley(ui, Some(egui::TextWrapMode::Truncate), f32::INFINITY, font_id);
                                let mut title_rect = rect;
                                title_rect.set_left(title_rect.left() + 40.0);
                                title_rect.set_top(title_rect.top() + 4.0);
                                ui.painter().galley_with_override_text_color(
                                    egui::Align2::LEFT_TOP.align_size_within_rect(galley.size(), title_rect.shrink(4.0)).min,
                                    galley,
                                    ui.style().visuals.strong_text_color(),
                                );

                                let font_id = FontId::new(14.0, eframe::epaint::FontFamily::Proportional);
                                let text: WidgetText = template.description().lines().next().unwrap_or_default().into();
                                let galley = text.into_galley(ui, Some(egui::TextWrapMode::Truncate), f32::INFINITY, font_id);
                                let mut descr_rect = rect;
                                descr_rect.set_top(descr_rect.top() + 34.0);
                                ui.painter().galley_with_override_text_color(
                                    egui::Align2::LEFT_TOP.align_size_within_rect(galley.size(), descr_rect.shrink(4.0)).min,
                                    galley,
                                    ui.style().visuals.text_color(),
                                );

                                if response.clicked() {
                                    self.selected = i;
                                }
                                if response.double_clicked() {
                                    self.selected = i;
                                    self.create = true;
                                    result = true;
                                }
                            }
                        });
                    });

                egui::CentralPanel::default().show_inside(ui, |ui| {
                    ui.label(self.templates[self.selected].description());
                    ui.separator();
                    self.templates[self.selected].show_ui(ui);
                });
            });
            ui.separator();
            ui.add_space(4.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button(fl!(crate::LANGUAGE_LOADER, "new-file-create")).clicked() {
                    self.create = true;
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
        self.create
    }

    fn commit_self(&self, _ctx: &egui::Context, window: &mut MainWindow<'_>) -> crate::TerminalResult<Option<Message>> {
        self.templates[self.selected].create_file(window)
    }
}
