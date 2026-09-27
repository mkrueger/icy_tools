pub mod address_book;
pub mod drafts;
pub mod editor;
pub mod options;
pub mod perf;
pub mod qwk;
pub mod reader;
pub mod state;
pub mod taglines;
pub mod text;
#[path = "ui/threading.rs"]
pub mod threading;

#[derive(rust_embed::RustEmbed)]
#[folder = "i18n"]
pub struct Localizations;

/// Translations of the user interface (`i18n/<language>/icy_mail.ftl`, English is the fallback).
pub static LANGUAGE_LOADER: std::sync::LazyLock<i18n_embed::fluent::FluentLanguageLoader> = std::sync::LazyLock::new(|| {
    let loader = i18n_embed::fluent::fluent_language_loader!();
    #[cfg(not(test))]
    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();
    #[cfg(test)]
    let requested_languages = vec!["en".parse::<i18n_embed::unic_langid::LanguageIdentifier>().unwrap()];
    let _result = i18n_embed::select(&loader, &Localizations, &requested_languages);
    // The directional isolation marks around arguments are not needed for left-to-right text and
    // would show up in fixed-width labels.
    loader.set_use_isolating(false);
    loader
});

/// Switches the user interface to `languages` (for tests, which match English labels).
pub fn select_languages(languages: &[i18n_embed::unic_langid::LanguageIdentifier]) {
    let _result = i18n_embed::select(&*LANGUAGE_LOADER, &Localizations, languages);
    LANGUAGE_LOADER.set_use_isolating(false);
}

pub type Res<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
