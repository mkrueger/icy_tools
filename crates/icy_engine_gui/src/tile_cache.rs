//! Tile-based caching for efficient terminal and minimap rendering
//!
//! This module provides a shared tile cache that both Terminal and Minimap views can use.
//! Content is split into tiles (configurable height), rendered once, and cached.
//! Scrolling only requires loading new tiles, not re-rendering the entire viewport.
//!
//! Key features:
//! - Configurable tile height via `TILE_HEIGHT_LINES` constant
//! - LRU eviction for memory management
//! - Shared cache between Terminal and Minimap (GPU handles scaling)
//! - Global invalidation via content_version
//! - Prefetching for smooth scrolling

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

// ============================================================================
// Configuration Constants
// ============================================================================

/// Height of each tile in lines (rows of text)
/// This is the primary tuning parameter for cache granularity vs overhead.
/// - Smaller values: Finer granularity, more tiles, higher cache overhead
/// - Larger values: Coarser granularity, fewer tiles, lower cache overhead
///
/// Recommended range: 50-200 lines
/// Default: 100 lines (~1600px at 16px font height)
pub const TILE_HEIGHT_LINES: u32 = 100;

/// Maximum height of a single texture slice in pixels
/// This must stay under GPU texture limits (typically 8192 or 16384)
pub const MAX_TILE_HEIGHT_PIXELS: u32 = 8000;

/// Maximum number of texture slices supported by the shaders.
///
/// Note: The actual number of slices used is dynamic and depends on the
/// current viewport height (plus padding slices).
pub const MAX_TEXTURE_SLICES: usize = 10;

/// Maximum total content height in pixels (MAX_TEXTURE_SLICES * MAX_TILE_HEIGHT_PIXELS)
pub const MAX_CONTENT_HEIGHT: u32 = (MAX_TEXTURE_SLICES as u32) * MAX_TILE_HEIGHT_PIXELS;

/// Default maximum number of cached tiles (for LRU eviction)
/// At 100 lines per tile with 800px width and 16px font:
/// ~1600px height * 800px width * 4 bytes = ~5MB per tile
/// 50 tiles = ~250MB max cache (worst case)
pub const DEFAULT_MAX_CACHED_TILES: usize = 50;

/// Default prefetch distance in tiles (how many tiles to preload beyond viewport)
pub const DEFAULT_PREFETCH_DISTANCE: u32 = 2;

// ============================================================================
// Data Structures
// ============================================================================

/// Configuration for the tile cache
#[derive(Clone, Debug)]
pub struct TileCacheConfig {
    /// Height of each tile in lines
    pub tile_height_lines: u32,
    /// Maximum number of cached tiles (LRU eviction when exceeded)
    pub max_cached_tiles: usize,
    /// Whether to cache both blink variants separately
    pub cache_blink_variants: bool,
    /// How many tiles to prefetch beyond the visible viewport
    pub prefetch_distance: u32,
}

impl Default for TileCacheConfig {
    fn default() -> Self {
        Self {
            tile_height_lines: TILE_HEIGHT_LINES,
            max_cached_tiles: DEFAULT_MAX_CACHED_TILES,
            cache_blink_variants: true,
            prefetch_distance: DEFAULT_PREFETCH_DISTANCE,
        }
    }
}

/// Unique identifier for a cached tile
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TileId {
    /// Tile index (vertical position, 0 = first tile_height_lines rows)
    pub index: u32,
    /// Blink state when rendered (true = blinking chars visible)
    pub blink_on: bool,
}

impl TileId {
    pub fn new(index: u32, blink_on: bool) -> Self {
        Self { index, blink_on }
    }
}

/// A single cached tile containing rendered RGBA data
#[derive(Clone, Debug)]
pub struct CachedTile {
    /// Tile identifier
    pub id: TileId,
    /// RGBA pixel data (shared via Arc for efficient cloning)
    pub rgba_data: Arc<Vec<u8>>,
    /// Size in pixels (width, height)
    pub size: (u32, u32),
    /// Timestamp of last access (for LRU eviction)
    pub last_access: Instant,
    /// Content version when this tile was rendered
    pub content_version: u64,
    /// Start line of this tile in the document (inclusive)
    pub start_line: u32,
    /// End line of this tile in the document (exclusive)
    pub end_line: u32,
}

impl CachedTile {
    /// Returns the height of this tile in lines
    pub fn line_count(&self) -> u32 {
        self.end_line - self.start_line
    }
}

/// Texture slice data for GPU upload (compatible with shader)
#[derive(Clone, Debug)]
pub struct TextureSliceData {
    /// RGBA pixel data
    pub rgba_data: Arc<Vec<u8>>,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
}

/// The main tile cache
pub struct TileCache {
    /// Configuration
    pub config: TileCacheConfig,
    /// Cached tiles keyed by TileId
    tiles: HashMap<TileId, CachedTile>,
    /// Current content version (incremented on any content change)
    content_version: u64,
    /// Font dimensions in pixels (width, height)
    font_size: (u32, u32),
    /// Whether scanlines are enabled (doubles vertical resolution)
    scan_lines: bool,
    /// Total number of lines in the document
    total_lines: u32,
    /// Document width in pixels
    pub content_width: u32,
}

impl TileCache {
    /// Create a new tile cache with default configuration
    pub fn new() -> Self {
        Self::with_config(TileCacheConfig::default())
    }

    /// Create a new tile cache with custom configuration
    pub fn with_config(config: TileCacheConfig) -> Self {
        Self {
            config,
            tiles: HashMap::new(),
            content_version: 0,
            font_size: (8, 16),
            scan_lines: false,
            total_lines: 0,
            content_width: 640,
        }
    }

    /// Update cache parameters from screen state
    /// Should be called when font, scanlines, or document size changes
    pub fn update_from_screen(&mut self, font_size: (u32, u32), scan_lines: bool, total_lines: u32, content_width: u32) {
        let font_changed = self.font_size != font_size;
        let scanlines_changed = self.scan_lines != scan_lines;

        self.font_size = font_size;
        self.scan_lines = scan_lines;
        self.total_lines = total_lines;
        self.content_width = content_width;

        // Invalidate all tiles if font or scanlines changed
        if font_changed || scanlines_changed {
            self.invalidate_all();
        }
    }

    /// Get current content version
    pub fn content_version(&self) -> u64 {
        self.content_version
    }

    /// Increment content version (call when content changes)
    pub fn bump_version(&mut self) {
        self.content_version += 1;
    }

    /// Set content version to a specific value (for syncing with buffer version)
    pub fn set_version(&mut self, version: u64) {
        if version != self.content_version {
            self.content_version = version;
        }
    }

    /// Calculate tile height in pixels (accounting for scanlines)
    pub fn tile_height_pixels(&self) -> u32 {
        let base_height = self.config.tile_height_lines * self.font_size.1;
        if self.scan_lines { base_height * 2 } else { base_height }
    }

    /// Calculate the tile index containing a given line
    pub fn tile_index_for_line(&self, line: u32) -> u32 {
        line / self.config.tile_height_lines
    }

    /// Calculate total number of tiles needed for the document
    pub fn total_tile_count(&self) -> u32 {
        if self.total_lines == 0 {
            return 1;
        }
        (self.total_lines + self.config.tile_height_lines - 1) / self.config.tile_height_lines
    }

    /// Get the line range for a tile (start inclusive, end exclusive)
    pub fn tile_line_range(&self, tile_index: u32) -> (u32, u32) {
        let start = tile_index * self.config.tile_height_lines;
        let end = (start + self.config.tile_height_lines).min(self.total_lines.max(1));
        (start, end)
    }

    /// Get total content height in pixels
    pub fn total_content_height(&self) -> u32 {
        let base_height = self.total_lines * self.font_size.1;
        if self.scan_lines { base_height * 2 } else { base_height }
    }

    /// Check if a cached tile is still valid
    pub fn is_tile_valid(&self, tile: &CachedTile) -> bool {
        tile.content_version == self.content_version
    }

    /// Get a cached tile if it exists and is valid
    pub fn get_tile(&mut self, id: TileId) -> Option<&CachedTile> {
        // First check if tile exists and is valid
        let is_valid = if let Some(tile) = self.tiles.get(&id) {
            tile.content_version == self.content_version
        } else {
            false
        };

        if is_valid {
            // Now we can safely get mutable access and update
            if let Some(tile) = self.tiles.get_mut(&id) {
                tile.last_access = Instant::now();
            }
            // Return immutable reference
            self.tiles.get(&id)
        } else {
            None
        }
    }

    /// Insert a tile into the cache, performing LRU eviction if needed
    pub fn insert_tile(&mut self, tile: CachedTile) {
        // LRU eviction if cache is full
        while self.tiles.len() >= self.config.max_cached_tiles {
            self.evict_oldest_tile();
        }

        self.tiles.insert(tile.id, tile);
    }

    /// Remove the least recently used tile
    fn evict_oldest_tile(&mut self) {
        if let Some(oldest_id) = self.tiles.iter().min_by_key(|(_, tile)| tile.last_access).map(|(id, _)| *id) {
            self.tiles.remove(&oldest_id);
        }
    }

    /// Invalidate all cached tiles
    pub fn invalidate_all(&mut self) {
        self.tiles.clear();
    }

    /// Get cache statistics for debugging
    pub fn stats(&self) -> TileCacheStats {
        TileCacheStats {
            cached_tile_count: self.tiles.len(),
            total_tile_count: self.total_tile_count() as usize,
            content_version: self.content_version,
            total_cached_bytes: self.tiles.values().map(|t| t.rgba_data.len()).sum(),
        }
    }

    /// Calculate which tile indices are needed for a viewport
    /// Returns (start_tile, end_tile) inclusive, plus prefetch tiles
    pub fn tiles_for_viewport(&self, scroll_y: f32, visible_height: f32) -> (u32, u32) {
        let font_h = self.font_size.1 as f32;
        let scanline_mult = if self.scan_lines { 2.0 } else { 1.0 };

        // Convert pixel scroll to line number
        let start_line = (scroll_y / (font_h * scanline_mult)) as u32;
        let visible_lines = (visible_height / (font_h * scanline_mult)).ceil() as u32;
        let end_line = start_line + visible_lines;

        let start_tile = self.tile_index_for_line(start_line);
        let end_tile = self.tile_index_for_line(end_line);

        // Add prefetch
        let prefetch_start = start_tile.saturating_sub(self.config.prefetch_distance);
        let prefetch_end = (end_tile + self.config.prefetch_distance).min(self.total_tile_count().saturating_sub(1));

        (prefetch_start, prefetch_end)
    }

    /// Collect valid cached tiles for a range, returning which indices need rendering
    /// Returns: (cached_tiles, missing_indices)
    pub fn collect_tiles_for_range(&mut self, start_tile: u32, end_tile: u32, blink_on: bool) -> (Vec<CachedTile>, Vec<u32>) {
        let mut cached = Vec::new();
        let mut missing = Vec::new();

        for idx in start_tile..=end_tile {
            let id = TileId::new(idx, blink_on);
            if let Some(tile) = self.get_tile(id) {
                cached.push(tile.clone());
            } else {
                missing.push(idx);
            }
        }

        (cached, missing)
    }

    /// Build texture slices from cached tiles for GPU upload
    /// Combines tiles into slices respecting MAX_TILE_HEIGHT_PIXELS
    /// Returns slices ready for the shader
    pub fn build_texture_slices(&mut self, start_tile: u32, end_tile: u32, blink_on: bool) -> Vec<TextureSliceData> {
        let mut slices = Vec::new();
        let mut current_slice_data: Vec<u8> = Vec::new();
        let mut current_slice_height: u32 = 0;
        let width = self.content_width;

        for idx in start_tile..=end_tile {
            let id = TileId::new(idx, blink_on);
            if let Some(tile) = self.tiles.get(&id) {
                if !self.is_tile_valid(tile) {
                    continue;
                }

                let tile_height = tile.size.1;

                // Check if adding this tile would exceed slice limit
                if current_slice_height + tile_height > MAX_TILE_HEIGHT_PIXELS && !current_slice_data.is_empty() {
                    // Finish current slice
                    slices.push(TextureSliceData {
                        rgba_data: Arc::new(current_slice_data),
                        width,
                        height: current_slice_height,
                    });
                    current_slice_data = Vec::new();
                    current_slice_height = 0;

                    // Check slice limit
                    if slices.len() >= MAX_TEXTURE_SLICES {
                        break;
                    }
                }

                // Add tile data to current slice
                current_slice_data.extend_from_slice(&tile.rgba_data);
                current_slice_height += tile_height;
            }
        }

        // Don't forget the last slice
        if !current_slice_data.is_empty() && slices.len() < MAX_TEXTURE_SLICES {
            slices.push(TextureSliceData {
                rgba_data: Arc::new(current_slice_data),
                width,
                height: current_slice_height,
            });
        }

        slices
    }
}

impl Default for TileCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about the tile cache
#[derive(Clone, Debug)]
pub struct TileCacheStats {
    pub cached_tile_count: usize,
    pub total_tile_count: usize,
    pub content_version: u64,
    pub total_cached_bytes: usize,
}

/// Thread-safe shared tile cache
pub type SharedTileCache = Arc<RwLock<TileCache>>;

/// Create a new shared tile cache
pub fn create_shared_tile_cache() -> SharedTileCache {
    Arc::new(RwLock::new(TileCache::new()))
}

/// Create a new shared tile cache with custom configuration
pub fn create_shared_tile_cache_with_config(config: TileCacheConfig) -> SharedTileCache {
    Arc::new(RwLock::new(TileCache::with_config(config)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tile_index_calculation() {
        let cache = TileCache::new();
        assert_eq!(cache.tile_index_for_line(0), 0);
        assert_eq!(cache.tile_index_for_line(99), 0);
        assert_eq!(cache.tile_index_for_line(100), 1);
        assert_eq!(cache.tile_index_for_line(199), 1);
        assert_eq!(cache.tile_index_for_line(200), 2);
    }

    #[test]
    fn test_tile_line_range() {
        let mut cache = TileCache::new();
        cache.total_lines = 250;

        assert_eq!(cache.tile_line_range(0), (0, 100));
        assert_eq!(cache.tile_line_range(1), (100, 200));
        assert_eq!(cache.tile_line_range(2), (200, 250)); // Last tile is partial
    }

    #[test]
    fn test_total_tile_count() {
        let mut cache = TileCache::new();

        cache.total_lines = 100;
        assert_eq!(cache.total_tile_count(), 1);

        cache.total_lines = 101;
        assert_eq!(cache.total_tile_count(), 2);

        cache.total_lines = 250;
        assert_eq!(cache.total_tile_count(), 3);
    }

    #[test]
    fn test_lru_eviction() {
        let config = TileCacheConfig {
            max_cached_tiles: 3,
            ..Default::default()
        };
        let mut cache = TileCache::with_config(config);
        cache.total_lines = 500;

        // Insert 3 tiles
        for i in 0..3 {
            cache.insert_tile(CachedTile {
                id: TileId::new(i, true),
                rgba_data: Arc::new(vec![0u8; 100]),
                size: (10, 10),
                last_access: Instant::now(),
                content_version: 0,
                start_line: i * 100,
                end_line: (i + 1) * 100,
            });
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        assert_eq!(cache.tiles.len(), 3);

        // Insert 4th tile - should evict oldest
        cache.insert_tile(CachedTile {
            id: TileId::new(3, true),
            rgba_data: Arc::new(vec![0u8; 100]),
            size: (10, 10),
            last_access: Instant::now(),
            content_version: 0,
            start_line: 300,
            end_line: 400,
        });

        assert_eq!(cache.tiles.len(), 3);
        // Tile 0 should have been evicted (oldest)
        assert!(!cache.tiles.contains_key(&TileId::new(0, true)));
        assert!(cache.tiles.contains_key(&TileId::new(3, true)));
    }
}
