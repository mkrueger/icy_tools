//! Menu bar with shortcut hints and enabled states, the keyboard reference and the about box.

use super::{Dialog, DrawApp, FileAction};
use eframe::egui::{self, Key, KeyboardShortcut, Modifiers};
use icy_engine::TextPane;
use icy_engine_edit::UndoState;
use icy_engine_gui::{
    egui::{
        appearance::{self, labels, DialogButton, DialogSize},
        shortcuts::keycaps,
    },
    ScalingMode,
};

const COMMAND_SHIFT: Modifiers = Modifiers::COMMAND.plus(Modifiers::SHIFT);
pub(super) const NEW: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::N);
pub(super) const OPEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
pub(super) const SAVE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
pub(super) const SAVE_AS: KeyboardShortcut = KeyboardShortcut::new(COMMAND_SHIFT, Key::S);
pub(super) const EXPORT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::E);
pub(super) const NEW_WINDOW: KeyboardShortcut = KeyboardShortcut::new(COMMAND_SHIFT, Key::N);
pub(super) const QUIT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Q);
pub(super) const UNDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
pub(super) const REDO: KeyboardShortcut = KeyboardShortcut::new(COMMAND_SHIFT, Key::Z);
pub(super) const CUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::X);
pub(super) const COPY: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::C);
pub(super) const PASTE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::V);
pub(super) const SELECT_ALL: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::A);
pub(super) const DESELECT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::D);
pub(super) const INVERT_SELECTION: KeyboardShortcut = KeyboardShortcut::new(COMMAND_SHIFT, Key::I);
pub(super) const ZOOM_IN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Plus);
pub(super) const ZOOM_OUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Minus);
pub(super) const ZOOM_FIT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Num0);
pub(super) const ZOOM_ACTUAL: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Num1);
pub(super) const GRID: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::G);
pub(super) const PANELS: KeyboardShortcut = KeyboardShortcut::new(COMMAND_SHIFT, Key::P);
pub(super) const LINE_NUMBERS: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::R);
pub(super) const TOGGLE_GUIDE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Semicolon);
pub(super) const REFERENCE_IMAGE: KeyboardShortcut = KeyboardShortcut::new(COMMAND_SHIFT, Key::O);
pub(super) const TOGGLE_REFERENCE_IMAGE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Tab);
#[cfg(target_os = "macos")]
pub(super) const FULLSCREEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::MAC_CMD.plus(Modifiers::CTRL), Key::F);
#[cfg(not(target_os = "macos"))]
pub(super) const FULLSCREEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::ALT, Key::Enter);
pub(super) const NEXT_FG: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::ArrowDown);
pub(super) const PREV_FG: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::ArrowUp);
pub(super) const NEXT_BG: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::ArrowRight);
pub(super) const PREV_BG: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::ArrowLeft);
pub(super) const PICK_ATTRIBUTE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::ALT, Key::U);
pub(super) const SWAP_COLORS: KeyboardShortcut = KeyboardShortcut::new(Modifiers::ALT, Key::X);

const COMMAND_ALT: Modifiers = Modifiers::COMMAND.plus(Modifiers::ALT);

/// Line, row, column and area editing commands from the original "Area Operations" menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AreaOp {
    JustifyLineLeft,
    JustifyLineCenter,
    JustifyLineRight,
    InsertRow,
    DeleteRow,
    InsertColumn,
    DeleteColumn,
    EraseRow,
    EraseRowToStart,
    EraseRowToEnd,
    EraseColumn,
    EraseColumnToStart,
    EraseColumnToEnd,
    ScrollUp,
    ScrollDown,
    ScrollLeft,
    ScrollRight,
}

/// Menu layout of the area operations; `None` inserts a separator.
const AREA_MENU: [Option<AreaOp>; 21] = [
    Some(AreaOp::JustifyLineLeft),
    Some(AreaOp::JustifyLineCenter),
    Some(AreaOp::JustifyLineRight),
    None,
    Some(AreaOp::InsertRow),
    Some(AreaOp::DeleteRow),
    Some(AreaOp::InsertColumn),
    Some(AreaOp::DeleteColumn),
    None,
    Some(AreaOp::EraseRow),
    Some(AreaOp::EraseRowToStart),
    Some(AreaOp::EraseRowToEnd),
    None,
    Some(AreaOp::EraseColumn),
    Some(AreaOp::EraseColumnToStart),
    Some(AreaOp::EraseColumnToEnd),
    None,
    Some(AreaOp::ScrollUp),
    Some(AreaOp::ScrollDown),
    Some(AreaOp::ScrollLeft),
    Some(AreaOp::ScrollRight),
];

impl AreaOp {
    pub(super) const ALL: [AreaOp; 17] = [
        AreaOp::JustifyLineLeft,
        AreaOp::JustifyLineCenter,
        AreaOp::JustifyLineRight,
        AreaOp::InsertRow,
        AreaOp::DeleteRow,
        AreaOp::InsertColumn,
        AreaOp::DeleteColumn,
        AreaOp::EraseRow,
        AreaOp::EraseRowToStart,
        AreaOp::EraseRowToEnd,
        AreaOp::EraseColumn,
        AreaOp::EraseColumnToStart,
        AreaOp::EraseColumnToEnd,
        AreaOp::ScrollUp,
        AreaOp::ScrollDown,
        AreaOp::ScrollLeft,
        AreaOp::ScrollRight,
    ];

    pub(super) fn label(self) -> &'static str {
        match self {
            AreaOp::JustifyLineLeft => "Left Justify Line",
            AreaOp::JustifyLineCenter => "Center Line",
            AreaOp::JustifyLineRight => "Right Justify Line",
            AreaOp::InsertRow => "Insert Row",
            AreaOp::DeleteRow => "Delete Row",
            AreaOp::InsertColumn => "Insert Column",
            AreaOp::DeleteColumn => "Delete Column",
            AreaOp::EraseRow => "Erase Row",
            AreaOp::EraseRowToStart => "Erase Row to Start",
            AreaOp::EraseRowToEnd => "Erase Row to End",
            AreaOp::EraseColumn => "Erase Column",
            AreaOp::EraseColumnToStart => "Erase Column to Start",
            AreaOp::EraseColumnToEnd => "Erase Column to End",
            AreaOp::ScrollUp => "Scroll Area Up",
            AreaOp::ScrollDown => "Scroll Area Down",
            AreaOp::ScrollLeft => "Scroll Area Left",
            AreaOp::ScrollRight => "Scroll Area Right",
        }
    }

    pub(super) fn shortcut(self) -> Option<KeyboardShortcut> {
        let alt = |key| Some(KeyboardShortcut::new(Modifiers::ALT, key));
        let command_alt = |key| Some(KeyboardShortcut::new(COMMAND_ALT, key));
        match self {
            AreaOp::JustifyLineLeft => alt(Key::L),
            AreaOp::JustifyLineCenter => alt(Key::C),
            AreaOp::JustifyLineRight => alt(Key::R),
            AreaOp::InsertRow => alt(Key::ArrowUp),
            AreaOp::DeleteRow => alt(Key::ArrowDown),
            AreaOp::InsertColumn => alt(Key::ArrowRight),
            AreaOp::DeleteColumn => alt(Key::ArrowLeft),
            AreaOp::EraseRow => alt(Key::E),
            AreaOp::EraseRowToStart => alt(Key::Home),
            AreaOp::EraseRowToEnd => alt(Key::End),
            AreaOp::EraseColumn | AreaOp::EraseColumnToStart | AreaOp::EraseColumnToEnd => None,
            AreaOp::ScrollUp => command_alt(Key::ArrowUp),
            AreaOp::ScrollDown => command_alt(Key::ArrowDown),
            AreaOp::ScrollLeft => command_alt(Key::ArrowLeft),
            AreaOp::ScrollRight => command_alt(Key::ArrowRight),
        }
    }

    pub(super) fn apply(self, state: &mut icy_engine_edit::EditState) -> icy_engine::Result<()> {
        match self {
            AreaOp::JustifyLineLeft => state.justify_line_left(),
            AreaOp::JustifyLineCenter => state.center_line(),
            AreaOp::JustifyLineRight => state.justify_line_right(),
            AreaOp::InsertRow => state.insert_row(),
            AreaOp::DeleteRow => state.delete_row(),
            AreaOp::InsertColumn => state.insert_column(),
            AreaOp::DeleteColumn => state.delete_column(),
            AreaOp::EraseRow => state.erase_row(),
            AreaOp::EraseRowToStart => state.erase_row_to_start(),
            AreaOp::EraseRowToEnd => state.erase_row_to_end(),
            AreaOp::EraseColumn => state.erase_column(),
            AreaOp::EraseColumnToStart => state.erase_column_to_start(),
            AreaOp::EraseColumnToEnd => state.erase_column_to_end(),
            AreaOp::ScrollUp => state.scroll_area_up(),
            AreaOp::ScrollDown => state.scroll_area_down(),
            AreaOp::ScrollLeft => state.scroll_area_left(),
            AreaOp::ScrollRight => state.scroll_area_right(),
        }
    }
}

/// Caret color commands from the original "Colors" menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ColorOp {
    NextForeground,
    PreviousForeground,
    NextBackground,
    PreviousBackground,
    PickAttributeUnderCaret,
    Swap,
    Default,
}

/// Guide presets of the original editor: (label, columns, rows).
const GUIDES: [(&str, i32, i32); 4] = [
    ("Smallscale 80×25", 80, 25),
    ("Square 80×40", 80, 40),
    ("Instagram 80×50", 80, 50),
    ("File_ID.DIZ 44×22", 44, 22),
];

/// Raster presets of the original editor in characters.
const RASTERS: [(i32, i32); 10] = [(1, 1), (2, 2), (4, 2), (4, 4), (8, 2), (8, 4), (8, 8), (16, 4), (16, 8), (16, 16)];

/// Fixed zoom presets of the original editor.
const ZOOM_PRESETS: [(&str, f32); 5] = [
    ("4:1  400%", 4.0),
    ("2:1  200%", 2.0),
    ("1:1  100%", 1.0),
    ("1:2  50%", 0.5),
    ("1:4  25%", 0.25),
];

/// Manual zoom levels used by the zoom commands, in canvas pixels per font pixel.
pub(super) const ZOOM_STEPS: [f32; 10] = [0.5, 0.75, 1.0, 1.5, 2.0, 3.0, 4.0, 5.0, 6.0, 8.0];

/// Menu entry with a right-aligned shortcut; closes the menu when clicked.
fn item(ui: &mut egui::Ui, label: &str, shortcut: Option<&KeyboardShortcut>, enabled: bool) -> bool {
    let mut button = egui::Button::new(label);
    if let Some(shortcut) = shortcut {
        button = button.shortcut_text(egui::RichText::new(ui.ctx().format_shortcut(shortcut)).weak());
    }
    let clicked = ui.add_enabled(enabled, button).clicked();
    if clicked {
        ui.close();
    }
    clicked
}

/// Check mark menu entry with an optional shortcut; returns true when toggled.
fn check_item(ui: &mut egui::Ui, label: &str, shortcut: Option<&KeyboardShortcut>, value: &mut bool) -> bool {
    let mut button = egui::Button::selectable(*value, label);
    if let Some(shortcut) = shortcut {
        button = button.shortcut_text(egui::RichText::new(ui.ctx().format_shortcut(shortcut)).weak());
    }
    let clicked = ui.add(button).clicked();
    if clicked {
        *value = !*value;
        ui.close();
    }
    clicked
}

/// Short, human readable label of a scaling mode.
pub(super) fn zoom_label(mode: ScalingMode) -> String {
    match mode {
        ScalingMode::Auto => "Fit to Window".into(),
        ScalingMode::FitWidth => "Fit Width".into(),
        ScalingMode::Manual(zoom) => format!("{:.0}%", zoom * 100.0),
    }
}

impl DrawApp {
    pub(super) fn menu(&mut self, context: &egui::Context) {
        let blocked = self.dialog.is_some() || self.picker || self.layer_properties_open() || self.document.paste_active();
        egui::TopBottomPanel::top("menu").show(context, |ui| {
            if blocked {
                ui.disable();
            }
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| self.file_menu(ui, context));
                if !self.show_start {
                    ui.menu_button("Edit", |ui| self.edit_menu(ui, context));
                }
                if self.animation.is_none() && !self.show_start {
                    ui.menu_button("Selection", |ui| self.selection_menu(ui));
                    ui.menu_button("Colors", |ui| self.colors_menu(ui));
                    ui.menu_button("Document", |ui| self.document_menu(ui));
                    ui.menu_button("View", |ui| self.view_menu(ui, context));
                    ui.menu_button("Extensions", |ui| self.extensions_menu(ui));
                }
                ui.menu_button("Help", |ui| {
                    if item(ui, "Discuss on GitHub", None, true) {
                        self.open_url("https://github.com/mkrueger/icy_tools/discussions");
                    }
                    if item(ui, "Report a Bug", None, true) {
                        self.open_url("https://github.com/mkrueger/icy_tools/issues");
                    }
                    if item(ui, "Open Log File", None, icy_draw::Settings::log_file().is_some()) {
                        self.open_log_file();
                    }
                    ui.separator();
                    if item(ui, "Keyboard Shortcuts…", None, true) {
                        self.dialog = Some(Dialog::Shortcuts);
                    }
                    ui.separator();
                    ui.hyperlink_to("Icy Draw on GitHub", "https://github.com/mkrueger/icy_tools");
                    if item(ui, "About Icy Draw…", None, true) {
                        self.dialog = Some(Dialog::About);
                    }
                });
            });
        });
    }

    fn file_menu(&mut self, ui: &mut egui::Ui, context: &egui::Context) {
        if item(ui, "New…", Some(&NEW), true) {
            self.request_new();
        }
        if item(ui, "Open…", Some(&OPEN), true) {
            self.choose(context, FileAction::Open);
        }
        let recent = self.settings.recent_files.files();
        ui.add_enabled_ui(!recent.is_empty(), |ui| {
            ui.menu_button("Open Recent", |ui| {
                for path in recent.iter().rev() {
                    let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
                    if ui.button(name).on_hover_text(path.display().to_string()).clicked() {
                        self.open(path.clone());
                        ui.close();
                    }
                }
                ui.separator();
                if item(ui, "Clear Recent Files", None, true) {
                    self.settings.recent_files.clear_recent_files();
                }
            });
        });
        ui.separator();
        if item(ui, "Save", Some(&SAVE), true) {
            self.save(context, false);
        }
        if item(ui, "Save As…", Some(&SAVE_AS), true) {
            self.save(context, true);
        }
        if self.animation.is_none() && item(ui, "Export…", Some(&EXPORT), true) {
            self.dialog = Some(Dialog::Export);
        }
        ui.separator();
        if item(ui, "New Window", Some(&NEW_WINDOW), true) {
            self.new_window();
        }
        if item(ui, "Quit", Some(&QUIT), true) {
            context.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn edit_menu(&mut self, ui: &mut egui::Ui, context: &egui::Context) {
        let animation = self.animation.is_some();
        let (undo, redo, selected) = self.document.with_state(|state| {
            let describe = |description: Option<String>| description.filter(|text| !text.is_empty());
            (
                state.can_undo().then(|| describe(state.undo_description())),
                state.can_redo().then(|| describe(state.redo_description())),
                state.is_something_selected(),
            )
        });
        let font_undo = self.charfont.as_ref().is_some_and(|font| font.state.can_undo());
        let font_redo = self.charfont.as_ref().is_some_and(|font| font.state.can_redo());
        let label = |verb: &str, description: &Option<Option<String>>| match description {
            Some(Some(text)) => format!("{verb} {text}"),
            _ => verb.to_owned(),
        };
        if item(ui, &label("Undo", &undo), Some(&UNDO), animation || undo.is_some() || font_undo) {
            self.undo(false);
        }
        if item(ui, &label("Redo", &redo), Some(&REDO), animation || redo.is_some() || font_redo) {
            self.undo(true);
        }
        ui.separator();
        let paint = self.document.can_paint();
        if item(ui, "Cut", Some(&CUT), animation || (selected && paint)) {
            if animation {
                context.memory_mut(|memory| memory.request_focus(egui::Id::new("animation-source-editor")));
                context.input_mut(|input| input.events.push(egui::Event::Cut));
            } else {
                self.copy(context);
                self.edit(|state| state.erase_selection());
            }
        }
        if item(ui, "Copy", Some(&COPY), animation || selected) {
            self.copy(context);
        }
        if item(ui, "Paste", Some(&PASTE), animation || paint) {
            if animation {
                context.memory_mut(|memory| memory.request_focus(egui::Id::new("animation-source-editor")));
            }
            context.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
        }
        if animation {
            ui.separator();
            if item(ui, "Select All", Some(&SELECT_ALL), true) {
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
            }
            return;
        }
        if item(ui, "Insert Image from File…", None, paint && !self.document.paste_active()) {
            self.choose(context, FileAction::InsertImage);
        }
        ui.separator();
        ui.add_enabled_ui(paint, |ui| {
            ui.menu_button("Area Operations", |ui| {
                for entry in AREA_MENU {
                    match entry {
                        Some(operation) => {
                            if item(ui, operation.label(), operation.shortcut().as_ref(), true) {
                                self.area_operation(operation);
                            }
                        }
                        None => {
                            ui.separator();
                        }
                    }
                }
            });
        });
        ui.separator();
        if item(ui, "Select Font…", None, true) {
            self.dialog = Some(Dialog::FontSelect);
        }
        ui.separator();
        let mut mirror = self.document.with_state(|state| state.get_mirror_mode());
        if check_item(ui, "Mirror Mode", None, &mut mirror) {
            self.document.with_state(|state| state.set_mirror_mode(mirror));
        }
        ui.separator();
        if item(ui, "Document Settings…", None, true) {
            self.dialog = Some(Dialog::Inspector);
        }
    }

    fn selection_menu(&mut self, ui: &mut egui::Ui) {
        let selected = self.document.with_state(|state| state.is_something_selected());
        let paint = self.document.can_paint();
        if item(ui, "Select All", Some(&SELECT_ALL), true) {
            self.select_all();
        }
        if item(ui, "Deselect", Some(&DESELECT), selected) {
            self.edit(|state| state.clear_selection());
        }
        if item(ui, "Invert Selection", Some(&INVERT_SELECTION), true) {
            self.edit(|state| state.inverse_selection());
        }
        ui.separator();
        if item(ui, "Flip Horizontally", None, paint) {
            self.edit(|state| state.flip_x());
        }
        if item(ui, "Flip Vertically", None, paint) {
            self.edit(|state| state.flip_y());
        }
        if item(ui, "Crop to Selection", None, selected && paint) {
            self.edit(|state| state.crop());
        }
        ui.separator();
        if item(ui, "Justify Left", None, paint) {
            self.edit(|state| state.justify_left());
        }
        if item(ui, "Justify Center", None, paint) {
            self.edit(|state| state.center());
        }
        if item(ui, "Justify Right", None, paint) {
            self.edit(|state| state.justify_right());
        }
    }

    fn colors_menu(&mut self, ui: &mut egui::Ui) {
        if item(ui, "Edit Palette…", None, true) {
            self.palette_edit = self.document.with_state(|state| state.get_buffer().palette.clone());
            self.palette_index = 0;
            self.dialog = Some(Dialog::Palette);
        }
        ui.separator();
        for (operation, label, shortcut) in [
            (ColorOp::NextForeground, "Next Foreground Color", Some(&NEXT_FG)),
            (ColorOp::PreviousForeground, "Previous Foreground Color", Some(&PREV_FG)),
            (ColorOp::NextBackground, "Next Background Color", Some(&NEXT_BG)),
            (ColorOp::PreviousBackground, "Previous Background Color", Some(&PREV_BG)),
            (ColorOp::PickAttributeUnderCaret, "Pick Attribute under Caret", Some(&PICK_ATTRIBUTE)),
            (ColorOp::Swap, "Swap Foreground/Background", Some(&SWAP_COLORS)),
            (ColorOp::Default, "Default Colors", None),
        ] {
            if matches!(operation, ColorOp::NextBackground | ColorOp::PickAttributeUnderCaret) {
                ui.separator();
            }
            if item(ui, label, shortcut, true) {
                self.color_operation(operation);
            }
        }
    }

    pub(super) fn area_operation(&mut self, operation: AreaOp) {
        if !self.document.can_paint() {
            return;
        }
        self.document.finish();
        self.edit(|state| operation.apply(state));
    }

    pub(super) fn color_operation(&mut self, operation: ColorOp) {
        let (foreground, background, count, under_caret) = self.document.with_state(|state| {
            let attribute = state.get_caret().attribute;
            let position = state.get_caret().position();
            let under_caret = state
                .get_cur_layer()
                .map(|layer| layer.char_at(position).attribute)
                .unwrap_or_else(|| state.get_buffer().char_at(position).attribute);
            (
                attribute.foreground(),
                attribute.background(),
                state.get_buffer().palette.len().max(1) as u32,
                under_caret,
            )
        });
        let result = match operation {
            ColorOp::NextForeground => self.document.set_caret_foreground((foreground + 1) % count),
            ColorOp::PreviousForeground => self.document.set_caret_foreground((foreground + count - 1) % count),
            ColorOp::NextBackground => self.document.set_caret_background((background + 1) % count),
            ColorOp::PreviousBackground => self.document.set_caret_background((background + count - 1) % count),
            ColorOp::PickAttributeUnderCaret => self
                .document
                .set_caret_foreground(under_caret.foreground())
                .and_then(|()| self.document.set_caret_background(under_caret.background())),
            ColorOp::Swap => self.document.swap_caret_colors(),
            ColorOp::Default => self.document.reset_caret_colors(),
        };
        self.result(result);
    }

    fn document_menu(&mut self, ui: &mut egui::Ui) {
        if item(ui, "Canvas Size…", None, self.charfont.is_none()) {
            self.open_resize();
        }
        if item(ui, "SAUCE Information…", None, self.charfont.is_none()) {
            self.open_sauce();
        }
        ui.separator();
        if item(ui, "Edit Bitmap Font…", None, true) {
            self.edit_bitmap_font();
        }
        if item(ui, "Tags…", None, self.charfont.is_none()) {
            self.dialog = Some(Dialog::Tags);
        }
    }

    fn view_menu(&mut self, ui: &mut egui::Ui, context: &egui::Context) {
        let current = self.settings.monitor_settings.scaling_mode;
        ui.menu_button(format!("Zoom ({})", zoom_label(current)), |ui| {
            if item(ui, "Zoom In", Some(&ZOOM_IN), true) {
                self.zoom_step(1);
            }
            if item(ui, "Zoom Out", Some(&ZOOM_OUT), true) {
                self.zoom_step(-1);
            }
            ui.separator();
            for (mode, shortcut) in [(ScalingMode::Auto, Some(&ZOOM_FIT)), (ScalingMode::FitWidth, None)] {
                let mut active = current == mode;
                if check_item(ui, &zoom_label(mode), shortcut, &mut active) {
                    self.settings.monitor_settings.scaling_mode = mode;
                }
            }
            ui.separator();
            for (label, zoom) in ZOOM_PRESETS {
                let mut active = current == ScalingMode::Manual(zoom);
                let shortcut = (zoom == 1.0).then_some(&ZOOM_ACTUAL);
                if check_item(ui, label, shortcut, &mut active) {
                    self.settings.monitor_settings.scaling_mode = ScalingMode::Manual(zoom);
                }
            }
        });
        ui.separator();
        ui.menu_button("Guides", |ui| {
            let mut off = self.guide.is_none();
            if check_item(ui, "Off", None, &mut off) {
                self.guide = None;
            }
            ui.separator();
            for (label, columns, rows) in GUIDES {
                let mut active = self.guide == Some((columns, rows));
                if check_item(ui, label, None, &mut active) {
                    self.guide = Some((columns, rows));
                    self.show_guide = true;
                }
            }
            ui.separator();
            ui.add_enabled_ui(self.guide.is_some(), |ui| {
                check_item(ui, "Show Guide", Some(&TOGGLE_GUIDE), &mut self.show_guide);
            });
        });
        ui.menu_button("Raster", |ui| {
            let mut off = self.raster.is_none();
            if check_item(ui, "Off", None, &mut off) {
                self.raster = None;
            }
            ui.separator();
            for (columns, rows) in RASTERS {
                let mut active = self.raster == Some((columns, rows));
                if check_item(ui, &format!("{columns}×{rows}"), None, &mut active) {
                    self.raster = Some((columns, rows));
                    self.show_raster = true;
                }
            }
            ui.separator();
            ui.add_enabled_ui(self.raster.is_some(), |ui| {
                check_item(ui, "Show Raster", None, &mut self.show_raster);
            });
        });
        check_item(ui, "Character Grid", Some(&GRID), &mut self.show_grid);
        check_item(ui, "Layer Borders", None, &mut self.show_layer_bounds);
        if check_item(ui, "Line Numbers", Some(&LINE_NUMBERS), &mut self.show_line_numbers) {
            self.store_line_numbers();
        }
        check_item(ui, "Side Panel", Some(&PANELS), &mut self.show_inspector);
        ui.separator();
        if item(ui, "Reference Image…", Some(&REFERENCE_IMAGE), true) {
            self.open_reference_image();
        }
        let mut reference_visible = self.reference_image.as_ref().is_some_and(|image| image.visible);
        ui.add_enabled_ui(self.reference_image.is_some(), |ui| {
            if check_item(ui, "Show Reference Image", Some(&TOGGLE_REFERENCE_IMAGE), &mut reference_visible) {
                self.toggle_reference_image();
            }
        });
        ui.separator();
        let mut fullscreen = context.input(|input| input.viewport().fullscreen.unwrap_or(false));
        if check_item(ui, "Fullscreen", Some(&FULLSCREEN), &mut fullscreen) {
            context.send_viewport_cmd(egui::ViewportCommand::Fullscreen(fullscreen));
        }
        ui.menu_button("Appearance", |ui| {
            egui::widgets::global_theme_preference_buttons(ui);
        });
        ui.separator();
        if item(ui, "Character Table…", None, true) {
            self.dialog = Some(Dialog::Characters);
        }
        if item(ui, "Monitor Settings…", None, true) {
            self.dialog = Some(Dialog::Monitor);
        }
    }

    pub(super) fn toggle_fullscreen(&mut self, context: &egui::Context) {
        let fullscreen = context.input(|input| input.viewport().fullscreen.unwrap_or(false));
        context.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fullscreen));
    }

    pub(super) fn store_line_numbers(&mut self) {
        self.settings.show_line_numbers = self.show_line_numbers;
        if self.persist_settings {
            self.settings.store_persistent();
        }
    }

    pub(super) fn open_reference_image(&mut self) {
        self.reference_draft = self.reference_image.clone().unwrap_or_else(|| icy_engine_gui::ReferenceImageSettings {
            mode: icy_engine_gui::ReferenceImageMode::Stretch,
            ..Default::default()
        });
        self.reference_path = self.reference_draft.path.display().to_string();
        self.dialog = Some(Dialog::ReferenceImage);
    }

    pub(super) fn toggle_reference_image(&mut self) {
        if let Some(image) = &mut self.reference_image {
            image.visible = !image.visible;
        }
    }

    fn open_url(&mut self, url: &str) {
        let result = open::that(url).map_err(|error| error.to_string());
        self.result(result);
    }

    fn open_log_file(&mut self) {
        let Some(log_file) = icy_draw::Settings::log_file() else {
            return;
        };
        let target = if log_file.exists() {
            log_file
        } else if let Some(parent) = log_file.parent() {
            parent.to_path_buf()
        } else {
            return;
        };
        let result = open::that(&target).map_err(|error| error.to_string());
        self.result(result);
    }

    fn extensions_menu(&mut self, ui: &mut egui::Ui) {
        if item(ui, "Run Lua Script…", None, true) {
            self.dialog = Some(Dialog::Script);
        }
        if ui.button("Reload Plugins").clicked() {
            self.plugins = None;
        }
        let paint = self.document.can_paint();
        let mut run = None;
        let plugins = self.plugins.get_or_insert_with(icy_draw::plugins::Plugin::read_plugin_directory);
        if !plugins.is_empty() {
            ui.separator();
        }
        for plugin in plugins.iter() {
            let label = plugin.path.iter().cloned().chain([plugin.title.clone()]).collect::<Vec<_>>().join(" / ");
            if ui
                .add_enabled(paint, egui::Button::new(label))
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
    }

    pub(super) fn new_window(&mut self) {
        let result = std::env::current_exe()
            .and_then(|executable| std::process::Command::new(executable).spawn())
            .map(|_| ())
            .map_err(|error| error.to_string());
        self.result(result);
    }

    pub(super) fn open_sauce(&mut self) {
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
    }

    pub(super) fn open_resize(&mut self) {
        let size = self.document.with_state(|state| state.get_buffer().size());
        self.new_size = [size.width, size.height];
        self.dialog = Some(Dialog::Resize);
    }

    pub(super) fn edit_bitmap_font(&mut self) {
        let Some(font) = self
            .document
            .with_state(|state| state.get_buffer().font(state.get_caret().attribute.font_page()).cloned())
        else {
            return;
        };
        if font.size().width <= 8 {
            self.font_editor = Some(super::super::font::FontEditor::new(font));
            self.dialog = Some(Dialog::Font);
        } else {
            self.dialog = Some(Dialog::Error(
                "The bitmap font editor currently supports glyphs up to 8 pixels wide.".into(),
            ));
        }
    }

    /// Moves to the next larger (`direction > 0`) or smaller manual zoom level.
    pub(super) fn zoom_step(&mut self, direction: i32) {
        let zoom = self.view.zoom;
        let next = if direction > 0 {
            ZOOM_STEPS.iter().copied().find(|step| *step > zoom + 0.01).unwrap_or(ZOOM_STEPS[ZOOM_STEPS.len() - 1])
        } else {
            ZOOM_STEPS.iter().rev().copied().find(|step| *step < zoom - 0.01).unwrap_or(ZOOM_STEPS[0])
        };
        self.settings.monitor_settings.scaling_mode = ScalingMode::Manual(next);
    }

    /// Handles the application-wide command shortcuts; returns true when `key` was consumed.
    pub(super) fn command_key(&mut self, context: &egui::Context, key: Key, modifiers: Modifiers) -> bool {
        let shift = modifiers.shift;
        let animation = self.animation.is_some();
        match key {
            Key::N if shift => self.new_window(),
            Key::Q if !shift => context.send_viewport_cmd(egui::ViewportCommand::Close),
            Key::E if !shift && !animation => self.dialog = Some(Dialog::Export),
            Key::Plus | Key::Equals if !animation => self.zoom_step(1),
            Key::Minus if !animation => self.zoom_step(-1),
            Key::Num0 if !animation => self.settings.monitor_settings.scaling_mode = ScalingMode::Auto,
            Key::Num1 if !animation => self.settings.monitor_settings.scaling_mode = ScalingMode::Manual(1.0),
            Key::G if !shift && !animation => self.show_grid = !self.show_grid,
            Key::P if shift && !animation => self.show_inspector = !self.show_inspector,
            Key::D if !shift && !animation && self.canvas_focus => self.edit(|state| state.clear_selection()),
            Key::I if shift && !animation && self.canvas_focus => self.edit(|state| state.inverse_selection()),
            Key::ArrowDown if !shift && !animation && self.canvas_focus => self.color_operation(ColorOp::NextForeground),
            Key::ArrowUp if !shift && !animation && self.canvas_focus => self.color_operation(ColorOp::PreviousForeground),
            Key::ArrowRight if !shift && !animation && self.canvas_focus => self.color_operation(ColorOp::NextBackground),
            Key::ArrowLeft if !shift && !animation && self.canvas_focus => self.color_operation(ColorOp::PreviousBackground),
            Key::R if !shift && !animation => {
                self.show_line_numbers = !self.show_line_numbers;
                self.store_line_numbers();
            }
            Key::Semicolon if !shift && !animation && self.guide.is_some() => self.show_guide = !self.show_guide,
            Key::O if shift && !animation => self.open_reference_image(),
            Key::Tab if !shift && !animation && self.reference_image.is_some() => self.toggle_reference_image(),
            #[cfg(target_os = "macos")]
            Key::F if modifiers.ctrl && !shift => self.toggle_fullscreen(context),
            _ => return false,
        }
        true
    }

    /// Alt shortcuts of the original editor: area operations, attribute picking, color swap and fullscreen.
    pub(super) fn alt_key(&mut self, context: &egui::Context, key: Key, modifiers: Modifiers) -> bool {
        #[cfg(not(target_os = "macos"))]
        if modifiers.matches_exact(FULLSCREEN.modifiers) && key == FULLSCREEN.logical_key {
            self.toggle_fullscreen(context);
            return true;
        }
        #[cfg(target_os = "macos")]
        let _ = context;
        if self.animation.is_some() || self.document.paste_active() {
            return false;
        }
        if let Some(operation) = AreaOp::ALL.into_iter().find(|operation| {
            operation
                .shortcut()
                .is_some_and(|shortcut| shortcut.logical_key == key && modifiers.matches_exact(shortcut.modifiers))
        }) {
            self.area_operation(operation);
            return true;
        }
        let color = [(PICK_ATTRIBUTE, ColorOp::PickAttributeUnderCaret), (SWAP_COLORS, ColorOp::Swap)]
            .into_iter()
            .find(|(shortcut, _)| shortcut.logical_key == key && modifiers.matches_exact(shortcut.modifiers));
        if let Some((_, operation)) = color {
            self.color_operation(operation);
            return true;
        }
        false
    }

    pub(super) fn shortcuts_dialog(&mut self, context: &egui::Context) -> bool {
        let format = |shortcut: &KeyboardShortcut| context.format_shortcut(shortcut);
        let groups: Vec<(&str, Vec<(String, &str)>)> = vec![
            (
                "File",
                vec![
                    (format(&NEW), "New document"),
                    (format(&OPEN), "Open"),
                    (format(&SAVE), "Save"),
                    (format(&SAVE_AS), "Save as"),
                    (format(&EXPORT), "Export"),
                    (format(&NEW_WINDOW), "New window"),
                    (format(&QUIT), "Quit"),
                ],
            ),
            (
                "Edit",
                vec![
                    (format(&UNDO), "Undo"),
                    (format(&REDO), "Redo"),
                    (format(&CUT), "Cut"),
                    (format(&COPY), "Copy"),
                    (format(&PASTE), "Paste as floating layer"),
                    (format(&SELECT_ALL), "Select all"),
                    (format(&DESELECT), "Deselect"),
                    (format(&INVERT_SELECTION), "Invert selection"),
                    ("Del".into(), "Erase selection"),
                    ("Esc".into(), "Cancel stroke / clear selection"),
                ],
            ),
            (
                "View",
                vec![
                    (format(&ZOOM_IN), "Zoom in"),
                    (format(&ZOOM_OUT), "Zoom out"),
                    (format(&ZOOM_FIT), "Fit to window"),
                    (format(&ZOOM_ACTUAL), "Actual size (100%)"),
                    (
                        if cfg!(target_os = "macos") { "Cmd+Wheel" } else { "Ctrl+Wheel" }.into(),
                        "Zoom",
                    ),
                    (format(&GRID), "Toggle character grid"),
                    (format(&LINE_NUMBERS), "Toggle line numbers"),
                    (format(&TOGGLE_GUIDE), "Toggle guide"),
                    (format(&REFERENCE_IMAGE), "Reference image"),
                    (format(&TOGGLE_REFERENCE_IMAGE), "Toggle reference image"),
                    (format(&PANELS), "Toggle side panel"),
                    (format(&FULLSCREEN), "Fullscreen"),
                ],
            ),
            (
                "Colors",
                vec![
                    (format(&NEXT_FG), "Next foreground color"),
                    (format(&PREV_FG), "Previous foreground color"),
                    (format(&NEXT_BG), "Next background color"),
                    (format(&PREV_BG), "Previous background color"),
                    (format(&PICK_ATTRIBUTE), "Pick attribute under caret"),
                    (format(&SWAP_COLORS), "Swap foreground / background"),
                ],
            ),
            (
                "Area Operations",
                AreaOp::ALL
                    .into_iter()
                    .filter_map(|operation| operation.shortcut().map(|shortcut| (format(&shortcut), operation.label())))
                    .collect(),
            ),
            (
                "Text Cursor Tool",
                vec![
                    ("F1–F12".into(), "Type character from the active F-key set"),
                    ("Alt+F1–F10".into(), "Choose F-key set 1–10"),
                    ("Alt+Shift+F1–F10".into(), "Choose F-key set 11–20"),
                    (format(&KeyboardShortcut::new(Modifiers::COMMAND, Key::Comma)), "Previous F-key set"),
                    (format(&KeyboardShortcut::new(Modifiers::COMMAND, Key::Period)), "Next F-key set"),
                    (format(&KeyboardShortcut::new(Modifiers::COMMAND, Key::Slash)), "Default F-key set"),
                    ("Shift+Arrows".into(), "Extend selection"),
                    ("Ins".into(), "Toggle insert / overwrite"),
                    ("Shift+Space".into(), "Hard blank (0xFF)"),
                    ("Tab".into(), "Next tab stop"),
                ],
            ),
            (
                "Brushes & Shapes",
                vec![
                    ("Alt++".into(), "Larger brush"),
                    ("Alt+-".into(), "Smaller brush"),
                    ("Alt+]".into(), "Reset brush size"),
                ],
            ),
            (
                "Floating Paste",
                vec![
                    ("Arrows".into(), "Move"),
                    ("Enter".into(), "Anchor"),
                    ("S".into(), "Stamp copy"),
                    ("R".into(), "Rotate"),
                    ("X".into(), "Flip horizontally"),
                    ("Y".into(), "Flip vertically"),
                    ("T".into(), "Toggle transparency"),
                    ("Esc".into(), "Cancel"),
                ],
            ),
        ];
        let response = appearance::Dialog::new("draw-shortcuts")
            .title("Keyboard Shortcuts")
            .size(DialogSize::Large)
            .max_height(620.0)
            .show(context, |dialog| {
                dialog.content(|ui| {
                    for (title, entries) in &groups {
                        appearance::compact_group(ui, title, |ui| {
                            for (keys, description) in entries {
                                ui.horizontal(|ui| {
                                    ui.allocate_ui_with_layout(egui::vec2(170.0, 22.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                        ui.set_min_width(170.0);
                                        ui.spacing_mut().item_spacing.x = 3.0;
                                        keycaps(ui, keys);
                                    });
                                    ui.label(*description);
                                });
                            }
                        });
                    }
                });
                dialog.buttons([DialogButton::primary(labels::close(), ()).cancels()]);
            });
        response.action.is_some() || response.dismissed
    }

    pub(super) fn about_dialog(&mut self, context: &egui::Context) -> bool {
        let response = appearance::Dialog::new("draw-about")
            .title("Icy Draw")
            .subtitle(format!("Version {}", env!("CARGO_PKG_VERSION")))
            .size(DialogSize::Medium)
            .show(context, |dialog| {
                dialog.content(|ui| {
                    appearance::group(ui, "", |ui| {
                        ui.label("ANSI, ASCII and PETSCII art editor with layers, TheDraw fonts, bitmap fonts and animations.");
                        ui.add_space(4.0);
                        appearance::value_row(ui, "Author", "Mike Krüger");
                        appearance::value_row(ui, "License", env!("CARGO_PKG_LICENSE"));
                        ui.horizontal(|ui| {
                            ui.hyperlink_to("github.com/mkrueger/icy_tools", "https://github.com/mkrueger/icy_tools");
                        });
                    });
                });
                dialog.buttons([DialogButton::primary(labels::close(), ()).cancels()]);
            });
        response.action.is_some() || response.dismissed
    }

    pub(super) fn insert_image(&mut self, path: &std::path::Path) {
        let result = image::open(path)
            .map_err(|error| format!("Could not load {}: {error}", path.display()))
            .and_then(|image| self.document.start_image_paste(&image.to_rgba8()).map_err(|error| error.to_string()));
        if result.is_ok() {
            self.canvas_focus = true;
        }
        self.result(result);
    }

    pub(super) fn reference_image_dialog(&mut self, context: &egui::Context) -> bool {
        use icy_engine_gui::ReferenceImageMode;
        #[derive(Clone, Copy)]
        enum Action {
            Clear,
            Cancel,
            Apply,
        }
        const MODES: [(ReferenceImageMode, &str); 6] = [
            (ReferenceImageMode::Stretch, "Stretch to Canvas"),
            (ReferenceImageMode::Contain, "Fit to Canvas"),
            (ReferenceImageMode::FitWidth, "Fit Width"),
            (ReferenceImageMode::FitHeight, "Fit Height"),
            (ReferenceImageMode::Original, "Original Size"),
            (ReferenceImageMode::Tile, "Tile"),
        ];
        let mode_label = |mode| MODES.iter().find(|(candidate, _)| *candidate == mode).map_or("", |(_, label)| *label);
        let mut browse = false;
        let path_valid = {
            let path = std::path::Path::new(self.reference_path.trim());
            !path.as_os_str().is_empty() && path.is_file()
        };
        let response = appearance::Dialog::new("reference-image")
            .title("Reference Image")
            .subtitle("Shown semi-transparent behind the canvas; not saved into exported files.")
            .size(DialogSize::Medium)
            .show(context, |dialog| {
                dialog.content(|ui| {
                    appearance::group(ui, "Image", |ui| {
                        appearance::form_row(ui, "File", |ui| {
                            ui.horizontal(|ui| {
                                let button_width = 80.0;
                                let width = (ui.available_width() - button_width - ui.spacing().item_spacing.x).max(80.0);
                                ui.add_sized([width, ui.spacing().interact_size.y], appearance::text_edit(&mut self.reference_path));
                                browse = ui
                                    .add_sized([button_width, ui.spacing().interact_size.y], egui::Button::new("Browse…"))
                                    .clicked();
                            });
                        });
                        if !self.reference_path.trim().is_empty() && !path_valid {
                            appearance::form_row(ui, "", |ui| {
                                ui.colored_label(ui.visuals().error_fg_color, "File not found");
                            });
                        }
                    });
                    appearance::group(ui, "Display", |ui| {
                        let draft = &mut self.reference_draft;
                        appearance::combo_row(ui, "Mode", mode_label(draft.mode), |ui| {
                            for (mode, label) in MODES {
                                ui.selectable_value(&mut draft.mode, mode, label);
                            }
                        });
                        let mut opacity = draft.alpha * 100.0;
                        appearance::slider_row(ui, "Opacity %", &mut opacity, 5.0..=100.0);
                        draft.alpha = (opacity / 100.0).clamp(0.05, 1.0);
                        if matches!(draft.mode, ReferenceImageMode::Original | ReferenceImageMode::Tile) {
                            appearance::form_row(ui, "Scale", |ui| {
                                ui.add(egui::DragValue::new(&mut draft.scale).range(0.05..=16.0).speed(0.01).fixed_decimals(2));
                            });
                        }
                        appearance::form_row(ui, "Offset", |ui| {
                            ui.add(egui::DragValue::new(&mut draft.offset.0).prefix("x ").suffix(" px"));
                            ui.add(egui::DragValue::new(&mut draft.offset.1).prefix("y ").suffix(" px"));
                        });
                        appearance::check_row(ui, "Visible", &mut draft.visible);
                    });
                });
                dialog.buttons([
                    DialogButton::destructive("Remove", Action::Clear)
                        .leading()
                        .enabled(self.reference_image.is_some()),
                    DialogButton::cancel(labels::cancel(), Action::Cancel),
                    DialogButton::primary(labels::ok(), Action::Apply).enabled(path_valid),
                ]);
            });
        if browse {
            self.choose(context, FileAction::ReferenceImage);
        }
        match response.action {
            Some(Action::Clear) => {
                self.reference_image = None;
                true
            }
            Some(Action::Cancel) => true,
            Some(Action::Apply) => {
                let mut image = self.reference_draft.clone();
                image.path = std::path::PathBuf::from(self.reference_path.trim());
                if image.load_and_cache().is_none() {
                    self.dialog = Some(Dialog::Error(format!("Could not load reference image {}", image.path.display())));
                    return true;
                }
                self.reference_image = Some(image);
                true
            }
            None => response.dismissed,
        }
    }
}
