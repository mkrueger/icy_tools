//! User settings of the mail reader, kept in `settings.toml` in the configuration directory.

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use i18n_embed_fl::fl;
use icy_engine_gui::{MonitorSettings, ScalingMode};
use serde::{Deserialize, Serialize};

use crate::{drafts::atomic_write, LANGUAGE_LOADER};

const FILE_NAME: &str = "settings.toml";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Options {
    pub theme: Theme,
    /// Monitor emulation and zoom of the message terminal.
    pub monitor_settings: MonitorSettings,
    /// New messages start with a random tagline from the tagline list.
    pub random_tagline: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            // Mail is mostly read at fit width, where integer steps leave large margins.
            monitor_settings: MonitorSettings {
                scaling_mode: ScalingMode::FitWidth,
                use_integer_scaling: false,
                ..Default::default()
            },
            random_tagline: true,
        }
    }
}

impl Options {
    pub fn directory() -> crate::Res<PathBuf> {
        Ok(directories::ProjectDirs::from("com", "GitHub", "icy_mail")
            .ok_or_else(|| fl!(LANGUAGE_LOADER, "text-configuration-directory-unavailable"))?
            .config_dir()
            .to_path_buf())
    }

    /// Settings stored in `directory`, the defaults when there are none yet.
    pub fn load_in(directory: &Path) -> crate::Res<Self> {
        match fs::read_to_string(directory.join(FILE_NAME)) {
            Ok(content) => Ok(toml::from_str(&content)?),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error.into()),
        }
    }

    pub fn save_in(&self, directory: &Path) -> crate::Res<()> {
        fs::create_dir_all(directory)?;
        let content = toml::to_string(self)?;
        atomic_write(&directory.join(FILE_NAME), |file| {
            file.write_all(content.as_bytes())?;
            Ok(())
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_and_default_when_missing() {
        let (dir, _package) = crate::qwk::tests::load();
        assert_eq!(Options::load_in(dir.path()).unwrap(), Options::default());
        assert!(!Options::default().monitor_settings.use_integer_scaling);
        let mut options = Options {
            theme: Theme::Dark,
            ..Default::default()
        };
        options.monitor_settings.scaling_mode = ScalingMode::Manual(2.0);
        options.monitor_settings.use_scanlines = true;
        options.save_in(dir.path()).unwrap();
        assert_eq!(Options::load_in(dir.path()).unwrap(), options);
        fs::write(dir.path().join(FILE_NAME), "theme = \"Light\"\n").unwrap();
        let partial = Options::load_in(dir.path()).unwrap();
        assert_eq!(partial.theme, Theme::Light);
        assert_eq!(partial.monitor_settings, Options::default().monitor_settings);
        assert!(partial.random_tagline);
    }
}
