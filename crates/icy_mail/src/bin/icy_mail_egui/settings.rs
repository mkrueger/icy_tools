use eframe::egui;
use i18n_embed_fl::fl;
use icy_engine_gui::{
    egui::{appearance, monitor},
    ScalingMode,
};
use icy_mail::{
    options::{ModernFont, Options, ReadingMode, ReadingPane, Theme, MODERN_FONT_SIZES},
    writing::validate_quote_header,
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

pub fn theme_name(theme: Theme) -> String {
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

fn reading_mode_name(mode: ReadingMode) -> String {
    match mode {
        ReadingMode::Classic => fl!(LANGUAGE_LOADER, "settings-reading-mode-classic"),
        ReadingMode::Modern => fl!(LANGUAGE_LOADER, "settings-reading-mode-modern"),
    }
}

fn modern_font_name(font: ModernFont) -> String {
    match font {
        ModernFont::Proportional => fl!(LANGUAGE_LOADER, "settings-modern-font-proportional"),
        ModernFont::Monospace => fl!(LANGUAGE_LOADER, "settings-modern-font-monospace"),
    }
}

pub const READING_PANES: [ReadingPane; 3] = [ReadingPane::Automatic, ReadingPane::Below, ReadingPane::Right];

pub fn reading_pane_name(pane: ReadingPane) -> String {
    match pane {
        ReadingPane::Automatic => fl!(LANGUAGE_LOADER, "settings-reading-pane-automatic"),
        ReadingPane::Below => fl!(LANGUAGE_LOADER, "settings-reading-pane-below"),
        ReadingPane::Right => fl!(LANGUAGE_LOADER, "settings-reading-pane-right"),
    }
}

pub fn theme_preference(theme: Theme) -> egui::ThemePreference {
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
            signature: self.signature.clone(),
            quote_header: self.quote_header.clone(),
            view_mode: self.reader.view_mode,
            reading_pane: self.reading_pane,
            conferences_unread_only: self.conferences_unread_only,
            reading_mode: self.reading_mode,
            modern_font: self.modern_font,
            modern_font_size: self.modern_font_size,
            search_fields: self.reader.search_fields,
            extraction_cache_days: self.extraction_cache_days,
        }
    }

    pub fn apply_options(&mut self, context: &egui::Context, options: &Options) {
        self.settings = options.monitor_settings.clone();
        self.random_tagline = options.random_tagline;
        self.signature = options.signature.clone();
        if validate_quote_header(&options.quote_header).is_ok() {
            self.quote_header = options.quote_header.clone();
        }
        self.reading_pane = options.reading_pane;
        self.conferences_unread_only = options.conferences_unread_only;
        self.reading_mode = options.reading_mode;
        self.modern_font = options.modern_font;
        self.modern_font_size = options.modern_font_size.clamp(*MODERN_FONT_SIZES.start(), *MODERN_FONT_SIZES.end());
        self.extraction_cache_days = options.extraction_cache_days;
        if self.reader.search_fields != options.search_fields && options.search_fields.any() {
            self.set_search_fields(options.search_fields);
        }
        if self.reader.view_mode != options.view_mode {
            self.set_mode(options.view_mode);
        }
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
        if let Some(after) = self.options_save_after {
            let remaining = after - context.input(|input| input.time);
            if remaining > 0.0 {
                context.request_repaint_after(std::time::Duration::from_secs_f64(remaining));
                return;
            }
            self.options_save_after = None;
        }
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
            (2, fl!(LANGUAGE_LOADER, "settings-section-cache")),
        ];
        let response = appearance::Dialog::new("mail-settings")
            .title(fl!(LANGUAGE_LOADER, "settings-title"))
            .size(appearance::DialogSize::XLarge)
            .fixed_height(560.0)
            .show(context, |frame| {
                frame.tabs(&mut dialog.page, &pages);
                frame.content(|ui| {
                    match dialog.page {
                        0 => general(ui, &mut dialog.draft),
                        1 => monitor::fields(ui, &mut dialog.draft.monitor_settings),
                        _ => cache(ui, &mut dialog.draft),
                    }
                    if let Err(error) = validate_quote_header(&dialog.draft.quote_header) {
                        ui.colored_label(
                            ui.visuals().error_fg_color,
                            fl!(LANGUAGE_LOADER, "settings-quote-header-invalid", error = error.to_string()),
                        );
                    }
                });
                frame.buttons([
                    appearance::DialogButton::secondary(appearance::labels::restore_defaults(), Footer::Restore).leading(),
                    appearance::DialogButton::cancel(appearance::labels::cancel(), Footer::Cancel),
                    appearance::DialogButton::primary(appearance::labels::ok(), Footer::Save)
                        .enabled(validate_quote_header(&dialog.draft.quote_header).is_ok()),
                ]);
            });
        let defaults = Options::default();
        let mut close = None;
        match response.action {
            Some(Footer::Restore) if dialog.page == 0 => {
                dialog.draft.theme = defaults.theme;
                dialog.draft.monitor_settings.scaling_mode = defaults.monitor_settings.scaling_mode;
                dialog.draft.random_tagline = defaults.random_tagline;
                dialog.draft.signature = defaults.signature;
                dialog.draft.quote_header = defaults.quote_header;
                dialog.draft.reading_pane = defaults.reading_pane;
                dialog.draft.reading_mode = defaults.reading_mode;
                dialog.draft.modern_font = defaults.modern_font;
                dialog.draft.modern_font_size = defaults.modern_font_size;
            }
            Some(Footer::Restore) if dialog.page == 2 => dialog.draft.extraction_cache_days = defaults.extraction_cache_days,
            Some(Footer::Restore) => {
                let zoom = dialog.draft.monitor_settings.scaling_mode;
                dialog.draft.monitor_settings = defaults.monitor_settings;
                dialog.draft.monitor_settings.scaling_mode = zoom;
            }
            Some(Footer::Save) => {
                if validate_quote_header(&dialog.draft.quote_header).is_ok() {
                    close = Some(true);
                }
            }
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
        let mode = &mut options.reading_mode;
        appearance::combo_row(ui, &fl!(LANGUAGE_LOADER, "settings-reading-mode-label"), reading_mode_name(*mode), |ui| {
            for choice in [ReadingMode::Classic, ReadingMode::Modern] {
                ui.selectable_value(mode, choice, reading_mode_name(choice));
            }
        });
        let modern = options.reading_mode == ReadingMode::Modern;
        ui.add_enabled_ui(modern, |ui| {
            let font = &mut options.modern_font;
            appearance::combo_row(ui, &fl!(LANGUAGE_LOADER, "settings-modern-font-label"), modern_font_name(*font), |ui| {
                for choice in [ModernFont::Proportional, ModernFont::Monospace] {
                    ui.selectable_value(font, choice, modern_font_name(choice));
                }
            });
            appearance::slider_row(
                ui,
                &fl!(LANGUAGE_LOADER, "settings-modern-font-size-label"),
                &mut options.modern_font_size,
                MODERN_FONT_SIZES,
            );
            options.modern_font_size = options.modern_font_size.round();
        });
        let zoom = &mut options.monitor_settings.scaling_mode;
        appearance::combo_row(ui, &fl!(LANGUAGE_LOADER, "settings-zoom-label"), zoom_name(*zoom), |ui| {
            for mode in ZOOMS {
                ui.selectable_value(zoom, mode, zoom_name(mode));
            }
        });
        let pane = &mut options.reading_pane;
        appearance::combo_row(ui, &fl!(LANGUAGE_LOADER, "settings-reading-pane-label"), reading_pane_name(*pane), |ui| {
            for choice in READING_PANES {
                ui.selectable_value(pane, choice, reading_pane_name(choice));
            }
        });
    });
    appearance::group(ui, &fl!(LANGUAGE_LOADER, "settings-section-writing"), |ui| {
        appearance::check_row(ui, &fl!(LANGUAGE_LOADER, "settings-add-random-tagline"), &mut options.random_tagline);
        ui.label(fl!(LANGUAGE_LOADER, "settings-signature"));
        ui.add(
            egui::TextEdit::multiline(&mut options.signature)
                .id_salt("settings-signature")
                .desired_rows(2)
                .desired_width(f32::INFINITY),
        )
        .on_hover_text(fl!(LANGUAGE_LOADER, "settings-signature-hint"));
        ui.label(fl!(LANGUAGE_LOADER, "settings-quote-header"));
        ui.add(
            appearance::text_edit(&mut options.quote_header)
                .id_salt("settings-quote-header")
                .desired_width(f32::INFINITY),
        );
        ui.label(egui::RichText::new(fl!(LANGUAGE_LOADER, "settings-quote-header-hint")).weak().small());
    });
}

fn cache(ui: &mut egui::Ui, options: &mut Options) {
    appearance::group(ui, &fl!(LANGUAGE_LOADER, "settings-section-cache"), |ui| {
        ui.horizontal(|ui| {
            ui.label(fl!(LANGUAGE_LOADER, "settings-extraction-cache-days"));
            ui.add(egui::DragValue::new(&mut options.extraction_cache_days).range(0..=u32::MAX));
        });
        ui.label(fl!(LANGUAGE_LOADER, "settings-extraction-cache-help"));
    });
}
