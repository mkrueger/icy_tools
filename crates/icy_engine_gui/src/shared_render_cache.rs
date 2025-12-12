//! Shared render cache for Terminal and Minimap
//!
//! This module provides a shared cache for rendered tiles that both
//! the Terminal view and Minimap can access, avoiding duplicate rendering.

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

use crate::TextureSliceData;

/// Maximum height of a single tile in pixels
pub const TILE_HEIGHT: u32 = 8000;

/// Cache key for a rendered tile
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TileCacheKey {
    /// Tile index (0 = top tile, 1 = second tile, etc.)
    pub tile_index: i32,
    /// Blink state when rendered
    pub blink_on: bool,
}

impl TileCacheKey {
    pub fn new(tile_index: i32, blink_on: bool) -> Self {
        Self { tile_index, blink_on }
    }
}

/// A cached tile with its texture data (for shared render cache)
#[derive(Clone, Debug)]
pub struct SharedCachedTile {
    /// The texture slice data
    pub texture: TextureSliceData,
    /// Height of this tile in pixels
    pub height: u32,
    /// Y start position in document space
    pub start_y: f32,
}

/// Shared render cache that stores all rendered tiles
/// Can be accessed by both Terminal and Minimap
pub struct SharedRenderCache {
    /// Cached tiles keyed by (tile_index, blink_state)
    tiles: HashMap<TileCacheKey, SharedCachedTile>,
    /// Content version for cache invalidation
    content_version: u64,
    /// Total content height in pixels
    pub content_height: f32,
    /// Content width in pixels
    pub content_width: u32,
    /// Selection version (changes when selection changes)
    selection_version: u64,
    /// Last blink state used by Terminal (so Minimap can use the same)
    pub last_blink_state: bool,
}

impl SharedRenderCache {
    pub fn new() -> Self {
        Self {
            tiles: HashMap::new(),
            content_version: 0,
            content_height: 0.0,
            content_width: 0,
            selection_version: 0,
            last_blink_state: false,
        }
    }

    /// Clear all cached tiles
    pub fn clear(&mut self) {
        self.tiles.clear();
    }

    /// Invalidate cache due to content change
    pub fn invalidate(&mut self, new_version: u64) {
        if new_version != self.content_version {
            self.content_version = new_version;
            self.tiles.clear();
        }
    }

    /// Invalidate cache due to selection change
    pub fn invalidate_selection(&mut self, new_version: u64) {
        if new_version != self.selection_version {
            self.selection_version = new_version;
            self.tiles.clear();
        }
    }

    /// Get a cached tile if available
    pub fn get(&self, key: &TileCacheKey) -> Option<&SharedCachedTile> {
        self.tiles.get(key)
    }

    /// Insert a new tile into the cache
    pub fn insert(&mut self, key: TileCacheKey, tile: SharedCachedTile) {
        self.tiles.insert(key, tile);
    }

    /// Get current content version
    pub fn content_version(&self) -> u64 {
        self.content_version
    }

    /// Get number of cached tiles
    pub fn tile_count(&self) -> usize {
        self.tiles.len()
    }

    /// Get all tiles (for minimap to read all cached content)
    pub fn all_tiles(&self) -> impl Iterator<Item = (&TileCacheKey, &SharedCachedTile)> {
        self.tiles.iter()
    }

    /// Get tiles in order by index for a specific blink state
    pub fn tiles_in_order(&self, blink_on: bool) -> Vec<(&TileCacheKey, &SharedCachedTile)> {
        let mut tiles: Vec<_> = self.tiles.iter().filter(|(k, _)| k.blink_on == blink_on).collect();
        tiles.sort_by_key(|(k, _)| k.tile_index);
        tiles
    }

    /// Calculate which tiles would be needed to cover a Y range
    pub fn tiles_needed_for_range(&self, start_y: f32, end_y: f32) -> Vec<i32> {
        let tile_height = TILE_HEIGHT as f32;
        let first_tile = (start_y / tile_height).floor() as i32;
        let last_tile = (end_y / tile_height).ceil() as i32;
        (first_tile.max(0)..=last_tile).collect()
    }

    /// Get the maximum tile index based on content height
    pub fn max_tile_index(&self) -> i32 {
        ((self.content_height / TILE_HEIGHT as f32).ceil() as i32 - 1).max(0)
    }
}

impl Default for SharedRenderCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-safe wrapper for SharedRenderCache
pub type SharedRenderCacheHandle = Arc<RwLock<SharedRenderCache>>;

/// Create a new shared render cache handle
pub fn create_shared_render_cache() -> SharedRenderCacheHandle {
    Arc::new(RwLock::new(SharedRenderCache::new()))
}
