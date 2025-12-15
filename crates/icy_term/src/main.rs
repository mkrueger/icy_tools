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

//mod ui;
use std::{
    path::PathBuf,
    sync::{Arc, atomic::AtomicU16},
    time::Instant,
};

use clap_i18n_richformatter::clap_i18n;

use flexi_logger::{Cleanup, Criterion, FileSpec, Logger, Naming};
use lazy_static::lazy_static;
use semver::Version;

//use ui::MainWindow;
pub type TerminalResult<T> = Res<T>;

pub mod data;
pub use data::*;
pub mod protocol;
pub mod terminal;
pub use terminal::*;

pub mod auto_login;
pub mod commands;
pub mod features;
pub mod mcp;
pub mod scripting;
pub mod ui;
mod util;
pub type Res<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

use clap::Parser;

lazy_static! {
    static ref VERSION: Version = Version::parse(env!("CARGO_PKG_VERSION")).unwrap();
    static ref START_TIME: Instant = Instant::now();
    static ref CLIPBOARD_CONTEXT: clipboard_rs::ClipboardContext = clipboard_rs::ClipboardContext::new().unwrap();
}

lazy_static::lazy_static! {
    static ref LATEST_VERSION: Version = {
        let github = github_release_check::GitHub::new().unwrap();
        if let Ok(ver) = github.get_all_versions("mkrueger/icy_tools") {
            for v in ver {
                if v.starts_with("IcyTerm") {
                    return Version::parse(&v[7..]).unwrap();
                }
            }
        }
        VERSION.clone()
    };
}

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
});

#[derive(Parser, Debug)]
#[command(author, version, about = i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "app-about"), long_about = None)]
#[clap_i18n]
struct Args {
    #[arg(value_name = "URL", help = i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "arg-url-help"), long_help = i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "arg-url-format"))]
    url: Option<String>,

    #[arg(short, long, value_name = "SCRIPT", help = i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "arg-run-help"))]
    run: Option<PathBuf>,

    #[arg(long, value_name = "PORT", help = i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "arg-mcp-port-help"))]
    mcp_port: Option<u16>,
}

pub type McpHandler = Option<tokio::sync::mpsc::UnboundedReceiver<mcp::McpCommand>>;

pub static MCP_PORT: AtomicU16 = AtomicU16::new(0);

fn main() {
    let args = Args::parse_i18n_or_exit();

    if let Some(log_dir) = Options::get_log_dir() {
        let _logger = Logger::try_with_str("info, iced=error, wgpu_hal=error, wgpu_core=error, i18n_embed=error, zbus=error, zbus::connection=error")
            .unwrap()
            .log_to_file(FileSpec::default().directory(&log_dir).basename("icy_term").suffix("log").suppress_timestamp())
            .rotate(
                Criterion::Size(64 * 1024), // 64 KB should be enough for everyone
                Naming::Numbers,
                Cleanup::KeepLogFiles(3),
            )
            .create_symlink(log_dir.join("icy_term.log"))
            .duplicate_to_stderr(flexi_logger::Duplicate::Warn)
            .start();
    } else {
        eprintln!("Failed to create log file");
    }

    if let Some(url) = &args.url {
        if let Err(e) = crate::ConnectionInformation::parse(url) {
            eprintln!(
                "{}",
                i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "cli-error-url-parse", url = url.as_str(), error = e.to_string())
            );
            std::process::exit(1);
        }
    }

    // Validate script file if provided
    if let Some(script_path) = &args.run {
        if !script_path.exists() {
            eprintln!(
                "{}",
                i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "cli-error-script-not-found", file = script_path.display().to_string())
            );
            std::process::exit(1);
        }
        if !script_path.is_file() {
            eprintln!(
                "{}",
                i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "cli-error-script-not-file", file = script_path.display().to_string())
            );
            std::process::exit(1);
        }
    }

    // Start MCP server if requested
    let mcp_rx = if let Some(port) = args.mcp_port {
        let (mcp_server, command_rx) = mcp::McpServer::new();
        let mcp_server = Arc::new(mcp_server);
        let mcp_clone = mcp_server.clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                if let Err(e) = mcp_clone.start(port).await {
                    log::error!("MCP server error: {}", e);
                }
            });
        });

        MCP_PORT.store(port, std::sync::atomic::Ordering::Relaxed);
        log::info!("MCP server started on port {}", port);
        Some(parking_lot::Mutex::new(Some(command_rx)))
    } else {
        None
    };

    log::info!("Starting iCY TERM {}", *VERSION);
    icy_net::websocket::init_websocket_providers();

    let url_for_closure = args.url;
    let script_for_closure = args.run;
    let mcp_rx = Arc::new(mcp_rx);

    iced::daemon(
        move || {
            let mcp_receiver = if let Some(mutex) = mcp_rx.as_ref() { mutex.lock().take() } else { None };
            if let Some(ref url) = url_for_closure {
                let mut manager: (WindowManager, iced::Task<ui::WindowManagerMessage>) = WindowManager::with_url(mcp_receiver, url.clone());
                if let Some(ref script) = script_for_closure {
                    manager.0.script_to_run = Some(script.clone());
                }
                manager
            } else if let Some(ref script) = script_for_closure {
                WindowManager::with_script(mcp_receiver, script.clone())
            } else {
                WindowManager::new(mcp_receiver)
            }
        },
        WindowManager::update,
        WindowManager::view,
    )
    .antialiasing(true)
    .theme(WindowManager::theme)
    .subscription(WindowManager::subscription)
    .title(WindowManager::title)
    .run()
    .expect("Failed to run application");
    log::info!("shutting down.");
}

fn load_window_icon(png_bytes: &[u8]) -> Result<iced::window::Icon, Box<dyn std::error::Error>> {
    // Add `image = "0.24"` (or latest) to Cargo.toml if not present.
    let img = image::load_from_memory(png_bytes)?;
    let rgba = img.to_rgba8();
    let w = img.width();
    let h = img.height();
    Ok(iced::window::icon::from_rgba(rgba.into_raw(), w, h)?)
}
