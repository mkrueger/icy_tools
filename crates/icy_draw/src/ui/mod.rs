//! UI module for icy_draw
//!
//! Contains the window manager, main window implementation, and all editors.
//!
//! ## Module Structure
//!
//! - `main_window/` - Main window, menu, commands, session management
//! - `editor/` - All editor types (ansi, animation, bitfont, charfont, palette)
//! - `dialog/` - Global dialogs (new_file, settings, about, font_export/import)
//! - `widget/` - Shared widgets (tool_panel, fkeys)

pub mod dialog;
pub mod editor;
mod main_window;
pub mod widget;

pub use editor::ansi::*;
pub use main_window::*;
pub use widget::*;
