use crate::{
    compute_viewport_auto, compute_viewport_manual,
    shared_render_cache::{SharedCachedTile, TileCacheKey, TILE_HEIGHT},
    tile_cache::MAX_TEXTURE_SLICES,
    CRTShaderState, CaretFrame, MonitorSettings, Terminal, TerminalShader, TextureSliceData,
};
use icy_engine::{CaretShape, EditableScreen};
use std::sync::{atomic::Ordering, Arc};

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

    pub fn frame(&self, state: &CRTShaderState, size: [f32; 2], pixels_per_point: f32) -> TerminalShader {
        let [bounds_width, bounds_height] = size;
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
        let viewport_width: f32;
        let viewport_height: f32;
        let tile_indices: Vec<i32>;
        let selection: Option<icy_engine::Selection>;
        let selection_fg: Option<icy_engine::Color>;
        let selection_bg: Option<icy_engine::Color>;
        let resolution: icy_engine::Size;
        let tile_height: f32;
        let full_content_height_raw: f32;

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

            // When the terminal is wrapped in a scrollable, `bounds` can represent the *content*
            // size, not the visible viewport size. For fitting the terminal window height we
            // must use the actual visible viewport height (reported via `show_viewport`).
            let scroll_state_for_fit = self.term.scroll_state();
            let fit_bounds_w = if scroll_state_for_fit.viewport_width_px > 1.0 {
                scroll_state_for_fit.viewport_width_px
            } else {
                bounds_width
            };
            let fit_bounds_h = if scroll_state_for_fit.viewport_height_px > 1.0 {
                scroll_state_for_fit.viewport_height_px
            } else {
                bounds_height
            };

            viewport_width = fit_bounds_w;
            viewport_height = fit_bounds_h;

            // Pre-compute the zoom level (needed for clamp_terminal_height_to_viewport)
            let pre_zoom = if self.monitor_settings.scaling_mode.is_auto() {
                1.0
            } else {
                self.monitor_settings
                    .scaling_mode
                    .compute_zoom(
                        original_res_w,
                        original_res_h,
                        fit_bounds_w,
                        fit_bounds_h,
                        self.monitor_settings.use_integer_scaling,
                    )
                    .max(0.001)
            };

            // Optional: Clamp the terminal window height to fit within bounds.
            // For small documents, this preserves their height (enabling centering).
            // For large documents, this shrinks to viewport (using full screen).
            // IMPORTANT: Do NOT apply in FitWidth mode.
            // FitWidth is implemented via uniform scaling + vertical scrolling; clamping the
            // terminal window height based on zoom causes a perceived "max height" and can
            // break aspect-ratio expectations when the window is resized.
            if self.term.fit_terminal_height_to_bounds
                && !matches!(self.monitor_settings.scaling_mode, crate::ScalingMode::Auto)
                && !self.monitor_settings.scaling_mode.is_fit_width()
            {
                if let Some(editable) = screen.as_editable() {
                    clamp_terminal_height_to_viewport(editable, fit_bounds_h, scan_lines, pixels_per_point, pre_zoom);
                }
            }

            let screen_type_changed = state.update_cached_screen_info(&**screen);
            *state.cached_mouse_state.lock() = Some(screen.terminal_state().mouse_state.clone());

            // DEC mode 2026: keep presenting the frame the application last
            // finished while it writes the next one.
            let synchronized = screen.terminal_state().synchronized_output_active();

            char_blink_supported = screen.ice_mode().has_blink();
            blink_on = if char_blink_supported { state.character_blink.is_on() } else { false };

            // Snapshot scroll inputs (scrolling is owned by `scroll_area`).
            let scroll_state = self.term.scroll_state();
            let virtual_size = screen.virtual_size();
            let content_width = virtual_size.width as f32;
            let content_height = virtual_size.height as f32;
            let requested_scroll_x = scroll_state.scroll_x;
            let requested_scroll_y = scroll_state.scroll_y;
            let viewport_changed = false;

            // The visible region must maintain the content's aspect ratio.
            // We use resolution() for the visible aspect ratio (terminal size × font),
            // not the full content_height which includes scrollback.
            // NOTE: We get resolution AFTER fit_terminal_height_to_bounds may have modified it,
            // but we use original_res_h (saved before) for centering in Manual zoom mode.
            resolution = screen.resolution();
            let res_w = resolution.width as f32;
            let res_h = resolution.height as f32;

            // CRITICAL: visible_width/height define what portion of the document is shown.
            // The shader maps UV 0-1 over visible_width/height, so these MUST match
            // the actual content dimensions to avoid stretching.
            //
            // For Auto scaling: show the entire terminal (resolution), centered in widget
            // For FitWidth: fill width but ensure minimum rows visible
            // For Manual scaling: show a portion based on zoom level
            if self.monitor_settings.scaling_mode.is_fit_width() {
                // FitWidth mode: fill width, but ensure terminal_height rows are visible
                // This allows x-stretch but never y-stretch (cutting off rows)
                let min_visible_h = original_res_h; // At least show terminal_height rows
                let dbg = cfg!(debug_assertions) && std::env::var("ICY_DEBUG_FITWIDTH").is_ok();
                if dbg {
                    eprintln!(
                        "[crt] bounds=({:.3}x{:.3}) fit_bounds=({:.3}x{:.3}) viewport_px=({:.3}x{:.3}) res=({:.3}x{:.3}) orig_res=({:.3}x{:.3}) content=({:.3}x{:.3}) req_scroll=({:.3},{:.3})",
                        bounds_width,
                        bounds_height,
                        fit_bounds_w,
                        fit_bounds_h,
                        scroll_state_for_fit.viewport_width_px,
                        scroll_state_for_fit.viewport_height_px,
                        res_w,
                        res_h,
                        original_res_w,
                        original_res_h,
                        content_width,
                        content_height,
                        requested_scroll_x,
                        requested_scroll_y
                    );
                }
                let params = crate::compute_viewport_fit_width(
                    res_w,
                    res_h,
                    fit_bounds_w,
                    fit_bounds_h,
                    content_width,
                    content_height,
                    min_visible_h,
                    requested_scroll_x,
                    requested_scroll_y,
                    self.monitor_settings.use_integer_scaling,
                );
                visible_width = params.visible_width;
                visible_height = params.visible_height;
                scroll_offset_y = params.scroll_offset_y;
                scroll_offset_x = params.scroll_offset_x;
                zoom = params.zoom;

                if dbg {
                    eprintln!(
                        "[crt] fitwidth -> zoom={:.6} visible=({:.3}x{:.3}) scroll=({:.3},{:.3})",
                        zoom, visible_width, visible_height, scroll_offset_x, scroll_offset_y
                    );
                }
            } else if self.monitor_settings.scaling_mode.is_auto() {
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
                    bounds_width,
                    bounds_height,
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
                        res_w, res_h, original_res_h, bounds_width, bounds_height, zoom, visible_width, visible_height
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
                    "[viewport] bounds=({:.1},{:.1}) res=({:.1},{:.1}) orig_res_h={:.1} -> zoom_eff={:.3} vis=({:.1},{:.1}) content=({:.1},{:.1}) scroll_req=({:.1},{:.1}) scroll_px=({:.1},{:.1}) max_scroll=({:.1},{:.1}) ratio=({:.3},{:.3}) mode={:?} int_scale={} ",
                    bounds_width,
                    bounds_height,
                    res_w,
                    res_h,
                    original_res_h,
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
                    self.monitor_settings.scaling_mode,
                    self.monitor_settings.use_integer_scaling
                );
            }

            full_content_height = content_height;
            texture_width = resolution.width as u32;

            // Scrolling is owned by the scroll_area widget. The shader clamps offsets locally
            // and exposes effective values via the shared render cache.

            // Check for content changes that require cache invalidation
            // Use the shared render cache from Terminal
            {
                let mut cache: parking_lot::lock_api::RwLockWriteGuard<'_, parking_lot::RawRwLock, crate::SharedRenderCache> = self.term.render_cache.write();

                // Tiles contain width-dependent RGBA rows. A buffer resize must
                // invalidate them before lookup, even if no dirty range was reported.
                let texture_width_changed = cache.content_width != 0 && cache.content_width != texture_width;

                // Full invalidation when screen type or rendered width changes.
                if screen_type_changed || texture_width_changed {
                    cache.invalidate();
                }

                // Selective tile invalidation based on dirty lines.
                // A synchronized update leaves the range pending, so everything
                // it touched repaints in one go once the update ends.
                let dirty_lines = if synchronized { None } else { screen.get_dirty_lines() };
                if let Some((first_dirty_line, last_dirty_line)) = dirty_lines {
                    // Calculate tile indices from dirty line range
                    let tile_height = crate::TILE_HEIGHT;
                    let font_height = screen.font_dimensions().height.max(1) as u32;
                    let tile_height_lines = tile_height / font_height;
                    if let Some(first_tile) = (first_dirty_line as u32).checked_div(tile_height_lines) {
                        let first_tile = first_tile as i32;
                        let last_tile = (((last_dirty_line as u32).saturating_sub(1)) / tile_height_lines) as i32;
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
            if let Some(frozen) = synchronized.then(|| *state.last_caret.lock()).flatten() {
                caret_pos = frozen.pos;
                caret_size = frozen.size;
                caret_visible = frozen.visible;
                caret_mode = frozen.mode;
            } else {
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

                *state.last_caret.lock() = Some(CaretFrame {
                    pos: caret_pos,
                    size: caret_size,
                    visible: caret_visible,
                    mode: caret_mode,
                });
            }

            // Calculate which tiles we need based on scroll position
            // Each tile is TILE_HEIGHT pixels tall
            tile_height = TILE_HEIGHT as f32;

            // Tile slicing/rendering happens in RAW texture pixel coordinates.
            // Viewport/scroll inputs are in aspect-corrected document pixels.
            let scroll_offset_y_raw = scroll_offset_y / aspect_ratio_y;
            let visible_height_raw = visible_height / aspect_ratio_y;
            full_content_height_raw = full_content_height / aspect_ratio_y;

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
            let mut needed_tiles: Vec<i32> = Vec::with_capacity(desired_count as usize);
            for i in 0..desired_count {
                let idx = first_tile_idx + i;
                if idx <= max_tile_idx {
                    needed_tiles.push(idx);
                }
            }
            tile_indices = needed_tiles;

            first_slice_start_y = first_tile_idx as f32 * tile_height;

            // Get or render each tile using the shared cache for BOTH blink states
            // For icy_term: get selection from screen and render it in the RGBA data
            // (icy_draw uses editor_markers for selection, so this only applies when editor_markers is None)
            (selection, selection_fg, selection_bg) = if self.editor_markers.is_none() {
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
        }

        let render_snapshot = self.term.screen.lock().render_snapshot();

        // Helper to get or render tiles for a specific blink state
        let mut get_or_render_tiles = |blink_state: bool, slices: &mut Vec<TextureSliceData>, heights: &mut Vec<u32>| {
            for &tile_idx in &tile_indices {
                let tile_start_y = tile_idx as f32 * tile_height;
                let tile_end_y = ((tile_idx + 1) as f32 * tile_height).min(full_content_height_raw);
                let actual_tile_height = (tile_end_y - tile_start_y).ceil().max(1.0) as u32;
                let selection_intersects = selection.is_some_and(|selection| {
                    let first_line = selection.anchor.y.min(selection.lead.y);
                    let last_line = selection.anchor.y.max(selection.lead.y);
                    let tile_first_line = (tile_start_y / font_h.max(1) as f32).floor() as i32;
                    let tile_last_line = ((tile_end_y - 1.0).max(tile_start_y) / font_h.max(1) as f32).floor() as i32;
                    first_line <= tile_last_line && last_line >= tile_first_line
                });

                let cache_key = TileCacheKey::new(tile_idx, blink_state);
                let cached_tile = if selection_intersects {
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
                    let tile_region: icy_engine::Rectangle = icy_engine::Rectangle::from(0, tile_start_y as i32, resolution.width, actual_tile_height as i32);

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
                    let (render_size, rgba_data) = if let Some(snapshot) = render_snapshot.as_ref() {
                        snapshot.render_text_region_to_rgba_raw(tile_region, &render_options)
                    } else {
                        self.term.screen.lock().render_text_region_to_rgba_raw(tile_region, &render_options)
                    };
                    let width = render_size.width as u32;
                    let height = render_size.height as u32;

                    let slice = TextureSliceData {
                        rgba_data: Arc::new(rgba_data),
                        width,
                        height,
                    };

                    // Selection changes independently of buffer dirtiness.
                    if !selection_intersects {
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

        // Ensure we have at least one text slice for both states.
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

        let text_slice_count = slices_blink_off.len();
        let mut graphics_slices = Vec::with_capacity(tile_indices.len());
        for (&tile_idx, &height) in tile_indices.iter().zip(slice_heights.iter()) {
            let tile_start_y = tile_idx as f32 * tile_height;
            if let Some(cached) = self.term.render_cache.read().get_overlay(tile_idx).cloned() {
                graphics_slices.push(cached.texture);
                continue;
            }
            let tile_region = icy_engine::Rectangle::from(0, tile_start_y as i32, resolution.width, height as i32);
            let render_options = icy_engine::RenderOptions {
                rect: tile_region.into(),
                blink_on: false,
                selection: None,
                selection_fg: None,
                selection_bg: None,
                override_scan_lines: None,
            };
            let (render_size, rgba_data) = if let Some(snapshot) = render_snapshot.as_ref() {
                snapshot.render_graphics_region_to_rgba_raw(tile_region, &render_options)
            } else {
                self.term.screen.lock().render_graphics_region_to_rgba_raw(tile_region, &render_options)
            };
            let slice = TextureSliceData {
                rgba_data: Arc::new(rgba_data),
                width: render_size.width as u32,
                height: render_size.height as u32,
            };
            self.term.render_cache.write().insert_overlay(
                tile_idx,
                SharedCachedTile {
                    texture: slice.clone(),
                    height,
                    start_y: tile_start_y,
                },
            );
            graphics_slices.push(slice);
            tiles_rendered = true;
        }
        if graphics_slices.is_empty() {
            graphics_slices.extend(slices_blink_off.iter().take(text_slice_count).map(|slice| TextureSliceData {
                rgba_data: Arc::new(vec![0; slice.width as usize * slice.height as usize * 4]),
                width: slice.width,
                height: slice.height,
            }));
        }
        slices_blink_off.extend(graphics_slices.iter().cloned());
        slices_blink_on.extend(graphics_slices);

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
            state.render_generation.fetch_add(1, Ordering::Relaxed) + 1
        } else {
            state.render_generation.load(Ordering::Relaxed)
        };
        let window_key = ((window_first_tile_idx as u32 as u64) << 32) ^ (window_slice_count as u32 as u64);
        let render_generation = base_generation ^ window_key;

        TerminalShader {
            slices_blink_off,
            slices_blink_on,
            text_slice_count,
            slice_heights,
            texture_width,
            total_content_height: full_content_height,
            monitor_settings: self.monitor_settings.clone(),
            instance_id: state.instance_id,
            render_generation,
            zoom,
            viewport_width,
            viewport_height,
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use icy_engine::{AttributedChar, Screen, ScreenSink, Size, TextAttribute, TextScreen};
    use icy_parser_core::{CommandSink, DecMode, OperatingSystemCommand, TerminalCommand};
    use parking_lot::Mutex;

    #[test]
    fn viewport_height_uses_the_frames_pixel_density() {
        let screen: Arc<Mutex<Box<dyn Screen>>> = Arc::new(Mutex::new(Box::new(TextScreen::new(Size::new(80, 100)))));
        let mut terminal = Terminal::new(screen.clone());
        terminal.set_fit_terminal_height_to_bounds(true);
        let settings = Arc::new(MonitorSettings {
            scaling_mode: crate::ScalingMode::Manual(1.0),
            ..Default::default()
        });
        let state = CRTShaderState::new(icy_engine::BufferType::CP437);
        let program = CRTShaderProgram::new(&terminal, settings, None);
        let cell_height = screen.lock().font_dimensions().height as f32;

        for (pixels_per_point, expected_rows) in [(1.0, 10), (2.0, 20), (1.0, 10)] {
            program.frame(&state, [640.0, cell_height * 10.0], pixels_per_point);
            assert_eq!(screen.lock().terminal_state().height(), expected_rows);
        }
    }

    #[test]
    fn palette_change_rebuilds_cached_texture_tiles() {
        let mut screen = TextScreen::new(Size::new(1, 1));
        let mut attribute = TextAttribute::default();
        attribute.set_foreground_ext(4);
        screen.set_char(icy_engine::Position::new(0, 0), AttributedChar::new(219 as char, attribute));

        let screen: Arc<Mutex<Box<dyn Screen>>> = Arc::new(Mutex::new(Box::new(screen)));
        let terminal = Terminal::new(screen.clone());
        let settings = Arc::new(MonitorSettings::default());
        let program = CRTShaderProgram::new(&terminal, settings, None);
        let state = CRTShaderState::new(icy_engine::BufferType::CP437);
        let size = [8.0, 16.0];

        let before = program.frame(&state, size, 1.0);

        {
            let mut screen = screen.lock();
            let editable = screen.as_editable().unwrap();
            ScreenSink::new(editable).operating_system_command(OperatingSystemCommand::SetPaletteColor(4, 12, 34, 56));
        }

        let after = program.frame(&state, size, 1.0);

        assert_ne!(before.render_generation, after.render_generation);
        assert_ne!(before.slices_blink_off[0].rgba_data, after.slices_blink_off[0].rgba_data);
    }

    #[test]
    fn synchronized_output_holds_the_frame_until_it_ends() {
        let screen: Arc<Mutex<Box<dyn Screen>>> = Arc::new(Mutex::new(Box::new(TextScreen::new(Size::new(1, 1)))));
        let terminal = Terminal::new(screen.clone());
        let settings = Arc::new(MonitorSettings::default());
        let program = CRTShaderProgram::new(&terminal, settings, None);
        let state = CRTShaderState::new(icy_engine::BufferType::CP437);
        let size = [8.0, 16.0];

        let before = program.frame(&state, size, 1.0);

        {
            let mut screen = screen.lock();
            let editable = screen.as_editable().unwrap();
            ScreenSink::new(editable).emit(TerminalCommand::CsiDecSetMode(DecMode::SynchronizedOutput, true));
            let mut attribute = TextAttribute::default();
            attribute.set_foreground(4);
            editable.set_char(icy_engine::Position::new(0, 0), AttributedChar::new(219 as char, attribute));
            editable.mark_dirty();
        }

        let during = program.frame(&state, size, 1.0);

        assert_eq!(before.render_generation, during.render_generation);
        assert_eq!(before.slices_blink_off[0].rgba_data, during.slices_blink_off[0].rgba_data);

        // Ending the update must repaint from the range that stayed pending.
        {
            let mut screen = screen.lock();
            let editable = screen.as_editable().unwrap();
            ScreenSink::new(editable).emit(TerminalCommand::CsiDecSetMode(DecMode::SynchronizedOutput, false));
        }

        let after = program.frame(&state, size, 1.0);

        assert_ne!(before.render_generation, after.render_generation);
        assert_ne!(before.slices_blink_off[0].rgba_data, after.slices_blink_off[0].rgba_data);
    }
}
