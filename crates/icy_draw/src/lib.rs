pub mod brush;
pub mod charfont;
pub mod document;
pub mod files;
pub mod fill;
pub mod host;
pub mod mcp;
pub mod palette_files;
pub mod recovery;
pub mod rip_document;
#[path = "ui/editor/ansi/tools/paint.rs"]
pub mod paint;
#[path = "util/plugins.rs"]
pub mod plugins;
#[path = "ui/editor/ansi/selection_drag.rs"]
pub mod selection_drag;
#[path = "ui/settings/mod.rs"]
pub mod settings;
#[path = "ui/editor/ansi/shape_points.rs"]
pub mod shape_points;
#[path = "util/tdf_font_library.rs"]
pub mod text_art_fonts;
pub use settings::*;

pub static VERSION: std::sync::LazyLock<semver::Version> = std::sync::LazyLock::new(|| semver::Version::parse(env!("CARGO_PKG_VERSION")).unwrap());

#[derive(rust_embed::RustEmbed)]
#[folder = "i18n"]
struct Localizations;

pub static LANGUAGE_LOADER: std::sync::LazyLock<i18n_embed::fluent::FluentLanguageLoader> = std::sync::LazyLock::new(|| {
    let loader = i18n_embed::fluent::fluent_language_loader!();
    let languages = i18n_embed::DesktopLanguageRequester::requested_languages();
    let _ = i18n_embed::select(&loader, &Localizations, &languages);
    // Directional isolation marks around arguments are not needed for left-to-right text and
    // would show up in fixed-width labels.
    loader.set_use_isolating(false);
    loader
});

/// Switches the user interface to `languages` (used by tests, which match English labels).
pub fn select_languages(languages: &[i18n_embed::unic_langid::LanguageIdentifier]) {
    let _ = i18n_embed::select(&*LANGUAGE_LOADER, &Localizations, languages);
    LANGUAGE_LOADER.set_use_isolating(false);
}

#[macro_export]
macro_rules! fl {
    ($message_id:literal) => {{ i18n_embed_fl::fl!($crate::LANGUAGE_LOADER, $message_id) }};
    ($message_id:literal, $($args:expr),* $(,)?) => {{ i18n_embed_fl::fl!($crate::LANGUAGE_LOADER, $message_id, $($args),*) }};
}
