pub mod ui;
use semver::Version;
pub use ui::*;
pub mod items;
pub use items::*;

use rust_embed::RustEmbed;
#[derive(RustEmbed)]
#[folder = "i18n"] // path to the compiled localization resources
struct Localizations;

use i18n_embed::{
    DesktopLanguageRequester,
    fluent::{FluentLanguageLoader, fluent_language_loader},
};

lazy_static::lazy_static! {
    static ref VERSION: Version = Version::parse( env!("CARGO_PKG_VERSION")).unwrap();
    static ref DEFAULT_TITLE: String = format!("iCY VIEW {}", *crate::VERSION);
}

use once_cell::sync::Lazy;
pub static LANGUAGE_LOADER: Lazy<FluentLanguageLoader> = Lazy::new(|| {
    let loader = fluent_language_loader!();
    let requested_languages = DesktopLanguageRequester::requested_languages();
    let _result = i18n_embed::select(&loader, &Localizations, &requested_languages);
    loader
});
pub type TerminalResult<T> = anyhow::Result<T>;
