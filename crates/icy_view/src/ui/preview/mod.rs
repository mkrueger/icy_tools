//! Preview module
//!
//! This module contains components for previewing files:
//! - `preview_view` - The main preview view component for terminal/image display
//! - `view_thread` - Background thread for streaming file parsing with baud emulation
//! - `image_viewer` - Image viewer widget with zoom and scroll support
//! - `content_view` - Common interface for scrollable content views
//! - `drag_scroll` - Shared drag-to-scroll with inertia support

mod content_view;
mod drag_scroll;
mod image_content_view;
mod image_viewer;
mod preview_view;
mod terminal_content_view;
mod view_thread;

pub use preview_view::{PreviewMessage, PreviewView, is_image_file, is_sixel_file};
pub use view_thread::prepare_parser_data;
