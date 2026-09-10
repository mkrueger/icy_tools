use eframe::egui;
use icy_term::Options;
use std::{
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Page {
    #[default]
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

    pub fn show(&mut self, context: &egui::Context) -> Option<Options> {
        let mut saved = None;
        egui::Modal::new(egui::Id::new("settings"))
            .frame(super::appearance::dialog_frame(context))
            .show(context, |ui| {
                let height = (context.content_rect().height() - 64.0).clamp(140.0, 560.0);
                let top = ui.cursor().top();
                ui.set_width((context.content_rect().width() - 48.0).clamp(240.0, 760.0));
                ui.set_min_height(height);
                ui.heading(&*tr!("settings-heading"));
                let pages = [
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
                                ui.add(egui::Slider::new(&mut options.master_volume, 0.0..=1.0).text(&*tr!("egui-volume")));
                                use icy_engine_gui::music::music::DialTone;
                                egui::ComboBox::from_label(&*tr!("egui-dial-tone"))
                                    .selected_text(format!("{:?}", options.dial_tone))
                                    .show_ui(ui, |ui| {
                                        for tone in [DialTone::US, DialTone::UK, DialTone::Europe, DialTone::France, DialTone::Japan] {
                                            ui.selectable_value(&mut options.dial_tone, tone, format!("{tone:?}"));
                                        }
                                    });
                                egui::ComboBox::from_label(&*tr!("settings-terminal-audio-device"))
                                    .selected_text(options.audio_device.as_deref().unwrap_or(&*tr!("egui-system-default")))
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut options.audio_device, None, &*tr!("egui-system-default"));
                                        for name in icy_engine_gui::music::music::SoundThread::output_devices() {
                                            ui.selectable_value(&mut options.audio_device, Some(name.clone()), name);
                                        }
                                    });
                            }
                            Page::Paths => {
                                path_field(ui, &*tr!("settings-paths-download-dir"), &mut options.download_path);
                                path_field(ui, &*tr!("egui-captures"), &mut options.capture_path);
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
                if let Some(error) = &self.error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
                ui.horizontal(|ui| {
                    if ui.add(super::appearance::primary_button(tr!("egui-save"))).clicked() {
                        match self.save() {
                            Ok(options) => saved = Some(options),
                            Err(error) => self.error = Some(error),
                        }
                    }
                    if ui.button(&*tr!("egui-discard")).clicked() {
                        self.closed = true;
                    }
                });
            });
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
        ui.label(label);
        ui.add(egui::TextEdit::singleline(value).desired_width(f32::INFINITY));
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
    text_field(ui, label, value);
    match icy_net::modem::ModemCommand::try_parse(value) {
        Ok(parsed) => {
            *command = parsed;
            invalid.remove(&id);
        }
        Err(error) => {
            invalid.insert(id);
            ui.colored_label(ui.visuals().error_fg_color, format!("{error:?}"));
        }
    }
}

fn path_field(ui: &mut egui::Ui, label: &str, value: &mut String) {
    text_field(ui, label, value);
    if ui.button(format!("Browse {label}...")).clicked() {
        if let Some(path) = rfd::FileDialog::new().set_directory(&*value).pick_folder() {
            *value = path.to_string_lossy().into_owned();
        }
    }
}

pub fn serial_fields(ui: &mut egui::Ui, serial: &mut icy_net::serial::Serial) {
    use icy_net::serial::{CharSize, FlowControl, Parity, StopBits};
    text_field(ui, &*tr!("settings-modem-device"), &mut serial.device);
    ui.horizontal(|ui| {
        ui.label(&*tr!("egui-baud-rate"));
        ui.add(egui::DragValue::new(&mut serial.baud_rate).range(50..=4_000_000));
    });
    egui::ComboBox::from_label(&*tr!("egui-data-bits"))
        .selected_text(char::from(serial.format.char_size).to_string())
        .show_ui(ui, |ui| {
            for size in [CharSize::Bits5, CharSize::Bits6, CharSize::Bits7, CharSize::Bits8] {
                ui.selectable_value(&mut serial.format.char_size, size, char::from(size).to_string());
            }
        });
    egui::ComboBox::from_label(&*tr!("egui-parity"))
        .selected_text(format!("{:?}", serial.format.parity))
        .show_ui(ui, |ui| {
            for parity in [Parity::None, Parity::Odd, Parity::Even] {
                ui.selectable_value(&mut serial.format.parity, parity, format!("{parity:?}"));
            }
        });
    egui::ComboBox::from_label(&*tr!("egui-stop-bits"))
        .selected_text(char::from(serial.format.stop_bits).to_string())
        .show_ui(ui, |ui| {
            for bits in [StopBits::One, StopBits::Two] {
                ui.selectable_value(&mut serial.format.stop_bits, bits, char::from(bits).to_string());
            }
        });
    egui::ComboBox::from_label(&*tr!("settings-modem-flow_control"))
        .selected_text(format!("{:?}", serial.flow_control))
        .show_ui(ui, |ui| {
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
