use eframe::egui::{self, Color32, Key};
use icy_draw::{Settings, brush::BrushPrimaryMode, document::Document};
use icy_engine::{FileFormat, Position, Selection, Size, TextPane};
use icy_engine_edit::UndoState;
use icy_engine_edit::tools::Tool;
use icy_engine_gui::{
    ScalingMode,
    egui::{
        appearance::{self, DialogButton, DialogSize, MessageBox, MessageKind, labels},
        screen::ScreenView,
    },
};
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
};

use super::widgets::{self, Icons};
#[path = "chrome.rs"]
mod chrome;
#[path = "mcp.rs"]
mod mcp;

enum Dialog {
    New,
    Resize,
    Close,
    Characters,
    FKeyCharacter(usize, usize),
    Inspector,
    Sauce,
    Export,
    Font,
    FontSelect,
    Palette,
    Tags,
    TagProperties(Option<usize>, Box<icy_engine::Tag>),
    Script,
    Monitor,
    Overwrite(PathBuf),
    ExportOverwrite(PathBuf),
    FontOverwrite(PathBuf),
    AnimationOverwrite(PathBuf, super::animation::ExportFormat),
    Error(String),
}
enum FileAction {
    Open,
    Save,
    Export(FileFormat),
    SaveFont,
    SaveTdf,
    SaveAnimation,
    ExportAnimation(super::animation::ExportFormat),
}
struct Picked {
    action: FileAction,
    path: Option<PathBuf>,
}

pub struct DrawApp {
    pub document: Document,
    pub view: ScreenView,
    settings: Settings,
    icons: Icons,
    dialog: Option<Dialog>,
    picker: bool,
    sender: Sender<Picked>,
    receiver: Receiver<Picked>,
    pending: Option<PathBuf>,
    quitting: bool,
    allow_close: bool,
    continue_after_save: bool,
    new_size: [i32; 2],
    show_inspector: bool,
    canvas_focus: bool,
    sauce_fields: [String; 4],
    export_format: FileFormat,
    export_sauce: bool,
    font_editor: Option<super::font::FontEditor>,
    palette_edit: icy_engine::Palette,
    palette_index: usize,
    charfont: Option<icy_draw::charfont::CharFontDocument>,
    new_font_type: Option<icy_engine_edit::charset::TdfFontType>,
    animation: Option<super::animation::AnimationEditor>,
    new_animation: bool,
    clipboard: Option<(String, Vec<u8>)>,
    font_filter: String,
    text_fonts: Option<icy_draw::text_art_fonts::SharedFontLibrary>,
    text_font: usize,
    text_preview: Option<(usize, egui::TextureHandle)>,
    plugins: Option<Vec<icy_draw::plugins::Plugin>>,
    script: String,
    script_output: String,
    mcp: Option<mcp::Bridge>,
    chrome: chrome::Chrome,
    pub persist_settings: bool,
    new_bitmap: bool,
    show_grid: bool,
    show_layer_bounds: bool,
    pub canvas_rect: egui::Rect,
}

impl DrawApp {
    pub fn new() -> Self {
        let document = Document::new(Size::new(80, 25));
        let view = ScreenView::from_shared(document.screen.clone());
        let mut settings = Settings::load();
        settings.monitor_settings.scaling_mode = ScalingMode::Manual(2.0);
        let (sender, receiver) = mpsc::channel();
        Self {
            document,
            view,
            settings,
            icons: Icons::default(),
            dialog: None,
            picker: false,
            sender,
            receiver,
            pending: None,
            quitting: false,
            allow_close: false,
            continue_after_save: false,
            new_size: [80, 25],
            show_inspector: true,
            canvas_focus: true,
            sauce_fields: Default::default(),
            export_format: FileFormat::Ansi,
            export_sauce: true,
            font_editor: None,
            palette_edit: icy_engine::Palette::default(),
            palette_index: 0,
            charfont: None,
            new_font_type: None,
            animation: None,
            new_animation: false,
            clipboard: None,
            font_filter: String::new(),
            text_fonts: None,
            text_font: 0,
            text_preview: None,
            plugins: None,
            script: String::new(),
            script_output: String::new(),
            mcp: None,
            chrome: chrome::Chrome::default(),
            persist_settings: false,
            new_bitmap: false,
            show_grid: false,
            show_layer_bounds: false,
            canvas_rect: egui::Rect::NOTHING,
        }
    }

    fn replace(&mut self, document: Document) {
        self.charfont = None;
        self.animation = None;
        self.font_editor = None;
        if matches!(self.dialog, Some(Dialog::Font)) {
            self.dialog = None;
        }
        self.document = document;
        self.chrome = chrome::Chrome::default();
        self.view = ScreenView::from_shared(self.document.screen.clone());
        self.canvas_focus = true;
    }

    pub fn open(&mut self, path: PathBuf) {
        self.document.finish();
        if let Some(editor) = &mut self.font_editor {
            editor.finish();
            if editor.modified() {
                self.dialog = Some(Dialog::Font);
                return;
            }
        }
        if path.extension().is_some_and(|extension| {
            ["psf", "psfu", "fnt", "fon", "f08", "f14", "f16", "yaff"]
                .iter()
                .any(|candidate| extension.eq_ignore_ascii_case(candidate))
        }) {
            match super::font::FontEditor::load(&path) {
                Ok(editor) => {
                    self.font_editor = Some(editor);
                    self.dialog = Some(Dialog::Font);
                }
                Err(error) => self.dialog = Some(Dialog::Error(error)),
            }
            return;
        }
        if self.modified() {
            self.pending = Some(path);
            self.dialog = Some(Dialog::Close);
            return;
        }
        self.load_path(path);
    }

    fn load_path(&mut self, path: PathBuf) {
        self.load_document(path.clone());
        if self.persist_settings && !matches!(self.dialog, Some(Dialog::Error(_))) {
            self.settings.recent_files.add_recent_file(&path);
        }
    }

    fn load_document(&mut self, path: PathBuf) {
        if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("icyanim") || extension.eq_ignore_ascii_case("lua"))
        {
            match super::animation::AnimationEditor::load(&path) {
                Ok(editor) => {
                    self.replace(Document::new(Size::new(80, 25)));
                    self.animation = Some(editor);
                }
                Err(error) => self.dialog = Some(Dialog::Error(error)),
            }
            return;
        }
        if path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("tdf")) {
            match icy_draw::charfont::CharFontDocument::load(&path) {
                Ok(font) => {
                    self.replace(font.document());
                    self.charfont = Some(font);
                }
                Err(error) => self.dialog = Some(Dialog::Error(error)),
            }
            return;
        }
        match Document::load(&path) {
            Ok(document) => self.replace(document),
            Err(error) => self.dialog = Some(Dialog::Error(error)),
        }
    }

    fn choose(&mut self, context: &egui::Context, action: FileAction) {
        if self.picker {
            return;
        }
        self.document.finish();
        self.picker = true;
        let sender = self.sender.clone();
        let context = context.clone();
        let initial = self.document.path.clone();
        std::thread::spawn(move || {
            let mut dialog = rfd::FileDialog::new();
            if let Some(path) = initial {
                if let Some(parent) = path.parent() {
                    dialog = dialog.set_directory(parent);
                }
            }
            let path = match action {
                FileAction::Open => dialog.pick_file(),
                FileAction::Save => dialog.add_filter("Icy Draw", &["icy"]).set_file_name("Untitled.icy").save_file(),
                FileAction::Export(format) => dialog
                    .add_filter(format.name(), &[format.primary_extension()])
                    .set_file_name(format!("Untitled.{}", format.primary_extension()))
                    .save_file(),
                FileAction::SaveFont => dialog.add_filter("PSF Bitmap Font", &["psf"]).set_file_name("Untitled.psf").save_file(),
                FileAction::SaveTdf => dialog.add_filter("TheDraw Font", &["tdf"]).set_file_name("Untitled.tdf").save_file(),
                FileAction::SaveAnimation => dialog.add_filter("Icy Animation", &["icyanim"]).set_file_name("Untitled.icyanim").save_file(),
                FileAction::ExportAnimation(format) => dialog
                    .add_filter(format.name(), &[format.extension()])
                    .set_file_name(format!("Untitled.{}", format.extension()))
                    .save_file(),
            };
            let _ = sender.send(Picked { action, path });
            context.request_repaint();
        });
    }

    fn save(&mut self, context: &egui::Context, save_as: bool) {
        if self.animation.is_some() {
            if let Some(path) = self.animation.as_ref().and_then(|editor| editor.path.clone()).filter(|_| !save_as) {
                self.save_path(context, path, false);
            } else {
                self.choose(context, FileAction::SaveAnimation);
            }
            return;
        }
        if self.charfont.is_some() {
            if let Some(path) = self.charfont.as_ref().and_then(|font| font.path.clone()).filter(|_| !save_as) {
                self.save_path(context, path, false);
            } else {
                self.choose(context, FileAction::SaveTdf);
            }
            return;
        }
        if !save_as {
            if let Some(path) = self
                .document
                .path
                .clone()
                .filter(|path| FileFormat::from_path(path) == Some(FileFormat::IcyDraw))
            {
                self.save_path(context, path, false);
                return;
            }
        }
        self.choose(context, FileAction::Save);
    }

    fn save_path(&mut self, context: &egui::Context, path: PathBuf, overwrite: bool) {
        let result = if let Some(editor) = &mut self.animation {
            editor.save(&path, overwrite)
        } else if let Some(font) = &mut self.charfont {
            font.save_with_overwrite(&mut self.document, &path, overwrite)
        } else {
            self.document.save(&path, overwrite)
        };
        match result {
            Ok(()) => {
                if self.persist_settings {
                    self.settings.recent_files.add_recent_file(&path);
                }
                if self.continue_after_save {
                    self.continue_after_save = false;
                    self.complete_close(context);
                }
            }
            Err(error) => {
                self.continue_after_save = false;
                self.dialog = Some(Dialog::Error(error));
            }
        }
    }

    fn complete_close(&mut self, context: &egui::Context) {
        if self.quitting {
            self.allow_close = true;
            context.send_viewport_cmd(egui::ViewportCommand::Close);
        } else if let Some(path) = self.pending.take() {
            self.load_path(path);
        } else {
            self.dialog = Some(Dialog::New);
        }
        self.quitting = false;
    }

    fn export_path(&mut self, path: PathBuf) {
        if self
            .document
            .path
            .as_ref()
            .is_some_and(|source| source == &path || source.canonicalize().ok().is_some_and(|source| Some(source) == path.canonicalize().ok()))
        {
            self.dialog = Some(Dialog::Error(
                "Export must not overwrite the current editable document. Choose a different filename.".into(),
            ));
            return;
        }
        let result = super::export::write(&self.document, &path, self.export_format, &self.settings.export_settings, self.export_sauce);
        self.result(result);
    }

    fn result(&mut self, result: Result<(), String>) {
        if let Err(error) = result {
            self.dialog = Some(Dialog::Error(error));
        }
    }

    fn edit(&mut self, action: impl FnOnce(&mut icy_engine_edit::EditState) -> icy_engine::Result<()>) {
        let result = self.document.with_state(action).map_err(|error| error.to_string());
        self.result(result);
    }

    fn modified(&self) -> bool {
        self.document.modified()
            || self.charfont.as_ref().is_some_and(|font| font.modified())
            || self.animation.as_ref().is_some_and(|editor| editor.modified())
    }

    fn undo(&mut self, redo: bool) {
        if let Some(animation) = &mut self.animation {
            animation.undo_source(redo);
            return;
        }
        self.document.finish();
        if self.document.paste_active() || self.document.with_state(|state| if redo { state.can_redo() } else { state.can_undo() }) || self.charfont.is_none() {
            let result = if redo { self.document.redo() } else { self.document.undo() };
            self.result(result);
        } else if let Some(font) = &mut self.charfont {
            if redo {
                font.state.redo();
            } else {
                font.state.undo();
            }
            let mut document = font.document();
            document.tool = if document.outline_font { Tool::Click } else { self.document.tool };
            document.brush = self.document.brush;
            self.document = document;
            self.chrome = chrome::Chrome::default();
            self.view = ScreenView::from_shared(self.document.screen.clone());
        }
    }

    fn change_charfont(&mut self, action: impl FnOnce(&mut icy_engine_edit::charset::CharSetEditState)) {
        if let Some(font) = &mut self.charfont {
            font.commit(&mut self.document);
            action(&mut font.state);
            let mut document = font.document();
            document.brush = self.document.brush;
            document.tool = if document.outline_font { Tool::Click } else { self.document.tool };
            let attribute = self.document.with_state(|state| state.get_caret().attribute);
            document.with_state(|state| state.set_caret_attribute(attribute));
            self.document = document;
            self.chrome = chrome::Chrome::default();
            self.view = ScreenView::from_shared(self.document.screen.clone());
        }
    }

    fn charfont_bar(&mut self, context: &egui::Context) {
        let Some(font) = &self.charfont else {
            return;
        };
        let fonts: Vec<_> = font.state.fonts().iter().map(|font| font.name.clone()).collect();
        let mut selected = font.state.selected_font_index();
        let mut character = font.state.selected_char().unwrap_or('A');
        let mut name = font.state.selected_font().map(|font| font.name.clone()).unwrap_or_default();
        let mut spacing = font.state.selected_font().map_or(0, |font| font.spacing);
        egui::TopBottomPanel::top("tdf-fonts").show(context, |ui| {
            if self.dialog.is_some() || self.picker || self.layer_properties_open() || self.document.paste_active() {
                ui.disable();
            }
            ui.horizontal_wrapped(|ui| {
                egui::ComboBox::from_id_salt("tdf-font")
                    .selected_text(format!("Font {}", selected + 1))
                    .show_ui(ui, |ui| {
                        for (index, name) in fonts.iter().enumerate() {
                            if ui.selectable_value(&mut selected, index, name).changed() {
                                self.change_charfont(|state| state.select_font(index));
                            }
                        }
                    });
                if ui.add(egui::TextEdit::singleline(&mut name).desired_width(120.0).char_limit(12)).changed() {
                    self.change_charfont(|state| state.set_font_name(name));
                }
                if ui.add(egui::DragValue::new(&mut spacing).range(0..=40).prefix("Spacing ")).changed() {
                    self.change_charfont(|state| state.set_font_spacing(spacing));
                }
                if self.icons.button(ui, "file_copy", "Duplicate Font", false).clicked() {
                    self.change_charfont(|state| state.clone_font());
                }
                if self.icons.button(ui, "delete", "Delete Font", false).clicked() && fonts.len() > 1 {
                    self.change_charfont(|state| state.delete_font());
                }
                ui.menu_button("Add Font", |ui| {
                    for kind in [
                        icy_engine_edit::charset::TdfFontType::Color,
                        icy_engine_edit::charset::TdfFontType::Block,
                        icy_engine_edit::charset::TdfFontType::Outline,
                    ] {
                        if ui.button(format!("{kind:?}")).clicked() {
                            self.change_charfont(|state| state.add_font(kind, "New Font".into(), 1));
                            ui.close();
                        }
                    }
                });
                egui::ComboBox::from_id_salt("tdf-character")
                    .selected_text(format!("Character {character}"))
                    .show_ui(ui, |ui| {
                        egui::Grid::new("tdf-chars").show(ui, |ui| {
                            for code in 33..=126 {
                                let code = char::from_u32(code).unwrap();
                                if ui.selectable_value(&mut character, code, code.to_string()).changed() {
                                    self.change_charfont(|state| state.select_char(code));
                                }
                                if (code as u32 - 33) % 12 == 11 {
                                    ui.end_row();
                                }
                            }
                        });
                    });
            });
        });
    }

    /// TDF font editing controls for the right sidebar.
    fn charfont_section(&mut self, ui: &mut egui::Ui) {
        let Some(font) = &self.charfont else {
            return;
        };
        let fonts: Vec<_> = font.state.fonts().iter().map(|font| font.name.clone()).collect();
        let mut selected = font.state.selected_font_index();
        let character = font.state.selected_char().unwrap_or('A');
        let mut name = font.state.selected_font().map(|font| font.name.clone()).unwrap_or_default();
        let mut spacing = font.state.selected_font().map_or(0, |font| font.spacing);
        let glyphs: Vec<bool> = (33..=126u32).map(|code| font.state.has_glyph(char::from_u32(code).unwrap())).collect();
        widgets::section_header(ui, "TDF Font", |ui| {
            let add = self.icons.button_sized(ui, "add", "Add Font", false, 26.0);
            egui::Popup::menu(&add).show(|ui| {
                for kind in [
                    icy_engine_edit::charset::TdfFontType::Color,
                    icy_engine_edit::charset::TdfFontType::Block,
                    icy_engine_edit::charset::TdfFontType::Outline,
                ] {
                    if ui.button(format!("{kind:?} Font")).clicked() {
                        self.change_charfont(|state| state.add_font(kind, "New Font".into(), 1));
                        ui.close();
                    }
                }
            });
            ui.add_enabled_ui(fonts.len() > 1, |ui| {
                if self.icons.button_sized(ui, "delete", "Delete Font", false, 26.0).clicked() {
                    self.change_charfont(|state| state.delete_font());
                }
            });
            if self.icons.button_sized(ui, "file_copy", "Duplicate Font", false, 26.0).clicked() {
                self.change_charfont(|state| state.clone_font());
            }
        });
        egui::ComboBox::from_id_salt("tdf-font")
            .width(ui.available_width())
            .selected_text(format!("{}. {}", selected + 1, fonts.get(selected).map(String::as_str).unwrap_or("")))
            .show_ui(ui, |ui| {
                for (index, name) in fonts.iter().enumerate() {
                    if ui.selectable_value(&mut selected, index, format!("{}. {name}", index + 1)).changed() {
                        self.change_charfont(|state| state.select_font(index));
                    }
                }
            });
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.label("Name");
            let spacing_width = 124.0;
            if ui
                .add(
                    egui::TextEdit::singleline(&mut name)
                        .desired_width(ui.available_width() - spacing_width)
                        .char_limit(12),
                )
                .changed()
            {
                self.change_charfont(|state| state.set_font_name(name));
            }
            ui.label("Spacing");
            if ui.add(egui::DragValue::new(&mut spacing).range(0..=40)).changed() {
                self.change_charfont(|state| state.set_font_spacing(spacing));
            }
        });
        ui.add_space(4.0);
        let columns = 12;
        let cell = egui::vec2(ui.available_width() / columns as f32, 18.0);
        let rows = glyphs.len().div_ceil(columns);
        let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), cell.y * rows as f32), egui::Sense::click());
        let hovered = response.hover_pos().and_then(|point| {
            let local = point - rect.min;
            let index = (local.y / cell.y) as usize * columns + (local.x / cell.x) as usize;
            (local.x >= 0.0 && local.y >= 0.0 && index < glyphs.len()).then_some(index)
        });
        let visuals = ui.visuals().clone();
        ui.painter().rect_filled(rect, 6, visuals.extreme_bg_color);
        for (index, &present) in glyphs.iter().enumerate() {
            let code = char::from_u32(33 + index as u32).unwrap();
            let target = egui::Rect::from_min_size(
                rect.min + egui::vec2((index % columns) as f32 * cell.x, (index / columns) as f32 * cell.y),
                cell,
            )
            .shrink(1.0);
            let color = if code == character {
                ui.painter().rect_filled(target, 4, appearance::PRIMARY);
                Color32::WHITE
            } else {
                if hovered == Some(index) {
                    ui.painter().rect_filled(target, 4, visuals.widgets.hovered.weak_bg_fill);
                }
                if present {
                    visuals.strong_text_color()
                } else {
                    visuals.weak_text_color().gamma_multiply(0.45)
                }
            };
            ui.painter()
                .text(target.center(), egui::Align2::CENTER_CENTER, code, egui::FontId::monospace(13.0), color);
        }
        if let Some(index) = hovered {
            let code = char::from_u32(33 + index as u32).unwrap();
            if response.clicked() {
                self.change_charfont(|state| state.select_char(code));
            }
            response.on_hover_text(if glyphs[index] {
                format!("Edit '{code}'")
            } else {
                format!("Create '{code}'")
            });
        }
    }

    fn menu(&mut self, context: &egui::Context) {
        let blocked = self.dialog.is_some() || self.picker || self.layer_properties_open() || self.document.paste_active();
        egui::TopBottomPanel::top("menu").show(context, |ui| {
            if blocked {
                ui.disable();
            }
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New Window").clicked() {
                        let result = std::env::current_exe()
                            .and_then(|executable| std::process::Command::new(executable).spawn())
                            .map(|_| ())
                            .map_err(|error| error.to_string());
                        self.result(result);
                        ui.close();
                    }
                    if ui.button("New...").clicked() {
                        self.request_new();
                        ui.close();
                    }
                    if ui.button("Open...").clicked() {
                        self.choose(context, FileAction::Open);
                        ui.close();
                    }
                    ui.menu_button("Recent Files", |ui| {
                        for path in self.settings.recent_files.files().into_iter().rev() {
                            if ui.button(path.display().to_string()).clicked() {
                                self.open(path);
                                ui.close();
                            }
                        }
                    });
                    ui.separator();
                    if ui.button("Save").clicked() {
                        self.save(context, false);
                        ui.close();
                    }
                    if ui.button("Save As...").clicked() {
                        self.save(context, true);
                        ui.close();
                    }
                    if self.animation.is_some() {
                        return;
                    }
                    if ui.button("Export...").clicked() {
                        self.dialog = Some(Dialog::Export);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("SAUCE...").clicked() {
                        self.sauce_fields = self.document.with_state(|state| {
                            let sauce = state.get_sauce_meta();
                            [
                                sauce.title.to_string(),
                                sauce.author.to_string(),
                                sauce.group.to_string(),
                                sauce.comments.iter().map(|line| line.to_string()).collect::<Vec<_>>().join("\n"),
                            ]
                        });
                        self.dialog = Some(Dialog::Sauce);
                        ui.close();
                    }
                    if ui.button("Resize...").clicked() {
                        let size = self.document.with_state(|state| state.get_buffer().size());
                        self.new_size = [size.width, size.height];
                        self.dialog = Some(Dialog::Resize);
                        ui.close();
                    }
                });
                ui.menu_button("Edit", |ui| {
                    if ui.button("Undo").clicked() {
                        self.undo(false);
                        ui.close();
                    }
                    if ui.button("Redo").clicked() {
                        self.undo(true);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Copy").clicked() {
                        self.copy(context);
                        ui.close();
                    }
                    if ui.button("Cut").clicked() && self.document.can_paint() {
                        if self.animation.is_some() {
                            context.memory_mut(|memory| memory.request_focus(egui::Id::new("animation-source-editor")));
                            context.input_mut(|input| input.events.push(egui::Event::Cut));
                        } else {
                            self.copy(context);
                            self.edit(|state| state.erase_selection());
                        }
                        ui.close();
                    }
                    if ui.button("Paste").clicked() {
                        if self.animation.is_some() {
                            context.memory_mut(|memory| memory.request_focus(egui::Id::new("animation-source-editor")));
                        }
                        context.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                        ui.close();
                    }
                    if ui.button("Select All").clicked() {
                        if self.animation.is_some() {
                            context.memory_mut(|memory| memory.request_focus(egui::Id::new("animation-source-editor")));
                            context.input_mut(|input| {
                                input.events.push(egui::Event::Key {
                                    key: Key::A,
                                    physical_key: None,
                                    pressed: true,
                                    repeat: false,
                                    modifiers: egui::Modifiers::COMMAND,
                                })
                            });
                        } else {
                            self.select_all();
                        }
                        ui.close();
                    }
                    if self.animation.is_some() {
                        return;
                    }
                    if ui.button("Deselect").clicked() {
                        self.edit(|state| state.clear_selection());
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Flip Horizontally").clicked() && self.document.can_paint() {
                        self.edit(|state| state.flip_x());
                        ui.close();
                    }
                    if ui.button("Flip Vertically").clicked() && self.document.can_paint() {
                        self.edit(|state| state.flip_y());
                        ui.close();
                    }
                    if ui.button("Crop to Selection").clicked() {
                        self.edit(|state| state.crop());
                        ui.close();
                    }
                });
                if self.animation.is_some() {
                    return;
                }
                ui.menu_button("View", |ui| {
                    ui.checkbox(&mut self.show_inspector, "Minimap & Layers");
                    ui.checkbox(&mut self.show_grid, "Character Grid");
                    ui.checkbox(&mut self.show_layer_bounds, "Layer Borders");
                    if ui.button("Monitor...").clicked() {
                        self.dialog = Some(Dialog::Monitor);
                        ui.close();
                    }
                    if ui.button("Inspector...").clicked() {
                        self.dialog = Some(Dialog::Inspector);
                        ui.close();
                    }
                    if ui.button("Characters...").clicked() {
                        self.dialog = Some(Dialog::Characters);
                        ui.close();
                    }
                    egui::widgets::global_theme_preference_buttons(ui);
                    for (label, mode) in [
                        ("Fit", ScalingMode::Auto),
                        ("Fit Width", ScalingMode::FitWidth),
                        ("100%", ScalingMode::Manual(1.0)),
                        ("200%", ScalingMode::Manual(2.0)),
                        ("400%", ScalingMode::Manual(4.0)),
                    ] {
                        if ui.button(label).clicked() {
                            self.settings.monitor_settings.scaling_mode = mode;
                            ui.close();
                        }
                    }
                });
                ui.menu_button("Extensions", |ui| {
                    if ui.button("Run Lua...").clicked() {
                        self.dialog = Some(Dialog::Script);
                        ui.close();
                    }
                    if ui.button("Reload Plugins").clicked() {
                        self.plugins = None;
                    }
                    ui.separator();
                    let mut run = None;
                    for plugin in self.plugins.get_or_insert_with(icy_draw::plugins::Plugin::read_plugin_directory) {
                        let label = plugin.path.iter().cloned().chain([plugin.title.clone()]).collect::<Vec<_>>().join(" / ");
                        if ui
                            .add_enabled(self.document.can_paint(), egui::Button::new(label))
                            .on_hover_text(format!("{}\n{}", plugin.description, plugin.author))
                            .clicked()
                        {
                            run = Some(plugin.clone());
                            ui.close();
                        }
                    }
                    if let Some(plugin) = run {
                        self.document.finish();
                        let result = plugin.run_plugin(&self.document.screen).map_err(|error| error.to_string());
                        self.result(result);
                    }
                });
            });
        });
    }

    fn tool_options(&mut self, ui: &mut egui::Ui, context: &egui::Context) {
        ui.spacing_mut().item_spacing.x = 4.0;
        if self.document.paste_active() {
            use icy_draw::document::PasteAction;
            for (icon, label, action) in [
                ("anchor", "Anchor (Enter)", PasteAction::Anchor),
                ("add_layer", "Keep as Layer", PasteAction::Keep),
                ("file_copy", "Stamp (S)", PasteAction::Stamp),
                ("replay", "Rotate (R)", PasteAction::Rotate),
                ("flip_tool", "Flip Horizontal (X)", PasteAction::FlipX),
                ("swap", "Flip Vertical (Y)", PasteAction::FlipY),
                ("invisible", "Make Transparent (T)", PasteAction::Transparent),
                ("delete", "Cancel Paste (Escape)", PasteAction::Cancel),
            ] {
                if action == PasteAction::Cancel {
                    widgets::divider(ui);
                }
                if self.icons.button(ui, icon, label, false).clicked() {
                    let result = self.document.paste_action(action);
                    self.result(result);
                    self.canvas_focus = true;
                }
            }
            return;
        }
        if self.document.outline_font && self.document.tool == Tool::Click {
            let font = self.document.with_state(|state| state.get_buffer().font(0).cloned());
            if let Some(font) = font {
                for index in 0..10 {
                    let code = char::from_u32('A' as u32 + index as u32).unwrap();
                    if widgets::fkey(ui, &font, code, index).clicked() {
                        let result = self.document.type_text(&code.to_string());
                        self.result(result);
                        self.canvas_focus = true;
                    }
                }
                widgets::divider(ui);
                for code in ['K', 'L', 'M', 'N', 'O', 'P', 'Q', '@', '&', ' ', '\u{00ff}'] {
                    if widgets::glyph(ui, &font, code, false, 28.0).clicked() {
                        let result = self.document.type_text(&code.to_string());
                        self.result(result);
                        self.canvas_focus = true;
                    }
                }
            }
            return;
        }
        let font = self
            .document
            .with_state(|state| state.get_buffer().font(state.get_caret().attribute.font_page()).cloned());
        match self.document.tool {
            tool if tool == Tool::Pencil || tool == Tool::Fill || tool.is_shape_tool() => {
                widgets::segmented(
                    ui,
                    &mut self.document.brush.primary,
                    &[
                        (BrushPrimaryMode::Char, "Char"),
                        (BrushPrimaryMode::HalfBlock, "Half Block"),
                        (BrushPrimaryMode::Shading, "Shade"),
                        (BrushPrimaryMode::Replace, "Replace"),
                        (BrushPrimaryMode::Blink, "Blink"),
                        (BrushPrimaryMode::Colorize, "Colorize"),
                    ],
                );
                widgets::divider(ui);
                if let Some(font) = &font {
                    if widgets::glyph(ui, font, self.document.brush.paint_char, false, widgets::CONTROL_HEIGHT)
                        .on_hover_text("Select Character")
                        .clicked()
                    {
                        self.dialog = Some(Dialog::Characters);
                    }
                }
                if self.document.tool == Tool::Pencil {
                    widgets::divider(ui);
                    ui.weak("Size");
                    ui.spacing_mut().item_spacing.x = 0.0;
                    if self.icons.button(ui, "arrow_left", "Smaller Brush", false).clicked() {
                        self.document.brush.brush_size = self.document.brush.brush_size.saturating_sub(1).max(1);
                    }
                    ui.add(egui::DragValue::new(&mut self.document.brush.brush_size).range(1..=9).suffix(" px"));
                    if self.icons.button(ui, "arrow_right", "Larger Brush", false).clicked() {
                        self.document.brush.brush_size = (self.document.brush.brush_size + 1).min(9);
                    }
                    ui.spacing_mut().item_spacing.x = 4.0;
                }
                widgets::divider(ui);
                widgets::toggle(ui, "FG", &mut self.document.brush.colorize_fg, "Paint the foreground color");
                widgets::toggle(ui, "BG", &mut self.document.brush.colorize_bg, "Paint the background color");
                if self.document.tool == Tool::Fill {
                    widgets::toggle(ui, "Exact", &mut self.document.brush.exact, "Only fill cells that match exactly");
                }
                let variants = match self.document.tool {
                    Tool::RectangleOutline | Tool::RectangleFilled => Some([Tool::RectangleOutline, Tool::RectangleFilled]),
                    Tool::EllipseOutline | Tool::EllipseFilled => Some([Tool::EllipseOutline, Tool::EllipseFilled]),
                    _ => None,
                };
                if let Some(variants) = variants {
                    widgets::divider(ui);
                    for tool in variants {
                        if self.icons.button(ui, tool.icon(), tool.name(), self.document.tool == tool).clicked() {
                            self.select_tool(tool);
                        }
                    }
                }
            }
            Tool::Font => {
                if let Some(library) = &self.text_fonts {
                    let mut library = library.write();
                    egui::ComboBox::from_id_salt("text-art-font")
                        .width(200.0)
                        .selected_text(library.font_name(self.text_font).unwrap_or("No fonts"))
                        .show_ui(ui, |ui| {
                            for (index, name) in library.font_names().iter().enumerate() {
                                ui.selectable_value(&mut self.text_font, index, name);
                            }
                        });
                    widgets::divider(ui);
                    ui.weak("Outline");
                    ui.add(egui::DragValue::new(&mut self.settings.font_outline_style).range(0..=18));
                    if self.text_preview.as_ref().is_none_or(|(index, _)| *index != self.text_font) {
                        if let Some(preview) = library.generate_preview(self.text_font) {
                            let texture = context.load_texture(
                                "text-art-preview",
                                egui::ColorImage::from_rgba_unmultiplied([preview.width as usize, preview.height as usize], &preview.rgba),
                                egui::TextureOptions::NEAREST,
                            );
                            self.text_preview = Some((self.text_font, texture));
                        }
                    }
                    if let Some((_, preview)) = &self.text_preview {
                        widgets::divider(ui);
                        ui.add(egui::Image::new(preview).max_height(32.0).max_width(240.0));
                    }
                }
                context.request_repaint_after(std::time::Duration::from_millis(250));
            }
            Tool::Click => {
                if let Some(font) = &font {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    if self.icons.button(ui, "navigate_prev", "Previous Character Set", false).clicked() {
                        self.settings.fkeys.current_set =
                            (self.settings.fkeys.current_set + self.settings.fkeys.set_count() - 1) % self.settings.fkeys.set_count();
                    }
                    for (index, code) in self.settings.fkeys.current_set_codes().into_iter().enumerate() {
                        let response = widgets::fkey(ui, font, char::from_u32(code as u32).unwrap_or(' '), index);
                        if response.secondary_clicked() {
                            self.dialog = Some(Dialog::FKeyCharacter(self.settings.fkeys.current_set, index));
                        }
                        if response.clicked() && self.document.can_paint() {
                            self.edit(|state| state.type_key(char::from_u32(code as u32).unwrap_or(' ')));
                            self.canvas_focus = true;
                        }
                    }
                    if self.icons.button(ui, "navigate_next", "Next Character Set", false).clicked() {
                        self.settings.fkeys.current_set = (self.settings.fkeys.current_set + 1) % self.settings.fkeys.set_count();
                    }
                    ui.add_space(4.0);
                    ui.weak(format!("Set {} / {}", self.settings.fkeys.current_set + 1, self.settings.fkeys.set_count()));
                    ui.spacing_mut().item_spacing.x = 4.0;
                    widgets::divider(ui);
                    if self.icons.button(ui, "font", "Character Table", false).clicked() {
                        self.dialog = Some(Dialog::Characters);
                    }
                }
            }
            Tool::Select => {
                use icy_draw::document::SelectionMode;
                let mut mode = self.document.selection_mode;
                if widgets::segmented(
                    ui,
                    &mut mode,
                    &[
                        (SelectionMode::Rectangle, "Rect"),
                        (SelectionMode::Character, "Char"),
                        (SelectionMode::Attribute, "Attr"),
                        (SelectionMode::Foreground, "Fg"),
                        (SelectionMode::Background, "Bg"),
                    ],
                ) {
                    self.document.finish();
                    self.document.selection_mode = mode;
                }
                widgets::divider(ui);
                let selected = self.document.with_state(|state| state.is_something_selected());
                if self.icons.button(ui, "select", "Select All", false).clicked() {
                    self.select_all();
                }
                ui.add_enabled_ui(selected, |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    if self.icons.button(ui, "file_copy", "Copy Selection", false).clicked() {
                        self.copy(context);
                    }
                    ui.add_enabled_ui(self.document.can_paint(), |ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        if self.icons.button(ui, "flip_tool", "Flip Horizontally", false).clicked() {
                            self.edit(|state| state.flip_x());
                        }
                        if self.icons.button(ui, "flip_tool", "Flip Vertically", false).clicked() {
                            self.edit(|state| state.flip_y());
                        }
                    });
                    if self.icons.button(ui, "delete", "Deselect", false).clicked() {
                        self.edit(|state| state.clear_selection());
                    }
                });
                if let Some(bounds) = self.document.with_state(|state| state.selection().map(|selection| selection.as_rectangle())) {
                    widgets::divider(ui);
                    ui.weak(format!("{}, {}  ·  {} × {}", bounds.left(), bounds.top(), bounds.width(), bounds.height()));
                }
            }
            Tool::Tag => {
                if self.icons.button(ui, "tag", "Tag List", false).clicked() {
                    self.dialog = Some(Dialog::Tags);
                }
                if self.icons.button(ui, "add", "New Tag", false).clicked() {
                    self.open_tag_properties(None);
                }
                ui.add_enabled_ui(!self.document.selected_tags.is_empty(), |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    if self.icons.button(ui, "text", "Edit Tag", false).clicked() {
                        self.open_tag_properties(self.document.selected_tags.first().copied());
                    }
                    if self.icons.button(ui, "delete", "Delete Selected Tags", false).clicked() {
                        let result = self.document.delete_selected_tags();
                        self.result(result);
                    }
                });
                widgets::divider(ui);
                ui.weak(format!("{} selected", self.document.selected_tags.len()));
            }
            _ => {}
        }
    }

    fn palette(&mut self, ui: &mut egui::Ui) {
        let (palette, foreground, background) = self.document.with_state(|state| {
            (
                state.get_buffer().palette.clone(),
                state.get_caret().attribute.foreground(),
                state.get_caret().attribute.background(),
            )
        });
        ui.horizontal_wrapped(|ui| {
            for index in 0..palette.len().min(256) {
                let (red, green, blue) = palette.rgb(index as u32);
                let response = widgets::swatch(ui, Color32::from_rgb(red, green, blue), foreground == index as u32, 22.0)
                    .on_hover_text(format!("{index}: #{red:02X}{green:02X}{blue:02X}"));
                if response.clicked() {
                    self.document.with_state(|state| state.set_caret_foreground(index as u32));
                }
                if response.secondary_clicked() {
                    self.document.with_state(|state| state.set_caret_background(index as u32));
                }
                if background == index as u32 {
                    ui.painter().circle_filled(response.rect.center(), 2.0, Color32::WHITE);
                }
            }
            if self.icons.button(ui, "swap", "Swap Foreground and Background", false).clicked() {
                self.document.with_state(|state| state.swap_caret_colors());
            }
        });
    }

    fn inspector(&mut self, ui: &mut egui::Ui) {
        ui.strong("Palette");
        self.palette(ui);
        if ui.button("Edit Palette...").clicked() {
            self.palette_edit = self.document.with_state(|state| state.get_buffer().palette.clone());
            self.palette_index = 0;
            self.dialog = Some(Dialog::Palette);
        }
        ui.separator();
        ui.strong("Layers");
        let (layers, current) = self.document.with_state(|state| {
            (
                state
                    .get_buffer()
                    .layers
                    .iter()
                    .map(|layer| (layer.properties.clone(), layer.is_visible(), layer.offset()))
                    .collect::<Vec<_>>(),
                state.get_current_layer().unwrap_or(0),
            )
        });
        for (index, (properties, visible, _offset)) in layers.iter().enumerate().rev() {
            ui.horizontal(|ui| {
                let mut properties = properties.clone();
                let mut visible = *visible;
                if ui.checkbox(&mut visible, "").changed() {
                    self.edit(|state| state.toggle_layer_visibility(index));
                }
                if ui.selectable_label(index == current, &properties.title).clicked() {
                    self.document.with_state(|state| state.set_current_layer(index));
                }
                if ui.checkbox(&mut properties.is_locked, "").on_hover_text("Lock Layer").changed() {
                    self.edit(|state| state.update_layer_properties(index, properties));
                }
            });
        }
        ui.horizontal_wrapped(|ui| {
            if self.icons.button(ui, "add_layer", "Add Layer", false).clicked() {
                self.edit(|state| state.add_new_layer(current));
            }
            if self.icons.button(ui, "file_copy", "Duplicate Layer", false).clicked() {
                self.edit(|state| state.duplicate_layer(current));
            }
            if self.icons.button(ui, "up", "Raise Layer", false).clicked() && current + 1 < layers.len() {
                self.edit(|state| state.raise_layer(current));
            }
            if self.icons.button(ui, "down", "Lower Layer", false).clicked() {
                self.edit(|state| state.lower_layer(current));
            }
            if self.icons.button(ui, "delete", "Delete Layer", false).clicked() && layers.len() > 1 {
                self.edit(|state| state.remove_layer(current));
            }
            if self.icons.button(ui, "anchor", "Anchor Pasted Layer", false).clicked() {
                self.edit(|state| state.anchor_layer());
            }
        });
        if let Some((properties, _visible, offset)) = layers.get(current) {
            let mut title = properties.title.clone();
            if ui.text_edit_singleline(&mut title).changed() {
                let mut properties = properties.clone();
                properties.title = title;
                self.edit(|state| state.update_layer_properties(current, properties));
            }
            let mut offset = *offset;
            ui.horizontal(|ui| {
                if properties.is_position_locked {
                    ui.disable();
                }
                let changed =
                    ui.add(egui::DragValue::new(&mut offset.x).prefix("X ")).changed() | ui.add(egui::DragValue::new(&mut offset.y).prefix("Y ")).changed();
                if changed {
                    self.edit(|state| state.move_layer(offset));
                }
            });
        }
        ui.separator();
        ui.strong("Character");
        let mut code = self.document.brush.paint_char as u32;
        if ui.add(egui::DragValue::new(&mut code).range(0..=255).prefix("Code ")).changed() {
            self.document.brush.paint_char = char::from_u32(code).unwrap_or(' ');
        }
        if ui.button("Characters...").clicked() {
            self.dialog = Some(Dialog::Characters);
        }
        if ui.button("Select Font...").clicked() {
            self.dialog = Some(Dialog::FontSelect);
        }
        if ui.button("Edit Bitmap Font...").clicked() {
            if let Some(font) = self
                .document
                .with_state(|state| state.get_buffer().font(state.get_caret().attribute.font_page()).cloned())
            {
                if font.size().width <= 8 {
                    self.font_editor = Some(super::font::FontEditor::new(font));
                    self.dialog = Some(Dialog::Font);
                } else {
                    self.dialog = Some(Dialog::Error(
                        "The bitmap editing backend currently supports glyphs up to 8 pixels wide.".into(),
                    ));
                }
            }
        }
        let mut mirror = self.document.with_state(|state| state.get_mirror_mode());
        if ui.checkbox(&mut mirror, "Mirror").changed() {
            self.document.with_state(|state| state.set_mirror_mode(mirror));
        }
        let (mut spacing, mut aspect, mut ice) = self.document.with_state(|state| {
            (
                state.get_buffer().use_letter_spacing(),
                state.get_buffer().use_aspect_ratio(),
                state.get_buffer().ice_mode,
            )
        });
        if ui.checkbox(&mut spacing, "9-pixel letter spacing").changed() {
            self.edit(|state| state.set_use_letter_spacing(spacing));
        }
        if ui.checkbox(&mut aspect, "Legacy aspect ratio").changed() {
            self.edit(|state| state.set_use_aspect_ratio(aspect));
        }
        egui::ComboBox::from_id_salt("ice-mode").selected_text(format!("{ice:?}")).show_ui(ui, |ui| {
            for mode in [icy_engine::IceMode::Blink, icy_engine::IceMode::Ice, icy_engine::IceMode::Unlimited] {
                if ui.selectable_value(&mut ice, mode, format!("{mode:?}")).changed() {
                    self.edit(|state| state.set_ice_mode(mode));
                }
            }
        });
    }

    fn half_blocks(&self) -> bool {
        self.document.brush.primary == BrushPrimaryMode::HalfBlock
            && (self.document.tool == Tool::Pencil || self.document.tool == Tool::Fill || self.document.tool.is_shape_tool())
    }

    fn position(&self, point: egui::Pos2) -> Option<Position> {
        let info = self.view.terminal.render_info.read();
        let (horizontal, vertical) = info.screen_to_terminal_pixels(point.x, point.y)?;
        let vertical = if info.scan_lines { vertical / 2.0 } else { vertical };
        let height = info.font_height / if self.half_blocks() { 2.0 } else { 1.0 };
        let position = Position::new(
            ((horizontal + self.view.terminal.scroll_x()) / info.font_width.max(1.0)).floor() as i32,
            ((vertical + self.view.terminal.scroll_y()) / height.max(1.0)).floor() as i32,
        );
        let size = self.document.with_state(|state| state.get_buffer().size());
        (position.x >= 0 && position.x < size.width && position.y >= 0 && position.y < size.height * if self.half_blocks() { 2 } else { 1 }).then_some(position)
    }

    fn canvas(&mut self, ui: &mut egui::Ui, blocked: bool) {
        self.view.terminal.has_focus = self.canvas_focus && !blocked;
        self.document
            .with_state(|state| state.set_caret_visible(!self.document.paste_active() && matches!(self.document.tool, Tool::Click | Tool::Font)));
        let zoom_delta = ui.input_mut(|input| {
            if !blocked && input.modifiers.command && input.pointer.hover_pos().is_some_and(|point| ui.max_rect().contains(point)) {
                let delta = input.smooth_scroll_delta.y;
                input.smooth_scroll_delta = egui::Vec2::ZERO;
                input.raw_scroll_delta = egui::Vec2::ZERO;
                delta
            } else {
                0.0
            }
        });
        if zoom_delta != 0.0 {
            self.settings.monitor_settings.scaling_mode = ScalingMode::Manual((self.view.zoom * (zoom_delta * 0.002).exp()).clamp(0.25, 8.0));
        }
        self.view.markers = Some(self.editor_markers());
        let response = self.view.show(ui, &self.settings.monitor_settings);
        self.canvas_rect = response.rect;
        if ui.ctx().wants_keyboard_input() && !response.has_focus() {
            self.canvas_focus = false;
        }
        let info = self.view.terminal.render_info.read().clone();
        let (red, green, blue) = self.document.preview_color();
        let cell_size = egui::vec2(
            info.font_width,
            info.font_height * if info.scan_lines { 2.0 } else { 1.0 } / if self.half_blocks() { 2.0 } else { 1.0 },
        );
        let origin = egui::pos2(info.bounds_x + info.viewport_x, info.bounds_y + info.viewport_y)
            - egui::vec2(
                self.view.terminal.scroll_x(),
                self.view.terminal.scroll_y() * if info.scan_lines { 2.0 } else { 1.0 },
            ) * info.display_scale;
        let painter = ui.painter().with_clip_rect(response.rect);
        if self.show_grid && cell_size.x * info.display_scale >= 8.0 {
            let step = cell_size * info.display_scale;
            let start = ((response.rect.min - origin) / step).floor();
            let count = (response.rect.size() / step).ceil();
            for column in 0..=count.x as i32 + 1 {
                let horizontal = origin.x + (start.x + column as f32) * step.x;
                painter.line_segment(
                    [egui::pos2(horizontal, response.rect.top()), egui::pos2(horizontal, response.rect.bottom())],
                    egui::Stroke::new(0.5, Color32::from_white_alpha(45)),
                );
            }
            for row in 0..=count.y as i32 + 1 {
                let vertical = origin.y + (start.y + row as f32) * step.y;
                painter.line_segment(
                    [egui::pos2(response.rect.left(), vertical), egui::pos2(response.rect.right(), vertical)],
                    egui::Stroke::new(0.5, Color32::from_white_alpha(45)),
                );
            }
        }
        for point in &self.document.preview {
            let rect = egui::Rect::from_min_size(
                origin + egui::vec2(point.x as f32 * cell_size.x, point.y as f32 * cell_size.y) * info.display_scale,
                cell_size * info.display_scale,
            );
            painter.rect_filled(rect, 0, Color32::from_rgba_unmultiplied(red, green, blue, 160));
        }
        if self.document.tool == Tool::Tag {
            if let Some(selection) = self.document.tag_selection_rectangle() {
                let rect = egui::Rect::from_min_size(
                    origin + egui::vec2(selection.left() as f32 * cell_size.x, selection.top() as f32 * cell_size.y) * info.display_scale,
                    egui::vec2(selection.width() as f32 * cell_size.x, selection.height() as f32 * cell_size.y) * info.display_scale,
                );
                painter.rect_stroke(rect, 0, ui.visuals().selection.stroke, egui::StrokeKind::Inside);
            }
            let tags = self.document.with_state(|state| state.get_buffer().tags.clone());
            for (index, tag) in tags.iter().enumerate() {
                let position = self.document.tag_preview_position(index, tag.position);
                let rect = egui::Rect::from_min_size(
                    origin + egui::vec2(position.x as f32 * cell_size.x, position.y as f32 * cell_size.y) * info.display_scale,
                    egui::vec2(tag.length.max(1) as f32 * cell_size.x, cell_size.y) * info.display_scale,
                );
                let color = if self.document.selected_tags.contains(&index) {
                    ui.visuals().selection.stroke.color
                } else {
                    Color32::from_rgb(90, 190, 160)
                };
                painter.rect_stroke(rect, 0, egui::Stroke::new(2.0, color), egui::StrokeKind::Inside);
            }
        }
        if blocked {
            return;
        }
        let pointer = ui.input(|input| input.pointer.clone());
        let pressed = pointer.button_pressed(egui::PointerButton::Primary) || pointer.button_pressed(egui::PointerButton::Secondary);
        if response.hovered() && pressed {
            self.canvas_focus = true;
            response.request_focus();
            if let Some(position) = pointer.interact_pos().and_then(|point| self.position(point)) {
                if self.document.tool == Tool::Pipette {
                    let modifiers = ui.input(|input| input.modifiers);
                    self.document.begin_with_modifiers(
                        position,
                        if pointer.secondary_down() {
                            icy_engine::MouseButton::Right
                        } else {
                            icy_engine::MouseButton::Left
                        },
                        icy_engine::KeyModifiers {
                            shift: modifiers.shift,
                            ctrl: modifiers.ctrl,
                            alt: modifiers.alt,
                            meta: modifiers.mac_cmd,
                        },
                    );
                } else {
                    let button = if pointer.button_pressed(egui::PointerButton::Secondary) {
                        icy_engine::MouseButton::Right
                    } else {
                        icy_engine::MouseButton::Left
                    };
                    let modifiers = ui.input(|input| input.modifiers);
                    self.document.begin_with_modifiers(
                        position,
                        button,
                        icy_engine::KeyModifiers {
                            shift: modifiers.shift,
                            ctrl: modifiers.ctrl,
                            alt: modifiers.alt,
                            meta: modifiers.mac_cmd,
                        },
                    );
                }
            }
        }
        if self.document.stroke_active() {
            if let Some(position) = pointer.interact_pos().and_then(|point| self.position(point)) {
                self.document.update(position);
            }
            if !pointer.any_down() {
                self.document.finish();
            }
        }
        if self.document.tool == Tool::Tag && response.double_clicked() && !self.document.selected_tags.is_empty() {
            self.document.finish();
            self.open_tag_properties(self.document.selected_tags.first().copied());
        }
        if self.document.tool == Tool::Tag {
            response.context_menu(|ui| {
                if ui.button("New Tag...").clicked() {
                    self.open_tag_properties(None);
                    ui.close();
                }
                ui.add_enabled_ui(!self.document.selected_tags.is_empty(), |ui| {
                    if ui.button("Properties...").clicked() {
                        self.open_tag_properties(self.document.selected_tags.first().copied());
                        ui.close();
                    }
                    if ui.button("Duplicate").clicked() {
                        self.document.finish();
                        let selected = self.document.selected_tags.clone();
                        self.edit(|state| {
                            let _undo = state.begin_atomic_undo("Duplicate tags");
                            for index in selected {
                                state.clone_tag(index)?;
                            }
                            Ok(())
                        });
                        ui.close();
                    }
                    if ui.button("Delete").clicked() {
                        let result = self.document.delete_selected_tags();
                        self.result(result);
                        ui.close();
                    }
                });
            });
        }
        if pointer.any_pressed() && !response.hovered() {
            self.canvas_focus = false;
        }
    }

    fn copy(&mut self, context: &egui::Context) {
        if self.animation.is_some() {
            context.memory_mut(|memory| memory.request_focus(egui::Id::new("animation-source-editor")));
            context.input_mut(|input| input.events.push(egui::Event::Copy));
            return;
        }
        let screen = self.document.screen.lock();
        if let Some(text) = screen.copy_text() {
            self.clipboard = screen.clipboard_data().map(|data| (text.clone(), data));
            context.copy_text(text);
        }
    }

    fn open_tag_properties(&mut self, index: Option<usize>) {
        self.document.finish();
        let tag = self.document.with_state(|state| {
            index
                .and_then(|index| state.get_buffer().tags.get(index).cloned())
                .unwrap_or_else(|| icy_engine::Tag {
                    is_enabled: true,
                    preview: "TAG".into(),
                    replacement_value: String::new(),
                    position: state.layer_to_document_position(state.get_caret().position()),
                    length: 3,
                    alignment: std::fmt::Alignment::Left,
                    tag_placement: icy_engine::TagPlacement::InText,
                    tag_role: icy_engine::TagRole::Displaycode,
                    attribute: state.get_caret().attribute,
                })
        });
        self.dialog = Some(Dialog::TagProperties(index, Box::new(tag)));
        self.canvas_focus = false;
    }

    fn paste(&mut self, text: &str) {
        let data = self.clipboard.as_ref().filter(|(copied, _)| copied == text).map(|(_, data)| data.as_slice());
        let result = self.document.start_paste(text, data);
        self.result(result);
    }

    fn select_all(&mut self) {
        self.edit(|state| {
            let mut selection = Selection::new(Position::new(0, 0));
            selection.lead = Position::new(state.get_buffer().width() - 1, state.get_buffer().height() - 1);
            selection.shape = icy_engine::Shape::Rectangle;
            state.set_selection(selection)
        });
    }

    fn request_new(&mut self) {
        self.document.finish();
        self.pending = None;
        self.quitting = false;
        self.dialog = Some(if self.modified() { Dialog::Close } else { Dialog::New });
    }

    fn keys(&mut self, context: &egui::Context) {
        if self.dialog.is_some() || self.picker || self.layer_properties_open() {
            return;
        }
        let before = self.document.with_state(|state| state.layer_to_document_position(state.get_caret().position()));
        let events = context.input(|input| input.events.clone());
        let hard_blank = events.iter().any(|event| {
            matches!(event, egui::Event::Key {
            key: Key::Space, pressed: true, modifiers, ..
        } if modifiers.shift && !modifiers.alt && if self.document.outline_font { modifiers.command || modifiers.ctrl } else { !modifiers.command })
        });
        for event in events {
            match event {
                egui::Event::Key {
                    key, pressed: true, modifiers, ..
                } if modifiers.command && !modifiers.alt => match key {
                    Key::O if !modifiers.shift && !modifiers.alt => self.choose(context, FileAction::Open),
                    Key::N if !modifiers.shift && !modifiers.alt => self.request_new(),
                    Key::S if !modifiers.alt => self.save(context, modifiers.shift),
                    Key::Z if self.canvas_focus => self.undo(modifiers.shift),
                    Key::Y if self.canvas_focus => self.undo(true),
                    Key::A if self.canvas_focus && !self.document.paste_active() => self.select_all(),
                    _ if self.canvas_focus => {
                        let result = super::input::key(&mut self.document, &mut self.settings.fkeys, key, modifiers);
                        self.result(result.map(|_| ()));
                    }
                    _ => {}
                },
                egui::Event::Copy if self.canvas_focus => self.copy(context),
                egui::Event::Cut if self.canvas_focus && self.document.can_paint() => {
                    self.copy(context);
                    self.edit(|state| state.erase_selection());
                }
                egui::Event::Paste(text) if self.canvas_focus && self.document.can_paint() => self.paste(&text),
                egui::Event::Text(text) if self.canvas_focus && self.document.tool == Tool::Click && !self.document.paste_active() => {
                    if !(hard_blank && text == " ") {
                        let result = self.document.type_text(&text);
                        self.result(result);
                    }
                }
                egui::Event::Text(text) if self.canvas_focus && self.document.tool == Tool::Font => {
                    let result = self.type_art_text(&text);
                    self.result(result);
                }
                egui::Event::Key {
                    key, pressed: true, modifiers, ..
                } if self.canvas_focus => {
                    if key == Key::Enter && self.document.tool == Tool::Font && !modifiers.alt {
                        let result = self.type_art_text("\n");
                        self.result(result);
                    } else {
                        let result = super::input::key(&mut self.document, &mut self.settings.fkeys, key, modifiers);
                        self.result(result.map(|_| ()));
                    }
                }
                _ => {}
            }
            if self.dialog.is_some() || self.picker || self.layer_properties_open() {
                break;
            }
        }
        let after = self.document.with_state(|state| state.layer_to_document_position(state.get_caret().position()));
        if after != before {
            self.reveal_caret(after);
        }
    }

    fn reveal_caret(&mut self, position: Position) {
        let info = self.view.terminal.render_info.read();
        let cell = egui::vec2(info.font_width, info.font_height * if info.scan_lines { 2.0 } else { 1.0 }) * self.view.zoom;
        let start = egui::vec2(position.x as f32, position.y as f32) * cell;
        let end = start + cell;
        let viewport = self.canvas_rect.size();
        let mut offset = self.view.offset;
        for axis in 0..2 {
            if start[axis] < offset[axis] {
                offset[axis] = start[axis];
            } else if end[axis] > offset[axis] + viewport[axis] {
                offset[axis] = end[axis] - viewport[axis];
            }
        }
        offset = offset.max(egui::Vec2::ZERO).min(self.view.max_offset);
        if offset != self.view.offset {
            self.view.scroll_to = Some(offset);
        }
    }

    fn type_art_text(&mut self, text: &str) -> Result<(), String> {
        let Some(library) = &self.text_fonts else {
            return Ok(());
        };
        let library = library.read();
        let Some(font) = library.get_font(self.text_font) else {
            return Ok(());
        };
        self.document.type_art_text(text, font, self.settings.font_outline_style)
    }

    fn dialogs(&mut self, context: &egui::Context) {
        let Some(dialog) = self.dialog.take() else {
            return;
        };
        let mut keep = true;
        match &dialog {
            Dialog::Monitor => {
                #[derive(Clone, Copy)]
                enum Action {
                    SaveDefaults,
                    Reset,
                    Close,
                }
                let response = appearance::Dialog::new("monitor").size(DialogSize::Medium).show(context, |dialog| {
                    dialog.content(|ui| icy_engine_gui::egui::monitor::fields(ui, &mut self.settings.monitor_settings));
                    dialog.buttons([
                        DialogButton::secondary("Save Defaults", Action::SaveDefaults)
                            .leading()
                            .enabled(self.persist_settings),
                        DialogButton::secondary(labels::restore_defaults(), Action::Reset).leading(),
                        DialogButton::primary(labels::close(), Action::Close).cancels(),
                    ]);
                });
                match response.action {
                    Some(Action::SaveDefaults) => self.settings.store_persistent(),
                    Some(Action::Reset) => {
                        self.settings.monitor_settings = icy_engine_gui::MonitorSettings::default();
                        self.settings.monitor_settings.scaling_mode = ScalingMode::Manual(2.0);
                    }
                    Some(Action::Close) => keep = false,
                    None => keep &= !response.dismissed,
                }
            }
            Dialog::Script => {
                #[derive(Clone, Copy)]
                enum Action {
                    Close,
                    Run,
                }
                let response = appearance::Dialog::new("lua-script")
                    .size(DialogSize::XLarge)
                    .fixed_height(460.0)
                    .show(context, |dialog| {
                        dialog.content(|ui| {
                            appearance::group(ui, "", |ui| {
                                ui.add(
                                    egui::TextEdit::multiline(&mut self.script)
                                        .code_editor()
                                        .desired_width(f32::INFINITY)
                                        .desired_rows(12),
                                );
                                if !self.script_output.is_empty() {
                                    ui.separator();
                                    ui.add(egui::Label::new(&self.script_output).wrap().selectable(true));
                                }
                            });
                        });
                        dialog.buttons([
                            DialogButton::cancel(labels::close(), Action::Close),
                            DialogButton::primary("Run", Action::Run).enabled(self.document.can_paint() && !self.picker),
                        ]);
                    });
                match response.action {
                    Some(Action::Run) => {
                        self.document.finish();
                        self.script_output =
                            icy_draw::plugins::Plugin::run_script_string(&self.document.screen, &self.script, "Lua script").unwrap_or_else(|error| error);
                    }
                    Some(Action::Close) => keep = false,
                    None => keep &= !response.dismissed,
                }
            }
            Dialog::Tags => {
                let mut edit = None;
                let mut delete = None;
                let mut add = false;
                let response = appearance::Dialog::new("tags").size(DialogSize::Large).show(context, |dialog| {
                    dialog.content(|ui| {
                        appearance::group(ui, "", |ui| {
                            let tags = self.document.with_state(|state| state.get_buffer().tags.clone());
                            egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                                for (index, tag) in tags.iter().enumerate() {
                                    ui.push_id(index, |ui| {
                                        ui.horizontal_wrapped(|ui| {
                                            let response = ui.selectable_label(self.document.selected_tags.contains(&index), &tag.preview);
                                            if response.clicked() {
                                                self.document.selected_tags = vec![index];
                                            }
                                            if response.double_clicked() {
                                                edit = Some(index);
                                            }
                                            ui.weak(format!("{}, {}", tag.position.x, tag.position.y));
                                            if self.icons.button(ui, "text", "Edit Tag", false).clicked() {
                                                edit = Some(index);
                                            }
                                            if self.icons.button(ui, "delete", "Delete Tag", false).clicked() {
                                                delete = Some(index);
                                            }
                                        });
                                    });
                                }
                            });
                            if self.icons.button(ui, "add", "Add Tag", false).clicked() {
                                add = true;
                            }
                        });
                    });
                    dialog.buttons([DialogButton::primary(labels::close(), ()).cancels()]);
                });
                if response.action.is_some() || response.dismissed {
                    keep = false;
                }
                if let Some(index) = delete {
                    self.document.selected_tags = vec![index];
                    let result = self.document.delete_selected_tags();
                    self.result(result);
                } else if add || edit.is_some() {
                    self.open_tag_properties(edit);
                    keep = false;
                }
            }
            Dialog::TagProperties(index, draft) => {
                #[derive(Clone, Copy)]
                enum Action {
                    Cancel,
                    Apply,
                }
                let mut tag = *draft.clone();
                let mut apply = false;
                let response = appearance::Dialog::new("tag-properties")
                    .size(DialogSize::Width(440.0))
                    .confirm_on_enter(true)
                    .show(context, |dialog| {
                        dialog.content(|ui| {
                            appearance::group(ui, "", |ui| {
                                ui.checkbox(&mut tag.is_enabled, "Enabled");
                                ui.horizontal_wrapped(|ui| {
                                    ui.add(egui::DragValue::new(&mut tag.position.x).range(0..=10000).prefix("X "));
                                    ui.add(egui::DragValue::new(&mut tag.position.y).range(0..=10000).prefix("Y "));
                                    ui.add(egui::DragValue::new(&mut tag.length).range(1..=1000).prefix("Length "));
                                });
                                ui.label("Preview");
                                ui.add(egui::TextEdit::singleline(&mut tag.preview).desired_width(f32::INFINITY));
                                ui.label("Replacement");
                                ui.add(egui::TextEdit::singleline(&mut tag.replacement_value).desired_width(f32::INFINITY));
                                egui::ComboBox::from_label("Alignment")
                                    .selected_text(format!("{:?}", tag.alignment))
                                    .show_ui(ui, |ui| {
                                        for alignment in [std::fmt::Alignment::Left, std::fmt::Alignment::Center, std::fmt::Alignment::Right] {
                                            ui.selectable_value(&mut tag.alignment, alignment, format!("{alignment:?}"));
                                        }
                                    });
                                egui::ComboBox::from_label("Role")
                                    .selected_text(format!("{:?}", tag.tag_role))
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut tag.tag_role, icy_engine::TagRole::Displaycode, "Display Code");
                                        ui.selectable_value(&mut tag.tag_role, icy_engine::TagRole::Hyperlink, "Hyperlink");
                                    });
                                egui::ComboBox::from_label("Placement")
                                    .selected_text(format!("{:?}", tag.tag_placement))
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut tag.tag_placement, icy_engine::TagPlacement::InText, "In Text");
                                        ui.selectable_value(&mut tag.tag_placement, icy_engine::TagPlacement::WithGotoXY, "Cursor Position");
                                    });
                            });
                        });
                        dialog.buttons([
                            DialogButton::cancel(labels::cancel(), Action::Cancel),
                            DialogButton::primary("Apply", Action::Apply),
                        ]);
                    });
                match response.action {
                    Some(Action::Apply) => apply = true,
                    Some(Action::Cancel) => keep = false,
                    None => keep &= !response.dismissed,
                }
                if apply {
                    if let Some(index) = index {
                        self.edit(|state| state.update_tag(tag, *index));
                    } else {
                        self.edit(|state| state.add_new_tag(tag));
                    }
                    keep = false;
                } else if keep {
                    self.dialog = Some(Dialog::TagProperties(*index, Box::new(tag)));
                }
                if !keep {
                    self.canvas_focus = true;
                }
            }
            Dialog::FontSelect => {
                let response = appearance::Dialog::new("font-select")
                    .size(DialogSize::Medium)
                    .fixed_height(380.0)
                    .show(context, |dialog| {
                        dialog.content(|ui| {
                            appearance::group(ui, "", |ui| {
                                ui.add(egui::TextEdit::singleline(&mut self.font_filter).hint_text("Filter"));
                                let filter = self.font_filter.to_lowercase();
                                let current = self.document.with_state(|state| {
                                    state
                                        .get_buffer()
                                        .font(state.get_caret().attribute.font_page())
                                        .map(|font| font.name().to_string())
                                        .unwrap_or_default()
                                });
                                let names: Vec<_> = icy_engine::get_sauce_font_names()
                                    .into_iter()
                                    .filter(|name| name.to_lowercase().contains(&filter))
                                    .collect();
                                egui::ScrollArea::vertical().max_height(280.0).show_rows(ui, 24.0, names.len(), |ui, rows| {
                                    for row in rows {
                                        let name = names[row];
                                        if ui.selectable_label(name == current, name).clicked() {
                                            self.edit(|state| state.set_sauce_font(name));
                                            keep = false;
                                        }
                                    }
                                });
                            });
                        });
                        dialog.buttons([DialogButton::cancel(labels::cancel(), ())]);
                    });
                if response.action.is_some() || response.dismissed {
                    keep = false;
                }
            }
            Dialog::AnimationOverwrite(path, format) => {
                #[derive(Clone, Copy)]
                enum Action {
                    Cancel,
                    Overwrite,
                }
                let response = MessageBox::new("animation-overwrite", MessageKind::Warning, "Replace Export File?", path.display().to_string())
                    .buttons([
                        DialogButton::cancel(labels::cancel(), Action::Cancel),
                        DialogButton::destructive(labels::overwrite(), Action::Overwrite),
                    ])
                    .show(context);
                match response.action {
                    Some(Action::Overwrite) => {
                        if let Some(editor) = &mut self.animation {
                            editor.export(path.clone(), *format, context.clone());
                        }
                        keep = false;
                    }
                    Some(Action::Cancel) => keep = false,
                    None => keep &= !response.dismissed,
                }
            }
            Dialog::Palette => {
                #[derive(Clone, Copy)]
                enum Action {
                    Restore,
                    Cancel,
                    Apply,
                }
                let response = appearance::Dialog::new("palette-edit").size(DialogSize::Large).show(context, |dialog| {
                    dialog.content(|ui| {
                        appearance::group(ui, "", |ui| {
                            ui.horizontal_wrapped(|ui| {
                                for index in 0..self.palette_edit.len().min(256) {
                                    let (red, green, blue) = self.palette_edit.rgb(index as u32);
                                    if widgets::swatch(ui, Color32::from_rgb(red, green, blue), index == self.palette_index, 28.0).clicked() {
                                        self.palette_index = index;
                                    }
                                }
                            });
                            let (red, green, blue) = self.palette_edit.rgb(self.palette_index as u32);
                            let mut rgb = [red, green, blue];
                            if ui.color_edit_button_srgb(&mut rgb).changed() {
                                self.palette_edit
                                    .set_color(self.palette_index as u32, icy_engine::Color::new(rgb[0], rgb[1], rgb[2]));
                            }
                            ui.horizontal(|ui| {
                                let mut changed = false;
                                for (index, channel) in ["R ", "G ", "B "].into_iter().enumerate() {
                                    changed |= ui.add(egui::DragValue::new(&mut rgb[index]).range(0..=255).prefix(channel)).changed();
                                }
                                if changed {
                                    self.palette_edit
                                        .set_color(self.palette_index as u32, icy_engine::Color::new(rgb[0], rgb[1], rgb[2]));
                                }
                            });
                        });
                    });
                    dialog.buttons([
                        DialogButton::secondary("Restore DOS Palette", Action::Restore).leading(),
                        DialogButton::cancel(labels::cancel(), Action::Cancel),
                        DialogButton::primary("Apply", Action::Apply),
                    ]);
                });
                match response.action {
                    Some(Action::Restore) => {
                        self.palette_edit = icy_engine::Palette::default();
                        self.palette_index = 0;
                    }
                    Some(Action::Apply) => {
                        let palette = self.palette_edit.clone();
                        self.edit(|state| state.switch_to_palette(palette));
                        keep = false;
                    }
                    Some(Action::Cancel) => keep = false,
                    None => keep &= !response.dismissed,
                }
            }
            Dialog::Font => {
                if let Some(editor) = &mut self.font_editor {
                    match editor.show(context, self.picker) {
                        Some(super::font::Action::Apply(font)) => {
                            match self.document.with_state(|state| state.set_font(font)) {
                                Ok(()) => self.font_editor = None,
                                Err(error) => self.dialog = Some(Dialog::Error(error.to_string())),
                            }
                            keep = false;
                        }
                        Some(super::font::Action::Save) => {
                            self.choose(context, FileAction::SaveFont);
                        }
                        Some(super::font::Action::Close) => {
                            self.font_editor = None;
                            keep = false;
                        }
                        None => {}
                    }
                } else {
                    keep = false;
                }
            }
            Dialog::FontOverwrite(path) => {
                #[derive(Clone, Copy)]
                enum Action {
                    Cancel,
                    Overwrite,
                }
                let response = MessageBox::new("font-overwrite", MessageKind::Warning, "Replace Font File?", path.display().to_string())
                    .buttons([
                        DialogButton::cancel(labels::cancel(), Action::Cancel),
                        DialogButton::destructive(labels::overwrite(), Action::Overwrite),
                    ])
                    .show(context);
                match response.action {
                    Some(Action::Overwrite) => {
                        if let Some(editor) = &mut self.font_editor {
                            let result = editor.save(path, true);
                            self.result(result);
                        }
                        if self.dialog.is_none() {
                            self.dialog = Some(Dialog::Font);
                        }
                        keep = false;
                    }
                    Some(Action::Cancel) | None => {
                        if response.action.is_some() || response.dismissed {
                            self.dialog = Some(Dialog::Font);
                            keep = false;
                        }
                    }
                }
            }
            Dialog::ExportOverwrite(path) => {
                #[derive(Clone, Copy)]
                enum Action {
                    Cancel,
                    Overwrite,
                }
                let response = MessageBox::new("export-overwrite", MessageKind::Warning, "Replace Export File?", path.display().to_string())
                    .buttons([
                        DialogButton::cancel(labels::cancel(), Action::Cancel),
                        DialogButton::destructive(labels::overwrite(), Action::Overwrite),
                    ])
                    .show(context);
                match response.action {
                    Some(Action::Overwrite) => {
                        self.export_path(path.clone());
                        keep = false;
                    }
                    Some(Action::Cancel) => keep = false,
                    None => keep &= !response.dismissed,
                }
            }
            Dialog::Sauce => {
                #[derive(Clone, Copy)]
                enum Action {
                    Cancel,
                    Apply,
                }
                let response = appearance::Dialog::new("sauce").size(DialogSize::Medium).show(context, |dialog| {
                    dialog.content(|ui| {
                        appearance::group(ui, "", |ui| {
                            for (index, label) in ["Title", "Author", "Group"].into_iter().enumerate() {
                                ui.label(label);
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.sauce_fields[index])
                                        .char_limit(if index == 0 { 35 } else { 20 })
                                        .desired_width(f32::INFINITY),
                                );
                            }
                            ui.label("Comments");
                            ui.add(
                                egui::TextEdit::multiline(&mut self.sauce_fields[3])
                                    .desired_width(f32::INFINITY)
                                    .desired_rows(5),
                            );
                        });
                    });
                    dialog.buttons([
                        DialogButton::cancel(labels::cancel(), Action::Cancel),
                        DialogButton::primary("Apply", Action::Apply),
                    ]);
                });
                match response.action {
                    Some(Action::Apply) => {
                        let metadata = icy_engine::formats::SauceMetaData {
                            title: self.sauce_fields[0].clone().into(),
                            author: self.sauce_fields[1].clone().into(),
                            group: self.sauce_fields[2].clone().into(),
                            comments: self.sauce_fields[3].lines().map(|line| line.to_string().into()).collect(),
                        };
                        self.edit(|state| state.update_sauce_data(metadata));
                        keep = false;
                    }
                    Some(Action::Cancel) => keep = false,
                    None => keep &= !response.dismissed,
                }
            }
            Dialog::Export => {
                #[derive(Clone, Copy)]
                enum Action {
                    Cancel,
                    Export,
                }
                let formats = self
                    .document
                    .with_state(|state| FileFormat::save_formats_with_images_for_buffer_type(state.get_buffer().buffer_type));
                let response = appearance::Dialog::new("export").size(DialogSize::Medium).show(context, |dialog| {
                    dialog.content(|ui| {
                        appearance::group(ui, "", |ui| {
                            egui::ComboBox::from_id_salt("export-format")
                                .selected_text(self.export_format.name())
                                .show_ui(ui, |ui| {
                                    for format in formats {
                                        ui.selectable_value(&mut self.export_format, format, format.name());
                                    }
                                });
                            ui.checkbox(&mut self.export_sauce, "SAUCE metadata");
                            let settings = &mut self.settings.export_settings;
                            if matches!(self.export_format, FileFormat::Ansi | FileFormat::AnsiMusic) {
                                egui::ComboBox::from_id_salt("ansi-level")
                                    .selected_text(settings.ansi_level.to_string())
                                    .show_ui(ui, |ui| {
                                        for &level in icy_engine::AnsiCompatibilityLevel::all() {
                                            ui.selectable_value(&mut settings.ansi_level, level, level.to_string());
                                        }
                                    });
                                ui.checkbox(&mut settings.ansi_rgb_output, "RGB colors");
                                ui.horizontal(|ui| {
                                    ui.checkbox(&mut settings.max_line_length_enabled, "Limit line length");
                                    ui.add(egui::DragValue::new(&mut settings.max_line_length).range(1..=65535));
                                });
                            }
                            egui::ComboBox::from_id_salt("screen-prep")
                                .selected_text(settings.screen_prep.to_string())
                                .show_ui(ui, |ui| {
                                    for &preparation in icy_engine::ScreenPreperation::all() {
                                        ui.selectable_value(&mut settings.screen_prep, preparation, preparation.to_string());
                                    }
                                });
                            ui.checkbox(&mut settings.utf8_output, "UTF-8 output");
                            ui.checkbox(&mut settings.compress, "Compression");
                        });
                    });
                    dialog.buttons([
                        DialogButton::cancel(labels::cancel(), Action::Cancel),
                        DialogButton::primary("Export...", Action::Export),
                    ]);
                });
                match response.action {
                    Some(Action::Export) => {
                        self.choose(context, FileAction::Export(self.export_format));
                        keep = false;
                    }
                    Some(Action::Cancel) => keep = false,
                    None => keep &= !response.dismissed,
                }
            }
            Dialog::Overwrite(path) => {
                #[derive(Clone, Copy)]
                enum Action {
                    Cancel,
                    Overwrite,
                }
                let response = MessageBox::new("overwrite", MessageKind::Warning, "Replace File?", path.display().to_string())
                    .buttons([
                        DialogButton::cancel(labels::cancel(), Action::Cancel),
                        DialogButton::destructive(labels::overwrite(), Action::Overwrite),
                    ])
                    .show(context);
                match response.action {
                    Some(Action::Overwrite) => {
                        self.save_path(context, path.clone(), true);
                        keep = false;
                    }
                    Some(Action::Cancel) | None => {
                        if response.action.is_some() || response.dismissed {
                            self.continue_after_save = false;
                            keep = false;
                        }
                    }
                }
            }
            Dialog::Inspector => {
                let response = appearance::Dialog::new("inspector-dialog").size(DialogSize::Medium).show(context, |dialog| {
                    dialog.content(|ui| appearance::group(ui, "", |ui| self.inspector(ui)));
                    dialog.buttons([DialogButton::primary(labels::close(), ()).cancels()]);
                });
                keep &= response.action.is_none() && !response.dismissed && self.dialog.is_none();
            }
            Dialog::Characters | Dialog::FKeyCharacter(_, _) => {
                #[derive(Clone, Copy)]
                enum Action {
                    Cancel,
                    Select,
                }
                let target = match dialog {
                    Dialog::FKeyCharacter(set, slot) => Some((set, slot)),
                    _ => None,
                };
                let font = self
                    .document
                    .with_state(|state| state.get_buffer().font(state.get_caret().attribute.font_page()).cloned());
                let initial = target
                    .map(|(set, slot)| char::from_u32(self.settings.fkeys.code_at(set, slot) as u32).unwrap_or(' '))
                    .unwrap_or(self.document.brush.paint_char);
                let cursor_id = egui::Id::new("character-dialog-cursor");
                let mut cursor = context.data(|data| data.get_temp::<u32>(cursor_id)).unwrap_or(initial as u32).min(255);
                let mut chosen = None;
                for event in context.input(|input| input.events.clone()) {
                    if let egui::Event::Key {
                        key, pressed: true, modifiers, ..
                    } = event
                    {
                        if modifiers.command || modifiers.alt {
                            continue;
                        }
                        match key {
                            Key::ArrowLeft => cursor = cursor.saturating_sub(1),
                            Key::ArrowRight => cursor = (cursor + 1).min(255),
                            Key::ArrowUp => cursor = cursor.saturating_sub(16),
                            Key::ArrowDown => cursor = (cursor + 16).min(255),
                            Key::Home => cursor = 0,
                            Key::End => cursor = 255,
                            Key::Enter => chosen = char::from_u32(cursor),
                            _ => {}
                        }
                    }
                }
                let title = target.map_or("Characters".to_owned(), |(_, slot)| format!("Assign F{}", slot + 1));
                let response = appearance::Dialog::new("characters")
                    .size(DialogSize::Width(500.0))
                    .scroll(false)
                    .show(context, |dialog| {
                        dialog.content(|ui| {
                            appearance::group(ui, &title, |ui| {
                                if let Some(font) = font {
                                    let size = ((ui.available_width() - 30.0) / 16.0).min(28.0);
                                    ui.scope(|ui| {
                                        ui.spacing_mut().item_spacing = egui::Vec2::splat(2.0);
                                        for row in 0..16 {
                                            ui.horizontal(|ui| {
                                                for column in 0..16 {
                                                    let character = char::from_u32(row * 16 + column).unwrap();
                                                    if widgets::glyph(ui, &font, character, character as u32 == cursor, size).clicked() {
                                                        chosen = Some(character);
                                                    }
                                                }
                                            });
                                        }
                                    });
                                }
                                ui.add_space(6.0);
                                ui.weak(format!("{} / 0x{cursor:02X}", cursor));
                            });
                        });
                        dialog.buttons([
                            DialogButton::cancel(labels::cancel(), Action::Cancel),
                            DialogButton::primary("Select", Action::Select),
                        ]);
                    });
                match response.action {
                    Some(Action::Select) => chosen = char::from_u32(cursor),
                    Some(Action::Cancel) => keep = false,
                    None => keep &= !response.dismissed,
                }
                if let Some(character) = chosen {
                    if let Some((set, slot)) = target {
                        self.settings.fkeys.set_code_at(set, slot, character as u16);
                        if self.persist_settings {
                            let result = self.settings.fkeys.save().map_err(|error| error.to_string());
                            self.result(result);
                        }
                    } else {
                        self.document.brush.paint_char = character;
                    }
                    keep = false;
                }
                if keep {
                    context.data_mut(|data| data.insert_temp(cursor_id, cursor));
                } else {
                    context.data_mut(|data| data.remove::<u32>(cursor_id));
                    self.canvas_focus = true;
                }
            }
            Dialog::Error(error) => {
                let response = MessageBox::new("error", MessageKind::Error, "Icy Draw", error)
                    .copyable()
                    .buttons([DialogButton::primary(labels::ok(), ()).cancels()])
                    .show(context);
                keep &= response.action.is_none() && !response.dismissed;
                if !keep && self.font_editor.is_some() {
                    self.dialog = Some(Dialog::Font);
                }
            }
            Dialog::New | Dialog::Resize => {
                #[derive(Clone, Copy)]
                enum Action {
                    Cancel,
                    Confirm,
                }
                let resize = matches!(dialog, Dialog::Resize);
                let response = appearance::Dialog::new("new")
                    .size(DialogSize::Medium)
                    .confirm_on_enter(true)
                    .show(context, |dialog| {
                        dialog.content(|ui| {
                            appearance::group(ui, if resize { "Resize Document" } else { "New Document" }, |ui| {
                                if !resize {
                                    egui::ComboBox::from_id_salt("new-kind")
                                        .selected_text(if self.new_bitmap {
                                            "Bitmap Font".into()
                                        } else if self.new_animation {
                                            "Animation".into()
                                        } else {
                                            self.new_font_type.map_or("ANSI / ASCII".into(), |kind| format!("TheDraw {kind:?}"))
                                        })
                                        .show_ui(ui, |ui| {
                                            if ui
                                                .selectable_label(!self.new_bitmap && !self.new_animation && self.new_font_type.is_none(), "ANSI / ASCII")
                                                .clicked()
                                            {
                                                self.new_bitmap = false;
                                                self.new_animation = false;
                                                self.new_font_type = None;
                                            }
                                            if ui.selectable_label(self.new_animation, "Animation").clicked() {
                                                self.new_bitmap = false;
                                                self.new_animation = true;
                                            }
                                            if ui.selectable_label(self.new_bitmap, "Bitmap Font").clicked() {
                                                self.new_bitmap = true;
                                                self.new_animation = false;
                                            }
                                            for kind in [
                                                icy_engine_edit::charset::TdfFontType::Color,
                                                icy_engine_edit::charset::TdfFontType::Block,
                                                icy_engine_edit::charset::TdfFontType::Outline,
                                            ] {
                                                if ui
                                                    .selectable_label(
                                                        !self.new_bitmap && !self.new_animation && self.new_font_type == Some(kind),
                                                        format!("TheDraw {kind:?}"),
                                                    )
                                                    .clicked()
                                                {
                                                    self.new_bitmap = false;
                                                    self.new_animation = false;
                                                    self.new_font_type = Some(kind);
                                                }
                                            }
                                        });
                                }
                                ui.add(egui::DragValue::new(&mut self.new_size[0]).range(1..=1000).prefix("Columns "));
                                ui.add(egui::DragValue::new(&mut self.new_size[1]).range(1..=10000).prefix("Rows "));
                            });
                        });
                        dialog.buttons([
                            DialogButton::cancel(labels::cancel(), Action::Cancel),
                            DialogButton::primary(if resize { "Resize" } else { "Create" }, Action::Confirm),
                        ]);
                    });
                match response.action {
                    Some(Action::Confirm) => {
                        let size = Size::new(self.new_size[0], self.new_size[1]);
                        if resize {
                            self.edit(|state| state.resize_buffer(true, size));
                        } else if self.new_bitmap {
                            self.font_editor = Some(super::font::FontEditor::new(icy_engine::BitFont::create_8("Untitled", 8, 16, &[0; 4096])));
                            self.dialog = Some(Dialog::Font);
                        } else if self.new_animation {
                            self.replace(Document::new(size));
                            self.animation = Some(super::animation::AnimationEditor::new());
                        } else if let Some(kind) = self.new_font_type {
                            let font = icy_draw::charfont::CharFontDocument::new(kind);
                            self.replace(font.document());
                            self.charfont = Some(font);
                        } else {
                            self.replace(Document::new(size));
                        }
                        keep = false;
                    }
                    Some(Action::Cancel) => keep = false,
                    None => keep &= !response.dismissed,
                }
            }
            Dialog::Close => {
                #[derive(Clone, Copy)]
                enum Action {
                    Discard,
                    Cancel,
                    Save,
                }
                let response = MessageBox::new("close", MessageKind::Question, "Unsaved Changes", "Save changes before closing this document?")
                    .buttons([
                        DialogButton::destructive("Discard", Action::Discard).leading(),
                        DialogButton::cancel(labels::cancel(), Action::Cancel),
                        DialogButton::primary("Save", Action::Save),
                    ])
                    .show(context);
                match response.action {
                    Some(Action::Save) => {
                        self.continue_after_save = true;
                        self.save(context, false);
                        keep = false;
                    }
                    Some(Action::Discard) => {
                        self.complete_close(context);
                        keep = false;
                    }
                    Some(Action::Cancel) | None => {
                        if response.action.is_some() || response.dismissed {
                            self.quitting = false;
                            self.pending = None;
                            self.continue_after_save = false;
                            keep = false;
                        }
                    }
                }
            }
        }
        if keep && self.dialog.is_none() {
            self.dialog = Some(dialog);
        }
    }

    pub fn show(&mut self, context: &egui::Context) {
        if !context.will_discard() {
            while let Some(event) = self.mcp.as_ref().and_then(|bridge| bridge.events.try_recv().ok()) {
                match event {
                    Ok(command) => self.mcp_command(command),
                    Err(error) => self.dialog = Some(Dialog::Error(error)),
                }
            }
        }
        while let Ok(picked) = self.receiver.try_recv() {
            self.picker = false;
            if let Some(mut path) = picked.path {
                match picked.action {
                    FileAction::Open => self.open(path),
                    FileAction::Save => {
                        if path.extension().is_none() {
                            path.set_extension("icy");
                        }
                        if path.exists() {
                            self.dialog = Some(Dialog::Overwrite(path));
                        } else {
                            self.save_path(context, path, false);
                        }
                    }
                    FileAction::Export(format) => {
                        self.export_format = format;
                        if path.extension().is_none() {
                            path.set_extension(format.primary_extension());
                        }
                        if path.exists() {
                            self.dialog = Some(Dialog::ExportOverwrite(path));
                        } else {
                            self.export_path(path);
                        }
                    }
                    FileAction::SaveFont => {
                        if path.extension().is_none() {
                            path.set_extension("psf");
                        }
                        if path.exists() {
                            self.dialog = Some(Dialog::FontOverwrite(path));
                        } else if let Some(editor) = &mut self.font_editor {
                            let result = editor.save(&path, false);
                            self.result(result);
                        }
                    }
                    FileAction::SaveTdf => {
                        if path.extension().is_none() {
                            path.set_extension("tdf");
                        }
                        if path.exists() {
                            self.dialog = Some(Dialog::Overwrite(path));
                        } else {
                            self.save_path(context, path, false);
                        }
                    }
                    FileAction::SaveAnimation => {
                        if path.extension().is_none() {
                            path.set_extension("icyanim");
                        }
                        if path.exists() {
                            self.dialog = Some(Dialog::Overwrite(path));
                        } else {
                            self.save_path(context, path, false);
                        }
                    }
                    FileAction::ExportAnimation(format) => {
                        if path.extension().is_none() {
                            path.set_extension(format.extension());
                        }
                        if path.exists() {
                            self.dialog = Some(Dialog::AnimationOverwrite(path, format));
                        } else if let Some(editor) = &mut self.animation {
                            editor.export(path, format, context.clone());
                        }
                    }
                }
            } else {
                self.continue_after_save = false;
            }
        }
        let close_requested = context.input(|input| input.viewport().close_requested());
        if close_requested {
            self.document.finish();
            if let Some(editor) = &mut self.font_editor {
                editor.finish();
            }
        }
        if close_requested && self.picker {
            context.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        } else if close_requested && !self.allow_close && self.modified() {
            context.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.quitting = true;
            self.dialog = Some(Dialog::Close);
        }
        if close_requested && !self.picker && self.font_editor.as_ref().is_some_and(|editor| editor.modified()) {
            context.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.dialog = Some(Dialog::Font);
        }
        if self.dialog.is_none() && !self.picker {
            for file in context.input(|input| input.raw.dropped_files.clone()) {
                if let Some(path) = file.path {
                    self.open(path);
                    break;
                }
            }
        }
        let blocked = self.dialog.is_some() || self.picker || self.layer_properties_open();
        let path = self
            .animation
            .as_ref()
            .and_then(|editor| editor.path.as_ref())
            .or(self.charfont.as_ref().and_then(|font| font.path.as_ref()))
            .or(self.document.path.as_ref());
        context.send_viewport_cmd(egui::ViewportCommand::Title(format!(
            "{}{} - Icy Draw",
            if self.modified() { "*" } else { "" },
            path.and_then(|path| path.file_name())
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Untitled".into())
        )));
        if blocked {
            self.document.finish();
        }
        self.menu(context);
        if let Some(editor) = &mut self.animation {
            editor.show(context, blocked || self.dialog.is_some() || self.picker);
            if let Some(format) = editor.take_export_request() {
                self.choose(context, FileAction::ExportAnimation(format));
            }
            self.canvas_focus = false;
            if !blocked {
                self.keys(context);
            }
            self.dialogs(context);
            return;
        }
        let panel_fill = context.style().visuals.panel_fill;
        egui::TopBottomPanel::top("toolbar")
            .exact_height(chrome::TOOLBAR_HEIGHT)
            .frame(egui::Frame::new().fill(panel_fill))
            .show(context, |ui| {
                if blocked {
                    ui.disable();
                }
                self.toolbar(ui, context);
            });
        let sidebar = self.show_inspector && context.content_rect().width() >= 850.0;
        if !sidebar {
            self.charfont_bar(context);
        }
        egui::TopBottomPanel::bottom("status")
            .exact_height(chrome::STATUS_HEIGHT)
            .frame(egui::Frame::new().fill(panel_fill))
            .show(context, |ui| {
                if blocked {
                    ui.disable();
                }
                self.status_bar(ui);
            });
        egui::SidePanel::left("sidebar")
            .exact_width(chrome::SIDEBAR_WIDTH)
            .frame(egui::Frame::new().fill(panel_fill))
            .resizable(false)
            .show(context, |ui| {
                if blocked || self.document.paste_active() {
                    ui.disable();
                }
                self.sidebar(ui);
            });
        if sidebar {
            egui::SidePanel::right("panel")
                .exact_width(chrome::PANEL_WIDTH)
                .frame(egui::Frame::new().fill(panel_fill).inner_margin(egui::Margin { top: 4, ..Default::default() }))
                .resizable(false)
                .show(context, |ui| {
                    if blocked || self.document.paste_active() {
                        ui.disable();
                    }
                    self.panel(ui);
                });
        }
        let well = if context.style().visuals.dark_mode {
            Color32::from_gray(22)
        } else {
            Color32::from_gray(212)
        };
        egui::CentralPanel::default().frame(egui::Frame::new().fill(well)).show(context, |ui| {
            self.canvas(ui, blocked || self.dialog.is_some() || self.picker || self.layer_properties_open())
        });
        if !blocked && !self.layer_properties_open() {
            self.keys(context);
        }
        self.layer_properties_dialog(context);
        self.dialogs(context);
    }
}

impl eframe::App for DrawApp {
    fn update(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        self.show(context);
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
