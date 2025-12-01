pub mod items;
pub mod ui;

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
}

use once_cell::sync::Lazy;

pub static LANGUAGE_LOADER: Lazy<FluentLanguageLoader> = Lazy::new(|| {
    let loader = fluent_language_loader!();
    let requested_languages = DesktopLanguageRequester::requested_languages();
    let _result = i18n_embed::select(&loader, &Localizations, &requested_languages);
    loader
});

pub type TerminalResult<T> = anyhow::Result<T>;
