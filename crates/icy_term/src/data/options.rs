use std::{
    fs::{self},
    path::PathBuf,
    time::Duration,
};

use directories::UserDirs;
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

#[derive(Default, Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum DialTone {
    /// 350 + 440 Hz dial tone
    #[default]
    US,
    /// 350 + 450 Hz dial tone
    UK,
    /// Europe 425 Hz
    Europe,
    /// France 440 Hz
    France,
    /// Japan 400 Hz
    Japan,
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

    #[serde(default)]
    pub dial_tone: DialTone,

    /// The path where the capture files are stored in. Defaults to documents
    #[serde(default)]
    pub capture_path: String,

    /// The  path downloads are stored in. Defaults to downloads.
    #[serde(default)]
    pub download_path: String,

    // pub window_rect: Option<Rect>,
    #[serde(default)]
    pub modems: Vec<Modem>,

    #[serde(default)]
    pub max_scrollback_lines: usize,
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
            dial_tone: DialTone::default(),
            capture_path: String::new(),
            download_path: String::new(),
            max_scrollback_lines: 2000,
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

    pub fn capture_path(&self) -> String {
        if self.capture_path.is_empty() {
            Self::default_capture_directory().to_string_lossy().to_string()
        } else {
            self.capture_path.clone()
        }
    }

    pub fn download_path(&self) -> String {
        if self.download_path.is_empty() {
            Self::download_directory().to_string_lossy().to_string()
        } else {
            self.download_path.clone()
        }
    }

    pub fn default_capture_directory() -> PathBuf {
        directories::UserDirs::new()
            .and_then(|dirs| dirs.document_dir().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }

    pub fn download_directory() -> PathBuf {
        if let Some(dirs) = UserDirs::new() {
            if let Some(upload_location) = dirs.download_dir() {
                return upload_location.to_path_buf();
            }
        }
        PathBuf::from(".")
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
        self.monitor_settings = MonitorSettings::default();
    }
    /*
    pub(crate) fn reset_keybindings(&mut self) {
        self.bind = KeyBindings::default();
    }*/
}
