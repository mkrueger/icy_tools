use directories::ProjectDirs;
use icy_engine::Color;
use icy_engine_gui::{BackgroundEffect, MonitorSettings};
use serde::{Deserialize, Serialize};
use std::{
    error::Error,
    fs::File,
    io::{self, BufReader, BufWriter},
    path::PathBuf,
};

use crate::TerminalResult;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Settings {
    #[serde(default)]
    pub is_dark_mode: Option<bool>,
    pub monitor_settings: MonitorSettings,
}

impl Settings {
    pub(crate) fn get_settings_file() -> TerminalResult<PathBuf> {
        if let Some(proj_dirs) = ProjectDirs::from("com", "GitHub", "icy_view") {
            let dir = proj_dirs.config_dir().join("settings.json");
            return Ok(dir);
        }
        Err(IcyViewError::ErrorCreatingDirectory("settings file".to_string()).into())
    }

    pub(crate) fn load(path: &PathBuf) -> io::Result<Settings> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);

        let result: Settings = serde_json::from_reader(reader)?;

        Ok(result)
    }

    pub(crate) fn save() -> io::Result<()> {
        let Ok(path) = Settings::get_settings_file() else {
            return Ok(());
        };

        unsafe {
            let file = File::create(path)?;
            let reader = BufWriter::new(file);

            serde_json::to_writer_pretty(reader, &SETTINGS.clone())?;

            Ok(())
        }
    }
}
pub static mut SETTINGS: Settings = Settings {
    is_dark_mode: None,
    monitor_settings: MonitorSettings {
        use_filter: false,
        monitor_type: 0,
        gamma: 50.,
        contrast: 50.,
        saturation: 50.,
        brightness: 30.,
        light: 40.,
        blur: 30.,
        curvature: 10.,
        scanlines: 10.,
        background_effect: BackgroundEffect::Checkers,
        selection_fg: Color::new(0xAB, 0x00, 0xAB),
        selection_bg: Color::new(0xAB, 0xAB, 0xAB),
        border_color: Color::new(64, 69, 74),
    },
};

#[derive(Debug, Clone)]
pub enum IcyViewError {
    // Error(String),
    ErrorCreatingDirectory(String),
}

impl std::fmt::Display for IcyViewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // IcyViewError::Error(err) => write!(f, "Error: {err}"),
            IcyViewError::ErrorCreatingDirectory(dir) => {
                write!(f, "Error creating directory: {dir}")
            }
        }
    }
}

impl Error for IcyViewError {
    fn description(&self) -> &str {
        "use std::display"
    }

    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }

    fn cause(&self) -> Option<&dyn Error> {
        self.source()
    }
}
