//! CRT Shader Program with sliding window rendering
//!
//! This module implements the shader program for terminal rendering using
//! a sliding window of texture slices that cover the visible area plus
//! one tile above and below for smooth scrolling.

use crate::{
    compute_viewport_auto, compute_viewport_manual, get_scale_factor, is_alt_pressed, is_ctrl_pressed, is_shift_pressed,
    shared_render_cache::{SharedCachedTile, TileCacheKey, TILE_HEIGHT},
    tile_cache::MAX_TEXTURE_SLICES,
    CRTShaderState, MonitorSettings, Terminal, TerminalMessage, TerminalMouseEvent, TerminalShader, TextureSliceData,
};
use iced::widget::shader;
use iced::{mouse, window, Rectangle};
use icy_engine::{CaretShape, EditableScreen, KeyModifiers, MouseButton};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Global render generation counter - incremented each time tiles are re-rendered
/// Used to detect content changes in the shader instead of Arc pointer hashing
static RENDER_GENERATION: AtomicU64 = AtomicU64::new(0);

/// Clamps the terminal height to fit within the viewport bounds.
///
/// This function sets the terminal window height (via `TerminalState`) to
/// the minimum of the viewport capacity and the document height:
/// - If the document is smaller than the viewport → keep document height (enables centering)
/// - If the document is larger than the viewport → shrink to viewport height (use full screen)
///
/// This does NOT resize the underlying buffer/scrollback, only the visible terminal window.
///
/// # Arguments
/// * `editable` - The editable screen to modify
/// * `bounds_height` - The widget bounds height in logical pixels
/// * `scan_lines` - Whether scanlines are enabled (doubles effective cell height)
/// * `scale_factor` - The display scale factor (e.g., 2.0 for HiDPI)
/// * `zoom` - The zoom level (e.g., 0.5 for 50%, 1.0 for 100%, 2.0 for 200%)
///
/// # Returns
/// `true` if the terminal height was changed, `false` otherwise.
pub fn clamp_terminal_height_to_viewport(editable: &mut dyn EditableScreen, bounds_height: f32, scan_lines: bool, scale_factor: f32, zoom: f32) -> bool {
    let scale_factor = scale_factor.max(0.001);
    let zoom = zoom.max(0.001);
    // At lower zoom levels, more rows fit in the viewport
    // At 50% zoom, each row takes half the pixels, so double the rows fit
    let avail_h_px = bounds_height.max(1.0) * scale_factor / zoom;

    let font_h = editable.font_dimensions().height as f32;
    let scan_mult = if scan_lines { 2.0 } else { 1.0 };
    let cell_h_px = (font_h * scan_mult).max(1.0);

    // Get the actual buffer height (document content height in rows).
    let buffer_height = editable.height();

    // Compute desired terminal height: how many rows can fit in the viewport?
    let viewport_rows = (avail_h_px / cell_h_px).floor().max(1.0) as i32;

    let desired_rows = viewport_rows.min(buffer_height);
    if desired_rows != editable.terminal_state().height() {
        editable.terminal_state_mut().set_height(desired_rows);
        true
    } else {
        false
    }
}

/// Program wrapper that renders the terminal using sliding window tile approach
pub struct CRTShaderProgram<'a> {
    pub term: &'a Terminal,
    pub monitor_settings: Arc<MonitorSettings>,
    /// Editor markers passed from caller (layer bounds, selection, etc.)
    /// If None, markers are not rendered.
    pub editor_markers: Option<crate::EditorMarkers>,
}

impl<'a> CRTShaderProgram<'a> {
    pub fn new(term: &'a Terminal, monitor_settings: Arc<MonitorSettings>, editor_markers: Option<crate::EditorMarkers>) -> Self {
        Self {
            term,
            monitor_settings,
            editor_markers,
        }
    }

    /// Helper function to get current keyboard modifier state
    fn get_modifiers() -> KeyModifiers {
        KeyModifiers {
            shift: is_shift_pressed(),
            ctrl: is_ctrl_pressed(),
            alt: is_alt_pressed(),
            meta: false,
        }
    }

    fn internal_draw(&self, state: &CRTShaderState, _cursor: mouse::Cursor, bounds: Rectangle) -> TerminalShader {
        let font_w;
        let font_h;
        let scan_lines;
        let scroll_offset_y: f32;
        let scroll_offset_x: f32;
        let visible_height: f32;
        let visible_width: f32;
        let full_content_height: f32;
        let aspect_ratio_y: f32;
        let texture_width: u32;
        let blink_on: bool;
        let char_blink_supported: bool;
        let zoom: f32;

        let mut slices_blink_off: Vec<TextureSliceData> = Vec::new();
        let mut slices_blink_on: Vec<TextureSliceData> = Vec::new();
        let mut slice_heights: Vec<u32> = Vec::new();
        #[allow(unused_assignments)]
        let mut first_slice_start_y: f32 = 0.0;
        // Track if any tiles were re-rendered this frame
        let mut tiles_rendered = false;

        // Track the current sliding-window selection so we can invalidate GPU resources
        // when the window moves, even if all tiles were served from cache.
        #[allow(unused_assignments)]
        let mut window_first_tile_idx: i32 = 0;
        #[allow(unused_assignments)]
        let mut window_slice_count: i32 = 0;

        // Caret rendering data (computed from screen, rendered in shader)
        let mut caret_pos: [f32; 2] = [0.0, 0.0];
        let mut caret_size: [f32; 2] = [0.0, 0.0];
        let mut caret_visible: bool = false;
        let mut caret_mode: u8 = 0;

        // Layer bounds rendering data (computed from screen, rendered in shader)
        let layer_rect: Option<[f32; 4]>;
        // Caret origin offset from editor_markers (layer offset in pixels)
        let caret_origin_px: (f32, f32) = self.editor_markers.as_ref().map(|m| m.caret_origin_px).unwrap_or((0.0, 0.0));

        {
            let mut screen = self.term.screen.lock();
            scan_lines = screen.scan_lines();

            // IMPORTANT: `screen.font_dimensions()` already includes aspect ratio correction
            // (and may involve rounding to integer pixels). For the shader sampling path,
            // we need the *effective* ratio between display pixel space and the raw
            // rendered texture pixel space.
            let font_dims = screen.font_dimensions();
            font_w = font_dims.width as usize;
            font_h = font_dims.height as usize;

            // Raw render uses the font bitmap height (no aspect ratio correction).
            // Use the actual ratio (including rounding) so textures and overlays stay aligned.
            let raw_font_h = screen.font(0).map(|f| f.size().height as f32).unwrap_or(font_h as f32).max(1.0);

            let display_font_h = font_h as f32;

            aspect_ratio_y = if screen.use_aspect_ratio() {
                (display_font_h / raw_font_h).max(1.0)
            } else {
                1.0
            };

            // Get the ORIGINAL document resolution BEFORE fit_terminal_height_to_bounds modifies it.
            // This is needed for proper centering calculation.
            let original_resolution = screen.resolution();
            let original_res_h = original_resolution.height as f32;
            let original_res_w = original_resolution.width as f32;

            // Pre-compute the zoom level (needed for clamp_terminal_height_to_viewport)
            let pre_zoom = if self.monitor_settings.scaling_mode.is_auto() {
                1.0
            } else {
                self.monitor_settings
                    .scaling_mode
                    .compute_zoom(
                        original_res_w,
                        original_res_h,
                        bounds.width,
                        bounds.height,
                        self.monitor_settings.use_integer_scaling,
                    )
                    .max(0.001)
            };

            // Optional: Clamp the terminal window height to fit within bounds.
            // For small documents, this preserves their height (enabling centering).
            // For large documents, this shrinks to viewport (using full screen).
            if self.term.fit_terminal_height_to_bounds && !self.monitor_settings.scaling_mode.is_auto() {
                if let Some(editable) = screen.as_editable() {
                    clamp_terminal_height_to_viewport(editable, bounds.height, scan_lines, get_scale_factor(), pre_zoom);
                }
            }

            state.update_cached_screen_info(&**screen);
            *state.cached_mouse_state.lock() = Some(screen.terminal_state().mouse_state.clone());

            char_blink_supported = screen.ice_mode().has_blink();
            blink_on = if char_blink_supported { state.character_blink.is_on() } else { false };

            // Snapshot viewport inputs (keep lock scopes small and borrow-check friendly).
            let (
                content_width,
                content_height,
                requested_scroll_x,
                requested_scroll_y,
                vp_visible_w,
                vp_visible_h,
                vp_zoom_before,
                vp_sb_x,
                vp_sb_y,
                viewport_changed,
            ) = {
                let vp = self.term.viewport.read();
                (
                    vp.content_width,
                    vp.content_height,
                    vp.scroll_x,
                    vp.scroll_y,
                    vp.visible_width,
                    vp.visible_height,
                    vp.zoom,
                    vp.scrollbar.scroll_position_x,
                    vp.scrollbar.scroll_position,
                    vp.changed.load(std::sync::atomic::Ordering::Acquire),
                )
            };

            // The visible region must maintain the content's aspect ratio.
            // We use resolution() for the visible aspect ratio (terminal size × font),
            // not the full content_height which includes scrollback.
            // NOTE: We get resolution AFTER fit_terminal_height_to_bounds may have modified it,
            // but we use original_res_h (saved before) for centering in Manual zoom mode.
            let resolution = screen.resolution();
            let res_w = resolution.width as f32;
            let res_h = resolution.height as f32;

            // CRITICAL: visible_width/height define what portion of the document is shown.
            // The shader maps UV 0-1 over visible_width/height, so these MUST match
            // the actual content dimensions to avoid stretching.
            //
            // For Auto scaling: show the entire terminal (resolution), centered in widget
            // For Manual scaling: show a portion based on zoom level
            if self.monitor_settings.scaling_mode.is_auto() {
                // Auto mode: entire resolution is visible, shader will center it
                let params = compute_viewport_auto(res_w, res_h, content_width, content_height, requested_scroll_x, requested_scroll_y);
                visible_width = params.visible_width;
                visible_height = params.visible_height;
                scroll_offset_y = params.scroll_offset_y;
                scroll_offset_x = params.scroll_offset_x;

                zoom = params.zoom;
            } else {
                // Manual zoom: calculate visible portion based on zoom
                // IMPORTANT: Use original_res_h for centering calculation, not the inflated res_h
                // from fit_terminal_height_to_bounds. This ensures small documents are centered
                // properly in the viewport instead of "sticking to the top".
                let params = compute_viewport_manual(
                    res_w,
                    original_res_h,
                    bounds.width,
                    bounds.height,
                    content_width,
                    content_height,
                    requested_scroll_x,
                    requested_scroll_y,
                    &self.monitor_settings.scaling_mode,
                    self.monitor_settings.use_integer_scaling,
                );

                zoom = params.zoom;
                visible_width = params.visible_width;
                visible_height = params.visible_height;

                // Debug output for centering issue
                if cfg!(debug_assertions) && std::env::var("ICY_DEBUG_VISIBLE").is_ok() {
                    eprintln!(
                        "[crt_shader] Manual zoom: res=({:.1},{:.1}) original_h={:.1} bounds=({:.1},{:.1}) zoom={:.2} => visible=({:.1},{:.1})",
                        res_w, res_h, original_res_h, bounds.width, bounds.height, zoom, visible_width, visible_height
                    );
                }

                scroll_offset_y = params.scroll_offset_y;
                scroll_offset_x = params.scroll_offset_x;
            }

            if cfg!(debug_assertions) && std::env::var("ICY_DEBUG_VIEWPORT").is_ok() && viewport_changed {
                let max_scroll_x = (content_width - visible_width).max(0.0);
                let max_scroll_y = (content_height - visible_height).max(0.0);
                let ratio_x = if max_scroll_x > 0.0 {
                    (scroll_offset_x / max_scroll_x).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let ratio_y = if max_scroll_y > 0.0 {
                    (scroll_offset_y / max_scroll_y).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                eprintln!(
                    "[viewport] bounds=({:.1},{:.1}) res=({:.1},{:.1}) orig_res_h={:.1} vp_visible=({:.1},{:.1}) vp_zoom_before={:.3} -> zoom_eff={:.3} vis=({:.1},{:.1}) content=({:.1},{:.1}) scroll_req=({:.1},{:.1}) scroll_px=({:.1},{:.1}) max_scroll=({:.1},{:.1}) ratio=({:.3},{:.3}) vp_sb_before=({:.3},{:.3}) mode={:?} int_scale={} ",
                    bounds.width,
                    bounds.height,
                    res_w,
                    res_h,
                    original_res_h,
                    vp_visible_w,
                    vp_visible_h,
                    vp_zoom_before,
                    zoom,
                    visible_width,
                    visible_height,
                    content_width,
                    content_height,
                    requested_scroll_x,
                    requested_scroll_y,
                    scroll_offset_x,
                    scroll_offset_y,
                    max_scroll_x,
                    max_scroll_y,
                    ratio_x,
                    ratio_y,
                    vp_sb_x,
                    vp_sb_y,
                    self.monitor_settings.scaling_mode,
                    self.monitor_settings.use_integer_scaling
                );
            }

            full_content_height = content_height;
            texture_width = resolution.width as u32;

            // Keep Viewport zoom in sync with effective zoom used for rendering.
            // This fixes scrollbar/minimap sizing at high zoom levels (e.g. 400%).
            {
                let mut vp = self.term.viewport.write();
                // The Viewport visible size must be the widget bounds (screen pixels).
                // `visible_content_*()` derives from this via division by `zoom`.
                vp.visible_width = bounds.width.max(1.0);
                vp.visible_height = bounds.height.max(1.0);

                vp.zoom = zoom;

                // Publish the same clamped scroll offsets that the shader uses.
                // This keeps minimap and scrollbars in sync even if other code directly
                // mutates vp.scroll_x/y without calling the helpers.
                vp.scroll_x = scroll_offset_x;
                vp.scroll_y = scroll_offset_y;

                // Clamp targets using the new visible size/zoom and sync thumb positions.
                vp.clamp_scroll();
                if viewport_changed {
                    vp.changed.store(false, std::sync::atomic::Ordering::Relaxed);
                }
            }

            // Check for content changes that require cache invalidation
            // Use the shared render cache from Terminal
            {
                let mut cache: parking_lot::lock_api::RwLockWriteGuard<'_, parking_lot::RawRwLock, crate::SharedRenderCache> = self.term.render_cache.write();

                // Selective tile invalidation based on dirty lines
                if let Some((first_dirty_line, last_dirty_line)) = screen.get_dirty_lines() {
                    // Calculate tile indices from dirty line range
                    let tile_height = crate::TILE_HEIGHT;
                    let font_height = screen.font_dimensions().height.max(1) as u32;
                    let tile_height_lines = tile_height / font_height;
                    if tile_height_lines > 0 {
                        let first_tile = (first_dirty_line as u32 / tile_height_lines) as i32;
                        let last_tile = ((last_dirty_line as u32).saturating_sub(1) / tile_height_lines) as i32;
                        // Selective invalidation: only remove tiles in dirty range
                        cache.invalidate_tiles(first_tile, last_tile);
                    } else {
                        cache.invalidate();
                    }
                    // Clear dirty range after processing
                    screen.clear_dirty_lines();
                }

                cache.content_height = full_content_height;
                cache.content_width = texture_width;
                cache.last_blink_state = blink_on;

                // Expose the *effective* visible region used by the shader so other widgets
                // (e.g. minimap) can match the terminal view exactly.
                cache.visible_width = visible_width;
                cache.visible_height = visible_height;
                cache.scroll_offset_x = scroll_offset_x;
                cache.scroll_offset_y = scroll_offset_y;

                // Selection is now rendered in the shader, so we don't need to invalidate
                // the cache when selection changes. This significantly improves performance.
                let info: parking_lot::lock_api::MutexGuard<'_, parking_lot::RawMutex, crate::CachedScreenInfo> = state.cached_screen_info.lock();
                let _ = info; // bounds size tracking removed along with version tracking
            }

            // TODO: Compute layer bounds from screen directly in shader
            // Currently blocked by trait method resolution issues with dyn Screen.
            // For now, layer bounds are still passed via EditorMarkers.
            // See: https://github.com/rust-lang/rust/issues/...
            /*
            // Compute layer bounds from screen (not from markers overlay)
            // This ensures layer bounds are always in sync with the buffer state
            {
                let font_width = font_w as f32;
                let font_height = font_h as f32;
                // Use explicit dereference to access Screen trait methods through MutexGuard<Box<dyn Screen>>
                let screen_ref: &dyn Screen = &**screen;
                let current_layer_idx = screen_ref.get_current_layer();

                // Check for floating paste layer first (using object-safe method)
                let mut target_layer_idx = current_layer_idx;
                for i in 0..screen_ref.layer_count() {
                    if screen_ref.is_layer_paste(i) {
                        target_layer_idx = i;
                        break;
                    }
                }

                // Get layer bounds using object-safe method
                if let Some((offset, size)) = screen_ref.get_layer_bounds(target_layer_idx) {
                    let x = offset.x as f32 * font_width;
                    let y = offset.y as f32 * font_height;
                    let w = size.width as f32 * font_width;
                    let h = size.height as f32 * font_height;

                    layer_rect = Some([x, y, x + w, y + h]);
                }

                // Caret origin is relative to current layer (not paste layer)
                if let Some((offset, _size)) = screen_ref.get_layer_bounds(current_layer_idx) {
                    caret_origin_px = (offset.x as f32 * font_width, offset.y as f32 * font_height);
                }
            }
            */

            // Compute caret position for shader rendering
            // This must happen AFTER cache invalidation to ensure caret state matches buffer state
            {
                let caret = screen.caret();
                let should_draw = caret.visible && (!caret.blinking || state.caret_blink.is_on()) && self.term.has_focus;

                // Caret origin offset in *document pixels* (used to anchor caret to current layer)
                let (caret_origin_x, caret_origin_y) = caret_origin_px;

                if should_draw && font_w > 0 && font_h > 0 {
                    let caret_cell_pos = caret.position();
                    let scan_mult = if scan_lines { 2.0 } else { 1.0 };

                    // Vertical document pixel space is scaled by scan_mult in the caret path,
                    // so we must match that scaling for the origin as well.
                    let origin_x = caret_origin_x;
                    let origin_y = caret_origin_y * scan_mult;

                    // Convert cell position to pixel position (viewport-relative)
                    // IMPORTANT: Use f32 for scroll offsets to avoid truncation errors
                    // that cause the caret to drift when scrolled with fractional offsets.
                    let (px_x, px_y) = if caret.use_pixel_positioning {
                        (
                            caret_cell_pos.x as f32 + origin_x - scroll_offset_x,
                            caret_cell_pos.y as f32 * scan_mult + origin_y - scroll_offset_y,
                        )
                    } else {
                        (
                            caret_cell_pos.x as f32 * font_w as f32 + origin_x - scroll_offset_x,
                            caret_cell_pos.y as f32 * font_h as f32 * scan_mult + origin_y - scroll_offset_y,
                        )
                    };

                    let actual_font_h = font_h as f32 * scan_mult;

                    // Only draw if caret is in visible area
                    // Convert to normalized UV coordinates (0-1) so it works with any zoom level
                    // IMPORTANT: Normalize by visible_width/height, NOT texture_width!
                    // The shader UV space maps 0-1 over the visible area, not the full texture.
                    let vis_w = visible_width;
                    let vis_h = visible_height;

                    if px_x >= 0.0 && px_y >= 0.0 && px_x < vis_w && px_y < vis_h {
                        // Normalize to 0-1 UV coordinates
                        caret_pos = [px_x / vis_w, px_y / vis_h];
                        caret_size = [font_w as f32 / vis_w, actual_font_h / vis_h];
                        caret_visible = true;
                        caret_mode = match caret.shape {
                            CaretShape::Bar => 0,
                            CaretShape::Block => 1,
                            CaretShape::Underline => 2,
                        };
                    }
                }
            }

            // Calculate which tiles we need based on scroll position
            // Each tile is TILE_HEIGHT pixels tall
            let tile_height = TILE_HEIGHT as f32;

            // Tile slicing/rendering happens in RAW texture pixel coordinates.
            // Viewport/scroll inputs are in aspect-corrected document pixels.
            let scroll_offset_y_raw = scroll_offset_y / aspect_ratio_y;
            let visible_height_raw = visible_height / aspect_ratio_y;
            let full_content_height_raw = full_content_height / aspect_ratio_y;

            // Current tile index based on scroll position
            let current_tile_idx = (scroll_offset_y_raw / tile_height).floor() as i32;
            let max_tile_idx = ((full_content_height_raw / tile_height).ceil() as i32 - 1).max(0);

            // Dynamic slice count: visible tiles + 1 above + 1 below
            let visible_tiles = (visible_height_raw / tile_height).ceil().max(1.0) as i32;
            let mut desired_count = (visible_tiles + 2).clamp(1, MAX_TEXTURE_SLICES as i32);
            desired_count = desired_count.min(max_tile_idx + 1);

            // Start one tile above current, but clamp so we can still fit desired_count tiles
            let max_first_tile_idx = (max_tile_idx - (desired_count - 1)).max(0);
            let first_tile_idx = (current_tile_idx - 1).clamp(0, max_first_tile_idx);

            window_first_tile_idx = first_tile_idx;
            window_slice_count = desired_count;

            // Calculate tile indices to render
            let mut tile_indices: Vec<i32> = Vec::with_capacity(desired_count as usize);
            for i in 0..desired_count {
                let idx = first_tile_idx + i;
                if idx <= max_tile_idx {
                    tile_indices.push(idx);
                }
            }

            first_slice_start_y = first_tile_idx as f32 * tile_height;

            // Get or render each tile using the shared cache for BOTH blink states
            let resolution = screen.resolution();

            // For icy_term: get selection from screen and render it in the RGBA data
            // (icy_draw uses editor_markers for selection, so this only applies when editor_markers is None)
            let (selection, selection_fg, selection_bg) = if self.editor_markers.is_none() {
                let sel = screen.selection();
                if sel.is_some() {
                    let (fg_sel, bg_sel) = screen.buffer_type().selection_colors();
                    (sel, Some(fg_sel), Some(bg_sel))
                } else {
                    (None, None, None)
                }
            } else {
                // icy_draw handles selection via shader/editor_markers
                (None, None, None)
            };
            let has_selection = selection.is_some();

            // Helper to get or render tiles for a specific blink state
            let mut get_or_render_tiles = |blink_state: bool, slices: &mut Vec<TextureSliceData>, heights: &mut Vec<u32>| {
                for &tile_idx in &tile_indices {
                    let tile_start_y = tile_idx as f32 * tile_height;
                    let tile_end_y = ((tile_idx + 1) as f32 * tile_height).min(full_content_height_raw);
                    let actual_tile_height = (tile_end_y - tile_start_y).ceil().max(1.0) as u32;

                    let cache_key = TileCacheKey::new(tile_idx, blink_state);
                    // Don't use cache when selection is active (selection changes frequently)
                    let cached_tile = if has_selection {
                        None
                    } else {
                        self.term.render_cache.read().get(&cache_key).cloned()
                    };

                    if let Some(cached) = cached_tile {
                        slices.push(cached.texture);
                        if heights.len() < tile_indices.len() {
                            heights.push(cached.height);
                        }
                    } else {
                        tiles_rendered = true;
                        // Render this tile
                        let tile_region: icy_engine::Rectangle =
                            icy_engine::Rectangle::from(0, tile_start_y as i32, resolution.width, actual_tile_height as i32);

                        // Include selection in render options (for icy_term)
                        let render_options = icy_engine::RenderOptions {
                            rect: icy_engine::Rectangle {
                                start: icy_engine::Position::new(0, tile_start_y as i32),
                                size: icy_engine::Size::new(resolution.width, actual_tile_height as i32),
                            }
                            .into(),
                            blink_on: blink_state,
                            selection,
                            selection_fg: selection_fg.clone(),
                            selection_bg: selection_bg.clone(),
                            override_scan_lines: None,
                        };
                        let (render_size, rgba_data) = screen.render_region_to_rgba_raw(tile_region, &render_options);
                        let width = render_size.width as u32;
                        let height = render_size.height as u32;

                        let slice = TextureSliceData {
                            rgba_data: Arc::new(rgba_data),
                            width,
                            height,
                        };

                        // Only cache if no selection is active
                        if !has_selection {
                            let cached_tile = SharedCachedTile {
                                texture: slice.clone(),
                                height,
                                start_y: tile_start_y,
                            };
                            self.term.render_cache.write().insert(cache_key, cached_tile);
                        }

                        slices.push(slice);
                        if heights.len() < tile_indices.len() {
                            heights.push(height);
                        }
                    }
                }
            };

            // Build blink-off tiles always. Only build blink-on tiles when character blinking is meaningful.
            get_or_render_tiles(false, &mut slices_blink_off, &mut slice_heights);
            if char_blink_supported {
                get_or_render_tiles(true, &mut slices_blink_on, &mut slice_heights);
            } else {
                slices_blink_on = slices_blink_off.clone();
            }
        }

        // Ensure we have at least one slice for both states
        if slices_blink_off.is_empty() {
            let empty_slice = TextureSliceData {
                rgba_data: Arc::new(vec![0u8; 4]),
                width: 1,
                height: 1,
            };
            slices_blink_off.push(empty_slice.clone());
            slices_blink_on.push(empty_slice);
            slice_heights.push(1);
            first_slice_start_y = 0.0;
        }

        // Read marker settings from editor_markers parameter (passed by caller)
        // This replaces the old approach of reading from term.markers
        let markers = self.editor_markers.as_ref();

        // Raster and guide are stored in pixel coordinates (already converted by the editor)
        let raster_spacing = markers.and_then(|m| m.raster);
        let guide_pos = markers.and_then(|m| m.guide);

        // Get marker colors from marker_settings if available
        let (raster_color, raster_alpha, guide_color, guide_alpha) = if let Some(settings) = markers.and_then(|m| m.marker_settings.as_ref()) {
            let (rr, rg, rb) = settings.raster_color.rgb();
            let (gr, gg, gb) = settings.guide_color.rgb();
            (
                [rr as f32 / 255.0, rg as f32 / 255.0, rb as f32 / 255.0, 1.0],
                settings.raster_alpha,
                [gr as f32 / 255.0, gg as f32 / 255.0, gb as f32 / 255.0, 1.0],
                settings.guide_alpha,
            )
        } else {
            // Default colors: white raster, cyan guide
            ([1.0, 1.0, 1.0, 1.0], 0.5, [0.0, 1.0, 1.0, 1.0], 0.8)
        };

        // Load reference image data from markers
        let (reference_image_data, reference_image_enabled, reference_image_alpha, reference_image_mode, reference_image_offset, reference_image_scale) =
            if let Some(ref ref_img) = markers.and_then(|m| m.reference_image.as_ref()) {
                if ref_img.visible && !ref_img.path.as_os_str().is_empty() {
                    // Use cached image data (caller should have loaded it)
                    if let Some((data, w, h)) = ref_img.get_cached() {
                        (
                            Some((data.clone(), *w, *h)),
                            true,
                            ref_img.alpha,
                            ref_img.mode as u8,
                            [ref_img.offset.0, ref_img.offset.1],
                            ref_img.scale,
                        )
                    } else {
                        (None, false, 0.5, 0, [0.0, 0.0], 1.0)
                    }
                } else {
                    (None, false, 0.5, 0, [0.0, 0.0], 1.0)
                }
            } else {
                (None, false, 0.5, 0, [0.0, 0.0], 1.0)
            };

        // Get layer display settings from markers
        let show_layer_bounds = markers.map_or(false, |m| m.show_layer_bounds);
        let paste_mode = markers.map_or(false, |m| m.paste_mode);
        // Get layer_rect from markers (set by caller before view)
        layer_rect = markers.and_then(|m| m.layer_bounds).map(|(x, y, w, h)| [x, y, x + w, y + h]);
        let selection_rect = markers.and_then(|m| m.selection_rect).map(|(x, y, w, h)| [x, y, x + w, y + h]);
        let selection_color = markers.map_or(crate::selection_colors::DEFAULT, |m| m.selection_color);
        let selection_mask_data = markers.and_then(|m| m.selection_mask_data.clone());
        let tool_overlay_mask_data = markers.and_then(|m| m.tool_overlay_mask_data.clone());
        let tool_overlay_rect = markers.and_then(|m| m.tool_overlay_rect).map(|(x, y, w, h)| [x, y, x + w, y + h]);
        let tool_overlay_cell_height_scale = markers.map_or(1.0, |m| m.tool_overlay_cell_height_scale);
        let brush_preview_rect = markers.and_then(|m| m.brush_preview_rect).map(|(x, y, w, h)| [x, y, x + w, y + h]);

        // Calculate render generation.
        // IMPORTANT: We must refresh GPU texture arrays not only when tiles were re-rendered,
        // but also when the *window selection* changes (first tile index / slice count).
        // Otherwise, scrolling across tile boundaries can reuse stale texture-array contents
        // (all tiles served from cache => no generation bump), causing visible jumps and
        // desync between minimap and terminal.
        let base_generation = if tiles_rendered {
            RENDER_GENERATION.fetch_add(1, Ordering::Relaxed) + 1
        } else {
            RENDER_GENERATION.load(Ordering::Relaxed)
        };
        let window_key = ((window_first_tile_idx as u32 as u64) << 32) ^ (window_slice_count as u32 as u64);
        let render_generation = base_generation ^ window_key;

        TerminalShader {
            slices_blink_off,
            slices_blink_on,
            slice_heights,
            texture_width,
            total_content_height: full_content_height,
            monitor_settings: self.monitor_settings.clone(),
            instance_id: state.instance_id,
            render_generation,
            zoom,
            render_info: self.term.render_info.clone(),
            font_width: font_w as f32,
            font_height: font_h as f32,
            scan_lines,
            background_color: *self.term.background_color.read(),
            scroll_offset_y,
            visible_height,
            aspect_ratio_y,
            first_slice_start_y,
            scroll_offset_x,
            visible_width,
            // Caret rendering in shader
            caret_pos,
            caret_size,
            caret_visible,
            caret_mode,
            blink_on,
            // Marker rendering in shader
            raster_spacing,
            raster_color,
            raster_alpha,
            guide_pos,
            guide_color,
            guide_alpha,
            // Reference image rendering
            reference_image_data,
            reference_image_enabled,
            reference_image_alpha,
            reference_image_mode,
            reference_image_offset,
            reference_image_scale,
            // Layer bounds rendering
            layer_rect,
            // Yellow for normal, Cyan for paste mode
            layer_color: if paste_mode { [0.0, 1.0, 1.0, 1.0] } else { [1.0, 1.0, 0.0, 1.0] },
            show_layer_bounds,
            paste_mode,
            // Selection rendering
            selection_rect,
            selection_color,
            selection_mask_data,

            // Tool overlay (Moebius-style alpha preview)
            tool_overlay_mask_data,
            tool_overlay_rect,
            tool_overlay_cell_height_scale,

            // Brush/Pencil preview rendering
            brush_preview_rect,
        }
    }

    /// Simplified internal_update that only handles coordinate mapping and event emission.
    pub fn internal_update(
        &self,
        state: &mut CRTShaderState,
        event: &iced::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<iced::widget::Action<TerminalMessage>> {
        let now = crate::Blink::now_ms();

        // Gate blink work to cases where it is actually visible/relevant.
        // This avoids a perpetual redraw loop when nothing is blinking.
        let mut char_blink_supported = true;
        let mut caret_blink_requested = false;

        if let Some(screen) = self.term.screen.try_lock() {
            let buffer_type = screen.buffer_type();
            state.caret_blink.set_rate(buffer_type.caret_blink_rate() as u128);
            state.character_blink.set_rate(buffer_type.blink_rate() as u128);

            // In IceMode::Ice, the blink attribute is repurposed for high background colors.
            char_blink_supported = screen.ice_mode().has_blink();

            let caret = screen.caret();
            caret_blink_requested = caret.visible && caret.blinking && self.term.has_focus;
        }

        if caret_blink_requested {
            state.caret_blink.update(now);
        }
        if char_blink_supported {
            state.character_blink.update(now);
        }

        let is_over = cursor.is_over(bounds);

        // Handle animation: on each redraw, check if we need more animation frames
        if let iced::Event::Window(window::Event::RedrawRequested(_instant)) = event {
            // Check if we need animation for:
            // 1. Caret blink
            // 2. Character blink
            // 3. Selection marching ants (always animate if selection is active)
            // 4. Layer bounds marching ants (when layer overlaps with selection)
            let needs_caret_blink = caret_blink_requested && state.caret_blink.is_due(now);
            let needs_char_blink = char_blink_supported && state.character_blink.is_due(now);

            // Check if there's an active selection or layer bounds that need marching ants animation
            let (has_selection, layer_border_animated) = {
                let markers = self.editor_markers.as_ref();
                let sel = markers.map_or(false, |m| m.selection_rect.is_some() || m.selection_mask_data.is_some());
                let animated = markers.map_or(false, |m| m.layer_border_animated);
                (sel, animated)
            };

            // Animation needed when:
            // - Selection is active (marching ants on selection border)
            // - Layer border is animated (paste mode or explicit flag)
            let needs_marching_ants = has_selection || layer_border_animated;

            // Calculate next redraw time
            let next_blink_time = if needs_caret_blink || needs_char_blink {
                // If blink is due, request immediate redraw
                Some(Duration::from_millis(16))
            } else {
                // Calculate time until next relevant blink
                let caret_remaining = if caret_blink_requested {
                    state.caret_blink.time_until_next(now)
                } else {
                    u128::MAX
                };
                let char_remaining = if char_blink_supported {
                    state.character_blink.time_until_next(now)
                } else {
                    u128::MAX
                };

                let remaining = caret_remaining.min(char_remaining);
                if remaining == u128::MAX {
                    None
                } else {
                    Some(Duration::from_millis(remaining as u64))
                }
            };

            // For marching ants, we need ~30fps animation
            let selection_frame_time = if needs_marching_ants {
                Some(Duration::from_millis(33)) // ~30fps for marching ants
            } else {
                None
            };

            // Use the shorter of the two timings
            let next_frame = match (next_blink_time, selection_frame_time) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (Some(a), None) => Some(a),
                (None, Some(b)) => Some(b),
                (None, None) => None,
            };

            if let Some(delay) = next_frame {
                let next = *_instant + delay;
                return Some(iced::widget::Action::request_redraw_at(next));
            }
        }

        if let iced::Event::Mouse(mouse_event) = event {
            let viewport = self.term.viewport.read();
            let render_info = self.term.render_info.read();

            // Mouse events should be relative to the terminal widget.
            // `TerminalMouseEvent.pixel_position` is documented as widget-relative.
            let local_pos_in_bounds = cursor.position_in(bounds);
            let local_pos_unclamped = cursor.position().map(|p| iced::Point {
                x: p.x - bounds.x,
                y: p.y - bounds.y,
            });

            if let mouse::Event::ButtonReleased(button) = mouse_event {
                if state.dragging && (matches!(button, mouse::Button::Left) || matches!(button, mouse::Button::Right)) {
                    state.dragging = false;
                    state.drag_anchor = None;
                    state.last_drag_position = None;

                    // Use unclamped for drag release to get position even outside viewport
                    let (pixel_pos, cell_pos) = if let Some(position) = local_pos_unclamped {
                        let pixel_pos = (position.x, position.y);
                        let cell_pos = state.map_mouse_to_cell_unclamped(&render_info, position.x, position.y, &viewport);
                        (pixel_pos, Some(cell_pos))
                    } else {
                        ((0.0, 0.0), None)
                    };

                    let modifiers = Self::get_modifiers();
                    let evt = TerminalMouseEvent::new(pixel_pos, cell_pos, state.drag_button, modifiers);
                    state.drag_button = MouseButton::None;
                    return Some(iced::widget::Action::publish(TerminalMessage::Release(evt)));
                }
            }

            if state.dragging {
                if let mouse::Event::CursorMoved { .. } = mouse_event {
                    if let Some(position) = local_pos_unclamped {
                        let pixel_pos = (position.x, position.y);
                        // Use unclamped version during drag to allow operations beyond viewport
                        let cell_pos = state.map_mouse_to_cell_unclamped(&render_info, position.x, position.y, &viewport);

                        if state.last_drag_position == Some(cell_pos) {
                            return None;
                        }
                        state.last_drag_position = Some(cell_pos);

                        let modifiers = Self::get_modifiers();
                        let evt = TerminalMouseEvent::new(pixel_pos, Some(cell_pos), state.drag_button, modifiers);
                        return Some(iced::widget::Action::publish(TerminalMessage::Drag(evt)));
                    }
                }
            }

            if !is_over {
                return None;
            }

            match mouse_event {
                mouse::Event::CursorMoved { .. } => {
                    if let Some(position) = local_pos_in_bounds {
                        let pixel_pos = (position.x, position.y);
                        let cell_pos = state.map_mouse_to_cell(&render_info, position.x, position.y, &viewport);

                        if state.last_move_position == cell_pos {
                            return None;
                        }
                        state.last_move_position = cell_pos;

                        if state.hovered_cell != cell_pos {
                            state.hovered_cell = cell_pos;
                        }

                        let modifiers = Self::get_modifiers();
                        let button = if state.dragging { state.drag_button } else { MouseButton::None };
                        let evt = TerminalMouseEvent::new(pixel_pos, cell_pos, button, modifiers);

                        if state.dragging {
                            return Some(iced::widget::Action::publish(TerminalMessage::Drag(evt)));
                        } else {
                            return Some(iced::widget::Action::publish(TerminalMessage::Move(evt)));
                        }
                    }
                }

                mouse::Event::ButtonPressed(button) => {
                    if let Some(position) = local_pos_in_bounds {
                        let pixel_pos = (position.x, position.y);
                        let cell_pos = state.map_mouse_to_cell(&render_info, position.x, position.y, &viewport);

                        if std::env::var_os("ICY_DEBUG_MOUSE_MAPPING").is_some() {
                            let clamped_term = render_info.screen_to_terminal_pixels(position.x, position.y);
                            let (term_x_u, term_y_u_raw) = render_info.screen_to_terminal_pixels_unclamped(position.x, position.y);
                            let term_y_u = if render_info.scan_lines { term_y_u_raw / 2.0 } else { term_y_u_raw };
                            let font_w = render_info.font_width.max(1.0);
                            let font_h = render_info.font_height.max(1.0);
                            let abs_px_x = term_x_u + viewport.scroll_x;
                            let abs_px_y = term_y_u + viewport.scroll_y;
                            let dbg_cell_x = (abs_px_x / font_w).floor() as i32;
                            let dbg_cell_y = (abs_px_y / font_h).floor() as i32;

                            eprintln!(
                                "[mouse_map][press {:?}] pos=({:.3},{:.3}) cell={:?} term_clamped={:?} term_u=({:.3},{:.3}) abs_px=({:.3},{:.3}) dbg_cell=({},{}); vp(scroll=({:.3},{:.3}) vis=({:.3},{:.3}) zoom={:.3}); ri(scale={:.3} vp=({:.3},{:.3},{:.3},{:.3}) term=({:.3},{:.3}) font=({:.3},{:.3}) scanlines={})",
                                button,
                                position.x,
                                position.y,
                                cell_pos,
                                clamped_term,
                                term_x_u,
                                term_y_u,
                                abs_px_x,
                                abs_px_y,
                                dbg_cell_x,
                                dbg_cell_y,
                                viewport.scroll_x,
                                viewport.scroll_y,
                                viewport.visible_width,
                                viewport.visible_height,
                                viewport.zoom,
                                render_info.display_scale,
                                render_info.viewport_x,
                                render_info.viewport_y,
                                render_info.viewport_width,
                                render_info.viewport_height,
                                render_info.terminal_width,
                                render_info.terminal_height,
                                render_info.font_width,
                                render_info.font_height,
                                render_info.scan_lines
                            );
                        }

                        let mouse_button = match button {
                            mouse::Button::Left => MouseButton::Left,
                            mouse::Button::Middle => MouseButton::Middle,
                            mouse::Button::Right => MouseButton::Right,
                            _ => return None,
                        };

                        if matches!(button, mouse::Button::Left | mouse::Button::Right) {
                            state.dragging = true;
                            state.drag_button = mouse_button;
                            state.drag_anchor = cell_pos;
                            state.last_drag_position = cell_pos;
                        }

                        let modifiers = Self::get_modifiers();
                        let evt = TerminalMouseEvent::new(pixel_pos, cell_pos, mouse_button, modifiers);

                        return Some(iced::widget::Action::publish(TerminalMessage::Press(evt)));
                    }
                }

                mouse::Event::ButtonReleased(button) => {
                    // Middle button single-click (Middle doesn't support drag)
                    if matches!(button, mouse::Button::Middle) {
                        if let Some(position) = local_pos_in_bounds {
                            let pixel_pos = (position.x, position.y);
                            let cell_pos = state.map_mouse_to_cell(&render_info, position.x, position.y, &viewport);

                            let modifiers = Self::get_modifiers();
                            let evt = TerminalMouseEvent::new(pixel_pos, cell_pos, MouseButton::Middle, modifiers);

                            return Some(iced::widget::Action::publish(TerminalMessage::Release(evt)));
                        }
                    // Left/Right single-click (only if not dragging)
                    } else if !state.dragging {
                        if let Some(position) = local_pos_in_bounds {
                            let pixel_pos = (position.x, position.y);
                            let cell_pos = state.map_mouse_to_cell(&render_info, position.x, position.y, &viewport);

                            state.dragging = false;
                            state.drag_anchor = None;
                            state.last_drag_position = None;
                            state.last_move_position = None;

                            let mouse_button = match button {
                                mouse::Button::Left => MouseButton::Left,
                                mouse::Button::Right => MouseButton::Right,
                                _ => return None,
                            };

                            let modifiers = Self::get_modifiers();
                            let evt = TerminalMouseEvent::new(pixel_pos, cell_pos, mouse_button, modifiers);

                            return Some(iced::widget::Action::publish(TerminalMessage::Release(evt)));
                        }
                    }
                }

                mouse::Event::WheelScrolled { delta } => {
                    let modifiers = Self::get_modifiers();

                    if modifiers.ctrl || crate::is_command_pressed() {
                        return Some(iced::widget::Action::publish(TerminalMessage::Zoom(crate::ZoomMessage::Wheel(*delta))));
                    }

                    return Some(iced::widget::Action::publish(TerminalMessage::Scroll(*delta)));
                }

                _ => {}
            }
        }
        None
    }
}

impl<'a> shader::Program<TerminalMessage> for CRTShaderProgram<'a> {
    type State = CRTShaderState;
    type Primitive = TerminalShader;

    fn draw(&self, state: &Self::State, _cursor: mouse::Cursor, _bounds: Rectangle) -> Self::Primitive {
        self.internal_draw(state, _cursor, _bounds)
    }

    fn update(
        &self,
        state: &mut CRTShaderState,
        event: &iced::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<iced::widget::Action<TerminalMessage>> {
        self.internal_update(state, event, bounds, cursor)
    }

    fn mouse_interaction(&self, _state: &Self::State, bounds: Rectangle, cursor: mouse::Cursor) -> mouse::Interaction {
        if !cursor.is_over(bounds) {
            return mouse::Interaction::default();
        }

        self.term.cursor_icon.read().unwrap_or_default()
    }
}
