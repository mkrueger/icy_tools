use directories::ProjectDirs;
use eframe::egui::Modifiers;
use egui::Vec2;
use icy_engine::{Color, SaveOptions};
use icy_engine_gui::{BackgroundEffect, MarkerSettings, MonitorSettings};
use serde::{Deserialize, Serialize};
use std::{
    error::Error,
    fs::{self, File},
    io::{self, BufReader, BufWriter},
    path::{Path, PathBuf},
};

use crate::TerminalResult;

const MAX_RECENT_FILES: usize = 10;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Settings {
    font_outline_style: usize,
    character_set: usize,

    #[serde(default)]
    pub is_dark_mode: Option<bool>,

    pub show_layer_borders: bool,
    pub show_line_numbers: bool,

    pub monitor_settings: MonitorSettings,
    pub marker_settings: MarkerSettings,
    pub save_options: SaveOptions,

    #[serde(default)]
    scale: Vec2,

    #[serde(skip_serializing)]
    pub character_sets: CharacterSets,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            font_outline_style: Default::default(),
            character_set: Default::default(),
            is_dark_mode: Default::default(),
            show_layer_borders: Default::default(),
            show_line_numbers: Default::default(),
            monitor_settings: Default::default(),
            marker_settings: Default::default(),
            save_options: Default::default(),
            scale: Vec2::splat(2.0),
            character_sets: Default::default(),
        }
    }
}

#[derive(Serialize, Clone, Deserialize, Debug)]
pub struct KeyBindings {
    pub key_bindings: Vec<(String, eframe::egui::Key, Modifiers)>,
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self {
            key_bindings: super::Commands::default_keybindings(),
        }
    }
}

impl KeyBindings {
    pub fn get_keybindings_file() -> TerminalResult<PathBuf> {
        if let Some(proj_dirs) = ProjectDirs::from("com", "GitHub", "icy_draw") {
            let dir = proj_dirs.config_dir().join("key_bindings.json");
            return Ok(dir);
        }
        Err(IcyDrawError::ErrorCreatingDirectory("key_bindings".to_string()).into())
    }

    pub fn load(path: &PathBuf) -> io::Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        // Read the JSON contents of the file as an instance of `User`.
        let u = serde_json::from_reader(reader)?;
        // Return the `User`.
        Ok(u)
    }

    pub fn save(&self) -> io::Result<()> {
        let Ok(path) = KeyBindings::get_keybindings_file() else {
            return Ok(());
        };

        let file = File::create(path)?;
        let reader = BufWriter::new(file);
        serde_json::to_writer_pretty(reader, &self.clone())?;
        Ok(())
    }
}

#[derive(Default, Clone, Serialize, Deserialize, Debug)]
pub struct CharacterSets {
    pub character_sets: Vec<CharSetMapping>,
}

impl CharacterSets {
    pub fn get_character_sets_file() -> TerminalResult<PathBuf> {
        if let Some(proj_dirs) = ProjectDirs::from("com", "GitHub", "icy_draw") {
            let dir = proj_dirs.config_dir().join("character_sets.json");
            return Ok(dir);
        }
        Err(IcyDrawError::ErrorCreatingDirectory("character_sets".to_string()).into())
    }

    pub fn load(path: &PathBuf) -> io::Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        // Read the JSON contents of the file as an instance of `User`.
        let u = serde_json::from_reader(reader)?;
        // Return the `User`.
        Ok(u)
    }

    pub fn save(&self) -> io::Result<()> {
        let Ok(path) = CharacterSets::get_character_sets_file() else {
            return Ok(());
        };

        let file = File::create(path)?;
        let reader = BufWriter::new(file);
        serde_json::to_writer_pretty(reader, &self.clone())?;
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CharSetMapping {
    pub font_checksum: u32,
    pub table: Vec<Vec<char>>,
}

impl Default for CharSetMapping {
    fn default() -> Self {
        let mut default_char_set = CharSetMapping {
            font_checksum: 0,
            table: Vec::new(),
        };
        for i in crate::DEFAULT_CHAR_SET_TABLE {
            default_char_set.table.push(i.iter().fold(Vec::new(), |mut s, c| {
                s.push(*c as char);
                s
            }));
        }
        default_char_set
    }
}

impl Settings {
    pub fn set_font_outline_style(font_outline_style: usize) {
        unsafe {
            SETTINGS.font_outline_style = font_outline_style;
        }
    }

    pub fn get_font_outline_style() -> usize {
        unsafe { SETTINGS.font_outline_style }
    }

    pub fn get_character_set_char(&self, checksum: u32, ch: usize) -> char {
        let mut table_idx = 0;

        for (i, set) in self.character_sets.character_sets.iter().enumerate() {
            if set.font_checksum == checksum {
                table_idx = i;
                break;
            }
        }
        let char_set = &self.character_sets.character_sets[table_idx];
        if self.character_set >= char_set.table.len() || ch >= char_set.table[self.character_set].len() {
            return ' ';
        }
        char_set.table[self.character_set][ch]
    }

    pub fn set_character_set(character_set: usize) {
        unsafe {
            SETTINGS.character_set = character_set;
        }
    }

    pub fn get_character_set() -> usize {
        unsafe { SETTINGS.character_set }
    }

    pub(crate) fn get_font_diretory() -> TerminalResult<PathBuf> {
        if let Some(proj_dirs) = ProjectDirs::from("com", "GitHub", "icy_draw") {
            let dir = proj_dirs.config_dir().join("data/fonts");
            if !dir.exists() && fs::create_dir_all(&dir).is_err() {
                return Err(IcyDrawError::ErrorCreatingDirectory(format!("{dir:?}")).into());
            }
            return Ok(dir);
        }
        Err(IcyDrawError::ErrorCreatingDirectory("font directory".to_string()).into())
    }

    pub(crate) fn get_tdf_diretory() -> TerminalResult<PathBuf> {
        if let Some(proj_dirs) = ProjectDirs::from("com", "GitHub", "icy_draw") {
            let dir = proj_dirs.config_dir().join("data/tdf");

            if !dir.exists() && fs::create_dir_all(&dir).is_err() {
                return Err(IcyDrawError::ErrorCreatingDirectory(format!("{dir:?}")).into());
            }
            return Ok(dir);
        }
        Err(IcyDrawError::ErrorCreatingDirectory("tdf directory".to_string()).into())
    }

    pub(crate) fn get_palettes_diretory() -> TerminalResult<PathBuf> {
        if let Some(proj_dirs) = ProjectDirs::from("com", "GitHub", "icy_draw") {
            let dir = proj_dirs.config_dir().join("data/palettes");

            if !dir.exists() {
                if fs::create_dir_all(&dir).is_err() {
                    return Err(IcyDrawError::ErrorCreatingDirectory(format!("{dir:?}")).into());
                }
                fs::write(dir.join("ansi32.gpl"), include_bytes!("../../data/palettes/ansi32.gpl"))?;
                fs::write(dir.join("ANSI32-EHB-64.gpl"), include_bytes!("../../data/palettes/ANSI32-EHB-64.gpl"))?;
            }

            return Ok(dir);
        }
        Err(IcyDrawError::ErrorCreatingDirectory("font directory".to_string()).into())
    }

    pub(crate) fn get_auto_save_diretory() -> TerminalResult<PathBuf> {
        if let Some(proj_dirs) = ProjectDirs::from("com", "GitHub", "icy_draw") {
            let dir = proj_dirs.config_dir().join("autosave");

            if !dir.exists() && fs::create_dir_all(&dir).is_err() {
                return Err(IcyDrawError::ErrorCreatingDirectory(format!("{dir:?}")).into());
            }
            return Ok(dir);
        }
        Err(IcyDrawError::ErrorCreatingDirectory("autosave directory".to_string()).into())
    }

    pub(crate) fn get_log_file() -> TerminalResult<PathBuf> {
        if let Some(proj_dirs) = ProjectDirs::from("com", "GitHub", "icy_draw") {
            let dir = proj_dirs.config_dir().join("icy_draw.log");
            return Ok(dir);
        }
        Err(IcyDrawError::ErrorCreatingDirectory("log_file".to_string()).into())
    }

    pub(crate) fn get_settings_file() -> TerminalResult<PathBuf> {
        if let Some(proj_dirs) = ProjectDirs::from("com", "GitHub", "icy_draw") {
            let dir = proj_dirs.config_dir().join("settings.json");
            return Ok(dir);
        }
        Err(IcyDrawError::ErrorCreatingDirectory("settings file".to_string()).into())
    }

    pub(crate) fn get_plugin_directory() -> TerminalResult<PathBuf> {
        if let Some(proj_dirs) = ProjectDirs::from("com", "GitHub", "icy_draw") {
            let dir = proj_dirs.config_dir().join("data/plugins");

            if !dir.exists() {
                if fs::create_dir_all(&dir).is_err() {
                    return Err(IcyDrawError::ErrorCreatingDirectory(format!("{dir:?}")).into());
                }
                fs::write(dir.join("elite-writing.lua"), include_bytes!("../plugins/elite-writing.lua.txt"))?;
            }
            return Ok(dir);
        }
        Err(IcyDrawError::ErrorCreatingDirectory("plugin directory".to_string()).into())
    }

    pub(crate) fn load(path: &PathBuf) -> io::Result<Settings> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);

        // Read the JSON contents of the file as an instance of `User`.
        let mut result: Settings = serde_json::from_reader(reader)?;

        if let Ok(settings_file) = CharacterSets::get_character_sets_file() {
            if settings_file.exists() {
                if let Ok(character_sets) = CharacterSets::load(&settings_file) {
                    result.character_sets = character_sets;
                }
            }
        }

        // Return the `User`.
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

    pub(crate) fn get_theme(&self) -> egui::Visuals {
        let is_dark = if let Some(dark_mode) = unsafe { SETTINGS.is_dark_mode } {
            dark_mode
        } else {
            dark_light::detect() != dark_light::Mode::Light
        };

        if is_dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        }
    }

    fn clamp_scale(&mut self) {
        self.scale = self.scale.clamp(Vec2::new(0.5, 0.5), Vec2::new(4., 4.));
    }

    pub fn get_scale(&mut self) -> Vec2 {
        self.clamp_scale();
        self.scale
    }

    pub fn set_scale(&mut self, scale: Vec2) {
        self.scale = (scale * 100.0).floor() / 100.0;
        self.clamp_scale();
        Settings::save().unwrap();
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MostRecentlyUsedFiles {
    files: Vec<PathBuf>,
}

impl MostRecentlyUsedFiles {
    pub fn get_mru_file() -> TerminalResult<PathBuf> {
        if let Some(proj_dirs) = ProjectDirs::from("com", "GitHub", "icy_draw") {
            let dir = proj_dirs.config_dir().join("recent_files.json");
            return Ok(dir);
        }
        Err(IcyDrawError::ErrorCreatingDirectory("mru file".to_string()).into())
    }

    pub fn get_recent_files(&mut self) -> &Vec<PathBuf> {
        self.files.retain(|p| p.exists());
        &self.files
    }

    pub fn add_recent_file(&mut self, file: &Path) {
        let file = file.to_path_buf();
        for i in 0..self.files.len() {
            if self.files[i] == file {
                self.files.remove(i);
                break;
            }
        }

        self.files.push(file);
        while self.files.len() > MAX_RECENT_FILES {
            self.files.remove(0);
        }
        if let Err(err) = self.save() {
            log::error!("Error saving recent files: {}", err);
        }
    }

    pub fn clear_recent_files(&mut self) {
        self.files.clear();
        if let Err(err) = self.save() {
            log::error!("Error saving recent files: {}", err);
        }
    }

    pub(crate) fn load(path: &PathBuf) -> io::Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        Ok(serde_json::from_reader(reader)?)
    }

    pub(crate) fn save(&self) -> io::Result<()> {
        let Ok(path) = MostRecentlyUsedFiles::get_mru_file() else {
            return Ok(());
        };

        let file = File::create(path)?;
        let reader = BufWriter::new(file);
        serde_json::to_writer_pretty(reader, &self)?;
        Ok(())
    }
}

pub static mut MRU_FILES: MostRecentlyUsedFiles = MostRecentlyUsedFiles { files: Vec::new() };

pub static mut SETTINGS: Settings = Settings {
    font_outline_style: 0,
    character_set: 5,
    show_layer_borders: true,
    show_line_numbers: false,
    save_options: SaveOptions::new(),
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
    marker_settings: MarkerSettings {
        reference_image_alpha: 0.2,
        raster_alpha: 0.2,
        raster_color: Color::new(0xAB, 0xAB, 0xAB),
        guide_alpha: 0.2,
        guide_color: Color::new(0xAB, 0xAB, 0xAB),
    },
    scale: Vec2::splat(2.0),
    character_sets: CharacterSets { character_sets: Vec::new() },
};

#[derive(Debug, Clone)]
pub enum IcyDrawError {
    Error(String),
    ErrorCreatingDirectory(String),
}

impl std::fmt::Display for IcyDrawError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IcyDrawError::Error(err) => write!(f, "Error: {err}"),
            IcyDrawError::ErrorCreatingDirectory(dir) => {
                write!(f, "Error creating directory: {dir}")
            }
        }
    }
}

impl Error for IcyDrawError {
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
