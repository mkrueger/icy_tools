use std::collections::HashMap;

use eframe::egui::{self, Modifiers};
use egui_bind::{BindTarget, KeyOrPointer};
use i18n_embed_fl::fl;
use icy_engine::PaletteMode;

use crate::{button_with_shortcut, DocumentTab, Message, SETTINGS};

pub trait CommandState {
    fn is_enabled(&self, _open_tab_opt: Option<&DocumentTab>) -> bool {
        true
    }
    fn is_checked(&self, _open_tab_opt: Option<&DocumentTab>) -> Option<bool> {
        None
    }
}

#[derive(Default)]
pub struct AlwaysEnabledState {}
impl CommandState for AlwaysEnabledState {}

#[derive(Default)]
pub struct BufferOpenState {}

impl CommandState for BufferOpenState {
    fn is_enabled(&self, open_tab_opt: Option<&DocumentTab>) -> bool {
        if let Some(pane) = open_tab_opt {
            return pane.doc.lock().get_ansi_editor().is_some();
        }
        false
    }
}

#[derive(Default)]
pub struct CanSwitchPaletteState {}

impl CommandState for CanSwitchPaletteState {
    fn is_enabled(&self, open_tab_opt: Option<&DocumentTab>) -> bool {
        if let Some(pane) = open_tab_opt {
            if let Some(editor) = pane.doc.lock().get_ansi_editor() {
                return !matches!(editor.buffer_view.lock().get_buffer().palette_mode, PaletteMode::Fixed16);
            }
        }
        false
    }
}

#[derive(Default)]
pub struct LayerBordersState {}

impl CommandState for LayerBordersState {
    fn is_enabled(&self, _open_tab_opt: Option<&DocumentTab>) -> bool {
        true
    }

    fn is_checked(&self, _open_tab_opt: Option<&DocumentTab>) -> Option<bool> {
        unsafe { Some(SETTINGS.show_layer_borders) }
    }
}

#[derive(Default)]
pub struct LineNumberState {}

impl CommandState for LineNumberState {
    fn is_enabled(&self, _open_tab_opt: Option<&DocumentTab>) -> bool {
        true
    }

    fn is_checked(&self, _open_tab_opt: Option<&DocumentTab>) -> Option<bool> {
        unsafe { Some(SETTINGS.show_line_numbers) }
    }
}

#[derive(Default)]
pub struct FileOpenState {}

impl CommandState for FileOpenState {
    fn is_enabled(&self, open_tab_opt: Option<&DocumentTab>) -> bool {
        open_tab_opt.is_some()
    }
}

#[derive(Default)]
pub struct FileIsDirtyState {}

impl CommandState for FileIsDirtyState {
    fn is_enabled(&self, open_tab_opt: Option<&DocumentTab>) -> bool {
        if let Some(pane) = open_tab_opt {
            pane.is_dirty()
        } else {
            false
        }
    }
}

/*
#[derive(Default)]
pub struct HasRecentFilesState {}

impl CommandState for HasRecentFilesState {
    fn is_enabled(&self, _open_tab_opt: Option<&DocumentTab>) -> bool {
        !self.MRU_FILES.get_recent_files().is_empty()
    }
}
*/

#[derive(Default)]
pub struct CanUndoState {}

impl CommandState for CanUndoState {
    fn is_enabled(&self, open_tab_opt: Option<&DocumentTab>) -> bool {
        if let Some(pane) = open_tab_opt {
            return pane.doc.lock().can_undo();
        }
        false
    }
}
#[derive(Default)]
pub struct CanRedoState {}

impl CommandState for CanRedoState {
    fn is_enabled(&self, open_tab_opt: Option<&DocumentTab>) -> bool {
        if let Some(pane) = open_tab_opt {
            return pane.doc.lock().can_redo();
        }
        false
    }
}

#[derive(Default)]
pub struct CanCutState {}

impl CommandState for CanCutState {
    fn is_enabled(&self, open_tab_opt: Option<&DocumentTab>) -> bool {
        if let Some(pane) = open_tab_opt {
            return pane.doc.lock().can_cut();
        }
        false
    }
}

#[derive(Default)]
pub struct CanCopyState {}

impl CommandState for CanCopyState {
    fn is_enabled(&self, open_tab_opt: Option<&DocumentTab>) -> bool {
        if let Some(pane) = open_tab_opt {
            return pane.doc.lock().can_copy();
        }
        false
    }
}

#[derive(Default)]
pub struct CanPasteState {}

impl CommandState for CanPasteState {
    fn is_enabled(&self, open_tab_opt: Option<&DocumentTab>) -> bool {
        if let Some(pane) = open_tab_opt {
            return pane.doc.lock().can_paste();
        }
        false
    }
}

#[derive(Default)]
pub struct LGAFontState {}

impl CommandState for LGAFontState {
    fn is_enabled(&self, open_tab_opt: Option<&DocumentTab>) -> bool {
        if let Some(pane) = open_tab_opt {
            return pane.doc.lock().get_ansi_editor().is_some();
        }
        false
    }
    fn is_checked(&self, open_tab_opt: Option<&DocumentTab>) -> Option<bool> {
        if let Some(pane) = open_tab_opt {
            if let Some(editor) = pane.doc.lock().get_ansi_editor() {
                return Some(editor.buffer_view.lock().get_buffer().use_letter_spacing());
            }
        }
        Some(false)
    }
}
#[derive(Default)]
pub struct AspectRatioState {}

impl CommandState for AspectRatioState {
    fn is_enabled(&self, open_tab_opt: Option<&DocumentTab>) -> bool {
        if let Some(pane) = open_tab_opt {
            return pane.doc.lock().get_ansi_editor().is_some();
        }
        false
    }
    fn is_checked(&self, open_tab_opt: Option<&DocumentTab>) -> Option<bool> {
        if let Some(pane) = open_tab_opt {
            if let Some(editor) = pane.doc.lock().get_ansi_editor() {
                return Some(editor.buffer_view.lock().get_buffer().use_aspect_ratio());
            }
        }
        Some(false)
    }
}

pub struct CommandWrapper {
    key: Option<(KeyOrPointer, Modifiers)>,
    message: Message,
    label: String,
    pub is_enabled: bool,
    pub is_checked: Option<bool>,
    state_key: u32,
}

mod modifier_keys {
    use eframe::egui::Modifiers;

    pub const NONE: Modifiers = Modifiers {
        alt: false,
        ctrl: false,
        shift: false,
        mac_cmd: false,
        command: false,
    };

    pub const CTRL: Modifiers = Modifiers {
        alt: false,
        ctrl: true,
        shift: false,
        mac_cmd: false,
        command: false,
    };

    pub const ALT: Modifiers = Modifiers {
        alt: true,
        ctrl: false,
        shift: false,
        mac_cmd: false,
        command: false,
    };

    pub const ALT_CTRL: Modifiers = Modifiers {
        alt: true,
        ctrl: true,
        shift: false,
        mac_cmd: false,
        command: false,
    };

    pub const CTRL_SHIFT: Modifiers = Modifiers {
        alt: false,
        ctrl: true,
        shift: true,
        mac_cmd: false,
        command: false,
    };
}

macro_rules! key {
    () => {
        None
    };
    ($key:ident, $modifier: ident) => {
        Some((egui::Key::$key, modifier_keys::$modifier))
    };
}

macro_rules! keys {
    ($( ($l:ident, $translation: expr, $message:ident, $cmd_state: ident$(, $key:ident, $modifier: ident)? ) ),* $(,)? ) => {

        pub struct Commands {
            state_map: HashMap<u32, Box<dyn CommandState>>,
            $(
                pub $l: CommandWrapper,
            )*
        }

        impl Default for Commands {
            fn default() -> Self {
                let mut state_map = HashMap::<u32, Box<dyn CommandState>>::new();
                $(
                    state_map.insert(hash(stringify!($cmd_state)), Box::<$cmd_state>::default());
                )*

                Self {
                    state_map,
                    $(
                        $l: CommandWrapper::new(key!($($key, $modifier)?), Message::$message, fl!(crate::LANGUAGE_LOADER, $translation), hash(stringify!($cmd_state))),
                    )*
                }
            }
        }

        impl Commands {
            pub fn default_keybindings() -> Vec<(String, egui::Key, Modifiers)> {
                let mut result = Vec::new();
                $(
                    let key = key!($($key, $modifier)?);
                    if let Some((key, modifier)) = key  {
                        result.push((stringify!($l).to_string(), key, modifier));
                    }
                )*
                result
            }
            pub fn check(&self, ctx: &egui::Context, message: &mut Option<Message>) {
                $(
                    if self.$l.is_pressed(ctx) {
                        *message = Some(self.$l.message.clone());
                        return;
                    }
                )*
            }

            pub fn update_states(&mut self, open_tab_opt: Option<&DocumentTab>) {
                let mut result_map = HashMap::new();
                for (k, v) in &self.state_map {
                    let is_enabled = v.is_enabled(open_tab_opt);
                    let is_checked = v.is_checked(open_tab_opt);
                    result_map.insert(k, (is_enabled, is_checked));
                }

                $(
                    self.$l.update_state(&result_map);
                )*
            }

            pub fn apply_key_bindings(&mut self, key_bindings: &Vec<(String, egui::Key, Modifiers)> ) {
                for (binding, key, modifier) in key_bindings {
                    match binding.as_str() {
                        $(
                            stringify!($l) => {
                                self.$l.key = Some((KeyOrPointer::Key(*key), *modifier));
                            }
                        )*

                        _ => {}
                    }
                }
            }

            pub(crate) fn show_keybinds_settings(ui: &mut egui::Ui, filter: &mut String, keys: &mut HashMap<String, (egui::Key, Modifiers)>) -> bool {
                let mut changed_bindings = false;

                ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                    let response = ui.button("🗙");
                    if response.clicked() {
                        filter.clear();
                    }

                    ui.add(
                        egui::TextEdit::singleline(filter).hint_text(fl!(
                            crate::LANGUAGE_LOADER,
                            "settings-key_filter_preview_text"
                        )),
                    );
                });
                egui::ScrollArea::vertical()
                    .max_height(240.0)
                    .show(ui, |ui| {

                    $(
                        let label = fl!(crate::LANGUAGE_LOADER, $translation);
                        if filter.is_empty() || label.to_lowercase().contains(filter.to_lowercase().as_str()) {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                                let mut bind = if let Some(x) = keys.get(stringify!($l)) { Some(x.clone ()) } else {None };
                                if ui.add(egui_bind::Bind::new(
                                    stringify!($l).to_string(),
                                    &mut bind,
                                )).changed() {
                                    if let Some(bind) = bind {
                                        keys.insert(stringify!($l).into(), bind);
                                    }  else {
                                        keys.remove(stringify!($l));
                                    }
                                    changed_bindings = true;
                                }
                                ui.label(label);
                            });
                        }
                    )*
                });
                changed_bindings
            }

        }
    };
}

fn hash(str: impl Into<String>) -> u32 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    str.into().hash(&mut hasher);
    hasher.finish() as u32
}

impl CommandWrapper {
    pub fn new(key: Option<(egui::Key, Modifiers)>, message: Message, description: String, state_key: u32) -> Self {
        let key = key.map(|(k, m)| (KeyOrPointer::Key(k), m));
        Self {
            key,
            message,
            label: description,
            state_key,
            is_enabled: true,
            is_checked: None,
        }
    }

    pub fn update_state(&mut self, result_map: &HashMap<&u32, (bool, Option<bool>)>) {
        if let Some((is_enabled, is_checked)) = result_map.get(&self.state_key) {
            self.is_enabled = *is_enabled;
            self.is_checked = *is_checked;
        }
    }

    pub fn is_pressed(&self, ctx: &egui::Context) -> bool {
        self.key.pressed(ctx)
    }

    pub fn ui(&self, ui: &mut egui::Ui, message: &mut Option<Message>) {
        if let Some(mut checked) = self.is_checked {
            if ui.add(egui::Checkbox::new(&mut checked, &self.label)).clicked() {
                *message = Some(self.message.clone());
                ui.close_menu();
            }
            return;
        }

        let response = ui.with_layout(ui.layout().with_cross_justify(true), |ui| {
            if !self.is_enabled {
                ui.disable();
            }
            if let Some((KeyOrPointer::Key(k), modifier)) = self.key {
                let mut shortcut = k.name().to_string();

                if modifier.ctrl {
                    shortcut.insert_str(0, "Ctrl+");
                }

                if modifier.alt {
                    shortcut.insert_str(0, "Alt+");
                }

                if modifier.shift {
                    shortcut.insert_str(0, "Shift+");
                }

                button_with_shortcut(ui, true, &self.label, shortcut)
            } else {
                ui.add(egui::Button::new(&self.label).wrap_mode(egui::TextWrapMode::Truncate))
            }
        });

        if response.inner.clicked() {
            *message = Some(self.message.clone());
            ui.close_menu();
        }
    }
}

keys![
    (new_file, "menu-new", NewFileDialog, AlwaysEnabledState, N, CTRL),
    (save, "menu-save", SaveFile, FileIsDirtyState, S, CTRL),
    (save_as, "menu-save-as", SaveFileAs, FileOpenState, S, CTRL_SHIFT),
    (open_file, "menu-open", OpenFileDialog, AlwaysEnabledState, O, CTRL),
    (export, "menu-export", ExportFile, BufferOpenState),
    (edit_font_outline, "menu-edit-font-outline", ShowOutlineDialog, AlwaysEnabledState),
    (close_window, "menu-close", CloseWindow, AlwaysEnabledState, Q, CTRL),
    (undo, "menu-undo", Undo, CanUndoState, Z, CTRL),
    (redo, "menu-redo", Redo, CanRedoState, Z, CTRL_SHIFT),
    (cut, "menu-cut", Cut, CanCutState, X, CTRL),
    (copy, "menu-copy", Copy, CanCopyState, C, CTRL),
    (paste, "menu-paste", Paste, CanPasteState, V, CTRL),
    (show_settings, "menu-show_settings", ShowSettings, AlwaysEnabledState),
    (select_all, "menu-select-all", SelectAll, BufferOpenState, A, CTRL),
    (deselect, "menu-select_nothing", SelectNothing, BufferOpenState),
    (erase_selection, "menu-erase", DeleteSelection, BufferOpenState, Delete, NONE),
    (flip_x, "menu-flipx", FlipX, BufferOpenState),
    (flip_y, "menu-flipy", FlipY, BufferOpenState),
    (justifycenter, "menu-justifycenter", Center, BufferOpenState),
    (justifyleft, "menu-justifyleft", JustifyLeft, BufferOpenState),
    (justifyright, "menu-justifyright", JustifyRight, BufferOpenState),
    (crop, "menu-crop", Crop, BufferOpenState),
    (about, "menu-about", ShowAboutDialog, AlwaysEnabledState),
    (justify_line_center, "menu-justify_line_center", CenterLine, BufferOpenState, C, ALT),
    (justify_line_left, "menu-justify_line_left", JustifyLineLeft, BufferOpenState, L, ALT),
    (justify_line_right, "menu-justify_line_right", JustifyLineRight, BufferOpenState, R, ALT),
    (insert_row, "menu-insert_row", InsertRow, BufferOpenState, ArrowUp, ALT),
    (delete_row, "menu-delete_row", DeleteRow, BufferOpenState, ArrowDown, ALT),
    (insert_column, "menu-insert_colum", InsertColumn, BufferOpenState, ArrowRight, ALT),
    (delete_column, "menu-delete_colum", DeleteColumn, BufferOpenState, ArrowLeft, ALT),
    (erase_row, "menu-erase_row", EraseRow, BufferOpenState, E, ALT),
    (erase_row_to_start, "menu-erase_row_to_start", EraseRowToStart, BufferOpenState, Home, ALT),
    (erase_row_to_end, "menu-erase_row_to_end", EraseRowToEnd, BufferOpenState, End, ALT),
    (erase_column, "menu-erase_column", EraseColumn, BufferOpenState, E, ALT),
    (
        erase_column_to_start,
        "menu-erase_column_to_start",
        EraseColumnToStart,
        BufferOpenState,
        Home,
        ALT
    ),
    (erase_column_to_end, "menu-erase_column_to_end", EraseColumnToEnd, BufferOpenState, End, ALT),
    (scroll_area_up, "menu-scroll_area_up", ScrollAreaUp, BufferOpenState, ArrowUp, ALT_CTRL),
    (scroll_area_down, "menu-scroll_area_down", ScrollAreaDown, BufferOpenState, ArrowDown, ALT_CTRL),
    (scroll_area_left, "menu-scroll_area_left", ScrollAreaLeft, BufferOpenState, ArrowLeft, ALT_CTRL),
    (
        scroll_area_right,
        "menu-scroll_area_right",
        ScrollAreaRight,
        BufferOpenState,
        ArrowRight,
        ALT_CTRL
    ),
    (set_reference_image, "menu-reference-image", SetReferenceImage, BufferOpenState, O, CTRL_SHIFT),
    (
        toggle_reference_image,
        "menu-toggle-reference-image",
        ToggleReferenceImage,
        BufferOpenState,
        Tab,
        CTRL
    ),
    (clear_reference_image, "menu-clear-reference-image", ClearReferenceImage, BufferOpenState),
    (
        pick_attribute_under_caret,
        "menu-pick_attribute_under_caret",
        PickAttributeUnderCaret,
        BufferOpenState,
        U,
        ALT
    ),
    (switch_to_default_color, "menu-default_color", SwitchToDefaultColor, BufferOpenState, D, CTRL),
    (toggle_color, "menu-toggle_color", ToggleColor, BufferOpenState, X, ALT),
    (fullscreen, "menu-toggle_fullscreen", ToggleFullScreen, AlwaysEnabledState, Enter, ALT),
    (zoom_reset, "menu-zoom_reset", ZoomReset, BufferOpenState, Backspace, CTRL),
    (zoom_in, "menu-zoom_in", ZoomIn, BufferOpenState, Plus, CTRL),
    (zoom_out, "menu-zoom_out", ZoomOut, BufferOpenState, Minus, CTRL),
    (open_tdf_directory, "menu-open_tdf_directoy", OpenTdfDirectory, AlwaysEnabledState),
    (open_font_selector, "menu-open_font_selector", OpenFontSelector, BufferOpenState),
    (add_fonts, "menu-add_fonts", OpenAddFonts, BufferOpenState),
    (open_font_manager, "menu-open_font_manager", OpenFontManager, BufferOpenState),
    (open_font_directory, "menu-open_font_directoy", OpenFontDirectory, AlwaysEnabledState),
    (
        open_palettes_directory,
        "menu-open_palettes_directoy",
        OpenPalettesDirectory,
        AlwaysEnabledState
    ),
    (mirror_mode, "menu-mirror_mode", ToggleMirrorMode, BufferOpenState),
    (clear_recent_open, "menu-open_recent_clear", ClearRecentOpenFiles, AlwaysEnabledState),
    (inverse_selection, "menu-inverse_selection", InverseSelection, BufferOpenState),
    (clear_selection, "menu-delete_row", ClearSelection, BufferOpenState, Escape, NONE),
    (select_palette, "menu-select_palette", SelectPalette, CanSwitchPaletteState),
    (show_layer_borders, "menu-show_layer_borders", ToggleLayerBorders, LayerBordersState),
    (show_line_numbers, "menu-show_line_numbers", ToggleLineNumbers, LineNumberState),
    (open_plugin_directory, "menu-open_plugin_directory", OpenPluginDirectory, AlwaysEnabledState),
    (next_fg_color, "menu-next_fg_color", NextFgColor, BufferOpenState, ArrowDown, CTRL),
    (prev_fg_color, "menu-prev_fg_color", PreviousFgColor, BufferOpenState, ArrowUp, CTRL),
    (next_bg_color, "menu-next_bg_color", NextBgColor, BufferOpenState, ArrowRight, CTRL),
    (prev_bg_color, "menu-prev_bg_color", PreviousBgColor, BufferOpenState, ArrowLeft, CTRL),
    (lga_font, "menu-9px-font", ToggleLGAFont, LGAFontState),
    (aspect_ratio, "menu-aspect-ratio", ToggleAspectRatio, AspectRatioState),
    (toggle_grid_guides, "menu-toggle_grid", ToggleGrid, BufferOpenState),
];
