//! UI module for icy_draw
//!
//! Contains the window manager and main window implementation.

mod ansi_editor;
mod commands;
mod main_window;
mod menu;
mod window_manager;

pub use ansi_editor::*;
pub use main_window::*;
pub use window_manager::*;
