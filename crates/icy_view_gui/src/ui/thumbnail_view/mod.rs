//! Thumbnail view module
//!
//! This module contains all components for rendering file thumbnails in a grid layout:
//! - `thumbnail` - Data structures for thumbnails (RgbaData, Thumbnail, ThumbnailState)
//! - `thumbnail_loader` - Background loading of thumbnails with worker threads
//! - `tile_shader` - GPU shader-based tile rendering with wgpu
//! - `tile_grid_view` - Main tile grid view component
//! - `masonry_layout` - Masonry layout algorithm for the grid

mod masonry_layout;
mod thumbnail;
mod thumbnail_loader;
mod tile_grid_view;
pub mod tile_shader;

// Re-export public API
pub use thumbnail::{
    DIZ_NOT_FOUND_PLACEHOLDER, ERROR_PLACEHOLDER, FOLDER_PLACEHOLDER, LOADING_PLACEHOLDER, RgbaData, THUMBNAIL_MAX_HEIGHT, THUMBNAIL_RENDER_WIDTH,
    THUMBNAIL_SCALE, Thumbnail, ThumbnailCache, ThumbnailResult, ThumbnailState, get_width_multiplier,
};
pub use thumbnail_loader::{ThumbnailLoader, ThumbnailRequest, append_label_to_rgba, create_labeled_placeholder};
pub use tile_grid_view::{TileGridMessage, TileGridView};
pub use tile_shader::{
    TILE_BORDER_WIDTH, TILE_CORNER_RADIUS, TILE_IMAGE_WIDTH, TILE_INNER_PADDING, TILE_PADDING, TILE_SPACING, TILE_WIDTH, TileGridShader, TileShaderState,
    TileTexture, new_tile_id,
};
