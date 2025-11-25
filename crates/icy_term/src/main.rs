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

use directories::ProjectDirs;
use lazy_static::lazy_static;
//use ui::MainWindow;
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

pub mod data;
pub use data::*;
pub mod terminal;
pub use terminal::*;

pub mod auto_login;
pub mod features;
mod icons;
pub mod mcp;
pub mod scripting;
pub mod ui;
mod util;
pub type Res<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

use clap::{CommandFactory, Parser};

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

fn get_log_file() -> anyhow::Result<PathBuf> {
    if let Some(proj_dirs) = ProjectDirs::from("com", "GitHub", "icy_term") {
        let dir = proj_dirs.config_dir().join("icy_term.log");
        return Ok(dir);
    }
    Err(anyhow::anyhow!("Error getting log directory"))
}

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
    clap_i18n_richformatter::init_clap_rich_formatter_localizer();
    use std::fs;

    let args = Args::parse_i18n();

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
                let mut manager = WindowManager::with_url(mcp_receiver, url.clone());
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
