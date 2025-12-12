//! CRT Shader Program with sliding window rendering
//!
//! This module implements the shader program for terminal rendering using
//! a sliding window of 3 texture slices that cover the visible area plus
//! one tile above and below for smooth scrolling.

use crate::{
    CRTShaderState, Message, MonitorSettings, Terminal, TerminalMouseEvent, TerminalShader, TextureSliceData, is_alt_pressed, is_ctrl_pressed,
    is_shift_pressed,
    shared_render_cache::{SharedCachedTile, TILE_HEIGHT, TileCacheKey},
};
use iced::widget::shader;
use iced::{Rectangle, mouse};
use icy_engine::{CaretShape, KeyModifiers, MouseButton};
use std::sync::Arc;

/// Program wrapper that renders the terminal using sliding window tile approach
pub struct CRTShaderProgram<'a> {
    pub term: &'a Terminal,
    pub monitor_settings: Arc<MonitorSettings>,
}

impl<'a> CRTShaderProgram<'a> {
    pub fn new(term: &'a Terminal, monitor_settings: Arc<MonitorSettings>) -> Self {
        Self { term, monitor_settings }
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
        let now = std::time::Instant::now();
        let mut font_w = 0usize;
        let mut font_h = 0usize;
        let scan_lines;
        let scroll_offset_y: f32;
        let scroll_offset_x: f32;
        let visible_height: f32;
        let visible_width: f32;
        let full_content_height: f32;
        let texture_width: u32;
        let blink_on: bool;

        let mut slices_blink_off: Vec<TextureSliceData> = Vec::new();
        let mut slices_blink_on: Vec<TextureSliceData> = Vec::new();
        let mut slice_heights: Vec<u32> = Vec::new();
        #[allow(unused_assignments)]
        let mut first_slice_start_y: f32 = 0.0;

        // Caret rendering data (computed from screen, rendered in shader)
        let mut caret_pos: [f32; 2] = [0.0, 0.0];
        let mut caret_size: [f32; 2] = [0.0, 0.0];
        let mut caret_visible: bool = false;
        let mut caret_mode: u8 = 0;

        {
            let screen = self.term.screen.lock();
            scan_lines = screen.scan_lines();
            if let Some(font) = screen.font(0) {
                font_w = font.size().width as usize;
                font_h = font.size().height as usize;
            }

            state.update_cached_screen_info(&**screen);
            *state.cached_mouse_state.lock() = Some(screen.terminal_state().mouse_state.clone());

            let current_buffer_version = screen.version();
            blink_on = state.character_blink.is_on();

            // Get viewport info
            let vp = self.term.viewport.read();
            let scale_factor = crate::get_scale_factor();
            let physical_bounds_height = bounds.height * scale_factor;
            let physical_bounds_width = bounds.width * scale_factor;

            let effective_zoom = self.monitor_settings.scaling_mode.compute_zoom(
                vp.content_width,
                vp.content_height,
                physical_bounds_width,
                physical_bounds_height,
                self.monitor_settings.use_integer_scaling,
            );

            visible_height = (physical_bounds_height / effective_zoom).min(vp.content_height);
            let visible_content_width = (physical_bounds_width / effective_zoom).min(vp.content_width);
            visible_width = visible_content_width;

            // Store computed visible dimensions
            vp.bounds_height.store(bounds.height as u32, std::sync::atomic::Ordering::Relaxed);
            vp.bounds_width.store(bounds.width as u32, std::sync::atomic::Ordering::Relaxed);
            vp.computed_visible_height.store(visible_height.to_bits(), std::sync::atomic::Ordering::Relaxed);
            vp.computed_visible_width
                .store(visible_content_width.to_bits(), std::sync::atomic::Ordering::Relaxed);

            let max_scroll_y = (vp.content_height - visible_height).max(0.0);
            scroll_offset_y = vp.scroll_y.clamp(0.0, max_scroll_y);

            let max_scroll_x = (vp.content_width - visible_width).max(0.0);
            scroll_offset_x = vp.scroll_x.clamp(0.0, max_scroll_x);

            full_content_height = vp.content_height;
            texture_width = screen.resolution().width as u32;

            // Clear viewport changed flag
            if vp.changed.load(std::sync::atomic::Ordering::Acquire) {
                vp.changed.store(false, std::sync::atomic::Ordering::Relaxed);
            }

            // Check for content changes that require full cache invalidation
            // Use the shared render cache from Terminal
            {
                let mut cache: parking_lot::lock_api::RwLockWriteGuard<'_, parking_lot::RawRwLock, crate::SharedRenderCache> = self.term.render_cache.write();
                let cache_version = cache.content_version();
                // Cache invalidation when buffer version changes
                if current_buffer_version != cache_version {
                    // Tiles will be cleared by invalidate()
                }
                cache.invalidate(current_buffer_version);
                cache.content_height = full_content_height;
                cache.content_width = texture_width;
                cache.last_blink_state = blink_on;

                // Also check selection changes
                let mut info: parking_lot::lock_api::MutexGuard<'_, parking_lot::RawMutex, crate::CachedScreenInfo> = state.cached_screen_info.lock();
                info.last_buffer_version = current_buffer_version;

                let selection = screen.selection();
                let sel_anchor = selection.as_ref().map(|s| s.anchor);
                let sel_lead = selection.as_ref().map(|s| s.lead);
                let sel_locked = selection.as_ref().map(|s| s.locked).unwrap_or(false);

                if info.last_selection_state.0 != sel_anchor || info.last_selection_state.1 != sel_lead || info.last_selection_state.2 != sel_locked {
                    info.last_selection_state = (sel_anchor, sel_lead, sel_locked);
                    cache.clear();
                }

                info.last_bounds_size = (bounds.width, bounds.height);
            }

            // Compute caret position for shader rendering
            // This must happen AFTER cache invalidation to ensure caret state matches buffer state
            {
                let caret = screen.caret();
                let should_draw = caret.visible && (!caret.blinking || state.caret_blink.is_on()) && self.term.has_focus;

                if should_draw && font_w > 0 && font_h > 0 {
                    let caret_cell_pos = caret.position();
                    let scroll_x = vp.scroll_x as i32;
                    let scroll_y_px = scroll_offset_y as i32;

                    // Convert cell position to pixel position (viewport-relative)
                    let (px_x, px_y) = if caret.use_pixel_positioning {
                        let scan_mult = if scan_lines { 2 } else { 1 };
                        (caret_cell_pos.x - scroll_x, caret_cell_pos.y * scan_mult - scroll_y_px)
                    } else {
                        let scan_mult = if scan_lines { 2 } else { 1 };
                        (
                            caret_cell_pos.x * font_w as i32 - scroll_x,
                            caret_cell_pos.y * font_h as i32 * scan_mult - scroll_y_px,
                        )
                    };

                    let actual_font_h = if scan_lines { font_h * 2 } else { font_h };

                    // Only draw if caret is in visible area
                    // Convert to normalized UV coordinates (0-1) so it works with any zoom level
                    let tex_w = texture_width as f32;
                    let vis_h = visible_height;

                    if px_x >= 0 && px_y >= 0 && (px_x as f32) < tex_w && (px_y as f32) < vis_h {
                        // Normalize to 0-1 UV coordinates
                        caret_pos = [px_x as f32 / tex_w, px_y as f32 / vis_h];
                        caret_size = [font_w as f32 / tex_w, actual_font_h as f32 / vis_h];
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

            // Current tile index based on scroll position
            let current_tile_idx = (scroll_offset_y / tile_height).floor() as i32;

            // We need: previous tile, current tile, next tile
            let first_tile_idx = (current_tile_idx - 1).max(0);
            let max_tile_idx = ((full_content_height / tile_height).ceil() as i32 - 1).max(0);

            // Calculate tile indices to render (up to 3)
            let mut tile_indices: Vec<i32> = Vec::new();
            for i in first_tile_idx..=first_tile_idx + 2 {
                if i <= max_tile_idx {
                    tile_indices.push(i);
                }
            }

            first_slice_start_y = first_tile_idx as f32 * tile_height;

            // Get or render each tile using the shared cache for BOTH blink states
            let resolution = screen.resolution();
            let selection = screen.selection();
            let (fg_sel, bg_sel) = screen.buffer_type().selection_colors();

            // Helper to get or render tiles for a specific blink state
            let get_or_render_tiles = |blink_state: bool, slices: &mut Vec<TextureSliceData>, heights: &mut Vec<u32>| {
                for &tile_idx in &tile_indices {
                    let tile_start_y = tile_idx as f32 * tile_height;
                    let tile_end_y = ((tile_idx + 1) as f32 * tile_height).min(full_content_height);
                    let actual_tile_height = (tile_end_y - tile_start_y).max(1.0) as u32;

                    let cache_key = TileCacheKey::new(tile_idx, blink_state);
                    let cached_tile = self.term.render_cache.read().get(&cache_key).cloned();

                    if let Some(cached) = cached_tile {
                        slices.push(cached.texture);
                        if heights.len() < tile_indices.len() {
                            heights.push(cached.height);
                        }
                    } else {
                        // Render this tile
                        let tile_region: icy_engine::Rectangle =
                            icy_engine::Rectangle::from(0, tile_start_y as i32, resolution.width, actual_tile_height as i32);

                        let render_options = icy_engine::RenderOptions {
                            rect: icy_engine::Rectangle {
                                start: icy_engine::Position::new(0, tile_start_y as i32),
                                size: icy_engine::Size::new(resolution.width, actual_tile_height as i32),
                            }
                            .into(),
                            blink_on: blink_state,
                            selection: selection.clone(),
                            selection_fg: Some(fg_sel.clone()),
                            selection_bg: Some(bg_sel.clone()),
                            override_scan_lines: None,
                        };
                        let (render_size, rgba_data) = screen.render_region_to_rgba(tile_region, &render_options);
                        let width = render_size.width as u32;
                        let height = render_size.height as u32;

                        let slice = TextureSliceData {
                            rgba_data: Arc::new(rgba_data),
                            width,
                            height,
                        };

                        // Cache this tile
                        let cached_tile = SharedCachedTile {
                            texture: slice.clone(),
                            height,
                            start_y: tile_start_y,
                        };
                        self.term.render_cache.write().insert(cache_key, cached_tile);

                        slices.push(slice);
                        if heights.len() < tile_indices.len() {
                            heights.push(height);
                        }
                    }
                }
            };

            // Get tiles for both blink states
            get_or_render_tiles(false, &mut slices_blink_off, &mut slice_heights);
            get_or_render_tiles(true, &mut slices_blink_on, &mut slice_heights);
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

        let zoom = self.term.viewport.read().zoom;

        // Read marker settings from terminal
        let mut markers = self.term.markers.write();
        // Raster and guide are stored in pixel coordinates (already converted by the editor)
        let raster_spacing = markers.raster;
        let guide_pos = markers.guide;

        // Get marker colors from marker_settings if available
        let (raster_color, raster_alpha, guide_color, guide_alpha) = if let Some(ref settings) = markers.marker_settings {
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
            if let Some(ref mut ref_img) = markers.reference_image {
                if ref_img.visible && !ref_img.path.as_os_str().is_empty() {
                    // Load and cache the image data
                    if let Some((data, w, h)) = ref_img.load_and_cache() {
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
        drop(markers);

        println!("CRTShaderProgram::internal_draw took {:?}", now.elapsed());
        TerminalShader {
            slices_blink_off,
            slices_blink_on,
            slice_heights,
            texture_width,
            total_content_height: full_content_height,
            monitor_settings: self.monitor_settings.clone(),
            instance_id: state.instance_id,
            zoom,
            render_info: self.term.render_info.clone(),
            font_width: font_w as f32,
            font_height: font_h as f32,
            scan_lines,
            background_color: *self.term.background_color.read(),
            scroll_offset_y,
            visible_height,
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
        }
    }

    /// Simplified internal_update that only handles coordinate mapping and event emission.
    pub fn internal_update(
        &self,
        state: &mut CRTShaderState,
        event: &iced::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<iced::widget::Action<Message>> {
        let now = crate::Blink::now_ms();

        if let Some(screen) = self.term.screen.try_lock() {
            let buffer_type = screen.buffer_type();
            state.caret_blink.set_rate(buffer_type.caret_blink_rate() as u128);
            state.character_blink.set_rate(buffer_type.blink_rate() as u128);
        }

        state.caret_blink.update(now);
        state.character_blink.update(now);

        let is_over = cursor.is_over(bounds);

        if let iced::Event::Mouse(mouse_event) = event {
            let viewport = self.term.viewport.read();
            let render_info = self.term.render_info.read();

            if let mouse::Event::ButtonReleased(button) = mouse_event {
                if state.dragging && matches!(button, mouse::Button::Left) {
                    state.dragging = false;
                    state.drag_anchor = None;
                    state.last_drag_position = None;

                    let (pixel_pos, cell_pos) = if let Some(position) = cursor.position() {
                        let pixel_pos = (position.x, position.y);
                        let cell_pos = state.map_mouse_to_cell(&render_info, position.x, position.y, &viewport);
                        (pixel_pos, cell_pos)
                    } else {
                        ((0.0, 0.0), None)
                    };

                    let modifiers = Self::get_modifiers();
                    let evt = TerminalMouseEvent::new(pixel_pos, cell_pos, MouseButton::Left, modifiers);
                    return Some(iced::widget::Action::publish(Message::Release(evt)));
                }
            }

            if state.dragging {
                if let mouse::Event::CursorMoved { .. } = mouse_event {
                    if let Some(position) = cursor.position() {
                        let pixel_pos = (position.x, position.y);
                        let cell_pos = state.map_mouse_to_cell(&render_info, position.x, position.y, &viewport);

                        let modifiers = Self::get_modifiers();
                        let evt = TerminalMouseEvent::new(pixel_pos, cell_pos, MouseButton::Left, modifiers);
                        return Some(iced::widget::Action::publish(Message::Drag(evt)));
                    }
                }
            }

            if !is_over {
                return None;
            }

            match mouse_event {
                mouse::Event::CursorMoved { .. } => {
                    if let Some(position) = cursor.position() {
                        let pixel_pos = (position.x, position.y);
                        let cell_pos = state.map_mouse_to_cell(&render_info, position.x, position.y, &viewport);

                        if state.hovered_cell != cell_pos {
                            state.hovered_cell = cell_pos;
                        }

                        let modifiers = Self::get_modifiers();
                        let button = if state.dragging { MouseButton::Left } else { MouseButton::None };
                        let evt = TerminalMouseEvent::new(pixel_pos, cell_pos, button, modifiers);

                        if state.dragging {
                            return Some(iced::widget::Action::publish(Message::Drag(evt)));
                        } else {
                            return Some(iced::widget::Action::publish(Message::Move(evt)));
                        }
                    }
                }

                mouse::Event::ButtonPressed(button) => {
                    if let Some(position) = cursor.position() {
                        let pixel_pos = (position.x, position.y);
                        let cell_pos = state.map_mouse_to_cell(&render_info, position.x, position.y, &viewport);

                        let mouse_button = match button {
                            mouse::Button::Left => MouseButton::Left,
                            mouse::Button::Middle => MouseButton::Middle,
                            mouse::Button::Right => MouseButton::Right,
                            _ => return None,
                        };

                        if matches!(button, mouse::Button::Left) {
                            state.dragging = true;
                            state.drag_anchor = cell_pos;
                            state.last_drag_position = cell_pos;
                        }

                        let modifiers = Self::get_modifiers();
                        let evt = TerminalMouseEvent::new(pixel_pos, cell_pos, mouse_button, modifiers);

                        return Some(iced::widget::Action::publish(Message::Press(evt)));
                    }
                }

                mouse::Event::ButtonReleased(button) => {
                    if !matches!(button, mouse::Button::Left) {
                        if let Some(position) = cursor.position() {
                            let pixel_pos = (position.x, position.y);
                            let cell_pos = state.map_mouse_to_cell(&render_info, position.x, position.y, &viewport);

                            let mouse_button = match button {
                                mouse::Button::Middle => MouseButton::Middle,
                                mouse::Button::Right => MouseButton::Right,
                                _ => return None,
                            };

                            let modifiers = Self::get_modifiers();
                            let evt = TerminalMouseEvent::new(pixel_pos, cell_pos, mouse_button, modifiers);

                            return Some(iced::widget::Action::publish(Message::Release(evt)));
                        }
                    } else if !state.dragging {
                        if let Some(position) = cursor.position() {
                            let pixel_pos = (position.x, position.y);
                            let cell_pos = state.map_mouse_to_cell(&render_info, position.x, position.y, &viewport);

                            state.dragging = false;
                            state.drag_anchor = None;
                            state.last_drag_position = None;

                            let modifiers = Self::get_modifiers();
                            let evt = TerminalMouseEvent::new(pixel_pos, cell_pos, MouseButton::Left, modifiers);

                            return Some(iced::widget::Action::publish(Message::Release(evt)));
                        }
                    }
                }

                mouse::Event::WheelScrolled { delta } => {
                    let modifiers = Self::get_modifiers();

                    if modifiers.ctrl || crate::is_command_pressed() {
                        return Some(iced::widget::Action::publish(Message::Zoom(crate::ZoomMessage::Wheel(*delta))));
                    }

                    return Some(iced::widget::Action::publish(Message::Scroll(*delta)));
                }

                _ => {}
            }
        }
        None
    }
}

impl<'a> shader::Program<Message> for CRTShaderProgram<'a> {
    type State = CRTShaderState;
    type Primitive = TerminalShader;

    fn draw(&self, state: &Self::State, _cursor: mouse::Cursor, _bounds: Rectangle) -> Self::Primitive {
        self.internal_draw(state, _cursor, _bounds)
    }

    fn update(&self, state: &mut CRTShaderState, event: &iced::Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<iced::widget::Action<Message>> {
        self.internal_update(state, event, bounds, cursor)
    }

    fn mouse_interaction(&self, _state: &Self::State, bounds: Rectangle, cursor: mouse::Cursor) -> mouse::Interaction {
        if !cursor.is_over(bounds) {
            return mouse::Interaction::default();
        }

        self.term.cursor_icon.read().unwrap_or_default()
    }
}
