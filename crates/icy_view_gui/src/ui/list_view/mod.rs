//! List view module
//!
//! This module contains the file list view and file browser components:
//! - `file_list_view` - Low-level list view widget for displaying files
//! - `file_list_shader` - GPU shader-based rendering for the file list
//! - `file_browser` - High-level file browser with navigation and filtering

mod file_browser;
mod file_list_shader;
mod file_list_view;

pub use file_browser::{FileBrowser, FileBrowserMessage};
pub use file_list_shader::{
    FileListShaderPrimitive, FileListShaderProgram, FileListShaderState, FileListThemeColors, ICON_PADDING, ICON_SIZE, ListItemRenderData, TEXT_START_X,
    invalidate_gpu_cache, render_icon_to_rgba, render_list_item, render_text_to_rgba,
};
pub use file_list_view::{FileListView, FileListViewMessage, ITEM_HEIGHT};
