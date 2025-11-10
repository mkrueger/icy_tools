#![warn(clippy::all, clippy::pedantic)]
#![allow(
    non_upper_case_globals,
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    clippy::struct_excessive_bools,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_lossless
)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

pub mod qwk;
mod ui;

use std::path::PathBuf;

use clap::Parser;
use directories::ProjectDirs;
use lazy_static::lazy_static;
use web_time::Instant;
pub type TerminalResult<T> = Res<T>;
use log4rs::{
    append::{
        console::{ConsoleAppender, Target},
        file::FileAppender,
    },
    config::{Appender, Config, Root},
    encode::pattern::PatternEncoder,
    filter::threshold::ThresholdFilter,
};
use semver::Version;

use crate::ui::window_manager::WindowManager;

pub type Res<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

lazy_static! {
    static ref VERSION: Version = Version::parse(env!("CARGO_PKG_VERSION")).unwrap();
    static ref START_TIME: Instant = Instant::now();
}

lazy_static::lazy_static! {
    static ref LATEST_VERSION: Version = {
        let github = github_release_check::GitHub::new().unwrap();
        if let Ok(ver) = github.get_all_versions("mkrueger/icy_tools") {
            for v in ver {
                if v.starts_with("IcyMail") {
                    return Version::parse(&v[7..]).unwrap();
                }
            }
        }
        VERSION.clone()
    };
}
/*
#[derive(rust_embed::RustEmbed)]
#[folder = "i18n"] // path to the compiled localization resources
struct Localizations;

use once_cell::sync::Lazy;

use crate::ui::WindowManager;

static LANGUAGE_LOADER: Lazy<i18n_embed::fluent::FluentLanguageLoader> = Lazy::new(|| {
    let loader = i18n_embed::fluent::fluent_language_loader!();
    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();
    let _result = i18n_embed::select(&loader, &Localizations, &requested_languages);
    loader
});*/

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Mail package file to open (QWK/REP/ZIP format)
    #[arg(value_name = "FILE")]
    file: Option<PathBuf>,
}

fn get_log_file() -> anyhow::Result<PathBuf> {
    if let Some(proj_dirs) = ProjectDirs::from("com", "GitHub", "icy_mail") {
        let dir = proj_dirs.config_dir().join("icy_mail.log");
        return Ok(dir);
    }
    Err(anyhow::anyhow!("Error getting log directory"))
}

fn main() {
    use std::fs;

    if let Ok(log_file) = get_log_file() {
        // delete log file when it is too big
        if let Ok(data) = fs::metadata(&log_file) {
            if data.len() > 1024 * 256 {
                fs::remove_file(&log_file).unwrap();
            }
        }

        let level = log::LevelFilter::Warn;

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
            .build(Root::builder().appender("logfile").appender("stderr").build(level))
            .unwrap();

        // Use this to change log levels at runtime.
        // This means you can change the default log level to trace
        // if you are trying to debug an issue and need more logs on then turn it off
        // once you are done.
        let _handle = log4rs::init_config(config);
    } else {
        eprintln!("Failed to create log file");
    }

    log::info!("Starting iCY MAIL {}", *VERSION);

    iced::daemon(WindowManager::new, WindowManager::update, WindowManager::view)
        .theme(WindowManager::theme)
        .subscription(WindowManager::subscription) // Add this line
        .title(WindowManager::title)
        .run()
        .expect("Failed to run application");
    log::info!("shutting down.");
}

fn load_window_icon(png_bytes: &[u8]) -> Result<iced::window::Icon, Box<dyn std::error::Error>> {
    // Add `image = "0.24"` (or latest) to Cargo.toml if not present.
    let img = iced::advanced::graphics::image::image_rs::load_from_memory(png_bytes)?;
    let rgba = img.to_rgba8();
    let w = img.width();
    let h = img.height();
    Ok(iced::window::icon::from_rgba(rgba.into_raw(), w, h)?)
}
