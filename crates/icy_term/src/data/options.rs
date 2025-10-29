use std::{
    fs::{self},
    time::Duration,
};

use iced_engine_gui::MonitorSettings;
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

    // pub scaling: Scaling,
    pub monitor_settings: MonitorSettings,

    // pub bind: KeyBindings,
    #[serde(default)]
    pub iemsi: IEMSISettings,

    // pub window_rect: Option<Rect>,
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
            monitor_settings: MonitorSettings::default(),
            iemsi: IEMSISettings::default(),
            console_beep: true,
            //            bind: KeyBindings::default(),
            is_dark_mode: None,
            //            window_rect: None,
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

    pub(crate) fn reset_monitor_settings(&mut self) {
        //        self.scaling = Scaling::Nearest;
        //        self.monitor_settings = MonitorSettings::default();
    }
    /*
    pub(crate) fn reset_keybindings(&mut self) {
        self.bind = KeyBindings::default();
    }*/
}
