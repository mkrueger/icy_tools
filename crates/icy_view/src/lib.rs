pub mod commands;
pub mod items;
pub mod options;
#[path = "ui/list_view/sauce_loader.rs"]
pub mod sauce_loader;
pub mod sort_order;
pub mod thumbnail;
#[path = "ui/thumbnail_view/backend.rs"]
pub mod thumbnail_backend;
#[path = "ui/preview/view_thread.rs"]
pub mod view_thread;

pub use options::*;

use i18n_embed::{
    fluent::{fluent_language_loader, FluentLanguageLoader},
    DesktopLanguageRequester,
};
use once_cell::sync::Lazy;
use rust_embed::RustEmbed;
use semver::Version;

#[derive(RustEmbed)]
#[folder = "i18n"]
struct Localizations;

pub static VERSION: Lazy<Version> = Lazy::new(|| Version::parse(env!("CARGO_PKG_VERSION")).unwrap());
pub static DEFAULT_TITLE: Lazy<String> = Lazy::new(|| format!("iCY VIEW {}", *VERSION));
pub static LANGUAGE_LOADER: Lazy<FluentLanguageLoader> = Lazy::new(|| {
    let loader = fluent_language_loader!();
    let _ = i18n_embed::select(&loader, &Localizations, &DesktopLanguageRequester::requested_languages());
    loader
});

pub type TerminalResult<T> = anyhow::Result<T>;
