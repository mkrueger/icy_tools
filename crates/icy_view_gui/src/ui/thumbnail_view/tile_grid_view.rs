use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use parking_lot::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;

use iced::mouse;
use iced::widget::canvas::Cache;
use iced::widget::{container, shader, stack, text};
use iced::{Color, Element, Length, Point, Rectangle, Shadow};
use tokio::sync::mpsc;

use icy_engine_gui::{ScrollbarOverlay, ScrollbarState, Viewport};

use super::masonry_layout::{self, ItemSize, MasonryConfig};
use super::thumbnail::{ERROR_PLACEHOLDER, LOADING_PLACEHOLDER, Thumbnail, ThumbnailResult, ThumbnailState};
use super::thumbnail_loader::{ThumbnailLoader, ThumbnailRequest, create_labeled_placeholder, render_label_tag};
use super::tile_shader::{TILE_PADDING, TILE_SPACING, TILE_WIDTH, TileGridShader, TileShaderState, TileTexture, new_tile_id};
use crate::Item;
use crate::items::{ItemFile, ItemFolder};
use crate::ui::options::ScrollSpeed;
use crate::ui::theme;

/// Base tile width for layout calculations (full tile including borders)
const TILE_BASE_WIDTH: f32 = TILE_WIDTH;

/// Time window for double-click detection (in milliseconds)
const DOUBLE_CLICK_MS: u128 = 400;

/// Maximum number of thumbnails to keep loaded in memory
/// This prevents memory issues with very large directories (100k+ files)
const MAX_LOADED_THUMBNAILS: usize = 500;

/// Buffer zone around viewport for preloading (in pixels)
const PRELOAD_BUFFER_PX: f32 = 500.0;

/// Wrapper for the shader program that implements shader::Program
struct TileShaderProgramWrapper {
    tiles: Vec<TileTexture>,
    scroll_y: f32,
    content_height: f32,
    _viewport_height: f32,
    _selected_tile_id: Option<u64>,
    /// Shared hover state - updated by shader, read by click handler
    shared_hovered_tile: Arc<Mutex<Option<u64>>>,
    /// Background color from theme (shared)
    background_color: Arc<RwLock<[f32; 4]>>,
}

impl<Message> shader::Program<Message> for TileShaderProgramWrapper
where
    Message: Clone + 'static,
{
    type State = ();
    type Primitive = TileGridShader;

    fn draw(&self, _state: &Self::State, _cursor: mouse::Cursor, bounds: Rectangle) -> Self::Primitive {
        // Apply hover state from shared state
        let hovered = *self.shared_hovered_tile.lock();
        let tiles: Vec<TileTexture> = self
            .tiles
            .iter()
            .map(|t| {
                let mut tile = t.clone();
                tile.is_hovered = hovered == Some(t.id);
                tile
            })
            .collect();

        TileGridShader {
            tiles,
            scroll_y: self.scroll_y,
            viewport_height: bounds.height,
            content_height: self.content_height,
            background_color: *self.background_color.read(),
            selection_color: [0.3, 0.5, 0.8, 0.5],
            hover_color: [0.5, 0.5, 0.5, 0.3],
        }
    }

    fn update(&self, _state: &mut Self::State, _event: &iced::Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<iced::widget::Action<Message>> {
        // Handle hover detection using correct relative coordinates
        let found_tile = if let Some(cursor_pos) = cursor.position_in(bounds) {
            let mut result = None;
            for tile in &self.tiles {
                let tile_top = tile.position.1 - self.scroll_y;
                let tile_bottom = tile_top + tile.display_size.1;
                let tile_left = tile.position.0;
                let tile_right = tile_left + tile.display_size.0;

                if cursor_pos.x >= tile_left && cursor_pos.x <= tile_right && cursor_pos.y >= tile_top && cursor_pos.y <= tile_bottom {
                    result = Some(tile.id);
                    break;
                }
            }
            result
        } else {
            None
        };

        // Update shared state
        *self.shared_hovered_tile.lock() = found_tile;

        None
    }

    fn mouse_interaction(&self, _state: &Self::State, _bounds: Rectangle, _cursor: mouse::Cursor) -> mouse::Interaction {
        if self.shared_hovered_tile.lock().is_some() {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }
}

/// A positioned tile in the layout
#[derive(Debug, Clone)]
pub struct LayoutTile {
    /// Index in the thumbnails vector
    pub index: usize,
    /// X position in pixels
    pub x: f32,
    /// Y position in pixels
    pub y: f32,
    /// Width in pixels
    pub width: f32,
    /// Height in pixels (including label)
    pub height: f32,
}

/// Messages for the tile grid view
#[derive(Debug, Clone)]
pub enum TileGridMessage {
    /// A tile was clicked
    TileClicked(usize),
    /// A tile was double-clicked
    TileDoubleClicked(usize),
    /// Scroll position changed
    Scrolled(f32),
    /// Animation tick (for blinking)
    AnimationTick,
    /// Thumbnail loading completed
    ThumbnailReady(String, ThumbnailState),
    /// Width changed (from responsive container)
    WidthChanged(f32),
    /// Scrollbar scroll event (from scrollbar overlay)
    ScrollbarScroll(f32, f32),
    /// Scrollbar hover state changed
    ScrollbarHover(bool),
    /// Keyboard: select previous item (up arrow)
    SelectPrevious,
    /// Keyboard: select next item (down arrow)
    SelectNext,
    /// Keyboard: select item to the left
    SelectLeft,
    /// Keyboard: select item to the right
    SelectRight,
    /// Keyboard: page up
    PageUp,
    /// Keyboard: page down
    PageDown,
    /// Keyboard: home
    Home,
    /// Keyboard: end
    End,
    /// Open selected item (Enter/double-click)
    OpenSelected,
}

/// Tile grid view for displaying thumbnails
pub struct TileGridView {
    /// All thumbnails
    thumbnails: Vec<Thumbnail>,
    /// Tile IDs for shader (one per thumbnail)
    pub tile_ids: Vec<u64>,
    /// Items for each thumbnail (for loading data from virtual files)
    items: Vec<Arc<dyn Item>>,
    /// Whether each thumbnail is a container (folder, zip, etc.)
    is_container: Vec<bool>,
    /// Computed layout (RefCell for interior mutability in view())
    pub layout: RefCell<Vec<LayoutTile>>,
    /// Total content height
    content_height: RefCell<f32>,
    /// Available width (for layout calculation)
    available_width: RefCell<f32>,
    /// Viewport for smooth scrolling
    viewport: Viewport,
    /// Currently selected tile index
    pub selected_index: Option<usize>,
    /// Thumbnail loader
    loader: ThumbnailLoader,
    /// Receiver for completed thumbnails
    result_rx: mpsc::UnboundedReceiver<ThumbnailResult>,
    /// Cache for canvas rendering
    cache: Cache,
    /// Blink state for animated thumbnails
    blink_on: bool,
    /// Time since last blink toggle
    blink_timer: f32,
    /// Shader state (for hover detection)
    pub shader_state: TileShaderState,
    /// Shared hover state - updated by shader, read by click handler
    pub shared_hovered_tile: Arc<Mutex<Option<u64>>>,
    /// Scrollbar state (animations, hover, drag)
    scrollbar: ScrollbarState,
    /// Shared hover state for scrollbar overlay
    scrollbar_hover_state: Arc<AtomicBool>,
    /// Last known cursor position (for click handling)
    pub last_cursor_position: Option<Point>,
    /// Last known widget bounds (for event handling, RefCell for interior mutability)
    last_bounds: RefCell<Rectangle>,
    /// Whether auto-scroll is currently active
    auto_scroll_active: bool,
    /// Scroll speed for auto-scroll
    scroll_speed: ScrollSpeed,
    /// Last click time and tile index for double-click detection
    last_click: Option<(Instant, usize)>,
    /// Flag indicating a double-click was detected in last mouse event
    pending_double_click: bool,
    /// Current filter string
    filter: String,
    /// Indices of visible items after filtering (maps visible index to thumbnail index)
    visible_indices: Vec<usize>,
    /// Cancellation token for async subitem loading
    subitems_cancel_token: CancellationToken,
    /// Receiver for async subitem loading results
    subitems_rx: Option<tokio::sync::oneshot::Receiver<Vec<Box<dyn Item>>>>,
    /// Background color for shader (shared, can be updated from theme)
    background_color: Arc<RwLock<[f32; 4]>>,
    /// LRU order for loaded thumbnails (front = oldest, back = newest)
    /// Contains indices of thumbnails that are currently loaded (Ready state)
    loaded_lru: Vec<usize>,
    /// Last viewport position for detecting scroll changes
    last_viewport_top: f32,
}

impl TileGridView {
    /// Create a new tile grid view
    pub fn new() -> Self {
        let (loader, result_rx) = ThumbnailLoader::spawn();

        let mut viewport = Viewport::default();
        // Smooth scroll animation speed (higher = faster animation)
        viewport.scroll_animation_speed = 15.0;

        Self {
            thumbnails: Vec::new(),
            tile_ids: Vec::new(),
            items: Vec::new(),
            is_container: Vec::new(),
            layout: RefCell::new(Vec::new()),
            content_height: RefCell::new(0.0),
            available_width: RefCell::new(0.0),
            viewport,
            selected_index: None,
            loader,
            result_rx,
            cache: Cache::new(),
            blink_on: true,
            blink_timer: 0.0,
            shader_state: TileShaderState::default(),
            shared_hovered_tile: Arc::new(Mutex::new(None)),
            scrollbar: ScrollbarState::new(),
            scrollbar_hover_state: Arc::new(AtomicBool::new(false)),
            last_cursor_position: None,
            last_bounds: RefCell::new(Rectangle::new(Point::ORIGIN, iced::Size::new(800.0, 600.0))),
            auto_scroll_active: false,
            scroll_speed: ScrollSpeed::Medium,
            last_click: None,
            pending_double_click: false,
            filter: String::new(),
            visible_indices: Vec::new(),
            subitems_cancel_token: CancellationToken::new(),
            subitems_rx: None,
            background_color: Arc::new(RwLock::new([0.1, 0.1, 0.12, 1.0])), // Default, will be set by set_background_color
            loaded_lru: Vec::new(),
            last_viewport_top: 0.0,
        }
    }

    /// Set background color from theme
    pub fn set_background_color(&self, color: iced::Color) {
        *self.background_color.write() = [color.r, color.g, color.b, color.a];
    }

    /// Get max scroll offset
    fn max_scroll(&self) -> f32 {
        self.viewport.max_scroll_y()
    }

    /// Scroll by a delta amount (with smooth animation)
    pub fn scroll_by(&mut self, delta: f32) {
        // User is scrolling manually, stop auto-scroll
        self.auto_scroll_active = false;
        self.viewport.scroll_y_by(-delta);
    }

    /// Scroll by a delta amount immediately (no animation)
    pub fn scroll_by_immediate(&mut self, delta: f32) {
        // User is scrolling manually, stop auto-scroll
        self.auto_scroll_active = false;
        let new_y = self.viewport.scroll_y - delta;
        self.viewport.scroll_y_to_immediate(new_y);
    }

    /// Scroll to position with smooth animation
    pub fn scroll_to(&mut self, y: f32) {
        // User is scrolling manually, stop auto-scroll
        self.auto_scroll_active = false;
        self.viewport.scroll_y_to(y);
    }

    /// Scroll to position immediately
    pub fn scroll_to_immediate(&mut self, y: f32) {
        // User is scrolling manually, stop auto-scroll
        self.auto_scroll_active = false;
        self.viewport.scroll_y_to_immediate(y);
    }

    /// Get current scroll position
    pub fn scroll_y(&self) -> f32 {
        self.viewport.scroll_y
    }

    /// Get the maximum scroll position
    pub fn max_scroll_y(&self) -> f32 {
        self.viewport.max_scroll_y()
    }

    /// Check if content is scrollable (has more content than visible area)
    pub fn is_scrollable(&self) -> bool {
        self.viewport.max_scroll_y() > 0.0
    }

    /// Start auto-scroll mode
    pub fn start_auto_scroll(&mut self) {
        // Always enable auto-scroll - it will naturally start scrolling
        // once content becomes scrollable (after thumbnails load)
        self.auto_scroll_active = true;
    }

    /// Stop auto-scroll mode
    pub fn stop_auto_scroll(&mut self) {
        self.auto_scroll_active = false;
    }

    /// Check if auto-scroll is currently active
    pub fn is_auto_scroll_active(&self) -> bool {
        self.auto_scroll_active
    }

    /// Set scroll speed for auto-scroll
    pub fn set_scroll_speed(&mut self, speed: ScrollSpeed) {
        self.scroll_speed = speed;
    }

    /// Apply a filter to the items - only matching items will be shown
    pub fn apply_filter(&mut self, filter: &str) {
        let old_filter = std::mem::replace(&mut self.filter, filter.to_string());

        // Only update if filter actually changed
        if old_filter == self.filter {
            return;
        }

        self.update_visible_indices();

        // Reset selection when filter changes
        self.selected_index = if self.visible_indices.is_empty() { None } else { Some(0) };

        // Force layout recalculation
        *self.available_width.borrow_mut() = 0.0;
        self.viewport.scroll_x_to_immediate(0.0);
        self.viewport.scroll_y_to_immediate(0.0);
        self.cache.clear();
    }

    /// Clear the filter
    pub fn clear_filter(&mut self) {
        self.apply_filter("");
    }

    /// Get the current filter
    pub fn get_filter(&self) -> &str {
        &self.filter
    }

    /// Update the visible indices based on current filter
    fn update_visible_indices(&mut self) {
        if self.filter.is_empty() {
            // No filter - all items visible
            self.visible_indices = (0..self.thumbnails.len()).collect();
        } else {
            let filter_lower = self.filter.to_lowercase();
            self.visible_indices = self
                .thumbnails
                .iter()
                .enumerate()
                .filter(|(_, thumb)| thumb.label.to_lowercase().contains(&filter_lower))
                .map(|(i, _)| i)
                .collect();
        }
    }

    /// Get the number of visible (filtered) items
    pub fn visible_count(&self) -> usize {
        self.visible_indices.len()
    }

    /// Set the items to display using item info tuples (path, label, is_container)
    pub fn set_items(&mut self, item_infos: Vec<(String, String, bool)>) {
        // Notify loader to cancel old tasks
        self.loader.cancel_loading();

        // Clear existing
        self.thumbnails.clear();
        self.tile_ids.clear();
        self.items.clear();
        self.is_container.clear();
        self.layout.borrow_mut().clear();
        self.selected_index = None;
        self.loader.clear_pending();
        self.filter.clear();
        self.loaded_lru.clear();
        self.last_viewport_top = 0.0;
        // Reset scroll position to prevent invalid viewport coordinates
        self.viewport.scroll_x_to_immediate(0.0);
        self.viewport.scroll_y_to_immediate(0.0);
        // Reset available_width to force recalculation on first view()
        *self.available_width.borrow_mut() = 0.0;

        // Create thumbnails for each item - also create Item objects for unified handling
        for (path, label, container) in item_infos {
            // Create appropriate Item type for unified get_sync_thumbnail() handling
            let item: Arc<dyn Item> = if container {
                Arc::new(ItemFolder::new(path.clone()))
            } else {
                Arc::new(ItemFile::new(path.clone()))
            };

            // For items with sync thumbnails (folders), load immediately to ensure correct layout
            let mut thumb = Thumbnail::new(path.clone(), label.clone());
            if let Some(rgba) = item.get_sync_thumbnail() {
                // Render label separately for GPU (don't embed in image)
                thumb.label_rgba = render_label_tag(&label, 1);
                thumb.state = ThumbnailState::Ready { rgba };
            }

            self.thumbnails.push(thumb);
            self.tile_ids.push(new_tile_id());
            self.items.push(item);
            self.is_container.push(container);
        }

        // Initialize visible indices (all visible by default)
        self.update_visible_indices();

        // Don't compute initial layout here - let view() do it with the correct width
        // The layout will be calculated when view_with_width() or view() is called

        // DON'T queue all loads - lazy loading will handle it when view() is called
        // This prevents memory issues with large directories

        // Invalidate cache
        self.cache.clear();
    }

    /// Set items to display using Box<dyn Item> (supports virtual files)
    pub fn set_items_from_items(&mut self, items_list: Vec<Box<dyn Item>>) {
        // Notify loader to cancel old tasks
        self.loader.cancel_loading();

        // Clear existing
        self.thumbnails.clear();
        self.tile_ids.clear();
        self.items.clear();
        self.is_container.clear();
        self.layout.borrow_mut().clear();
        self.selected_index = None;
        self.loader.clear_pending();
        self.filter.clear();
        self.loaded_lru.clear();
        self.last_viewport_top = 0.0;
        // Reset scroll position to prevent invalid viewport coordinates
        self.viewport.scroll_x_to_immediate(0.0);
        self.viewport.scroll_y_to_immediate(0.0);
        // Reset available_width to force recalculation on first view()
        *self.available_width.borrow_mut() = 0.0;

        // Create thumbnails for each item, skipping parent directory
        for item in items_list {
            // Skip parent directory - doesn't belong in thumbnail view
            if item.is_parent() {
                continue;
            }
            let path = item.get_file_path();
            let label = item.get_label();
            let container = item.is_container();

            // For items with sync thumbnails (folders), load immediately to ensure correct layout
            let item_arc: Arc<dyn Item> = Arc::from(item);
            let mut thumb = Thumbnail::new(path, label.clone());
            if let Some(rgba) = item_arc.get_sync_thumbnail() {
                // Render label separately for GPU (don't embed in image)
                thumb.label_rgba = render_label_tag(&label, 1);
                thumb.state = ThumbnailState::Ready { rgba };
            }

            self.thumbnails.push(thumb);
            self.tile_ids.push(new_tile_id());
            self.items.push(item_arc);
            self.is_container.push(container);
        }

        // Initialize visible indices (all visible by default)
        self.update_visible_indices();

        // Don't compute initial layout here - let view() do it with the correct width
        // The layout will be calculated when view_with_width() or view() is called

        // DON'T queue all loads - lazy loading will handle it when view() is called
        // This prevents memory issues with large directories

        // Invalidate cache
        self.cache.clear();
    }

    /// Load subitems from an item asynchronously in the background
    /// This is used for loading folder contents without blocking the UI
    pub fn load_subitems_async(&mut self, item: Box<dyn Item>) {
        log::info!("[TileGridView] load_subitems_async called for: {:?}", item.get_file_path());

        // Cancel any previous loading operation
        self.subitems_cancel_token.cancel();
        self.subitems_cancel_token = CancellationToken::new();

        // Clear current items while loading
        self.loader.cancel_loading();
        self.thumbnails.clear();
        self.tile_ids.clear();
        self.items.clear();
        self.is_container.clear();
        self.layout.borrow_mut().clear();
        self.selected_index = None;
        self.loader.clear_pending();
        self.filter.clear();
        self.loaded_lru.clear();
        self.last_viewport_top = 0.0;
        self.viewport.scroll_x_to_immediate(0.0);
        self.viewport.scroll_y_to_immediate(0.0);
        *self.available_width.borrow_mut() = 0.0;
        self.cache.clear();

        // Create oneshot channel for result
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.subitems_rx = Some(rx);

        // Clone what we need for the async task
        let cancel_token = self.subitems_cancel_token.clone();

        // Use the loader's runtime to spawn the async task
        // This ensures we're running in a proper Tokio context
        let runtime = self.loader.runtime();
        let item_path = item.get_file_path();
        runtime.spawn(async move {
            log::info!("[TileGridView] async task started for: {:?}", item_path);
            let items = item.get_subitems(&cancel_token).await;
            let count = items.as_ref().map(|v| v.len()).unwrap_or(0);
            log::info!("[TileGridView] async task completed for: {:?}, got {} items", item_path, count);
            // Send result (ignore error if receiver dropped)
            let _ = tx.send(items.unwrap_or_default());
        });
    }

    /// Evict oldest thumbnails from memory when we exceed the limit
    /// This frees up memory by resetting old thumbnails back to Pending state
    fn evict_old_thumbnails(&mut self, keep_visible_top: f32, keep_visible_bottom: f32) {
        // Only evict if we're over the limit
        if self.loaded_lru.len() <= MAX_LOADED_THUMBNAILS {
            return;
        }

        let to_evict = self.loaded_lru.len() - MAX_LOADED_THUMBNAILS;
        let layout = self.layout.borrow();

        // Find indices to evict (from front of LRU, which are oldest)
        let mut evict_indices = Vec::new();
        for &idx in self.loaded_lru.iter().take(to_evict * 2) {
            // Don't evict if it's in the visible range
            if let Some(tile) = layout.iter().find(|t| t.index == idx) {
                if tile.y + tile.height >= keep_visible_top && tile.y <= keep_visible_bottom {
                    continue; // Skip - it's visible
                }
            }
            evict_indices.push(idx);
            if evict_indices.len() >= to_evict {
                break;
            }
        }
        drop(layout);

        // Evict the selected thumbnails
        for idx in &evict_indices {
            if let Some(thumb) = self.thumbnails.get_mut(*idx) {
                // Only evict Ready thumbnails, not Loading ones
                if matches!(thumb.state, ThumbnailState::Ready { .. }) {
                    thumb.state = ThumbnailState::Pending { placeholder: None };
                    // Remove from LRU
                    self.loaded_lru.retain(|&i| i != *idx);
                }
            }
        }

        if !evict_indices.is_empty() {
            log::debug!("[TileGridView] Evicted {} thumbnails to stay under limit", evict_indices.len());
        }
    }

    /// Add an index to the LRU tracker (marks it as recently used)
    fn mark_as_loaded(&mut self, index: usize) {
        // Remove if already in list (will re-add at end)
        self.loaded_lru.retain(|&i| i != index);
        // Add to end (most recently used)
        self.loaded_lru.push(index);
    }

    /// Queue thumbnail loading for items in the visible range
    /// This is the main lazy-loading method that manages memory usage
    fn queue_visible_range_loads(&mut self, viewport_top: f32, viewport_height: f32) {
        let viewport_bottom = viewport_top + viewport_height;
        let load_top = viewport_top - PRELOAD_BUFFER_PX;
        let load_bottom = viewport_bottom + PRELOAD_BUFFER_PX;

        // First, evict old thumbnails if we're over the limit
        self.evict_old_thumbnails(load_top, load_bottom);

        // Collect indices in visible range
        let layout = self.layout.borrow();
        let visible_layout_indices: Vec<usize> = layout
            .iter()
            .enumerate()
            .filter(|(_, tile)| tile.y + tile.height >= load_top && tile.y <= load_bottom)
            .map(|(i, _)| i)
            .collect();
        drop(layout);

        // Handle sync thumbnails and folders immediately (no memory cost for placeholders)
        for &layout_idx in &visible_layout_indices {
            let actual_idx = self.layout.borrow().get(layout_idx).map(|t| t.index).unwrap_or(layout_idx);

            // Skip if not pending
            let is_pending = self
                .thumbnails
                .get(actual_idx)
                .map_or(false, |t| matches!(t.state, ThumbnailState::Pending { .. }));
            if !is_pending {
                continue;
            }

            // Check if item has a sync thumbnail (no thread needed)
            if let Some(item) = self.items.get(actual_idx) {
                if let Some(rgba) = item.get_sync_thumbnail() {
                    if let Some(thumb) = self.thumbnails.get_mut(actual_idx) {
                        // Render label separately for GPU (don't embed in image)
                        thumb.label_rgba = render_label_tag(&thumb.label, 1);
                        thumb.state = ThumbnailState::Ready { rgba };
                        self.mark_as_loaded(actual_idx);
                    }
                    continue;
                }
            }
        }

        // Collect indices that need async loading
        let mut indices_to_load: Vec<(usize, u32)> = Vec::new();
        let layout = self.layout.borrow();

        for &layout_idx in &visible_layout_indices {
            let actual_idx = layout.get(layout_idx).map(|t| t.index).unwrap_or(layout_idx);
            let tile_y = layout.get(layout_idx).map(|t| t.y).unwrap_or(0.0);

            // Skip if not pending
            let is_pending = self
                .thumbnails
                .get(actual_idx)
                .map_or(false, |t| matches!(t.state, ThumbnailState::Pending { .. }));
            if !is_pending {
                continue;
            }

            // Calculate priority based on distance from center of viewport
            let viewport_center = viewport_top + viewport_height / 2.0;
            let distance = (tile_y - viewport_center).abs() as u32;
            indices_to_load.push((actual_idx, distance));
        }
        drop(layout);

        // Sort by priority (closest to viewport center first)
        indices_to_load.sort_by_key(|(_, priority)| *priority);

        // Limit how many we queue at once to prevent overwhelming the loader
        let max_queue = 20;
        for (actual_idx, priority) in indices_to_load.into_iter().take(max_queue) {
            if let Some(item) = self.items.get(actual_idx) {
                self.loader.load(ThumbnailRequest { item: item.clone(), priority });
                if let Some(thumb) = self.thumbnails.get_mut(actual_idx) {
                    thumb.state = ThumbnailState::Loading {
                        placeholder: thumb.state.placeholder().cloned(),
                    };
                }
            }
        }
    }

    /// Queue thumbnail loading for items in the visible range (for scroll events)
    /// DEPRECATED: Use queue_visible_range_loads instead
    fn queue_visible_loads(&mut self, viewport_top: f32, viewport_height: f32) {
        self.queue_visible_range_loads(viewport_top, viewport_height);
    }

    /// Load thumbnail for an item at given index using a cloned item
    pub fn load_thumbnail_from_item(&mut self, index: usize, item: Arc<dyn Item>) {
        if let Some(thumb) = self.thumbnails.get(index) {
            if matches!(thumb.state, ThumbnailState::Pending { .. } | ThumbnailState::Loading { .. }) {
                let priority = if let Some(tile) = self.layout.borrow().get(index) {
                    (tile.y - self.viewport.scroll_y).abs() as u32
                } else {
                    1000
                };

                self.loader.load(ThumbnailRequest { item, priority });

                if let Some(thumb) = self.thumbnails.get_mut(index) {
                    thumb.state = ThumbnailState::Loading {
                        placeholder: thumb.state.placeholder().cloned(),
                    };
                }
            }
        }
    }

    /// Recalculate layout based on available width
    /// Uses a masonry/bin-packing algorithm - each tile goes to the shortest column
    /// Only includes visible (non-filtered) items
    pub fn recalculate_layout(&self, width: f32) {
        let current_available = *self.available_width.borrow();
        let layout_empty = self.layout.borrow().is_empty();
        let diff = (width - current_available).abs();

        if diff < 1.0 && !layout_empty {
            return; // No significant change
        }
        *self.available_width.borrow_mut() = width;
        self.layout.borrow_mut().clear();

        if self.visible_indices.is_empty() || width < 100.0 {
            *self.content_height.borrow_mut() = 0.0;
            return;
        }

        // Calculate layout configuration
        let tile_width = TILE_BASE_WIDTH;
        let outer_margin = 2.0;
        let num_columns = MasonryConfig::columns_for_width(width, tile_width, TILE_SPACING, outer_margin);

        let config = MasonryConfig::new(tile_width, TILE_SPACING, outer_margin, num_columns);

        // Build item sizes for masonry layout
        let item_sizes: Vec<ItemSize> = self
            .visible_indices
            .iter()
            .filter_map(|&actual_index| {
                let thumb = self.thumbnails.get(actual_index)?;
                let column_span = thumb.get_width_multiplier() as usize;

                // Calculate item width based on column span
                let item_width = if column_span == 1 {
                    tile_width
                } else {
                    tile_width * column_span as f32 + TILE_SPACING * (column_span - 1) as f32
                };

                // Get the image display height (aspect-ratio based)
                let content_width = item_width - (TILE_PADDING * 2.0);
                let image_height = thumb.display_height(content_width);

                // No height capping - multi-pass rendering handles tall tiles
                let capped_height = image_height;

                // Total tile height: TILE_PADDING (top) + image_height + TILE_PADDING (bottom)
                let total_height = TILE_PADDING + capped_height + TILE_PADDING;

                Some(ItemSize {
                    index: actual_index,
                    column_span,
                    height: total_height,
                })
            })
            .collect();

        // Calculate masonry layout
        let masonry_result = masonry_layout::calculate_masonry_layout(&config, &item_sizes);

        // Convert to LayoutTile format
        let mut layout = self.layout.borrow_mut();
        for item in masonry_result.items {
            layout.push(LayoutTile {
                index: item.index,
                x: item.x,
                y: item.y,
                width: item.width,
                height: item.height,
            });
        }

        *self.content_height.borrow_mut() = masonry_result.content_height;
        self.cache.clear();
    }

    /// Process pending thumbnail results
    pub fn poll_results(&mut self) -> Vec<TileGridMessage> {
        // Check for completed subitem loading first
        let has_rx = self.subitems_rx.is_some();
        if has_rx {
            log::info!("[TileGridView] poll_results: checking subitems_rx");
        }
        if let Some(mut rx) = self.subitems_rx.take() {
            match rx.try_recv() {
                Ok(items) => {
                    log::info!("[TileGridView] poll_results: received {} subitems", items.len());
                    // Subitems loaded - set them directly
                    self.set_items_from_items(items);
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                    // Still loading - put receiver back
                    log::info!("[TileGridView] poll_results: still loading, putting rx back");
                    self.subitems_rx = Some(rx);
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                    log::warn!("[TileGridView] poll_results: channel closed unexpectedly");
                    // Channel closed (cancelled or error) - do nothing
                }
            }
        }

        let mut messages = Vec::new();
        while let Ok(result) = self.result_rx.try_recv() {
            // Find the thumbnail by path - try exact match first, then filename match
            let found = self.thumbnails.iter_mut().enumerate().find(|(_, t)| t.path == result.path);

            let thumb_opt = if found.is_some() {
                found
            } else {
                // Fallback: match by filename only (in case paths differ slightly)
                let result_filename = &result.path;
                self.thumbnails
                    .iter_mut()
                    .enumerate()
                    .find(|(_, t)| &t.path == result_filename && matches!(t.state, ThumbnailState::Loading { .. } | ThumbnailState::Pending { .. }))
            };

            if let Some((idx, thumb)) = thumb_opt {
                // For Error state without placeholder, create one with the label
                let new_state = match &result.state {
                    ThumbnailState::Error { message, placeholder: None } => {
                        let placeholder = create_labeled_placeholder(&ERROR_PLACEHOLDER, &thumb.label);
                        ThumbnailState::Error {
                            message: message.clone(),
                            placeholder: Some(placeholder),
                        }
                    }
                    other => other.clone(),
                };
                thumb.state = new_state.clone();
                thumb.sauce_info = result.sauce_info.clone();
                thumb.width_multiplier = result.width_multiplier;
                thumb.label_rgba = result.label_rgba.clone();
                // Track this thumbnail in LRU (it's now loaded)
                self.mark_as_loaded(idx);
                messages.push(TileGridMessage::ThumbnailReady(result.path.clone(), new_state));
            }
        }

        if !messages.is_empty() {
            self.cache.clear();
            // Recalculate layout since heights may have changed
            let width = *self.available_width.borrow();
            *self.available_width.borrow_mut() = 0.0; // Force recalc
            self.recalculate_layout(width);
        }

        messages
    }

    /// Handle animation tick
    pub fn tick(&mut self, delta_seconds: f32) {
        self.blink_timer += delta_seconds;

        // Update scrollbar animation
        self.scrollbar.update_animation();

        // Update smooth scroll animation
        self.viewport.update_animation();

        // Queue visible range loads on every tick (lazy loading)
        // This ensures thumbnails get loaded even when view() is called with &self
        let viewport_top = self.viewport.scroll_y;
        let viewport_height = self.viewport.visible_height;
        if viewport_height > 0.0 {
            self.queue_visible_range_loads(viewport_top, viewport_height);
        }

        // Handle auto-scroll mode
        if self.auto_scroll_active {
            let scroll_speed = self.scroll_speed.get_speed();
            let scroll_delta = scroll_speed * delta_seconds;
            let current_y = self.viewport.scroll_y;
            let max_scroll_y = self.viewport.max_scroll_y();
            let new_y = (current_y + scroll_delta).min(max_scroll_y);

            self.viewport.scroll_y_to_immediate(new_y);

            // Stop auto-scroll when we reach the bottom
            if new_y >= max_scroll_y {
                self.auto_scroll_active = false;
            }
        }

        // Toggle blink every 0.5 seconds
        if self.blink_timer >= 0.5 {
            self.blink_timer = 0.0;
            self.blink_on = !self.blink_on;

            // Advance animated thumbnails
            for thumb in &mut self.thumbnails {
                if self.blink_on {
                    thumb.state.next_frame();
                }
            }

            self.cache.clear();
        }
    }

    /// Check if any thumbnails are animated
    pub fn has_animated(&self) -> bool {
        self.thumbnails.iter().any(|t| t.state.is_animated())
    }

    /// Check if animation/polling is needed (loading thumbnails or animated content)
    pub fn needs_animation(&self) -> bool {
        // Check if any visible (filtered) thumbnails are still loading or need to be loaded
        // Note: map_or(true, ...) treats missing entries as "needs loading"
        let has_loading_or_missing = self.visible_indices.iter().any(|&idx| {
            self.thumbnails
                .get(idx)
                .map_or(true, |t| matches!(t.state, ThumbnailState::Loading { .. } | ThumbnailState::Pending { .. }))
        });

        // Or if any thumbnails have blinking/animated content
        // Or if scrollbar needs animation updates
        // Or if viewport is animating (smooth scroll)
        // Or if auto-scroll is active
        // Or if we're waiting for async subitems to load
        has_loading_or_missing
            || self.has_animated()
            || self.scrollbar.needs_animation()
            || self.viewport.is_animating()
            || self.auto_scroll_active
            || self.subitems_rx.is_some()
    }

    // ==================== Keyboard Navigation ====================

    /// Ensure the selected tile is visible, scrolling immediately if not
    fn ensure_visible_immediate(&mut self, index: usize) {
        let layout = self.layout.borrow();
        let Some(tile) = layout.get(index) else { return };

        let tile_top = tile.y;
        let tile_bottom = tile.y + tile.height;
        let visible_top = self.viewport.scroll_y;
        let visible_bottom = visible_top + self.viewport.visible_height;

        // Small margin so tile isn't right at edge
        let margin = 8.0;

        drop(layout);

        if tile_top < visible_top + margin {
            // Tile is above viewport - scroll up immediately
            self.scroll_to_immediate((tile_top - margin).max(0.0));
        } else if tile_bottom > visible_bottom - margin {
            // Tile is below viewport - scroll down immediately
            let target = tile_bottom + margin - self.viewport.visible_height;
            self.scroll_to_immediate(target.min(self.max_scroll()));
        }
        // If tile is already visible, don't scroll at all
    }

    /// Find the tile visually above the current tile
    fn find_tile_above(&self, current: usize) -> Option<usize> {
        let layout = self.layout.borrow();
        let current_tile = layout.get(current)?;
        let current_center_x = current_tile.x + current_tile.width / 2.0;
        let current_top = current_tile.y;

        let mut best_index = None;
        let mut best_y = f32::MIN;
        let mut best_x_dist = f32::MAX;

        for (i, tile) in layout.iter().enumerate() {
            // Must be above current tile (tile bottom should be above current top)
            let tile_bottom = tile.y + tile.height;
            if tile_bottom > current_top - 1.0 {
                continue;
            }

            let tile_center_x = tile.x + tile.width / 2.0;
            let x_dist = (tile_center_x - current_center_x).abs();

            // Prefer tiles that are:
            // 1. Closest to current Y (highest Y value that's still above)
            // 2. With smallest X distance as tiebreaker
            if tile.y > best_y || (tile.y == best_y && x_dist < best_x_dist) {
                best_y = tile.y;
                best_x_dist = x_dist;
                best_index = Some(i);
            }
        }

        best_index
    }

    /// Find the tile visually below the current tile
    fn find_tile_below(&self, current: usize) -> Option<usize> {
        let layout = self.layout.borrow();
        let current_tile = layout.get(current)?;
        let current_center_x = current_tile.x + current_tile.width / 2.0;
        let current_bottom = current_tile.y + current_tile.height;

        let mut best_index = None;
        let mut best_y = f32::MAX;
        let mut best_x_dist = f32::MAX;

        for (i, tile) in layout.iter().enumerate() {
            // Must be below current tile (tile top should be below current bottom)
            if tile.y < current_bottom + 1.0 {
                continue;
            }

            let tile_center_x = tile.x + tile.width / 2.0;
            let x_dist = (tile_center_x - current_center_x).abs();

            // Prefer tiles that are:
            // 1. Closest to current Y (lowest Y value that's still below)
            // 2. With smallest X distance as tiebreaker
            if tile.y < best_y || (tile.y == best_y && x_dist < best_x_dist) {
                best_y = tile.y;
                best_x_dist = x_dist;
                best_index = Some(i);
            }
        }

        best_index
    }

    /// Find the tile immediately to the left (no wrapping)
    fn find_tile_left(&self, current: usize) -> Option<usize> {
        let layout = self.layout.borrow();
        let current_tile = layout.get(current)?;
        let current_center_y = current_tile.y + current_tile.height / 2.0;
        let current_left = current_tile.x;

        let mut best_index = None;
        let mut best_x = f32::MIN;

        for (i, tile) in layout.iter().enumerate() {
            // Must be to the left
            let tile_right = tile.x + tile.width;
            if tile_right > current_left - 1.0 {
                continue;
            }

            // Check if on roughly the same row (within tile height tolerance)
            let tile_center_y = tile.y + tile.height / 2.0;
            let y_diff = (tile_center_y - current_center_y).abs();

            if y_diff > current_tile.height * 0.7 {
                continue; // Not on the same row
            }

            // Prefer the rightmost tile that's still to our left
            if tile.x > best_x {
                best_x = tile.x;
                best_index = Some(i);
            }
        }

        best_index
    }

    /// Find the tile immediately to the right (no wrapping)
    fn find_tile_right(&self, current: usize) -> Option<usize> {
        let layout = self.layout.borrow();
        let current_tile = layout.get(current)?;
        let current_center_y = current_tile.y + current_tile.height / 2.0;
        let current_right = current_tile.x + current_tile.width;

        let mut best_index = None;
        let mut best_x = f32::MAX;

        for (i, tile) in layout.iter().enumerate() {
            // Must be to the right
            if tile.x < current_right + 1.0 {
                continue;
            }

            // Check if on roughly the same row (within tile height tolerance)
            let tile_center_y = tile.y + tile.height / 2.0;
            let y_diff = (tile_center_y - current_center_y).abs();

            if y_diff > current_tile.height * 0.7 {
                continue; // Not on the same row
            }

            // Prefer the leftmost tile that's still to our right
            if tile.x < best_x {
                best_x = tile.x;
                best_index = Some(i);
            }
        }

        best_index
    }

    /// Select the tile above (or first if none selected)
    fn select_up(&mut self) {
        if self.visible_indices.is_empty() {
            return;
        }

        match self.selected_index {
            Some(current) => {
                if let Some(new_index) = self.find_tile_above(current) {
                    self.selected_index = Some(new_index);
                    self.ensure_visible_immediate(new_index);
                }
                // If no tile above, don't change selection or scroll
            }
            None => {
                let first = self.visible_indices.first().copied().unwrap_or(0);
                self.selected_index = Some(first);
                self.ensure_visible_immediate(first);
            }
        }
    }

    /// Select the tile below (or first if none selected)
    fn select_down(&mut self) {
        if self.visible_indices.is_empty() {
            return;
        }

        match self.selected_index {
            Some(current) => {
                if let Some(new_index) = self.find_tile_below(current) {
                    self.selected_index = Some(new_index);
                    self.ensure_visible_immediate(new_index);
                }
                // If no tile below, don't change selection or scroll
            }
            None => {
                let first = self.visible_indices.first().copied().unwrap_or(0);
                self.selected_index = Some(first);
                self.ensure_visible_immediate(first);
            }
        }
    }

    /// Select the tile to the left (or first if none selected)
    fn select_left(&mut self) {
        if self.visible_indices.is_empty() {
            return;
        }

        match self.selected_index {
            Some(current) => {
                if let Some(new_index) = self.find_tile_left(current) {
                    self.selected_index = Some(new_index);
                    self.ensure_visible_immediate(new_index);
                }
                // If no tile to the left, don't change selection or scroll
            }
            None => {
                let first = self.visible_indices.first().copied().unwrap_or(0);
                self.selected_index = Some(first);
                self.ensure_visible_immediate(first);
            }
        }
    }

    /// Select the tile to the right (or first if none selected)
    fn select_right(&mut self) {
        if self.visible_indices.is_empty() {
            return;
        }

        match self.selected_index {
            Some(current) => {
                if let Some(new_index) = self.find_tile_right(current) {
                    self.selected_index = Some(new_index);
                    self.ensure_visible_immediate(new_index);
                }
                // If no tile to the right, don't change selection or scroll
            }
            None => {
                let first = self.visible_indices.first().copied().unwrap_or(0);
                self.selected_index = Some(first);
                self.ensure_visible_immediate(first);
            }
        }
    }

    /// Page up - scroll viewport up by one page (no selection change)
    fn page_up(&mut self) {
        // Use last_bounds.height as it's more reliably updated than viewport.visible_height
        let visible_height = self.last_bounds.borrow().height.max(self.viewport.visible_height);
        // Scroll up by nearly a full page (leave some overlap for context)
        let page_height = (visible_height - 50.0).max(100.0);
        self.scroll_by(page_height);
        self.scrollbar.mark_interaction(true);
    }

    /// Page down - scroll viewport down by one page (no selection change)
    fn page_down(&mut self) {
        // Use last_bounds.height as it's more reliably updated than viewport.visible_height
        let visible_height = self.last_bounds.borrow().height.max(self.viewport.visible_height);
        // Scroll down by nearly a full page (leave some overlap for context)
        let page_height = (visible_height - 50.0).max(100.0);
        self.scroll_by(-page_height);
        self.scrollbar.mark_interaction(true);
    }

    /// Go to first tile with smooth scroll
    fn go_home(&mut self) {
        if self.visible_indices.is_empty() {
            return;
        }

        let first = self.visible_indices.first().copied().unwrap_or(0);
        self.selected_index = Some(first);
        // Smooth scroll to top
        self.scroll_to(0.0);
    }

    /// Go to last tile with smooth scroll
    fn go_end(&mut self) {
        if self.visible_indices.is_empty() {
            return;
        }

        let last = self.visible_indices.last().copied().unwrap_or(0);
        self.selected_index = Some(last);
        // Smooth scroll to bottom
        self.scroll_to(self.max_scroll());
    }

    /// Get the number of thumbnails
    pub fn len(&self) -> usize {
        self.thumbnails.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.thumbnails.is_empty()
    }

    /// Get selected thumbnail path
    pub fn selected_path(&self) -> Option<&String> {
        self.selected_index.and_then(|i| self.thumbnails.get(i).map(|t| &t.path))
    }

    /// Get selected item's info (path, label, is_container)
    pub fn get_selected_info(&self) -> Option<(String, String, bool)> {
        self.selected_index.and_then(|i| {
            let thumb = self.thumbnails.get(i)?;
            let is_container = *self.is_container.get(i)?;
            Some((thumb.path.clone(), thumb.label.clone(), is_container))
        })
    }

    /// Select an item by its label
    /// Returns true if the item was found and selected
    pub fn select_by_label(&mut self, label: &str) -> bool {
        for (i, thumb) in self.thumbnails.iter().enumerate() {
            if thumb.label == label {
                self.selected_index = Some(i);
                self.ensure_visible_immediate(i);
                return true;
            }
        }
        false
    }

    /// Get the current viewport height
    pub fn get_viewport_height(&self) -> f32 {
        self.viewport.visible_height
    }

    /// Get the selected item (if any)
    pub fn get_selected_item(&self) -> Option<Arc<dyn Item>> {
        let index = self.selected_index?;
        self.items.get(index).cloned()
    }

    /// Get item at index
    pub fn get_item_at(&self, index: usize) -> Option<Arc<dyn Item>> {
        self.items.get(index).cloned()
    }

    /// Get the last known bounds width
    pub fn get_bounds_width(&self) -> f32 {
        self.last_bounds.borrow().width
    }

    /// Get the index of the currently hovered tile (if any)
    pub fn get_hovered_index(&self) -> Option<usize> {
        let hovered_id = (*self.shared_hovered_tile.lock())?;
        self.tile_ids.iter().position(|&id| id == hovered_id)
    }

    /// Get status info for the hovered tile, or selected tile if nothing is hovered
    /// Returns (path, label, is_container, sauce_info)
    pub fn get_status_info(&self) -> Option<(String, String, bool, Option<icy_sauce::SauceRecord>)> {
        // First try hovered tile
        let index = self.get_hovered_index().or(self.selected_index)?;

        let thumb = self.thumbnails.get(index)?;
        let is_container = *self.is_container.get(index)?;
        Some((thumb.path.clone(), thumb.label.clone(), is_container, thumb.sauce_info.clone()))
    }

    /// Get item info at index
    pub fn get_item_info(&self, index: usize) -> Option<(String, String, bool)> {
        let thumb = self.thumbnails.get(index)?;
        let is_container = *self.is_container.get(index)?;
        Some((thumb.path.clone(), thumb.label.clone(), is_container))
    }

    /// View the tile grid with a given width for responsive layout
    pub fn view_with_width(&mut self, width: f32, height: f32) -> Element<'_, TileGridMessage> {
        // Recalculate layout if width changed
        self.recalculate_layout(width);

        // Sync viewport with content dimensions
        let content_height = *self.content_height.borrow();
        self.viewport.set_visible_size(width, height);
        self.viewport.set_content_size(width, content_height);

        // Update bounds
        {
            let mut bounds = self.last_bounds.borrow_mut();
            bounds.width = width;
            bounds.height = height;
        }

        // Lazy loading: always queue thumbnails for the visible range
        // The queue_visible_range_loads method is efficient and won't re-queue already loading items
        let viewport_top = self.viewport.scroll_y;
        self.queue_visible_range_loads(viewport_top, height);

        self.view_shader_with_size(width, height)
    }

    /// View the tile grid using responsive container to detect width
    pub fn view(&self) -> Element<'_, TileGridMessage> {
        use iced::widget::responsive;

        if self.thumbnails.is_empty() {
            return container(text("No files to display - click grid icon to load").size(14))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .style(|theme: &iced::Theme| container::Style {
                    background: Some(iced::Background::Color(theme::main_area_background(theme))),
                    ..Default::default()
                })
                .into();
        }

        // Use responsive to get the actual width
        responsive(|size| {
            let width = size.width;
            let height = size.height;
            self.view_shader_with_size(width, height)
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    /// Build shader view with given dimensions
    fn view_shader_with_size(&self, width: f32, height: f32) -> Element<'_, TileGridMessage> {
        // Recalculate layout if width changed
        self.recalculate_layout(width);

        // Sync last_bounds with current dimensions (for page up/down calculations)
        {
            let mut bounds = self.last_bounds.borrow_mut();
            bounds.width = width;
            bounds.height = height;
        }

        // Build tile textures from thumbnails - only process visible tiles
        let mut tiles = Vec::with_capacity(self.visible_indices.len().min(50)); // Pre-allocate for visible tiles
        let layout = self.layout.borrow();

        // Iterate over layout tiles (which only contain visible/filtered items)
        for (layout_idx, layout_tile) in layout.iter().enumerate() {
            let actual_index = layout_tile.index;
            let Some(thumb) = self.thumbnails.get(actual_index) else { continue };
            let Some(&tile_id) = self.tile_ids.get(actual_index) else { continue };

            // Get layout position - layout_tile.height is the TOTAL tile height including label
            let (x, y, tile_width, total_tile_height) = (layout_tile.x, layout_tile.y, layout_tile.width, layout_tile.height);

            // Early visibility check - skip tiles outside viewport
            let scroll_offset = self.viewport.scroll_y;
            let tile_top = y - scroll_offset;
            let tile_bottom = tile_top + total_tile_height;
            if tile_bottom < 0.0 || tile_top > height {
                continue; // Skip tiles outside viewport
            }

            // Get RGBA data based on state - use pre-rendered placeholders with labels
            let rgba = match &thumb.state {
                ThumbnailState::Ready { rgba } => rgba,
                ThumbnailState::Animated { frames, current_frame } => frames.get(*current_frame).unwrap_or(&*LOADING_PLACEHOLDER),
                ThumbnailState::Loading { placeholder } | ThumbnailState::Pending { placeholder } => placeholder.as_ref().unwrap_or(&*LOADING_PLACEHOLDER),
                ThumbnailState::Error { placeholder, .. } => placeholder.as_ref().unwrap_or(&*ERROR_PLACEHOLDER),
            };

            let is_selected = self.selected_index == Some(layout_idx);
            let is_hovered = *self.shared_hovered_tile.lock() == Some(tile_id);

            // Get label RGBA data for separate GPU rendering
            // label_size stores the RAW texture dimensions for texture creation
            // The shader will scale the texture to fit the label_rect
            let (label_rgba, label_raw_size) = if let Some(ref label) = thumb.label_rgba {
                (Some(label.data.clone()), (label.width, label.height))
            } else {
                (None, (0, 0))
            };

            // Calculate the displayed image height - raw texture scaled to fit content_width
            // This is the height the image will be rendered at, not the raw texture height
            let content_width = tile_width - (TILE_PADDING * 2.0);
            let scale = if rgba.width > 0 { content_width / rgba.width as f32 } else { 1.0 };
            let scaled_image_height = rgba.height as f32 * scale;

            // No height capping - multi-pass rendering handles tall tiles
            let image_height = scaled_image_height;

            // Label is rendered at 2x scale for readability
            // label_size = scaled label dimensions for shader uniforms
            const LABEL_SCALE: u32 = 2;
            let label_size = (label_raw_size.0 * LABEL_SCALE, label_raw_size.1 * LABEL_SCALE);

            tiles.push(TileTexture {
                id: tile_id,
                rgba_data: rgba.data.clone(),
                width: rgba.width,
                height: rgba.height,
                label_rgba,
                label_raw_size,
                label_size,
                position: (x, y),
                display_size: (tile_width, total_tile_height),
                image_height,
                is_selected,
                is_hovered,
            });
        }
        drop(layout); // Release borrow before creating program

        let content_height = *self.content_height.borrow();

        // Create shader primitive
        let program = TileShaderProgramWrapper {
            tiles,
            scroll_y: self.viewport.scroll_y,
            content_height,
            _viewport_height: height,
            _selected_tile_id: self.selected_index.and_then(|i| self.tile_ids.get(i).copied()),
            shared_hovered_tile: self.shared_hovered_tile.clone(),
            background_color: self.background_color.clone(),
        };

        // Build the shader widget with background color
        let shader_widget = container(shader(program).width(Length::Fill).height(Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|theme: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(theme::main_area_background(theme))),
                ..Default::default()
            });

        // Create overlay scrollbar if content is taller than viewport
        // Use content_height from RefCell directly since viewport may not be synced yet
        let max_scroll = (content_height - height).max(0.0);
        if max_scroll > 0.0 {
            // Calculate scroll ratio and height ratio for scrollbar
            let scroll_ratio = if max_scroll > 0.0 {
                (self.viewport.scroll_y / max_scroll).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let height_ratio = (height / content_height).clamp(0.0, 1.0);

            let scrollbar_overlay = ScrollbarOverlay::new(
                self.scrollbar.visibility,
                scroll_ratio,
                height_ratio,
                max_scroll,
                self.scrollbar_hover_state.clone(),
                |_x, y| TileGridMessage::ScrollbarScroll(0.0, y),
                TileGridMessage::ScrollbarHover,
            );

            // Use stack to overlay scrollbar on top of shader widget
            // Scrollbar aligned to right edge
            stack![
                shader_widget,
                container(scrollbar_overlay.view().map(|m| m))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .align_x(iced::alignment::Horizontal::Right)
                    .align_y(iced::alignment::Vertical::Center)
            ]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
        } else {
            shader_widget.into()
        }
    }

    /// Update with a message, returns true if an item should be opened
    pub fn update(&mut self, message: TileGridMessage) -> bool {
        match message {
            TileGridMessage::TileClicked(index) => {
                self.selected_index = Some(index);
                false
            }
            TileGridMessage::TileDoubleClicked(index) => {
                self.selected_index = Some(index);
                // Signal that item should be opened
                true
            }
            TileGridMessage::Scrolled(delta) => {
                self.scroll_by(delta);
                // Mark scrollbar interaction for visibility
                self.scrollbar.mark_interaction(true);
                // Queue loading for newly visible items
                let scroll = self.viewport.scroll_y;
                let vh = self.viewport.visible_height;
                self.queue_visible_loads(scroll, vh);
                false
            }
            TileGridMessage::AnimationTick => {
                // Handled in tick()
                false
            }
            TileGridMessage::ThumbnailReady(_, _) => {
                // Already handled in poll_results
                false
            }
            TileGridMessage::WidthChanged(width) => {
                self.recalculate_layout(width);
                self.viewport.clamp_scroll();
                false
            }
            TileGridMessage::ScrollbarScroll(_x, y) => {
                // y is absolute scroll position
                self.scroll_to_immediate(y.clamp(0.0, self.max_scroll()));
                self.scrollbar.mark_interaction(true);
                // Sync scrollbar position
                let max_scroll = self.max_scroll();
                if max_scroll > 0.0 {
                    self.scrollbar.set_scroll_position(self.viewport.scroll_y / max_scroll);
                }
                // Queue loading for newly visible items
                let scroll = self.viewport.scroll_y;
                let vh = self.viewport.visible_height;
                self.queue_visible_loads(scroll, vh);
                false
            }
            TileGridMessage::ScrollbarHover(hovered) => {
                self.scrollbar.set_hovered(hovered);
                false
            }
            TileGridMessage::SelectPrevious => {
                self.select_up();
                false
            }
            TileGridMessage::SelectNext => {
                self.select_down();
                false
            }
            TileGridMessage::SelectLeft => {
                self.select_left();
                false
            }
            TileGridMessage::SelectRight => {
                self.select_right();
                false
            }
            TileGridMessage::PageUp => {
                self.page_up();
                false
            }
            TileGridMessage::PageDown => {
                self.page_down();
                false
            }
            TileGridMessage::Home => {
                self.go_home();
                false
            }
            TileGridMessage::End => {
                self.go_end();
                false
            }
            TileGridMessage::OpenSelected => self.selected_index.is_some(),
        }
    }

    /// Handle mouse events for scroll wheel and hover
    pub fn handle_mouse_event(&mut self, event: &iced::Event, bounds: Rectangle, cursor_position: Option<Point>) -> bool {
        // Update viewport size and bounds
        self.viewport.set_visible_size(bounds.width, bounds.height);
        *self.last_bounds.borrow_mut() = bounds;

        // Sync content height from RefCell to viewport (layout updates the RefCell)
        let content_height = *self.content_height.borrow();
        if (self.viewport.content_height - content_height).abs() > 1.0 {
            self.viewport.set_content_size(bounds.width, content_height);
        }

        // Track if viewport changed due to resize
        let scroll_changed = self.viewport.is_animating();

        // Update last cursor position
        if let Some(pos) = cursor_position {
            self.last_cursor_position = Some(pos);
        }

        match event {
            iced::Event::Mouse(iced::mouse::Event::WheelScrolled { delta }) => {
                let pos = cursor_position.or(self.last_cursor_position);
                if let Some(pos) = pos {
                    if bounds.contains(pos) {
                        let scroll_delta = match delta {
                            iced::mouse::ScrollDelta::Lines { y, .. } => *y * 50.0,
                            iced::mouse::ScrollDelta::Pixels { y, .. } => *y,
                        };
                        self.scroll_by(scroll_delta);
                        // Mark interaction for scrollbar visibility
                        self.scrollbar.mark_interaction(true);
                        // Sync scrollbar position
                        let max_scroll = self.max_scroll();
                        if max_scroll > 0.0 {
                            self.scrollbar.set_scroll_position(self.viewport.scroll_y / max_scroll);
                        }
                        return true;
                    }
                }
            }
            iced::Event::Mouse(iced::mouse::Event::CursorMoved { .. }) => {
                // Update hover state
                if let Some(pos) = cursor_position {
                    if bounds.contains(pos) {
                        // Calculate position relative to widget
                        let rel_x = pos.x - bounds.x;
                        let rel_y = pos.y - bounds.y + self.viewport.scroll_y;

                        // Find which tile is under the cursor
                        let mut found_tile = None;
                        let layout = self.layout.borrow();
                        for (i, layout_tile) in layout.iter().enumerate() {
                            if rel_x >= layout_tile.x
                                && rel_x < layout_tile.x + layout_tile.width
                                && rel_y >= layout_tile.y
                                && rel_y < layout_tile.y + layout_tile.height
                            {
                                if let Some(&tile_id) = self.tile_ids.get(i) {
                                    found_tile = Some(tile_id);
                                    break;
                                }
                            }
                        }
                        drop(layout);

                        let current = *self.shared_hovered_tile.lock();
                        if current != found_tile {
                            *self.shared_hovered_tile.lock() = found_tile;
                            return true; // Need to redraw
                        }
                    } else {
                        // Cursor left the widget
                        if self.shared_hovered_tile.lock().is_some() {
                            *self.shared_hovered_tile.lock() = None;
                            return true;
                        }
                    }
                }
            }
            iced::Event::Mouse(iced::mouse::Event::CursorLeft) => {
                self.last_cursor_position = None;
                if self.shared_hovered_tile.lock().is_some() {
                    *self.shared_hovered_tile.lock() = None;
                    return true;
                }
            }
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)) => {
                let pos = cursor_position.or(self.last_cursor_position);
                if let Some(pos) = pos {
                    if bounds.contains(pos) {
                        // Calculate position relative to widget
                        let rel_x = pos.x - bounds.x;
                        let rel_y = pos.y - bounds.y + self.viewport.scroll_y;

                        // Find which tile is under the cursor
                        let layout = self.layout.borrow();
                        for (i, layout_tile) in layout.iter().enumerate() {
                            if rel_x >= layout_tile.x
                                && rel_x < layout_tile.x + layout_tile.width
                                && rel_y >= layout_tile.y
                                && rel_y < layout_tile.y + layout_tile.height
                            {
                                // Found clicked tile
                                let now = Instant::now();

                                // Check for double-click
                                if let Some((last_time, last_index)) = self.last_click {
                                    if last_index == i && now.duration_since(last_time).as_millis() < DOUBLE_CLICK_MS {
                                        // Double-click detected
                                        self.last_click = None;
                                        self.selected_index = Some(i);
                                        self.pending_double_click = true;
                                        return true;
                                    }
                                }

                                // Single click - update selection and record click time
                                self.selected_index = Some(i);
                                self.last_click = Some((now, i));
                                return true;
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        scroll_changed
    }

    /// Check if a double-click was detected and consume the flag
    /// Returns true if a double-click was pending, false otherwise
    pub fn take_pending_double_click(&mut self) -> bool {
        if self.pending_double_click {
            self.pending_double_click = false;
            true
        } else {
            false
        }
    }
}

impl Default for TileGridView {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
fn tile_shadow() -> Shadow {
    Shadow {
        color: Color::from_rgba(0.0, 0.0, 0.0, 0.25),
        offset: iced::Vector::new(2.0, 3.0),
        blur_radius: 6.0,
    }
}

#[allow(dead_code)]
fn tile_hover_shadow() -> Shadow {
    Shadow {
        color: Color::from_rgba(0.0, 0.0, 0.0, 0.35),
        offset: iced::Vector::new(3.0, 4.0),
        blur_radius: 10.0,
    }
}
