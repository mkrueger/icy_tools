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
pub use tile_grid_view::{TileGridMessage, TileGridView};
