use std::{
    fs::{self, File},
    io::Write,
    time::Duration,
};

use egui::{Modifiers, Rect};
use egui_bind::KeyOrPointer;
use i18n_embed_fl::fl;
use icy_engine::Color;
use icy_engine_gui::MonitorSettings;
use toml::Value;

use crate::{Modem, TerminalResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Scaling {
    #[default]
    Nearest,
    Linear,
}

impl Scaling {
    pub const ALL: [Scaling; 2] = [Scaling::Nearest, Scaling::Linear];

    #[must_use]
    pub fn get_filter(&self) -> i32 {
        match self {
            Scaling::Nearest => glow::NEAREST as i32,
            Scaling::Linear => glow::LINEAR as i32,
        }
    }
}

pub struct Key {
    pub description: String,
    pub key: egui::Key,
    pub modifiers: Modifiers,
}

type KeyType = Option<(KeyOrPointer, Modifiers)>;

macro_rules! keys {
    ($( ($l:ident, $key:ident, $mod: ident, $translation: expr) ),* ) => {
        #[derive(Debug, Clone, PartialEq)]
        pub struct KeyBindings {
            $(
                pub $l: KeyType,
            )*
        }

        impl Default for KeyBindings {
            fn default() -> Self {
                Self {
                    $(
                        $l: Some((KeyOrPointer::Key(egui::Key::$key), Modifiers::$mod)),
                    )*
                }
            }
        }

        fn parse_keybinding_settings(options: &mut Options, key_settings: &toml::map::Map<String, Value>) {
            for (k, v) in key_settings {
                match k.as_str() {
                    $(
                        stringify!($l) =>  {
                            if let Value::String(str) = v {
                                options.bind.$l = parse_key_binding(str);
                            }
                        }
                    )*
                    _ => {}
                }
            }
        }

        fn write_keybindings(file: &mut File, bind: &KeyBindings) -> TerminalResult<()> {
            file.write_all("[KEYBINDINGS]\n".to_string().as_bytes())?;
            $(
                file.write_all(
                    format!(
                        "{} = \"{}\"\n",
                        stringify!($l),
                        convert_to_string(bind.$l)
                    )
                    .as_bytes(),
                )?;
            )*
            Ok(())
        }

        pub(crate) fn show_keybinds_settings(state: &crate::ui::MainWindowState, ui: &mut egui::Ui) -> Option<crate::ui::dialogs::settings_dialog::Message>  {
            let mut bind = state.options.bind.clone();
            let mut changed_bindings = false;
            egui::Grid::new("keybinds_grid")
                .num_columns(2)
                .show(ui, |ui| {
                    $(
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(fl!(crate::LANGUAGE_LOADER, $translation));
                        });
                        if ui.add(egui_bind::Bind::new(
                            stringify!($l),
                            &mut bind.$l,
                        )).changed() {
                            changed_bindings = true;
                        }
                        ui.end_row();
                    )*
                });
            if changed_bindings {
                Some(crate::ui::dialogs::settings_dialog::Message::UpdateKeybinds(bind))
            } else {
                None
            }
        }
    }
}

fn parse_key_binding(str: impl Into<String>) -> KeyType {
    let str: String = str.into();
    let parts = str.split('|').map(str::trim).collect::<Vec<_>>();
    if parts.len() != 2 {
        return None;
    }
    let key = parse_key(parts[0]);
    let m = Modifiers {
        alt: parts[1].contains("Alt"),
        ctrl: parts[1].contains("Ctrl"),
        shift: parts[1].contains("Shift"),
        mac_cmd: parts[1].contains("Command"),
        command: parts[1].contains("Ctrl") | parts[1].contains("Command"),
    };
    Some((KeyOrPointer::Key(key), m))
}

// Generated from "egui::Key::name()"
// \s*(.*)\s*=>\s*(.*)\s*,
// replace:
// $2 => egui::$1,
fn parse_key(s: &str) -> egui::Key {
    match s {
        "Down" => egui::Key::ArrowDown,
        "Left" => egui::Key::ArrowLeft,
        "Right" => egui::Key::ArrowRight,
        "Up" => egui::Key::ArrowUp,
        "Escape" => egui::Key::Escape,
        "Tab" => egui::Key::Tab,
        "Backspace" => egui::Key::Backspace,
        "Enter" => egui::Key::Enter,
        "Space" => egui::Key::Space,
        "Insert" => egui::Key::Insert,
        "Delete" => egui::Key::Delete,
        "Home" => egui::Key::Home,
        "End" => egui::Key::End,
        "PageUp" => egui::Key::PageUp,
        "PageDown" => egui::Key::PageDown,
        "Minus" => egui::Key::Minus,
        "Plus" => egui::Key::Plus,
        "0" => egui::Key::Num0,
        "1" => egui::Key::Num1,
        "2" => egui::Key::Num2,
        "3" => egui::Key::Num3,
        "4" => egui::Key::Num4,
        "5" => egui::Key::Num5,
        "6" => egui::Key::Num6,
        "7" => egui::Key::Num7,
        "8" => egui::Key::Num8,
        "9" => egui::Key::Num9,
        "A" => egui::Key::A,
        "B" => egui::Key::B,
        "C" => egui::Key::C,
        "D" => egui::Key::D,
        "E" => egui::Key::E,
        "F" => egui::Key::F,
        "G" => egui::Key::G,
        "H" => egui::Key::H,
        "I" => egui::Key::I,
        "J" => egui::Key::J,
        "K" => egui::Key::K,
        "L" => egui::Key::L,
        "M" => egui::Key::M,
        "N" => egui::Key::N,
        "O" => egui::Key::O,
        "P" => egui::Key::P,
        "Q" => egui::Key::Q,
        "R" => egui::Key::R,
        "S" => egui::Key::S,
        "T" => egui::Key::T,
        "U" => egui::Key::U,
        "V" => egui::Key::V,
        "W" => egui::Key::W,
        "X" => egui::Key::X,
        "Y" => egui::Key::Y,
        "Z" => egui::Key::Z,
        "F1" => egui::Key::F1,
        "F2" => egui::Key::F2,
        "F3" => egui::Key::F3,
        "F4" => egui::Key::F4,
        "F5" => egui::Key::F5,
        "F6" => egui::Key::F6,
        "F7" => egui::Key::F7,
        "F8" => egui::Key::F8,
        "F9" => egui::Key::F9,
        "F10" => egui::Key::F10,
        "F11" => egui::Key::F11,
        "F12" => egui::Key::F12,
        "F13" => egui::Key::F13,
        "F14" => egui::Key::F14,
        "F15" => egui::Key::F15,
        "F16" => egui::Key::F16,
        "F17" => egui::Key::F17,
        "F18" => egui::Key::F18,
        "F19" => egui::Key::F19,
        _ => egui::Key::F20,
    }
}

fn convert_to_string(key: KeyType) -> String {
    match key {
        Some((key, modifier)) => {
            if let KeyOrPointer::Key(key) = key {
                let m = format!(
                    "{}{}{}{}",
                    if modifier.command { "Command" } else { "" },
                    if modifier.ctrl { "Ctrl" } else { "" },
                    if modifier.alt { "Alt" } else { "" },
                    if modifier.shift { "Shift" } else { "" }
                );
                format!("{}|{}", key.name(), m)
            } else {
                "None".to_string()
            }
        }
        None => "None".to_string(),
    }
}

keys![
    (hangup, H, ALT, "settings-keybinds-disconnect"),
    (dialing_directory, D, ALT, "settings-keybinds-dialing-directory"),
    (send_login_pw, L, ALT, "settings-keybinds-send-login"),
    (upload, PageUp, ALT, "settings-keybinds-upload"),
    (download, PageDown, ALT, "settings-keybinds-download"),
    (clear_screen, C, ALT, "settings-keybinds-clear-screen"),
    (quit, X, ALT, "settings-keybinds-quit"),
    (full_screen, F11, NONE, "settings-keybinds-toggle-fullscreen"),
    (show_settings, O, ALT, "settings-keybinds-show-settings"),
    (show_find, F, ALT, "settings-keybinds-show-find"),
    (show_capture, P, ALT, "settings-keybinds-capture-control")
];

#[derive(Debug, Clone, PartialEq)]
pub struct IEMSISettings {
    pub autologin: bool,
    pub alias: String,
    pub location: String,
    pub data_phone: String,
    pub voice_phone: String,
    pub birth_date: String,
}

impl Default for IEMSISettings {
    fn default() -> Self {
        Self {
            autologin: true,
            alias: String::default(),
            location: String::default(),
            data_phone: String::default(),
            voice_phone: String::default(),
            birth_date: String::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    pub scaling: Scaling,
    pub connect_timeout: Duration,
    pub console_beep: bool,
    pub is_dark_mode: Option<bool>,

    pub monitor_settings: MonitorSettings,
    pub bind: KeyBindings,
    pub iemsi: IEMSISettings,

    pub window_rect: Option<Rect>,

    pub modem: Modem,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            scaling: Scaling::default(),
            connect_timeout: Duration::from_secs(1000),
            monitor_settings: MonitorSettings::default(),
            iemsi: IEMSISettings::default(),
            console_beep: true,
            bind: KeyBindings::default(),
            is_dark_mode: None,
            window_rect: None,
            modem: Modem::default(),
        }
    }
}

impl Options {
    /// .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn load_options() -> TerminalResult<Self> {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(proj_dirs) = directories::ProjectDirs::from("com", "GitHub", "icy_term") {
            let options_file = proj_dirs.config_dir().join("options.toml");
            if options_file.exists() {
                match fs::read_to_string(&options_file) {
                    Ok(content) => {
                        return Ok(Options::from_str(&content));
                    }
                    Err(err) => {
                        return Err(err.into());
                    }
                }
            }
        }
        Ok(Options::default())
    }

    /// Returns the store options of this [`Options`].
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn store_options(&self) -> TerminalResult<()> {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(proj_dirs) = directories::ProjectDirs::from("com", "GitHub", "icy_term") {
            let file_name = proj_dirs.config_dir().join("options.toml");
            let mut write_name = file_name.clone();
            write_name.set_extension("new");

            let mut file = File::create(&write_name)?;
            file.write_all(b"version = \"1.1\"\n")?;

            if let Some(rect) = self.window_rect {
                file.write_all(format!("window_coord = \"{}/{}-{}/{}\"\n", rect.min.x, rect.min.y, rect.max.x, rect.max.y).as_bytes())?;
            }

            file.write_all(format!("scaling = \"{:?}\"\n", self.scaling).as_bytes())?;
            if let Some(dark_mode) = self.is_dark_mode {
                file.write_all(format!("is_dark_mode = {dark_mode}\n").as_bytes())?;
            }
            file.write_all(format!("border_color = {:?}\n", self.monitor_settings.border_color.to_hex()).as_bytes())?;
            file.write_all(format!("use_crt_filter = {:?}\n", self.monitor_settings.use_filter).as_bytes())?;
            file.write_all(format!("monitor_type = {:?}\n", self.monitor_settings.monitor_type).as_bytes())?;
            file.write_all(format!("monitor_gamma = {:?}\n", self.monitor_settings.gamma).as_bytes())?;
            file.write_all(format!("monitor_contrast = {:?}\n", self.monitor_settings.contrast).as_bytes())?;
            file.write_all(format!("monitor_saturation = {:?}\n", self.monitor_settings.saturation).as_bytes())?;
            file.write_all(format!("monitor_brightness = {:?}\n", self.monitor_settings.brightness).as_bytes())?;
            file.write_all(format!("monitor_blur = {:?}\n", self.monitor_settings.blur).as_bytes())?;
            file.write_all(format!("monitor_curvature = {:?}\n", self.monitor_settings.curvature).as_bytes())?;
            file.write_all(format!("monitor_scanlines = {:?}\n", self.monitor_settings.scanlines).as_bytes())?;

            if self.console_beep != Options::default().console_beep {
                file.write_all(format!("console_beep = {}\n", self.console_beep).as_bytes())?;
            }
            /*
            if !self.capture_filename.is_empty() {
                file.write_all(format!("capture_filename = \"{}\"\n", self.capture_filename).as_bytes())?;
            }*/

            file.write_all("[IEMSI]\n".to_string().as_bytes())?;

            if !self.iemsi.autologin {
                file.write_all(format!("autologin = {}\n", self.iemsi.autologin).as_bytes())?;
            }
            if !self.iemsi.location.is_empty() {
                file.write_all(format!("location = \"{}\"\n", self.iemsi.location).as_bytes())?;
            }
            if !self.iemsi.alias.is_empty() {
                file.write_all(format!("alias = \"{}\"\n", self.iemsi.alias).as_bytes())?;
            }
            if !self.iemsi.data_phone.is_empty() {
                file.write_all(format!("data_phone = \"{}\"\n", self.iemsi.data_phone).as_bytes())?;
            }
            if !self.iemsi.voice_phone.is_empty() {
                file.write_all(format!("voice_phone = \"{}\"\n", self.iemsi.voice_phone).as_bytes())?;
            }
            if !self.iemsi.birth_date.is_empty() {
                file.write_all(format!("birth_date = \"{}\"\n", self.iemsi.birth_date).as_bytes())?;
            }

            write_keybindings(&mut file, &self.bind)?;

            file.write_all("[[modem]]\n".to_string().as_bytes())?;
            self.modem.write_modem_settings(&mut file)?;

            file.flush()?;

            // move temp file to the real file
            std::fs::rename(&write_name, &file_name)?;
        }
        Ok(())
    }

    fn from_str(input_text: &str) -> Options {
        match input_text.parse::<Value>() {
            Ok(value) => {
                let mut result = Options::default();
                parse_value(&mut result, &value);
                result
            }
            Err(err) => {
                log::error!("Error parsing options: {err}");
                Options::default()
            }
        }
    }

    pub(crate) fn reset_monitor_settings(&mut self) {
        self.scaling = Scaling::Nearest;
        self.monitor_settings = MonitorSettings::default();
    }

    pub(crate) fn reset_keybindings(&mut self) {
        self.bind = KeyBindings::default();
    }
}

fn parse_value(options: &mut Options, value: &Value) {
    match value {
        Value::Array(array) => {
            for v in array {
                parse_value(options, v);
            }
        }
        Value::Table(table) => {
            for (k, v) in table {
                match k.as_str() {
                    "window_coord" => {
                        if let Value::String(str) = v {
                            let mut minmax = str.split('-');
                            if let Some(min) = minmax.next() {
                                let min = min.split('/').collect::<Vec<&str>>();
                                if let Some(max) = minmax.next() {
                                    let max = max.split('/').collect::<Vec<&str>>();
                                    if let (Ok(x1), Ok(y1)) = (min[0].parse::<f32>(), min[1].parse::<f32>()) {
                                        if let (Ok(x2), Ok(y2)) = (max[0].parse::<f32>(), max[1].parse::<f32>()) {
                                            options.window_rect = Some(Rect::from_min_max(egui::Pos2::new(x1, y1), egui::Pos2::new(x2, y2)));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    "scaling" => {
                        if let Value::String(str) = v {
                            match str.as_str() {
                                "Nearest" => options.scaling = Scaling::Nearest,
                                "Linear" => options.scaling = Scaling::Linear,
                                _ => {}
                            }
                        }
                    }
                    "is_dark_mode" => {
                        if let Value::Boolean(b) = v {
                            options.is_dark_mode = Some(*b);
                        }
                    }
                    "border_color" => {
                        if let Value::String(str) = v {
                            match Color::from_hex(str) {
                                Ok(color) => options.monitor_settings.border_color = color,
                                Err(err) => {
                                    log::error!("Error parsing border_color: {}", err);
                                }
                            }
                        }
                    }
                    "use_crt_filter" => {
                        if let Value::Boolean(b) = v {
                            options.monitor_settings.use_filter = *b;
                        }
                    }
                    "monitor_type" => {
                        if let Value::Integer(b) = v {
                            options.monitor_settings.monitor_type = *b as usize;
                        }
                    }
                    "monitor_gamma" => {
                        if let Value::Float(f) = v {
                            options.monitor_settings.gamma = *f as f32;
                        }
                    }
                    "monitor_contrast" => {
                        if let Value::Float(f) = v {
                            options.monitor_settings.contrast = *f as f32;
                        }
                    }
                    "monitor_saturation" => {
                        if let Value::Float(f) = v {
                            options.monitor_settings.saturation = *f as f32;
                        }
                    }
                    "monitor_brightness" => {
                        if let Value::Float(f) = v {
                            options.monitor_settings.brightness = *f as f32;
                        }
                    }
                    "monitor_blur" => {
                        if let Value::Float(f) = v {
                            options.monitor_settings.blur = *f as f32;
                        }
                    }
                    "monitor_curvature" => {
                        if let Value::Float(f) = v {
                            options.monitor_settings.curvature = *f as f32;
                        }
                    }
                    "monitor_scanlines" => {
                        if let Value::Float(f) = v {
                            options.monitor_settings.scanlines = *f as f32;
                        }
                    }
                    "IEMSI" => {
                        if let Value::Table(iemsi_settings) = v {
                            parse_iemsi_settings(options, iemsi_settings);
                        }
                    }
                    "KEYBINDINGS" => {
                        if let Value::Table(keybind_settings) = v {
                            parse_keybinding_settings(options, keybind_settings);
                        }
                    }

                    "console_beep" => {
                        if let Value::Boolean(b) = v {
                            options.console_beep = *b;
                        }
                    } /*
                    "capture_filename" => {
                    if let Value::String(b) = v {
                    options.capture_filename = b.clone();
                    }
                    }*/
                    "modem" => {
                        if let Value::Array(array) = v {
                            for v in array {
                                if let Value::Table(b) = v {
                                    options.modem = Modem::from_table(b);
                                    break;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

fn parse_iemsi_settings(options: &mut Options, iemsi_settings: &toml::map::Map<String, Value>) {
    for (k, v) in iemsi_settings {
        match k.as_str() {
            "autologin" => {
                if let Value::Boolean(autologin) = v {
                    options.iemsi.autologin = *autologin;
                }
            }
            "location" => {
                if let Value::String(str) = v {
                    options.iemsi.location = str.clone();
                }
            }
            "alias" => {
                if let Value::String(str) = v {
                    options.iemsi.alias = str.clone();
                }
            }
            "data_phone" => {
                if let Value::String(str) = v {
                    options.iemsi.data_phone = str.clone();
                }
            }
            "voice_phone" => {
                if let Value::String(str) = v {
                    options.iemsi.voice_phone = str.clone();
                }
            }
            "birth_date" => {
                if let Value::String(str) = v {
                    options.iemsi.birth_date = str.clone();
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::field_reassign_with_default)]
    use super::*;

    #[test]
    fn test_reset_monitor_settings() {
        let mut opt: Options = Options::default();
        opt.scaling = Scaling::Linear;
        opt.monitor_settings.blur = 0.0;
        opt.monitor_settings.brightness = 1.0;

        assert_ne!(Options::default().scaling, opt.scaling);
        assert_ne!(Options::default().monitor_settings, opt.monitor_settings);
        opt.reset_monitor_settings();
        assert_eq!(Options::default().scaling, opt.scaling);
        assert_eq!(Options::default().monitor_settings, opt.monitor_settings);
    }

    #[test]
    fn test_reset_keybindings() {
        let mut opt = Options::default();
        opt.bind.download = None;
        opt.bind.full_screen = None;

        assert_ne!(Options::default().bind, opt.bind);
        opt.reset_keybindings();
        assert_eq!(Options::default().bind, opt.bind);
    }
}
