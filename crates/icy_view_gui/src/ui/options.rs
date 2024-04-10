use serde::{Deserialize, Serialize};
use std::fs;

const SCROLL_SPEED: [f32; 3] = [80.0, 160.0, 320.0];

#[derive(Serialize, Deserialize, Debug)]
pub enum ScrollSpeed {
    Slow,
    Medium,
    Fast,
}

impl ScrollSpeed {
    pub fn get_speed(&self) -> f32 {
        match self {
            ScrollSpeed::Slow => SCROLL_SPEED[0],
            ScrollSpeed::Medium => SCROLL_SPEED[1],
            ScrollSpeed::Fast => SCROLL_SPEED[2],
        }
    }

    pub(crate) fn next(&self) -> ScrollSpeed {
        match self {
            ScrollSpeed::Slow => ScrollSpeed::Medium,
            ScrollSpeed::Medium => ScrollSpeed::Fast,
            ScrollSpeed::Fast => ScrollSpeed::Slow,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Options {
    pub auto_scroll_enabled: bool,
    pub scroll_speed: ScrollSpeed,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            auto_scroll_enabled: true,
            scroll_speed: ScrollSpeed::Medium,
        }
    }
}

impl Options {
    pub fn load_options() -> Self {
        if let Some(proj_dirs) = directories::ProjectDirs::from("com", "GitHub", "icy_view") {
            if !proj_dirs.config_dir().exists() && fs::create_dir_all(proj_dirs.config_dir()).is_err() {
                log::error!("Can't create configuration directory {:?}", proj_dirs.config_dir());
                return Self::default();
            }
            let options_file = proj_dirs.config_dir().join("options.toml");
            if options_file.exists() {
                match fs::read_to_string(options_file) {
                    Ok(txt) => {
                        if let Ok(result) = toml::from_str(&txt) {
                            return result;
                        }
                    }
                    Err(err) => log::error!("Error reading options file: {}", err),
                }
            }
        }
        Self::default()
    }

    pub fn store_options(&self) {
        if let Some(proj_dirs) = directories::ProjectDirs::from("com", "GitHub", "icy_view") {
            let file_name = proj_dirs.config_dir().join("options.toml");
            match toml::to_string(self) {
                Ok(text) => {
                    if let Err(err) = fs::write(file_name, text) {
                        log::error!("Error writing options file: {}", err);
                    }
                }
                Err(err) => log::error!("Error writing options file: {}", err),
            }
        }
    }
}
