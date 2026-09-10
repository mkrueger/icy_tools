pub mod auto_login;
pub mod commands;
pub mod data;
pub mod features;
pub mod mcp;
pub mod protocol;
pub mod scripting;
pub mod terminal;
pub mod util;

pub use data::*;
pub use terminal::*;

pub type Res<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
pub type TerminalResult<T> = Res<T>;

#[derive(rust_embed::RustEmbed)]
#[folder = "i18n"]
struct Localizations;

pub static LANGUAGE_LOADER: std::sync::LazyLock<i18n_embed::fluent::FluentLanguageLoader> = std::sync::LazyLock::new(|| {
    let loader = i18n_embed::fluent::fluent_language_loader!();
    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();
    let _result = i18n_embed::select(&loader, &Localizations, &requested_languages);
    loader
});
