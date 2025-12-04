use crate::{CRTShaderState, Message, MonitorSettings, RenderUnicodeOptions, Terminal, TerminalShader, render_unicode_to_rgba};
use iced::widget::shader;
use iced::{Rectangle, mouse};
use icy_engine::{Caret, CaretShape, KeyModifiers, MouseButton, MouseEvent, MouseEventType};

// Program wrapper that renders the terminal and creates the shader
pub struct CRTShaderProgram<'a> {
    pub term: &'a Terminal,
    pub monitor_settings: MonitorSettings,
}

impl<'a> CRTShaderProgram<'a> {
    pub fn new(term: &'a Terminal, monitor_settings: MonitorSettings) -> Self {
        Self { term, monitor_settings }
    }

    pub fn internal_update(
        &self,
        state: &mut CRTShaderState,
        event: &iced::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<iced::widget::Action<Message>> {
        let now = crate::Blink::now_ms();

        state.caret_blink.update(now);
        state.character_blink.update(now);

        // Track modifier keys - store both locally and globally
        // Global storage survives widget state resets
        if let iced::Event::Keyboard(kbd_event) = event {
            match kbd_event {
                iced::keyboard::Event::ModifiersChanged(mods) => {
                    let ctrl = mods.control();
                    let alt = mods.alt();
                    let shift = mods.shift();
                    state.alt_pressed = alt;
                    state.ctrl_pressed = ctrl;
                    state.shift_pressed = shift;
                    // Also store globally for cross-widget access
                    crate::set_global_modifiers(ctrl, alt, shift);
                }
                _ => {}
            }
        }

        if !cursor.is_over(bounds) {
            return None;
        }

        // Handle mouse events
        if let iced::Event::Mouse(mouse_event) = event {
            // Use cached mouse state from last draw (avoids extra lock)
            let mouse_state = state.cached_mouse_state.lock().clone();
            let mouse_tracking_enabled = mouse_state.as_ref().map(|ms| ms.tracking_enabled()).unwrap_or(false);

            // Read viewport and render_info for mouse coordinate calculations
            let viewport = self.term.viewport.read();
            let render_info = self.term.render_info.read();

            match mouse_event {
                mouse::Event::CursorMoved { .. } => {
                    if let Some(position) = cursor.position() {
                        // Calculate cell and xy positions using shared RenderInfo from shader
                        let cell_pos = state.map_mouse_to_cell(&render_info, position.x, position.y, &viewport);
                        let xy_pos = state.map_mouse_to_xy(&render_info, position.x, position.y);

                        // Only update if position actually changed
                        if state.hovered_cell != cell_pos {
                            state.hovered_cell = cell_pos;
                        }

                        // Handle RIP field hovering - needs screen lock for mouse_fields
                        if let Some(rip_pos) = xy_pos {
                            if let Some(screen) = self.term.screen.try_lock() {
                                let mouse_fields = screen.mouse_fields();
                                if !mouse_fields.is_empty() {
                                    let mut found_field = None;
                                    for mouse_field in mouse_fields {
                                        if !mouse_field.style.is_mouse_button() {
                                            continue;
                                        }
                                        if mouse_field.contains(rip_pos.x, rip_pos.y) {
                                            if let Some(old_field) = &found_field {
                                                if mouse_field.contains_field(old_field) {
                                                    found_field = Some(mouse_field.clone());
                                                }
                                            } else {
                                                found_field = Some(mouse_field.clone());
                                            }
                                        }
                                    }

                                    if state.hovered_rip_field != found_field {
                                        state.hovered_rip_field = found_field;
                                    }
                                } else if state.hovered_rip_field.is_some() {
                                    state.hovered_rip_field = None;
                                }
                            }
                        } else if state.hovered_rip_field.is_some() {
                            state.hovered_rip_field = None;
                        }

                        // Send mouse motion event if mouse tracking is enabled
                        if let Some(ref ms) = mouse_state {
                            if mouse_tracking_enabled {
                                if let Some(cell) = cell_pos {
                                    let should_report_motion = match ms.mouse_mode {
                                        icy_engine::MouseMode::ButtonEvents => state.dragging,
                                        icy_engine::MouseMode::AnyEvents => true,
                                        _ => false,
                                    };

                                    if should_report_motion {
                                        let modifiers = KeyModifiers {
                                            shift: state.shift_pressed,
                                            ctrl: state.ctrl_pressed,
                                            alt: state.alt_pressed,
                                            meta: false,
                                        };

                                        let button = if state.dragging { MouseButton::Left } else { MouseButton::None };

                                        let mouse_event = MouseEvent {
                                            mouse_state: ms.clone(),
                                            event_type: MouseEventType::Motion,
                                            position: cell,
                                            button,
                                            modifiers,
                                        };

                                        return Some(iced::widget::Action::publish(Message::SendMouseEvent(mouse_event)));
                                    }
                                }
                            }
                        }

                        // Check hyperlinks only when not dragging
                        if !state.dragging {
                            if let Some(cell) = cell_pos {
                                if let Some(screen) = self.term.screen.try_lock() {
                                    let hyperlinks = screen.hyperlinks();
                                    if !hyperlinks.is_empty() {
                                        let mut found_link: Option<String> = None;
                                        for hyperlink in hyperlinks {
                                            if screen.is_position_in_range(cell, hyperlink.position, hyperlink.length) {
                                                found_link = Some(hyperlink.get_url(&**screen));
                                                break;
                                            }
                                        }

                                        if state.hovered_link != found_link {
                                            state.hovered_link = found_link;
                                        }
                                    } else if state.hovered_link.is_some() {
                                        state.hovered_link = None;
                                    }
                                }
                            } else if state.hovered_link.is_some() {
                                state.hovered_link = None;
                            }
                        }

                        // Handle dragging for selection - send message instead of direct modification
                        if state.dragging {
                            if let Some(cell) = state.hovered_cell {
                                if state.last_drag_position != Some(cell) {
                                    state.last_drag_position = Some(cell);
                                    return Some(iced::widget::Action::publish(Message::UpdateSelection(cell)));
                                }
                            }
                        }
                    }
                }

                mouse::Event::ButtonPressed(button) => {
                    if let Some(position) = cursor.position() {
                        if let Some(cell) = state.map_mouse_to_cell(&render_info, position.x, position.y, &viewport) {
                            // Convert iced mouse button to our MouseButton type
                            let mouse_button = match button {
                                mouse::Button::Left => MouseButton::Left,
                                mouse::Button::Middle => MouseButton::Middle,
                                mouse::Button::Right => MouseButton::Right,
                                _ => return None,
                            };

                            if let Some(mouse_field) = &state.hovered_rip_field {
                                if let Some(cmd) = &mouse_field.host_command {
                                    let clear_rip_screen = mouse_field.style.reset_screen_after_click();
                                    return Some(iced::widget::Action::publish(Message::RipCommand(clear_rip_screen, cmd.clone())));
                                }
                            }

                            // clicking on links should always have prio.
                            if matches!(button, mouse::Button::Left) {
                                // Check if clicking on a hyperlink
                                if let Some(url) = &state.hovered_link {
                                    return Some(iced::widget::Action::publish(Message::OpenLink(url.clone())));
                                }
                            }

                            // Send mouse press event if mouse tracking is enabled
                            if let Some(ref ms) = mouse_state {
                                if mouse_tracking_enabled {
                                    let modifiers = KeyModifiers {
                                        shift: state.shift_pressed,
                                        ctrl: state.ctrl_pressed,
                                        alt: state.alt_pressed,
                                        meta: false,
                                    };

                                    let mouse_event = MouseEvent {
                                        mouse_state: ms.clone(),
                                        event_type: MouseEventType::Press,
                                        position: cell,
                                        button: mouse_button,
                                        modifiers,
                                    };

                                    // When mouse tracking is enabled, send the event and skip local selection handling
                                    return Some(iced::widget::Action::publish(Message::SendMouseEvent(mouse_event)));
                                }
                            }

                            // Handle selection only when mouse tracking is NOT enabled
                            if matches!(button, mouse::Button::Left) {
                                // Start selection - send message instead of direct modification
                                // Clear existing selection unless shift is held
                                if !state.shift_pressed {
                                    // Create new selection
                                    let mut sel = icy_engine::Selection::new(cell);
                                    sel.shape = if state.alt_pressed {
                                        icy_engine::Shape::Rectangle
                                    } else {
                                        icy_engine::Shape::Lines
                                    };
                                    sel.locked = false;

                                    state.dragging = true;
                                    state.drag_anchor = Some(cell);
                                    state.last_drag_position = Some(cell);

                                    return Some(iced::widget::Action::publish(Message::StartSelection(sel)));
                                } else {
                                    // Shift is held - just update drag state
                                    state.dragging = true;
                                    state.drag_anchor = Some(cell);
                                    state.last_drag_position = Some(cell);
                                }
                            } else if matches!(button, mouse::Button::Middle) {
                                // Middle click: copy if selection exists, paste if no selection
                                if let Some(screen) = self.term.screen.try_lock() {
                                    if screen.get_selection().is_some() {
                                        // Has selection - copy it
                                        return Some(iced::widget::Action::publish(Message::Copy));
                                    } else {
                                        // No selection - paste
                                        return Some(iced::widget::Action::publish(Message::Paste));
                                    }
                                }
                            }
                            // Note: Removed middle click paste since Message::Paste doesn't exist
                        } else {
                            // Clicked outside terminal area - send clear selection message
                            return Some(iced::widget::Action::publish(Message::ClearSelection));
                        }
                    }
                }

                mouse::Event::ButtonReleased(button) => {
                    // Convert iced mouse button to our MouseButton type
                    let mouse_button = match button {
                        mouse::Button::Left => MouseButton::Left,
                        mouse::Button::Middle => MouseButton::Middle,
                        mouse::Button::Right => MouseButton::Right,
                        _ => return None, // Skip other buttons for now
                    };

                    // Send mouse release event if mouse tracking is enabled
                    if let Some(ref ms) = mouse_state {
                        if mouse_tracking_enabled {
                            if let Some(position) = cursor.position() {
                                if let Some(cell) = state.map_mouse_to_cell(&render_info, position.x, position.y, &viewport) {
                                    let modifiers = KeyModifiers {
                                        shift: state.shift_pressed,
                                        ctrl: state.ctrl_pressed,
                                        alt: state.alt_pressed,
                                        meta: false,
                                    };

                                    if matches!(button, mouse::Button::Left) {
                                        state.dragging = false;
                                    }

                                    let mouse_event = MouseEvent {
                                        mouse_state: ms.clone(),
                                        event_type: MouseEventType::Release,
                                        position: cell,
                                        button: mouse_button,
                                        modifiers,
                                    };

                                    return Some(iced::widget::Action::publish(Message::SendMouseEvent(mouse_event)));
                                }
                            }
                        }
                    }

                    if !mouse_tracking_enabled && matches!(button, mouse::Button::Left) && state.dragging {
                        // Handle selection release when not in mouse tracking mode
                        state.dragging = false;
                        state.shift_pressed_during_selection = state.shift_pressed;

                        state.drag_anchor = None;
                        state.last_drag_position = None;

                        // Send message to lock the selection
                        return Some(iced::widget::Action::publish(Message::EndSelection));
                    }
                }

                mouse::Event::WheelScrolled { delta } => {
                    // Check if Ctrl is held for zooming - use global state as it survives widget resets
                    let ctrl_pressed = crate::is_ctrl_pressed();
                    if ctrl_pressed {
                        let zoom_delta = match delta {
                            mouse::ScrollDelta::Lines { y, .. } => *y,
                            mouse::ScrollDelta::Pixels { y, .. } => *y / 100.0,
                        };
                        if zoom_delta != 0.0 {
                            return Some(iced::widget::Action::publish(Message::ZoomWheel(zoom_delta)));
                        }
                    } else if mouse_tracking_enabled {
                        // Send wheel events as button press events
                        if let Some(ref ms) = mouse_state {
                            if let Some(position) = cursor.position() {
                                if let Some(cell) = state.map_mouse_to_cell(&render_info, position.x, position.y, &viewport) {
                                    let lines = match delta {
                                        mouse::ScrollDelta::Lines { y, .. } => *y,
                                        mouse::ScrollDelta::Pixels { y, .. } => *y / 20.0,
                                    };

                                    if lines != 0.0 {
                                        let button = if lines > 0.0 { MouseButton::WheelUp } else { MouseButton::WheelDown };

                                        let modifiers = KeyModifiers {
                                            shift: state.shift_pressed,
                                            ctrl: state.ctrl_pressed,
                                            alt: state.alt_pressed,
                                            meta: false,
                                        };

                                        let mouse_event = MouseEvent {
                                            mouse_state: ms.clone(),
                                            event_type: MouseEventType::Press,
                                            position: cell,
                                            button,
                                            modifiers,
                                        };

                                        // Wheel events are sent as press events
                                        return Some(iced::widget::Action::publish(Message::SendMouseEvent(mouse_event)));
                                    }
                                }
                            }
                        }
                    } else {
                        // Viewport-based scrolling when mouse tracking is disabled
                        match delta {
                            mouse::ScrollDelta::Lines { x, y, .. } => {
                                // Scroll by pixel amount based on line height
                                let scroll_y = *y * 20.0; // ~20 pixels per line
                                let scroll_x = *x * 10.0; // ~10 pixels per column

                                return Some(iced::widget::Action::publish(Message::ScrollViewport(-scroll_x, -scroll_y)));
                            }
                            mouse::ScrollDelta::Pixels { x, y, .. } => {
                                // Direct pixel scrolling
                                return Some(iced::widget::Action::publish(Message::ScrollViewport(-*x, -*y)));
                            }
                        }
                    }
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
            if let Some(font) = screen.get_font(0) {
                font_w = font.size().width as usize;
                font_h = font.size().height as usize;
            }

            // Cache screen info and mouse state for use in internal_update (avoids extra locks there)
            state.update_cached_screen_info(&**screen);
            *state.cached_mouse_state.lock() = Some(screen.terminal_state().mouse_state.clone());

            let current_buffer_version = screen.get_version();
            let blink_on = state.character_blink.is_on();

            // Check if buffer version or selection changed (need single lock for cached_screen_info)
            let selection = screen.get_selection();
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
                let (fg_sel, bg_sel) = screen.buffer_type().get_selection_colors();

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
                    let resolution = screen.get_resolution();

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

                    // Store computed visible dimensions for scrollbar calculations
                    self.term.bounds_height.store(bounds.height as u32, std::sync::atomic::Ordering::Relaxed);
                    self.term.bounds_width.store(bounds.width as u32, std::sync::atomic::Ordering::Relaxed);

                    // Create viewport region - scroll_x/y are already in content coordinates
                    let viewport_region = icy_engine::Rectangle::from(
                        vp.scroll_x as i32,
                        vp.scroll_y as i32,
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

    fn mouse_interaction(&self, state: &Self::State, _bounds: Rectangle, _cursor: mouse::Cursor) -> mouse::Interaction {
        if state.hovered_link.is_some() || state.hovered_rip_field.is_some() {
            mouse::Interaction::Pointer
        } else if state.dragging {
            mouse::Interaction::Crosshair
        } else if state.hovered_cell.is_some() {
            // Only show text cursor for text mode screens (selection not yet supported for graphics)
            let info: parking_lot::lock_api::MutexGuard<'_, parking_lot::RawMutex, crate::CachedScreenInfo> = state.cached_screen_info.lock();
            if info.graphics_type == icy_engine::GraphicsType::Text {
                mouse::Interaction::Text
            } else {
                mouse::Interaction::default()
            }
        } else {
            mouse::Interaction::default()
        }
    }
}
