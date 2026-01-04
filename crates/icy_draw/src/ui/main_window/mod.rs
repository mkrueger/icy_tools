//! Main window module for icy_draw
//!
//! Contains:
//! - `main_window` - Main window implementation
//! - `window_manager` - Multi-window management
//! - `commands` - Keyboard shortcuts and commands
//! - `options` - Application options
//! - `session` - Session/hot-exit management
//! - `recent_files` - Recent files tracking

pub mod commands;
mod main_window;
mod main_window_file_handling;

pub use main_window::{EditMode, MainWindow, Message};
