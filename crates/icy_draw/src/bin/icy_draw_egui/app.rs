use eframe::egui::{self, Color32, Key};
use icy_draw::{brush::BrushPrimaryMode, document::Document, Settings};
use icy_engine::{FileFormat, Position, Selection, Size, TextPane};
use icy_engine_edit::tools::Tool;
use icy_engine_edit::UndoState;
use icy_engine_gui::{
    egui::{
        appearance::{self, labels, DialogButton, DialogSize, MessageBox, MessageKind},
        screen::ScreenView,
    },
    ScalingMode,
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
};

use super::widgets::{self, Icons};
#[path = "chrome.rs"]
mod chrome;
#[path = "collab.rs"]
mod collab;
#[path = "mcp.rs"]
mod mcp;
#[path = "menus.rs"]
mod menus;
#[path = "welcome.rs"]
mod welcome;

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
    TextArtFontSelect,
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
    ReferenceImage,
    Shortcuts,
    About,
    Connect,
}
enum FileAction {
    Open,
    Save,
    Export(FileFormat),
    SaveFont,
    SaveTdf,
    SaveAnimation,
    ExportAnimation(super::animation::ExportFormat),
    InsertImage,
    ReferenceImage,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TextArtFontKind {
    Outline,
    Block,
    Color,
    Figlet,
}

impl TextArtFontKind {
    const ALL: [(Self, &'static str); 4] = [
        (Self::Outline, "Outline"),
        (Self::Block, "Block"),
        (Self::Color, "Color"),
        (Self::Figlet, "Figlet"),
    ];

    fn of(font: &retrofont::Font) -> Self {
        match font {
            retrofont::Font::Figlet(_) => Self::Figlet,
            retrofont::Font::Tdf(font) => match font.font_type() {
                retrofont::tdf::TdfFontType::Outline => Self::Outline,
                retrofont::tdf::TdfFontType::Block => Self::Block,
                retrofont::tdf::TdfFontType::Color => Self::Color,
            },
        }
    }

    fn index(self) -> usize {
        match self {
            Self::Outline => 0,
            Self::Block => 1,
            Self::Color => 2,
            Self::Figlet => 3,
        }
    }
}

fn filter_match_ranges(name: &str, filter: &str) -> Vec<std::ops::Range<usize>> {
    let filter = filter.trim().to_lowercase();
    if filter.is_empty() {
        return Vec::new();
    }
    let lower_name = name.to_lowercase();
    if lower_name.len() != name.len() {
        return Vec::new();
    }
    lower_name.match_indices(&filter).map(|(start, value)| start..start + value.len()).collect()
}

/// Document kinds offered by the New dialog and the start screen.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NewKind {
    Ansi,
    Animation,
    BitmapFont,
    TheDraw(icy_engine_edit::charset::TdfFontType),
}

impl NewKind {
    pub const ALL: [NewKind; 6] = [
        NewKind::Ansi,
        NewKind::Animation,
        NewKind::BitmapFont,
        NewKind::TheDraw(icy_engine_edit::charset::TdfFontType::Color),
        NewKind::TheDraw(icy_engine_edit::charset::TdfFontType::Block),
        NewKind::TheDraw(icy_engine_edit::charset::TdfFontType::Outline),
    ];

    pub fn name(self) -> &'static str {
        use icy_engine_edit::charset::TdfFontType;
        match self {
            NewKind::Ansi => "ANSI Art",
            NewKind::Animation => "Animation",
            NewKind::BitmapFont => "Bitmap Font",
            NewKind::TheDraw(TdfFontType::Color) => "TheDraw Color Font",
            NewKind::TheDraw(TdfFontType::Block) => "TheDraw Block Font",
            NewKind::TheDraw(TdfFontType::Outline) => "TheDraw Outline Font",
        }
    }

    pub fn description(self) -> &'static str {
        use icy_engine_edit::charset::TdfFontType;
        match self {
            NewKind::Ansi => "Text mode canvas with layers",
            NewKind::Animation => "Lua scripted ANSI animation",
            NewKind::BitmapFont => "8 × 16 pixel console font",
            NewKind::TheDraw(TdfFontType::Color) => "Characters with colors",
            NewKind::TheDraw(TdfFontType::Block) => "Block characters, one color",
            NewKind::TheDraw(TdfFontType::Outline) => "Outline placeholders",
        }
    }

    pub fn icon(self) -> &'static str {
        use icy_engine_edit::charset::TdfFontType;
        match self {
            NewKind::Ansi => "pencil",
            NewKind::Animation => "play",
            NewKind::BitmapFont => "font",
            NewKind::TheDraw(TdfFontType::Color) => "paint_brush",
            NewKind::TheDraw(TdfFontType::Block) => "rectangle_filled",
            NewKind::TheDraw(TdfFontType::Outline) => "rectangle_outline",
        }
    }

    /// Only ANSI documents use the size entered in the New dialog.
    pub fn has_size(self) -> bool {
        self == NewKind::Ansi
    }
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
    pending_connect: bool,
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
    new_kind: NewKind,
    animation: Option<super::animation::AnimationEditor>,
    clipboard: Option<(String, Vec<u8>)>,
    font_filter: String,
    text_fonts: Option<icy_draw::text_art_fonts::SharedFontLibrary>,
    text_font: usize,
    text_preview: Option<(usize, egui::TextureHandle)>,
    text_font_filter: String,
    text_font_types: [bool; 4],
    text_font_pending: usize,
    text_font_dialog_previews: HashMap<usize, egui::TextureHandle>,
    scroll_to_text_font: bool,
    text_font_preview_text: String,
    text_font_dialog_preview_text: String,
    text_font_size_filter: [u32; 4],
    text_font_favorites_only: bool,
    text_font_info: Vec<(String, TextArtFontKind)>,
    plugins: Option<Vec<icy_draw::plugins::Plugin>>,
    script: String,
    script_output: String,
    mcp: Option<mcp::Bridge>,
    chrome: chrome::Chrome,
    pub persist_settings: bool,
    pub show_start: bool,
    show_grid: bool,
    show_layer_bounds: bool,
    show_line_numbers: bool,
    guide: Option<(i32, i32)>,
    show_guide: bool,
    raster: Option<(i32, i32)>,
    show_raster: bool,
    reference_image: Option<icy_engine_gui::ReferenceImageSettings>,
    reference_draft: icy_engine_gui::ReferenceImageSettings,
    reference_path: String,
    pipette_hover: Option<(Position, egui::Modifiers)>,
    pub canvas_rect: egui::Rect,
    collab: collab::Collaboration,
}

impl DrawApp {
    pub fn new() -> Self {
        let document = Document::new(Size::new(80, 25));
        let view = ScreenView::from_shared(document.screen.clone());
        let mut settings = Settings::load();
        settings.monitor_settings.scaling_mode = ScalingMode::Manual(2.0);
        let show_line_numbers = settings.show_line_numbers;
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
            pending_connect: false,
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
            new_kind: NewKind::Ansi,
            animation: None,
            clipboard: None,
            font_filter: String::new(),
            text_fonts: None,
            text_font: 0,
            text_preview: None,
            text_font_filter: String::new(),
            text_font_types: [true; 4],
            text_font_pending: 0,
            text_font_dialog_previews: HashMap::new(),
            scroll_to_text_font: false,
            text_font_preview_text: "HALLO".to_owned(),
            text_font_dialog_preview_text: String::new(),
            text_font_size_filter: [0; 4],
            text_font_favorites_only: false,
            text_font_info: Vec::new(),
            plugins: None,
            script: String::new(),
            script_output: String::new(),
            mcp: None,
            chrome: chrome::Chrome::default(),
            persist_settings: false,
            show_start: false,
            show_grid: false,
            show_layer_bounds: false,
            show_line_numbers,
            guide: None,
            show_guide: true,
            raster: None,
            show_raster: true,
            reference_image: None,
            reference_draft: Default::default(),
            reference_path: String::new(),
            pipette_hover: None,
            canvas_rect: egui::Rect::NOTHING,
            collab: collab::Collaboration::default(),
        }
    }

    /// Replaces the current document with a new, empty one of the given kind.
    pub(super) fn create(&mut self, kind: NewKind, size: Size) {
        match kind {
            NewKind::Ansi => self.replace(Document::new(size)),
            NewKind::Animation => {
                self.replace(Document::new(Size::new(80, 25)));
                self.animation = Some(super::animation::AnimationEditor::new());
            }
            NewKind::BitmapFont => {
                self.font_editor = Some(super::font::FontEditor::new(icy_engine::BitFont::create_8("Untitled", 8, 16, &[0; 4096])));
                self.dialog = Some(Dialog::Font);
            }
            NewKind::TheDraw(kind) => {
                let font = icy_draw::charfont::CharFontDocument::new(kind);
                self.replace(font.document());
                self.charfont = Some(font);
            }
        }
    }

    /// Replacing the document leaves any collaboration session, since the server owns the shared canvas.
    fn replace(&mut self, document: Document) {
        if self.collab.in_session() {
            self.collab.disconnect();
        }
        self.replace_document(document);
    }

    fn replace_document(&mut self, document: Document) {
        self.show_start = false;
        self.charfont = None;
        self.animation = None;
        self.font_editor = None;
        if matches!(self.dialog, Some(Dialog::Font)) {
            self.dialog = None;
        }
        self.document = document;
        self.chrome = chrome::Chrome::default();
        self.pipette_hover = None;
        self.reference_image = None;
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
            self.pending_connect = false;
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
                FileAction::InsertImage | FileAction::ReferenceImage => dialog
                    .add_filter("Images", &["png", "jpg", "jpeg", "gif", "bmp", "webp", "tga", "tif", "tiff"])
                    .pick_file(),
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
        } else if std::mem::take(&mut self.pending_connect) {
            self.show_connect_dialog();
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
        // The collaboration server persists the shared document.
        if self.collab.active {
            return false;
        }
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
                        (BrushPrimaryMode::Char, "Character", "Paint with the selected character and colors"),
                        (BrushPrimaryMode::HalfBlock, "Half Block", "Paint half-block pixels with twice the vertical resolution"),
                        (BrushPrimaryMode::Shading, "Shade", "Shade characters: left click lighter, right click darker"),
                        (BrushPrimaryMode::Replace, "Replace", "Replace existing characters, keep the colors"),
                        (BrushPrimaryMode::Blink, "Blink", "Toggle the blink attribute"),
                        (BrushPrimaryMode::Colorize, "Colorize", "Change only the colors of existing characters"),
                    ],
                );
                widgets::divider(ui);
                if let Some(font) = &font {
                    if widgets::glyph(ui, font, self.document.brush.paint_char, false, widgets::CONTROL_HEIGHT)
                        .on_hover_text("Brush character – click to choose from the character table")
                        .clicked()
                    {
                        self.dialog = Some(Dialog::Characters);
                    }
                }
                if self.document.tool == Tool::Pencil {
                    widgets::divider(ui);
                    ui.weak("Brush size");
                    ui.spacing_mut().item_spacing.x = 0.0;
                    if self.icons.button(ui, "arrow_left", "Smaller Brush", false).clicked() {
                        self.document.brush.brush_size = self.document.brush.brush_size.saturating_sub(1).max(1);
                    }
                    ui.add(egui::DragValue::new(&mut self.document.brush.brush_size).range(1..=9))
                        .on_hover_text("Brush size in cells (Alt+Plus / Alt+Minus)");
                    if self.icons.button(ui, "arrow_right", "Larger Brush", false).clicked() {
                        self.document.brush.brush_size = (self.document.brush.brush_size + 1).min(9);
                    }
                    ui.spacing_mut().item_spacing.x = 4.0;
                }
                widgets::divider(ui);
                ui.weak("Apply");
                widgets::toggle(ui, "Foreground", &mut self.document.brush.colorize_fg, "Apply the foreground color");
                widgets::toggle(ui, "Background", &mut self.document.brush.colorize_bg, "Apply the background color");
                if self.document.tool == Tool::Fill {
                    widgets::divider(ui);
                    widgets::toggle(ui, "Exact Match", &mut self.document.brush.exact, "Only fill cells that match character and colors exactly");
                }
                let variants = match self.document.tool {
                    Tool::RectangleOutline | Tool::RectangleFilled => Some([Tool::RectangleOutline, Tool::RectangleFilled]),
                    Tool::EllipseOutline | Tool::EllipseFilled => Some([Tool::EllipseOutline, Tool::EllipseFilled]),
                    _ => None,
                };
                if let Some(variants) = variants {
                    widgets::divider(ui);
                    for tool in variants {
                        if self.icons.button(ui, tool.icon(), chrome::tool_label(tool), self.document.tool == tool).clicked() {
                            self.select_tool(tool);
                        }
                    }
                }
            }
            Tool::Font => {
                if let Some(library) = &self.text_fonts {
                    let font_name = library.read().font_name(self.text_font).unwrap_or("No fonts").to_owned();
                    if ui
                        .add_sized([290.0, widgets::CONTROL_HEIGHT], egui::Button::new(font_name).truncate())
                        .on_hover_text("Choose a text-art font")
                        .clicked()
                    {
                        self.text_font_pending = self.text_font;
                        self.scroll_to_text_font = true;
                        self.dialog = Some(Dialog::TextArtFontSelect);
                    }
                    widgets::divider(ui);
                    ui.weak("Outline");
                    let style = &mut self.settings.font_outline_style;
                    widgets::outline_style_picker(ui, style);
                    let mut library = library.write();
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
                    ui.weak(format!("Set {} of {}", self.settings.fkeys.current_set + 1, self.settings.fkeys.set_count()))
                        .on_hover_text("F-key character set (Ctrl+Comma / Ctrl+Period to switch, right click a key to reassign)");
                    ui.spacing_mut().item_spacing.x = 4.0;
                    widgets::divider(ui);
                    if self.icons.button(ui, "font", "Character Table", false).clicked() {
                        self.dialog = Some(Dialog::Characters);
                    }
                }
            }
            Tool::Pipette => {
                self.pipette_options(ui);
            }
            Tool::Select => {
                use icy_draw::document::SelectionMode;
                let mut mode = self.document.selection_mode;
                if widgets::segmented(
                    ui,
                    &mut mode,
                    &[
                        (SelectionMode::Rectangle, "Rectangle", "Drag a rectangular selection"),
                        (SelectionMode::Character, "Character", "Select every cell with the clicked character"),
                        (SelectionMode::Attribute, "Attribute", "Select every cell with the clicked colors"),
                        (SelectionMode::Foreground, "Foreground", "Select every cell with the clicked foreground color"),
                        (SelectionMode::Background, "Background", "Select every cell with the clicked background color"),
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
                        if self.icons.button(ui, "swap", "Flip Vertically", false).clicked() {
                            self.edit(|state| state.flip_y());
                        }
                    });
                    if self.icons.button(ui, "delete", "Deselect", false).clicked() {
                        self.edit(|state| state.clear_selection());
                    }
                });
                widgets::divider(ui);
                if let Some(bounds) = self.document.with_state(|state| state.selection().map(|selection| selection.as_rectangle())) {
                    ui.weak(format!("{}, {}  ·  {} × {}", bounds.left(), bounds.top(), bounds.width(), bounds.height()));
                } else {
                    ui.weak("Shift adds to the selection, Ctrl removes from it");
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
                match self.document.selected_tags.len() {
                    0 => {
                        ui.weak("Click to place a tag, drag to move it");
                    }
                    1 => {
                        let index = self.document.selected_tags[0];
                        if let Some(tag) = self.document.with_state(|state| state.get_buffer().tags.get(index).cloned()) {
                            let preview: String = tag.preview.chars().take(20).collect();
                            ui.strong(if preview.is_empty() { "(empty)" } else { &preview });
                            ui.weak(format!("at {}, {}  ·  {} chars", tag.position.x, tag.position.y, tag.len()));
                        }
                    }
                    count => {
                        ui.weak(format!("{count} tags selected"));
                    }
                };
            }
            _ => {}
        }
    }

    fn pipette_options(&mut self, ui: &mut egui::Ui) {
        let Some((position, modifiers)) = self.pipette_hover else {
            ui.weak("Hover over the canvas to preview sampled colors");
            widgets::divider(ui);
            ui.weak("Shift: foreground only  ·  Ctrl/right click: background only");
            return;
        };
        let Some((character, font, foreground, background, preview_foreground, preview_background)) = self.document.with_state(|state| {
            let buffer = state.get_buffer();
            let character = buffer.char_at(position);
            let font = buffer
                .font(character.attribute.font_page())
                .or_else(|| buffer.font(0))
                .cloned()?;
            let palette = &buffer.palette;
            let color = |direct: icy_engine::AttributeColor, index| direct.as_rgb().unwrap_or_else(|| palette.rgb(index));
            let foreground = color(character.attribute.foreground_color(), character.attribute.foreground());
            let background = color(character.attribute.background_color(), character.attribute.background());
            let caret = state.get_caret().attribute;
            let caret_foreground = color(caret.foreground_color(), caret.foreground());
            let caret_background = color(caret.background_color(), caret.background());
            let foreground_only = modifiers.shift;
            let background_only = modifiers.ctrl || modifiers.command || modifiers.mac_cmd;
            Some((
                character,
                font,
                foreground,
                background,
                if background_only { caret_foreground } else { foreground },
                if foreground_only { caret_background } else { background },
            ))
        }) else {
            return;
        };
        let foreground_only = modifiers.shift;
        let background_only = modifiers.ctrl || modifiers.command || modifiers.mac_cmd;
        let take_foreground = !background_only;
        let take_background = !foreground_only;

        ui.monospace(format!("#{:02X}", character.ch as u32));
        widgets::colored_glyph(
            ui,
            &font,
            character.ch,
            Color32::from_rgb(preview_foreground.0, preview_foreground.1, preview_foreground.2),
            Color32::from_rgb(preview_background.0, preview_background.1, preview_background.2),
            34.0,
        );
        chrome::color_sample(ui, "FG", character.attribute.foreground(), foreground, take_foreground);
        chrome::color_sample(ui, "BG", character.attribute.background(), background, take_background);
        widgets::divider(ui);
        ui.weak("Shift: foreground only  ·  Ctrl/right click: background only");
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
                    let result = self.document.set_caret_foreground(index as u32);
                    self.result(result);
                }
                if response.secondary_clicked() {
                    let result = self.document.set_caret_background(index as u32);
                    self.result(result);
                }
                if background == index as u32 {
                    ui.painter().circle_filled(response.rect.center(), 2.0, Color32::WHITE);
                }
            }
            if self.icons.button(ui, "swap", "Swap Foreground and Background", false).clicked() {
                let result = self.document.swap_caret_colors();
                self.result(result);
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
            self.edit_bitmap_font();
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
        let mut response = self.view.show(ui, &self.settings.monitor_settings);
        self.canvas_rect = response.rect;
        if self.document.tool == Tool::Tag && !blocked {
            let hovering_tag = response
                .hover_pos()
                .and_then(|point| self.position(point))
                .is_some_and(|position| self.document.with_state(|state| state.get_buffer().tags.iter().any(|tag| tag.contains(position))));
            response = response.on_hover_cursor(if self.document.tag_drag_active() {
                egui::CursorIcon::Grabbing
            } else if hovering_tag {
                egui::CursorIcon::Grab
            } else {
                egui::CursorIcon::Crosshair
            });
        }
        if self.document.tool == Tool::Pipette && !blocked {
            let modifiers = ui.input(|input| input.modifiers);
            let hover = response.hover_pos().and_then(|point| self.position(point)).map(|position| (position, modifiers));
            if self.pipette_hover != hover {
                self.pipette_hover = hover;
                ui.ctx().request_repaint();
            }
        } else {
            if self.pipette_hover.take().is_some() {
                ui.ctx().request_repaint();
            }
        }
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
            let (tags, palette) = self.document.with_state(|state| (state.get_buffer().tags.clone(), state.get_buffer().palette.clone()));
            for (index, tag) in tags.iter().enumerate() {
                let position = self.document.tag_preview_position(index, tag.position);
                let rect = egui::Rect::from_min_size(
                    origin + egui::vec2(position.x as f32 * cell_size.x, position.y as f32 * cell_size.y) * info.display_scale,
                    egui::vec2(tag.length.max(1) as f32 * cell_size.x, cell_size.y) * info.display_scale,
                );
                let (red, green, blue) = tag.attribute.foreground_color().as_rgb().unwrap_or_else(|| palette.rgb(tag.attribute.foreground()));
                let color = Color32::from_rgb(red, green, blue);
                let selected = self.document.selected_tags.contains(&index);
                if selected {
                    painter.rect_filled(rect, 0, color.gamma_multiply(0.12));
                }
                painter.rect_stroke(rect, 0, egui::Stroke::new(if selected { 2.0 } else { 1.0 }, color), egui::StrokeKind::Inside);
                if selected {
                    for corner in [rect.left_top(), rect.right_top(), rect.left_bottom(), rect.right_bottom()] {
                        painter.rect_filled(egui::Rect::from_center_size(corner, egui::Vec2::splat(4.0)), 0, color);
                    }
                }
            }
        }
        let step = egui::vec2(info.font_width, info.font_height * if info.scan_lines { 2.0 } else { 1.0 }) * info.display_scale;
        self.remote_cursors(ui.ctx(), &painter, origin, step);
        if self.show_line_numbers {
            self.line_numbers(ui, &painter, response.rect, origin, step);
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
        self.pending_connect = false;
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
        // macOS turns Option shortcuts into text as well, so a consumed Alt shortcut swallows this frame's text.
        let mut alt_consumed = false;
        for event in events {
            if alt_consumed && matches!(event, egui::Event::Text(_)) {
                continue;
            }
            match event {
                egui::Event::Key {
                    key, pressed: true, modifiers, ..
                } if modifiers.alt && !modifiers.shift && (self.canvas_focus || key == Key::Enter) && self.alt_key(context, key, modifiers) => {
                    alt_consumed = true;
                }
                egui::Event::Key {
                    key, pressed: true, modifiers, ..
                } if modifiers.command && !modifiers.alt => match key {
                    Key::O if !modifiers.shift && !modifiers.alt => self.choose(context, FileAction::Open),
                    Key::N if !modifiers.shift && !modifiers.alt => self.request_new(),
                    Key::S if !modifiers.alt => self.save(context, modifiers.shift),
                    Key::Z if self.canvas_focus => self.undo(modifiers.shift),
                    Key::Y if self.canvas_focus => self.undo(true),
                    Key::A if self.canvas_focus && !self.document.paste_active() => self.select_all(),
                    _ if !self.document.paste_active() && self.command_key(context, key, modifiers) => {}
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

    fn text_art_font_preview(&mut self, context: &egui::Context, index: usize) -> Option<egui::TextureHandle> {
        if self.text_font_dialog_preview_text != self.text_font_preview_text {
            self.text_font_dialog_previews.clear();
            self.text_font_dialog_preview_text.clone_from(&self.text_font_preview_text);
        }
        if let Some(texture) = self.text_font_dialog_previews.get(&index) {
            return Some(texture.clone());
        }
        let preview = self.text_fonts.as_ref()?.read().generate_preview_text(index, &self.text_font_preview_text)?;
        let texture = context.load_texture(
            format!("text-art-font-preview-{index}"),
            egui::ColorImage::from_rgba_unmultiplied([preview.width as usize, preview.height as usize], &preview.rgba),
            egui::TextureOptions::NEAREST,
        );
        self.text_font_dialog_previews.insert(index, texture.clone());
        Some(texture)
    }

    fn text_art_font_metrics(&self, index: usize) -> Option<(usize, usize)> {
        self.text_fonts.as_ref()?.read().font_dimensions(index)
    }

    fn text_art_font_id(name: &str, kind: TextArtFontKind) -> String {
        format!("{}:{name}", TextArtFontKind::ALL[kind.index()].1)
    }

    fn toggle_text_art_favorite(&mut self, id: &str) {
        if let Some(position) = self.settings.text_art_font_favorites.iter().position(|favorite| favorite == id) {
            self.settings.text_art_font_favorites.remove(position);
        } else {
            self.settings.text_art_font_favorites.push(id.to_owned());
            self.settings.text_art_font_favorites.sort();
        }
        if self.persist_settings {
            self.settings.store_persistent();
        }
    }

    fn export_text_art_font(&mut self, index: usize) {
        let Some(library) = &self.text_fonts else {
            return;
        };
        let library = library.read();
        let Some(font) = library.get_font(index) else {
            return;
        };
        let name = font.name().to_owned();
        let extension = font.default_extension();
        let bytes = match font.to_bytes() {
            Ok(bytes) => bytes,
            Err(error) => {
                self.dialog = Some(Dialog::Error(format!("Failed to export font: {error}")));
                return;
            }
        };
        drop(library);
        let Some(path) = rfd::FileDialog::new()
            .set_title("Export Text-Art Font")
            .set_file_name(format!("{name}.{extension}"))
            .add_filter("Font file", &[extension])
            .save_file()
        else {
            return;
        };
        if let Err(error) = std::fs::write(path, bytes) {
            self.dialog = Some(Dialog::Error(format!("Failed to export font: {error}")));
        }
    }

    fn text_art_font_dialog(&mut self, context: &egui::Context) -> bool {
        #[derive(Clone, Copy)]
        enum Action {
            Export,
            Cancel,
            Apply,
        }

        let Some(library) = self.text_fonts.clone() else {
            return false;
        };
        let font_count = library.read().font_count();
        if self.text_font_info.len() != font_count {
            let library = library.read();
            self.text_font_info = (0..library.font_count())
                .filter_map(|index| {
                    let font = library.get_font(index)?;
                    Some((font.name().to_owned(), TextArtFontKind::of(font)))
                })
                .collect();
            self.text_font_dialog_previews.clear();
        }
        let filter = self.text_font_filter.to_lowercase();
        let mut fonts = Vec::with_capacity(self.text_font_info.len());
        let size_filter_active = self.text_font_size_filter.iter().any(|value| *value != 0);
        for index in 0..self.text_font_info.len() {
            let (name, kind) = &self.text_font_info[index];
            let favorite = self.settings.text_art_font_favorites.contains(&Self::text_art_font_id(name, *kind));
            let visible = self.text_font_types[kind.index()]
                && (!self.text_font_favorites_only || favorite)
                && (filter.is_empty() || name.to_lowercase().contains(&filter));
            if !visible {
                continue;
            }
            if size_filter_active {
                let (width, height) = self.text_art_font_metrics(index).unwrap_or_default();
                let [min_width, max_width, min_height, max_height] = self.text_font_size_filter;
                if (min_width != 0 && width < min_width as usize)
                    || (max_width != 0 && width > max_width as usize)
                    || (min_height != 0 && height < min_height as usize)
                    || (max_height != 0 && height > max_height as usize)
                {
                    continue;
                }
            }
            fonts.push(index);
        }

        let mut apply = false;
        let mut double_clicked = false;
        let response = appearance::Dialog::new("text-art-font-select")
            .size(DialogSize::Width(980.0))
            .fixed_height(650.0)
            .show(context, |dialog| {
                dialog.content(|ui| {
                    ui.horizontal(|ui| {
                        ui.add(
                            appearance::text_edit(&mut self.text_font_filter)
                                .hint_text("Filter fonts...")
                                .desired_width(220.0),
                        );
                        ui.add(
                            appearance::text_edit(&mut self.text_font_preview_text)
                                .hint_text("Preview text")
                                .desired_width(180.0),
                        );
                        if ui.add(egui::Button::selectable(self.text_font_favorites_only, "★ Favorites")).clicked() {
                            self.text_font_favorites_only = !self.text_font_favorites_only;
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.weak(format!("{} fonts", fonts.len()));
                        });
                    });
                    ui.horizontal(|ui| {
                        for (kind, label) in TextArtFontKind::ALL {
                            let enabled = &mut self.text_font_types[kind.index()];
                            if ui.add(egui::Button::selectable(*enabled, label)).clicked() {
                                *enabled = !*enabled;
                            }
                        }
                        ui.separator();
                        ui.weak("Width");
                        ui.add(egui::DragValue::new(&mut self.text_font_size_filter[0]).range(0..=255).prefix("min "));
                        ui.add(egui::DragValue::new(&mut self.text_font_size_filter[1]).range(0..=255).prefix("max "));
                        ui.weak("Height");
                        ui.add(egui::DragValue::new(&mut self.text_font_size_filter[2]).range(0..=255).prefix("min "));
                        ui.add(egui::DragValue::new(&mut self.text_font_size_filter[3]).range(0..=255).prefix("max "));
                        ui.weak("0 = any");
                    });
                    ui.add_space(8.0);

                    let row_height = 108.0;
                    let mut scroll = egui::ScrollArea::vertical().id_salt("text-art-font-list").auto_shrink([false, false]);
                    if std::mem::take(&mut self.scroll_to_text_font) {
                        let selected_row = fonts.iter().position(|font| *font == self.text_font_pending).unwrap_or(0);
                        scroll = scroll.vertical_scroll_offset(selected_row as f32 * row_height);
                    }
                    scroll.show_rows(ui, row_height, fonts.len(), |ui, rows| {
                        for row in rows {
                            let index = fonts[row];
                            let (name, kind) = self.text_font_info[index].clone();
                            let favorite = self.settings.text_art_font_favorites.contains(&Self::text_art_font_id(&name, kind));
                            let (width, height) = self.text_art_font_metrics(index).unwrap_or_default();
                            let selected = self.text_font_pending == index;
                            let (rect, response) =
                                ui.allocate_exact_size(egui::vec2(ui.available_width(), row_height - 4.0), egui::Sense::click());
                            let fill = if selected {
                                ui.visuals().selection.bg_fill
                            } else if response.hovered() {
                                ui.visuals().widgets.hovered.weak_bg_fill
                            } else {
                                ui.visuals().faint_bg_color
                            };
                            ui.painter().rect_filled(rect, 4, fill);
                            ui.painter().rect_stroke(
                                rect,
                                4,
                                ui.visuals().widgets.noninteractive.bg_stroke,
                                egui::StrokeKind::Inside,
                            );
                            let left = rect.shrink2(egui::vec2(16.0, 10.0));
                            let preview_width = (rect.width() * 0.46).min(420.0);
                            let preview_left = rect.right() - preview_width - 16.0;
                            let mut name_job = egui::text::LayoutJob::default();
                            let base = egui::TextFormat {
                                font_id: egui::FontId::proportional(16.0),
                                color: ui.visuals().strong_text_color(),
                                ..Default::default()
                            };
                            let highlight = egui::TextFormat {
                                background: Color32::from_rgb(230, 174, 55),
                                color: Color32::BLACK,
                                ..base.clone()
                            };
                            let mut offset = 0;
                            for range in filter_match_ranges(&name, &filter) {
                                name_job.append(&name[offset..range.start], 0.0, base.clone());
                                name_job.append(&name[range.clone()], 0.0, highlight.clone());
                                offset = range.end;
                            }
                            name_job.append(&name[offset..], 0.0, base);
                            name_job.wrap.max_width = (preview_left - left.left() - 16.0).max(80.0);
                            let name_galley = ui.fonts_mut(|fonts| fonts.layout_job(name_job));
                            ui.painter().galley(left.left_top(), name_galley, ui.visuals().strong_text_color());
                            ui.painter().text(
                                left.left_top() + egui::vec2(0.0, 30.0),
                                egui::Align2::LEFT_TOP,
                                "!\"#$%&'()*+,-./0123456789:;<=>?@\nABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`\nabcdefghijklmnopqrstuvwxyz{|}~",
                                egui::FontId::monospace(12.0),
                                ui.visuals().text_color(),
                            );
                            let preview_header = egui::Rect::from_min_max(
                                egui::pos2(preview_left, rect.top() + 6.0),
                                egui::pos2(rect.right() - 16.0, rect.top() + 26.0),
                            );
                            ui.painter().text(
                                preview_header.left_center(),
                                egui::Align2::LEFT_CENTER,
                                TextArtFontKind::ALL[kind.index()].1,
                                egui::FontId::proportional(11.0),
                                ui.visuals().weak_text_color(),
                            );
                            ui.painter().text(
                                preview_header.right_center(),
                                egui::Align2::RIGHT_CENTER,
                                format!("{width}×{height}"),
                                egui::FontId::monospace(11.0),
                                ui.visuals().weak_text_color(),
                            );
                            let star_rect = egui::Rect::from_center_size(
                                egui::pos2(preview_left - 16.0, rect.bottom() - 18.0),
                                egui::Vec2::splat(26.0),
                            );
                            let star = ui.interact(star_rect, ui.id().with(("font-favorite", index)), egui::Sense::click());
                            ui.painter().text(
                                star_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                if favorite { "★" } else { "☆" },
                                egui::FontId::proportional(20.0),
                                if favorite || star.hovered() {
                                    Color32::from_rgb(245, 190, 65)
                                } else {
                                    ui.visuals().weak_text_color()
                                },
                            );
                            if let Some(texture) = self.text_art_font_preview(context, index) {
                                let preview_rect = egui::Rect::from_min_max(
                                    egui::pos2(preview_left, rect.top() + 28.0),
                                    egui::pos2(rect.right() - 16.0, rect.bottom() - 10.0),
                                );
                                let size = texture.size_vec2();
                                let scale = (preview_rect.width() / size.x).min(preview_rect.height() / size.y).min(1.0);
                                let image_rect = egui::Rect::from_center_size(preview_rect.center(), size * scale);
                                ui.painter().rect_filled(preview_rect, 3, Color32::BLACK);
                                ui.painter().image(
                                    texture.id(),
                                    image_rect,
                                    egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                                    Color32::WHITE,
                                );
                            }
                            if star.clicked() {
                                self.toggle_text_art_favorite(&Self::text_art_font_id(&name, kind));
                            } else if response.clicked() {
                                self.text_font_pending = index;
                            }
                            if response.double_clicked() {
                                self.text_font_pending = index;
                                double_clicked = true;
                            }
                        }
                    });
                });
                dialog.buttons([
                    DialogButton::secondary("Export...", Action::Export).leading(),
                    DialogButton::cancel(labels::cancel(), Action::Cancel),
                    DialogButton::primary("OK", Action::Apply),
                ]);
            });
        match response.action {
            Some(Action::Export) => {
                self.export_text_art_font(self.text_font_pending);
            }
            Some(Action::Apply) => apply = true,
            Some(Action::Cancel) => return false,
            None if response.dismissed => return false,
            None => {}
        }
        if double_clicked {
            apply = true;
        }
        if apply {
            self.text_font = self.text_font_pending;
            self.text_preview = None;
            return false;
        }
        true
    }

    fn dialogs(&mut self, context: &egui::Context) {
        let Some(dialog) = self.dialog.take() else {
            return;
        };
        let mut keep = true;
        match &dialog {
            Dialog::Shortcuts => keep = !self.shortcuts_dialog(context),
            Dialog::About => keep = !self.about_dialog(context),
            Dialog::ReferenceImage => keep = !self.reference_image_dialog(context),
            Dialog::Connect => keep = !self.connect_dialog(context),
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
                            appearance::group(ui, "Lua Script", |ui| {
                                ui.add(
                                    egui::TextEdit::multiline(&mut self.script)
                                        .code_editor()
                                        .hint_text("-- Runs against the current document, e.g.\n-- buf:set_char(0, 0, \"A\")")
                                        .desired_width(f32::INFINITY)
                                        .desired_rows(if self.script_output.is_empty() { 16 } else { 10 }),
                                );
                            });
                            if !self.script_output.is_empty() {
                                appearance::group(ui, "Output", |ui| {
                                    ui.add(egui::Label::new(egui::RichText::new(&self.script_output).monospace()).wrap().selectable(true));
                                });
                            }
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
                #[derive(Clone, Copy)]
                enum Action {
                    Add,
                    Close,
                }
                let response = appearance::Dialog::new("tags").size(DialogSize::Large).show(context, |dialog| {
                    dialog.content(|ui| {
                        let tags = self.document.with_state(|state| state.get_buffer().tags.clone());
                        appearance::group(ui, &format!("Tags ({})", tags.len()), |ui| {
                            if tags.is_empty() {
                                ui.add_space(8.0);
                                ui.vertical_centered(|ui| {
                                    ui.weak("This document has no tags yet.");
                                    ui.weak("Add one here or place it with the Tag tool.");
                                });
                                ui.add_space(8.0);
                                return;
                            }
                            egui::ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
                                for (index, tag) in tags.iter().enumerate() {
                                    ui.push_id(index, |ui| {
                                        ui.horizontal(|ui| {
                                            let selected = self.document.selected_tags.contains(&index);
                                            let preview = egui::RichText::new(if tag.preview.is_empty() { "(empty)" } else { &tag.preview }).monospace();
                                            let response = ui
                                                .add_sized([180.0, 26.0], egui::Button::selectable(selected, preview))
                                                .on_hover_text("Double click to edit");
                                            if response.clicked() {
                                                self.document.selected_tags = vec![index];
                                                self.document.with_state(|state| state.set_current_tag(index));
                                            }
                                            if response.double_clicked() {
                                                edit = Some(index);
                                            }
                                            let role = match tag.tag_role {
                                                icy_engine::TagRole::Displaycode => "Display code",
                                                icy_engine::TagRole::Hyperlink => "Hyperlink",
                                            };
                                            ui.weak(format!("{}, {}  ·  {role}", tag.position.x, tag.position.y));
                                            if !tag.is_enabled {
                                                ui.weak("· disabled");
                                            }
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                if self.icons.button(ui, "delete", "Delete Tag", false).clicked() {
                                                    delete = Some(index);
                                                }
                                                if self.icons.button(ui, "text", "Edit Tag…", false).clicked() {
                                                    edit = Some(index);
                                                }
                                            });
                                        });
                                    });
                                }
                            });
                        });
                    });
                    dialog.buttons([
                        DialogButton::secondary("Add Tag…", Action::Add).leading(),
                        DialogButton::primary(labels::close(), Action::Close).cancels(),
                    ]);
                });
                add = matches!(response.action, Some(Action::Add));
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
                            appearance::group(ui, if index.is_some() { "Tag" } else { "New Tag" }, |ui| {
                                appearance::check_row(ui, "Enabled", &mut tag.is_enabled);
                                appearance::form_row(ui, "Preview", |ui| {
                                    ui.add(appearance::text_edit(&mut tag.preview).hint_text("Shown in the editor").desired_width(f32::INFINITY));
                                });
                                appearance::form_row(ui, "Replacement", |ui| {
                                    ui.add(
                                        appearance::text_edit(&mut tag.replacement_value)
                                            .hint_text("Inserted by the BBS")
                                            .desired_width(f32::INFINITY),
                                    );
                                });
                                let role = |role: icy_engine::TagRole| match role {
                                    icy_engine::TagRole::Displaycode => "Display Code",
                                    icy_engine::TagRole::Hyperlink => "Hyperlink",
                                };
                                appearance::combo_row(ui, "Role", role(tag.tag_role), |ui| {
                                    for value in [icy_engine::TagRole::Displaycode, icy_engine::TagRole::Hyperlink] {
                                        ui.selectable_value(&mut tag.tag_role, value, role(value));
                                    }
                                });
                            });
                            appearance::group(ui, "Layout", |ui| {
                                appearance::form_row(ui, "Column", |ui| {
                                    ui.add(egui::DragValue::new(&mut tag.position.x).range(0..=10000));
                                });
                                appearance::form_row(ui, "Row", |ui| {
                                    ui.add(egui::DragValue::new(&mut tag.position.y).range(0..=10000));
                                });
                                appearance::form_row(ui, "Length", |ui| {
                                    ui.add(egui::DragValue::new(&mut tag.length).range(1..=1000).suffix(" characters"));
                                });
                                let alignment = |alignment: std::fmt::Alignment| match alignment {
                                    std::fmt::Alignment::Left => "Left",
                                    std::fmt::Alignment::Center => "Center",
                                    std::fmt::Alignment::Right => "Right",
                                };
                                appearance::combo_row(ui, "Alignment", alignment(tag.alignment), |ui| {
                                    for value in [std::fmt::Alignment::Left, std::fmt::Alignment::Center, std::fmt::Alignment::Right] {
                                        ui.selectable_value(&mut tag.alignment, value, alignment(value));
                                    }
                                });
                                let placement = |placement: icy_engine::TagPlacement| match placement {
                                    icy_engine::TagPlacement::InText => "In Text",
                                    icy_engine::TagPlacement::WithGotoXY => "At Cursor Position",
                                };
                                appearance::combo_row(ui, "Placement", placement(tag.tag_placement), |ui| {
                                    for value in [icy_engine::TagPlacement::InText, icy_engine::TagPlacement::WithGotoXY] {
                                        ui.selectable_value(&mut tag.tag_placement, value, placement(value));
                                    }
                                });
                            });
                        });
                        dialog.buttons([
                            DialogButton::cancel(labels::cancel(), Action::Cancel),
                            DialogButton::primary(if index.is_some() { "Apply" } else { "Add Tag" }, Action::Apply),
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
                            appearance::group(ui, "Font", |ui| {
                                ui.add(
                                    appearance::text_edit(&mut self.font_filter)
                                        .hint_text("Search fonts")
                                        .desired_width(f32::INFINITY),
                                );
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
                                if names.is_empty() {
                                    ui.weak("No font matches the search.");
                                }
                                egui::ScrollArea::vertical().max_height(270.0).show_rows(ui, 24.0, names.len(), |ui, rows| {
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
            Dialog::TextArtFontSelect => {
                keep = self.text_art_font_dialog(context);
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
                        appearance::group(ui, &format!("Palette ({} colors)", self.palette_edit.len()), |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = egui::Vec2::splat(4.0);
                                for index in 0..self.palette_edit.len().min(256) {
                                    let (red, green, blue) = self.palette_edit.rgb(index as u32);
                                    if widgets::swatch(ui, Color32::from_rgb(red, green, blue), index == self.palette_index, 28.0)
                                        .on_hover_text(format!("{index}: #{red:02X}{green:02X}{blue:02X}"))
                                        .clicked()
                                    {
                                        self.palette_index = index;
                                    }
                                }
                            });
                        });
                        appearance::group(ui, &format!("Color {}", self.palette_index), |ui| {
                            let (red, green, blue) = self.palette_edit.rgb(self.palette_index as u32);
                            let mut rgb = [red, green, blue];
                            let mut changed = false;
                            appearance::form_row(ui, "Color", |ui| {
                                changed |= ui.color_edit_button_srgb(&mut rgb).changed();
                                ui.weak(format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]));
                            });
                            for (index, channel) in ["Red", "Green", "Blue"].into_iter().enumerate() {
                                appearance::form_row(ui, channel, |ui| {
                                    changed |= ui.add(egui::DragValue::new(&mut rgb[index]).range(0..=255)).changed();
                                });
                            }
                            if changed {
                                self.palette_edit
                                    .set_color(self.palette_index as u32, icy_engine::Color::new(rgb[0], rgb[1], rgb[2]));
                            }
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
                        appearance::group(ui, "SAUCE Information", |ui| {
                            for (index, (label, limit)) in [("Title", 35), ("Author", 20), ("Group", 20)].into_iter().enumerate() {
                                appearance::form_row(ui, label, |ui| {
                                    ui.add(
                                        appearance::text_edit(&mut self.sauce_fields[index])
                                            .char_limit(limit)
                                            .hint_text(format!("Up to {limit} characters"))
                                            .desired_width(f32::INFINITY),
                                    );
                                });
                            }
                        });
                        appearance::group(ui, "Comments", |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut self.sauce_fields[3])
                                    .hint_text("One comment line per line, up to 64 characters each")
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
                        appearance::group(ui, "Format", |ui| {
                            appearance::combo_row(ui, "File format", self.export_format.name(), |ui| {
                                for format in formats {
                                    ui.selectable_value(&mut self.export_format, format, format.name());
                                }
                            });
                            appearance::check_row(ui, "Include SAUCE record", &mut self.export_sauce);
                        });
                        let settings = &mut self.settings.export_settings;
                        if matches!(self.export_format, FileFormat::Ansi | FileFormat::AnsiMusic) {
                            appearance::group(ui, "ANSI Options", |ui| {
                                appearance::combo_row(ui, "Compatibility", settings.ansi_level.to_string(), |ui| {
                                    for &level in icy_engine::AnsiCompatibilityLevel::all() {
                                        ui.selectable_value(&mut settings.ansi_level, level, level.to_string());
                                    }
                                });
                                appearance::check_row(ui, "24-bit RGB colors", &mut settings.ansi_rgb_output);
                                appearance::check_row(ui, "Limit line length", &mut settings.max_line_length_enabled);
                                if settings.max_line_length_enabled {
                                    appearance::form_row(ui, "Maximum length", |ui| {
                                        ui.add(egui::DragValue::new(&mut settings.max_line_length).range(1..=65535).suffix(" characters"));
                                    });
                                }
                            });
                        }
                        appearance::group(ui, "Output", |ui| {
                            appearance::combo_row(ui, "Screen preparation", settings.screen_prep.to_string(), |ui| {
                                for &preparation in icy_engine::ScreenPreperation::all() {
                                    ui.selectable_value(&mut settings.screen_prep, preparation, preparation.to_string());
                                }
                            });
                            appearance::check_row(ui, "UTF-8 output", &mut settings.utf8_output);
                            appearance::check_row(ui, "Compress", &mut settings.compress);
                        });
                    });
                    dialog.buttons([
                        DialogButton::cancel(labels::cancel(), Action::Cancel),
                        DialogButton::primary("Export…", Action::Export),
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
                let current = self.document.with_state(|state| state.get_buffer().size());
                let response = appearance::Dialog::new(if resize { "resize" } else { "new" })
                    .size(DialogSize::Medium)
                    .confirm_on_enter(true)
                    .show(context, |dialog| {
                        dialog.content(|ui| {
                            if !resize {
                                appearance::group(ui, "Type", |ui| {
                                    ui.spacing_mut().item_spacing.y = 2.0;
                                    for kind in NewKind::ALL {
                                        let response = welcome::kind_row(&mut self.icons, ui, kind, self.new_kind == kind);
                                        if response.clicked() {
                                            self.new_kind = kind;
                                        }
                                    }
                                });
                            }
                            if resize || self.new_kind.has_size() {
                                appearance::group(ui, if resize { "Canvas Size" } else { "Size" }, |ui| {
                                    let preset = welcome::SIZE_PRESETS
                                        .iter()
                                        .find(|(columns, rows, _)| [*columns, *rows] == self.new_size)
                                        .map_or("Custom".to_owned(), |(columns, rows, name)| format!("{columns} × {rows}  ·  {name}"));
                                    appearance::combo_row(ui, "Preset", preset, |ui| {
                                        for (columns, rows, name) in welcome::SIZE_PRESETS {
                                            ui.selectable_value(&mut self.new_size, [columns, rows], format!("{columns} × {rows}  ·  {name}"));
                                        }
                                    });
                                    appearance::form_row(ui, "Columns", |ui| {
                                        ui.add(egui::DragValue::new(&mut self.new_size[0]).range(1..=1000).suffix(" characters"));
                                    });
                                    appearance::form_row(ui, "Rows", |ui| {
                                        ui.add(egui::DragValue::new(&mut self.new_size[1]).range(1..=10000).suffix(" lines"));
                                    });
                                    if resize {
                                        appearance::value_row(ui, "Current size", &format!("{} × {}", current.width, current.height));
                                    }
                                });
                            }
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
                        } else {
                            self.create(self.new_kind, size);
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
                            self.pending_connect = false;
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
        // Ctrl+Plus/Minus zoom the canvas instead of the whole user interface.
        context.options_mut(|options| options.zoom_with_keyboard = false);
        if !context.will_discard() {
            self.poll_collaboration();
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
                    FileAction::InsertImage => self.insert_image(&path),
                    FileAction::ReferenceImage => {
                        self.reference_path = path.display().to_string();
                        self.reference_draft.path = path;
                        self.reference_draft.visible = true;
                    }
                }
            } else {
                self.continue_after_save = false;
                self.pending_connect = false;
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
        let title = if self.show_start {
            "Icy Draw".to_owned()
        } else if self.collab.active {
            format!("{} — Icy Draw", self.collab.server)
        } else {
            format!(
                "{}{} — Icy Draw",
                if self.modified() { "*" } else { "" },
                path.and_then(|path| path.file_name())
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "Untitled".into())
            )
        };
        context.send_viewport_cmd(egui::ViewportCommand::Title(title));
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
        if self.show_start {
            self.canvas_focus = false;
            egui::CentralPanel::default().show(context, |ui| {
                if blocked {
                    ui.disable();
                }
                self.start_screen(ui);
            });
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
        if self.collab.active && self.collab.chat_visible {
            egui::TopBottomPanel::bottom("chat")
                .resizable(true)
                .default_height(220.0)
                .height_range(120.0..=520.0)
                .frame(egui::Frame::new().fill(panel_fill))
                .show(context, |ui| {
                    if blocked {
                        ui.disable();
                    }
                    self.chat_panel(ui);
                });
        }
        egui::CentralPanel::default().frame(egui::Frame::new().fill(well)).show(context, |ui| {
            self.canvas(ui, blocked || self.dialog.is_some() || self.picker || self.layer_properties_open())
        });
        if !blocked && !self.layer_properties_open() {
            self.keys(context);
        }
        self.sync_collaboration();
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
