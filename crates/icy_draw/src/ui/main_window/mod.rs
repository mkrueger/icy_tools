//! Main window module for icy_draw
//!
//! Contains:
//! - `main_window` - Main window implementation
//! - `window_manager` - Multi-window management
//! - `menu` - Menu bar
//! - `commands` - Keyboard shortcuts and commands
//! - `options` - Application options
//! - `session` - Session/hot-exit management
//! - `recent_files` - Recent files tracking

pub mod commands;
mod main_window;
mod main_window_file_handling;
pub mod menu;
mod options;
mod recent_files;
mod session;
mod window_manager;

pub use main_window::*;
pub use options::*;
pub use recent_files::*;
pub use window_manager::*;
