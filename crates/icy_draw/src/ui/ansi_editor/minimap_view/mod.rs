mod minimap_shader;

use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use iced::widget::shader;
use iced::{Element, Length, Size, Task};
use icy_engine::Screen;
use icy_engine_gui::tile_cache::MAX_TEXTURE_SLICES;
use icy_engine_gui::{SharedCachedTile, SharedRenderCacheHandle, TILE_HEIGHT, TileCacheKey};
use parking_lot::Mutex;

use minimap_shader::MinimapProgram;
pub use minimap_shader::ViewportInfo;
pub(crate) use minimap_shader::viewport_info_from_effective_view;

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
    /// Click on minimap to scroll to position (normalized 0.0-1.0 in texture space)
    ///
    /// `pointer_x/pointer_y` are pointer coordinates relative to the minimap bounds.
    Click { norm_x: f32, norm_y: f32, pointer_x: f32, pointer_y: f32 },
    /// Drag on minimap to scroll (normalized 0.0-1.0 in texture space)
    ///
    /// `pointer_x/pointer_y` are pointer coordinates relative to the minimap bounds and may be
    /// outside (negative / beyond width/height) to support drag-out autoscroll.
    Drag { norm_x: f32, norm_y: f32, pointer_x: f32, pointer_y: f32 },
    /// Drag ended (mouse released).
    DragEnd,
    /// Scroll the minimap vertically
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
pub struct MinimapView {
    instance_id: usize,
    /// Shared state for communicating bounds from shader
    shared_state: Arc<Mutex<SharedMinimapState>>,
    /// Current scroll position (0.0 = top, 1.0 = fully scrolled down)
    scroll_position: RefCell<f32>,

    /// Last minimap bounds we saw (from shader feedback).
    last_available_size: RefCell<(f32, f32)>,
    /// Timestamp of the last bounds change (used to detect active resize).
    last_resize_at: RefCell<Option<Instant>>,
    /// Timestamp of the last time we synchronously rendered a missing tile.
    last_missing_tile_render_at: RefCell<Option<Instant>>,
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

            last_available_size: RefCell::new((0.0, 0.0)),
            last_resize_at: RefCell::new(None),
            last_missing_tile_render_at: RefCell::new(None),
        }
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
            MinimapMessage::Click { .. } => {
                // Parent handles the actual scrolling
                Task::none()
            }
            MinimapMessage::Drag { .. } => {
                // Parent handles the actual scrolling
                Task::none()
            }
            MinimapMessage::DragEnd => Task::none(),
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
    pub fn view(
        &self,
        screen: &Arc<Mutex<Box<dyn Screen>>>,
        viewport_info: &ViewportInfo,
        render_cache: Option<&SharedRenderCacheHandle>,
    ) -> Element<'_, MinimapMessage> {
        // Get available space from shared state (updated by shader in previous frame)
        let (avail_width, avail_height) = {
            let shared = self.shared_state.lock();
            (
                if shared.available_width > 0.0 { shared.available_width } else { 140.0 },
                if shared.available_height > 0.0 { shared.available_height } else { 600.0 },
            )
        };

        // During interactive window resize, avoid synchronously rendering missing tiles.
        // That path can be expensive and causes visible stutter.
        let now = Instant::now();
        let is_resizing = {
            let mut last = self.last_available_size.borrow_mut();
            let mut last_resize_at = self.last_resize_at.borrow_mut();

            if last.0 <= 0.0 || last.1 <= 0.0 {
                *last = (avail_width, avail_height);
            } else {
                let dw = (avail_width - last.0).abs();
                let dh = (avail_height - last.1).abs();
                if dw > 1.0 || dh > 1.0 {
                    *last = (avail_width, avail_height);
                    *last_resize_at = Some(now);
                }
            }

            last_resize_at.as_ref().is_some_and(|t| now.duration_since(*t) < Duration::from_millis(150))
        };

        // Try to use tiles from shared render cache
        if let Some(cache_handle) = render_cache {
            // Read metadata without holding the lock for long; we may need to lock the Screen.
            let (tile_count, content_width_u32, content_height_f32, blink_state, max_tile_idx) = {
                let shared_cache = cache_handle.read();
                (
                    shared_cache.tile_count(),
                    shared_cache.content_width,
                    shared_cache.content_height,
                    shared_cache.last_blink_state,
                    shared_cache.max_tile_index(),
                )
            };

            // Check if shared cache has usable dimensions
            if tile_count > 0 && content_width_u32 > 0 && content_height_f32 > 0.0 {
                let content_width = content_width_u32 as f32;
                let content_height = content_height_f32;

                // Auto-scroll to keep viewport visible
                self.ensure_viewport_visible(viewport_info.y, viewport_info.height, content_width, content_height);

                // Calculate which tiles to select based on visible area (like CRT shader)
                let tile_height = TILE_HEIGHT as f32;
                let scroll_normalized = *self.scroll_position.borrow(); // 0.0 - 1.0

                // Convert normalized scroll position to document-space pixel Y using the same UV math as the shader
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

                let mut slices = Vec::new();
                let mut heights = Vec::new();
                let mut total_height = 0u32;
                let first_slice_start_y = first_tile_idx as f32 * tile_height;

                let mut rendered_missing_tile = false;

                // Lock order matches Terminal rendering path: Screen -> Cache
                let screen_guard = screen.lock();
                let resolution = screen_guard.resolution();

                // Select tiles
                for i in 0..desired_count {
                    let tile_idx = first_tile_idx + i;
                    if tile_idx > max_tile_idx {
                        break;
                    }

                    let key = TileCacheKey::new(tile_idx, blink_state);
                    let cached_tile: Option<SharedCachedTile> = cache_handle.read().get(&key).cloned();

                    let tile = if let Some(tile) = cached_tile {
                        tile
                    } else {
                        // Rendering missing tiles here is expensive (CPU) and can cause stutter,
                        // especially during interactive window resize. Prefer using what's
                        // already in the shared cache.
                        if is_resizing {
                            break;
                        }

                        // Throttle: render at most one missing tile per view call, and not more
                        // often than every ~30ms.
                        if rendered_missing_tile {
                            break;
                        }
                        {
                            let mut last = self.last_missing_tile_render_at.borrow_mut();
                            let allowed = match *last {
                                None => true,
                                Some(t) => now.duration_since(t) >= Duration::from_millis(30),
                            };
                            if !allowed {
                                break;
                            }
                            *last = Some(now);
                        }
                        rendered_missing_tile = true;

                        // Render missing tile into the shared cache so Terminal and Minimap can share it.
                        let tile_start_y = tile_idx as f32 * tile_height;
                        let tile_end_y = ((tile_idx + 1) as f32 * tile_height).min(content_height);
                        let actual_tile_height = (tile_end_y - tile_start_y).max(1.0) as u32;

                        let tile_region: icy_engine::Rectangle =
                            icy_engine::Rectangle::from(0, tile_start_y as i32, resolution.width, actual_tile_height as i32);

                        let render_options = icy_engine::RenderOptions {
                            rect: icy_engine::Rectangle {
                                start: icy_engine::Position::new(0, tile_start_y as i32),
                                size: icy_engine::Size::new(resolution.width, actual_tile_height as i32),
                            }
                            .into(),
                            blink_on: blink_state,
                            selection: None,
                            selection_fg: None,
                            selection_bg: None,
                            override_scan_lines: None,
                        };

                        let (render_size, rgba_data) = screen_guard.render_region_to_rgba(tile_region, &render_options);
                        let width = render_size.width as u32;
                        let height = render_size.height as u32;

                        let slice = icy_engine_gui::TextureSliceData {
                            rgba_data: Arc::new(rgba_data),
                            width,
                            height,
                        };

                        let cached_tile = SharedCachedTile {
                            texture: slice,
                            height,
                            start_y: tile_start_y,
                        };

                        cache_handle.write().insert(key, cached_tile.clone());
                        cached_tile
                    };

                    // Use Arc::clone for cheap reference counting instead of data clone
                    slices.push(TextureSliceData {
                        rgba_data: Arc::clone(&tile.texture.rgba_data),
                        width: tile.texture.width,
                        height: tile.texture.height,
                    });
                    heights.push(tile.height);
                    total_height += tile.height;
                }

                drop(screen_guard);

                // Use dimensions from shared cache - they match what Terminal rendered
                let full_size = (content_width_u32, content_height as u32);

                if !slices.is_empty() {
                    return self.create_shader_element(
                        slices,
                        heights,
                        total_height,
                        full_size,
                        viewport_info,
                        avail_height,
                        scroll_normalized,
                        first_slice_start_y,
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
            available_height: avail_height,
            full_content_height: 1.0,
            first_slice_start_y: 0.0,
            shared_state: Arc::clone(&self.shared_state),
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
        available_height: f32,
        scroll_offset: f32,
        first_slice_start_y: f32,
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
            available_height,
            full_content_height: full_content_size.1 as f32,
            first_slice_start_y,
            shared_state: Arc::clone(&self.shared_state),
        };

        shader(shader_program).width(Length::Fill).height(Length::Fill).into()
    }

    /// Handle mouse press for click-to-navigate functionality
    /// Returns the normalized position (0.0-1.0) in full buffer space
    /// This position represents where the CENTER of the viewport should be
    pub fn handle_click(&self, _bounds: Size, position: iced::Point, render_cache: Option<&SharedRenderCacheHandle>) -> Option<(f32, f32)> {
        // Get tile info from shared cache
        let (render_width, total_rendered_height, full_w, full_h) = if let Some(cache_handle) = render_cache {
            let shared_cache = cache_handle.read();
            if shared_cache.tile_count() == 0 || shared_cache.content_width == 0 {
                return None;
            }
            (
                shared_cache.content_width,
                shared_cache.content_height as u32,
                shared_cache.content_width,
                shared_cache.content_height as u32,
            )
        } else {
            return None;
        };

        if render_width == 0 || total_rendered_height == 0 || full_h == 0 || full_w == 0 {
            return None;
        }

        let shared = self.shared_state.lock();
        let avail_width = shared.available_width;
        let avail_height = shared.available_height;
        drop(shared);

        if avail_width <= 0.0 || avail_height <= 0.0 {
            return None;
        }

        // Scale factor (minimap fills available width)
        let scale = avail_width / full_w as f32;
        let scaled_content_height = total_rendered_height as f32 * scale;

        // Calculate visible UV range (same logic as in shader prepare())
        let visible_uv_height = (avail_height / scaled_content_height).min(1.0);
        let max_scroll_uv = (1.0 - visible_uv_height).max(0.0);
        let scroll_uv = *self.scroll_position.borrow() * max_scroll_uv;

        // Pointer can be outside when drag-out autoscroll is active; clamp to edge.
        let local_x = position.x.clamp(0.0, avail_width);
        let local_y = position.y.clamp(0.0, avail_height);

        // Convert screen position to texture UV
        // screen_uv_y is 0-1 in the visible area
        let screen_uv_y = local_y / avail_height;
        // texture_uv_y is the absolute position in the rendered texture (0-1)
        let texture_uv_y = scroll_uv + screen_uv_y * visible_uv_height;

        // Convert texture UV to full buffer coordinates
        let render_ratio = total_rendered_height as f32 / full_h as f32;
        let buffer_y = texture_uv_y / render_ratio;

        let norm_x = local_x / avail_width;
        let norm_y = buffer_y.clamp(0.0, 1.0);

        Some((norm_x, norm_y))
    }
}
