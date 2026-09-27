use super::appearance::{labels, Dialog, DialogButton, DialogSize};
use eframe::egui;
use icy_term::Options;
use std::{
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Page {
    #[default]
    Monitor,
    Terminal,
    Audio,
    Paths,
    Login,
    Serial,
    Sources,
    Modems,
    Protocols,
}

pub struct Settings {
    pub draft: Options,
    pub page: Page,
    pub error: Option<String>,
    path: PathBuf,
    baseline: Option<Vec<u8>>,
    pub closed: bool,
    commands: std::collections::HashMap<egui::Id, String>,
    invalid_commands: std::collections::HashSet<egui::Id>,
    #[cfg(test)]
    pub bounds: Option<egui::Rect>,
}

fn read_existing(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

impl Settings {
    pub fn open(options: &Options, path: PathBuf) -> Result<Self, String> {
        let baseline = read_existing(&path)?;
        let mut draft = if let Some(bytes) = &baseline {
            toml::from_str(std::str::from_utf8(bytes).map_err(|error| error.to_string())?).map_err(|error| error.to_string())?
        } else {
            options.clone()
        };
        draft.monitor_settings = options.monitor_settings.clone();
        Ok(Self {
            draft,
            page: Page::default(),
            error: None,
            path,
            baseline,
            closed: false,
            commands: Default::default(),
            invalid_commands: Default::default(),
            #[cfg(test)]
            bounds: None,
        })
    }

    pub fn save(&mut self) -> Result<Options, String> {
        if !self.invalid_commands.is_empty() {
            return Err("Correct invalid control sequences before saving".into());
        }
        if read_existing(&self.path)? != self.baseline {
            return Err("Settings changed on disk. Close and reopen Settings before saving.".into());
        }
        validate(&self.draft)?;
        let mut updated = toml::Value::try_from(&self.draft).map_err(|error| error.to_string())?;
        if let Some(bytes) = &self.baseline {
            let original: toml::Value = toml::from_str(std::str::from_utf8(bytes).map_err(|error| error.to_string())?).map_err(|error| error.to_string())?;
            let known: Options = original.clone().try_into().map_err(|error| error.to_string())?;
            preserve_unknown(&original, &toml::Value::try_from(known).map_err(|error| error.to_string())?, &mut updated);
        }
        let encoded = toml::to_string_pretty(&updated).map_err(|error| error.to_string())?;
        let temporary = self.path.with_extension("new");
        let result = (|| -> std::io::Result<()> {
            if let Some(parent) = self.path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&temporary)?;
            let result = (|| -> std::io::Result<()> {
                file.write_all(encoded.as_bytes())?;
                file.sync_all()?;
                if self.path.exists() && !self.path.with_extension("bak").exists() {
                    std::fs::copy(&self.path, self.path.with_extension("bak"))?;
                }
                std::fs::rename(&temporary, &self.path)
            })();
            if result.is_err() {
                let _ = std::fs::remove_file(&temporary);
            }
            result
        })();
        result.map_err(|error| error.to_string())?;
        self.baseline = Some(encoded.into_bytes());
        self.closed = true;
        Ok(self.draft.clone())
    }

    /// Restores only the fields the current page edits, like the legacy per-category reset.
    fn reset_page(&mut self) {
        let defaults = Options::default();
        match self.page {
            Page::Monitor => self.draft.monitor_settings = defaults.monitor_settings,
            Page::Terminal => {
                self.draft.connect_timeout = defaults.connect_timeout;
                self.draft.max_scrollback_lines = defaults.max_scrollback_lines;
                self.draft.default_cursor_shape = defaults.default_cursor_shape;
                self.draft.default_cursor_blinking = defaults.default_cursor_blinking;
                self.draft.is_dark_mode = defaults.is_dark_mode;
                self.draft.invert_mouse_wheel = defaults.invert_mouse_wheel;
            }
            Page::Audio => {
                self.draft.audio_enabled = defaults.audio_enabled;
                self.draft.console_beep = defaults.console_beep;
                self.draft.master_volume = defaults.master_volume;
                self.draft.dial_tone = defaults.dial_tone;
                self.draft.audio_device = defaults.audio_device;
            }
            Page::Paths => {
                self.draft.download_path = defaults.download_path;
                self.draft.capture_path = defaults.capture_path;
            }
            Page::Login => self.draft.iemsi = defaults.iemsi,
            Page::Serial => self.draft.serial = defaults.serial,
            Page::Sources => self.draft.web_directories = defaults.web_directories,
            Page::Modems => self.draft.modems = defaults.modems,
            Page::Protocols => {
                self.draft.transfer_protocols = defaults.transfer_protocols;
                self.commands.clear();
                self.invalid_commands.clear();
            }
        }
    }

    pub fn show(&mut self, context: &egui::Context) -> Option<Options> {
        #[derive(Clone, Copy)]
        enum Footer {
            Restore,
            Cancel,
            Ok,
        }
        let mut saved = None;
        let pages = [
            (Page::Monitor, tr!("settings-monitor-category")),
            (Page::Terminal, tr!("settings-terminal-category")),
            (Page::Audio, tr!("egui-audio")),
            (Page::Paths, tr!("settings-paths-category")),
            (Page::Login, tr!("settings-iemsi-category")),
            (Page::Serial, tr!("egui-serial")),
            (Page::Sources, tr!("egui-web-directories")),
            (Page::Modems, tr!("settings-modem-list-section")),
            (Page::Protocols, tr!("settings-protocol-category")),
        ];
        let response = Dialog::untitled("settings")
            .size(DialogSize::XLarge)
            .fixed_height(560.0)
            .show(context, |dialog| {
                dialog.tabs(&mut self.page, &pages);
                dialog.content(|ui| self.page_contents(ui));
                dialog.buttons([
                    DialogButton::secondary(labels::restore_defaults(), Footer::Restore).leading(),
                    DialogButton::cancel(tr!("egui-cancel"), Footer::Cancel),
                    DialogButton::primary(labels::ok(), Footer::Ok),
                ]);
            });
        match response.action {
            Some(Footer::Restore) => self.reset_page(),
            Some(Footer::Cancel) => self.closed = true,
            Some(Footer::Ok) => match self.save() {
                Ok(options) => saved = Some(options),
                Err(error) => self.error = Some(error),
            },
            None => {}
        }
        #[cfg(test)]
        {
            self.bounds = Some(response.rect);
        }
        if let Some(error) = &self.error {
            if super::messages::MessageBox::error("settings-error", &tr!("egui-message-settings"), error)
                .show(context)
                .is_some()
            {
                self.error = None;
            }
        }
        saved
    }

    fn page_contents(&mut self, ui: &mut egui::Ui) {
        use super::appearance::{check_row, combo_row, form_row, group};
        let options = &mut self.draft;
        match self.page {
            Page::Monitor => {
                super::monitor::fields(ui, &mut options.monitor_settings);
            }
            Page::Terminal => {
                group(ui, &tr!("settings-terminal-general-section"), |ui| {
                    let mut timeout = options.connect_timeout.as_secs();
                    form_row(ui, &tr!("egui-connect-timeout"), |ui| {
                        if number(ui, egui::DragValue::new(&mut timeout).range(1..=3600)).changed() {
                            options.connect_timeout = std::time::Duration::from_secs(timeout);
                        }
                    });
                    form_row(ui, &tr!("egui-scrollback-lines"), |ui| {
                        number(ui, egui::DragValue::new(&mut options.max_scrollback_lines).range(0..=1_000_000));
                    });
                    combo_row(ui, &tr!("egui-appearance"), theme_name(options.is_dark_mode), |ui| {
                        for mode in [None, Some(true), Some(false)] {
                            ui.selectable_value(&mut options.is_dark_mode, mode, theme_name(mode));
                        }
                    });
                    check_row(ui, &tr!("settings-terminal-invert-mouse-wheel"), &mut options.invert_mouse_wheel);
                });
                group(ui, &tr!("egui-cursor"), |ui| {
                    combo_row(
                        ui,
                        &tr!("settings-terminal-cursor-shape"),
                        format!("{:?}", options.default_cursor_shape),
                        |ui| {
                            for shape in [
                                icy_parser_core::CaretShape::Block,
                                icy_parser_core::CaretShape::Underline,
                                icy_parser_core::CaretShape::Bar,
                            ] {
                                ui.selectable_value(&mut options.default_cursor_shape, shape, format!("{shape:?}"));
                            }
                        },
                    );
                    check_row(ui, &tr!("settings-terminal-cursor-blinking"), &mut options.default_cursor_blinking);
                });
            }
            Page::Audio => {
                group(ui, &tr!("egui-audio"), |ui| {
                    check_row(ui, &tr!("egui-audio-enabled"), &mut options.audio_enabled);
                    check_row(ui, &tr!("settings-terminal-console-beep-checkbox"), &mut options.console_beep);
                    super::appearance::slider_row(ui, &tr!("egui-volume"), &mut options.master_volume, 0.0..=1.0);
                    use icy_engine_gui::music::music::DialTone;
                    combo_row(ui, &tr!("egui-dial-tone"), format!("{:?}", options.dial_tone), |ui| {
                        for tone in [DialTone::US, DialTone::UK, DialTone::Europe, DialTone::France, DialTone::Japan] {
                            ui.selectable_value(&mut options.dial_tone, tone, format!("{tone:?}"));
                        }
                    });
                    combo_row(
                        ui,
                        &tr!("settings-terminal-audio-device"),
                        options.audio_device.clone().unwrap_or_else(|| tr!("egui-system-default")),
                        |ui| {
                            ui.selectable_value(&mut options.audio_device, None, &*tr!("egui-system-default"));
                            for name in icy_engine_gui::music::music::SoundThread::output_devices() {
                                ui.selectable_value(&mut options.audio_device, Some(name.clone()), name);
                            }
                        },
                    );
                });
            }
            Page::Paths => {
                group(ui, &tr!("settings-paths-header"), |ui| {
                    let config = directories::ProjectDirs::from("com", "GitHub", "icy_term");
                    let directory = config.as_ref().map(|dirs| dirs.config_dir().to_path_buf());
                    location_row(ui, &tr!("settings-paths-config-dir"), directory.as_deref(), directory.as_deref());
                    location_row(
                        ui,
                        &tr!("settings-paths-config-file"),
                        directory.as_ref().map(|dir| dir.join("options.toml")).as_deref(),
                        None,
                    );
                    location_row(
                        ui,
                        &tr!("settings-paths-phonebook"),
                        directory.as_ref().map(|dir| dir.join("phonebook.toml")).as_deref(),
                        None,
                    );
                    let log = Options::get_log_file();
                    location_row(ui, &tr!("settings-paths-log-file"), log.as_deref(), log.as_deref());
                });
                group(ui, &tr!("settings-paths-editable-header"), |ui| {
                    path_field(ui, &tr!("settings-paths-download-dir"), &mut options.download_path);
                    path_field(ui, &tr!("settings-paths-capture-path"), &mut options.capture_path);
                });
            }
            Page::Login => {
                group(ui, &tr!("settings-iemsi-autologin-section"), |ui| {
                    check_row(ui, &tr!("egui-iemsi-login"), &mut options.iemsi.autologin);
                    text_field(ui, &tr!("settings-iemsi-alias"), &mut options.iemsi.alias);
                    text_field(ui, &tr!("settings-iemsi-location"), &mut options.iemsi.location);
                    text_field(ui, &tr!("settings-iemsi-data-phone"), &mut options.iemsi.data_phone);
                    text_field(ui, &tr!("settings-iemsi-voice-phone"), &mut options.iemsi.voice_phone);
                    text_field(ui, &tr!("settings-iemsi-birth-date"), &mut options.iemsi.birth_date);
                });
            }
            Page::Serial => {
                group(ui, &tr!("settings-modem-serial-section"), |ui| serial_fields(ui, &mut options.serial));
            }
            Page::Sources => {
                let mut remove = None;
                for (index, source) in options.web_directories.iter_mut().enumerate() {
                    ui.push_id(index, |ui| {
                        group(ui, &source.name.clone(), |ui| {
                            check_row(ui, &tr!("settings-enabled-checkbox"), &mut source.enabled);
                            text_field(ui, &tr!("settings-modem-name"), &mut source.name);
                            text_field(ui, &tr!("settings-web-directory-url"), &mut source.url);
                            item_actions(ui, |ui| {
                                if ui.button(&*tr!("settings-modem-remove-button")).clicked() {
                                    remove = Some(index);
                                }
                            });
                        });
                    });
                }
                if let Some(index) = remove {
                    options.web_directories.remove(index);
                }
                if ui.button(&*tr!("settings-web-directory-add")).clicked() {
                    options.web_directories.push(icy_term::WebDirectorySource {
                        name: String::new(),
                        url: String::new(),
                        enabled: true,
                    });
                }
            }
            Page::Modems => {
                let mut remove = None;
                for (index, modem) in options.modems.iter_mut().enumerate() {
                    ui.push_id(("modem", index), |ui| {
                        group(ui, &modem.name.clone(), |ui| {
                            text_field(ui, &tr!("settings-modem-name"), &mut modem.name);
                            let mut serial = icy_net::serial::Serial::from(modem.clone());
                            serial_fields(ui, &mut serial);
                            modem.device = serial.device;
                            modem.baud_rate = serial.baud_rate;
                            modem.format = serial.format;
                            modem.flow_control = serial.flow_control;
                            subheading(ui, &tr!("settings-modem-commands-section"));
                            for (label, command) in [
                                (&*tr!("egui-initialize"), &mut modem.init_command),
                                (&*tr!("settings-modem-dial_prefix"), &mut modem.dial_prefix),
                                (&*tr!("egui-dial-suffix"), &mut modem.dial_suffix),
                                (&*tr!("egui-hang-up"), &mut modem.hangup_command),
                            ] {
                                command_field(ui, label, command, &mut self.commands, &mut self.invalid_commands);
                            }
                            let mut responses: Vec<_> = modem.modem_responses.iter_mut().collect();
                            responses.sort_by_key(|(response, _)| format!("{response:?}"));
                            for (response, command) in responses {
                                command_field(ui, &format!("{response:?}"), command, &mut self.commands, &mut self.invalid_commands);
                            }
                            item_actions(ui, |ui| {
                                if ui
                                    .add_enabled(self.invalid_commands.is_empty(), egui::Button::new(tr!("egui-remove-modem")))
                                    .clicked()
                                {
                                    remove = Some(index);
                                }
                            });
                        });
                    });
                }
                if let Some(index) = remove {
                    options.modems.remove(index);
                    self.commands.clear();
                    self.invalid_commands.clear();
                }
                if ui.button(&*tr!("egui-add-modem")).clicked() {
                    options.modems.push(icy_net::modem::ModemConfiguration {
                        name: format!("Modem {}", options.modems.len() + 1),
                        ..Default::default()
                    });
                }
            }
            Page::Protocols => {
                let mut remove = None;
                let mut move_up = None;
                let mut move_down = None;
                let count = options.transfer_protocols.len();
                group(ui, &tr!("settings-protocol-list-section"), |ui| {
                    for (index, protocol) in options.transfer_protocols.iter_mut().enumerate() {
                        if index > 0 {
                            ui.separator();
                        }
                        ui.push_id(("protocol", index), |ui| {
                            let title = super::appearance::bold(ui, protocol.get_name());
                            egui::CollapsingHeader::new(title).id_salt(index).show(ui, |ui| {
                                ui.add_space(4.0);
                                check_row(ui, &tr!("settings-enabled-checkbox"), &mut protocol.enabled);
                                check_row(ui, &tr!("egui-automatic-detection"), &mut protocol.auto_transfer)
                                    .on_hover_text(&*tr!("egui-automatic-detection-hint"));
                                check_row(ui, &tr!("egui-multiple-files"), &mut protocol.batch);
                                check_row(ui, &tr!("egui-ask-filename"), &mut protocol.ask_for_download_location);
                                if !protocol.is_internal() {
                                    subheading(ui, &tr!("settings-protocol-commands-section"));
                                    text_field(ui, &tr!("settings-protocol-id"), &mut protocol.id);
                                    text_field(ui, &tr!("settings-modem-name"), &mut protocol.name);
                                    text_field(ui, &tr!("settings-protocol-description"), &mut protocol.description);
                                    text_field(ui, &tr!("settings-protocol-send-command"), &mut protocol.send_command);
                                    text_field(ui, &tr!("settings-protocol-recv-command"), &mut protocol.recv_command);
                                }
                                command_field(
                                    ui,
                                    &tr!("settings-protocol-download-signature"),
                                    &mut protocol.download_signature,
                                    &mut self.commands,
                                    &mut self.invalid_commands,
                                );
                                command_field(
                                    ui,
                                    &tr!("settings-protocol-upload-signature"),
                                    &mut protocol.upload_signature,
                                    &mut self.commands,
                                    &mut self.invalid_commands,
                                );
                                item_actions(ui, |ui| {
                                    if !protocol.is_internal()
                                        && ui
                                            .add_enabled(self.invalid_commands.is_empty(), egui::Button::new(tr!("egui-remove-protocol")))
                                            .clicked()
                                    {
                                        remove = Some(index);
                                    }
                                    if ui
                                        .add_enabled(
                                            self.invalid_commands.is_empty() && index + 1 < count,
                                            egui::Button::new(&*tr!("egui-move-down")),
                                        )
                                        .clicked()
                                    {
                                        move_down = Some(index);
                                    }
                                    if ui
                                        .add_enabled(self.invalid_commands.is_empty() && index > 0, egui::Button::new(&*tr!("egui-move-up")))
                                        .clicked()
                                    {
                                        move_up = Some(index);
                                    }
                                });
                            });
                        });
                    }
                });
                if let Some(index) = remove {
                    options.transfer_protocols.remove(index);
                    self.commands.clear();
                    self.invalid_commands.clear();
                }
                if let Some(index) = move_up {
                    options.transfer_protocols.swap(index, index - 1);
                    self.commands.clear();
                    self.invalid_commands.clear();
                }
                if let Some(index) = move_down {
                    options.transfer_protocols.swap(index, index + 1);
                    self.commands.clear();
                    self.invalid_commands.clear();
                }
                if ui.button(&*tr!("egui-add-external-protocol")).clicked() {
                    options.transfer_protocols.push(icy_term::TransferProtocol {
                        enabled: true,
                        id: format!("external-{}", options.transfer_protocols.len()),
                        name: tr!("egui-external-protocol"),
                        ..Default::default()
                    });
                }
            }
        }
    }
}

fn theme_name(dark: Option<bool>) -> String {
    match dark {
        None => tr!("egui-system"),
        Some(true) => tr!("egui-dark"),
        Some(false) => tr!("egui-light"),
    }
}

/// Numeric input sized like the other controls instead of shrinking to its digits.
fn number(ui: &mut egui::Ui, value: egui::DragValue<'_>) -> egui::Response {
    ui.add_sized([120.0, ui.spacing().interact_size.y], value)
}

fn subheading(ui: &mut egui::Ui, title: &str) {
    ui.add_space(6.0);
    ui.label(egui::RichText::new(title).weak().size(12.0));
}

/// Right aligned buttons at the bottom of a group; add them from right to left.
fn item_actions(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    ui.add_space(2.0);
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), add);
}

fn preserve_unknown(original: &toml::Value, known: &toml::Value, updated: &mut toml::Value) {
    if let (Some(original), Some(known), Some(updated)) = (original.as_table(), known.as_table(), updated.as_table_mut()) {
        for (key, value) in original {
            match known.get(key) {
                None => {
                    updated.insert(key.clone(), value.clone());
                }
                Some(known) => {
                    if let Some(updated) = updated.get_mut(key) {
                        preserve_unknown(value, known, updated);
                    }
                }
            }
        }
    }
}

pub fn text_field(ui: &mut egui::Ui, label: &str, value: &mut String) {
    ui.push_id(label, |ui| {
        super::appearance::form_row(ui, label, |ui| {
            ui.add(super::appearance::text_edit(value).desired_width(f32::INFINITY));
        });
    });
}

fn command_field(
    ui: &mut egui::Ui,
    label: &str,
    command: &mut icy_net::modem::ModemCommand,
    drafts: &mut std::collections::HashMap<egui::Id, String>,
    invalid: &mut std::collections::HashSet<egui::Id>,
) {
    let id = ui.id().with(label);
    let value = drafts.entry(id).or_insert_with(|| command.to_string());
    ui.push_id(id, |ui| {
        super::appearance::form_row(ui, label, |ui| {
            ui.horizontal(|ui| {
                let width = (ui.available_width() - 30.0 - ui.spacing().item_spacing.x - 2.0 * super::appearance::FIELD_MARGIN.x).max(40.0);
                let response = ui.add(super::appearance::text_edit(value).desired_width(width));
                if let Err(error) = icy_net::modem::ModemCommand::try_parse(value) {
                    let detail = format!("{error:?}");
                    response.on_hover_text(&detail);
                    ui.add(egui::Label::new(egui::RichText::new("!").strong().color(ui.visuals().error_fg_color)).sense(egui::Sense::hover()))
                        .on_hover_ui(|ui| {
                            ui.set_max_width((ui.ctx().content_rect().width() - 48.0).min(360.0));
                            ui.strong(label);
                            ui.add(egui::Label::new(detail).wrap());
                        });
                }
            });
        });
    });
    match icy_net::modem::ModemCommand::try_parse(value) {
        Ok(parsed) => {
            *command = parsed;
            invalid.remove(&id);
        }
        Err(_) => {
            invalid.insert(id);
        }
    }
}

/// Read-only location with an optional button that reveals it in the file manager.
fn location_row(ui: &mut egui::Ui, label: &str, path: Option<&Path>, open: Option<&Path>) {
    let mut text = path.map_or_else(|| "N/A".to_string(), |path| path.display().to_string());
    ui.push_id(label, |ui| {
        super::appearance::form_row(ui, label, |ui| {
            // Right to left so the button claims its width first and the field takes the rest.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(target) = open {
                    if ui.button(&*tr!("settings-paths-open")).clicked() && !ui.ctx().will_discard() {
                        let _ = open::that(target);
                    }
                }
                let response = ui.add_enabled(false, super::appearance::text_edit(&mut text).desired_width(f32::INFINITY));
                response.on_disabled_hover_text(&text);
            });
        });
    });
}

fn path_field(ui: &mut egui::Ui, label: &str, value: &mut String) {
    ui.push_id(label, |ui| {
        super::appearance::form_row(ui, label, |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("...").on_hover_text(tr!("egui-browse")).clicked() && !ui.ctx().will_discard() {
                    if let Some(path) = rfd::FileDialog::new().set_directory(&*value).pick_folder() {
                        *value = path.to_string_lossy().into_owned();
                    }
                }
                ui.add(super::appearance::text_edit(value).desired_width(f32::INFINITY));
            });
        })
    });
}

pub fn serial_fields(ui: &mut egui::Ui, serial: &mut icy_net::serial::Serial) {
    use icy_net::serial::{CharSize, FlowControl, Parity, StopBits};
    text_field(ui, &tr!("settings-modem-device"), &mut serial.device);
    super::appearance::form_row(ui, &tr!("egui-baud-rate"), |ui| {
        ui.add(egui::DragValue::new(&mut serial.baud_rate).range(50..=4_000_000));
    });
    super::appearance::combo_row(ui, &tr!("egui-data-bits"), char::from(serial.format.char_size).to_string(), |ui| {
        for size in [CharSize::Bits5, CharSize::Bits6, CharSize::Bits7, CharSize::Bits8] {
            ui.selectable_value(&mut serial.format.char_size, size, char::from(size).to_string());
        }
    });
    super::appearance::combo_row(ui, &tr!("egui-parity"), format!("{:?}", serial.format.parity), |ui| {
        for parity in [Parity::None, Parity::Odd, Parity::Even] {
            ui.selectable_value(&mut serial.format.parity, parity, format!("{parity:?}"));
        }
    });
    super::appearance::combo_row(ui, &tr!("egui-stop-bits"), char::from(serial.format.stop_bits).to_string(), |ui| {
        for bits in [StopBits::One, StopBits::Two] {
            ui.selectable_value(&mut serial.format.stop_bits, bits, char::from(bits).to_string());
        }
    });
    super::appearance::combo_row(ui, &tr!("settings-modem-flow_control"), format!("{:?}", serial.flow_control), |ui| {
        for flow in [FlowControl::None, FlowControl::XonXoff, FlowControl::RtsCts] {
            ui.selectable_value(&mut serial.flow_control, flow, format!("{flow:?}"));
        }
    });
}

fn validate(options: &Options) -> Result<(), String> {
    if !(0.0..=1.0).contains(&options.master_volume) {
        return Err("Invalid audio volume".into());
    }
    for source in &options.web_directories {
        if source.name.trim().is_empty() {
            return Err("Web directory name is required".into());
        }
        let url = url::Url::parse(&source.url).map_err(|_| "Invalid web directory URL")?;
        if url.scheme() != "https" || url.host_str().is_none() || !url.username().is_empty() || url.password().is_some() {
            return Err("Web directories require an HTTPS URL without credentials".into());
        }
    }
    let mut names = std::collections::HashSet::new();
    for modem in &options.modems {
        if modem.name.trim().is_empty() || !names.insert(&modem.name) {
            return Err("Modem names must be nonempty and unique".into());
        }
        if modem.device.trim().is_empty() || modem.baud_rate == 0 {
            return Err("Modems require a device and baud rate".into());
        }
        if modem.modem_responses.values().any(|command| command.to_bytes().is_empty()) {
            return Err("Modem response patterns must not be empty".into());
        }
    }
    let mut ids = std::collections::HashSet::new();
    for protocol in &options.transfer_protocols {
        if protocol.id.trim().is_empty() || !ids.insert(&protocol.id) {
            return Err("Protocol IDs must be nonempty and unique".into());
        }
        if !protocol.is_internal() && protocol.enabled && protocol.send_command.is_empty() && protocol.recv_command.is_empty() {
            return Err("External protocols require a send or receive command".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_page_keeps_the_same_dialog_size() {
        for viewport in [egui::vec2(1000.0, 800.0), egui::vec2(760.0, 620.0), egui::vec2(420.0, 560.0)] {
            let path = std::env::temp_dir().join(format!("icy-size-{}-{}.toml", std::process::id(), fastrand::u64(..)));
            let mut settings = Settings::open(&Options::default(), path).unwrap();
            let context = egui::Context::default();
            super::super::appearance::apply(&context);
            let mut sizes = Vec::new();
            for page in [
                Page::Monitor,
                Page::Terminal,
                Page::Audio,
                Page::Paths,
                Page::Login,
                Page::Serial,
                Page::Sources,
                Page::Modems,
                Page::Protocols,
            ] {
                settings.page = page;
                for _ in 0..3 {
                    let _ = context.run(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, viewport)),
                            ..Default::default()
                        },
                        |context| {
                            settings.show(context);
                        },
                    );
                }
                sizes.push((page, settings.bounds.unwrap().size()));
            }
            let (_, first) = sizes[0];
            for (page, size) in &sizes {
                assert!(
                    (size.x - first.x).abs() < 0.5 && (size.y - first.y).abs() < 0.5,
                    "{page:?} changes the dialog size at {viewport:?}: {size:?} vs {first:?}"
                );
            }
        }
    }

    #[test]
    fn escape_cancels_without_saving() {
        let path = std::env::temp_dir().join(format!("icy-escape-{}-{}.toml", std::process::id(), fastrand::u64(..)));
        let mut settings = Settings::open(&Options::default(), path.clone()).unwrap();
        let context = egui::Context::default();
        let mut saved = None;
        for events in [
            vec![],
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        ] {
            let _ = context.run(egui::RawInput { events, ..Default::default() }, |context| {
                saved = saved.take().or(settings.show(context));
            });
        }
        assert!(settings.closed, "Escape must close the dialog");
        assert!(saved.is_none() && !path.exists(), "Escape must not save");
    }

    #[test]
    fn every_input_shares_one_height() {
        let context = egui::Context::default();
        super::super::appearance::apply(&context);
        let mut plain = String::new();
        let mut path = "/tmp".to_string();
        let mut command = icy_net::modem::ModemCommand::try_parse("ATZ^M").unwrap();
        let mut drafts = std::collections::HashMap::new();
        let mut invalid = std::collections::HashSet::new();
        let mut heights = Vec::new();
        let _ = context.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0))),
                ..Default::default()
            },
            |context| {
                egui::CentralPanel::default().show(context, |ui| {
                    ui.set_width(600.0);
                    let mut row = |name: &str, add: &mut dyn FnMut(&mut egui::Ui)| {
                        let before = ui.cursor().top();
                        add(ui);
                        heights.push((name.to_owned(), ui.cursor().top() - before));
                    };
                    row("text", &mut |ui| text_field(ui, "A", &mut plain));
                    row("path", &mut |ui| path_field(ui, "B", &mut path));
                    row("command", &mut |ui| command_field(ui, "C", &mut command, &mut drafts, &mut invalid));
                });
            },
        );
        let (_, first) = &heights[0];
        for (name, height) in &heights {
            assert!(
                (height - first).abs() < 0.5,
                "the {name} field is {height} high, expected {first} ({heights:?})"
            );
        }
    }

    #[test]
    fn invalid_command_hover_preserves_the_last_valid_value() {
        let context = egui::Context::default();
        let mut command = icy_net::modem::ModemCommand::try_parse("ATZ^M").unwrap();
        let original = command.to_string();
        let mut drafts = std::collections::HashMap::new();
        let mut invalid = std::collections::HashSet::new();
        for (text, is_invalid) in [("^", true), ("AT^M", false)] {
            let _ = context.run(egui::RawInput::default(), |context| {
                egui::CentralPanel::default().show(context, |ui| {
                    drafts.insert(ui.id().with("Init"), text.into());
                    command_field(ui, "Init", &mut command, &mut drafts, &mut invalid);
                });
            });
            assert_eq!(!invalid.is_empty(), is_invalid);
            if is_invalid {
                assert_eq!(command.to_string(), original);
            } else {
                assert_eq!(command.to_string(), "AT^M");
            }
        }
    }

    #[test]
    fn settings_save_is_explicit_and_preserves_external_changes() {
        let path = std::env::temp_dir().join(format!("icy-settings-{}-{}.toml", std::process::id(), fastrand::u64(..)));
        let mut settings = Settings::open(&Options::default(), path.clone()).unwrap();
        settings.draft.master_volume = 0.3;
        assert!(!path.exists());
        settings.save().unwrap();
        let read: Options = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(read.master_volume, 0.3);
        std::fs::write(&path, "external").unwrap();
        assert!(settings.save().is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "external");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn opening_settings_uses_disk_values_and_preserves_unknown_tables() {
        let path = std::env::temp_dir().join(format!("icy-settings-current-{}.toml", fastrand::u64(..)));
        let current = Options {
            master_volume: 0.2,
            ..Default::default()
        };
        let mut original = toml::Value::try_from(&current).unwrap();
        original
            .as_table_mut()
            .unwrap()
            .insert("future_option".into(), toml::Value::String("keep".into()));
        original["monitor_settings"]
            .as_table_mut()
            .unwrap()
            .insert("future_monitor_option".into(), toml::Value::Boolean(true));
        std::fs::write(&path, toml::to_string(&original).unwrap()).unwrap();
        let mut settings = Settings::open(&Options::default(), path.clone()).unwrap();
        assert_eq!(settings.draft.master_volume, 0.2);
        settings.save().unwrap();
        let saved: toml::Value = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(saved["future_option"].as_str(), Some("keep"));
        assert_eq!(saved["monitor_settings"]["future_monitor_option"].as_bool(), Some(true));
        std::fs::remove_file(path.with_extension("bak")).unwrap();
        std::fs::remove_file(path).unwrap();
    }
}
