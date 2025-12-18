//! UI module for icy_draw
//!
//! Contains the window manager, main window implementation, and all editors.
//!
//! ## Module Structure
//!
//! - `main_window/` - Main window, menu, commands, session management
//! - `editor/` - All editor types (ansi, animation, bitfont, charfont, palette)
//! - `dialog/` - Global dialogs (new_file, settings, about, font_export/import)
//! - `widget/` - Shared widgets (tool_panel)
//! - `settings/` - Application settings, F-keys, recent files

pub mod dialog;
pub mod editor;
pub mod main_window;
pub mod settings;
pub mod widget;

pub use editor::ansi::*;
pub use main_window::{EditMode, MainWindow, Message};
pub use settings::FKeySets;
pub use widget::*;
