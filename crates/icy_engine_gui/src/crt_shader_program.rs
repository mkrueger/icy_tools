//! CRT Shader Program with sliding window rendering
//!
//! This module implements the shader program for terminal rendering using
//! a sliding window of 3 texture slices that cover the visible area plus
//! one tile above and below for smooth scrolling.

use crate::{
    CRTShaderState, Message, MonitorSettings, Terminal, TerminalMouseEvent, TerminalShader, TextureSliceData, 
    is_alt_pressed, is_ctrl_pressed, is_shift_pressed,
    shared_render_cache::{SharedCachedTile, TileCacheKey, TILE_HEIGHT},
};
use iced::widget::shader;
use iced::{Rectangle, mouse};
use icy_engine::{Caret, CaretShape, KeyModifiers, MouseButton};
use std::sync::Arc;

/// Program wrapper that renders the terminal using sliding window tile approach
pub struct CRTShaderProgram<'a> {
    pub term: &'a Terminal,
    pub monitor_settings: MonitorSettings,
}

impl<'a> CRTShaderProgram<'a> {
    pub fn new(term: &'a Terminal, monitor_settings: MonitorSettings) -> Self {
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
        let mut font_w = 0usize;
        let mut font_h = 0usize;
        let scan_lines;
        let scroll_offset_y: f32;
        let visible_height: f32;
        let full_content_height: f32;
        let texture_width: u32;

        let mut slices: Vec<TextureSliceData> = Vec::new();
        let mut slice_heights: Vec<u32> = Vec::new();
        #[allow(unused_assignments)]
        let mut first_slice_start_y: f32 = 0.0;

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
            let blink_on = state.character_blink.is_on();

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

            // Store computed visible dimensions
            vp.bounds_height.store(bounds.height as u32, std::sync::atomic::Ordering::Relaxed);
            vp.bounds_width.store(bounds.width as u32, std::sync::atomic::Ordering::Relaxed);
            vp.computed_visible_height.store(visible_height.to_bits(), std::sync::atomic::Ordering::Relaxed);
            vp.computed_visible_width.store(visible_content_width.to_bits(), std::sync::atomic::Ordering::Relaxed);

            let max_scroll_y = (vp.content_height - visible_height).max(0.0);
            scroll_offset_y = vp.scroll_y.clamp(0.0, max_scroll_y);
            full_content_height = vp.content_height;
            texture_width = screen.resolution().width as u32;
            
            // Clear viewport changed flag
            if vp.changed.load(std::sync::atomic::Ordering::Acquire) {
                vp.changed.store(false, std::sync::atomic::Ordering::Relaxed);
            }

            // Check for content changes that require full cache invalidation
            // Use the shared render cache from Terminal
            {
                let mut cache = self.term.render_cache.write();
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

                if info.last_selection_state.0 != sel_anchor 
                    || info.last_selection_state.1 != sel_lead 
                    || info.last_selection_state.2 != sel_locked 
                {
                    info.last_selection_state = (sel_anchor, sel_lead, sel_locked);
                    cache.clear();
                }

                info.last_bounds_size = (bounds.width, bounds.height);
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

            // Get or render each tile using the shared cache
            let resolution = screen.resolution();
            let selection = screen.selection();
            let (fg_sel, bg_sel) = screen.buffer_type().selection_colors();
            for tile_idx in tile_indices {
                let tile_start_y = tile_idx as f32 * tile_height;
                let tile_end_y = ((tile_idx + 1) as f32 * tile_height).min(full_content_height);
                let actual_tile_height = (tile_end_y - tile_start_y).max(1.0) as u32;
                // Check shared render cache
                let cache_key = TileCacheKey::new(tile_idx, blink_on);
                let cached_tile = self.term.render_cache.read().get(&cache_key).cloned();
                if cached_tile.is_none() {
                    println!("Requesting tile {} (y: {} to {}, height: {}) found : {}", tile_idx, tile_start_y, tile_end_y, actual_tile_height, screen.version());
                }
                if let Some(cached) = cached_tile {
                    slices.push(cached.texture);
                    slice_heights.push(cached.height);
                } else {
                    // Render this tile
                    let tile_region = icy_engine::Rectangle::from(
                        0,
                        tile_start_y as i32,
                        resolution.width,
                        actual_tile_height as i32,
                    );

                    let render_options = icy_engine::RenderOptions {
                        rect: icy_engine::Rectangle {
                            start: icy_engine::Position::new(0, tile_start_y as i32),
                            size: icy_engine::Size::new(resolution.width, actual_tile_height as i32),
                        }.into(),
                        blink_on,
                        selection: selection.clone(),
                        selection_fg: Some(fg_sel.clone()),
                        selection_bg: Some(bg_sel.clone()),
                        override_scan_lines: None,
                    };
                    println!("Rendering tile {} (y: {} to {}, height: {})", tile_idx, tile_start_y, tile_end_y, actual_tile_height);
                    let (render_size, rgba_data) = screen.render_region_to_rgba(tile_region, &render_options);
                    let width = render_size.width as u32;
                    let height = render_size.height as u32;

                    let slice = TextureSliceData {
                        rgba_data: Arc::new(rgba_data),
                        width,
                        height,
                    };

                    // Cache this tile in the shared render cache
                    let cached_tile = SharedCachedTile {
                        texture: slice.clone(),
                        height,
                        start_y: tile_start_y,
                    };
                    self.term.render_cache.write().insert(cache_key, cached_tile);

                    slices.push(slice);
                    slice_heights.push(height);
                }
            }
        }

        // Ensure we have at least one slice
        if slices.is_empty() {
            slices.push(TextureSliceData {
                rgba_data: Arc::new(vec![0u8; 4]),
                width: 1,
                height: 1,
            });
            slice_heights.push(1);
            first_slice_start_y = 0.0;
        }

        let zoom = self.term.viewport.read().zoom;

        TerminalShader {
            slices,
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

    pub fn draw_caret(
        &self,
        caret: &Caret,
        state: &CRTShaderState,
        rgba_data: &mut Vec<u8>,
        size: (u32, u32),
        font_w: usize,
        font_h: usize,
        scan_lines: bool,
        scroll_x: i32,
        scroll_y: i32,
    ) {
        let should_draw = caret.visible && (!caret.blinking || state.caret_blink.is_on());
        let font_w = font_w as i32;
        let font_h = font_h as i32;

        if should_draw && self.term.has_focus {
            let caret_pos = caret.position();
            if font_w > 0 && font_h > 0 && size.0 > 0 && size.1 > 0 {
                let line_bytes = (size.0 as i32) * 4;
                let cell_x = caret_pos.x;
                let cell_y = caret_pos.y;

                if cell_x >= 0 && cell_y >= 0 {
                    let (px_x, px_y) = if caret.use_pixel_positioning {
                        (caret_pos.x - scroll_x, caret_pos.y * if scan_lines { 2 } else { 1 } - scroll_y)
                    } else {
                        (
                            cell_x * font_w - scroll_x,
                            if scan_lines { cell_y * font_h * 2 } else { cell_y * font_h } - scroll_y,
                        )
                    };
                    let actual_font_h = if scan_lines { font_h * 2 } else { font_h };

                    if px_x >= 0 && px_y >= 0 && px_x + font_w <= size.0 as i32 && px_y + actual_font_h <= size.1 as i32 {
                        match caret.shape {
                            CaretShape::Bar => {
                                let bar_width = if caret.insert_mode { font_w / 2 } else { 2.min(font_w) };

                                for row in 0..actual_font_h {
                                    let row_offset = ((px_y + row) * line_bytes + px_x * 4) as usize;
                                    let slice = &mut rgba_data[row_offset..row_offset + bar_width as usize * 4];
                                    for p in slice.chunks_exact_mut(4) {
                                        p[0] = 255 - p[0];
                                        p[1] = 255 - p[1];
                                        p[2] = 255 - p[2];
                                    }
                                }
                            }
                            CaretShape::Block => {
                                let start = if caret.insert_mode { actual_font_h / 2 } else { 0 };
                                for row in start..actual_font_h {
                                    let row_offset = ((px_y + row) * line_bytes + px_x * 4) as usize;
                                    let slice = &mut rgba_data[row_offset..row_offset + font_w as usize * 4];
                                    for p in slice.chunks_exact_mut(4) {
                                        p[0] = 255 - p[0];
                                        p[1] = 255 - p[1];
                                        p[2] = 255 - p[2];
                                    }
                                }
                            }
                            CaretShape::Underline => {
                                let start_row = if caret.insert_mode {
                                    actual_font_h / 2
                                } else {
                                    actual_font_h.saturating_sub(2)
                                };
                                for row in start_row..actual_font_h {
                                    let row_offset = ((px_y + row) * line_bytes + px_x * 4) as usize;
                                    let slice = &mut rgba_data[row_offset..row_offset + font_w as usize * 4];
                                    for p in slice.chunks_exact_mut(4) {
                                        p[0] = 255 - p[0];
                                        p[1] = 255 - p[1];
                                        p[2] = 255 - p[2];
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
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
