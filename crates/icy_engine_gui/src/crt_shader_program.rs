use crate::{
    CRTShaderState, Message, MonitorSettings, RenderUnicodeOptions, Terminal, TerminalMouseEvent, TerminalShader, is_alt_pressed, is_ctrl_pressed,
    is_shift_pressed, render_unicode_to_rgba,
};
use iced::widget::shader;
use iced::{Rectangle, mouse};
use icy_engine::{Caret, CaretShape, KeyModifiers, MouseButton};

// Program wrapper that renders the terminal and creates the shader
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

    /// Simplified internal_update that only handles coordinate mapping and event emission.
    /// All application-specific logic (selection, RIP fields, mouse tracking) is delegated
    /// to the consuming application via Message variants.
    pub fn internal_update(
        &self,
        state: &mut CRTShaderState,
        event: &iced::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<iced::widget::Action<Message>> {
        let now = crate::Blink::now_ms();

        // Synchronize blink rates with buffer type (only if screen is available without blocking)
        // This handles cases where buffer type changes during runtime
        if let Some(screen) = self.term.screen.try_lock() {
            let buffer_type = screen.buffer_type();
            state.caret_blink.set_rate(buffer_type.caret_blink_rate() as u128);
            state.character_blink.set_rate(buffer_type.blink_rate() as u128);
        }

        state.caret_blink.update(now);
        state.character_blink.update(now);

        // Check if cursor is over bounds
        let is_over = cursor.is_over(bounds);

        // Handle mouse events - always process Release events and Drag events while dragging
        // to support "snapped" drag behavior (like scrollbars)
        if let iced::Event::Mouse(mouse_event) = event {
            // Read viewport and render_info for mouse coordinate calculations
            let viewport = self.term.viewport.read();
            let render_info = self.term.render_info.read();

            // Handle Release events globally (even when cursor is outside bounds)
            // This ensures drag state is properly cleaned up
            if let mouse::Event::ButtonReleased(button) = mouse_event {
                if state.dragging && matches!(button, mouse::Button::Left) {
                    state.dragging = false;
                    state.drag_anchor = None;
                    state.last_drag_position = None;

                    // Get position (may be outside bounds, that's ok)
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

            // Handle Drag events while dragging (even when cursor is outside bounds)
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

            // For other events, only process if cursor is over bounds
            if !is_over {
                return None;
            }

            match mouse_event {
                mouse::Event::CursorMoved { .. } => {
                    if let Some(position) = cursor.position() {
                        let pixel_pos = (position.x, position.y);
                        let cell_pos = state.map_mouse_to_cell(&render_info, position.x, position.y, &viewport);

                        // Update hovered cell for mouse_interaction cursor changes
                        if state.hovered_cell != cell_pos {
                            state.hovered_cell = cell_pos;
                        }

                        let modifiers = Self::get_modifiers();
                        let button = if state.dragging { MouseButton::Left } else { MouseButton::None };
                        let evt = TerminalMouseEvent::new(pixel_pos, cell_pos, button, modifiers);

                        // Emit Drag if dragging, otherwise Move
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

                        // Convert iced mouse button to our MouseButton type
                        let mouse_button = match button {
                            mouse::Button::Left => MouseButton::Left,
                            mouse::Button::Middle => MouseButton::Middle,
                            mouse::Button::Right => MouseButton::Right,
                            _ => return None,
                        };

                        // Update drag state for left button
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
                    // Only handle non-left-button releases here (left-button is handled above for drag snapping)
                    if !matches!(button, mouse::Button::Left) {
                        if let Some(position) = cursor.position() {
                            let pixel_pos = (position.x, position.y);
                            let cell_pos = state.map_mouse_to_cell(&render_info, position.x, position.y, &viewport);

                            // Convert iced mouse button to our MouseButton type
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
                        // Left button release when not dragging (e.g., simple click release)
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

                    // Check if Cmd/Ctrl is held for zooming
                    if modifiers.ctrl || crate::is_command_pressed() {
                        return Some(iced::widget::Action::publish(Message::Zoom(crate::ZoomMessage::Wheel(*delta))));
                    }

                    // Pass the scroll delta directly (WheelDelta is a re-export of ScrollDelta)
                    return Some(iced::widget::Action::publish(Message::Scroll(*delta)));
                }

                _ => {}
            }
        }
        None
    }

    fn internal_draw(&self, state: &CRTShaderState, _cursor: mouse::Cursor, bounds: Rectangle) -> TerminalShader {
        // Fast path: reuse cached buffer if content not dirty; only reapply caret/blink overlays.
        let mut rgba_data: Vec<u8>;
        let size: (u32, u32);
        let mut font_w = 0usize;
        let mut font_h = 0usize;
        let scan_lines;
        // Check if we need to re-render based on buffer version and blink state
        let mut needs_full_render = false;
        {
            let screen = self.term.screen.lock();
            scan_lines = screen.scan_lines();
            if let Some(font) = screen.font(0) {
                font_w = font.size().width as usize;
                font_h = font.size().height as usize;
            }

            // Cache screen info and mouse state for use in internal_update (avoids extra locks there)
            state.update_cached_screen_info(&**screen);
            *state.cached_mouse_state.lock() = Some(screen.terminal_state().mouse_state.clone());

            let current_buffer_version = screen.version();
            let blink_on = state.character_blink.is_on();

            // Check if buffer version or selection changed (need single lock for cached_screen_info)
            let selection = screen.selection();
            let sel_anchor = selection.as_ref().map(|s| s.anchor);
            let sel_lead = selection.as_ref().map(|s| s.lead);
            let sel_locked = selection.as_ref().map(|s| s.locked).unwrap_or(false);

            {
                let mut info = state.cached_screen_info.lock();
                let vp = self.term.viewport.read();

                // Check if buffer version changed (content modified)
                if current_buffer_version != info.last_buffer_version || vp.changed.load(std::sync::atomic::Ordering::Acquire) {
                    needs_full_render = true;
                    vp.changed.store(false, std::sync::atomic::Ordering::Relaxed);
                    info.last_buffer_version = current_buffer_version;
                }

                // Check if selection changed (affects highlighting)
                if info.last_selection_state.0 != sel_anchor || info.last_selection_state.1 != sel_lead || info.last_selection_state.2 != sel_locked {
                    needs_full_render = true;
                    info.last_selection_state = (sel_anchor, sel_lead, sel_locked);
                }

                // Check if bounds size changed (window resize) - need to re-render with new visible height
                let current_bounds = (bounds.width, bounds.height);
                if (info.last_bounds_size.0 - current_bounds.0).abs() > 0.5 || (info.last_bounds_size.1 - current_bounds.1).abs() > 0.5 {
                    needs_full_render = true;
                    info.last_bounds_size = current_bounds;
                }
            }

            if needs_full_render {
                // Full re-render - generate both blink on and blink off versions
                let (fg_sel, bg_sel) = screen.buffer_type().selection_colors();

                // Render both versions
                let (render_blink_on, render_blink_off) = if matches!(screen.buffer_type(), icy_engine::BufferType::Unicode) {
                    let render_on = render_unicode_to_rgba(
                        &**screen,
                        &RenderUnicodeOptions {
                            selection,
                            selection_fg: Some(fg_sel.clone()),
                            selection_bg: Some(bg_sel.clone()),
                            blink_on: true,
                            font_px_size: Some(font_h as f32),
                            glyph_cache: state.unicode_glyph_cache.clone(),
                        },
                    );
                    let render_off = render_unicode_to_rgba(
                        &**screen,
                        &RenderUnicodeOptions {
                            selection,
                            selection_fg: Some(fg_sel.clone()),
                            selection_bg: Some(bg_sel.clone()),
                            blink_on: false,
                            font_px_size: Some(font_h as f32),
                            glyph_cache: state.unicode_glyph_cache.clone(),
                        },
                    );
                    (render_on, render_off)
                } else {
                    // Use viewport-based region rendering for both normal and scrollback mode
                    // Get the actual content resolution
                    let resolution = screen.resolution();

                    let vp = self.term.viewport.read();

                    // bounds are in logical pixels, but we need physical pixels for the texture
                    // The shader's clip_bounds will be in physical pixels (bounds * scale_factor)
                    let scale_factor = crate::get_scale_factor();
                    let physical_bounds_height = bounds.height * scale_factor;
                    let physical_bounds_width = bounds.width * scale_factor;

                    // Calculate effective zoom: for Auto mode, compute from bounds; for Manual, use viewport zoom
                    let effective_zoom = self.monitor_settings.scaling_mode.compute_zoom(
                        vp.content_width,
                        vp.content_height,
                        physical_bounds_width,
                        physical_bounds_height,
                        self.monitor_settings.use_integer_scaling,
                    );

                    // Calculate how many content pixels can be shown in the available screen space
                    // At zoom=1: visible_content = physical_bounds (in content pixels)
                    // At zoom=2: visible_content = physical_bounds/2 (we see half as much content)
                    let visible_content_height = (physical_bounds_height / effective_zoom).min(vp.content_height);
                    let visible_content_width = (physical_bounds_width / effective_zoom).min(vp.content_width);

                    // Store computed visible dimensions in viewport for scrollbar calculations
                    // These are stored as f32 bits in atomic fields for thread-safe access
                    vp.bounds_height.store(bounds.height as u32, std::sync::atomic::Ordering::Relaxed);
                    vp.bounds_width.store(bounds.width as u32, std::sync::atomic::Ordering::Relaxed);
                    vp.computed_visible_height
                        .store(visible_content_height.to_bits(), std::sync::atomic::Ordering::Relaxed);
                    vp.computed_visible_width
                        .store(visible_content_width.to_bits(), std::sync::atomic::Ordering::Relaxed);

                    // Clamp scroll to valid range based on current visible content
                    // This prevents rendering past the content bounds
                    let max_scroll_y = (vp.content_height - visible_content_height).max(0.0);
                    let max_scroll_x = (vp.content_width - visible_content_width).max(0.0);
                    let clamped_scroll_y = vp.scroll_y.clamp(0.0, max_scroll_y);
                    let clamped_scroll_x = vp.scroll_x.clamp(0.0, max_scroll_x);

                    // Create viewport region - scroll_x/y are already in content coordinates
                    let viewport_region = icy_engine::Rectangle::from(
                        clamped_scroll_x as i32,
                        clamped_scroll_y as i32,
                        visible_content_width as i32,
                        visible_content_height as i32,
                    );

                    let base_options = icy_engine::RenderOptions {
                        rect: icy_engine::Rectangle {
                            start: icy_engine::Position::new(0, 0),
                            size: icy_engine::Size::new(resolution.width, visible_content_height as i32),
                        }
                        .into(),
                        blink_on: true,
                        selection,
                        selection_fg: Some(fg_sel.clone()),
                        selection_bg: Some(bg_sel.clone()),
                        override_scan_lines: None,
                    };

                    let render_on = screen.render_region_to_rgba(viewport_region, &base_options);

                    let base_options_off = icy_engine::RenderOptions {
                        rect: icy_engine::Rectangle {
                            start: icy_engine::Position::new(0, 0),
                            size: icy_engine::Size::new(resolution.width, visible_content_height as i32),
                        }
                        .into(),
                        blink_on: false,
                        selection,
                        selection_fg: Some(fg_sel.clone()),
                        selection_bg: Some(bg_sel.clone()),
                        override_scan_lines: None,
                    };

                    let render_off = screen.render_region_to_rgba(viewport_region, &base_options_off);
                    (render_on, render_off)
                };

                size = (render_blink_on.0.width as u32, render_blink_on.0.height as u32);

                // Use the appropriate version based on current blink state
                rgba_data = if blink_on { render_blink_on.1.clone() } else { render_blink_off.1.clone() };

                // Cache both versions
                {
                    let mut cached_on = state.cached_rgba_blink_on.lock();
                    if cached_on.len() != render_blink_on.1.len() {
                        cached_on.clear();
                        cached_on.reserve(render_blink_on.1.len());
                    }
                    cached_on.clone_from(&render_blink_on.1);

                    let mut cached_off = state.cached_rgba_blink_off.lock();
                    if cached_off.len() != render_blink_off.1.len() {
                        cached_off.clear();
                        cached_off.reserve(render_blink_off.1.len());
                    }
                    cached_off.clone_from(&render_blink_off.1);
                }
                state.cached_screen_info.lock().render_size = size;
            } else {
                // Reuse cached images - just pick the right one based on blink state
                size = state.cached_screen_info.lock().render_size;
                rgba_data = if blink_on {
                    state.cached_rgba_blink_on.lock().clone()
                } else {
                    state.cached_rgba_blink_off.lock().clone()
                };
            }

            // Always overlay the caret if visible and blinking is on
            // Caret is just an inversion overlay, no need to track changes
            if state.caret_blink.is_on() {
                let vp = self.term.viewport.read();
                let scroll_x = vp.scroll_x as i32;
                let scroll_y = vp.scroll_y as i32;
                drop(vp);
                self.draw_caret(&screen.caret(), state, &mut rgba_data, size, font_w, font_h, scan_lines, scroll_x, scroll_y);
            }
        }

        if rgba_data.len() != (size.0 as usize * size.1 as usize * 4) {
            panic!(
                "RGBA data size mismatch (expected {}, got {})",
                size.0 as usize * size.1 as usize * 4,
                rgba_data.len()
            );
        }

        // Get zoom level from viewport
        let zoom = self.term.viewport.read().zoom;

        TerminalShader {
            terminal_rgba: rgba_data,
            terminal_size: size,
            monitor_settings: self.monitor_settings.clone(),
            instance_id: state.instance_id,
            zoom,
            render_info: self.term.render_info.clone(),
            font_width: font_w as f32,
            font_height: font_h as f32,
            scan_lines,
            background_color: *self.term.background_color.read(),
        }
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
        // Check both the caret's is_blinking property and the blink timer state
        let should_draw = caret.visible && (!caret.blinking || state.caret_blink.is_on());
        let font_w = font_w as i32;
        let font_h = font_h as i32;

        if should_draw && self.term.has_focus {
            let caret_pos = caret.position();
            if font_w > 0 && font_h > 0 && size.0 > 0 && size.1 > 0 {
                let line_bytes = (size.0 as i32) * 4;
                // Adjust caret position by scroll offset
                let cell_x = caret_pos.x;
                let cell_y = caret_pos.y;

                if cell_x >= 0 && cell_y >= 0 {
                    let (px_x, px_y) = if caret.use_pixel_positioning {
                        (caret_pos.x - scroll_x, caret_pos.y * if scan_lines { 2 } else { 1 } - scroll_y)
                    } else {
                        (
                            cell_x * font_w - scroll_x,
                            // In scanline mode, y-position and height are doubled
                            if scan_lines { cell_y * font_h * 2 } else { cell_y * font_h } - scroll_y,
                        )
                    };
                    let actual_font_h = if scan_lines { font_h * 2 } else { font_h };
                    // Check bounds: px_x/px_y can be negative due to scrolling
                    if px_x >= 0 && px_y >= 0 && px_x + font_w <= size.0 as i32 && px_y + actual_font_h <= size.1 as i32 {
                        match caret.shape {
                            CaretShape::Bar => {
                                // Draw a vertical bar on the left edge of the character cell
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
        //let start = std::time::Instant::now();
        let res = self.internal_draw(state, _cursor, _bounds);
        //println!("CRTShaderProgram::draw took {:?}", start.elapsed());
        res
    }

    fn update(&self, state: &mut CRTShaderState, event: &iced::Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<iced::widget::Action<Message>> {
        let res = self.internal_update(state, event, bounds, cursor);
        res
    }

    fn mouse_interaction(&self, _state: &Self::State, bounds: Rectangle, cursor: mouse::Cursor) -> mouse::Interaction {
        // Only show custom cursors when mouse is over the widget
        if !cursor.is_over(bounds) {
            return mouse::Interaction::default();
        }

        // Let the application (icy_term, icy_view, etc.) control the cursor
        self.term.cursor_icon.read().unwrap_or_default()
    }
}
