#![cfg_attr(not(debug_assertions), deny(warnings))] // Forbid warnings in release builds
#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::path::PathBuf;

mod model;
mod paint;
mod plugins;
mod ui;
mod util;

use log::LevelFilter;
use log4rs::{
    append::{
        console::{ConsoleAppender, Target},
        file::FileAppender,
    },
    config::{Appender, Config, Root},
    encode::pattern::PatternEncoder,
    filter::threshold::ThresholdFilter,
};

use i18n_embed::{
    fluent::{fluent_language_loader, FluentLanguageLoader},
    DesktopLanguageRequester,
};
use rust_embed::RustEmbed;
use semver::Version;
pub use ui::*;

lazy_static::lazy_static! {
    static ref VERSION: Version = Version::parse( env!("CARGO_PKG_VERSION")).unwrap();
    static ref DEFAULT_TITLE: String = format!("iCY DRAW {}", *crate::VERSION);
}

lazy_static::lazy_static! {
    static ref LATEST_VERSION: Version = {
        let github = github_release_check::GitHub::new().unwrap();
        if let Ok(ver) = github.get_all_versions("mkrueger/icy_tools") {
            for v in ver {
                if v.starts_with("IcyDraw") {
                    return Version::parse(&v[7..]).unwrap();
                }
            }
        }
        VERSION.clone()
    };
}

#[derive(RustEmbed)]
#[folder = "i18n"] // path to the compiled localization resources
struct Localizations;

use once_cell::sync::Lazy;
static LANGUAGE_LOADER: Lazy<FluentLanguageLoader> = Lazy::new(|| {
    let loader = fluent_language_loader!();
    let requested_languages = DesktopLanguageRequester::requested_languages();
    let _result = i18n_embed::select(&loader, &Localizations, &requested_languages);
    loader
});
use clap::Parser;

#[derive(Parser, Debug)]
pub struct Cli {
    path: Option<PathBuf>,
}

// When compiling natively:
#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use std::fs;

    let args = Cli::parse();

    use eframe::icon_data::from_png_bytes;

    use crate::plugins::Plugin;
    let mut native_options = eframe::NativeOptions {
        multisampling: 0,
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    let icon_data = from_png_bytes(include_bytes!("../build/linux/256x256.png")).unwrap();
    native_options.viewport = native_options.viewport.with_inner_size(egui::vec2(1284. + 8., 839.)).with_icon(icon_data);

    if let Ok(log_file) = Settings::get_log_file() {
        // delete log file when it is too big
        if let Ok(data) = fs::metadata(&log_file) {
            if data.len() > 1024 * 256 {
                fs::remove_file(&log_file).unwrap();
            }
        }

        let level = log::LevelFilter::Info;

        // Build a stderr logger.
        let stderr = ConsoleAppender::builder().target(Target::Stderr).build();

        // Logging to log file.
        let logfile = FileAppender::builder()
            // Pattern: https://docs.rs/log4rs/*/log4rs/encode/pattern/index.html
            .encoder(Box::new(PatternEncoder::new("{l} - {m}\n")))
            .build(log_file)
            .unwrap();

        let config = Config::builder()
            .appender(Appender::builder().build("logfile", Box::new(logfile)))
            .appender(
                Appender::builder()
                    .filter(Box::new(ThresholdFilter::new(level)))
                    .build("stderr", Box::new(stderr)),
            )
            .build(Root::builder().appender("logfile").appender("stderr").build(LevelFilter::Info))
            .unwrap();

        // Use this to change log levels at runtime.
        // This means you can change the default log level to trace
        // if you are trying to debug an issue and need more logs on then turn it off
        // once you are done.
        let _handle = log4rs::init_config(config);
    } else {
        eprintln!("Failed to create log file");
    }

    if let Ok(settings_file) = Settings::get_settings_file() {
        if settings_file.exists() {
            if let Ok(settings) = Settings::load(&settings_file) {
                unsafe {
                    SETTINGS = settings;
                }
            }
        }
    }

    if let Ok(settings_file) = KeyBindings::get_keybindings_file() {
        if settings_file.exists() {
            if let Ok(key_bindings) = KeyBindings::load(&settings_file) {
                unsafe {
                    KEYBINDINGS = key_bindings;
                }
            }
        }
    }

    if let Ok(settings_file) = CharacterSets::get_character_sets_file() {
        if settings_file.exists() {
            if let Ok(character_sets) = CharacterSets::load(&settings_file) {
                unsafe {
                    CHARACTER_SETS = character_sets;
                }
            }
        }
    }

    if let Ok(settings_file) = MostRecentlyUsedFiles::get_mru_file() {
        if settings_file.exists() {
            if let Ok(character_sets) = MostRecentlyUsedFiles::load(&settings_file) {
                unsafe {
                    MRU_FILES = character_sets;
                }
            }
        }
    }

    unsafe {
        if KEYBINDINGS.key_bindings.is_empty() {
            KEYBINDINGS.key_bindings = Commands::default_keybindings();
        }

        if CHARACTER_SETS.character_sets.is_empty() {
            CHARACTER_SETS.character_sets.push(CharSetMapping::default());
        }
    }

    log::info!("Starting iCY DRAW {}", *VERSION);
    Plugin::read_plugin_directory();
    if let Err(err) = eframe::run_native(
        &DEFAULT_TITLE,
        native_options,
        Box::new(|cc| {
            let mut window = MainWindow::new(cc);
            if let Some(mut path) = args.path {
                if path.is_relative() {
                    path = std::env::current_dir().unwrap().join(path);
                }
                window.open_file(&path, false);
            }
            Box::new(window)
        }),
    ) {
        log::error!("Error returned by run_native: {}", err);
    }
    log::info!("shutting down.");
}
