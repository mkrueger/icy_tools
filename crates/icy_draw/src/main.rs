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
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use flexi_logger::{Cleanup, Criterion, FileSpec, Logger, Naming};
use lazy_static::lazy_static;
use semver::Version;

mod session;
mod ui;
mod util;
mod window_manager;

pub use ui::settings::*;
pub use window_manager::*;

pub use util::*;

lazy_static! {
    pub static ref VERSION: Version = Version::parse(env!("CARGO_PKG_VERSION")).unwrap();
}

lazy_static! {
    /// Global clipboard context for copy/paste operations
    pub static ref CLIPBOARD_CONTEXT: clipboard_rs::ClipboardContext = clipboard_rs::ClipboardContext::new().unwrap();
}

lazy_static! {
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

#[derive(rust_embed::RustEmbed)]
#[folder = "i18n"]
#[allow(dead_code)]
struct Localizations;

use once_cell::sync::Lazy;

#[allow(dead_code)]
static LANGUAGE_LOADER: Lazy<i18n_embed::fluent::FluentLanguageLoader> = Lazy::new(|| {
    let loader = i18n_embed::fluent::fluent_language_loader!();
    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();
    let _result = i18n_embed::select(&loader, &Localizations, &requested_languages);
    loader
});

#[macro_export]
macro_rules! fl {
    ($message_id:literal) => {{
        i18n_embed_fl::fl!($crate::LANGUAGE_LOADER, $message_id)
    }};
    ($message_id:literal, $($args:expr),* $(,)?) => {{
        i18n_embed_fl::fl!($crate::LANGUAGE_LOADER, $message_id, $($args),*)
    }};
}

#[derive(Parser, Debug)]
#[command(version, about = "A drawing program for ANSI & ASCII art", long_about = None)]
pub struct Args {
    /// Path to file to open
    #[arg(value_name = "PATH")]
    path: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Start a collaboration server
    Serve {
        /// Port to listen on (default: 6464)
        #[arg(short, long, default_value = "6464")]
        port: u16,

        /// Bind address (default: 0.0.0.0)
        #[arg(short, long, default_value = "0.0.0.0")]
        bind: String,

        /// Session password (optional)
        #[arg(long)]
        password: Option<String>,

        /// Maximum number of users (0 = unlimited)
        #[arg(long, default_value = "0")]
        max_users: usize,

        /// File to host (optional, creates empty 80x25 canvas if not specified)
        #[arg(value_name = "FILE")]
        file: Option<PathBuf>,
    },
}

fn get_log_dir() -> Option<PathBuf> {
    if let Some(dir) = Settings::config_dir() {
        if !dir.exists() {
            std::fs::create_dir_all(&dir).ok()?;
        }
        return Some(dir);
    }
    None
}

/// Run the collaboration server in headless mode.
fn run_server(bind: String, port: u16, password: Option<String>, max_users: usize, file: Option<PathBuf>) {
    use icy_engine_edit::collaboration::{ServerConfig, run_server as run_collab_server};

    // Determine document dimensions (default 80x25)
    // TODO: Load document from file to get actual dimensions and content
    let (columns, rows) = if let Some(ref path) = file {
        println!("Note: File loading not yet implemented, starting with 80x25 canvas");
        println!("File: {}", path.display());
        (80, 25)
    } else {
        (80, 25)
    };

    let bind_addr = format!("{}:{}", bind, port);
    let bind_addr: std::net::SocketAddr = match bind_addr.parse() {
        Ok(addr) => addr,
        Err(e) => {
            eprintln!("Error: Invalid bind address '{}': {}", bind_addr, e);
            std::process::exit(1);
        }
    };

    let config = ServerConfig {
        bind_addr,
        password: password.unwrap_or_default(),
        max_users,
        columns,
        rows,
        enable_extended_protocol: true,
        status_message: String::new(),
    };

    // Create tokio runtime and run the server
    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    rt.block_on(async {
        if let Err(e) = run_collab_server(config).await {
            eprintln!("Server error: {}", e);
            std::process::exit(1);
        }
    });
}

fn main() {
    let args = Args::parse();

    // Check if we're running the server subcommand
    if let Some(Command::Serve {
        port,
        bind,
        password,
        max_users,
        file,
    }) = args.command
    {
        run_server(bind, port, password, max_users, file);
        return;
    }

    if let Some(log_dir) = get_log_dir() {
        let _logger = Logger::try_with_env_or_str("info, iced=error, wgpu_hal=error, wgpu_core=error, i18n_embed=error")
            .unwrap()
            .log_to_file(FileSpec::default().directory(&log_dir).basename("icy_draw").suffix("log").suppress_timestamp())
            .rotate(Criterion::Size(64 * 1024), Naming::Numbers, Cleanup::KeepLogFiles(3))
            .create_symlink(log_dir.join("icy_draw.log"))
            .duplicate_to_stderr(flexi_logger::Duplicate::Warn)
            .start();
    } else {
        eprintln!("Failed to create log file");
    }

    log::info!("Starting iCY DRAW {}", *VERSION);

    iced::daemon(
        move || {
            let font_library = FontLibrary::create_shared();

            if let Some(ref path) = args.path {
                WindowManager::with_path(font_library, path.clone())
            } else {
                WindowManager::new(font_library)
            }
        },
        WindowManager::update,
        WindowManager::view,
    )
    .settings(iced::Settings {
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
}

fn load_window_icon(png_bytes: &[u8]) -> Result<iced::window::Icon, Box<dyn std::error::Error>> {
    let img = image::load_from_memory(png_bytes)?;
    let rgba = img.to_rgba8();
    let w = img.width();
    let h = img.height();
    Ok(iced::window::icon::from_rgba(rgba.into_raw(), w, h)?)
}
