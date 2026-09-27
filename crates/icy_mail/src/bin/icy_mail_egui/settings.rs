use eframe::egui;
use i18n_embed_fl::fl;
use icy_engine_gui::{
    egui::{appearance, monitor},
    ScalingMode,
};
use icy_mail::{
    options::{Options, Theme},
    LANGUAGE_LOADER,
};

use super::app::MailApp;

/// The settings dialog previews its changes live and restores `original` when cancelled.
pub struct SettingsDialog {
    original: Options,
    draft: Options,
    page: usize,
}

pub const ZOOMS: [ScalingMode; 4] = [
    ScalingMode::FitWidth,
    ScalingMode::Manual(1.0),
    ScalingMode::Manual(1.5),
    ScalingMode::Manual(2.0),
];

fn theme_name(theme: Theme) -> String {
    match theme {
        Theme::System => fl!(LANGUAGE_LOADER, "settings-theme-follow-system"),
        Theme::Light => fl!(LANGUAGE_LOADER, "settings-theme-light"),
        Theme::Dark => fl!(LANGUAGE_LOADER, "settings-theme-dark"),
    }
}

pub fn zoom_name(mode: ScalingMode) -> String {
    match mode {
        ScalingMode::FitWidth => fl!(LANGUAGE_LOADER, "settings-zoom-fit-width"),
        ScalingMode::Manual(zoom) => format!("{:.0}%", zoom * 100.0),
        _ => fl!(LANGUAGE_LOADER, "settings-zoom-fit"),
    }
}

fn theme_preference(theme: Theme) -> egui::ThemePreference {
    match theme {
        Theme::System => egui::ThemePreference::System,
        Theme::Light => egui::ThemePreference::Light,
        Theme::Dark => egui::ThemePreference::Dark,
    }
}

impl MailApp {
    /// The settings as currently shown, including theme and zoom changed from the menu.
    pub fn current_options(&self, context: &egui::Context) -> Options {
        Options {
            theme: match context.options(|options| options.theme_preference) {
                egui::ThemePreference::System => Theme::System,
                egui::ThemePreference::Light => Theme::Light,
                egui::ThemePreference::Dark => Theme::Dark,
            },
            monitor_settings: self.settings.clone(),
            random_tagline: self.random_tagline,
        }
    }

    pub fn apply_options(&mut self, context: &egui::Context, options: &Options) {
        self.settings = options.monitor_settings.clone();
        self.random_tagline = options.random_tagline;
        if self.current_options(context).theme != options.theme {
            context.set_theme(theme_preference(options.theme));
        }
    }

    pub fn open_settings(&mut self, context: &egui::Context) {
        let current = self.current_options(context);
        self.settings_dialog = Some(SettingsDialog {
            original: current.clone(),
            draft: current,
            page: 0,
        });
        self.modal = Some(super::app::Modal::Settings);
    }

    /// Stores the settings when they changed, e.g. the zoom from the status bar.
    pub fn persist_options(&mut self, context: &egui::Context) {
        let current = self.current_options(context);
        if current == self.options {
            return;
        }
        self.options = current;
        let Some(directory) = self.options_directory() else {
            return;
        };
        if let Err(error) = self.options.save_in(&directory) {
            log::warn!("could not save the settings: {error}");
        }
    }

    /// Shows the settings dialog; returns whether it closed.
    pub fn settings(&mut self, context: &egui::Context) -> bool {
        let Some(dialog) = &mut self.settings_dialog else {
            return true;
        };
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum Footer {
            Restore,
            Cancel,
            Save,
        }
        let pages = [
            (0, fl!(LANGUAGE_LOADER, "settings-page-general")),
            (1, fl!(LANGUAGE_LOADER, "settings-page-monitor")),
        ];
        let response = appearance::Dialog::new("mail-settings")
            .title(fl!(LANGUAGE_LOADER, "settings-title"))
            .size(appearance::DialogSize::XLarge)
            .fixed_height(560.0)
            .show(context, |frame| {
                frame.tabs(&mut dialog.page, &pages);
                frame.content(|ui| match dialog.page {
                    0 => general(ui, &mut dialog.draft),
                    _ => monitor::fields(ui, &mut dialog.draft.monitor_settings),
                });
                frame.buttons([
                    appearance::DialogButton::secondary(appearance::labels::restore_defaults(), Footer::Restore).leading(),
                    appearance::DialogButton::cancel(appearance::labels::cancel(), Footer::Cancel),
                    appearance::DialogButton::primary(appearance::labels::ok(), Footer::Save),
                ]);
            });
        let defaults = Options::default();
        let mut close = None;
        match response.action {
            Some(Footer::Restore) if dialog.page == 0 => {
                dialog.draft.theme = defaults.theme;
                dialog.draft.monitor_settings.scaling_mode = defaults.monitor_settings.scaling_mode;
                dialog.draft.random_tagline = defaults.random_tagline;
            }
            Some(Footer::Restore) => {
                let zoom = dialog.draft.monitor_settings.scaling_mode;
                dialog.draft.monitor_settings = defaults.monitor_settings;
                dialog.draft.monitor_settings.scaling_mode = zoom;
            }
            Some(Footer::Save) => close = Some(true),
            Some(Footer::Cancel) => close = Some(false),
            None if response.dismissed => close = Some(false),
            None => {}
        }
        let options = match close {
            Some(false) => dialog.original.clone(),
            _ => dialog.draft.clone(),
        };
        self.apply_options(context, &options);
        if close.is_none() {
            return false;
        }
        self.settings_dialog = None;
        if close == Some(true) {
            self.persist_options(context);
        }
        true
    }
}

fn general(ui: &mut egui::Ui, options: &mut Options) {
    appearance::group(ui, &fl!(LANGUAGE_LOADER, "settings-section-appearance"), |ui| {
        appearance::combo_row(ui, &fl!(LANGUAGE_LOADER, "settings-theme-label"), theme_name(options.theme), |ui| {
            for theme in [Theme::System, Theme::Light, Theme::Dark] {
                ui.selectable_value(&mut options.theme, theme, theme_name(theme));
            }
        });
    });
    appearance::group(ui, &fl!(LANGUAGE_LOADER, "settings-section-messages"), |ui| {
        let zoom = &mut options.monitor_settings.scaling_mode;
        appearance::combo_row(ui, &fl!(LANGUAGE_LOADER, "settings-zoom-label"), zoom_name(*zoom), |ui| {
            for mode in ZOOMS {
                ui.selectable_value(zoom, mode, zoom_name(mode));
            }
        });
    });
    appearance::group(ui, &fl!(LANGUAGE_LOADER, "settings-section-writing"), |ui| {
        appearance::check_row(ui, &fl!(LANGUAGE_LOADER, "settings-add-random-tagline"), &mut options.random_tagline);
    });
}
