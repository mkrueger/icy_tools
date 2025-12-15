//! List view module
//!
//! This module contains the file list view and file browser components:
//! - `file_list_view` - Low-level list view widget for displaying files
//! - `file_list_shader` - GPU shader-based rendering for the file list
//! - `file_browser` - High-level file browser with navigation and filtering
//! - `sauce_loader` - Async SAUCE information loader

mod file_browser;
mod file_list_shader;
mod file_list_view;
mod sauce_loader;

pub use file_browser::{FileBrowser, FileBrowserMessage};
pub use file_list_shader::{
    FileListShaderPrimitive, FileListShaderProgram, FileListShaderState, FileListThemeColors, ICON_PADDING, ICON_SIZE, ListItemRenderData, SAUCE_AUTHOR_WIDTH,
    SAUCE_GROUP_WIDTH, SAUCE_NAME_WIDTH, SAUCE_TITLE_WIDTH, TEXT_START_X, invalidate_gpu_cache, render_icon_to_rgba, render_list_item,
    render_list_item_with_sauce, render_text_to_rgba,
};
pub use file_list_view::{FileListView, FileListViewMessage, ITEM_HEIGHT};
pub use sauce_loader::{SauceCache, SauceInfo, SauceLoader, SauceRequest, SauceResult, SharedSauceCache};
