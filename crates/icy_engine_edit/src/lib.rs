mod editor;
pub use editor::*;
// FormatMode is re-exported from editor module

pub mod bitfont;
pub mod charset;

pub mod brushes;

mod layer_utils;
pub use layer_utils::{layer_from_area, stamp_layer};

pub mod tools;

#[cfg(feature = "collaboration")]
pub mod collaboration;

use i18n_embed::{
    DesktopLanguageRequester,
    fluent::{FluentLanguageLoader, fluent_language_loader},
};
use rust_embed::RustEmbed;

// Re-export all necessary types from icy_engine
pub use icy_engine::{
    AddType, AnsiSaveOptionsV2, AttributedChar, BitFont, BufferType, Caret, CompactGlyph, DOS_DEFAULT_PALETTE, EditableScreen, EngineError, FontMode,
    GraphicsType, HyperLink, IceMode, Layer, Line, MouseField, Palette, Position, Properties, Rectangle, RenderOptions, Result, Role, SavedCaretState, Screen,
    Selection, SelectionMask, Shape, Sixel, Size, Tag, TerminalState, TextAttribute, TextBuffer, TextPane, TextScreen, clipboard, load_with_parser,
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
