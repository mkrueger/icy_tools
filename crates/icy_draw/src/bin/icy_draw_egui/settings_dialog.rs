//! Application settings in the layout of the classic Icy Draw settings dialog:
//! monitor, font outline style, F-key character sets and the application paths.

use std::path::PathBuf;

use eframe::egui;
use icy_draw::{fl, FKeySets, Settings};
use icy_engine_gui::{
    egui::appearance::{self, labels, DialogButton, DialogSize},
    MonitorSettings, ScalingMode,
};

use super::{widgets, DrawApp};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Monitor,
    FontOutline,
    Charset,
    Paths,
}

/// Edited copy of the settings; OK applies and stores it, Cancel drops it.
#[derive(Clone)]
pub struct SettingsDraft {
    pub page: Page,
    pub monitor: MonitorSettings,
    pub outline_style: usize,
    pub fkeys: FKeySets,
    /// F-key slot that the character grid assigns to.
    pub slot: Option<usize>,
}

impl SettingsDraft {
    pub fn new(settings: &Settings) -> Self {
        let mut fkeys = settings.fkeys.clone();
        fkeys.clamp_current_set();
        Self {
            page: Page::Monitor,
            monitor: settings.monitor_settings.clone(),
            outline_style: settings.font_outline_style,
            fkeys,
            slot: None,
        }
    }

    /// Restores the defaults of the current page; the paths page has nothing to reset.
    pub fn reset_page(&mut self) {
        match self.page {
            Page::Monitor => self.monitor = default_monitor(),
            Page::FontOutline => self.outline_style = 0,
            Page::Charset => {
                let set = self.fkeys.current_set();
                self.fkeys.reset_set(set);
                self.slot = None;
            }
            Page::Paths => {}
        }
    }

    fn page_is_default(&self) -> bool {
        match self.page {
            Page::Monitor => self.monitor == default_monitor(),
            Page::FontOutline => self.outline_style == 0,
            Page::Charset => self.fkeys.is_set_default(self.fkeys.current_set()),
            Page::Paths => true,
        }
    }

    pub fn apply(&self, settings: &mut Settings) {
        settings.monitor_settings = self.monitor.clone();
        settings.font_outline_style = self.outline_style;
        settings.fkeys = self.fkeys.clone();
    }
}

/// The drawing defaults: a fixed 2× zoom rather than the viewer's fit-to-window.
fn default_monitor() -> MonitorSettings {
    MonitorSettings {
        scaling_mode: ScalingMode::Manual(2.0),
        ..Default::default()
    }
}

impl DrawApp {
    pub(super) fn open_settings(&mut self) {
        self.dialog = Some(super::Dialog::Settings(Box::new(SettingsDraft::new(&self.settings))));
    }

    /// Returns true while the dialog stays open.
    pub(super) fn settings_dialog(&mut self, context: &egui::Context, draft: &mut SettingsDraft) -> bool {
        #[derive(Clone, Copy)]
        enum Action {
            Reset,
            Cancel,
            Apply,
        }
        let font = self
            .document
            .with_state(|state| state.get_buffer().font(state.get_caret().attribute.font_page()).cloned())
            .unwrap_or_default();
        let pages = [
            (Page::Monitor, fl!("settings-monitor-category")),
            (Page::FontOutline, fl!("settings-font-outline-category")),
            (Page::Charset, fl!("settings-char-set-category")),
            (Page::Paths, fl!("settings-paths-category")),
        ];
        let mut open_path = None;
        let response = appearance::Dialog::new("settings")
            .title(fl!("settings-heading"))
            .size(DialogSize::XLarge)
            .fixed_height(600.0)
            .show(context, |dialog| {
                dialog.tabs(&mut draft.page, &pages);
                dialog.content(|ui| match draft.page {
                    Page::Monitor => icy_engine_gui::egui::monitor::fields(ui, &mut draft.monitor),
                    Page::FontOutline => {
                        appearance::section(ui, &fl!("settings-font-outline-header"));
                        widgets::outline_style_grid(ui, &mut draft.outline_style);
                    }
                    Page::Charset => charset_page(ui, draft, &font),
                    Page::Paths => open_path = paths_page(ui),
                });
                let reset = DialogButton::secondary(labels::restore_defaults(), Action::Reset)
                    .leading()
                    .enabled(!draft.page_is_default());
                let mut buttons = Vec::new();
                if draft.page != Page::Paths {
                    buttons.push(reset);
                }
                buttons.push(DialogButton::cancel(labels::cancel(), Action::Cancel));
                buttons.push(DialogButton::primary(labels::ok(), Action::Apply));
                dialog.buttons(buttons);
            });
        if let Some(path) = open_path {
            let result = open::that(&path).map_err(|error| error.to_string());
            self.result(result);
        }
        match response.action {
            Some(Action::Reset) => {
                draft.reset_page();
                true
            }
            Some(Action::Apply) => {
                draft.apply(&mut self.settings);
                if self.persist_settings {
                    self.settings.store_persistent();
                    let result = self.settings.fkeys.save().map_err(|error| error.to_string());
                    self.result(result);
                }
                self.canvas_focus = true;
                false
            }
            Some(Action::Cancel) => {
                self.canvas_focus = true;
                false
            }
            None if response.dismissed => {
                self.canvas_focus = true;
                false
            }
            None => true,
        }
    }
}

/// Set selector, the twelve F-key slots and the character grid; a click in the grid
/// assigns the character to the selected slot.
fn charset_page(ui: &mut egui::Ui, draft: &mut SettingsDraft, font: &icy_engine::BitFont) {
    appearance::section(ui, &fl!("settings-charset-header"));
    let count = draft.fkeys.set_count();
    let set = draft.fkeys.current_set();
    ui.horizontal(|ui| {
        if ui.add_enabled(set > 0, egui::Button::new("◀")).clicked() {
            draft.fkeys.current_set = set - 1;
            draft.slot = None;
        }
        ui.label(format!("{} {} / {count}", fl!("settings-charset-set"), set + 1));
        if ui.add_enabled(set + 1 < count, egui::Button::new("▶")).clicked() {
            draft.fkeys.current_set = set + 1;
            draft.slot = None;
        }
    });
    draft.fkeys.clamp_current_set();
    let set = draft.fkeys.current_set();
    ui.add_space(8.0);
    let slot_size = ((ui.available_width() - 11.0 * 6.0) / 12.0).min(40.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        for slot in 0..12 {
            ui.vertical(|ui| {
                ui.set_width(slot_size);
                ui.vertical_centered(|ui| ui.weak(format!("F{}", slot + 1)));
                let code = char::from_u32(u32::from(draft.fkeys.code_at(set, slot))).unwrap_or(' ');
                if widgets::glyph(ui, font, code, draft.slot == Some(slot), slot_size).clicked() {
                    draft.slot = Some(slot);
                }
            });
        }
    });
    ui.add_space(12.0);
    let selected = draft.slot.map(|slot| u32::from(draft.fkeys.code_at(set, slot)));
    let size = ((ui.available_width() - 30.0) / 16.0).min(26.0);
    ui.add_enabled_ui(draft.slot.is_some(), |ui| {
        ui.spacing_mut().item_spacing = egui::Vec2::splat(2.0);
        for row in 0..16u32 {
            ui.horizontal(|ui| {
                for column in 0..16u32 {
                    let code = row * 16 + column;
                    let character = char::from_u32(code).unwrap_or(' ');
                    if widgets::glyph(ui, font, character, selected == Some(code), size).clicked() {
                        if let Some(slot) = draft.slot {
                            draft.fkeys.set_code_at(set, slot, code as u16);
                        }
                    }
                }
            });
        }
    });
}

/// Read-only application paths with buttons to open them; returns the path to open.
fn paths_page(ui: &mut egui::Ui) -> Option<PathBuf> {
    appearance::section(ui, &fl!("settings-paths-header"));
    let mut open = None;
    let rows: [(String, Option<PathBuf>, bool); 6] = [
        (fl!("settings-paths-config-dir"), Settings::config_dir(), true),
        (fl!("settings-paths-config-file"), Settings::config_file(), false),
        (fl!("settings-paths-log-file"), Settings::log_file(), true),
        (fl!("settings-paths-font-dir"), Settings::text_art_font_dir(), true),
        (fl!("settings-paths-plugin-dir"), Settings::plugin_dir(), true),
        (fl!("settings-paths-taglists-dir"), Settings::taglists_dir(), true),
    ];
    for (label, path, openable) in rows {
        appearance::form_row(ui, label.trim_end_matches(':'), |ui| {
            let mut shown = path.as_ref().map_or_else(|| "N/A".to_string(), |path| path.display().to_string());
            let button_width = if openable { 90.0 } else { 0.0 };
            ui.add(
                appearance::text_edit(&mut shown)
                    .interactive(false)
                    .desired_width((ui.available_width() - button_width).max(80.0)),
            );
            if openable && ui.add_enabled(path.is_some(), egui::Button::new(fl!("settings-paths-open"))).clicked() {
                open = path.map(|path| openable_target(&path));
            }
        });
    }
    open
}

/// Folders are created on demand; a file that does not exist yet opens its folder instead.
fn openable_target(path: &std::path::Path) -> PathBuf {
    if path.extension().is_none() {
        let _ = std::fs::create_dir_all(path);
        return path.to_path_buf();
    }
    if path.exists() {
        path.to_path_buf()
    } else {
        path.parent().map_or_else(|| path.to_path_buf(), std::path::Path::to_path_buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draft_applies_and_resets_per_page() {
        let mut settings = Settings::load();
        let mut draft = SettingsDraft::new(&settings);
        draft.outline_style = 5;
        draft.monitor.use_scanlines = !draft.monitor.use_scanlines;
        let set = draft.fkeys.current_set();
        let original = draft.fkeys.code_at(set, 0);
        draft.fkeys.set_code_at(set, 0, 65);
        draft.apply(&mut settings);
        assert_eq!(settings.font_outline_style, 5);
        assert_eq!(settings.fkeys.code_at(set, 0), 65);
        assert_eq!(settings.monitor_settings, draft.monitor);

        draft.page = Page::FontOutline;
        assert!(!draft.page_is_default());
        draft.reset_page();
        assert_eq!(draft.outline_style, 0);
        draft.page = Page::Charset;
        draft.reset_page();
        assert_eq!(draft.fkeys.code_at(set, 0), original);
        draft.page = Page::Monitor;
        draft.reset_page();
        assert_eq!(draft.monitor, default_monitor());
        draft.page = Page::Paths;
        assert!(draft.page_is_default());
    }

    #[test]
    fn missing_files_open_their_folder() {
        let directory = tempfile::tempdir().unwrap();
        let log = directory.path().join("icy_draw.log");
        assert_eq!(openable_target(&log), directory.path());
        std::fs::write(&log, b"").unwrap();
        assert_eq!(openable_target(&log), log);
        let folder = directory.path().join("data/plugins");
        assert_eq!(openable_target(&folder), folder);
        assert!(folder.is_dir());
    }
}
