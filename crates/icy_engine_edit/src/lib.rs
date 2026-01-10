mod editor;
pub use editor::*;
// FormatMode is re-exported from editor module

pub mod bitfont;
pub mod charset;

pub mod brushes;

mod layer_utils;
pub use layer_utils::{chars_from_area, stamp_char_grid, CharGrid, layer_from_area, stamp_layer};

pub mod tools;

pub mod collaboration;

use i18n_embed::{
    fluent::{fluent_language_loader, FluentLanguageLoader},
    DesktopLanguageRequester,
};
use rust_embed::RustEmbed;

// Re-export all necessary types from icy_engine
pub use icy_engine::{
    clipboard, load_with_parser, overlay_mask, parsers, AddType, AttributedChar, BitFont, BufferType, Caret, CompactGlyph, EditableScreen, EngineError,
    FontMode, GraphicsType, HyperLink, IceMode, Layer, LayerProperties, Line, MouseField, Palette, Position, Rectangle, RenderOptions, Result, Role,
    SaveOptions, SavedCaretState, Screen, Selection, SelectionMask, Shape, Sixel, Size, Tag, TerminalState, TextAttribute, TextBuffer, TextPane, TextScreen,
    DOS_DEFAULT_PALETTE,
};

// Re-export AnsiParser directly for convenient use
pub use icy_parser_core::AnsiParser;

// Re-export SAUCE metadata type
pub use icy_sauce::MetaData as SauceMetaData;

#[derive(RustEmbed)]
#[folder = "i18n"] // path to the compiled localization resources
struct Localizations;

use once_cell::sync::Lazy;
pub static LANGUAGE_LOADER: Lazy<FluentLanguageLoader> = Lazy::new(|| {
    let loader = fluent_language_loader!();
    let requested_languages = DesktopLanguageRequester::requested_languages();
    if i18n_embed::select(&loader, &Localizations, &requested_languages).is_err() {
        let fallback: Vec<i18n_embed::unic_langid::LanguageIdentifier> = vec!["en".parse().unwrap()];
        let _ = i18n_embed::select(&loader, &Localizations, &fallback);
    }
    loader
});

#[cfg(test)]
mod i18n_tests {
    use super::Localizations;

    #[test]
    fn en_has_undo_set_selection() {
        let loader = i18n_embed::fluent::fluent_language_loader!();
        let languages: Vec<i18n_embed::unic_langid::LanguageIdentifier> = vec!["en".parse().unwrap()];
        i18n_embed::select(&loader, &Localizations, &languages).unwrap();

        let translated = i18n_embed_fl::fl!(&loader, "undo-set_selection");
        assert_ne!(translated, "undo-set_selection");
    }
}
