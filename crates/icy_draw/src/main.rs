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
use clap_i18n_richformatter::clap_i18n;
use flexi_logger::{Cleanup, Criterion, FileSpec, Logger, Naming};
use lazy_static::lazy_static;
use semver::Version;

mod mcp;
mod session;
mod ui;
mod util;
mod window_manager;

pub use mcp::McpCommand;
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

/// Default port for the icy_draw collaboration server (kept in sync with Moebius).
pub const DEFAULT_COLLAB_PORT: u16 = 8000;
pub const DEFAULT_COLLAB_PORT_STR: &str = "8000";

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
#[command(version, about = i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "app-about"), long_about = None)]
#[clap_i18n]
pub struct Args {
    /// File to open on startup
    #[arg(value_name = "PATH", help = i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "arg-path-help"))]
    path: Option<PathBuf>,

    /// Start an MCP server on the given port (e.g. --mcp-port 8080)
    #[arg(long, value_name = "PORT", help = i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "arg-mcp-port-help"))]
    mcp_port: Option<u16>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Host a real-time collaboration session (Moebius-compatible)
    #[command(version, about = i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "cmd-host-about"))]
    Host {
        /// Port to listen on (default: 8000)
        #[arg(short, long, default_value_t = DEFAULT_COLLAB_PORT, help = i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "arg-host-port-help"))]
        port: u16,

        /// Bind address (default: 0.0.0.0)
        #[arg(short, long, default_value = "0.0.0.0", help = i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "arg-host-bind-help"))]
        bind: String,

        /// Session password (optional)
        #[arg(long, help = i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "arg-host-password-help"))]
        password: Option<String>,

        /// Maximum number of users (0 = unlimited)
        #[arg(long, default_value = "0", help = i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "arg-host-max-users-help"))]
        max_users: usize,

        /// File to host (optional; starts with an empty 80x25 canvas if omitted)
        #[arg(value_name = "FILE", help = i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "arg-host-file-help"))]
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

/// Returns the default EGA 16-color palette (8-bit RGB values).
fn default_ega_palette() -> [[u8; 3]; 16] {
    [
        [0x00, 0x00, 0x00], // 0: Black
        [0x00, 0x00, 0xAA], // 1: Blue
        [0x00, 0xAA, 0x00], // 2: Green
        [0x00, 0xAA, 0xAA], // 3: Cyan
        [0xAA, 0x00, 0x00], // 4: Red
        [0xAA, 0x00, 0xAA], // 5: Magenta
        [0xAA, 0x55, 0x00], // 6: Brown/Yellow
        [0xAA, 0xAA, 0xAA], // 7: Light Gray
        [0x55, 0x55, 0x55], // 8: Dark Gray
        [0x55, 0x55, 0xFF], // 9: Light Blue
        [0x55, 0xFF, 0x55], // 10: Light Green
        [0x55, 0xFF, 0xFF], // 11: Light Cyan
        [0xFF, 0x55, 0x55], // 12: Light Red
        [0xFF, 0x55, 0xFF], // 13: Light Magenta
        [0xFF, 0xFF, 0x55], // 14: Yellow
        [0xFF, 0xFF, 0xFF], // 15: White
    ]
}

/// Run the collaboration server in headless mode.
fn run_server(bind: String, port: u16, password: Option<String>, max_users: usize, file: Option<PathBuf>) {
    use icy_engine::{FileFormat, IceMode, TextPane};
    use icy_engine_edit::SauceMetaData;
    use icy_engine_edit::collaboration::{Block, ServerConfig, run_server as run_collab_server};

    // Load document from file or create empty 80x25 canvas
    let (columns, rows, initial_document, ice_colors, use_9px_font, font_name, sauce, palette) = if let Some(ref path) = file {
        // Detect format and load file
        let format = match FileFormat::from_path(path) {
            Some(f) => f,
            None => {
                eprintln!(
                    "{}",
                    i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "server-error-unknown-format", path = path.display().to_string())
                );
                eprintln!("{}", i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "server-starting-empty-canvas"));
                FileFormat::Ansi // Fallback, will likely fail to load
            }
        };
        match format.load(path, None) {
            Ok(loaded_doc) => {
                let buffer = &loaded_doc.screen.buffer;
                let cols = buffer.width() as u32;
                let rws: u32 = buffer.height() as u32;

                // Extract character data (column-major for server)
                let mut doc = Vec::with_capacity(cols as usize);
                for col in 0..cols as i32 {
                    let mut column = Vec::with_capacity(rws as usize);
                    for row in 0..rws as i32 {
                        let ch = buffer.char_at((col, row).into());
                        // Extract palette index from AttributeColor
                        let fg = match ch.attribute.foreground_color() {
                            icy_engine::AttributeColor::Palette(n) | icy_engine::AttributeColor::ExtendedPalette(n) => n,
                            icy_engine::AttributeColor::Rgb(_, _, _) => 7, // Default to light gray
                            icy_engine::AttributeColor::Transparent => 0,
                        };
                        let bg = match ch.attribute.background_color() {
                            icy_engine::AttributeColor::Palette(n) | icy_engine::AttributeColor::ExtendedPalette(n) => n,
                            icy_engine::AttributeColor::Rgb(_, _, _) => 0, // Default to black
                            icy_engine::AttributeColor::Transparent => 0,
                        };
                        column.push(Block { code: ch.ch as u32, fg, bg });
                    }
                    doc.push(column);
                }

                // Extract metadata
                let ice = matches!(buffer.ice_mode, IceMode::Ice);
                let font = buffer.font(0).map(|f| f.name().to_string()).unwrap_or_else(|| "IBM VGA".to_string());

                // Get SAUCE data from the LoadedDocument
                let sauce: SauceMetaData = if let Some(ref sauce_record) = loaded_doc.sauce_opt {
                    sauce_record.metadata()
                } else {
                    SauceMetaData::default()
                };

                // Extract 16-color palette from buffer
                let mut palette = [[0u8; 3]; 16];
                for i in 0..16 {
                    let (r, g, b) = buffer.palette.rgb(i as u32);
                    palette[i] = [r, g, b];
                }

                println!(
                    "{}",
                    i18n_embed_fl::fl!(
                        crate::LANGUAGE_LOADER,
                        "server-loaded",
                        path = path.display().to_string(),
                        cols = cols.to_string(),
                        rows = rws.to_string()
                    )
                );
                (cols, rws, Some(doc), ice, false, font, sauce, palette)
            }
            Err(e) => {
                eprintln!(
                    "{}",
                    i18n_embed_fl::fl!(
                        crate::LANGUAGE_LOADER,
                        "server-error-loading-file",
                        path = path.display().to_string(),
                        error = e.to_string()
                    )
                );
                eprintln!("{}", i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "server-starting-empty-canvas"));
                (
                    80,
                    25,
                    None,
                    false,
                    false,
                    "IBM VGA".to_string(),
                    SauceMetaData::default(),
                    default_ega_palette(),
                )
            }
        }
    } else {
        (
            80,
            25,
            None,
            false,
            false,
            "IBM VGA".to_string(),
            SauceMetaData::default(),
            default_ega_palette(),
        )
    };

    let bind_addr = format!("{}:{}", bind, port);
    let bind_addr: std::net::SocketAddr = match bind_addr.parse() {
        Ok(addr) => addr,
        Err(e) => {
            eprintln!(
                "{}",
                i18n_embed_fl::fl!(
                    crate::LANGUAGE_LOADER,
                    "server-error-invalid-bind-address",
                    addr = format!("{}:{}", bind, port),
                    error = e.to_string()
                )
            );
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
        initial_document,
        ice_colors,
        use_9px_font,
        font_name,
        palette,
        sauce,
        // Localized UI strings for server banner
        ui_title: i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "server-title"),
        ui_bind_address: i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "server-bind-address"),
        ui_password: i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "server-password"),
        ui_document: i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "server-document"),
        ui_max_users: i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "server-max-users"),
        ui_connect_with: i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "server-connect-with"),
        ui_stop_hint: i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "server-stop-hint"),
        ui_none: i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "server-none"),
        ui_unlimited: i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "server-unlimited"),
    };

    // Create tokio runtime and run the server
    let rt = tokio::runtime::Runtime::new().expect(&i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "server-error-runtime"));
    rt.block_on(async {
        if let Err(e) = run_collab_server(config).await {
            eprintln!("{}", i18n_embed_fl::fl!(crate::LANGUAGE_LOADER, "server-error", error = e.to_string()));
            std::process::exit(1);
        }
    });
}

fn main() {
    let args = Args::parse_i18n_or_exit();

    // Check if we're running the server subcommand
    if let Some(Command::Host {
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

    // Start MCP server if requested
    let mcp_rx = if let Some(port) = args.mcp_port {
        let (mcp_server, command_rx) = mcp::McpServer::new();
        let mcp_server = std::sync::Arc::new(mcp_server);
        let mcp_clone = mcp_server.clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                if let Err(e) = mcp_clone.start(port).await {
                    log::error!("MCP server error: {}", e);
                }
            });
        });

        log::info!("MCP server started on port {}", port);
        Some(parking_lot::Mutex::new(Some(command_rx)))
    } else {
        None
    };

    iced::daemon(
        move || {
            let font_library = TextArtFontLibrary::create_shared();

            // Take the MCP receiver if available
            let mcp_receiver = mcp_rx.as_ref().and_then(|mutex| mutex.lock().take());

            if let Some(ref path) = args.path {
                WindowManager::with_path(font_library, path.clone(), mcp_receiver)
            } else {
                WindowManager::new(font_library, mcp_receiver)
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
