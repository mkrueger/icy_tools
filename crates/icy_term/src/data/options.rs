use std::{
    fs::{self, File},
    io::Write,
    time::Duration,
};

use icy_engine::Color;
use icy_engine_gui::{MonitorSettings, MonitorType};
use serde::{Deserialize, Serialize};

use crate::{Modem, TerminalResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Scaling {
    #[default]
    Nearest,
    Linear,
}

impl Scaling {
    pub const ALL: [Scaling; 2] = [Scaling::Nearest, Scaling::Linear];
    /*
    #[must_use]
    pub fn get_filter(&self) -> i32 {
        match self {
            Scaling::Nearest => glow::NEAREST as i32,
            Scaling::Linear => glow::LINEAR as i32,
        }
    }*/
}

// ...existing code...

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IEMSISettings {
    #[serde(default = "default_true")]
    pub autologin: bool,
    #[serde(default)]
    pub alias: String,
    #[serde(default)]
    pub location: String,
    #[serde(default)]
    pub data_phone: String,
    #[serde(default)]
    pub voice_phone: String,
    #[serde(default)]
    pub birth_date: String,
}

fn default_true() -> bool {
    true
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Options {
    #[serde(default = "default_connect_timeout")]
    #[serde(with = "duration_secs")]
    pub connect_timeout: Duration,

    #[serde(default = "default_true")]
    pub console_beep: bool,

    #[serde(default)]
    pub is_dark_mode: Option<bool>,

    #[serde(default)]
    pub theme: String,

    // pub scaling: Scaling,

    // pub monitor_settings: MonitorSettings,

    //    pub bind: KeyBindings,
    #[serde(default)]
    pub iemsi: IEMSISettings,

    //    pub window_rect: Option<Rect>,
    #[serde(default)]
    pub modems: Vec<Modem>,
}

fn default_connect_timeout() -> Duration {
    Duration::from_secs(1000)
}

// Custom serialization for Duration as seconds
mod duration_secs {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(duration.as_secs())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        Ok(Duration::from_secs(secs))
    }
}

impl Default for Options {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(1000),
            //scaling: Scaling::default(),
            //monitor_settings: MonitorSettings::default(),
            iemsi: IEMSISettings::default(),
            console_beep: true,
            //            bind: KeyBindings::default(),
            is_dark_mode: None,
            //            window_rect: None,
            theme: "".to_string(),
            modems: Vec::new(),
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
        if let Some(proj_dirs) = directories::ProjectDirs::from("com", "GitHub", "icy_term") {
            let options_file = proj_dirs.config_dir().join("options.toml");
            if options_file.exists() {
                let content = fs::read_to_string(&options_file)?;
                let options: Options = toml::from_str(&content)?;
                return Ok(options);
            }
        }
        Ok(Options::default())
    }
    /*
    pub(crate) fn get_theme(&self) -> egui::ThemePreference {
        if let Some(dark_mode) = self.is_dark_mode {
            if dark_mode {
                egui::ThemePreference::Dark
            } else {
                egui::ThemePreference::Light
            }
        } else {
            egui::ThemePreference::System
        }
    }*/

    /// Returns the store options of this [`Options`].
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn store_options(&self) -> TerminalResult<()> {
        if let Some(proj_dirs) = directories::ProjectDirs::from("com", "GitHub", "icy_term") {
            let file_name = proj_dirs.config_dir().join("options.toml");
            let mut write_name = file_name.clone();
            write_name.set_extension("new");

            // Create config directory if it doesn't exist
            fs::create_dir_all(proj_dirs.config_dir())?;

            // Serialize to TOML
            let toml_string = toml::to_string_pretty(self)?;

            // Write to temp file
            fs::write(&write_name, toml_string)?;

            // Move temp file to the real file
            fs::rename(&write_name, &file_name)?;
        }
        Ok(())
    }

    pub fn get_theme(&self) -> iced::Theme {
        match self.theme.as_str() {
            "Light" => iced::Theme::Light,
            "Dark" => iced::Theme::Dark,
            "Dracula" => iced::Theme::Dracula,
            "Nord" => iced::Theme::Nord,
            "SolarizedLight" => iced::Theme::SolarizedLight,
            "SolarizedDark" => iced::Theme::SolarizedDark,
            "GruvboxLight" => iced::Theme::GruvboxLight,
            "GruvboxDark" => iced::Theme::GruvboxDark,
            "Ferra" => iced::Theme::Ferra,
            "CatppuccinLatte" => iced::Theme::CatppuccinLatte,
            "CatppuccinFrappe" => iced::Theme::CatppuccinFrappe,
            "CatppuccinMacchiato" => iced::Theme::CatppuccinMacchiato,
            "CatppuccinMocha" => iced::Theme::CatppuccinMocha,
            "TokyoNight" => iced::Theme::TokyoNight,
            "TokyoNightStorm" => iced::Theme::TokyoNightStorm,
            "TokyoNightLight" => iced::Theme::TokyoNightLight,
            "KanagawaWave" => iced::Theme::KanagawaWave,
            "KanagawaDragon" => iced::Theme::KanagawaDragon,
            "KanagawaLotus" => iced::Theme::KanagawaLotus,
            "Moonfly" => iced::Theme::Moonfly,
            "Nightfly" => iced::Theme::Nightfly,
            "Oxocarbon" => iced::Theme::Oxocarbon,
            // Default to Dark theme if theme string is empty or unrecognized
            _ => iced::Theme::Dark,
        }
    }

    pub fn set_theme(&mut self, theme: iced::Theme) {
        self.theme = match theme {
            iced::Theme::Light => "Light",
            iced::Theme::Dark => "Dark",
            iced::Theme::Dracula => "Dracula",
            iced::Theme::Nord => "Nord",
            iced::Theme::SolarizedLight => "SolarizedLight",
            iced::Theme::SolarizedDark => "SolarizedDark",
            iced::Theme::GruvboxLight => "GruvboxLight",
            iced::Theme::GruvboxDark => "GruvboxDark",
            iced::Theme::CatppuccinLatte => "CatppuccinLatte",
            iced::Theme::CatppuccinFrappe => "CatppuccinFrappe",
            iced::Theme::CatppuccinMacchiato => "CatppuccinMacchiato",
            iced::Theme::CatppuccinMocha => "CatppuccinMocha",
            iced::Theme::TokyoNight => "TokyoNight",
            iced::Theme::TokyoNightStorm => "TokyoNightStorm",
            iced::Theme::TokyoNightLight => "TokyoNightLight",
            iced::Theme::KanagawaWave => "KanagawaWave",
            iced::Theme::KanagawaDragon => "KanagawaDragon",
            iced::Theme::KanagawaLotus => "KanagawaLotus",
            iced::Theme::Moonfly => "Moonfly",
            iced::Theme::Nightfly => "Nightfly",
            iced::Theme::Oxocarbon => "Oxocarbon",
            iced::Theme::Custom(_) => "Dark",
            iced::Theme::Ferra => "Ferra",
        }
        .to_string();
    }

    pub(crate) fn reset_monitor_settings(&mut self) {
        //        self.scaling = Scaling::Nearest;
        //        self.monitor_settings = MonitorSettings::default();
    }
    /*
    pub(crate) fn reset_keybindings(&mut self) {
        self.bind = KeyBindings::default();
    }*/
}
