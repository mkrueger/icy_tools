use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use std::sync::{Mutex, atomic::AtomicU64};

use crate::{Blink, CRTShaderProgram, Message, MonitorSettings, RenderUnicodeOptions, Terminal, TerminalShader, UnicodeGlyphCache, render_unicode_to_rgba};
use iced::widget::shader;
use iced::{Element, Rectangle, mouse};
use icy_engine::ansi::mouse_event::{KeyModifiers, MouseButton, MouseEventType};
use icy_engine::{MouseState, Position, Selection};

pub static TERMINAL_SHADER_INSTANCE_COUNTER: AtomicU64 = AtomicU64::new(1);
pub static PENDING_INSTANCE_REMOVALS: Mutex<Vec<u64>> = Mutex::new(Vec::new());

pub struct CRTShaderState {
    pub caret_blink: crate::Blink,
    pub character_blink: crate::Blink,

    // Mouse/selection tracking
    dragging: bool,
    drag_anchor: Option<Position>,
    last_drag_position: Option<Position>,
    shift_pressed_during_selection: bool,

    // Modifier tracking
    alt_pressed: bool,
    shift_pressed: bool,
    ctrl_pressed: bool,

    // Hover tracking
    hovered_cell: Option<Position>,
    hovered_link: Option<String>,
    /// Track which RIP field is hovered (by index)
    hovered_rip_field: Option<usize>,

    last_rendered_size: Option<(u32, u32)>,
    instance_id: u64,

    unicode_glyph_cache: Arc<parking_lot::Mutex<Option<UnicodeGlyphCache>>>,
}

impl CRTShaderState {
    pub fn reset_caret(&mut self) {
        self.caret_blink.reset();
    }
}

impl Drop for CRTShaderState {
    fn drop(&mut self) {
        if let Ok(mut v) = PENDING_INSTANCE_REMOVALS.lock() {
            v.push(self.instance_id);
        }
    }
}

impl Default for CRTShaderState {
    fn default() -> Self {
        Self {
            caret_blink: Blink::new((1000.0 / 1.875) as u128 / 2),
            character_blink: Blink::new((1000.0 / 1.8) as u128),
            dragging: false,
            drag_anchor: None,
            last_drag_position: None,
            shift_pressed_during_selection: false,
            alt_pressed: false,
            shift_pressed: false,
            ctrl_pressed: false,
            hovered_cell: None,
            hovered_link: None,
            hovered_rip_field: None,
            last_rendered_size: None,
            instance_id: TERMINAL_SHADER_INSTANCE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            unicode_glyph_cache: Arc::new(parking_lot::Mutex::new(None)),
        }
    }
}

impl<'a> shader::Program<Message> for CRTShaderProgram<'a> {
    type State = CRTShaderState;
    type Primitive = TerminalShader;

    fn draw(&self, state: &Self::State, _cursor: mouse::Cursor, _bounds: Rectangle) -> Self::Primitive {
        let mut rgba_data = Vec::new();
        let mut size = (640, 400); // fallback
        let mut font_w = 0usize;
        let mut font_h = 0usize;

        if let Ok(screen) = self.term.screen.try_lock() {
            if let Some(font) = screen.get_font(0) {
                font_w = font.size.width as usize;
                font_h = font.size.height as usize;
            }

            let (fg_sel, bg_sel) = screen.buffer_type().get_selection_colors();

            if let Some((s, data)) = &self.term.picture_data {
                // Use cached rendering if available
                size = (s.width as u32, s.height as u32);
                rgba_data = data.clone();
            } else {
                if matches!(screen.buffer_type(), icy_engine::BufferType::Unicode) {
                    // Unicode path - use cached glyph cache with Arc<Mutex<>>
                    let (img_size, data) = render_unicode_to_rgba(
                        &*screen,
                        &RenderUnicodeOptions {
                            selection: screen.get_selection(),
                            selection_fg: Some(fg_sel),
                            selection_bg: Some(bg_sel),
                            blink_on: state.character_blink.is_on(),
                            font_px_size: Some(font_h as f32),
                            glyph_cache: state.unicode_glyph_cache.clone(), // Pass Arc clone
                        },
                    );
                    size = (img_size.width as u32, img_size.height as u32);
                    rgba_data = data;
                } else {
                    // Existing ANSI path
                    let rect = icy_engine::Rectangle {
                        start: icy_engine::Position::new(0, 0),
                        size: icy_engine::Size::new(screen.get_width(), screen.get_height()),
                    };
                    let (img_size, data) = screen.render_to_rgba(&icy_engine::RenderOptions {
                        rect: rect.into(),
                        blink_on: state.character_blink.is_on(),
                        selection: screen.get_selection(),
                        selection_fg: Some(fg_sel),
                        selection_bg: Some(bg_sel),
                    });
                    size = (img_size.width as u32, img_size.height as u32);
                    rgba_data = data;
                }
            }
        }

        // Caret overlay (shared)
        if let Ok(edit_state) = self.term.screen.try_lock() {
            self.draw_caret(&edit_state.caret(), state, &mut rgba_data, size, font_w, font_h);
        }

        TerminalShader {
            terminal_rgba: rgba_data,
            terminal_size: size,
            monitor_settings: self.monitor_settings.clone(),
            instance_id: state.instance_id,
        }
    }

    fn update(&self, state: &mut Self::State, event: &iced::Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<iced::widget::Action<Message>> {
        let mut needs_redraw = false;
        let now = crate::Blink::now_ms();

        // Update blink timers
        if state.caret_blink.update(now) {
            needs_redraw = true;
        }
        if state.character_blink.update(now) {
            needs_redraw = true;
        }

        // Track the actual rendered size to detect real changes
        // Only update if we successfully get the lock
        if let Ok(screen) = self.term.screen.try_lock() {
            if let Some(font) = screen.get_font(0) {
                let font_w = font.size.width as u32;
                let font_h = font.size.height as u32;
                let current_size = (screen.get_width() as u32 * font_w, screen.get_height() as u32 * font_h);

                // Only trigger redraw if size actually changed
                if state.last_rendered_size != Some(current_size) {
                    state.last_rendered_size = Some(current_size);
                    needs_redraw = true;
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

            let use_rip = !self.term.mouse_fields.is_empty();

            let mouse_tracking_enabled = mouse_state.tracking_enabled();
            match mouse_event {
                mouse::Event::CursorMoved { .. } => {
                    if let Some(position) = cursor.position() {
                        let cell_pos = map_mouse_to_cell(self.term, &self.monitor_settings, bounds, position.x, position.y);
                        state.hovered_cell = cell_pos;

                        // Handle RIP field hovering
                        if use_rip {
                            // Convert cell position to RIP coordinates (640x350)
                            if let Some(cell) = cell_pos {
                                if let Ok(screen) = self.term.screen.try_lock() {
                                    // Convert cell position to RIP pixel coordinates
                                    // RIP uses 640x350 coordinate system
                                    let cells_x = screen.get_width() as f32;
                                    let cells_y = screen.get_height() as f32;

                                    let rip_x = ((cell.x as f32 / cells_x) * 640.0) as i32;
                                    let rip_y = ((cell.y as f32 / cells_y) * 350.0) as i32;

                                    // Check if we're hovering over a RIP field
                                    let mut found_field = None;
                                    for (idx, mouse_field) in self.term.mouse_fields.iter().enumerate() {
                                        if !mouse_field.style.is_mouse_button() {
                                            continue;
                                        }

                                        if mouse_field.contains(rip_x, rip_y) {
                                            // Check if this field contains a previously found field
                                            // (handle nested fields by preferring innermost)
                                            if let Some(found_idx) = found_field {
                                                if mouse_field.contains_field(&self.term.mouse_fields[found_idx]) {
                                                    continue;
                                                }
                                            }
                                            found_field = Some(idx);
                                        }
                                    }

                                    if state.hovered_rip_field != found_field {
                                        state.hovered_rip_field = found_field;
                                        needs_redraw = true;
                                    }
                                }
                            } else {
                                // Not hovering over terminal
                                if state.hovered_rip_field.is_some() {
                                    state.hovered_rip_field = None;
                                    needs_redraw = true;
                                }
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

                                    let mouse_event = icy_engine::ansi::mouse_event::MouseEvent {
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
                                    needs_redraw = true;
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
                                            found_link = Some(hyperlink.get_url(&*screen));
                                            break;
                                        }
                                    }

                                    if state.hovered_link != found_link {
                                        state.hovered_link = found_link;
                                        needs_redraw = true;
                                    }
                                }
                            } else {
                                if state.hovered_link.is_some() {
                                    state.hovered_link = None;
                                    needs_redraw = true;
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

                            if let Some(mouse_field_idx) = state.hovered_rip_field {
                                if let Some(mouse_field) = &self.term.mouse_fields.get(mouse_field_idx) {
                                    if let Some(cmd) = &mouse_field.host_command {
                                        let clear_rip_screen = mouse_field.style.reset_screen_after_click();
                                        // Handle screen reset if
                                        /* TODO
                                        if clear_rip_screen {
                                            if let Ok(mut edit_state) = self.term.edit_state.lock() {
                                                let mut caret = edit_state.caret().clone();
                                                {
                                                    let buffer = edit_state.get_buffer_mut();
                                                    buffer.terminal_state.clear_margins_left_right();
                                                    buffer.terminal_state.clear_margins_top_bottom();
                                                    buffer.clear_screen();
                                                }
                                                *edit_state.get_caret_mut() = caret;
                                            }
                                        }*/

                                        // Send the RIP command
                                        return Some(iced::widget::Action::publish(Message::RipCommand(clear_rip_screen, cmd.clone())));
                                    }
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

                                let mouse_event = icy_engine::ansi::mouse_event::MouseEvent {
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
                                        let mut sel = Selection::new(cell);
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
                                        needs_redraw = true;
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
                                needs_redraw = true;
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

                                let mouse_event = icy_engine::ansi::mouse_event::MouseEvent {
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
                        needs_redraw = true;
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

                                    let mouse_event = icy_engine::ansi::mouse_event::MouseEvent {
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

        if needs_redraw { Some(iced::widget::Action::request_redraw()) } else { None }
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

// Helper function to create shader with terminal and monitor settings
pub fn create_crt_shader<'a>(term: &'a Terminal, monitor_settings: MonitorSettings) -> Element<'a, Message> {
    // Let the parent wrapper decide sizing; shader can just be Fill.
    shader(CRTShaderProgram::new(term, monitor_settings))
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .into()
}

static SCALE_FACTOR_BITS: AtomicU32 = AtomicU32::new(f32::to_bits(1.0));

#[inline]
pub fn set_scale_factor(sf: f32) {
    // You can clamp or sanity-check here if desired
    SCALE_FACTOR_BITS.store(sf.to_bits(), std::sync::atomic::Ordering::Relaxed);
}

#[inline]
fn get_scale_factor() -> f32 {
    f32::from_bits(SCALE_FACTOR_BITS.load(std::sync::atomic::Ordering::Relaxed))
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
    let font_w = font.size.width as f32;
    let font_h = font.size.height as f32;

    let scale_factor = get_scale_factor();
    let bounds = bounds * scale_factor;
    let mx = mx * scale_factor;
    let my = my * scale_factor;
    if font_w <= 0.0 || font_h <= 0.0 {
        return None;
    }

    let term_px_w = screen.get_width() as f32 * font_w;
    let term_px_h = screen.get_height() as f32 * font_h;
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
    let local_px_y = (my - vp_y) / display_scale;

    // 9. Convert to cell indices
    let cx = (local_px_x / font_w).floor() as i32;
    let cy = (local_px_y / font_h).floor() as i32;

    if cx < 0 || cy < 0 || cx >= screen.get_width() as i32 || cy >= screen.get_height() as i32 {
        return None;
    }

    Some(Position::new(cx, cy))
}
