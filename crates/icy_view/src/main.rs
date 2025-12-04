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

use std::path::PathBuf;

use clap::Parser;
use clap_i18n_richformatter::clap_i18n;
use flexi_logger::{Cleanup, Criterion, FileSpec, Logger, Naming};
use iced::Settings;
use lazy_static::lazy_static;
use semver::Version;

mod ui;
use ui::WindowManager;

lazy_static! {
    static ref VERSION: Version = Version::parse(env!("CARGO_PKG_VERSION")).unwrap();
}

lazy_static! {
    static ref LATEST_VERSION: Version = {
        let github = github_release_check::GitHub::new().unwrap();
        if let Ok(ver) = github.get_all_versions("mkrueger/icy_tools") {
            for v in ver {
                if v.starts_with("IcyView") {
                    return Version::parse(&v[7..]).unwrap();
                }
            }
        }
        VERSION.clone()
    };
}

#[derive(rust_embed::RustEmbed)]
#[folder = "i18n"]
struct Localizations;

use once_cell::sync::Lazy;

static LANGUAGE_LOADER: Lazy<i18n_embed::fluent::FluentLanguageLoader> = Lazy::new(|| {
    let loader = i18n_embed::fluent::fluent_language_loader!();
    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();
    let _result = i18n_embed::select(&loader, &Localizations, &requested_languages);
    loader
});

#[derive(Parser, Debug)]
#[command(version, about = i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "app-about"), long_about = None)]
#[clap_i18n]
pub struct Args {
    /// Path to file or directory to open
    #[arg(value_name = "PATH", help = i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "arg-path-help"))]
    path: Option<PathBuf>,

    /// Enable auto-scrolling
    #[clap(long, default_value_t = false, help = i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "arg-auto-help"))]
    auto: bool,

    /// Baud rate emulation (e.g., 9600, 19200, 38400)
    #[clap(long, value_name = "RATE", help = i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "arg-bps-help"))]
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

    icy_engine_gui::set_default_auto_scaling_xy(false);

    iced::daemon(
        move || {
            if let Some(ref path) = args.path {
                WindowManager::with_path(path.clone(), args.auto, args.bps)
            } else {
                WindowManager::new(args.auto, args.bps)
            }
        },
        WindowManager::update,
        WindowManager::view,
    )
    .settings(Settings {
        vsync: true,
        antialiasing: true,
        ..Default::default()
    })
    .theme(WindowManager::theme)
    .subscription(WindowManager::subscription)
    .title(WindowManager::title)
    .run()
    .expect("Failed to run application");

    log::info!("Shutting down.");

    // Cleanup temp files from this session
    icy_view_gui::ui::Options::cleanup_session_temp();
}

fn load_window_icon(png_bytes: &[u8]) -> Result<iced::window::Icon, Box<dyn std::error::Error>> {
    let img = image::load_from_memory(png_bytes)?;
    let rgba = img.to_rgba8();
    let w = img.width();
    let h = img.height();
    Ok(iced::window::icon::from_rgba(rgba.into_raw(), w, h)?)
}
