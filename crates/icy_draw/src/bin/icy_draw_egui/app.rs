use eframe::egui::{self, Color32, Key};
use icy_draw::{box_lines::BoxStyle, brush::BrushPrimaryMode, document::Document, fl, screen_profile::AtasciiMode, selection_drag::SelectionDrag, Settings};
use icy_engine::{AddType, FileFormat, Position, Selection, Size, TextPane};
use icy_engine_edit::tools::Tool;
use icy_engine_edit::UndoState;
use icy_engine_gui::system_clipboard::{self, PasteContent};
use icy_engine_gui::{
    egui::{
        appearance::{self, labels, DialogButton, DialogSize, MessageBox, MessageKind},
        export::{ExportAction, ExportDialog, ExportRequest},
        screen::ScreenView,
    },
    ScalingMode,
};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, Sender},
};

use super::widgets::{self, Icons};
#[path = "atascii.rs"]
mod atascii;
#[path = "chrome.rs"]
mod chrome;
#[path = "collab.rs"]
mod collab;
#[path = "file_settings.rs"]
mod file_settings;
#[path = "font_select.rs"]
mod font_select;
#[path = "mcp.rs"]
mod mcp;
#[path = "menus.rs"]
mod menus;
#[path = "petscii.rs"]
mod petscii;
#[path = "recovery.rs"]
mod recovery;
#[path = "retro.rs"]
mod retro;
#[path = "settings_dialog.rs"]
mod settings_dialog;
#[path = "shade.rs"]
mod shade;
#[path = "tag_picker.rs"]
mod tag_picker;
#[path = "vt52.rs"]
mod vt52;
#[path = "welcome.rs"]
mod welcome;

enum Dialog {
    New,
    /// The screens of a Petmate workspace to open one of.
    PetmateScreens(Box<petscii::PetmatePick>),
    Resize,
    Close,
    Characters,
    FKeyCharacter(usize, usize),
    FileSettings(Box<file_settings::FileSettingsDraft>),
    Settings(Box<settings_dialog::SettingsDraft>),
    Sauce(Box<file_settings::SauceDraft>),
    Export,
    FontSelect,
    TextArtFontSelect,
    Palette,
    Tags,
    TagProperties(Option<usize>, Box<icy_engine::Tag>),
    Script,
    Monitor,
    Overwrite(PathBuf),
    AnimationOverwrite(PathBuf, super::animation::ExportFormat),
    Error(String),
    ReferenceImage,
    Shortcuts,
    About,
    Connect,
    Recovery,
    ShadeRamps(Box<shade::RampDraft>),
}

#[derive(Clone, Copy)]
enum FontSelectionTarget {
    Add,
    Replace(u8),
}

enum FileAction {
    Open,
    Save,
    SaveFont,
    SaveTdf,
    SaveAnimation,
    SaveRip,
    SaveIgs,
    ExportAnimation(super::animation::ExportFormat),
    InsertImage,
    ReferenceImage,
    ImportPalette,
    ExportPalette,
    LoadFont,
    /// A font for the ATASCII screen.
    LoadAtasciiFont,
    ImportTaglist,
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

    fn label(self) -> String {
        match self {
            Self::Outline => fl!("tdf-font-selector-type_outline"),
            Self::Block => fl!("tdf-font-selector-type_block"),
            Self::Color => fl!("tdf-font-selector-type_color"),
            Self::Figlet => fl!("tdf-font-selector-type_figlet"),
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

fn tdf_type_label(kind: icy_engine_edit::charset::TdfFontType) -> String {
    use icy_engine_edit::charset::TdfFontType;
    match kind {
        TdfFontType::Color => fl!("tdf-editor-font_type_color"),
        TdfFontType::Block => fl!("tdf-editor-font_type_block"),
        TdfFontType::Outline => fl!("tdf-editor-font_type_outline"),
    }
}

fn tdf_font_entry(index: usize, font: &retrofont::tdf::TdfFont) -> String {
    format!("{}. {} ({})", index + 1, font.name, tdf_type_label(font.font_type))
}

/// How a Select tool click combines with the current selection, matching `Document::start`.
fn selection_add_type(modifiers: egui::Modifiers) -> AddType {
    if modifiers.shift {
        AddType::Add
    } else if modifiers.ctrl || modifiers.mac_cmd {
        AddType::Subtract
    } else {
        AddType::Default
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

/// How opaque the preview of a shape being dragged is drawn over the picture.
const PREVIEW_OPACITY: f32 = 0.7;

/// Document kinds offered by the New dialog and the start screen.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NewKind {
    Ansi,
    Atascii,
    Vt52,
    Petscii,
    Rip,
    Igs,
    Animation,
    BitmapFont,
    TheDraw(icy_engine_edit::charset::TdfFontType),
}

impl NewKind {
    pub const ALL: [NewKind; 11] = [
        NewKind::Ansi,
        NewKind::Petscii,
        NewKind::Atascii,
        NewKind::Vt52,
        NewKind::Rip,
        NewKind::Igs,
        NewKind::Animation,
        NewKind::BitmapFont,
        NewKind::TheDraw(icy_engine_edit::charset::TdfFontType::Color),
        NewKind::TheDraw(icy_engine_edit::charset::TdfFontType::Block),
        NewKind::TheDraw(icy_engine_edit::charset::TdfFontType::Outline),
    ];

    pub fn name(self) -> String {
        use icy_engine_edit::charset::TdfFontType;
        match self {
            NewKind::Ansi => fl!("new-file-editor-ansi"),
            NewKind::Atascii => fl!("new-file-editor-atascii"),
            NewKind::Vt52 => fl!("new-file-editor-vt52"),
            NewKind::Petscii => fl!("new-file-editor-petscii"),
            NewKind::Rip => fl!("rip-editor-title"),
            NewKind::Igs => fl!("igs-editor-title"),
            NewKind::Animation => fl!("new-file-editor-animation"),
            NewKind::BitmapFont => fl!("new-file-template-bit_font-title"),
            NewKind::TheDraw(TdfFontType::Color) => fl!("new-file-template-color_font-title"),
            NewKind::TheDraw(TdfFontType::Block) => fl!("new-file-template-block_font-title"),
            NewKind::TheDraw(TdfFontType::Outline) => fl!("new-file-template-outline_font-title"),
        }
    }

    pub fn description(self) -> String {
        use icy_engine_edit::charset::TdfFontType;
        match self {
            NewKind::Ansi => fl!("new-kind-ansi-description"),
            NewKind::Atascii => fl!("new-kind-atascii-description"),
            NewKind::Vt52 => fl!("new-kind-vt52-description"),
            NewKind::Petscii => fl!("new-kind-petscii-description"),
            NewKind::Rip => fl!("rip-editor-description"),
            NewKind::Igs => fl!("igs-editor-description"),
            NewKind::Animation => fl!("new-kind-animation-description"),
            NewKind::BitmapFont => fl!("new-kind-bitfont-description"),
            NewKind::TheDraw(TdfFontType::Color) => fl!("new-kind-color_font-description"),
            NewKind::TheDraw(TdfFontType::Block) => fl!("new-kind-block_font-description"),
            NewKind::TheDraw(TdfFontType::Outline) => fl!("new-kind-outline_font-description"),
        }
    }

    pub fn icon(self) -> &'static str {
        use icy_engine_edit::charset::TdfFontType;
        match self {
            NewKind::Ansi => "pencil",
            NewKind::Atascii => "text",
            NewKind::Vt52 => "cursor",
            NewKind::Petscii => "spray",
            NewKind::Rip => "rectangle_outline",
            NewKind::Igs => "ellipse_filled",
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
    /// Moebius' attribute picker, opened by Escape without a selection; the render pass it opened in.
    attribute_picker: Option<u64>,
    /// The replacement list browser of the open tag properties dialog.
    tag_picker: Option<tag_picker::ReplacementPicker>,
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
    /// A text field had the keyboard in the previous frame.
    field_focused_last_frame: bool,
    export_dialog: Option<ExportDialog>,
    font_editor: Option<super::font::FontEditor>,
    palette_editor: super::palette::PaletteEditor,
    charfont: Option<icy_draw::charfont::CharFontDocument>,
    new_kind: NewKind,
    new_template: file_settings::AnsiTemplate,
    animation: Option<super::animation::AnimationEditor>,
    rip: Option<super::rip::RipEditor>,
    igs: Option<super::igs::IgsEditor>,
    /// The ATASCII editor's state while the document is an ATASCII screen.
    pub(super) atascii: Option<atascii::AtasciiEditor>,
    /// The VT52 editor's state while the document is an Atari ST text screen.
    pub(super) vt52: Option<vt52::Vt52Editor>,
    pub(super) new_vt52_resolution: icy_parser_core::TerminalResolution,
    /// The PETSCII editor's state while the document is a Commodore screen.
    pub(super) petscii: Option<petscii::PetsciiEditor>,
    pub(super) new_petscii: (icy_engine::PetsciiMachine, icy_engine::PetsciiCase),
    /// Resolution of IGS drawings created from the New dialog.
    new_igs_resolution: icy_parser_core::TerminalResolution,
    pub(super) new_atascii_mode: AtasciiMode,
    /// The text and ICY data copied last, to paste with attributes without a system clipboard.
    clipboard: Option<(String, Vec<u8>)>,
    font_selector: font_select::FontSelector,
    font_slots_open: bool,
    font_selection_target: Option<FontSelectionTarget>,
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
    about: Option<icy_engine_gui::egui::about::AboutDialog>,
    collab: collab::Collaboration,
    recovery: Option<icy_draw::recovery::Recovery>,
    offers: Vec<recovery::Offer>,
    next_autosave: Option<std::time::Instant>,
    autosave_error: Option<String>,
    autosave_failing: bool,
}

impl DrawApp {
    pub fn new() -> Self {
        #[cfg(test)]
        tests::use_english();
        let document = Document::new(Size::new(80, 25));
        let view = ScreenView::from_shared(document.screen.clone());
        let mut settings = Settings::load();
        settings.monitor_settings.scaling_mode = ScalingMode::Manual(2.0);
        let show_line_numbers = settings.show_line_numbers;
        let (sender, receiver) = mpsc::channel();
        #[cfg(test)]
        system_clipboard::disable();
        Self {
            document,
            view,
            settings,
            icons: Icons::default(),
            dialog: None,
            picker: false,
            attribute_picker: None,
            tag_picker: None,
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
            field_focused_last_frame: false,
            export_dialog: None,
            font_editor: None,
            palette_editor: Default::default(),
            charfont: None,
            new_kind: NewKind::Ansi,
            new_template: file_settings::AnsiTemplate::default(),
            animation: None,
            rip: None,
            igs: None,
            atascii: None,
            vt52: None,
            new_vt52_resolution: icy_parser_core::TerminalResolution::Medium,
            petscii: None,
            new_petscii: (icy_engine::PetsciiMachine::C64, icy_engine::PetsciiCase::Upper),
            new_igs_resolution: icy_draw::igs_document::DEFAULT_RESOLUTION,
            new_atascii_mode: AtasciiMode::default(),
            clipboard: None,
            font_selector: Default::default(),
            font_slots_open: false,
            font_selection_target: None,
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
            about: None,
            collab: collab::Collaboration::default(),
            recovery: None,
            offers: Vec::new(),
            next_autosave: None,
            autosave_error: None,
            autosave_failing: false,
        }
    }

    /// Replaces the current document with a new, empty one of the given kind.
    pub(super) fn create(&mut self, kind: NewKind, size: Size) {
        match kind {
            NewKind::Ansi => {
                let document = Document::new(size);
                let template = self.new_template;
                document.with_state(|state| template.apply(state.get_buffer_mut()));
                self.replace(document);
            }
            NewKind::Atascii => self.replace(Document::new_atascii(self.new_atascii_mode)),
            NewKind::Vt52 => self.replace(Document::new_atari_st(self.new_vt52_resolution)),
            NewKind::Petscii => self.replace(Document::new_petscii(self.new_petscii.0, self.new_petscii.1)),
            NewKind::Rip => {
                self.replace(Document::new(Size::new(80, 25)));
                self.rip = Some(super::rip::RipEditor::new());
            }
            NewKind::Igs => {
                self.replace(Document::new(Size::new(80, 25)));
                self.igs = Some(super::igs::IgsEditor::new(self.new_igs_resolution));
            }
            NewKind::Animation => {
                self.replace(Document::new(Size::new(80, 25)));
                self.animation = Some(super::animation::AnimationEditor::new());
            }
            NewKind::BitmapFont => {
                self.replace(Document::new(Size::new(80, 25)));
                // New fonts start from the IBM VGA font (CP437), like the classic editor.
                self.font_editor = Some(super::font::FontEditor::new(icy_engine::BitFont::default()));
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
        self.rip = None;
        self.igs = None;
        self.font_editor = None;
        self.document = document;
        self.chrome = chrome::Chrome::default();
        self.pipette_hover = None;
        self.reference_image = None;
        self.view = ScreenView::from_shared(self.document.screen.clone());
        self.canvas_focus = true;
        self.atascii = None;
        self.vt52 = None;
        self.petscii = None;
        match self.document.profile() {
            icy_draw::screen_profile::ScreenProfile::Atascii(_) => self.start_atascii(),
            icy_draw::screen_profile::ScreenProfile::AtariSt(_) => self.start_vt52(),
            icy_draw::screen_profile::ScreenProfile::Petscii(..) => self.start_petscii(),
            _ => {}
        }
    }

    pub fn open(&mut self, path: PathBuf) {
        self.document.finish();
        if let Some(editor) = &mut self.font_editor {
            editor.finish();
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
        if path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("petmate")) {
            self.open_petmate(path);
            return;
        }
        if path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("rip")) {
            match super::rip::RipEditor::load(&path) {
                Ok(editor) => {
                    self.replace(Document::new(Size::new(80, 25)));
                    self.rip = Some(editor);
                }
                Err(error) => self.dialog = Some(Dialog::Error(error)),
            }
            return;
        }
        if path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("ig")) {
            match super::igs::IgsEditor::load(&path) {
                Ok(editor) => {
                    self.replace(Document::new(Size::new(80, 25)));
                    self.igs = Some(editor);
                }
                Err(error) => self.dialog = Some(Dialog::Error(error)),
            }
            return;
        }
        if path.extension().is_some_and(|extension| {
            ["psf", "psfu", "fnt", "fon", "f08", "f14", "f16", "yaff"]
                .iter()
                .any(|candidate| extension.eq_ignore_ascii_case(candidate))
        }) {
            match super::font::FontEditor::load(&path) {
                Ok(editor) => {
                    self.replace(Document::new(Size::new(80, 25)));
                    self.font_editor = Some(editor);
                }
                Err(error) => self.dialog = Some(Dialog::Error(error)),
            }
            return;
        }
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
            let untitled = fl!("unsaved-title");
            let path = match action {
                FileAction::Open => dialog.pick_file(),
                FileAction::Save => dialog
                    .add_filter(fl!("file-dialog-filter-icydraw-files"), &["icy"])
                    .set_file_name(format!("{untitled}.icy"))
                    .save_file(),
                FileAction::SaveFont => dialog
                    .add_filter(fl!("file-dialog-filter-font-files"), &["psf"])
                    .set_file_name(format!("{untitled}.psf"))
                    .save_file(),
                FileAction::SaveTdf => dialog
                    .add_filter(fl!("file-dialog-filter-tdf-files"), &["tdf"])
                    .set_file_name(format!("{untitled}.tdf"))
                    .save_file(),
                FileAction::SaveAnimation => dialog
                    .add_filter(fl!("file-dialog-filter-animation-files"), &["icyanim"])
                    .set_file_name(format!("{untitled}.icyanim"))
                    .save_file(),
                FileAction::SaveRip => dialog
                    .add_filter(fl!("rip-editor-title"), &["rip"])
                    .set_file_name(format!("{untitled}.rip"))
                    .save_file(),
                FileAction::SaveIgs => dialog
                    .add_filter(fl!("igs-editor-title"), &["ig"])
                    .set_file_name(format!("{untitled}.ig"))
                    .save_file(),
                FileAction::ExportAnimation(format) => dialog
                    .add_filter(format.name(), &[format.extension()])
                    .set_file_name(format!("{untitled}.{}", format.extension()))
                    .save_file(),
                FileAction::InsertImage => dialog
                    .add_filter(fl!("file-dialog-filter-artwork"), &icy_draw::files::INSERT_ART_EXTENSIONS)
                    .add_filter(fl!("file-dialog-filter-images"), &icy_draw::files::INSERT_IMAGE_EXTENSIONS)
                    .add_filter(fl!("set-font-filter-all"), &["*"])
                    .pick_file(),
                FileAction::ReferenceImage => dialog
                    .add_filter(
                        fl!("file-dialog-filter-images"),
                        &["png", "jpg", "jpeg", "gif", "bmp", "webp", "tga", "tif", "tiff"],
                    )
                    .pick_file(),
                FileAction::ImportPalette => dialog
                    .add_filter(fl!("file-dialog-filter-palette"), icy_draw::palette_files::IMPORT_EXTENSIONS)
                    .pick_file(),
                FileAction::LoadFont => dialog
                    .add_filter(fl!("set-font-filter-fonts"), font_select::FONT_EXTENSIONS)
                    .add_filter(fl!("set-font-filter-all"), &["*"])
                    .pick_file(),
                FileAction::LoadAtasciiFont => dialog
                    .add_filter(fl!("atascii-font-filter"), &["fnt", "fon", "set", "psf", "yaff"])
                    .add_filter(fl!("set-font-filter-all"), &["*"])
                    .pick_file(),
                FileAction::ExportPalette => icy_draw::palette_files::EXPORT_FILTERS
                    .iter()
                    .fold(dialog.set_file_name("palette.gpl"), |dialog, (name, extensions)| {
                        dialog.add_filter(*name, extensions)
                    })
                    .save_file(),
                FileAction::ImportTaglist => dialog.add_filter(fl!("tag-replacements-filter"), &["toml"]).pick_file(),
            };
            let _ = sender.send(Picked { action, path });
            context.request_repaint();
        });
    }

    fn save(&mut self, context: &egui::Context, save_as: bool) {
        if let Some(editor) = &self.rip {
            if let Some(path) = editor.path().filter(|_| !save_as).map(Path::to_path_buf) {
                self.save_path(context, path, false);
            } else {
                self.choose(context, FileAction::SaveRip);
            }
            return;
        }
        if let Some(editor) = &self.igs {
            if let Some(path) = editor.path().filter(|_| !save_as).map(Path::to_path_buf) {
                self.save_path(context, path, false);
            } else {
                self.choose(context, FileAction::SaveIgs);
            }
            return;
        }
        if let Some(editor) = &mut self.font_editor {
            editor.finish();
            match editor.path.clone().filter(|_| !save_as && !editor.apply_target) {
                Some(path) => self.save_path(context, path, false),
                None => self.choose(context, FileAction::SaveFont),
            }
            return;
        }
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
        let result = if let Some(editor) = &mut self.rip {
            editor.save(&path, overwrite)
        } else if let Some(editor) = &mut self.igs {
            editor.save(&path, overwrite)
        } else if let Some(editor) = &mut self.font_editor {
            editor.save(&path, overwrite)
        } else if let Some(editor) = &mut self.animation {
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
            self.finish_recovery();
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

    /// The export dialog for the document, starting with the last used folder and options.
    pub(super) fn export_dialog(&self) -> ExportDialog {
        let (formats, sauce) = self.document.with_state(|state| {
            (
                FileFormat::save_formats_with_images_for_buffer_type(state.get_buffer().buffer_type),
                state.get_sauce_meta().clone(),
            )
        });
        let source = self.document.path.as_deref();
        let directory = self
            .settings
            .last_export_directory
            .clone()
            .or_else(|| source.and_then(Path::parent).map(Path::to_path_buf))
            .unwrap_or_default();
        let name = source
            .and_then(Path::file_stem)
            .map_or_else(|| fl!("unsaved-title"), |name| name.to_string_lossy().into_owned());
        let dialog = ExportDialog::new(formats, directory, &name)
            .with_settings(&self.settings.export_settings)
            .with_sauce(Some(sauce));
        // Documents of a home computer's screen export to its own format first.
        match self.document.profile().native_format() {
            Some((format, extension)) => dialog.with_format(format).with_extension(format, extension),
            None => dialog,
        }
    }

    pub(super) fn export(&mut self, request: &ExportRequest) -> Result<(), String> {
        let path = &request.path;
        if self
            .document
            .path
            .as_ref()
            .is_some_and(|source| source == path || source.canonicalize().ok().is_some_and(|source| Some(source) == path.canonicalize().ok()))
        {
            return Err(fl!("error-export-overwrites-document"));
        }
        super::export::write(&self.document, request)
    }

    fn result(&mut self, result: Result<(), String>) {
        if let Err(error) = result {
            self.dialog = Some(Dialog::Error(error));
        }
    }

    fn edit(&mut self, action: impl FnOnce(&mut icy_engine_edit::EditState) -> icy_engine::Result<()>) {
        let result = self.document.edit_tags(action).map_err(|error| error.to_string());
        self.result(result);
    }

    pub(super) fn open_font_selector(&mut self) {
        if self
            .document
            .with_state(|state| state.get_buffer().font_mode == icy_engine::FontMode::Unlimited)
        {
            self.font_slots_open = true;
            return;
        }
        self.choose_font(FontSelectionTarget::Replace(self.document.with_state(|state| state.get_caret().font_page())));
    }

    fn choose_font(&mut self, target: FontSelectionTarget) {
        self.font_selection_target = Some(target);
        self.font_selector = self.document.with_state(|state| font_select::FontSelector::new(state));
        self.dialog = Some(Dialog::FontSelect);
    }

    /// Selects a font for drawing, adding a slot rather than replacing one in unrestricted mode.
    fn apply_font(&mut self, font: icy_engine::BitFont) {
        let target = self.font_selection_target.take();
        let result = self
            .document
            .with_state(|state| match target {
                Some(FontSelectionTarget::Replace(slot)) => state.set_font_in_slot(slot, font),
                _ => state.apply_font(font),
            })
            .map_err(|error| error.to_string());
        let applied = result.is_ok();
        self.result(result);
        if applied && matches!(self.dialog, Some(Dialog::FontSelect)) {
            self.dialog = None;
        }
    }

    fn font_slot_entries(&self) -> (Vec<(u8, String)>, u8) {
        self.document.with_state(|state| {
            let buffer = state.get_buffer();
            let mut slots = (0..icy_engine::ANSI_SLOT_COUNT)
                .filter_map(|page| {
                    let page = page as u8;
                    buffer.font(page).map(|font| (page, font.name().to_owned()))
                })
                .collect::<Vec<_>>();
            slots.extend(
                buffer
                    .font_iter()
                    .filter(|(page, _)| usize::from(**page) >= icy_engine::ANSI_SLOT_COUNT)
                    .map(|(page, font)| (*page, font.name().to_owned())),
            );
            slots.sort_by_key(|(page, _)| *page);
            (slots, state.get_caret().font_page())
        })
    }

    fn font_slots_window(&mut self, context: &egui::Context, blocked: bool) {
        if !self.font_slots_open
            || self
                .document
                .with_state(|state| state.get_buffer().font_mode != icy_engine::FontMode::Unlimited)
        {
            self.font_slots_open = false;
            return;
        }
        let (slots, active) = self.font_slot_entries();
        let mut open = self.font_slots_open;
        let mut selected = None;
        let mut target = None;
        egui::Window::new(fl!("font-slots-title"))
            .id(egui::Id::new("font-slots"))
            .open(&mut open)
            .default_width(280.0)
            .default_height(360.0)
            .show(context, |ui| {
                if blocked || self.document.paste_active() {
                    ui.disable();
                }
                ui.label(fl!("font-slots-active", slot = i64::from(active)));
                ui.horizontal(|ui| {
                    if ui.add_enabled(slots.len() < 256, egui::Button::new(fl!("font-slots-add"))).clicked() {
                        target = Some(FontSelectionTarget::Add);
                    }
                    if ui.button(fl!("font-slots-replace")).on_hover_text(fl!("font-slots-replace-tip")).clicked() {
                        target = Some(FontSelectionTarget::Replace(active));
                    }
                });
                if slots.len() == 256 {
                    ui.weak(fl!("font-slots-full"));
                }
                ui.separator();
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    let mut custom = false;
                    for (slot, name) in &slots {
                        if *slot == 0 {
                            ui.weak(fl!("font-slots-predefined"));
                        } else if !custom && usize::from(*slot) >= icy_engine::ANSI_SLOT_COUNT {
                            ui.weak(fl!("font-slots-custom"));
                            custom = true;
                        }
                        if ui
                            .selectable_label(*slot == active, format!("{slot:3}  {name}"))
                            .on_hover_text(fl!("status-font-slot-tooltip", slot = i64::from(*slot), font = name.as_str()))
                            .clicked()
                        {
                            selected = Some(*slot);
                        }
                    }
                });
            });
        self.font_slots_open = open;
        if let Some(slot) = selected {
            self.edit(|state| state.switch_to_font_page(slot));
        }
        if let Some(target) = target {
            self.choose_font(target);
        }
    }

    fn modified(&self) -> bool {
        // The collaboration server persists the shared document.
        if self.collab.active {
            return false;
        }
        // A font of its own is the document; a font taken from the drawing only changes it once applied.
        if let Some(editor) = self.font_editor.as_ref().filter(|editor| !editor.apply_target) {
            return editor.modified();
        }
        self.document.modified()
            || self.rip.as_ref().is_some_and(|editor| editor.modified())
            || self.igs.as_ref().is_some_and(|editor| editor.modified())
            || self.charfont.as_ref().is_some_and(|font| font.modified())
            || self.animation.as_ref().is_some_and(|editor| editor.modified())
    }

    fn undo(&mut self, redo: bool) {
        if let Some(editor) = &mut self.rip {
            editor.undo(redo);
            return;
        }
        if let Some(editor) = &mut self.igs {
            editor.undo(redo);
            return;
        }
        if let Some(editor) = &mut self.font_editor {
            editor.undo(redo);
            return;
        }
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
        let fonts: Vec<_> = font.state.fonts().iter().enumerate().map(|(index, font)| tdf_font_entry(index, font)).collect();
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
                    .selected_text(fonts.get(selected).map(String::as_str).unwrap_or_default())
                    .show_ui(ui, |ui| {
                        for (index, label) in fonts.iter().enumerate() {
                            if ui.selectable_value(&mut selected, index, label).changed() {
                                self.change_charfont(|state| state.select_font(index));
                            }
                        }
                    });
                if ui.add(egui::TextEdit::singleline(&mut name).desired_width(120.0).char_limit(12)).changed() {
                    self.change_charfont(|state| state.set_font_name(name));
                }
                if ui
                    .add(
                        egui::DragValue::new(&mut spacing)
                            .range(0..=40)
                            .prefix(format!("{} ", fl!("tdf-editor-spacing_label"))),
                    )
                    .changed()
                {
                    self.change_charfont(|state| state.set_font_spacing(spacing));
                }
                if self.icons.button(ui, "file_copy", &fl!("tdf-duplicate-font"), false).clicked() {
                    self.change_charfont(|state| state.clone_font());
                }
                if self.icons.button(ui, "delete", &fl!("tdf-delete-font"), false).clicked() && fonts.len() > 1 {
                    self.change_charfont(|state| state.delete_font());
                }
                ui.menu_button(fl!("tdf-dialog-add-font-title"), |ui| {
                    for kind in [
                        icy_engine_edit::charset::TdfFontType::Color,
                        icy_engine_edit::charset::TdfFontType::Block,
                        icy_engine_edit::charset::TdfFontType::Outline,
                    ] {
                        if ui.button(tdf_type_label(kind)).clicked() {
                            self.change_charfont(|state| state.add_font(kind, fl!("tdf-new-font-name"), 1));
                            ui.close();
                        }
                    }
                });
                egui::ComboBox::from_id_salt("tdf-character")
                    .selected_text(format!("{} {character}", fl!("tool-character")))
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
        let fonts: Vec<_> = font.state.fonts().iter().enumerate().map(|(index, font)| tdf_font_entry(index, font)).collect();
        let mut selected = font.state.selected_font_index();
        let character = font.state.selected_char().unwrap_or('A');
        let mut name = font.state.selected_font().map(|font| font.name.clone()).unwrap_or_default();
        let mut spacing = font.state.selected_font().map_or(0, |font| font.spacing);
        let glyphs: Vec<bool> = (33..=126u32).map(|code| font.state.has_glyph(char::from_u32(code).unwrap())).collect();
        widgets::section_header(ui, &fl!("new-file-editor-tdf"), |ui| {
            let add = self.icons.button_sized(ui, "add", &fl!("tdf-dialog-add-font-title"), false, 26.0);
            egui::Popup::menu(&add).show(|ui| {
                for kind in [
                    icy_engine_edit::charset::TdfFontType::Color,
                    icy_engine_edit::charset::TdfFontType::Block,
                    icy_engine_edit::charset::TdfFontType::Outline,
                ] {
                    if ui.button(tdf_type_label(kind)).clicked() {
                        self.change_charfont(|state| state.add_font(kind, fl!("tdf-new-font-name"), 1));
                        ui.close();
                    }
                }
            });
            ui.add_enabled_ui(fonts.len() > 1, |ui| {
                if self.icons.button_sized(ui, "delete", &fl!("tdf-delete-font"), false, 26.0).clicked() {
                    self.change_charfont(|state| state.delete_font());
                }
            });
            if self.icons.button_sized(ui, "file_copy", &fl!("tdf-duplicate-font"), false, 26.0).clicked() {
                self.change_charfont(|state| state.clone_font());
            }
        });
        egui::ComboBox::from_id_salt("tdf-font")
            .width(ui.available_width())
            .selected_text(fonts.get(selected).map(String::as_str).unwrap_or_default())
            .show_ui(ui, |ui| {
                for (index, label) in fonts.iter().enumerate() {
                    if ui.selectable_value(&mut selected, index, label).changed() {
                        self.change_charfont(|state| state.select_font(index));
                    }
                }
            });
        ui.add_space(2.0);
        egui::Grid::new("tdf-font-properties").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
            ui.label(fl!("edit-layer-dialog-name-label"));
            if ui
                .add(egui::TextEdit::singleline(&mut name).desired_width(f32::INFINITY).char_limit(12))
                .changed()
            {
                self.change_charfont(|state| state.set_font_name(name));
            }
            ui.end_row();
            ui.label(fl!("tdf-editor-spacing_label").trim_end_matches(':'));
            if ui.add(egui::DragValue::new(&mut spacing).range(0..=40)).changed() {
                self.change_charfont(|state| state.set_font_spacing(spacing));
            }
            ui.end_row();
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
                fl!("tdf-edit-glyph", ch = code.to_string())
            } else {
                fl!("tdf-create-glyph", ch = code.to_string())
            });
        }
    }

    /// The select tool's modes and what it does with the selection, in every editor's toolbar.
    pub(super) fn selection_options(&mut self, ui: &mut egui::Ui, context: &egui::Context) {
        use icy_draw::document::SelectionMode;
        let mut mode = self.document.selection_mode;
        if widgets::segmented(
            ui,
            &mut mode,
            &[
                (SelectionMode::Rectangle, fl!("tool-select-normal"), fl!("select-mode-normal-tooltip")),
                (SelectionMode::Character, fl!("tool-select-character"), fl!("select-mode-character-tooltip")),
                (SelectionMode::Attribute, fl!("tool-select-attribute"), fl!("select-mode-attribute-tooltip")),
                (SelectionMode::Foreground, fl!("tool-select-foreground"), fl!("select-mode-foreground-tooltip")),
                (SelectionMode::Background, fl!("tool-select-background"), fl!("select-mode-background-tooltip")),
            ],
        ) {
            self.document.finish();
            self.document.selection_mode = mode;
        }
        widgets::divider(ui);
        let selected = self.document.with_state(|state| state.is_something_selected());
        let paint = self.document.can_paint();
        let shortcut = |label: String, shortcut: &egui::KeyboardShortcut| format!("{label} ({})", context.format_shortcut(shortcut));
        let key = |label: String, key: &str| format!("{label} ({key})");
        if self
            .icons
            .button(ui, "select", &shortcut(fl!("menu-select-all"), &menus::SELECT_ALL), false)
            .clicked()
        {
            self.select_all();
        }
        ui.add_enabled_ui(selected, |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            if self
                .icons
                .button(ui, "deselect", &shortcut(fl!("select-deselect"), &menus::DESELECT), false)
                .clicked()
            {
                self.edit(|state| state.clear_selection());
            }
            widgets::divider(ui);
            if self.icons.button(ui, "file_copy", &fl!("select-copy"), false).clicked() {
                self.copy(context);
            }
            ui.add_enabled_ui(paint, |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                if self.icons.button(ui, "library_add", &key(fl!("shortcut-block-copy"), "C"), false).clicked() {
                    let result = self.document.float_selection(false);
                    self.result(result);
                }
                if self.icons.button(ui, "move", &key(fl!("shortcut-block-move"), "M"), false).clicked() {
                    let result = self.document.float_selection(true);
                    self.result(result);
                }
                widgets::divider(ui);
                if self.icons.button(ui, "fill", &key(fl!("shortcut-block-fill"), "F"), false).clicked() {
                    let result = self.document.fill_selection();
                    self.result(result);
                }
                let erase = egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::Delete);
                if self
                    .icons
                    .button(ui, "eraser", &shortcut(fl!("shortcut-erase-selection"), &erase), false)
                    .clicked()
                {
                    self.document.finish();
                    self.edit(|state| state.erase_selection());
                }
                if self.icons.button(ui, "crop", &shortcut(fl!("menu-crop"), &menus::CROP), false).clicked() {
                    self.edit(|state| state.crop());
                }
                widgets::divider(ui);
                if self.icons.button(ui, "flip_tool", &fl!("menu-flip-x"), false).clicked() {
                    self.edit(|state| state.flip_x());
                }
                if self.icons.button(ui, "swap", &fl!("menu-flip-y"), false).clicked() {
                    self.edit(|state| state.flip_y());
                }
                let justify = self.icons.button(ui, "format_align_center", &fl!("select-justify"), false);
                egui::Popup::menu(&justify).id(egui::Id::new("select-justify")).show(|ui| {
                    if ui.button(fl!("menu-justifyleft")).clicked() {
                        self.edit(|state| state.justify_left());
                        ui.close();
                    }
                    if ui.button(fl!("menu-justifycenter")).clicked() {
                        self.edit(|state| state.center());
                        ui.close();
                    }
                    if ui.button(fl!("menu-justifyright")).clicked() {
                        self.edit(|state| state.justify_right());
                        ui.close();
                    }
                });
            });
        });
        widgets::divider(ui);
        match selection_add_type(ui.input(|input| input.modifiers)) {
            AddType::Add => {
                ui.label(egui::RichText::new(fl!("select-mode-add")).color(appearance::PRIMARY).strong());
            }
            AddType::Subtract => {
                ui.label(egui::RichText::new(fl!("select-mode-subtract")).color(appearance::PRIMARY).strong());
            }
            AddType::Default => {
                if let Some(bounds) = self.document.with_state(|state| state.selection().map(|selection| selection.as_rectangle())) {
                    ui.weak(format!("{}, {}  ·  {} × {}", bounds.left(), bounds.top(), bounds.width(), bounds.height()));
                } else {
                    ui.weak(fl!("tool-select-description"));
                }
            }
        }
    }

    /// Anchoring, a new layer, stamping, rotating, flipping and cancelling of a floating paste.
    pub(super) fn paste_options(&mut self, ui: &mut egui::Ui) {
        use icy_draw::document::PasteAction;
        for (icon, label, action) in [
            ("anchor", fl!("paste-tool-anchor"), PasteAction::Anchor),
            ("add_layer", fl!("paste-tool-keep"), PasteAction::Keep),
            ("file_copy", fl!("paste-tool-stamp"), PasteAction::Stamp),
            ("replay", fl!("paste-tool-rotate"), PasteAction::Rotate),
            ("flip_tool", format!("{} (X)", fl!("paste-tool-flip-x")), PasteAction::FlipX),
            ("swap", format!("{} (Y)", fl!("paste-tool-flip-y")), PasteAction::FlipY),
            ("invisible", fl!("paste-tool-transparent"), PasteAction::Transparent),
            ("delete", fl!("paste-tool-cancel"), PasteAction::Cancel),
        ] {
            if action == PasteAction::Keep && self.charfont.is_some() {
                continue;
            }
            if action == PasteAction::Cancel {
                widgets::divider(ui);
            }
            let selected = action == PasteAction::Transparent && self.document.paste_transparent();
            if self.icons.button(ui, icon, &label, selected).clicked() {
                let result = self.document.paste_action(action);
                self.result(result);
                self.canvas_focus = true;
            }
        }
    }

    fn tool_options(&mut self, ui: &mut egui::Ui, context: &egui::Context) {
        ui.spacing_mut().item_spacing.x = 4.0;
        if self.document.paste_active() {
            self.paste_options(ui);
            return;
        }
        if self.document.outline_font && self.document.tool == Tool::Click {
            let font = self.document.with_state(|state| state.get_buffer().font(0).cloned());
            if let Some(font) = font {
                let style = self.chrome.outline_style;
                ui.spacing_mut().item_spacing.x = 0.0;
                widgets::outline_sheet_captions(ui);
                for (index, (key, code)) in icy_draw::document::OUTLINE_KEYS.iter().enumerate() {
                    if index == 10 {
                        widgets::divider(ui);
                    }
                    let (label, result) = match *code {
                        '@' | '&' => (code.to_string(), Some(*code)),
                        '\u{00ff}' => ("FF".to_owned(), None),
                        code => (code.to_string(), Some(widgets::outline_result(style, code as u8))),
                    };
                    let hover = match *code {
                        '@' => fl!("outline-code-fill"),
                        '&' => fl!("outline-code-end"),
                        'O' => fl!("outline-code-hole"),
                        '\u{00ff}' => fl!("shortcut-hard-blank"),
                        _ => fl!("outline-code-placeholder", code = label.as_str()),
                    };
                    if widgets::outline_key(ui, &font, key, &label, result)
                        .on_hover_text(format!("{key}: {hover}"))
                        .clicked()
                    {
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
                let all_modes = [
                    (BrushPrimaryMode::Char, fl!("tool-character"), fl!("brush-mode-char-tooltip")),
                    (BrushPrimaryMode::HalfBlock, fl!("tool-half-block"), fl!("brush-mode-half_block-tooltip")),
                    (BrushPrimaryMode::Shading, fl!("tool-shade"), fl!("brush-mode-shading-tooltip")),
                    (BrushPrimaryMode::Replace, fl!("brush-mode-replace"), fl!("brush-mode-replace-tooltip")),
                    (BrushPrimaryMode::Blink, fl!("color-is_blinking"), fl!("brush-mode-blink-tooltip")),
                    (BrushPrimaryMode::Colorize, fl!("tool-colorize"), fl!("brush-mode-colorize-tooltip")),
                ];
                let fill_modes = [
                    (BrushPrimaryMode::HalfBlock, fl!("tool-half-block"), fl!("brush-mode-half_block-tooltip")),
                    (BrushPrimaryMode::Char, fl!("tool-character"), fl!("brush-mode-char-tooltip")),
                    (BrushPrimaryMode::Colorize, fl!("tool-colorize"), fl!("brush-mode-colorize-tooltip")),
                ];
                let brush_modes = if tool == Tool::Fill { fill_modes.as_slice() } else { all_modes.as_slice() };
                // `None` is the line tool's outline mode, which draws box-drawing lines instead of the brush.
                let mut modes: Vec<(Option<BrushPrimaryMode>, String, String)> = brush_modes
                    .iter()
                    .map(|(mode, label, tooltip)| (Some(*mode), label.clone(), tooltip.clone()))
                    .collect();
                if matches!(tool, Tool::Line | Tool::RectangleOutline) {
                    modes.push((None, fl!("line-style-outline"), fl!("line-style-outline-tooltip")));
                }
                let primary = match self.document.brush.primary {
                    BrushPrimaryMode::Char | BrushPrimaryMode::HalfBlock | BrushPrimaryMode::Colorize => self.document.brush.primary,
                    _ if tool == Tool::Fill => BrushPrimaryMode::Char,
                    mode => mode,
                };
                let outline = self.document.draws_boxes();
                let mut choice = if outline { None } else { Some(primary) };
                if widgets::segmented(ui, &mut choice, &modes) {
                    match choice {
                        Some(mode) => {
                            self.document.brush.primary = mode;
                            self.document.box_line = None;
                        }
                        None => self.document.box_line = Some(self.document.box_style),
                    }
                }
                match choice {
                    None => {
                        widgets::divider(ui);
                        self.outline_style(ui);
                    }
                    Some(BrushPrimaryMode::Shading) => {
                        widgets::divider(ui);
                        self.shade_options(ui, font.as_ref());
                    }
                    Some(BrushPrimaryMode::Char | BrushPrimaryMode::Replace) => {
                        if let Some(font) = &font {
                            widgets::divider(ui);
                            if widgets::glyph(ui, font, self.document.brush.paint_char, false, widgets::CONTROL_HEIGHT)
                                .on_hover_text(fl!("brush-char-tooltip"))
                                .clicked()
                            {
                                self.dialog = Some(Dialog::Characters);
                            }
                        }
                    }
                    _ => {}
                }
                if self.document.tool == Tool::Pencil {
                    widgets::divider(ui);
                    ui.weak(fl!("brush-size"));
                    ui.spacing_mut().item_spacing.x = 0.0;
                    if self.icons.button(ui, "arrow_left", &fl!("shortcut-brush-smaller"), false).clicked() {
                        self.document.brush.brush_size = self.document.brush.brush_size.saturating_sub(1).max(1);
                    }
                    ui.add(egui::DragValue::new(&mut self.document.brush.brush_size).range(1..=9))
                        .on_hover_text(fl!("brush-size-tooltip"));
                    if self.icons.button(ui, "arrow_right", &fl!("shortcut-brush-larger"), false).clicked() {
                        self.document.brush.brush_size = (self.document.brush.brush_size + 1).min(9);
                    }
                    ui.spacing_mut().item_spacing.x = 4.0;
                }
                widgets::divider(ui);
                ui.weak(fl!("brush-apply"));
                widgets::toggle(
                    ui,
                    &fl!("channel_tool_fg"),
                    &mut self.document.brush.colorize_fg,
                    &fl!("brush-apply-fg-tooltip"),
                );
                widgets::toggle(
                    ui,
                    &fl!("channel_tool_bg"),
                    &mut self.document.brush.colorize_bg,
                    &fl!("brush-apply-bg-tooltip"),
                );
                if self.document.tool == Tool::Fill {
                    widgets::divider(ui);
                    widgets::toggle(
                        ui,
                        &fl!("tool-fill-exact_match_label"),
                        &mut self.document.brush.exact,
                        &fl!("brush-exact-tooltip"),
                    );
                }
                let variants = match self.document.tool {
                    Tool::RectangleOutline | Tool::RectangleFilled => Some([Tool::RectangleOutline, Tool::RectangleFilled]),
                    Tool::EllipseOutline | Tool::EllipseFilled => Some([Tool::EllipseOutline, Tool::EllipseFilled]),
                    _ => None,
                };
                if let Some(variants) = variants {
                    widgets::divider(ui);
                    for tool in variants {
                        if self
                            .icons
                            .button(ui, tool.icon(), &chrome::tool_label(tool), self.document.tool == tool)
                            .clicked()
                        {
                            self.select_tool(tool);
                        }
                    }
                }
            }
            Tool::Font => {
                if let Some(library) = &self.text_fonts {
                    let font_name = library
                        .read()
                        .font_name(self.text_font)
                        .map_or_else(|| fl!("font-tool-no_fonts"), str::to_owned);
                    if ui
                        .add_sized([290.0, widgets::CONTROL_HEIGHT], egui::Button::new(font_name).truncate())
                        .on_hover_text(fl!("font-tool-select_font"))
                        .clicked()
                    {
                        self.text_font_pending = self.text_font;
                        self.scroll_to_text_font = true;
                        self.dialog = Some(Dialog::TextArtFontSelect);
                    }
                    widgets::divider(ui);
                    ui.weak(fl!("tdf-font-selector-type_outline"));
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
                    if self.icons.button(ui, "navigate_prev", &fl!("shortcut-fkey-prev"), false).clicked() {
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
                    if self.icons.button(ui, "navigate_next", &fl!("shortcut-fkey-next"), false).clicked() {
                        self.settings.fkeys.current_set = (self.settings.fkeys.current_set + 1) % self.settings.fkeys.set_count();
                    }
                    ui.add_space(4.0);
                    ui.weak(fl!(
                        "fkey-set-of",
                        set = (self.settings.fkeys.current_set + 1),
                        count = self.settings.fkeys.set_count()
                    ))
                    .on_hover_text(fl!("fkey-set-tooltip"));
                    ui.spacing_mut().item_spacing.x = 4.0;
                    widgets::divider(ui);
                    if self.icons.button(ui, "font", &fl!("char_table_tool_title"), false).clicked() {
                        self.dialog = Some(Dialog::Characters);
                    }
                }
            }
            Tool::Pipette => {
                self.pipette_options(ui);
            }
            Tool::Select => self.selection_options(ui, context),

            Tool::Tag => {
                if self.icons.button(ui, "tag", &fl!("tag-list-title"), false).clicked() {
                    self.dialog = Some(Dialog::Tags);
                }
                if self.icons.button(ui, "add", &fl!("tag-new"), false).clicked() {
                    self.open_tag_properties(None);
                }
                ui.add_enabled_ui(!self.document.selected_tags.is_empty(), |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    if self.icons.button(ui, "text", &fl!("tag-edit"), false).clicked() {
                        self.open_tag_properties(self.document.selected_tags.first().copied());
                    }
                    if self.icons.button(ui, "delete", &fl!("tag-delete-selected"), false).clicked() {
                        let result = self.document.delete_selected_tags();
                        self.result(result);
                    }
                });
                widgets::divider(ui);
                match self.document.selected_tags.len() {
                    0 => {
                        ui.weak(fl!("tag-place-hint"));
                    }
                    1 => {
                        let index = self.document.selected_tags[0];
                        if let Some(tag) = self.document.with_state(|state| state.get_buffer().tags.get(index).cloned()) {
                            let preview: String = tag.preview.chars().take(20).collect();
                            ui.strong(if preview.is_empty() { fl!("tag-empty") } else { preview.clone() });
                            ui.weak(fl!("tag-info", x = tag.position.x, y = tag.position.y, length = tag.len()));
                            ui.weak("→");
                            if tag.replacement_value.is_empty() {
                                ui.weak(fl!("tag-toolbar-no-replacement"));
                            } else {
                                ui.label(egui::RichText::new(tag.replacement_value.chars().take(30).collect::<String>()).monospace());
                            }
                        }
                    }
                    count => {
                        ui.weak(fl!("tag-toolbar-selected-tags", count = count));
                    }
                };
            }
            _ => {}
        }
    }

    fn pipette_options(&mut self, ui: &mut egui::Ui) {
        let Some((position, modifiers)) = self.pipette_hover else {
            ui.weak(fl!("pipette-hover_hint"));
            widgets::divider(ui);
            ui.weak(fl!("pipette-modifier-hint"));
            return;
        };
        let Some((character, font, foreground, background, preview_foreground, preview_background)) = self.document.with_state(|state| {
            let buffer = state.get_buffer();
            let character = buffer.char_at(position);
            let font = buffer.font(character.attribute.font_page()).or_else(|| buffer.font(0)).cloned()?;
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
        chrome::color_sample(
            ui,
            &fl!("pipette-foreground", index = character.attribute.foreground()),
            foreground,
            take_foreground,
        );
        chrome::color_sample(
            ui,
            &fl!("pipette-background", index = character.attribute.background()),
            background,
            take_background,
        );
        widgets::divider(ui);
        ui.weak(fl!("pipette-modifier-hint"));
    }

    /// Canvas size controls of the New and Canvas Size dialogs, with the ANSI format for new documents.
    fn new_size_group(&mut self, ui: &mut egui::Ui, resize: bool, current: Size) {
        let title = if resize {
            fl!("file-settings-canvas-size")
        } else {
            fl!("new-file-size-section")
        };
        // Home computer screens have the width of their text mode.
        let fixed_width = if resize { self.document.profile().fixed_width() } else { None };
        if let Some(width) = fixed_width {
            self.new_size[0] = width;
        }
        appearance::group(ui, &title, |ui| {
            if !resize && self.new_kind == NewKind::Ansi {
                appearance::combo_row(ui, &fl!("file-settings-format"), self.new_template.title(), |ui| {
                    for template in file_settings::AnsiTemplate::ALL {
                        ui.selectable_value(&mut self.new_template, template, template.title())
                            .on_hover_text(template.description());
                    }
                });
                ui.add(egui::Label::new(egui::RichText::new(self.new_template.description()).weak()).wrap());
            }
            let presets = if fixed_width.is_some() {
                Vec::new()
            } else {
                welcome::size_presets().to_vec()
            };
            let preset = presets
                .iter()
                .find(|(columns, rows, _)| [*columns, *rows] == self.new_size)
                .map_or_else(|| fl!("new-file-custom-size"), |(columns, rows, name)| format!("{columns} × {rows}  ·  {name}"));
            if !presets.is_empty() {
                appearance::combo_row(ui, &fl!("new-file-preset"), preset, |ui| {
                    for (columns, rows, name) in &presets {
                        ui.selectable_value(&mut self.new_size, [*columns, *rows], format!("{columns} × {rows}  ·  {name}"));
                    }
                });
            }
            appearance::form_row(ui, &fl!("new-file-width"), |ui| {
                let width = ui.add_enabled(
                    fixed_width.is_none(),
                    egui::DragValue::new(&mut self.new_size[0])
                        .range(1..=1000)
                        .suffix(format!(" {}", fl!("unit-characters"))),
                );
                if fixed_width.is_some() {
                    width.on_disabled_hover_text(fl!("canvas-fixed-width"));
                }
            });
            appearance::form_row(ui, &fl!("new-file-height"), |ui| {
                ui.add(
                    egui::DragValue::new(&mut self.new_size[1])
                        .range(1..=10000)
                        .suffix(format!(" {}", fl!("unit-lines"))),
                );
            });
            if resize {
                appearance::value_row(ui, &fl!("canvas-current-size"), &format!("{} × {}", current.width, current.height));
            }
        });
    }

    /// The resolution of a new IGS drawing.
    fn new_igs_group(&mut self, ui: &mut egui::Ui) {
        use icy_parser_core::TerminalResolution;
        appearance::group(ui, &fl!("igs-new-resolution"), |ui| {
            for resolution in [TerminalResolution::Low, TerminalResolution::Medium, TerminalResolution::High] {
                let name = match resolution {
                    TerminalResolution::Low => fl!("igs-resolution-low"),
                    TerminalResolution::Medium => fl!("igs-resolution-medium"),
                    TerminalResolution::High => fl!("igs-resolution-high"),
                };
                ui.radio_value(&mut self.new_igs_resolution, resolution, name);
            }
        });
    }

    /// The screen of a new ATASCII document.
    fn new_atascii_group(&mut self, ui: &mut egui::Ui) {
        appearance::group(ui, &fl!("atascii-new-mode"), |ui| {
            for mode in AtasciiMode::ALL {
                let size = mode.screen_size();
                let name = match mode {
                    AtasciiMode::Antic => fl!("atascii-mode-antic", columns = size.width, rows = size.height),
                    AtasciiMode::Xep80 => fl!("atascii-mode-xep80", columns = size.width, rows = size.height),
                };
                ui.radio_value(&mut self.new_atascii_mode, mode, name);
            }
            ui.add(egui::Label::new(egui::RichText::new(fl!("atascii-new-hint")).weak()).wrap());
        });
    }

    /// The machine and character set of a new PETSCII document.
    fn new_petscii_group(&mut self, ui: &mut egui::Ui) {
        appearance::group(ui, &fl!("petscii-new-screen"), |ui| {
            for machine in petscii::MACHINES {
                ui.radio_value(&mut self.new_petscii.0, machine, petscii::machine_name(machine));
            }
            ui.separator();
            for case in petscii::CASES {
                ui.radio_value(&mut self.new_petscii.1, case, petscii::case_name(case));
            }
            ui.add(egui::Label::new(egui::RichText::new(fl!("petscii-new-hint")).weak()).wrap());
        });
    }

    /// The resolution of a new VT52 document.
    fn new_vt52_group(&mut self, ui: &mut egui::Ui) {
        appearance::group(ui, &fl!("vt52-new-resolution"), |ui| {
            for resolution in vt52::RESOLUTIONS {
                let name = format!("{} · {}", vt52::resolution_name(resolution), vt52::resolution_detail(resolution));
                ui.radio_value(&mut self.new_vt52_resolution, resolution, name);
            }
            ui.add(egui::Label::new(egui::RichText::new(fl!("vt52-new-hint")).weak()).wrap());
        });
    }

    /// Leaves the bitmap font editor; a font of its own returns to the start screen.
    fn close_font_editor(&mut self) {
        let standalone = self.font_editor.take().is_some_and(|editor| !editor.apply_target);
        self.canvas_focus = !standalone;
        self.show_start |= standalone;
    }

    /// File name of the active document, or the localized "Untitled".
    fn document_name(&self) -> String {
        if let Some(path) = self.igs.as_ref().map(|editor| editor.path()) {
            return path
                .and_then(|path| path.file_name())
                .map_or_else(|| fl!("unsaved-title"), |name| name.to_string_lossy().into_owned());
        }
        if let Some(editor) = &self.rip {
            return editor
                .path()
                .and_then(|path| path.file_name())
                .map_or_else(|| fl!("unsaved-title"), |name| name.to_string_lossy().into_owned());
        }
        if let Some(editor) = self.font_editor.as_ref().filter(|editor| !editor.apply_target) {
            return editor
                .path
                .as_ref()
                .and_then(|path| path.file_name())
                .map_or_else(|| fl!("unsaved-title"), |name| name.to_string_lossy().into_owned());
        }
        self.animation
            .as_ref()
            .and_then(|editor| editor.path.as_ref())
            .or(self.charfont.as_ref().and_then(|font| font.path.as_ref()))
            .or(self.document.path.as_ref())
            .and_then(|path| path.file_name())
            .map_or_else(|| fl!("unsaved-title"), |name| name.to_string_lossy().into_owned())
    }

    /// Pointer positions per character: two rows for half blocks, 2 × 2 for quarter block pixels.
    fn sub_cells(&self) -> (i32, i32) {
        if self.document.draws_pixels() {
            (2, 2)
        } else if self.half_blocks() {
            (1, 2)
        } else {
            (1, 1)
        }
    }

    fn half_blocks(&self) -> bool {
        self.document.brush.primary == BrushPrimaryMode::HalfBlock
            && (self.document.tool == Tool::Pencil || self.document.tool == Tool::Fill || self.document.tool.is_shape_tool())
            && !self.document.draws_boxes()
    }

    /// The outline mode's line style, shown beside the modes like the shading options.
    fn outline_style(&mut self, ui: &mut egui::Ui) {
        let mut style = self.document.box_style;
        let options = [
            (BoxStyle::Single, BoxStyle::Single.sample().to_string(), fl!("line-style-single-tooltip")),
            (BoxStyle::Double, BoxStyle::Double.sample().to_string(), fl!("line-style-double-tooltip")),
            (
                BoxStyle::DoubleHorizontal,
                BoxStyle::DoubleHorizontal.sample().to_string(),
                fl!("line-style-double-horizontal-tooltip"),
            ),
            (
                BoxStyle::DoubleVertical,
                BoxStyle::DoubleVertical.sample().to_string(),
                fl!("line-style-double-vertical-tooltip"),
            ),
        ];
        if widgets::segmented(ui, &mut style, &options) {
            self.document.box_style = style;
            self.document.box_line = Some(style);
        }
    }

    fn position(&self, point: egui::Pos2) -> Option<Position> {
        let info = self.view.terminal.render_info.read();
        let (horizontal, vertical) = info.screen_to_terminal_pixels(point.x, point.y)?;
        let vertical = if info.scan_lines { vertical / 2.0 } else { vertical };
        let (columns, rows) = self.sub_cells();
        let width = info.font_width / columns as f32;
        let height = info.font_height / rows as f32;
        let position = Position::new(
            ((horizontal + self.view.terminal.scroll_x()) / width.max(1.0)).floor() as i32,
            ((vertical + self.view.terminal.scroll_y()) / height.max(1.0)).floor() as i32,
        );
        let size = self.document.with_state(|state| state.get_buffer().size());
        (position.x >= 0 && position.x < size.width * columns && position.y >= 0 && position.y < size.height * rows).then_some(position)
    }

    fn canvas(&mut self, ui: &mut egui::Ui, blocked: bool) {
        self.view.terminal.has_focus = self.canvas_focus && !blocked;
        self.document
            .with_state(|state| state.set_caret_visible(!self.document.paste_active() && matches!(self.document.tool, Tool::Click | Tool::Font)));
        if let Some(mode) =
            icy_engine_gui::egui::zoom::mouse_wheel(ui, !blocked, self.view.zoom, self.settings.monitor_settings.use_integer_scaling, 0.25..=8.0)
        {
            self.settings.monitor_settings.scaling_mode = mode;
        }
        self.view.markers = Some(self.editor_markers());
        let mut response = self.view.show(ui, &self.settings.monitor_settings);
        self.canvas_rect = response.rect;
        if !blocked {
            ui.memory_mut(|memory| {
                memory.set_focus_lock_filter(
                    response.id,
                    egui::EventFilter {
                        tab: true,
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        escape: true,
                    },
                );
            });
        }
        if self.document.tool == Tool::Tag && !blocked {
            let hovering_tag = response.hover_pos().and_then(|point| self.position(point)).is_some_and(|position| {
                self.document
                    .with_state(|state| state.get_buffer().tags.iter().any(|tag| tag.contains(position)))
            });
            response = response.on_hover_cursor(if self.document.tag_drag_active() {
                egui::CursorIcon::Grabbing
            } else if hovering_tag {
                egui::CursorIcon::Grab
            } else {
                egui::CursorIcon::Crosshair
            });
        }
        if matches!(self.document.tool, Tool::Select | Tool::Click | Tool::Font) && !blocked {
            let modifiers = ui.input(|input| input.modifiers);
            let add_type = if self.document.tool == Tool::Select {
                selection_add_type(modifiers)
            } else {
                AddType::Default
            };
            // Modifiers start a new selection or move the layer instead of dragging a handle.
            let hover = response
                .hover_pos()
                .filter(|_| !(modifiers.shift || modifiers.ctrl || modifiers.mac_cmd))
                .and_then(|point| self.position(point));
            let cursor = match self.document.selection_handle(hover) {
                SelectionDrag::None | SelectionDrag::Create => None,
                SelectionDrag::Move => Some(egui::CursorIcon::Move),
                SelectionDrag::Left | SelectionDrag::Right => Some(egui::CursorIcon::ResizeHorizontal),
                SelectionDrag::Top | SelectionDrag::Bottom => Some(egui::CursorIcon::ResizeVertical),
                SelectionDrag::TopLeft | SelectionDrag::BottomRight => Some(egui::CursorIcon::ResizeNwSe),
                SelectionDrag::TopRight | SelectionDrag::BottomLeft => Some(egui::CursorIcon::ResizeNeSw),
            };
            if let Some(cursor) = cursor.filter(|_| response.hovered() || response.is_pointer_button_down_on()) {
                ui.ctx().set_cursor_icon(cursor);
            }
            if let Some(pointer) = response.hover_pos().filter(|_| add_type != AddType::Default) {
                let painter = ui
                    .ctx()
                    .layer_painter(egui::LayerId::new(egui::Order::Tooltip, response.id.with("selection-add-type")));
                let center = pointer + egui::vec2(14.0, 14.0);
                let stroke = egui::Stroke::new(1.5, Color32::WHITE);
                painter.circle_filled(center, 7.0, appearance::PRIMARY);
                painter.line_segment([center - egui::vec2(3.5, 0.0), center + egui::vec2(3.5, 0.0)], stroke);
                if add_type == AddType::Add {
                    painter.line_segment([center - egui::vec2(0.0, 3.5), center + egui::vec2(0.0, 3.5)], stroke);
                }
            }
        }
        if self.document.tool == Tool::Pipette && !blocked {
            let modifiers = ui.input(|input| input.modifiers);
            let hover = response
                .hover_pos()
                .and_then(|point| self.position(point))
                .map(|position| (position, modifiers));
            if self.pipette_hover != hover {
                self.pipette_hover = hover;
                ui.ctx().request_repaint();
            }
        } else {
            if self.pipette_hover.take().is_some() {
                ui.ctx().request_repaint();
            }
        }
        // Like Moebius, the canvas keeps the keyboard when panels, tools or colors are clicked;
        // only a focused text field takes it. It comes back a frame after the field lets go, so
        // the Enter that ends a field (e.g. chat) does not also reach the canvas.
        let field_focused = ui.ctx().wants_keyboard_input() && !response.has_focus();
        if field_focused {
            self.canvas_focus = false;
        } else if !self.field_focused_last_frame && !self.layer_properties_open() {
            self.canvas_focus = true;
        }
        self.field_focused_last_frame = field_focused;
        let info = self.view.terminal.render_info.read().clone();
        let (red, green, blue) = self.document.preview_color();
        let (columns, rows) = self.sub_cells();
        let cell_size = egui::vec2(
            info.font_width / columns as f32,
            info.font_height * if info.scan_lines { 2.0 } else { 1.0 } / rows as f32,
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
        if self.document.preview_cells.is_empty() {
            // Only a shape erasing with the right button has no cells: it shows where it erases.
            for point in &self.document.preview {
                let rect = egui::Rect::from_min_size(
                    origin + egui::vec2(point.x as f32 * cell_size.x, point.y as f32 * cell_size.y) * info.display_scale,
                    cell_size * info.display_scale,
                );
                painter.rect_filled(rect, 0, Color32::from_rgba_unmultiplied(red, green, blue, 160));
            }
        } else {
            // The operation tried on a copy: its cells as they will look, drawn over the picture
            // translucent, so what they cover stays visible.
            let character = egui::vec2(info.font_width, info.font_height * if info.scan_lines { 2.0 } else { 1.0 });
            let (fonts, palette) = self.document.with_state(|state| {
                let buffer = state.get_buffer();
                let fonts: HashMap<u8, icy_engine::BitFont> = buffer.font_iter().map(|(slot, font)| (*slot, font.clone())).collect();
                (fonts, buffer.palette.clone())
            });
            let rgb = |color: icy_engine::AttributeColor, index: u32| {
                let (red, green, blue) = color.as_rgb().unwrap_or_else(|| palette.rgb(index));
                Color32::from_rgb(red, green, blue).gamma_multiply(PREVIEW_OPACITY)
            };
            for (point, ch) in &self.document.preview_cells {
                let rect = egui::Rect::from_min_size(
                    origin + egui::vec2(point.x as f32 * character.x, point.y as f32 * character.y) * info.display_scale,
                    character * info.display_scale,
                );
                let attribute = ch.attribute;
                painter.rect_filled(rect, 0, rgb(attribute.background_color(), attribute.background()));
                if let Some(font) = fonts.get(&attribute.font_page()).or_else(|| fonts.get(&0)) {
                    widgets::paint_glyph_on(&painter, font, ch.ch, rect, rgb(attribute.foreground_color(), attribute.foreground()));
                }
            }
        }
        let tool = self.document.tool;
        let show_paint_hover =
            !blocked && (tool == Tool::Pencil || tool == Tool::Fill || tool.is_shape_tool()) && !(tool.is_shape_tool() && self.document.stroke_active());
        if show_paint_hover {
            if let Some(position) = response.hover_pos().and_then(|point| self.position(point)) {
                let brush_size = self.document.brush.brush_size.max(1) as i32;
                let half = brush_size / 2;
                let rect = egui::Rect::from_min_size(
                    origin + egui::vec2((position.x - half) as f32 * cell_size.x, (position.y - half) as f32 * cell_size.y) * info.display_scale,
                    cell_size * brush_size as f32 * info.display_scale,
                );
                painter.rect_stroke(rect, 0, egui::Stroke::new(2.0, Color32::WHITE), egui::StrokeKind::Inside);
            }
        }
        if self.document.tool == Tool::Tag {
            if let Some(selection) = self.document.tag_selection_rectangle() {
                let rect = egui::Rect::from_min_size(
                    origin + egui::vec2(selection.left() as f32 * cell_size.x, selection.top() as f32 * cell_size.y) * info.display_scale,
                    egui::vec2(selection.width() as f32 * cell_size.x, selection.height() as f32 * cell_size.y) * info.display_scale,
                );
                painter.rect_stroke(rect, 0, ui.visuals().selection.stroke, egui::StrokeKind::Inside);
            }
            let (tags, palette) = self
                .document
                .with_state(|state| (state.get_buffer().tags.clone(), state.get_buffer().palette.clone()));
            for (index, tag) in tags.iter().enumerate() {
                let position = self.document.tag_preview_position(index, tag.position);
                let rect = egui::Rect::from_min_size(
                    origin + egui::vec2(position.x as f32 * cell_size.x, position.y as f32 * cell_size.y) * info.display_scale,
                    egui::vec2(tag.length.max(1) as f32 * cell_size.x, cell_size.y) * info.display_scale,
                );
                let (red, green, blue) = tag
                    .attribute
                    .foreground_color()
                    .as_rgb()
                    .unwrap_or_else(|| palette.rgb(tag.attribute.foreground()));
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
                if self.document.tool == Tool::Pipette && self.atascii.is_some() {
                    self.pipette_atascii(position);
                } else if self.document.tool == Tool::Pipette && self.vt52.is_some() {
                    self.pipette_vt52(position);
                } else if self.document.tool == Tool::Pipette && self.petscii.is_some() {
                    self.pipette_petscii(position);
                } else if self.document.tool == Tool::Pipette {
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
                } else if pointer.button_pressed(egui::PointerButton::Secondary) && self.has_canvas_context_menu() {
                    // The right button opens the context menu; outside a selection it first moves the caret there.
                    if self.document.tool != Tool::Select {
                        self.document.finish();
                        self.document.with_state(|state| {
                            if !state.is_selected(position) {
                                state.set_caret_from_document_position(position);
                            }
                        });
                    }
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
        if self.document.tool == Tool::Tag && response.double_clicked() {
            self.document.finish();
            // A double click on a tag edits it; on empty canvas it adds one there.
            self.open_tag_properties(self.document.selected_tags.first().copied());
        }
        if let Some((position, width)) = self.document.new_tag_request.take() {
            self.open_new_tag(position, width);
        }
        if self.document.tool == Tool::Tag {
            response.context_menu(|ui| {
                if ui.button(fl!("tag-new-menu")).clicked() {
                    self.open_tag_properties(None);
                    ui.close();
                }
                ui.add_enabled_ui(!self.document.selected_tags.is_empty(), |ui| {
                    if ui.button(fl!("tag-properties-menu")).clicked() {
                        self.open_tag_properties(self.document.selected_tags.first().copied());
                        ui.close();
                    }
                    if ui.button(fl!("tag-duplicate")).clicked() {
                        let result = self.document.duplicate_selected_tags();
                        self.result(result.map(|_| ()));
                        ui.close();
                    }
                    if ui.button(fl!("tag-toolbar-delete")).clicked() {
                        let result = self.document.delete_selected_tags();
                        self.result(result);
                        ui.close();
                    }
                });
            });
        }
        if self.has_canvas_context_menu() {
            let context = ui.ctx().clone();
            response.context_menu(|ui| self.canvas_context_menu(ui, &context));
        }
    }

    fn copy(&mut self, context: &egui::Context) {
        if self.animation.is_some() {
            context.memory_mut(|memory| memory.request_focus(egui::Id::new("animation-source-editor")));
            context.input_mut(|input| input.events.push(egui::Event::Copy));
            return;
        }
        if self.document.tool == Tool::Tag {
            if let Some(text) = self.document.copy_selected_tags() {
                system_clipboard::copy_text_or_egui(context, text);
                return;
            }
        }
        let screen = self.document.screen.lock();
        if let Ok(data) = icy_engine_gui::prepare_clipboard_data(&**screen) {
            self.clipboard = data.icy_data.clone().map(|icy| (data.text.clone(), icy));
            system_clipboard::copy_data_or_text(context, &data);
        }
    }

    fn cut(&mut self, context: &egui::Context) {
        self.copy(context);
        if self.document.tool == Tool::Tag && !self.document.selected_tags.is_empty() {
            let result = self.document.delete_selected_tags();
            self.result(result);
            return;
        }
        self.edit(|state| state.erase_selection());
    }

    /// Pastes the system clipboard, falling back to asking egui for a text paste.
    fn paste_clipboard(&mut self, context: &egui::Context) {
        if let Some(content) = system_clipboard::read() {
            // Read directly: egui reports no paste for a clipboard with only an image.
            self.paste_content(content);
        } else {
            context.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
        }
    }

    /// Opens the new tag dialog for a tag dragged out along a row: `width` cells from `position`,
    /// shown as "TAG" padded to that width until a replacement is picked.
    fn open_new_tag(&mut self, position: Position, width: usize) {
        self.open_tag_properties(None);
        if let Some(Dialog::TagProperties(None, tag)) = &mut self.dialog {
            tag.position = position;
            tag.length = width;
            tag.preview = format!("{:<width$}", "TAG").chars().take(width).collect();
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
        self.tag_picker = None;
        self.canvas_focus = false;
    }

    /// Where the user's replacement lists live; tests run without one.
    fn taglists_dir(&self) -> Option<PathBuf> {
        self.persist_settings.then(Settings::taglists_dir).flatten()
    }

    fn select_taglist(&mut self, id: &str) {
        self.settings.selected_taglist = id.to_owned();
        if self.persist_settings {
            self.settings.store_persistent();
        }
    }

    fn tag_picker_action(&mut self, context: &egui::Context, action: tag_picker::Action, tag: &mut icy_engine::Tag) {
        let result = match action {
            tag_picker::Action::Pick(entry) => {
                tag.preview = tag_picker::preview(&entry);
                tag.replacement_value = entry.tag;
                self.tag_picker = None;
                Ok(())
            }
            tag_picker::Action::SelectList(id) => {
                self.select_taglist(&id);
                Ok(())
            }
            tag_picker::Action::Import => {
                self.choose(context, FileAction::ImportTaglist);
                Ok(())
            }
            tag_picker::Action::Create => self
                .taglists_dir()
                .ok_or_else(|| fl!("tag-replacements-no-folder"))
                .and_then(|dir| icy_draw::tag_replacements::create_taglist(&dir))
                .and_then(|(id, path)| {
                    self.show_taglist(&id);
                    open::that(&path).map_err(|error| error.to_string())
                }),
            tag_picker::Action::OpenFolder => self
                .taglists_dir()
                .ok_or_else(|| fl!("tag-replacements-no-folder"))
                .and_then(|dir| open::that(&dir).map_err(|error| error.to_string())),
            tag_picker::Action::Close => {
                self.tag_picker = None;
                Ok(())
            }
        };
        if let (Err(error), Some(picker)) = (result, &mut self.tag_picker) {
            picker.set_error(error);
        }
    }

    /// Reloads the replacement lists and shows `id`, which becomes the remembered list.
    fn show_taglist(&mut self, id: &str) {
        if let Some(picker) = &mut self.tag_picker {
            picker.reload(id);
        }
        self.select_taglist(id);
    }

    /// Copies a picked TOML file into the user's replacement lists and shows it.
    fn import_taglist(&mut self, path: &Path) {
        let result = self
            .taglists_dir()
            .ok_or_else(|| fl!("tag-replacements-no-folder"))
            .and_then(|dir| icy_draw::tag_replacements::import_taglist(path, &dir));
        match result {
            Ok(id) => self.show_taglist(&id),
            Err(error) => {
                let file = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
                let error = fl!("tag-replacements-import-failed", file = file, error = error);
                match &mut self.tag_picker {
                    Some(picker) => picker.set_error(error),
                    None => self.result(Err(error)),
                }
            }
        }
    }

    /// Pastes for an egui paste event, which egui only sends when the clipboard holds text.
    fn paste(&mut self, text: &str) {
        self.paste_content(system_clipboard::paste_event(text));
    }

    /// Pastes the richest clipboard content: characters with attributes, an image or text.
    pub(crate) fn paste_content(&mut self, content: PasteContent) {
        if let (Tool::Tag, PasteContent::Text(text)) = (self.document.tool, &content) {
            match self.document.paste_tags(text) {
                Ok(false) => {}
                result => return self.result(result.map(|_| ())),
            }
        }
        let result = match content {
            PasteContent::Icy { text, data } => self.document.start_paste(&text, Some(&data)),
            PasteContent::Image(image) => self.document.start_image_paste(&image),
            PasteContent::Text(text) => {
                // Without a system clipboard, text copied here still pastes with its attributes.
                let data = self.clipboard.as_ref().filter(|(copied, _)| *copied == text).map(|(_, data)| data.clone());
                self.document.start_paste(&text, data.as_deref())
            }
        };
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

    /// Escape first ends what is going on (pasting, a stroke, a selection); only then does it open
    /// the attribute picker, like in Moebius.
    fn escape_opens_attribute_picker(&self) -> bool {
        !self.document.paste_active()
            && !self.document.stroke_active()
            && self.document.selected_tags.is_empty()
            && !self.document.with_state(|state| state.is_something_selected())
    }

    /// Shows the attribute picker while it is open and applies what is picked at once.
    fn attribute_picker(&mut self, context: &egui::Context) {
        let Some(opened) = self.attribute_picker else {
            return;
        };
        let (palette, foreground, background, high_backgrounds) = self.document.with_state(|state| {
            let buffer = state.get_buffer();
            let attribute = state.get_caret().attribute;
            (
                buffer.palette.clone(),
                attribute.foreground(),
                attribute.background(),
                buffer.ice_mode.has_high_bg_colors(),
            )
        });
        let available = palette.len().max(1) as u32;
        let colors = super::attribute_picker::Colors {
            palette: &palette,
            foreground,
            background,
            foregrounds: available.min(16),
            backgrounds: available.min(if high_backgrounds { 16 } else { 8 }),
        };
        let center = if self.canvas_rect.is_positive() {
            self.canvas_rect.center()
        } else {
            context.content_rect().center()
        };
        let keys = context.cumulative_pass_nr() != opened;
        let (picks, open) = super::attribute_picker::show(context, center, &colors, keys);
        for pick in picks {
            let result = match pick {
                super::attribute_picker::Pick::Foreground(color) => self.document.set_caret_foreground(color),
                super::attribute_picker::Pick::Background(color) => self.document.set_caret_background(color),
            };
            self.result(result);
        }
        if !open {
            self.attribute_picker = None;
        }
    }

    fn keys(&mut self, context: &egui::Context) {
        if self.dialog.is_some() || self.picker || self.layer_properties_open() || self.attribute_picker.is_some() {
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
                    key: Key::Escape,
                    pressed: true,
                    modifiers,
                    ..
                } if self.canvas_focus && modifiers.is_none() && self.escape_opens_attribute_picker() => {
                    self.attribute_picker = Some(context.cumulative_pass_nr());
                    break;
                }
                egui::Event::Key {
                    key, pressed: true, modifiers, ..
                } if modifiers.alt && (self.canvas_focus || key == Key::Enter) && self.alt_key(context, key, modifiers) => {
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
                egui::Event::Cut if self.canvas_focus && self.document.can_paint() => self.cut(context),
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
                } if self.canvas_focus && !modifiers.any() && self.mode_key(key) => {
                    // The key's text must not be typed by the keyboard tool it may switch to.
                    alt_consumed = true;
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

    /// The mode keys of Moebius outside the text tools: K keyboard, B brush, I shifter (the
    /// shading brush) and P paint bucket.
    fn mode_key(&mut self, key: Key) -> bool {
        if matches!(self.document.tool, Tool::Click | Tool::Font) || self.document.paste_active() {
            return false;
        }
        match key {
            Key::K => self.select_tool(Tool::Click),
            Key::B => {
                if self.document.brush.primary == BrushPrimaryMode::Shading {
                    self.document.brush.primary = BrushPrimaryMode::HalfBlock;
                }
                self.select_tool(Tool::Pencil);
            }
            Key::I => {
                self.document.brush.primary = BrushPrimaryMode::Shading;
                self.select_tool(Tool::Pencil);
            }
            Key::P => self.select_tool(Tool::Fill),
            _ => return false,
        }
        true
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
                self.dialog = Some(Dialog::Error(fl!("error-export-font", error = error.to_string())));
                return;
            }
        };
        drop(library);
        let Some(path) = rfd::FileDialog::new()
            .set_title(fl!("tdf-font-selector-export_title"))
            .set_file_name(format!("{name}.{extension}"))
            .add_filter(fl!("file-dialog-filter-font-files"), &[extension])
            .save_file()
        else {
            return;
        };
        if let Err(error) = std::fs::write(path, bytes) {
            self.dialog = Some(Dialog::Error(fl!("error-export-font", error = error.to_string())));
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
                                .hint_text(fl!("tdf-font-selector-filter_placeholder"))
                                .desired_width(220.0),
                        );
                        ui.add(
                            appearance::text_edit(&mut self.text_font_preview_text)
                                .hint_text(fl!("font-preview-text-hint"))
                                .desired_width(180.0),
                        );
                        if ui
                            .add(egui::Button::selectable(self.text_font_favorites_only, format!("★ {}", fl!("font-favorites"))))
                            .clicked()
                        {
                            self.text_font_favorites_only = !self.text_font_favorites_only;
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.weak(fl!("tdf-font-selector-font_count", count = fonts.len()));
                        });
                    });
                    ui.horizontal(|ui| {
                        for (kind, _) in TextArtFontKind::ALL {
                            let enabled = &mut self.text_font_types[kind.index()];
                            if ui.add(egui::Button::selectable(*enabled, kind.label())).clicked() {
                                *enabled = !*enabled;
                            }
                        }
                        ui.separator();
                        ui.weak(fl!("font-size-width"));
                        ui.add(
                            egui::DragValue::new(&mut self.text_font_size_filter[0])
                                .range(0..=255)
                                .prefix(format!("{} ", fl!("font-filter-min"))),
                        );
                        ui.add(
                            egui::DragValue::new(&mut self.text_font_size_filter[1])
                                .range(0..=255)
                                .prefix(format!("{} ", fl!("font-filter-max"))),
                        );
                        ui.weak(fl!("font-size-height"));
                        ui.add(
                            egui::DragValue::new(&mut self.text_font_size_filter[2])
                                .range(0..=255)
                                .prefix(format!("{} ", fl!("font-filter-min"))),
                        );
                        ui.add(
                            egui::DragValue::new(&mut self.text_font_size_filter[3])
                                .range(0..=255)
                                .prefix(format!("{} ", fl!("font-filter-max"))),
                        );
                        ui.weak(fl!("font-filter-any"));
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
                            let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), row_height - 4.0), egui::Sense::click());
                            let fill = if selected {
                                ui.visuals().selection.bg_fill
                            } else if response.hovered() {
                                ui.visuals().widgets.hovered.weak_bg_fill
                            } else {
                                ui.visuals().faint_bg_color
                            };
                            ui.painter().rect_filled(rect, 4, fill);
                            ui.painter()
                                .rect_stroke(rect, 4, ui.visuals().widgets.noninteractive.bg_stroke, egui::StrokeKind::Inside);
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
                            let preview_header =
                                egui::Rect::from_min_max(egui::pos2(preview_left, rect.top() + 6.0), egui::pos2(rect.right() - 16.0, rect.top() + 26.0));
                            ui.painter().text(
                                preview_header.left_center(),
                                egui::Align2::LEFT_CENTER,
                                kind.label(),
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
                            let star_rect = egui::Rect::from_center_size(egui::pos2(preview_left - 16.0, rect.bottom() - 18.0), egui::Vec2::splat(26.0));
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
                    DialogButton::secondary(fl!("tdf-font-selector-export"), Action::Export).leading(),
                    DialogButton::cancel(labels::cancel(), Action::Cancel),
                    DialogButton::primary(labels::ok(), Action::Apply),
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
        if !matches!(dialog, Dialog::Export) {
            self.export_dialog = None;
        }
        match &dialog {
            Dialog::Shortcuts => keep = !self.shortcuts_dialog(context),
            Dialog::About => keep = self.about.as_mut().is_some_and(|about| about.show(context)),
            Dialog::ReferenceImage => keep = !self.reference_image_dialog(context),
            Dialog::Connect => keep = !self.connect_dialog(context),
            Dialog::Recovery => keep = self.recovery_dialog(context),
            Dialog::PetmateScreens(pick) => {
                let mut pick = pick.clone();
                keep = false;
                if self.petmate_dialog(context, &mut pick) {
                    self.dialog = Some(Dialog::PetmateScreens(pick));
                }
            }
            Dialog::ShadeRamps(draft) => {
                let mut draft = draft.clone();
                keep = self.shade_ramps_dialog(context, &mut draft);
                if keep {
                    self.dialog = Some(Dialog::ShadeRamps(draft));
                } else {
                    self.canvas_focus = true;
                }
            }
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
                        DialogButton::secondary(fl!("monitor-save-defaults"), Action::SaveDefaults)
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
                            appearance::group(ui, &fl!("script-group"), |ui| {
                                ui.add(
                                    egui::TextEdit::multiline(&mut self.script)
                                        .code_editor()
                                        .hint_text(fl!("script-hint"))
                                        .desired_width(f32::INFINITY)
                                        .desired_rows(if self.script_output.is_empty() { 16 } else { 10 }),
                                );
                            });
                            if !self.script_output.is_empty() {
                                appearance::group(ui, &fl!("script-output"), |ui| {
                                    ui.add(egui::Label::new(egui::RichText::new(&self.script_output).monospace()).wrap().selectable(true));
                                });
                            }
                        });
                        dialog.buttons([
                            DialogButton::cancel(labels::close(), Action::Close),
                            DialogButton::primary(fl!("script-run"), Action::Run).enabled(self.document.can_paint() && !self.picker),
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
                #[derive(Clone, Copy)]
                enum Action {
                    Add,
                    Close,
                }
                let response = appearance::Dialog::new("tags").size(DialogSize::Large).show(context, |dialog| {
                    dialog.content(|ui| {
                        let tags = self.document.with_state(|state| state.get_buffer().tags.clone());
                        appearance::group(ui, &format!("{} ({})", fl!("tag-list-title"), tags.len()), |ui| {
                            if tags.is_empty() {
                                ui.add_space(8.0);
                                ui.vertical_centered(|ui| {
                                    ui.weak(fl!("tag-list-no-tags"));
                                    ui.weak(fl!("tag-list-empty-hint"));
                                });
                                ui.add_space(8.0);
                                return;
                            }
                            egui::ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
                                for (index, tag) in tags.iter().enumerate() {
                                    ui.push_id(index, |ui| {
                                        ui.horizontal(|ui| {
                                            let selected = self.document.selected_tags.contains(&index);
                                            let preview =
                                                egui::RichText::new(if tag.preview.is_empty() { fl!("tag-empty") } else { tag.preview.clone() }).monospace();
                                            let response = ui
                                                .add_sized([180.0, 26.0], egui::Button::selectable(selected, preview))
                                                .on_hover_text(fl!("tag-double-click-edit"));
                                            if response.clicked() {
                                                self.document.selected_tags = vec![index];
                                                self.document.with_state(|state| state.set_current_tag(index));
                                            }
                                            if response.double_clicked() {
                                                edit = Some(index);
                                            }
                                            let role = match tag.tag_role {
                                                icy_engine::TagRole::Displaycode => fl!("edit-tag-role-displaycode"),
                                                icy_engine::TagRole::Hyperlink => fl!("edit-tag-role-hyperlink"),
                                            };
                                            if !tag.replacement_value.is_empty() {
                                                ui.label(egui::RichText::new(&tag.replacement_value).monospace());
                                            }
                                            ui.weak(format!("{}, {}  ·  {role}", tag.position.x, tag.position.y));
                                            if !tag.is_enabled {
                                                ui.weak(format!("· {}", fl!("tag-disabled")));
                                            }
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                if self.icons.button(ui, "delete", &fl!("delete_tag_tooltip"), false).clicked() {
                                                    delete = Some(index);
                                                }
                                                if self.icons.button(ui, "text", &fl!("tag-edit-menu"), false).clicked() {
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
                        DialogButton::secondary(fl!("tag-add-menu"), Action::Add).leading(),
                        DialogButton::primary(labels::close(), Action::Close).cancels(),
                    ]);
                });
                let add = matches!(response.action, Some(Action::Add));
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
                    Picker(fn() -> tag_picker::Action),
                }
                let mut tag = *draft.clone();
                let mut apply = false;
                let mut browse = false;
                let mut picked = None;
                let response = appearance::Dialog::new("tag-properties")
                    .size(DialogSize::Width(if self.tag_picker.is_some() { 640.0 } else { 440.0 }))
                    // Enter in the replacement filter picks the first match instead.
                    .confirm_on_enter(self.tag_picker.is_none())
                    .show(context, |dialog| {
                        dialog.content(|ui| {
                            // The replacement lists take the place of the form until one is picked.
                            if let Some(picker) = &mut self.tag_picker {
                                picked = picker.show(ui);
                                return;
                            }
                            appearance::group(ui, &if index.is_some() { fl!("edit-tag-title") } else { fl!("tag-new") }, |ui| {
                                appearance::check_row(ui, &fl!("tag-enabled"), &mut tag.is_enabled);
                                appearance::form_row(ui, &fl!("tag-edit-preview"), |ui| {
                                    ui.add(
                                        appearance::text_edit(&mut tag.preview)
                                            .hint_text(fl!("tag-preview-hint"))
                                            .desired_width(f32::INFINITY),
                                    );
                                });
                                appearance::form_row(ui, &fl!("tag-edit-replacement"), |ui| {
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui
                                            .add(egui::Button::new("…").min_size(egui::vec2(28.0, 0.0)))
                                            .on_hover_text(fl!("tag-replacement-browse"))
                                            .clicked()
                                        {
                                            browse = true;
                                        }
                                        ui.add(
                                            appearance::text_edit(&mut tag.replacement_value)
                                                .hint_text(fl!("tag-replacement-hint"))
                                                .desired_width(f32::INFINITY),
                                        );
                                    });
                                });
                                let role = |role: icy_engine::TagRole| match role {
                                    icy_engine::TagRole::Displaycode => fl!("edit-tag-role-displaycode"),
                                    icy_engine::TagRole::Hyperlink => fl!("edit-tag-role-hyperlink"),
                                };
                                appearance::combo_row(ui, &fl!("tag-role"), role(tag.tag_role), |ui| {
                                    for value in [icy_engine::TagRole::Displaycode, icy_engine::TagRole::Hyperlink] {
                                        ui.selectable_value(&mut tag.tag_role, value, role(value));
                                    }
                                });
                            });
                            appearance::group(ui, &fl!("tag-layout"), |ui| {
                                appearance::form_row(ui, &fl!("tag-column"), |ui| {
                                    ui.add(egui::DragValue::new(&mut tag.position.x).range(0..=10000));
                                });
                                appearance::form_row(ui, &fl!("tag-row"), |ui| {
                                    ui.add(egui::DragValue::new(&mut tag.position.y).range(0..=10000));
                                });
                                appearance::form_row(ui, &fl!("tag-length"), |ui| {
                                    ui.add(
                                        egui::DragValue::new(&mut tag.length)
                                            .range(1..=1000)
                                            .suffix(format!(" {}", fl!("unit-characters"))),
                                    );
                                });
                                let alignment = |alignment: std::fmt::Alignment| match alignment {
                                    std::fmt::Alignment::Left => fl!("edit-tag-alignment-left"),
                                    std::fmt::Alignment::Center => fl!("edit-tag-alignment-center"),
                                    std::fmt::Alignment::Right => fl!("edit-tag-alignment-right"),
                                };
                                appearance::combo_row(ui, &fl!("tag-alignment"), alignment(tag.alignment), |ui| {
                                    for value in [std::fmt::Alignment::Left, std::fmt::Alignment::Center, std::fmt::Alignment::Right] {
                                        ui.selectable_value(&mut tag.alignment, value, alignment(value));
                                    }
                                });
                                let placement = |placement: icy_engine::TagPlacement| match placement {
                                    icy_engine::TagPlacement::InText => fl!("tag-list-in-text"),
                                    icy_engine::TagPlacement::WithGotoXY => fl!("tag-list-with-gotoxy"),
                                };
                                appearance::combo_row(ui, &fl!("tag-list-placement"), placement(tag.tag_placement), |ui| {
                                    for value in [icy_engine::TagPlacement::InText, icy_engine::TagPlacement::WithGotoXY] {
                                        ui.selectable_value(&mut tag.tag_placement, value, placement(value));
                                    }
                                });
                            });
                        });
                        if let Some(picker) = &self.tag_picker {
                            let folder = picker.has_folder();
                            dialog.buttons([
                                DialogButton::secondary(fl!("tag-replacements-import"), Action::Picker(|| tag_picker::Action::Import))
                                    .leading()
                                    .enabled(folder),
                                DialogButton::secondary(fl!("tag-replacements-new"), Action::Picker(|| tag_picker::Action::Create))
                                    .leading()
                                    .enabled(folder)
                                    .tooltip(fl!("tag-replacements-new-tooltip")),
                                DialogButton::secondary(fl!("tag-replacements-open-folder"), Action::Picker(|| tag_picker::Action::OpenFolder))
                                    .leading()
                                    .enabled(folder)
                                    .tooltip(fl!("tag-replacements-custom-hint")),
                                DialogButton::cancel(fl!("tag-replacements-back"), Action::Picker(|| tag_picker::Action::Close)),
                            ]);
                        } else {
                            dialog.buttons([
                                DialogButton::cancel(labels::cancel(), Action::Cancel),
                                DialogButton::primary(if index.is_some() { fl!("button-apply") } else { fl!("add_tag_tooltip") }, Action::Apply),
                            ]);
                        }
                    });
                match response.action {
                    Some(Action::Apply) => apply = true,
                    Some(Action::Cancel) => keep = false,
                    Some(Action::Picker(action)) => picked = Some(action()),
                    None => keep &= !response.dismissed,
                }
                if browse {
                    self.tag_picker = Some(tag_picker::ReplacementPicker::new(&self.settings.selected_taglist, self.taglists_dir()));
                }
                if let Some(action) = picked {
                    self.tag_picker_action(context, action, &mut tag);
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
                    self.tag_picker = None;
                    self.canvas_focus = true;
                }
            }
            Dialog::FontSelect => match self.font_selector.show(context, self.picker) {
                Some(font_select::Action::Load) => self.choose(context, FileAction::LoadFont),
                Some(font_select::Action::Apply(font)) => {
                    self.apply_font(*font);
                    keep = false;
                }
                Some(font_select::Action::Cancel) => keep = false,
                None => {}
            },
            Dialog::TextArtFontSelect => {
                keep = self.text_art_font_dialog(context);
            }
            Dialog::AnimationOverwrite(path, format) => {
                #[derive(Clone, Copy)]
                enum Action {
                    Cancel,
                    Overwrite,
                }
                let response = MessageBox::new(
                    "animation-overwrite",
                    MessageKind::Warning,
                    fl!("replace-export-file"),
                    path.display().to_string(),
                )
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
            Dialog::Palette => match self.palette_editor.show(context, self.picker) {
                Some(super::palette::Action::Import) => self.choose(context, FileAction::ImportPalette),
                Some(super::palette::Action::Export) => self.choose(context, FileAction::ExportPalette),
                Some(super::palette::Action::Apply(palette)) => {
                    self.edit(|state| state.switch_to_palette(palette));
                    keep = false;
                }
                Some(super::palette::Action::Cancel) => keep = false,
                None => {}
            },
            Dialog::Sauce(draft) => {
                let mut draft = draft.clone();
                keep = false;
                if self.sauce_dialog(context, &mut draft) {
                    self.dialog = Some(Dialog::Sauce(draft));
                }
            }
            Dialog::Export => {
                let mut dialog = self.export_dialog.take().unwrap_or_else(|| self.export_dialog());
                match dialog.show(context) {
                    Some(ExportAction::Export(request)) => match self.export(&request) {
                        Ok(()) => {
                            self.settings.export_settings = dialog.settings();
                            self.settings.last_export_directory = Some(PathBuf::from(dialog.directory()));
                            if self.persist_settings {
                                self.settings.store_persistent();
                            }
                            keep = false;
                        }
                        Err(error) => {
                            dialog.set_error(error);
                            self.export_dialog = Some(dialog);
                        }
                    },
                    Some(ExportAction::Cancel) => keep = false,
                    None => self.export_dialog = Some(dialog),
                }
            }
            Dialog::Overwrite(path) => {
                #[derive(Clone, Copy)]
                enum Action {
                    Cancel,
                    Overwrite,
                }
                let response = MessageBox::new("overwrite", MessageKind::Warning, fl!("replace-file"), path.display().to_string())
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
            Dialog::Settings(draft) => {
                let mut draft = draft.clone();
                keep = false;
                if self.settings_dialog(context, &mut draft) {
                    self.dialog = Some(Dialog::Settings(draft));
                }
            }
            Dialog::FileSettings(draft) => {
                let mut draft = draft.clone();
                keep = false;
                if self.file_settings_dialog(context, &mut draft) {
                    self.dialog = Some(Dialog::FileSettings(draft));
                }
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
                let title = target.map_or_else(|| fl!("characters-title"), |(_, slot)| fl!("characters-assign-fkey", key = (slot + 1)));
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
                            DialogButton::primary(fl!("select-font-dialog-select"), Action::Select),
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
                    .size(if resize { DialogSize::Medium } else { DialogSize::Width(780.0) })
                    .confirm_on_enter(true)
                    .show(context, |dialog| {
                        dialog.content(|ui| {
                            if resize {
                                self.new_size_group(ui, true, current);
                                return;
                            }
                            let kinds = |this: &mut Self, ui: &mut egui::Ui| {
                                appearance::group(ui, &fl!("new-file-type"), |ui| {
                                    ui.spacing_mut().item_spacing.y = 2.0;
                                    for kind in NewKind::ALL {
                                        let response = welcome::kind_row(&mut this.icons, ui, kind, this.new_kind == kind);
                                        if response.clicked() {
                                            this.new_kind = kind;
                                        }
                                    }
                                });
                            };
                            if ui.available_width() >= 640.0 {
                                ui.columns(2, |columns| {
                                    kinds(self, &mut columns[0]);
                                    if self.new_kind.has_size() {
                                        self.new_size_group(&mut columns[1], false, current);
                                    } else if self.new_kind == NewKind::Igs {
                                        self.new_igs_group(&mut columns[1]);
                                    } else if self.new_kind == NewKind::Atascii {
                                        self.new_atascii_group(&mut columns[1]);
                                    } else if self.new_kind == NewKind::Vt52 {
                                        self.new_vt52_group(&mut columns[1]);
                                    } else if self.new_kind == NewKind::Petscii {
                                        self.new_petscii_group(&mut columns[1]);
                                    }
                                });
                            } else {
                                kinds(self, ui);
                                if self.new_kind.has_size() {
                                    self.new_size_group(ui, false, current);
                                } else if self.new_kind == NewKind::Igs {
                                    self.new_igs_group(ui);
                                } else if self.new_kind == NewKind::Atascii {
                                    self.new_atascii_group(ui);
                                } else if self.new_kind == NewKind::Vt52 {
                                    self.new_vt52_group(ui);
                                } else if self.new_kind == NewKind::Petscii {
                                    self.new_petscii_group(ui);
                                }
                            }
                        });
                        dialog.buttons([
                            DialogButton::cancel(labels::cancel(), Action::Cancel),
                            DialogButton::primary(if resize { fl!("edit-canvas-size-resize") } else { fl!("new-file-create") }, Action::Confirm),
                        ]);
                    });
                match response.action {
                    Some(Action::Confirm) => {
                        let mut size = Size::new(self.new_size[0], self.new_size[1]);
                        if resize {
                            if let Some(width) = self.document.profile().fixed_width() {
                                size.width = width;
                            }
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
                let response = MessageBox::new(
                    "close",
                    MessageKind::Question,
                    fl!("save-changes-title", filename = self.document_name()),
                    fl!("save-changes-description"),
                )
                .buttons([
                    DialogButton::destructive(fl!("ask_close_file_dialog-dont_save_button"), Action::Discard).leading(),
                    DialogButton::cancel(labels::cancel(), Action::Cancel),
                    DialogButton::primary(fl!("ask_close_file_dialog-save_button"), Action::Save),
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
        if self.dialog.is_none() && !self.picker {
            if let Some(error) = self.autosave_error.take() {
                self.dialog = Some(Dialog::Error(fl!("recovery-failed", error = error)));
            } else if !self.offers.is_empty() {
                self.dialog = Some(Dialog::Recovery);
            }
        }
        self.show_ui(context);
        self.autosave(context);
    }

    /// The panels of the ANSI editor around the canvas.
    fn ansi_panels(&mut self, context: &egui::Context, blocked: bool) {
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
    }

    fn show_ui(&mut self, context: &egui::Context) {
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
                    FileAction::SaveFont => {
                        if path.extension().is_none() {
                            path.set_extension("psf");
                        }
                        if path.exists() {
                            self.dialog = Some(Dialog::Overwrite(path));
                        } else {
                            self.save_path(context, path, false);
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
                    FileAction::SaveRip => {
                        if path.extension().is_none() {
                            path.set_extension("rip");
                        }
                        if path.exists() {
                            self.dialog = Some(Dialog::Overwrite(path));
                        } else {
                            self.save_path(context, path, false);
                        }
                    }
                    FileAction::SaveIgs => {
                        if path.extension().is_none() {
                            path.set_extension("ig");
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
                        if let Some(editor) = &mut self.animation {
                            editor.set_export_path(path);
                        }
                    }
                    FileAction::InsertImage => self.insert_image(&path),
                    FileAction::ReferenceImage => {
                        self.reference_path = path.display().to_string();
                        self.reference_draft.path = path;
                        self.reference_draft.visible = true;
                    }
                    FileAction::ImportPalette => self.palette_editor.import(&path),
                    FileAction::LoadAtasciiFont => self.load_atascii_font(&path),
                    FileAction::LoadFont => {
                        if let Some(font) = self.font_selector.load(&path) {
                            self.apply_font(font);
                        }
                    }
                    FileAction::ExportPalette => {
                        if path.extension().is_none() {
                            path.set_extension("gpl");
                        }
                        self.palette_editor.export(&path);
                    }
                    FileAction::ImportTaglist => self.import_taglist(&path),
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
        if let Some(editor) = self.font_editor.as_mut().filter(|editor| editor.apply_target && editor.modified()) {
            if close_requested && !self.picker && !self.allow_close {
                context.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                editor.confirm_close = true;
            }
        }
        let closing = close_requested
            && !self.picker
            && (self.allow_close || (!self.modified() && !self.font_editor.as_ref().is_some_and(|editor| editor.apply_target && editor.modified())));
        if closing {
            // Waits for pending removals, since the process ends with the window.
            self.finish_recovery();
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
        let title = if self.show_start {
            "Icy Draw".to_owned()
        } else if self.collab.active {
            format!("{} — Icy Draw", self.collab.server)
        } else {
            format!("{}{} — Icy Draw", if self.modified() { "*" } else { "" }, self.document_name())
        };
        context.send_viewport_cmd(egui::ViewportCommand::Title(title));
        if blocked {
            self.document.finish();
        }
        self.menu(context);
        if let Some(editor) = &mut self.font_editor {
            let layout = super::font::Layout {
                toolbar_height: chrome::TOOLBAR_HEIGHT,
                status_height: chrome::STATUS_HEIGHT,
                rail_width: chrome::SIDEBAR_WIDTH,
            };
            let action = editor.show(context, blocked, layout);
            self.canvas_focus = false;
            match action {
                Some(super::font::Action::Apply(font)) => match self.document.with_state(|state| state.set_font(*font)) {
                    Ok(()) => self.close_font_editor(),
                    Err(error) => self.dialog = Some(Dialog::Error(error.to_string())),
                },
                Some(super::font::Action::Close) => self.close_font_editor(),
                None => {}
            }
            if !blocked {
                self.keys(context);
            }
            self.dialogs(context);
            return;
        }
        if let Some(editor) = &mut self.rip {
            editor.show(context, blocked);
            self.canvas_focus = false;
            if !blocked {
                if context.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, Key::Z)) {
                    editor.undo(false);
                }
                if context.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT), Key::Z))
                    || context.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, Key::Y))
                {
                    editor.undo(true);
                }
                self.keys(context);
            }
            self.dialogs(context);
            return;
        }
        if let Some(editor) = &mut self.igs {
            editor.show(context, blocked);
            self.canvas_focus = false;
            if !blocked {
                if context.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, Key::Z)) {
                    editor.undo(false);
                }
                if context.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT), Key::Z))
                    || context.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, Key::Y))
                {
                    editor.undo(true);
                }
                self.keys(context);
            }
            self.dialogs(context);
            return;
        }
        if let Some(editor) = &mut self.animation {
            editor.show(context, blocked || self.dialog.is_some() || self.picker);
            match editor.take_request() {
                Some(super::animation::Request::Browse(format)) => self.choose(context, FileAction::ExportAnimation(format)),
                Some(super::animation::Request::Export(path, format)) if path.exists() => {
                    self.dialog = Some(Dialog::AnimationOverwrite(path, format));
                }
                Some(super::animation::Request::Export(path, format)) => editor.export(path, format, context.clone()),
                None => {}
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
        if self.atascii.is_some() {
            self.atascii_panels(context, blocked);
        } else if self.vt52.is_some() {
            self.vt52_panels(context, blocked);
        } else if self.petscii.is_some() {
            self.petscii_panels(context, blocked);
        } else {
            self.ansi_panels(context, blocked);
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
        if let Some(font) = &self.charfont {
            font.sync_display_color(&self.document);
        }
        egui::CentralPanel::default().frame(egui::Frame::new().fill(well)).show(context, |ui| {
            let blocked = blocked || self.dialog.is_some() || self.picker || self.layer_properties_open();
            if !self.document.outline_font {
                self.canvas(ui, blocked);
                return;
            }
            // Outline fonts show the styled result beside (or below, when narrow) the editor.
            let full = ui.available_rect_before_wrap();
            let side_by_side = full.width() >= full.height() || full.width() >= 700.0;
            let (editor, preview) = if side_by_side {
                let middle = full.center().x.round();
                (
                    egui::Rect::from_min_max(full.min, egui::pos2(middle - 1.0, full.bottom())),
                    egui::Rect::from_min_max(egui::pos2(middle + 1.0, full.top()), full.max),
                )
            } else {
                let middle = full.center().y.round();
                (
                    egui::Rect::from_min_max(full.min, egui::pos2(full.right(), middle - 1.0)),
                    egui::Rect::from_min_max(egui::pos2(full.left(), middle + 1.0), full.max),
                )
            };
            ui.scope_builder(egui::UiBuilder::new().id_salt("outline-editor").max_rect(editor), |ui| self.canvas(ui, blocked));
            let stroke = ui.visuals().widgets.noninteractive.bg_stroke;
            if side_by_side {
                ui.painter().vline(full.center().x.round(), full.y_range(), stroke);
            } else {
                ui.painter().hline(full.x_range(), full.center().y.round(), stroke);
            }
            ui.scope_builder(egui::UiBuilder::new().id_salt("outline-preview").max_rect(preview), |ui| {
                self.outline_preview_pane(ui)
            });
            ui.advance_cursor_after_rect(full);
        });
        if !blocked && !self.layer_properties_open() {
            if self.atascii.is_some() {
                self.atascii_keys(context);
                self.keys(context);
            } else if self.vt52.is_some() {
                self.vt52_keys(context);
                self.keys(context);
            } else if self.petscii.is_some() {
                self.petscii_keys(context);
                self.keys(context);
            } else {
                self.keys(context);
                self.attribute_picker(context);
            }
        }
        self.sync_collaboration();
        self.font_slots_window(context, blocked || self.dialog.is_some());
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
