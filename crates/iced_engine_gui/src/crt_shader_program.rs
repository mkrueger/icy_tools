use crate::{CRTShaderState, Message, MonitorSettings, RenderUnicodeOptions, Terminal, TerminalShader, render_unicode_to_rgba};
use iced::widget::shader;
use iced::{Rectangle, mouse};
use icy_engine::{Caret, CaretShape, KeyModifiers, MouseButton, MouseEvent, MouseEventType, MouseState, Position};

// Program wrapper that renders the terminal and creates the shader
pub struct CRTShaderProgram<'a> {
    pub term: &'a Terminal,
    pub monitor_settings: MonitorSettings,
}

impl<'a> CRTShaderProgram<'a> {
    pub fn new(term: &'a Terminal, monitor_settings: MonitorSettings) -> Self {
        Self { term, monitor_settings }
    }

    fn mark_content_dirty(state: &CRTShaderState) {
        *state.content_dirty.lock() = true;
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

        // Size change triggers full content redraw
        if let Ok(screen) = self.term.screen.try_lock() {
            if let Some(font) = screen.get_font(0) {
                let font_w = font.size().width as u32;
                let font_h = font.size().height as u32;
                let current_size = (screen.get_width() as u32 * font_w, screen.get_height() as u32 * font_h);
                if state.last_rendered_size != Some(current_size) {
                    state.last_rendered_size = Some(current_size);
                    Self::mark_content_dirty(state);
                }
            }
        }

        // Track modifier keys
        if let iced::Event::Keyboard(kbd_event) = event {
            match kbd_event {
                iced::keyboard::Event::ModifiersChanged(mods) => {
                    state.alt_pressed = mods.alt();
                    state.ctrl_pressed = mods.command();
                    state.shift_pressed = mods.shift();
                }
                _ => {}
            }
        }

        // Handle mouse events
        // Handle mouse events
        if let iced::Event::Mouse(mouse_event) = event {
            // Check if mouse tracking is enabled
            let mouse_state = if let Ok(screen) = self.term.screen.lock() {
                screen.terminal_state().mouse_state.clone()
            } else {
                MouseState::default()
            };

            let mouse_tracking_enabled = mouse_state.tracking_enabled();
            match mouse_event {
                mouse::Event::CursorMoved { .. } => {
                    if let Some(position) = cursor.position() {
                        let cell_pos = map_mouse_to_cell(self.term, &self.monitor_settings, bounds, position.x, position.y);
                        let xy_pos = map_mouse_to_xy(self.term, &self.monitor_settings, bounds, position.x, position.y);
                        state.hovered_cell = cell_pos;

                        // Handle RIP field hovering
                        // Convert cell position to RIP coordinates (640x350)
                        if let Some(rip_pos) = xy_pos {
                            if let Ok(screen) = self.term.screen.try_lock() {
                                // Convert cell position to RIP pixel coordinates
                                // RIP uses 640x350 coordinate system

                                // Check if we're hovering over a RIP field
                                let mut found_field = None;
                                for mouse_field in screen.mouse_fields() {
                                    if !mouse_field.style.is_mouse_button() {
                                        continue;
                                    }
                                    if mouse_field.contains(rip_pos.x, rip_pos.y) {
                                        // Check if this field contains a previously found field
                                        // (handle nested fields by preferring innermost)
                                        if let Some(old_field) = &found_field {
                                            if mouse_field.contains_field(old_field) {
                                                found_field = Some(mouse_field.clone());
                                            }
                                        } else {
                                            // First matching field found
                                            found_field = Some(mouse_field.clone());
                                        }
                                    }
                                }

                                if state.hovered_rip_field != found_field {
                                    state.hovered_rip_field = found_field;
                                }
                            }
                        } else {
                            // Not hovering over terminal
                            if state.hovered_rip_field.is_some() {
                                state.hovered_rip_field = None;
                            }
                        }

                        // Send mouse motion event if mouse tracking is enabled
                        if mouse_tracking_enabled {
                            if let Some(cell) = cell_pos {
                                // Check if we should report motion events based on the mouse mode
                                let should_report_motion = match mouse_state.mouse_mode {
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
                                        mouse_state: mouse_state.clone(),
                                        event_type: MouseEventType::Motion,
                                        position: cell,
                                        button,
                                        modifiers,
                                    };

                                    // Send motion event immediately
                                    return Some(iced::widget::Action::publish(Message::SendMouseEvent(mouse_event)));
                                }
                            }
                        }

                        // Handle dragging for selection (even when mouse tracking is enabled)
                        if state.dragging {
                            if let Some(cell) = cell_pos {
                                if state.last_drag_position != Some(cell) {
                                    state.last_drag_position = Some(cell);
                                    if let Ok(mut edit_state) = self.term.screen.try_lock() {
                                        // Update selection
                                        if let Some(mut sel) = edit_state.get_selection().clone() {
                                            if !sel.locked {
                                                sel.lead = cell;
                                                sel.shape = if state.alt_pressed {
                                                    icy_engine::Shape::Rectangle
                                                } else {
                                                    icy_engine::Shape::Lines
                                                };
                                                let _ = edit_state.set_selection(sel);
                                            }
                                        }
                                    }
                                }
                            }
                        } else {
                            // Check hyperlinks only when not dragging
                            if let Some(cell) = cell_pos {
                                if let Ok(screen) = self.term.screen.try_lock() {
                                    // Check hyperlinks
                                    let mut found_link: Option<String> = None;
                                    for hyperlink in screen.hyperlinks() {
                                        if screen.is_position_in_range(cell, hyperlink.position, hyperlink.length) {
                                            found_link = Some(hyperlink.get_url(&**screen));
                                            break;
                                        }
                                    }

                                    if state.hovered_link != found_link {
                                        state.hovered_link = found_link;
                                    }
                                }
                            } else {
                                if state.hovered_link.is_some() {
                                    state.hovered_link = None;
                                }
                            }
                        }
                    }
                }

                mouse::Event::ButtonPressed(button) => {
                    if let Some(position) = cursor.position() {
                        if let Some(cell) = map_mouse_to_cell(self.term, &self.monitor_settings, bounds, position.x, position.y) {
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

                            // Send mouse press event if mouse tracking is enabled
                            if mouse_tracking_enabled {
                                let modifiers = KeyModifiers {
                                    shift: state.shift_pressed,
                                    ctrl: state.ctrl_pressed,
                                    alt: state.alt_pressed,
                                    meta: false,
                                };

                                let mouse_event = MouseEvent {
                                    mouse_state: mouse_state.clone(),
                                    event_type: MouseEventType::Press,
                                    position: cell,
                                    button: mouse_button,
                                    modifiers,
                                };

                                // Send the mouse event, but continue to handle selection
                                // Note: We don't return here to allow selection to work
                                let _ = Some(iced::widget::Action::publish(Message::SendMouseEvent(mouse_event)));
                            }

                            // Handle selection regardless of mouse tracking
                            if matches!(button, mouse::Button::Left) {
                                // Check if clicking on a hyperlink
                                if let Some(url) = &state.hovered_link {
                                    return Some(iced::widget::Action::publish(Message::OpenLink(url.clone())));
                                } else {
                                    // Start selection
                                    if let Ok(mut screen) = self.term.screen.try_lock() {
                                        // Clear existing selection unless shift is held
                                        if !state.shift_pressed {
                                            let _ = screen.clear_selection();
                                        }

                                        // Create new selection
                                        let mut sel = icy_engine::Selection::new(cell);
                                        sel.shape = if state.alt_pressed {
                                            icy_engine::Shape::Rectangle
                                        } else {
                                            icy_engine::Shape::Lines
                                        };
                                        sel.locked = false;
                                        let _ = screen.set_selection(sel);

                                        state.dragging = true;
                                        state.drag_anchor = Some(cell);
                                        state.last_drag_position = Some(cell);
                                    }
                                }
                            } else if matches!(button, mouse::Button::Middle) {
                                // Middle click: copy if selection exists, paste if no selection
                                if let Ok(screen) = self.term.screen.try_lock() {
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
                            // Clicked outside terminal area - clear selection
                            if let Ok(mut screen) = self.term.screen.try_lock() {
                                let _ = screen.clear_selection();
                            }
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
                    if mouse_tracking_enabled {
                        if let Some(position) = cursor.position() {
                            if let Some(cell) = map_mouse_to_cell(self.term, &self.monitor_settings, bounds, position.x, position.y) {
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
                                    mouse_state: mouse_state.clone(),
                                    event_type: MouseEventType::Release,
                                    position: cell,
                                    button: mouse_button,
                                    modifiers,
                                };

                                return Some(iced::widget::Action::publish(Message::SendMouseEvent(mouse_event)));
                            }
                        }
                    } else if matches!(button, mouse::Button::Left) && state.dragging {
                        // Handle selection release when not in mouse tracking mode
                        state.dragging = false;
                        state.shift_pressed_during_selection = state.shift_pressed;

                        // Lock the selection
                        if let Ok(mut screen) = self.term.screen.try_lock() {
                            if let Some(mut sel) = screen.get_selection().clone() {
                                sel.locked = true;
                                let _ = screen.set_selection(sel);
                            }
                        }

                        state.drag_anchor = None;
                        state.last_drag_position = None;
                    }
                }

                mouse::Event::WheelScrolled { delta } => {
                    if mouse_tracking_enabled {
                        // Send wheel events as button press events
                        if let Some(position) = cursor.position() {
                            if let Some(cell) = map_mouse_to_cell(self.term, &self.monitor_settings, bounds, position.x, position.y) {
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
                                        mouse_state: mouse_state.clone(),
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
                    } else {
                        // Normal scrolling when mouse tracking is disabled
                        match delta {
                            mouse::ScrollDelta::Lines { y, .. } => {
                                let lines = -(*y as i32); // Negative for natural scrolling
                                return Some(iced::widget::Action::publish(Message::Scroll(lines)));
                            }
                            mouse::ScrollDelta::Pixels { y, .. } => {
                                let lines = -((*y / 20.0) as i32); // Convert pixels to lines
                                if lines != 0 {
                                    return Some(iced::widget::Action::publish(Message::Scroll(lines)));
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn internal_draw(&self, state: &CRTShaderState, _cursor: mouse::Cursor, _bounds: Rectangle) -> TerminalShader {
        // Fast path: reuse cached buffer if content not dirty; only reapply caret/blink overlays.
        let mut rgba_data: Vec<u8>;
        let size: (u32, u32);
        let mut font_w = 0usize;
        let mut font_h = 0usize;
        let scan_lines;
        // Check if we need to re-render based on buffer version and blink state
        let mut needs_full_render = false;

        if let Ok(screen) = self.term.screen.lock() {
            scan_lines = screen.scan_lines();
            if let Some(font) = screen.get_font(0) {
                font_w = font.size().width as usize;
                font_h = font.size().height as usize;
            }

            let current_buffer_version = screen.get_version();
            let blink_on = state.character_blink.is_on();

            // Check if buffer version changed (content modified)
            let last_version = *state.last_buffer_version.lock();
            if current_buffer_version != last_version {
                needs_full_render = true;
                *state.last_buffer_version.lock() = current_buffer_version;
            }

            // Check if selection changed (affects highlighting)
            let selection = screen.get_selection();
            let sel_anchor = selection.as_ref().map(|s| s.anchor);
            let sel_lead = selection.as_ref().map(|s| s.lead);
            let sel_locked = selection.as_ref().map(|s| s.locked).unwrap_or(false);
            {
                let mut last_sel = state.last_selection_state.lock();
                if last_sel.0 != sel_anchor || last_sel.1 != sel_lead || last_sel.2 != sel_locked {
                    needs_full_render = true;
                    *last_sel = (sel_anchor, sel_lead, sel_locked);
                }
            }

            let mut cached_size_guard = state.cached_size.lock();
            let mut cached_font_guard = state.cached_font_wh.lock();

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
                    let rect = icy_engine::Rectangle {
                        start: icy_engine::Position::new(0, 0),
                        size: icy_engine::Size::new(screen.get_width(), screen.get_height()),
                    };
                    let render_on = screen.render_to_rgba(&icy_engine::RenderOptions {
                        rect: rect.into(),
                        blink_on: true,
                        selection,
                        selection_fg: Some(fg_sel.clone()),
                        selection_bg: Some(bg_sel.clone()),
                    });
                    let render_off = screen.render_to_rgba(&icy_engine::RenderOptions {
                        rect: rect.into(),
                        blink_on: false,
                        selection,
                        selection_fg: Some(fg_sel.clone()),
                        selection_bg: Some(bg_sel.clone()),
                    });
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
                *cached_size_guard = size;
                *cached_font_guard = (font_w, font_h);

                // Clear buffer dirty flag after rendering
                screen.clear_dirty();
            } else {
                // Reuse cached images - just pick the right one based on blink state
                size = *cached_size_guard;
                let (cfw, cfh) = *cached_font_guard;
                if cfw != 0 && cfh != 0 {
                    font_w = cfw;
                    font_h = cfh;
                }
                rgba_data = if blink_on {
                    state.cached_rgba_blink_on.lock().clone()
                } else {
                    state.cached_rgba_blink_off.lock().clone()
                };
            }

            // Always overlay the caret if visible and blinking is on
            // Caret is just an inversion overlay, no need to track changes
            if state.caret_blink.is_on() {
                self.draw_caret(&screen.caret(), state, &mut rgba_data, size, font_w, font_h, scan_lines);
            }
        } else {
            // Fallback minimal buffer
            size = (640, 400);
            rgba_data = vec![0; size.0 as usize * size.1 as usize * 4];
        }

        if rgba_data.len() != (size.0 as usize * size.1 as usize * 4) {
            panic!(
                "RGBA data size mismatch (expected {}, got {})",
                size.0 as usize * size.1 as usize * 4,
                rgba_data.len()
            );
        }

        TerminalShader {
            terminal_rgba: rgba_data,
            terminal_size: size,
            monitor_settings: self.monitor_settings.clone(),
            instance_id: state.instance_id,
        }
    }

    pub fn draw_caret(&self, caret: &Caret, state: &CRTShaderState, rgba_data: &mut Vec<u8>, size: (u32, u32), font_w: usize, font_h: usize, scan_lines: bool) {
        // Check both the caret's is_blinking property and the blink timer state
        let should_draw = caret.visible && (!caret.blinking || state.caret_blink.is_on());

        if should_draw && self.term.has_focus {
            let caret_pos = caret.position();
            if font_w > 0 && font_h > 0 && size.0 > 0 && size.1 > 0 {
                let line_bytes = (size.0 as usize) * 4;
                let cell_x = caret_pos.x;
                let cell_y = caret_pos.y;

                if cell_x >= 0 && cell_y >= 0 {
                    let (px_x, px_y) = if caret.use_pixel_positioning {
                        (caret_pos.x as usize, caret_pos.y as usize * if scan_lines { 2 } else { 1 })
                    } else {
                        (
                            (cell_x as usize) * font_w,
                            // In scanline mode, y-position and height are doubled
                            if scan_lines {
                                (cell_y as usize) * font_h * 2
                            } else {
                                (cell_y as usize) * font_h
                            },
                        )
                    };
                    let actual_font_h = if scan_lines { font_h * 2 } else { font_h };
                    if px_x + font_w <= size.0 as usize && px_y + actual_font_h <= size.1 as usize {
                        match caret.shape {
                            CaretShape::Bar => {
                                // Draw a vertical bar on the left edge of the character cell
                                let bar_width = 2.min(font_w); // 2 pixels wide or font width if smaller
                                for row in 0..actual_font_h {
                                    let row_offset = (px_y + row) * line_bytes + px_x * 4;
                                    let slice = &mut rgba_data[row_offset..row_offset + bar_width * 4];
                                    for p in slice.chunks_exact_mut(4) {
                                        p[0] = 255 - p[0];
                                        p[1] = 255 - p[1];
                                        p[2] = 255 - p[2];
                                    }
                                }
                            }
                            CaretShape::Block => {
                                for row in 0..actual_font_h {
                                    let row_offset = (px_y + row) * line_bytes + px_x * 4;
                                    let slice = &mut rgba_data[row_offset..row_offset + font_w * 4];
                                    for p in slice.chunks_exact_mut(4) {
                                        p[0] = 255 - p[0];
                                        p[1] = 255 - p[1];
                                        p[2] = 255 - p[2];
                                    }
                                }
                            }
                            CaretShape::Underline => {
                                let start_row = actual_font_h.saturating_sub(2);
                                for row in start_row..actual_font_h {
                                    let row_offset = (px_y + row) * line_bytes + px_x * 4;
                                    let slice = &mut rgba_data[row_offset..row_offset + font_w * 4];
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
            mouse::Interaction::Text
        } else {
            mouse::Interaction::default()
        }
    }
}

fn map_mouse_to_xy(
    term: &Terminal,
    monitor: &MonitorSettings,
    bounds: Rectangle,
    mx: f32, // mouse x in logical space
    my: f32, // mouse y in logical space
) -> Option<Position> {
    // 3. Lock edit state & obtain font + buffer size (already in pixel units)
    let screen = term.screen.try_lock().ok()?;
    let font = screen.get_font(0)?;
    let font_w = font.size().width as f32;
    let font_h = font.size().height as f32;

    let scale_factor = crate::get_scale_factor();
    let bounds = bounds * scale_factor;
    let mx = mx * scale_factor;
    let my = my * scale_factor;
    if font_w <= 0.0 || font_h <= 0.0 {
        return None;
    }

    let resolution = screen.get_resolution();
    let resolution_x = resolution.width as f32;
    let mut resolution_y = resolution.height as f32;

    // In scanline mode, the output height is doubled
    if screen.scan_lines() {
        resolution_y *= 2.0;
    }
    // 4. Aspect-fit scale in PHYSICAL space (match render())
    let avail_w = bounds.width.max(1.0);
    let avail_h = bounds.height.max(1.0);
    let uniform_scale = (avail_w / resolution_x).min(avail_h / resolution_y);

    let use_pp = monitor.use_pixel_perfect_scaling;
    let display_scale = if use_pp { uniform_scale.floor().max(1.0) } else { uniform_scale };

    let scaled_w = resolution_x * display_scale;
    let scaled_h = resolution_y * display_scale;

    // 5. Center terminal inside physical bounds (same as render())
    let offset_x = bounds.x + (avail_w - scaled_w) / 2.0;
    let offset_y = bounds.y + (avail_h - scaled_h) / 2.0;

    // 6. Pixel-perfect rounding (only position & size used for viewport clipping)
    let (vp_x, vp_y, vp_w, vp_h) = if use_pp {
        (offset_x.round(), offset_y.round(), scaled_w.round(), scaled_h.round())
    } else {
        (offset_x, offset_y, scaled_w, scaled_h)
    };

    // 7. Hit test in physical viewport
    if mx < vp_x || my < vp_y || mx >= vp_x + vp_w || my >= vp_y + vp_h {
        return None;
    }

    // 8. Undo scaling using display_scale, not viewport width ratios
    let local_px_x = (mx - vp_x) / display_scale;
    let local_px_y = (my - vp_y) / display_scale;

    Some(Position::new(local_px_x as i32, local_px_y as i32))
}

fn map_mouse_to_cell(
    term: &Terminal,
    monitor: &MonitorSettings,
    bounds: Rectangle,
    mx: f32, // mouse x in logical space
    my: f32, // mouse y in logical space
) -> Option<Position> {
    // 3. Lock edit state & obtain font + buffer size (already in pixel units)
    let screen = term.screen.try_lock().ok()?;
    let font = screen.get_font(0)?;
    let font_w = font.size().width as f32;
    let font_h = font.size().height as f32;

    let scale_factor = crate::get_scale_factor();
    let bounds = bounds * scale_factor;
    let mx = mx * scale_factor;
    let my = my * scale_factor;
    if font_w <= 0.0 || font_h <= 0.0 {
        return None;
    }

    let term_px_w = screen.get_width() as f32 * font_w;
    let mut term_px_h = screen.get_height() as f32 * font_h;

    // In scanline mode, each line is doubled
    if screen.scan_lines() {
        term_px_h *= 2.0;
    }

    if term_px_w <= 0.0 || term_px_h <= 0.0 {
        return None;
    }

    // 4. Aspect-fit scale in PHYSICAL space (match render())
    let avail_w = bounds.width.max(1.0);
    let avail_h = bounds.height.max(1.0);
    let uniform_scale = (avail_w / term_px_w).min(avail_h / term_px_h);

    let use_pp = monitor.use_pixel_perfect_scaling;
    let display_scale = if use_pp { uniform_scale.floor().max(1.0) } else { uniform_scale };

    let scaled_w = term_px_w * display_scale;
    let scaled_h = term_px_h * display_scale;

    // 5. Center terminal inside physical bounds (same as render())
    let offset_x = bounds.x + (avail_w - scaled_w) / 2.0;
    let offset_y = bounds.y + (avail_h - scaled_h) / 2.0;

    // 6. Pixel-perfect rounding (only position & size used for viewport clipping)
    let (vp_x, vp_y, vp_w, vp_h) = if use_pp {
        (offset_x.round(), offset_y.round(), scaled_w.round(), scaled_h.round())
    } else {
        (offset_x, offset_y, scaled_w, scaled_h)
    };

    // 7. Hit test in physical viewport
    if mx < vp_x || my < vp_y || mx >= vp_x + vp_w || my >= vp_y + vp_h {
        return None;
    }

    // 8. Undo scaling using display_scale, not viewport width ratios
    let local_px_x = (mx - vp_x) / display_scale;
    let mut local_px_y = (my - vp_y) / display_scale;

    // In scanline mode, we need to divide y by 2 to get the actual cell position
    let actual_font_h = if screen.scan_lines() {
        local_px_y /= 2.0;
        font_h
    } else {
        font_h
    };

    // 9. Convert to cell indices
    let cx = (local_px_x / font_w).floor() as i32;
    let cy = (local_px_y / actual_font_h).floor() as i32;

    if cx < 0 || cy < 0 || cx >= screen.get_width() as i32 || cy >= screen.get_height() as i32 {
        return None;
    }

    Some(Position::new(cx, cy))
}
