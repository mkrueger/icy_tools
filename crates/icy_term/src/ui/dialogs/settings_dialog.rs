use eframe::egui::{self, RichText};
use egui::{Layout, TextEdit, Vec2};
use i18n_embed_fl::fl;
use icy_engine_gui::show_monitor_settings;
use icy_net::serial::{CharSize, Parity, StopBits};

use crate::{
    ui::{MainWindowMode, MainWindowState},
    KeyBindings, Modem,
};

#[derive(Default)]
pub struct DialogState {
    pub settings_category: usize,
}

#[derive(Clone, Debug)]
pub(crate) enum Message {
    SwitchSettingsCategory(usize),
    CloseDialog,
    OpenSettingsFolder,
    ResetMonitorSettings,
    ResetKeybindSettings,
    UpdateIEMSI(crate::IEMSISettings),
    UpdateModem(Modem),
    UpdateMonitorSettings(icy_engine_gui::MonitorSettings),
    // ChangeOpenglScaling(Scaling),
    UpdateKeybinds(KeyBindings),
    ChangeConsoleBeep(bool),
}

type ShowSettingsCallback = fn(&MainWindowState, ui: &mut egui::Ui) -> Option<Message>;
type ResetMessage = Option<Message>;

lazy_static::lazy_static! {
    static ref SETTING_CATEGORIES: [(String, ShowSettingsCallback, ResetMessage); 5] = [
        (
            fl!(crate::LANGUAGE_LOADER, "settings-monitor-category"),
            show_monitor_settings2,
            Some(Message::ResetMonitorSettings)
        ),
        (
            fl!(crate::LANGUAGE_LOADER, "settings-iemsi-category"),
            show_iemsi_settings,
            None
        ),
        (
            fl!(crate::LANGUAGE_LOADER, "settings-terminal-category"),
            show_terminal_settings,
            None
        ),
        (
            fl!(crate::LANGUAGE_LOADER, "settings-keybinds-category"),
            crate::show_keybinds_settings,
            Some(Message::ResetKeybindSettings)
        ),
        (
            fl!(crate::LANGUAGE_LOADER, "settings-modem-category"),
            show_modem_settings,
            None
        ),
    ];
}

impl MainWindowState {
    pub fn show_settings(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut result = None;

        let mut open = true;
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
                    let old_dark = self.options.is_dark_mode;
                    egui::global_theme_preference_switch(ui);
                    self.options.is_dark_mode = Some(ui.visuals().dark_mode);
                    if self.options.is_dark_mode != old_dark {
                        self.store_options();
                    }
                    let settings_category = self.settings_dialog.settings_category;
                    for i in 0..SETTING_CATEGORIES.len() {
                        if ui.selectable_label(settings_category == i, &SETTING_CATEGORIES[i].0).clicked() {
                            result = Some(Message::SwitchSettingsCategory(i));
                        }
                    }
                });
                ui.separator();
                let settings_category = self.settings_dialog.settings_category;
                if let Some(cat) = SETTING_CATEGORIES.get(settings_category) {
                    if let Some(cmd) = cat.1(self, ui) {
                        result = Some(cmd);
                    }
                } else {
                    ui.colored_label(ui.style().visuals.error_fg_color, "Invalid settings category");
                }

                ui.separator();
                ui.add_space(4.0);
                ui.with_layout(Layout::right_to_left(egui::Align::TOP), |ui| {
                    if ui.button(fl!(crate::LANGUAGE_LOADER, "dialing_directory-ok-button")).clicked() {
                        result = Some(Message::CloseDialog);
                    }

                    let settings_category = self.settings_dialog.settings_category;
                    if let Some(cat) = SETTING_CATEGORIES.get(settings_category) {
                        if let Some(reset_cmd) = &cat.2 {
                            if ui.button(fl!(crate::LANGUAGE_LOADER, "settings-reset-button")).clicked() {
                                result = Some(reset_cmd.clone());
                            }
                        }
                    }
                });
            });

        if !open {
            result = Some(Message::CloseDialog);
        }

        update_state(self, result);
    }
}

fn show_iemsi_settings(state: &MainWindowState, ui: &mut egui::Ui) -> Option<Message> {
    let mut iemsi = state.options.iemsi.clone();
    ui.checkbox(&mut iemsi.autologin, fl!(crate::LANGUAGE_LOADER, "settings-iemsi-autologin-checkbox"));

    egui::Grid::new("some_unique_id")
        .num_columns(2)
        .spacing([4.0, 8.0])
        .min_row_height(24.)
        .show(ui, |ui| {
            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "settings-iemsi-alias")));
            });
            ui.add(TextEdit::singleline(&mut iemsi.alias));
            ui.end_row();

            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "settings-iemsi-location")));
            });
            ui.add(TextEdit::singleline(&mut iemsi.location));
            ui.end_row();

            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "settings-iemsi-data-phone")));
            });
            ui.add(TextEdit::singleline(&mut iemsi.data_phone));
            ui.end_row();

            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "settings-iemsi-voice-phone")));
            });
            ui.add(TextEdit::singleline(&mut iemsi.voice_phone));
            ui.end_row();

            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "settings-iemsi-birth-date")));
            });
            ui.add(TextEdit::singleline(&mut iemsi.birth_date));
            ui.end_row();
        });

    if iemsi == state.options.iemsi {
        None
    } else {
        Some(Message::UpdateIEMSI(iemsi))
    }
}

fn show_terminal_settings(state: &MainWindowState, ui: &mut egui::Ui) -> Option<Message> {
    let mut result = None;
    let mut beep = state.options.console_beep;

    if ui
        .checkbox(&mut beep, fl!(crate::LANGUAGE_LOADER, "settings-terminal-console-beep-checkbox"))
        .changed()
    {
        result = Some(Message::ChangeConsoleBeep(beep));
    }

    ui.add_space(16.0);
    if ui.button(fl!(crate::LANGUAGE_LOADER, "settings-terminal-open-settings-dir-button")).clicked() {
        result = Some(Message::OpenSettingsFolder);
    }
    ui.add_space(8.0);

    result
}

fn show_monitor_settings2(state: &MainWindowState, ui: &mut egui::Ui) -> Option<Message> {
    let mut result = None;

    let monitor_settings = state.options.monitor_settings.clone();
    if let Some(settings) = show_monitor_settings(ui, &monitor_settings) {
        result = Some(Message::UpdateMonitorSettings(settings));
    }

    result
}

fn update_state(state: &mut MainWindowState, message_opt: Option<Message>) {
    match message_opt {
        Some(Message::CloseDialog) => {
            state.mode = MainWindowMode::ShowTerminal;
        }
        Some(Message::SwitchSettingsCategory(category)) => {
            state.settings_dialog.settings_category = category;
        }
        Some(Message::ResetMonitorSettings) => {
            state.options.reset_monitor_settings();
            state.store_options();
        }
        Some(Message::ResetKeybindSettings) => {
            state.options.reset_keybindings();
            state.store_options();
        }
        Some(Message::OpenSettingsFolder) => {
            if let Some(proj_dirs) = directories::ProjectDirs::from("com", "GitHub", "icy_term") {
                open::that(proj_dirs.config_dir()).unwrap();
            }
        }
        Some(Message::UpdateIEMSI(iemsi)) => {
            state.options.iemsi = iemsi;
            state.store_options();
        }
        Some(Message::UpdateModem(modem)) => {
            state.options.modem = modem;
            state.store_options();
        }
        Some(Message::UpdateMonitorSettings(monitor_settings)) => {
            state.options.monitor_settings = monitor_settings;
            state.store_options();
        } /*
        Some(Message::ChangeOpenglScaling(scaling)) => {
        state.options.scaling = scaling;
        state.store_options();
        }*/
        Some(Message::UpdateKeybinds(keybinds)) => {
            state.options.bind = keybinds;
            state.store_options();
        }
        Some(Message::ChangeConsoleBeep(beep)) => {
            state.options.console_beep = beep;
            state.store_options();
        }
        _ => {}
    }
}

fn show_modem_settings(state: &MainWindowState, ui: &mut egui::Ui) -> Option<Message> {
    let mut modem = state.options.modem.clone();

    egui::Grid::new("some_unique_id")
        .num_columns(2)
        .spacing([4.0, 8.0])
        .min_row_height(24.)
        .show(ui, |ui| {
            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "settings-modem-device")));
            });
            ui.add(TextEdit::singleline(&mut modem.device));
            ui.end_row();

            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "settings-modem-baud_rate")));
            });
            let mut baud = modem.baud_rate.to_string();
            ui.add(TextEdit::singleline(&mut baud));
            if let Ok(baud) = baud.parse() {
                modem.baud_rate = baud;
            }
            ui.end_row();

            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui: &mut egui::Ui| {
                ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "settings-modem-data_bits")));
            });

            ui.horizontal(|ui| {
                let txt = match modem.char_size {
                    CharSize::Bits5 => "5",
                    CharSize::Bits6 => "6",
                    CharSize::Bits7 => "7",
                    CharSize::Bits8 => "8",
                };
                egui::ComboBox::from_id_salt("combobox1").selected_text(RichText::new(txt)).show_ui(ui, |ui| {
                    ui.selectable_value(&mut modem.char_size, CharSize::Bits5, "5");
                    ui.selectable_value(&mut modem.char_size, CharSize::Bits6, "6");
                    ui.selectable_value(&mut modem.char_size, CharSize::Bits7, "7");
                    ui.selectable_value(&mut modem.char_size, CharSize::Bits8, "8");
                });

                let txt = match modem.stop_bits {
                    StopBits::One => "1",
                    StopBits::Two => "2",
                };
                egui::ComboBox::from_id_salt("combobox2").selected_text(RichText::new(txt)).show_ui(ui, |ui| {
                    ui.selectable_value(&mut modem.stop_bits, StopBits::One, "1");
                    ui.selectable_value(&mut modem.stop_bits, StopBits::Two, "2");
                });
                let txt = match modem.parity {
                    Parity::None => "None",
                    Parity::Odd => "Odd",
                    Parity::Even => "Even",
                };
                egui::ComboBox::from_id_salt("combobox3").selected_text(RichText::new(txt)).show_ui(ui, |ui| {
                    ui.selectable_value(&mut modem.parity, Parity::None, "None");
                    ui.selectable_value(&mut modem.parity, Parity::Odd, "Odd");
                    ui.selectable_value(&mut modem.parity, Parity::Even, "Even");
                });
            });
            ui.end_row();

            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "settings-modem-init_string")));
            });
            ui.add(TextEdit::singleline(&mut modem.init_string));
            ui.end_row();

            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(fl!(crate::LANGUAGE_LOADER, "settings-modem_dial_string")));
            });
            ui.add(TextEdit::singleline(&mut modem.dial_string));
            ui.end_row();
        });

    if modem == state.options.modem {
        None
    } else {
        Some(Message::UpdateModem(modem))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::field_reassign_with_default)]
    use icy_engine_gui::MonitorSettings;

    use crate::{
        ui::{
            dialogs::settings_dialog::{update_state, SETTING_CATEGORIES},
            MainWindowState,
        },
        IEMSISettings, KeyBindings, Options, Scaling,
    };

    #[test]
    fn test_close_dialog() {
        let mut state: MainWindowState = MainWindowState::default();
        state.mode = super::MainWindowMode::ShowSettings;
        update_state(&mut state, Some(super::Message::CloseDialog));
        assert!(matches!(state.mode, super::MainWindowMode::ShowTerminal));
        assert!(!state.options_written);
    }

    #[test]
    fn test_switch_category_dialog() {
        let mut state: MainWindowState = MainWindowState::default();
        for i in 0..SETTING_CATEGORIES.len() {
            update_state(&mut state, Some(super::Message::SwitchSettingsCategory(i)));
            assert_eq!(i, state.settings_dialog.settings_category);
        }
        assert!(!state.options_written);
    }

    #[test]
    fn test_reset_monitor_settings() {
        let mut state: MainWindowState = MainWindowState::default();
        let mut opt: Options = Options::default();
        opt.scaling = Scaling::Linear;
        opt.monitor_settings.blur = 0.0;
        opt.monitor_settings.brightness = 1.0;
        state.options = opt;

        assert_ne!(Options::default().scaling, state.options.scaling);
        assert_ne!(Options::default().monitor_settings, state.options.monitor_settings);
        update_state(&mut state, Some(super::Message::ResetMonitorSettings));
        assert_eq!(Options::default().scaling, state.options.scaling);
        assert_eq!(Options::default().monitor_settings, state.options.monitor_settings);
        assert!(state.options_written);
    }

    #[test]
    fn test_reset_keybindings() {
        let mut state: MainWindowState = MainWindowState::default();
        let mut opt = Options::default();
        opt.bind.download = None;
        opt.bind.full_screen = None;
        state.options = opt;

        assert_ne!(Options::default().bind, state.options.bind);
        update_state(&mut state, Some(super::Message::ResetKeybindSettings));
        assert_eq!(Options::default().bind, state.options.bind);
        assert!(state.options_written);
    }

    #[test]
    fn test_change_console_beep() {
        let mut state: MainWindowState = MainWindowState::default();
        update_state(&mut state, Some(super::Message::ChangeConsoleBeep(false)));
        assert_ne!(Options::default().console_beep, state.options.console_beep);
        assert!(state.options_written);
    }

    #[test]
    fn test_set_keybindings() {
        let mut state: MainWindowState = MainWindowState::default();
        let mut bind = KeyBindings::default();
        bind.download = None;
        bind.full_screen = None;
        update_state(&mut state, Some(super::Message::UpdateKeybinds(bind)));
        assert_ne!(KeyBindings::default(), state.options.bind);
        assert!(state.options_written);
    }

    #[test]
    fn test_set_monitor_settings() {
        let mut state: MainWindowState = MainWindowState::default();
        let mut settings = MonitorSettings::default();
        settings.blur = 0.0;
        settings.brightness = 1.0;
        update_state(&mut state, Some(super::Message::UpdateMonitorSettings(settings)));
        assert_ne!(MonitorSettings::default(), state.options.monitor_settings);
        assert!(state.options_written);
    }

    #[test]
    fn test_set_iemsi_settings() {
        let mut state: MainWindowState = MainWindowState::default();
        let mut settings = IEMSISettings::default();
        settings.alias = "foo".to_string();
        settings.voice_phone = "42".to_string();
        update_state(&mut state, Some(super::Message::UpdateIEMSI(settings)));
        assert_ne!(IEMSISettings::default(), state.options.iemsi);
        assert!(state.options_written);
    }
}
