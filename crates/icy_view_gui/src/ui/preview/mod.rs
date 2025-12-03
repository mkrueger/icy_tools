//! Preview module
//!
//! This module contains components for previewing files:
//! - `preview_view` - The main preview view component for terminal/image display
//! - `view_thread` - Background thread for streaming file parsing with baud emulation

mod preview_view;
mod view_thread;

pub use preview_view::{PreviewMessage, PreviewMode, PreviewView};
pub use view_thread::{
    ScrollMode,
    ViewCommand,
    ViewEvent,
    create_view_thread,
    prepare_parser_data,
};
