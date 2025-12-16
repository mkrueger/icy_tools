//! Shared widgets for icy_draw
//!
//! Contains reusable widgets that are used across different editors:
//! - `tool_panel` - Tool selection panel
//! - `fkeys` - F-key sets management

mod fkeys;
pub mod plugins;
pub mod tool_panel;

pub use fkeys::*;
