#![warn(clippy::all, clippy::pedantic)]
#![allow(
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    clippy::struct_excessive_bools,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_lossless,
    clippy::missing_panics_doc,
    clippy::needless_pass_by_value,
    clippy::unused_self,
    clippy::uninlined_format_args,
    clippy::vec_init_then_push,
    clippy::doc_markdown,
    clippy::wildcard_imports,
    clippy::default_trait_access,
    clippy::match_same_arms,
    clippy::useless_conversion,
    clippy::unnecessary_debug_formatting,
    clippy::redundant_closure_for_method_calls,
    clippy::items_after_test_module,
    clippy::must_use_candidate,
    clippy::return_self_not_must_use,
    clippy::trivially_copy_pass_by_ref,
    clippy::explicit_iter_loop,
    clippy::non_std_lazy_statics,
    clippy::manual_strip,
    clippy::unused_async,
    clippy::redundant_else,
    clippy::multiple_bound_locations,
    clippy::redundant_field_names,
    clippy::ptr_arg,
    clippy::field_reassign_with_default,
    clippy::needless_borrow,
    clippy::unnecessary_map_or,
    clippy::used_underscore_binding,
    clippy::if_not_else,
    clippy::ignored_unit_patterns,
    clippy::clone_on_copy,
    clippy::unnecessary_cast,
    clippy::map_unwrap_or,
    clippy::cloned_instead_of_copied,
    clippy::needless_range_loop,
    clippy::manual_clamp,
    clippy::items_after_statements,
    clippy::single_match_else,
    clippy::manual_let_else,
    clippy::float_cmp,
    clippy::needless_continue,
    clippy::map_clone,
    clippy::doc_lazy_continuation,
    clippy::map_entry,
    clippy::manual_div_ceil,
    clippy::elidable_lifetime_names,
    clippy::unnested_or_patterns,
    clippy::similar_names,
    clippy::empty_line_after_doc_comments,
    clippy::type_complexity,
    clippy::unnecessary_unwrap,
    clippy::collapsible_else_if,
    clippy::bind_instead_of_map,
    clippy::option_option,
    clippy::unwrap_or_default,
    clippy::unnecessary_sort_by,
    clippy::unnecessary_wraps,
    clippy::assigning_clones,
    clippy::redundant_locals,
    clippy::borrowed_box,
    clippy::explicit_counter_loop,
    clippy::missing_fields_in_debug,
    clippy::single_match,
    clippy::needless_return,
    clippy::too_many_arguments,
    clippy::double_ended_iterator_last,
    clippy::to_string_in_format_args,
    clippy::redundant_closure,
    clippy::implicit_clone,
    clippy::if_same_then_else,
    clippy::manual_range_contains,
    clippy::semicolon_if_nothing_returned,
    dead_code,
    unused_variables,
    unused_mut
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
use icy_ui::Settings;
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

    /// Run in portable mode (config saved next to executable)
    #[clap(long, default_value_t = false, help = i18n_embed_fl::fl!(LANGUAGE_LOADER, "arg-portable-help"))]
    portable: bool,

    /// Custom configuration directory path
    #[clap(long, value_name = "DIR", help = i18n_embed_fl::fl!(LANGUAGE_LOADER, "arg-config-dir-help"))]
    config_dir: Option<PathBuf>,
}

fn main() {
    let args = Args::parse_i18n_or_exit();

    // Initialize the global config directory based on CLI args and auto-detection.
    // Must happen before logger init and options loading.
    options::init_config_dir(args.portable, args.config_dir.clone());

    let log_dir = options::get_config_dir();
    if !log_dir.exists() {
        let _ = std::fs::create_dir_all(log_dir);
    }
    let _logger = Logger::try_with_env_or_str("info, iced=error, wgpu_hal=error, wgpu_core=error, i18n_embed=error")
        .unwrap()
        .log_to_file(FileSpec::default().directory(log_dir).basename("icy_view").suffix("log").suppress_timestamp())
        .rotate(Criterion::Size(64 * 1024), Naming::Numbers, Cleanup::KeepLogFiles(3))
        .create_symlink(log_dir.join("icy_view.log"))
        .duplicate_to_stderr(flexi_logger::Duplicate::Warn)
        .start();

    log::info!("Starting iCY VIEW {}", *VERSION);

    icy_ui::daemon(
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
