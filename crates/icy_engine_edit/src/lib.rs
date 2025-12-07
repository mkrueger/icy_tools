mod editor;
pub use editor::*;

mod layer_utils;
pub use layer_utils::{layer_from_area, stamp_layer};

#[cfg(feature = "collaboration")]
pub mod collaboration;

use i18n_embed::{
    DesktopLanguageRequester,
    fluent::{FluentLanguageLoader, fluent_language_loader},
};
use rust_embed::RustEmbed;

// Re-export all necessary types from icy_engine
pub use icy_engine::{
    AddType, AttributedChar, BitFont, Caret, DOS_DEFAULT_PALETTE, EditableScreen, EngineError, Result, FontMode, IceMode, Layer, Line, Palette, PaletteMode, Position,
    Properties, Rectangle, Role, Selection, SelectionMask, Sixel, Size, Tag, TextAttribute, TextBuffer, TextPane, TextScreen, clipboard, load_with_parser,
    overlay_mask, parsers,
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
    let _result = i18n_embed::select(&loader, &Localizations, &requested_languages);
    loader
});
