//! Global dialogs for icy_draw
//!
//! Contains dialogs that are not editor-specific:
//! - `new_file` - New file dialog
//! - `settings` - Settings dialog
//! - `about` - About dialog
//! - `font_export` - Font export dialog
//! - `font_import` - Font import dialog
//! - `connect` - Connect to server dialog

pub mod about;
pub mod connect;
pub mod font_export;
pub mod font_import;
pub mod new_file;
pub mod settings;

pub use connect::{ConnectDialog, ConnectDialogMessage, ConnectDialogResult};
