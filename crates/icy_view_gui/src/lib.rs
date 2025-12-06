pub mod commands;
pub mod items;
pub mod ui;

pub use commands::*;
pub use items::*;
pub use ui::*;

use rust_embed::RustEmbed;
use semver::Version;

#[derive(RustEmbed)]
#[folder = "i18n"]
struct Localizations;

use i18n_embed::{
    DesktopLanguageRequester,
    fluent::{FluentLanguageLoader, fluent_language_loader},
};

lazy_static::lazy_static! {
    pub static ref VERSION: Version = Version::parse(env!("CARGO_PKG_VERSION")).unwrap();
    pub static ref DEFAULT_TITLE: String = format!("iCY VIEW {}", *VERSION);
    pub static ref CLIPBOARD_CONTEXT: clipboard_rs::ClipboardContext = clipboard_rs::ClipboardContext::new().unwrap();

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

use once_cell::sync::Lazy;

pub static LANGUAGE_LOADER: Lazy<FluentLanguageLoader> = Lazy::new(|| {
    let loader = fluent_language_loader!();
    let requested_languages = DesktopLanguageRequester::requested_languages();
    let _result = i18n_embed::select(&loader, &Localizations, &requested_languages);
    loader
});

pub type TerminalResult<T> = anyhow::Result<T>;
