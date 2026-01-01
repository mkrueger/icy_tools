mod minimap_shader;

use std::cell::RefCell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use icy_ui::widget::shader;
use icy_ui::{Element, Length, Task, Theme};
use icy_engine::Screen;
use icy_engine_gui::theme::main_area_background;
use icy_engine_gui::tile_cache::MAX_TEXTURE_SLICES;
use icy_engine_gui::{CheckerboardColors, SharedRenderCacheHandle, TileCacheKey, TILE_HEIGHT};
use parking_lot::Mutex;

pub(crate) use minimap_shader::viewport_info_from_effective_view;
use minimap_shader::MinimapProgram;
pub use minimap_shader::ViewportInfo;

static MINIMAP_ID_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Generates a unique instance ID for minimap views
fn generate_minimap_id() -> usize {
    MINIMAP_ID_COUNTER.fetch_add(1, Ordering::Relaxed)
}

/// Shared state between MinimapView and shader for communicating bounds
#[derive(Debug, Default)]
pub struct SharedMinimapState {
    pub available_width: f32,
    pub available_height: f32,
}

/// Cache state for optimizing minimap rendering
#[derive(Debug, Clone)]
struct MinimapCacheState {
    /// Cached texture slices
    cached_slices: Vec<TextureSliceData>,
    /// Cached slice heights
    cached_heights: Vec<u32>,
    /// Total height of cached content
    cached_total_height: u32,
    /// First tile index in cache
    first_tile_idx: i32,
    /// Content dimensions when cached
    content_width: u32,
    content_height: f32,

    /// Whether this cached window contained placeholder slices for missing tiles.
    /// If true, we should refresh until all tiles are available.
    incomplete: bool,
}

/// A single texture slice for tall images
#[derive(Clone, Debug)]
pub struct TextureSliceData {
    /// RGBA pixel data for this slice
    pub rgba_data: Arc<Vec<u8>>,
    /// Width of this slice
    pub width: u32,
    /// Height of this slice
    pub height: u32,
}

/// Messages for the minimap view
#[derive(Clone, Debug)]
pub enum MinimapMessage {
    /// Scroll to normalized position (0.0-1.0) - sent during click and drag
    ScrollTo { norm_x: f32, norm_y: f32 },
    /// Scroll the minimap itself vertically (mouse wheel)
    Scroll(f32),
    /// Ensure viewport is visible in minimap (auto-scroll to follow terminal)
    EnsureViewportVisible(f32, f32),
}

/// A minimap view that displays a scaled-down version of the buffer
/// with a viewport rectangle overlay showing the current visible area.
/// The minimap fills the available width and is scrollable vertically.
///
/// This view uses the Terminal's shared render cache for textures,
/// but maintains its own scroll position that syncs with the terminal viewport.
///
/// **Optimization**: The minimap tracks the buffer version and only re-fetches
/// tiles when the buffer has changed or the visible tile range has changed.
pub struct MinimapView {
    instance_id: usize,
    /// Shared state for communicating bounds from shader
    shared_state: Arc<Mutex<SharedMinimapState>>,
    /// Current scroll position (0.0 = top, 1.0 = fully scrolled down)
    scroll_position: RefCell<f32>,
    /// Checkerboard colors for transparency
    checkerboard_colors: CheckerboardColors,
    /// Cached state for avoiding redundant tile fetches
    cache_state: RefCell<Option<MinimapCacheState>>,
    /// Last computed first tile index (for change detection)
    last_first_tile_idx: RefCell<i32>,
    /// Last computed tile count (for change detection)
    last_tile_count: RefCell<i32>,
}

impl Default for MinimapView {
    fn default() -> Self {
        Self::new()
    }
}

impl MinimapView {
    pub fn new() -> Self {
        Self {
            instance_id: generate_minimap_id(),
            shared_state: Arc::new(Mutex::new(SharedMinimapState::default())),
            scroll_position: RefCell::new(0.0),
            checkerboard_colors: CheckerboardColors::default(),
            cache_state: RefCell::new(None),
            last_first_tile_idx: RefCell::new(-1),
            last_tile_count: RefCell::new(0),
        }
    }

    pub fn available_size(&self) -> (f32, f32) {
        let shared = self.shared_state.lock();
        (
            if shared.available_width > 0.0 { shared.available_width } else { 140.0 },
            if shared.available_height > 0.0 { shared.available_height } else { 600.0 },
        )
    }

    /// Ensure the viewport rectangle is visible in the minimap
    /// Adjusts scroll_position so the viewport is always on screen
    /// Uses interior mutability so it can be called from view()
    ///
    /// viewport_y and viewport_height are normalized (0-1) relative to full buffer
    /// content_width and content_height are in pixels
    fn ensure_viewport_visible(&self, viewport_y: f32, viewport_height: f32, content_width: f32, content_height: f32) {
        let shared = self.shared_state.lock();
        let avail_width = shared.available_width;
        let avail_height = shared.available_height;
        drop(shared);

        if avail_width <= 0.0 || avail_height <= 0.0 || content_height <= 0.0 || content_width <= 0.0 {
            return;
        }

        // Scale factor: minimap fills available width
        let scale = avail_width / content_width;

        // Scaled height of the entire content
        let scaled_content_height = content_height * scale;

        // If content fits in available space, no scrolling needed
        if scaled_content_height <= avail_height {
            *self.scroll_position.borrow_mut() = 0.0;
            return;
        }

        // viewport_y and viewport_height are already normalized (0-1)
        // Convert to scaled pixels
        let viewport_top_scaled = viewport_y * content_height * scale;
        let viewport_height_scaled = viewport_height * content_height * scale;
        let viewport_bottom_scaled = viewport_top_scaled + viewport_height_scaled;

        // Maximum scroll offset in pixels
        let max_scroll_px = scaled_content_height - avail_height;

        // Current scroll offset in pixels
        let current_scroll = *self.scroll_position.borrow();
        let current_scroll_px = current_scroll * max_scroll_px;

        // Visible range in scaled pixels
        let visible_top = current_scroll_px;
        let visible_bottom = current_scroll_px + avail_height;

        // If viewport is larger than visible area, align to viewport top to avoid oscillation
        if viewport_height_scaled >= avail_height {
            let new_scroll_px = viewport_top_scaled;
            let new_scroll = (new_scroll_px / max_scroll_px).clamp(0.0, 1.0);
            *self.scroll_position.borrow_mut() = new_scroll;
            return;
        }

        // Check if viewport is above visible area
        if viewport_top_scaled < visible_top {
            // Scroll up to show viewport at top
            let new_scroll_px = viewport_top_scaled;
            let new_scroll = (new_scroll_px / max_scroll_px).clamp(0.0, 1.0);
            *self.scroll_position.borrow_mut() = new_scroll;
        } else if viewport_bottom_scaled > visible_bottom {
            // Scroll down to show viewport at bottom
            let new_scroll_px = viewport_bottom_scaled - avail_height;
            let new_scroll = (new_scroll_px / max_scroll_px).clamp(0.0, 1.0);
            *self.scroll_position.borrow_mut() = new_scroll;
        }
        // If viewport is within visible area, don't change scroll
    }

    /// Update the minimap view state
    pub fn update(&mut self, message: MinimapMessage) -> Task<MinimapMessage> {
        match message {
            MinimapMessage::ScrollTo { .. } => {
                // Parent handles the actual scrolling
                Task::none()
            }
            MinimapMessage::Scroll(delta) => {
                let mut pos = self.scroll_position.borrow_mut();
                *pos = (*pos + delta).clamp(0.0, 1.0);
                Task::none()
            }
            MinimapMessage::EnsureViewportVisible(_y, _height) => {
                // Handled directly in view() now
                Task::none()
            }
        }
    }

    /// Create the view element for the minimap
    ///
    /// Uses tiles from the Terminal's shared render cache.
    /// The Terminal must have rendered before calling this.
    ///
    /// **Optimization**: Only fetches tiles when:
    /// - Buffer version has changed (content modified)
    /// - Visible tile range has changed (scroll/resize)
    /// - No cached data exists
    ///
    /// Missing tiles are skipped (not synchronously rendered) to avoid blocking.
    /// The Terminal will render them on the next frame.
    pub fn view(
        &self,
        theme: &Theme,
        _screen: &Arc<Mutex<Box<dyn Screen>>>,
        viewport_info: &ViewportInfo,
        render_cache: Option<&SharedRenderCacheHandle>,
    ) -> Element<'_, MinimapMessage> {
        let viewport_color = {
            let c = theme.accent.base;
            [c.r, c.g, c.b, 0.9]
        };
        let canvas_bg = {
            let c = main_area_background(theme);
            [c.r, c.g, c.b, c.a]
        };

        // Get available space from shared state (updated by shader in previous frame)
        let (avail_width, avail_height) = self.available_size();

        // Try to use tiles from shared render cache
        if let Some(cache_handle) = render_cache {
            // Read metadata without holding the lock for long
            let (content_width_u32, content_height_f32, blink_state) = {
                let shared_cache = cache_handle.read();
                (shared_cache.content_width, shared_cache.content_height, shared_cache.last_blink_state)
            };

            // Check if shared cache has usable dimensions
            if content_width_u32 > 0 && content_height_f32 > 0.0 {
                let content_width = content_width_u32 as f32;
                let content_height = content_height_f32;

                // Auto-scroll to keep viewport visible
                self.ensure_viewport_visible(viewport_info.y, viewport_info.height, content_width, content_height);

                // Calculate which tiles to select based on visible area
                let tile_height = TILE_HEIGHT as f32;
                let scroll_normalized = *self.scroll_position.borrow();

                let max_tile_idx = ((content_height / tile_height).ceil().max(1.0) as i32) - 1;

                // Convert normalized scroll position to document-space pixel Y
                let scale = (avail_width / content_width).max(0.0001);
                let scaled_content_height = content_height * scale;
                let visible_uv_height = (avail_height / scaled_content_height).min(1.0);
                let max_scroll_uv = (1.0 - visible_uv_height).max(0.0);
                let scroll_uv_y = scroll_normalized * max_scroll_uv;
                let scroll_pixel_y = scroll_uv_y * content_height;

                // Visible document height (in pixels)
                let visible_doc_height = visible_uv_height * content_height;

                // Calculate current tile based on pixel position
                let current_tile_idx = (scroll_pixel_y / tile_height).floor() as i32;

                // Dynamic slice count: visible tiles + 1 above + 1 below
                let visible_tiles = (visible_doc_height / tile_height).ceil().max(1.0) as i32;
                let mut desired_count = (visible_tiles + 2).clamp(1, MAX_TEXTURE_SLICES as i32);
                desired_count = desired_count.min(max_tile_idx + 1);
                let max_first_tile_idx = (max_tile_idx - (desired_count - 1)).max(0);
                let first_tile_idx = (current_tile_idx - 1).clamp(0, max_first_tile_idx);

                // Check if we can use cached data
                let needs_refresh = {
                    let cache_state = self.cache_state.borrow();
                    let last_first = *self.last_first_tile_idx.borrow();
                    let last_count = *self.last_tile_count.borrow();

                    cache_state.is_none()
                        || cache_state.as_ref().map_or(true, |cs| {
                            cs.incomplete || cs.content_width != content_width_u32 || (cs.content_height - content_height).abs() > 0.1
                        })
                        || last_first != first_tile_idx
                        || last_count != desired_count
                };

                let (slices, heights, total_height, first_slice_start_y) = if needs_refresh {
                    // Need to fetch tiles from cache
                    let mut slices = Vec::with_capacity(desired_count as usize);
                    let mut heights = Vec::with_capacity(desired_count as usize);
                    let mut total_height = 0u32;
                    let mut incomplete = false;
                    let first_slice_start_y = first_tile_idx as f32 * tile_height;

                    // Small placeholder (1x1) for missing tiles. Height is defined by `heights`.
                    let placeholder_slice = TextureSliceData {
                        rgba_data: Arc::new(vec![0, 0, 0, 0]),
                        width: 1,
                        height: 1,
                    };

                    // Only read from cache - don't synchronously render missing tiles
                    let cache = cache_handle.read();

                    for i in 0..desired_count {
                        let tile_idx = first_tile_idx + i;
                        if tile_idx > max_tile_idx {
                            break;
                        }

                        // Expected height in document pixels for this tile.
                        let tile_start_y = tile_idx as f32 * tile_height;
                        let tile_end_y = ((tile_idx + 1) as f32 * tile_height).min(content_height);
                        let expected_tile_height = (tile_end_y - tile_start_y).ceil().max(1.0) as u32;

                        let key = TileCacheKey::new(tile_idx, blink_state);

                        // Try exact match first, then opposite blink state
                        let tile_opt = cache.get(&key).or_else(|| cache.get(&TileCacheKey::new(tile_idx, !blink_state)));

                        if let Some(tile) = tile_opt {
                            slices.push(TextureSliceData {
                                rgba_data: Arc::clone(&tile.texture.rgba_data),
                                width: tile.texture.width,
                                height: tile.texture.height,
                            });
                            // Preserve the expected document-space height for stable mapping,
                            // even if the cached slice is slightly smaller due to clamping.
                            heights.push(expected_tile_height);
                            total_height += expected_tile_height;
                        } else {
                            // Preserve tile spacing by inserting a placeholder slice.
                            // This prevents the minimap window from collapsing when tiles
                            // are not yet rendered by the Terminal.
                            incomplete = true;
                            slices.push(placeholder_slice.clone());
                            heights.push(expected_tile_height);
                            total_height += expected_tile_height;
                        }
                    }

                    drop(cache);

                    // Update cache state
                    if !slices.is_empty() {
                        *self.cache_state.borrow_mut() = Some(MinimapCacheState {
                            cached_slices: slices.clone(),
                            cached_heights: heights.clone(),
                            cached_total_height: total_height,
                            first_tile_idx,
                            content_width: content_width_u32,
                            content_height,
                            incomplete,
                        });
                        *self.last_first_tile_idx.borrow_mut() = first_tile_idx;
                        *self.last_tile_count.borrow_mut() = desired_count;
                    }

                    (slices, heights, total_height, first_slice_start_y)
                } else {
                    // Use cached data
                    let cache_state = self.cache_state.borrow();
                    let cs = cache_state.as_ref().unwrap();
                    (
                        cs.cached_slices.clone(),
                        cs.cached_heights.clone(),
                        cs.cached_total_height,
                        cs.first_tile_idx as f32 * tile_height,
                    )
                };

                if !slices.is_empty() {
                    // Use dimensions from shared cache
                    let full_size = (content_width_u32, content_height as u32);

                    // Calculate local scroll offset for the shader.
                    // The WGSL computes:
                    //   scroll_uv_y = local_scroll_offset * (1 - visible_uv_height)
                    // We want `scroll_uv_y` to match the desired *window-UV* top position.
                    let window_h = total_height as f32;
                    let scaled_window_h = window_h * scale;
                    let window_visible_uv_h = (avail_height / scaled_window_h).min(1.0);
                    let window_max_scroll_uv = (1.0 - window_visible_uv_h).max(0.0);

                    // Desired top of the visible area within the window, in window UV.
                    let offset_in_window = (scroll_pixel_y - first_slice_start_y).max(0.0);
                    let desired_scroll_uv = if window_h > 0.0 {
                        (offset_in_window / window_h).clamp(0.0, window_max_scroll_uv)
                    } else {
                        0.0
                    };
                    let local_scroll_offset = if window_max_scroll_uv > 0.0 {
                        (desired_scroll_uv / window_max_scroll_uv).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };

                    return self.create_shader_element(
                        slices,
                        heights,
                        total_height,
                        full_size,
                        viewport_info,
                        local_scroll_offset,
                        first_slice_start_y,
                        viewport_color,
                        canvas_bg,
                    );
                }
            }
        }

        // Fallback: empty 1x1 texture (Terminal hasn't rendered yet)
        let shader_program = MinimapProgram {
            slices: vec![TextureSliceData {
                rgba_data: Arc::new(vec![0, 0, 0, 255]),
                width: 1,
                height: 1,
            }],
            slice_heights: vec![1],
            texture_width: 1,
            total_rendered_height: 1,
            instance_id: self.instance_id,
            viewport_info: ViewportInfo::default(),
            scroll_offset: 0.0,

            full_content_height: 1.0,
            first_slice_start_y: 0.0,
            shared_state: Arc::clone(&self.shared_state),
            checkerboard_colors: self.checkerboard_colors.clone(),
            viewport_color,
            canvas_bg,
        };

        shader(shader_program).width(Length::Fill).height(Length::Fill).into()
    }

    /// Helper function to create the shader element
    fn create_shader_element(
        &self,
        slices: Vec<TextureSliceData>,
        slice_heights: Vec<u32>,
        total_rendered_height: u32,
        full_content_size: (u32, u32),
        viewport_info: &ViewportInfo,
        scroll_offset: f32,
        first_slice_start_y: f32,
        viewport_color: [f32; 4],
        canvas_bg: [f32; 4],
    ) -> Element<'_, MinimapMessage> {
        // The shader renders a sliding window of texture slices.
        // `total_rendered_height` is the sum of the slice heights in pixels.
        // `full_content_height` is the total document height (for normalization).
        let shader_program = MinimapProgram {
            slices,
            slice_heights,
            texture_width: full_content_size.0,
            total_rendered_height,
            instance_id: self.instance_id,
            viewport_info: viewport_info.clone(),
            scroll_offset,

            full_content_height: full_content_size.1 as f32,
            first_slice_start_y,
            shared_state: Arc::clone(&self.shared_state),
            checkerboard_colors: self.checkerboard_colors.clone(),
            viewport_color,
            canvas_bg,
        };

        shader(shader_program).width(Length::Fill).height(Length::Fill).into()
    }
}
