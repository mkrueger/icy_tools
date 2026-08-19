use std::{
    fs::{self},
    path::PathBuf,
    sync::OnceLock,
    time::Duration,
};

static OPTIONS_FILE_OVERRIDE: OnceLock<PathBuf> = OnceLock::new();

use directories::UserDirs;
use icy_engine_gui::{music::music::DialTone, MonitorSettings};
use icy_net::{modem::ModemConfiguration, serial::Serial};
use icy_parser_core::CaretShape;
use serde::{Deserialize, Serialize};

use crate::{default_protocols, TerminalResult, TransferProtocol};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebDirectorySource {
    pub name: String,
    pub url: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

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

    #[serde(default = "default_true")]
    pub audio_enabled: bool,

    #[serde(default = "default_master_volume")]
    pub master_volume: f32,

    #[serde(default)]
    pub audio_device: Option<String>,

    #[serde(default)]
    pub invert_mouse_wheel: bool,

    #[serde(default)]
    pub default_cursor_shape: CaretShape,

    #[serde(default = "default_true")]
    pub default_cursor_blinking: bool,

    #[serde(default)]
    pub web_directories: Vec<WebDirectorySource>,

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
    pub modems: Vec<ModemConfiguration>,

    #[serde(default = "default_max_scrollback_lines")]
    pub max_scrollback_lines: usize,

    #[serde(default)]
    pub serial: Serial,

    /// External/custom transfer protocols
    #[serde(default = "default_protocols")]
    pub transfer_protocols: Vec<TransferProtocol>,
}

fn default_max_scrollback_lines() -> usize {
    2000
}

fn default_connect_timeout() -> Duration {
    Duration::from_secs(1000)
}

fn default_master_volume() -> f32 {
    0.25
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
            audio_enabled: true,
            master_volume: default_master_volume(),
            audio_device: None,
            invert_mouse_wheel: false,
            default_cursor_shape: CaretShape::default(),
            default_cursor_blinking: true,
            web_directories: Vec::new(),
            //            bind: KeyBindings::default(),
            is_dark_mode: None,
            //            window_rect: None,
            modems: Vec::new(),
            dial_tone: DialTone::default(),
            capture_path: String::new(),
            download_path: String::new(),
            max_scrollback_lines: 2000,
            serial: Serial::default(),
            transfer_protocols: default_protocols(),
        }
    }
}

impl Options {
    pub fn set_options_file(path: PathBuf) {
        let _ = OPTIONS_FILE_OVERRIDE.set(path);
    }

    fn options_file() -> Option<PathBuf> {
        OPTIONS_FILE_OVERRIDE
            .get()
            .cloned()
            .or_else(|| directories::ProjectDirs::from("com", "GitHub", "icy_term").map(|dirs| dirs.config_dir().join("options.toml")))
    }

    /// Returns the log directory path (for flexi_logger configuration).
    pub fn get_log_dir() -> Option<PathBuf> {
        directories::ProjectDirs::from("com", "GitHub", "icy_term").map(|proj_dirs| proj_dirs.config_dir().to_path_buf())
    }

    /// Returns the path to the current log file.
    /// On Windows, this returns the rotated file name (icy_term_rCURRENT.log)
    /// since symlinks don't work reliably there.
    /// On other platforms, this returns the symlink (icy_term.log).
    pub fn get_log_file() -> Option<PathBuf> {
        Self::get_log_dir().map(|log_dir| {
            if cfg!(windows) {
                log_dir.join("icy_term_rCURRENT.log")
            } else {
                log_dir.join("icy_term.log")
            }
        })
    }

    /// .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn load_options() -> TerminalResult<Self> {
        if let Some(options_file) = Self::options_file() {
            if options_file.exists() {
                let content = fs::read_to_string(&options_file)?;
                let mut options: Options = toml::from_str(&content)?;
                for protocol in default_protocols() {
                    if !options.transfer_protocols.iter().any(|existing| existing.id == protocol.id) {
                        options.transfer_protocols.push(protocol);
                    }
                }
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
        if let Some(file_name) = Self::options_file() {
            let mut write_name = file_name.clone();
            write_name.set_extension("new");

            // Create config directory if it doesn't exist
            if let Some(parent) = file_name.parent() {
                fs::create_dir_all(parent)?;
            }

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

#[cfg(test)]
mod tests {
    use super::Options;

    #[test]
    fn audio_settings_roundtrip() {
        let mut options = Options::default();
        options.audio_enabled = false;
        options.master_volume = 0.42;
        options.audio_device = Some("Test Device".to_string());

        let encoded = toml::to_string(&options).unwrap();
        let decoded: Options = toml::from_str(&encoded).unwrap();

        assert!(!decoded.audio_enabled);
        assert_eq!(decoded.master_volume, 0.42);
        assert_eq!(decoded.audio_device.as_deref(), Some("Test Device"));
    }

    #[test]
    fn cursor_settings_roundtrip() {
        let mut options = Options::default();
        options.default_cursor_shape = icy_parser_core::CaretShape::Bar;
        options.default_cursor_blinking = false;

        let encoded = toml::to_string(&options).unwrap();
        let decoded: Options = toml::from_str(&encoded).unwrap();

        assert_eq!(decoded.default_cursor_shape, icy_parser_core::CaretShape::Bar);
        assert!(!decoded.default_cursor_blinking);
    }
}
