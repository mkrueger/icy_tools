pub mod brush;
pub mod charfont;
pub mod document;
pub mod files;
pub mod fill;
pub mod host;
pub mod mcp;
#[path = "ui/editor/ansi/tools/paint.rs"]
pub mod paint;
#[path = "util/plugins.rs"]
pub mod plugins;
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
    loader
});

#[macro_export]
macro_rules! fl {
    ($message_id:literal) => {{ i18n_embed_fl::fl!($crate::LANGUAGE_LOADER, $message_id) }};
    ($message_id:literal, $($args:expr),* $(,)?) => {{ i18n_embed_fl::fl!($crate::LANGUAGE_LOADER, $message_id, $($args),*) }};
}
