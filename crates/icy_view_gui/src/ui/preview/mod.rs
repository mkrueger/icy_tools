//! Preview module
//!
//! This module contains components for previewing files:
//! - `preview_view` - The main preview view component for terminal/image display
//! - `view_thread` - Background thread for streaming file parsing with baud emulation
//! - `image_viewer` - Image viewer widget with zoom and scroll support
//! - `content_view` - Common interface for scrollable content views

mod content_view;
mod image_content_view;
mod image_viewer;
mod preview_view;
mod terminal_content_view;
mod view_thread;

pub use content_view::ContentView;
pub use image_content_view::ImageContentView;
pub use image_viewer::{ImageViewer, ImageViewerMessage};
pub use preview_view::{PreviewMessage, PreviewMode, PreviewView, is_image_file, is_sixel_file};
pub use terminal_content_view::TerminalContentView;
pub use view_thread::{ScrollMode, ViewCommand, ViewEvent, create_view_thread, prepare_parser_data};
