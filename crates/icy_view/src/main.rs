#![warn(clippy::all, clippy::pedantic)]
#![allow(
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    clippy::struct_excessive_bools,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_lossless
)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

mod commands;
mod items;
mod sort_order;
mod thumbnail;
mod ui;

mod options;
mod window_manager;

pub use options::*;
pub use window_manager::*;

use std::path::PathBuf;

use clap::Parser;
use clap_i18n_richformatter::clap_i18n;
use flexi_logger::{Cleanup, Criterion, FileSpec, Logger, Naming};
use iced::Settings;
use rust_embed::RustEmbed;
use semver::Version;

use i18n_embed::{
    fluent::{fluent_language_loader, FluentLanguageLoader},
    DesktopLanguageRequester,
};

use once_cell::sync::Lazy;

#[derive(RustEmbed)]
#[folder = "i18n"]
struct Localizations;

lazy_static::lazy_static! {
    pub static ref VERSION: Version = Version::parse(env!("CARGO_PKG_VERSION")).unwrap();
    pub static ref DEFAULT_TITLE: String = format!("iCY VIEW {}", *VERSION);

    /// Latest version available on GitHub (checked at startup)
    pub static ref LATEST_VERSION: Version = {
        let github = github_release_check::GitHub::new().unwrap();
        if let Ok(ver) = github.get_all_versions("mkrueger/icy_tools") {
            for v in ver {
                if v.starts_with("IcyView") {
                    if let Ok(parsed) = Version::parse(&v[7..]) {
                        return parsed;
                    }
                }
            }
        }
        VERSION.clone()
    };
}

pub static LANGUAGE_LOADER: Lazy<FluentLanguageLoader> = Lazy::new(|| {
    let loader = fluent_language_loader!();
    let requested_languages = DesktopLanguageRequester::requested_languages();
    let _result = i18n_embed::select(&loader, &Localizations, &requested_languages);
    loader
});

pub type TerminalResult<T> = anyhow::Result<T>;

#[derive(Parser, Debug)]
#[command(version, about = i18n_embed_fl::fl!(LANGUAGE_LOADER, "app-about"), long_about = None)]
#[clap_i18n]
pub struct Args {
    /// Path to file or directory to open
    #[arg(value_name = "PATH", help = i18n_embed_fl::fl!(LANGUAGE_LOADER, "arg-path-help"))]
    path: Option<PathBuf>,

    /// Enable auto-scrolling
    #[clap(long, default_value_t = false, help = i18n_embed_fl::fl!(LANGUAGE_LOADER, "arg-auto-help"))]
    auto: bool,

    /// Baud rate emulation (e.g., 9600, 19200, 38400)
    #[clap(long, value_name = "RATE", help = i18n_embed_fl::fl!(LANGUAGE_LOADER, "arg-bps-help"))]
    bps: Option<u32>,
}

fn get_log_dir() -> Option<PathBuf> {
    if let Some(proj_dirs) = directories::ProjectDirs::from("com", "GitHub", "icy_view") {
        let dir = proj_dirs.config_dir().to_path_buf();
        if !dir.exists() {
            std::fs::create_dir_all(&dir).ok()?;
        }
        return Some(dir);
    }
    None
}

fn main() {
    let args = Args::parse_i18n_or_exit();

    if let Some(log_dir) = get_log_dir() {
        let _logger = Logger::try_with_env_or_str("info, iced=error, wgpu_hal=error, wgpu_core=error, i18n_embed=error")
            .unwrap()
            .log_to_file(FileSpec::default().directory(&log_dir).basename("icy_view").suffix("log").suppress_timestamp())
            .rotate(Criterion::Size(64 * 1024), Naming::Numbers, Cleanup::KeepLogFiles(3))
            .create_symlink(log_dir.join("icy_view.log"))
            .duplicate_to_stderr(flexi_logger::Duplicate::Warn)
            .start();
    } else {
        eprintln!("Failed to create log file");
    }

    log::info!("Starting iCY VIEW {}", *VERSION);

    iced::daemon(
        move || {
            if let Some(ref path) = args.path {
                window_manager::WindowManager::with_path(path.clone(), args.auto, args.bps)
            } else {
                window_manager::WindowManager::new(args.auto, args.bps)
            }
        },
        window_manager::WindowManager::update,
        window_manager::WindowManager::view,
    )
    .settings(Settings {
        vsync: true,
        antialiasing: true,
        ..Default::default()
    })
    .theme(window_manager::WindowManager::theme)
    .subscription(window_manager::WindowManager::subscription)
    .title(window_manager::WindowManager::title)
    .run()
    .expect("Failed to run application");

    log::info!("Shutting down.");

    // Cleanup temp files from this session
    Options::cleanup_session_temp();
}
