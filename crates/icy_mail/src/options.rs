//! User settings of the mail reader, kept in `settings.toml` in the configuration directory.

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use i18n_embed_fl::fl;
use icy_engine_gui::{MonitorSettings, ScalingMode};
use serde::{Deserialize, Serialize};

use crate::{
    drafts::atomic_write,
    reader::{SearchFields, ViewMode},
    writing::{validate_quote_header, DEFAULT_QUOTE_HEADER},
    LANGUAGE_LOADER,
};

const FILE_NAME: &str = "settings.toml";
pub const DEFAULT_MODERN_FONT_SIZE: f32 = 15.0;
pub const MODERN_FONT_SIZES: std::ops::RangeInclusive<f32> = 11.0..=24.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

/// Where the message is shown relative to the message list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReadingPane {
    /// To the right on wide windows, below the list otherwise.
    #[default]
    Automatic,
    Below,
    Right,
}

/// How message text is shown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReadingMode {
    /// The 80 column terminal with the BBS font, as on the board.
    Classic,
    /// Selectable text that wraps to the pane and follows the theme; ANSI art keeps the BBS font.
    #[default]
    Modern,
}

/// Font of the modern reading mode; graphics and aligned lines always use the fixed-width font.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModernFont {
    Proportional,
    #[default]
    Monospace,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Options {
    pub theme: Theme,
    /// Monitor emulation and zoom of the message terminal.
    pub monitor_settings: MonitorSettings,
    /// New messages start with a random tagline from the tagline list.
    pub random_tagline: bool,
    /// Plain message footer seeded into new drafts, independently of their body and tagline.
    pub signature: String,
    /// Quote attribution template using original message metadata; empty uses the localized default.
    pub quote_header: String,
    /// Message list shown flat or as threads.
    pub view_mode: ViewMode,
    pub reading_pane: ReadingPane,
    /// The sidebar lists only conferences with unread messages.
    pub conferences_unread_only: bool,
    pub reading_mode: ReadingMode,
    pub modern_font: ModernFont,
    pub modern_font_size: f32,
    /// Render ANSI art as images; otherwise keep its Unicode characters as selectable text.
    pub modern_art_images: bool,
    /// Parts of the messages the search looks at.
    pub search_fields: SearchFields,
    /// Days since last use to retain packet extractions; zero disables the disk cache.
    pub extraction_cache_days: u32,
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
            signature: String::new(),
            quote_header: DEFAULT_QUOTE_HEADER.to_string(),
            view_mode: ViewMode::default(),
            reading_pane: ReadingPane::default(),
            conferences_unread_only: false,
            reading_mode: ReadingMode::default(),
            modern_font: ModernFont::default(),
            modern_font_size: DEFAULT_MODERN_FONT_SIZE,
            modern_art_images: true,
            search_fields: SearchFields::default(),
            extraction_cache_days: 30,
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
            Ok(content) => {
                let options: Self = toml::from_str(&content)?;
                validate_quote_header(&options.quote_header)?;
                Ok(options)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error.into()),
        }
    }

    pub fn save_in(&self, directory: &Path) -> crate::Res<()> {
        validate_quote_header(&self.quote_header)?;
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
        assert_eq!(Options::default().extraction_cache_days, 30);
        let mut options = Options {
            theme: Theme::Dark,
            ..Default::default()
        };
        options.monitor_settings.scaling_mode = ScalingMode::Manual(2.0);
        options.monitor_settings.use_scanlines = true;
        options.view_mode = ViewMode::Threads;
        options.reading_pane = ReadingPane::Right;
        options.reading_mode = ReadingMode::Classic;
        options.modern_font = ModernFont::Proportional;
        options.modern_font_size = 18.0;
        options.modern_art_images = false;
        options.search_fields.text = false;
        options.extraction_cache_days = 0;
        options.signature = "Regards,\nJörg".to_string();
        options.quote_header = "On {date}, {author} wrote about {subject}:".to_string();
        options.save_in(dir.path()).unwrap();
        assert_eq!(Options::load_in(dir.path()).unwrap(), options);
        fs::write(dir.path().join(FILE_NAME), "theme = \"Light\"\n").unwrap();
        let partial = Options::load_in(dir.path()).unwrap();
        assert_eq!(partial.theme, Theme::Light);
        assert_eq!(partial.monitor_settings, Options::default().monitor_settings);
        assert!(partial.random_tagline);
        assert!(partial.signature.is_empty());
        assert_eq!(partial.quote_header, DEFAULT_QUOTE_HEADER);
        assert_eq!(partial.view_mode, ViewMode::List);
        assert_eq!(partial.reading_pane, ReadingPane::Automatic);
        assert_eq!(partial.reading_mode, ReadingMode::Modern, "new installations read in the modern mode");
        assert_eq!(partial.modern_font, ModernFont::Monospace);
        assert!(partial.modern_art_images, "older settings keep ANSI image rendering enabled");
        assert!(partial.search_fields.all(), "search looks everywhere until restricted");
        assert_eq!(partial.extraction_cache_days, 30, "older settings inherit the cache retention default");
    }

    #[test]
    fn invalid_quote_attribution_is_not_saved_or_loaded() {
        let (dir, _package) = crate::qwk::tests::load();
        let options = Options {
            quote_header: "{unsupported}".to_string(),
            ..Default::default()
        };
        assert!(options.save_in(dir.path()).is_err());
        assert!(!dir.path().join(FILE_NAME).exists());
        fs::write(dir.path().join(FILE_NAME), "quote_header = \"{author\"\n").unwrap();
        assert!(Options::load_in(dir.path()).is_err());
    }
}
