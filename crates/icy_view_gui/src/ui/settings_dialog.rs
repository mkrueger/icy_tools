use eframe::{
    egui::{self, Layout, RichText},
    epaint::Vec2,
};
use egui::global_theme_preference_switch;
use i18n_embed_fl::fl;
use icy_engine_gui::{show_monitor_settings, MonitorSettings};

use super::settings::{Settings, SETTINGS};

pub struct SettingsDialog {
    settings_category: usize,

    monitor_settings: MonitorSettings,
    pub is_dark_mode: Option<bool>,
}
const MONITOR_CAT: usize = 0;

impl SettingsDialog {
    pub fn new() -> Self {
        Self {
            settings_category: MONITOR_CAT,
            is_dark_mode: unsafe { SETTINGS.is_dark_mode },
            monitor_settings: unsafe { SETTINGS.monitor_settings.clone() },
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> bool {
        let mut open = true;
        let mut dialog_open = true;
        let title = RichText::new(fl!(crate::LANGUAGE_LOADER, "settings-heading"));
        if ctx.input(|i| i.key_down(egui::Key::Escape)) {
            open = false;
        }

        egui::Window::new(title)
            .open(&mut open)
            .collapsible(false)
            .fixed_size(Vec2::new(400., 300.))
            .resizable(false)
            .frame(egui::Frame::window(&ctx.style()))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    global_theme_preference_switch(ui);
                    self.is_dark_mode = Some(ui.visuals().dark_mode);

                    let settings_category = self.settings_category;

                    if ui
                        .selectable_label(settings_category == MONITOR_CAT, fl!(crate::LANGUAGE_LOADER, "settings-monitor-category"))
                        .clicked()
                    {
                        self.settings_category = MONITOR_CAT;
                    }
                });
                ui.separator();
                match self.settings_category {
                    MONITOR_CAT => unsafe {
                        if let Some(new_settings) = show_monitor_settings(ui, &SETTINGS.monitor_settings) {
                            SETTINGS.monitor_settings = new_settings;
                        }
                    },

                    _ => {}
                }

                ui.separator();
                ui.add_space(4.0);
                ui.with_layout(Layout::right_to_left(egui::Align::TOP), |ui| {
                    if ui.button(fl!(crate::LANGUAGE_LOADER, "button-ok")).clicked() {
                        unsafe {
                            SETTINGS.is_dark_mode = self.is_dark_mode;
                            if let Err(err) = Settings::save() {
                                log::error!("Error saving settings: {err}");
                            }
                        }
                        dialog_open = false;
                    }

                    if ui.button(fl!(crate::LANGUAGE_LOADER, "button-cancel")).clicked() {
                        unsafe {
                            SETTINGS.monitor_settings = self.monitor_settings.clone();
                            if let Some(dark_mode) = SETTINGS.is_dark_mode {
                                ui.visuals_mut().dark_mode = dark_mode;
                            }
                        }
                        dialog_open = false;
                    }

                    if ui.button(fl!(crate::LANGUAGE_LOADER, "settings-reset_button")).clicked() {
                        unsafe {
                            match self.settings_category {
                                MONITOR_CAT => SETTINGS.monitor_settings = Default::default(),
                                _ => {}
                            }
                        }
                    }
                });
            });

        open && dialog_open
    }
}
