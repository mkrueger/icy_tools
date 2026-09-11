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
        let mut saved = None;
        let _modal = egui::Modal::new(egui::Id::new("settings"))
            .frame(super::appearance::dialog_frame(context))
            .show(context, |ui| {
                let height = (context.content_rect().height() - 64.0).clamp(140.0, 560.0);
                let top = ui.cursor().top();
                ui.set_width((context.content_rect().width() - 48.0).clamp(240.0, 760.0));
                ui.set_min_height(height);
                self.closed |= super::appearance::dialog_header(ui, &tr!("settings-heading"));
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
                if ui.available_width() < 640.0 || height < 300.0 {
                    egui::ComboBox::from_id_salt("settings-category")
                        .width(ui.available_width())
                        .selected_text(&pages.iter().find(|(page, _)| *page == self.page).unwrap().1)
                        .show_ui(ui, |ui| {
                            for (page, name) in &pages {
                                ui.selectable_value(&mut self.page, *page, name);
                            }
                        });
                } else {
                    ui.horizontal_wrapped(|ui| {
                        for (page, name) in pages {
                            super::appearance::tab(ui, &mut self.page, page, &name);
                        }
                    });
                }
                ui.separator();
                egui::ScrollArea::vertical()
                    .id_salt(("settings-page", self.page))
                    .auto_shrink([false, false])
                    .min_scrolled_height(0.0)
                    .max_height((height - (ui.cursor().top() - top) - 48.0).max(0.0))
                    .show(ui, |ui| {
                        let options = &mut self.draft;
                        match self.page {
                            Page::Monitor => {
                                super::monitor::fields(ui, &mut options.monitor_settings);
                            }
                            Page::Terminal => {
                                let mut timeout = options.connect_timeout.as_secs();
                                super::appearance::form_row(ui, &tr!("egui-connect-timeout"), |ui| {
                                    if ui.add(egui::DragValue::new(&mut timeout).range(1..=3600)).changed() {
                                        options.connect_timeout = std::time::Duration::from_secs(timeout);
                                    }
                                });
                                super::appearance::form_row(ui, &tr!("egui-scrollback-lines"), |ui| {
                                    ui.add(egui::DragValue::new(&mut options.max_scrollback_lines).range(0..=1_000_000));
                                });
                                super::appearance::form_row(ui, &tr!("egui-cursor"), |ui| {
                                    egui::ComboBox::from_id_salt("cursor-shape")
                                        .selected_text(format!("{:?}", options.default_cursor_shape))
                                        .show_ui(ui, |ui| {
                                            for shape in [
                                                icy_parser_core::CaretShape::Block,
                                                icy_parser_core::CaretShape::Underline,
                                                icy_parser_core::CaretShape::Bar,
                                            ] {
                                                ui.selectable_value(&mut options.default_cursor_shape, shape, format!("{shape:?}"));
                                            }
                                        });
                                });
                                super::appearance::form_row(ui, &tr!("egui-appearance"), |ui| {
                                    egui::ComboBox::from_id_salt("appearance")
                                        .selected_text(match options.is_dark_mode {
                                            None => tr!("egui-system"),
                                            Some(true) => tr!("egui-dark"),
                                            Some(false) => tr!("egui-light"),
                                        })
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(&mut options.is_dark_mode, None, &*tr!("egui-system"));
                                            ui.selectable_value(&mut options.is_dark_mode, Some(true), &*tr!("egui-dark"));
                                            ui.selectable_value(&mut options.is_dark_mode, Some(false), &*tr!("egui-light"));
                                        });
                                });
                                ui.add_space(12.0);
                                ui.separator();
                                ui.checkbox(&mut options.invert_mouse_wheel, &*tr!("settings-terminal-invert-mouse-wheel"));
                                ui.checkbox(&mut options.default_cursor_blinking, &*tr!("settings-terminal-cursor-blinking"));
                            }
                            Page::Audio => {
                                ui.checkbox(&mut options.audio_enabled, &*tr!("egui-audio-enabled"));
                                ui.checkbox(&mut options.console_beep, &*tr!("settings-terminal-console-beep-checkbox"));
                                super::appearance::slider_row(ui, &tr!("egui-volume"), &mut options.master_volume, 0.0..=1.0);
                                use icy_engine_gui::music::music::DialTone;
                                super::appearance::combo_row(ui, &tr!("egui-dial-tone"), format!("{:?}", options.dial_tone), |ui| {
                                    for tone in [DialTone::US, DialTone::UK, DialTone::Europe, DialTone::France, DialTone::Japan] {
                                        ui.selectable_value(&mut options.dial_tone, tone, format!("{tone:?}"));
                                    }
                                });
                                super::appearance::combo_row(
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
                            }
                            Page::Paths => {
                                super::appearance::section(ui, &tr!("settings-paths-header"));
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
                                super::appearance::section(ui, &tr!("settings-paths-editable-header"));
                                path_field(ui, &*tr!("settings-paths-download-dir"), &mut options.download_path);
                                path_field(ui, &*tr!("settings-paths-capture-path"), &mut options.capture_path);
                            }
                            Page::Login => {
                                ui.checkbox(&mut options.iemsi.autologin, &*tr!("egui-iemsi-login"));
                                text_field(ui, &*tr!("settings-iemsi-alias"), &mut options.iemsi.alias);
                                text_field(ui, &*tr!("settings-iemsi-location"), &mut options.iemsi.location);
                                text_field(ui, &*tr!("settings-iemsi-data-phone"), &mut options.iemsi.data_phone);
                                text_field(ui, &*tr!("settings-iemsi-voice-phone"), &mut options.iemsi.voice_phone);
                                text_field(ui, &*tr!("settings-iemsi-birth-date"), &mut options.iemsi.birth_date);
                            }
                            Page::Serial => {
                                serial_fields(ui, &mut options.serial);
                            }
                            Page::Sources => {
                                let mut remove = None;
                                for (index, source) in options.web_directories.iter_mut().enumerate() {
                                    ui.push_id(index, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.checkbox(&mut source.enabled, &*tr!("settings-enabled-checkbox"));
                                            if ui.small_button(&*tr!("settings-modem-remove-button")).clicked() {
                                                remove = Some(index);
                                            }
                                        });
                                        text_field(ui, &*tr!("settings-modem-name"), &mut source.name);
                                        text_field(ui, &*tr!("settings-web-directory-url"), &mut source.url);
                                        ui.separator();
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
                                        egui::CollapsingHeader::new(&modem.name).id_salt(index).default_open(true).show(ui, |ui| {
                                            text_field(ui, &*tr!("settings-modem-name"), &mut modem.name);
                                            let mut serial = icy_net::serial::Serial::from(modem.clone());
                                            serial_fields(ui, &mut serial);
                                            modem.device = serial.device;
                                            modem.baud_rate = serial.baud_rate;
                                            modem.format = serial.format;
                                            modem.flow_control = serial.flow_control;
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
                                            if ui
                                                .add_enabled(self.invalid_commands.is_empty(), egui::Button::new(tr!("egui-remove-modem")))
                                                .clicked()
                                            {
                                                remove = Some(index);
                                            }
                                        });
                                    });
                                }
                                if let Some(index) = remove {
                                    options.modems.remove(index);
                                    self.commands.clear();
                                    self.invalid_commands.clear();
                                }
                                if ui.button(&*tr!("egui-add-modem")).clicked() {
                                    let mut modem = icy_net::modem::ModemConfiguration::default();
                                    modem.name = format!("Modem {}", options.modems.len() + 1);
                                    options.modems.push(modem);
                                }
                            }
                            Page::Protocols => {
                                let mut remove = None;
                                let mut move_up = None;
                                let mut move_down = None;
                                let count = options.transfer_protocols.len();
                                for (index, protocol) in options.transfer_protocols.iter_mut().enumerate() {
                                    ui.push_id(("protocol", index), |ui| {
                                        egui::CollapsingHeader::new(protocol.get_name()).id_salt(index).show(ui, |ui| {
                                            ui.checkbox(&mut protocol.enabled, &*tr!("settings-enabled-checkbox"));
                                            ui.checkbox(&mut protocol.auto_transfer, &*tr!("egui-automatic-detection"));
                                            ui.checkbox(&mut protocol.batch, &*tr!("egui-multiple-files"));
                                            ui.checkbox(&mut protocol.ask_for_download_location, &*tr!("egui-ask-filename"));
                                            if !protocol.is_internal() {
                                                text_field(ui, &*tr!("settings-protocol-id"), &mut protocol.id);
                                                text_field(ui, &*tr!("settings-modem-name"), &mut protocol.name);
                                                text_field(ui, &*tr!("settings-protocol-description"), &mut protocol.description);
                                                text_field(ui, &*tr!("settings-protocol-send-command"), &mut protocol.send_command);
                                                text_field(ui, &*tr!("settings-protocol-recv-command"), &mut protocol.recv_command);
                                            }
                                            command_field(
                                                ui,
                                                &*tr!("settings-protocol-download-signature"),
                                                &mut protocol.download_signature,
                                                &mut self.commands,
                                                &mut self.invalid_commands,
                                            );
                                            command_field(
                                                ui,
                                                &*tr!("settings-protocol-upload-signature"),
                                                &mut protocol.upload_signature,
                                                &mut self.commands,
                                                &mut self.invalid_commands,
                                            );
                                            ui.horizontal_wrapped(|ui| {
                                                if ui
                                                    .add_enabled(self.invalid_commands.is_empty() && index > 0, egui::Button::new(&*tr!("egui-move-up")))
                                                    .clicked()
                                                {
                                                    move_up = Some(index);
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
                                                if !protocol.is_internal()
                                                    && ui
                                                        .add_enabled(self.invalid_commands.is_empty(), egui::Button::new(tr!("egui-remove-protocol")))
                                                        .clicked()
                                                {
                                                    remove = Some(index);
                                                }
                                            });
                                        });
                                    });
                                }
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
                    });
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    if ui.add(super::appearance::primary_button(tr!("egui-save"))).clicked() {
                        match self.save() {
                            Ok(options) => saved = Some(options),
                            Err(error) => self.error = Some(error),
                        }
                    }
                    if ui.button(&*tr!("egui-discard")).clicked() {
                        self.closed = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(&*tr!("egui-reset-page")).clicked() {
                            self.reset_page();
                        }
                    });
                });
            });
        #[cfg(test)]
        {
            self.bounds = Some(_modal.response.rect);
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
    let text = path.map_or_else(|| "N/A".to_string(), |path| path.display().to_string());
    super::appearance::form_row(ui, label, |ui| {
        // Right to left so the button claims its width first and the path truncates instead.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some(target) = open {
                if ui.button(&*tr!("settings-paths-open")).clicked() && !ui.ctx().will_discard() {
                    let _ = open::that(target);
                }
            }
            ui.add(egui::Label::new(egui::RichText::new(&text).monospace().size(11.0)).truncate().selectable(true))
                .on_hover_text(&text);
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
    text_field(ui, &*tr!("settings-modem-device"), &mut serial.device);
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
        let mut current = Options::default();
        current.master_volume = 0.2;
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
