use crate::{CRTShaderState, TerminalMessage, TerminalMouseEvent, TerminalShader};
use icy_engine::{KeyModifiers, MouseButton};
use icy_ui::{mouse, widget::shader, window, Rectangle};
use std::time::Duration;

pub use super::frame::{clamp_terminal_height_to_viewport, CRTShaderProgram};

impl CRTShaderProgram<'_> {
    /// Helper function to convert icy_ui Modifiers to icy_engine KeyModifiers
    fn convert_modifiers(modifiers: icy_ui::keyboard::Modifiers) -> KeyModifiers {
        KeyModifiers {
            shift: modifiers.shift(),
            ctrl: modifiers.control(),
            alt: modifiers.alt(),
            meta: modifiers.logo(),
        }
    }

    fn internal_draw(&self, state: &CRTShaderState, _cursor: mouse::Cursor, bounds: Rectangle) -> TerminalShader {
        self.frame(state, [bounds.width, bounds.height], crate::get_scale_factor())
    }

    /// Simplified internal_update that only handles coordinate mapping and event emission.
    pub fn internal_update(
        &self,
        state: &mut CRTShaderState,
        event: &icy_ui::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<icy_ui::widget::Action<TerminalMessage>> {
        let now = crate::Blink::now_ms();

        // Gate blink work to cases where it is actually visible/relevant.
        // This avoids a perpetual redraw loop when nothing is blinking.
        let mut char_blink_supported = true;
        let mut caret_blink_requested = false;
        let mut synchronized_frame: Option<Duration> = None;

        if let Some(screen) = self.term.screen.try_lock() {
            let buffer_type = screen.buffer_type();
            state.caret_blink.set_rate(buffer_type.caret_blink_rate() as u128);
            state.character_blink.set_rate(buffer_type.blink_rate() as u128);

            // In IceMode::Ice, the blink attribute is repurposed for high background colors.
            char_blink_supported = screen.ice_mode().has_blink();

            let caret = screen.caret();
            caret_blink_requested = caret.visible && caret.blinking && self.term.has_focus;

            // Keep frames coming so a synchronized update that is never closed
            // still reaches the screen when it times out.
            synchronized_frame = screen.terminal_state().synchronized_output_remaining();
        }

        if caret_blink_requested {
            state.caret_blink.update(now);
        }
        if char_blink_supported {
            state.character_blink.update(now);
        }

        let is_over = cursor.is_over(bounds);

        // Handle animation: on each redraw, check if we need more animation frames
        if let icy_ui::Event::Window(window::Event::RedrawRequested(_instant)) = event {
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
            let next_frame = match (next_frame, synchronized_frame) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            };

            if let Some(delay) = next_frame {
                let next = *_instant + delay;
                return Some(icy_ui::widget::Action::request_redraw_at(next));
            }
        }

        if let icy_ui::Event::Mouse(mouse_event) = event {
            let scroll_state = self.term.scroll_state();
            let render_info = self.term.render_info.read();

            // Mouse events should be relative to the terminal widget.
            // `TerminalMouseEvent.pixel_position` is documented as widget-relative.
            let local_pos_in_bounds = cursor.position_in(bounds);
            let local_pos_unclamped = cursor.position().map(|p| icy_ui::Point {
                x: p.x - bounds.x,
                y: p.y - bounds.y,
            });

            if let mouse::Event::ButtonReleased { button, modifiers } = mouse_event {
                if state.dragging && (matches!(button, mouse::Button::Left) || matches!(button, mouse::Button::Right)) {
                    state.dragging = false;
                    state.drag_anchor = None;
                    state.last_drag_position = None;
                    state.last_drag_pixel_position = None;

                    // Use unclamped for drag release to get position even outside viewport
                    let (pixel_pos, cell_pos) = if let Some(position) = local_pos_unclamped {
                        let pixel_pos = (position.x, position.y);
                        let cell_pos = state.map_mouse_to_cell_unclamped(&render_info, position.x, position.y, scroll_state.scroll_x, scroll_state.scroll_y);
                        (pixel_pos, Some(cell_pos))
                    } else {
                        ((0.0, 0.0), None)
                    };

                    let modifiers = Self::convert_modifiers(*modifiers);
                    let terminal_pixel = state.map_mouse_to_pixel(&render_info, pixel_pos.0, pixel_pos.1, scroll_state.scroll_x, scroll_state.scroll_y);
                    let evt = TerminalMouseEvent::new(pixel_pos, cell_pos, state.drag_button, modifiers).with_terminal_pixel_position(terminal_pixel);
                    state.drag_button = MouseButton::None;
                    return Some(icy_ui::widget::Action::publish(TerminalMessage::Release(evt)));
                }
            }

            if state.dragging {
                if let mouse::Event::CursorMoved { modifiers, .. } = mouse_event {
                    if let Some(position) = local_pos_unclamped {
                        let pixel_pos = (position.x, position.y);
                        // Use unclamped version during drag to allow operations beyond viewport
                        let cell_pos = state.map_mouse_to_cell_unclamped(&render_info, position.x, position.y, scroll_state.scroll_x, scroll_state.scroll_y);
                        let terminal_pixel = state.map_mouse_to_pixel(&render_info, position.x, position.y, scroll_state.scroll_x, scroll_state.scroll_y);
                        let pixel_mode = state
                            .cached_mouse_state
                            .lock()
                            .as_ref()
                            .is_some_and(|mouse| mouse.extended_mode == icy_engine::ExtMouseMode::PixelPosition);

                        if if pixel_mode {
                            state.last_drag_pixel_position == terminal_pixel
                        } else {
                            state.last_drag_position == Some(cell_pos)
                        } {
                            return None;
                        }
                        state.last_drag_position = Some(cell_pos);
                        state.last_drag_pixel_position = terminal_pixel;

                        let modifiers = Self::convert_modifiers(*modifiers);
                        let evt = TerminalMouseEvent::new(pixel_pos, Some(cell_pos), state.drag_button, modifiers).with_terminal_pixel_position(terminal_pixel);
                        return Some(icy_ui::widget::Action::publish(TerminalMessage::Drag(evt)));
                    }
                }
            }

            if !is_over {
                return None;
            }

            match mouse_event {
                mouse::Event::CursorMoved { modifiers, .. } => {
                    if let Some(position) = local_pos_in_bounds {
                        let pixel_pos = (position.x, position.y);
                        let cell_pos = state.map_mouse_to_cell(&render_info, position.x, position.y, scroll_state.scroll_x, scroll_state.scroll_y);
                        let terminal_pixel = state.map_mouse_to_pixel(&render_info, position.x, position.y, scroll_state.scroll_x, scroll_state.scroll_y);
                        let pixel_mode = state
                            .cached_mouse_state
                            .lock()
                            .as_ref()
                            .is_some_and(|mouse| mouse.extended_mode == icy_engine::ExtMouseMode::PixelPosition);

                        if if pixel_mode {
                            state.last_move_pixel_position == terminal_pixel
                        } else {
                            state.last_move_position == cell_pos
                        } {
                            return None;
                        }
                        state.last_move_position = cell_pos;
                        state.last_move_pixel_position = terminal_pixel;

                        if state.hovered_cell != cell_pos {
                            state.hovered_cell = cell_pos;
                        }

                        let modifiers = Self::convert_modifiers(*modifiers);
                        let button = if state.dragging { state.drag_button } else { MouseButton::None };
                        let evt = TerminalMouseEvent::new(pixel_pos, cell_pos, button, modifiers).with_terminal_pixel_position(terminal_pixel);

                        if state.dragging {
                            return Some(icy_ui::widget::Action::publish(TerminalMessage::Drag(evt)));
                        } else {
                            return Some(icy_ui::widget::Action::publish(TerminalMessage::Move(evt)));
                        }
                    }
                }

                mouse::Event::ButtonPressed { button, modifiers } => {
                    if let Some(position) = local_pos_in_bounds {
                        let pixel_pos = (position.x, position.y);
                        let cell_pos = state.map_mouse_to_cell(&render_info, position.x, position.y, scroll_state.scroll_x, scroll_state.scroll_y);

                        let mouse_button = match button {
                            mouse::Button::Left => MouseButton::Left,
                            mouse::Button::Middle => MouseButton::Middle,
                            mouse::Button::Right => MouseButton::Right,
                            _ => return None,
                        };

                        if matches!(button, mouse::Button::Left | mouse::Button::Right) {
                            // If a previous drag was in progress (e.g. the matching
                            // ButtonReleased was consumed by an overlay like a context
                            // menu), reset the stale drag state before starting a new one.
                            if state.dragging {
                                state.drag_anchor = None;
                                state.last_drag_position = None;
                                state.last_drag_pixel_position = None;
                                state.drag_button = MouseButton::None;
                            }
                            state.dragging = true;
                            state.drag_button = mouse_button;
                            state.drag_anchor = cell_pos;
                            state.last_drag_position = cell_pos;
                        }

                        let modifiers = Self::convert_modifiers(*modifiers);
                        let terminal_pixel = state.map_mouse_to_pixel(&render_info, position.x, position.y, scroll_state.scroll_x, scroll_state.scroll_y);
                        let evt = TerminalMouseEvent::new(pixel_pos, cell_pos, mouse_button, modifiers).with_terminal_pixel_position(terminal_pixel);

                        return Some(icy_ui::widget::Action::publish(TerminalMessage::Press(evt)));
                    }
                }

                mouse::Event::ButtonReleased { button, modifiers } => {
                    // Middle button single-click (Middle doesn't support drag)
                    if matches!(button, mouse::Button::Middle) {
                        if let Some(position) = local_pos_in_bounds {
                            let pixel_pos = (position.x, position.y);
                            let cell_pos = state.map_mouse_to_cell(&render_info, position.x, position.y, scroll_state.scroll_x, scroll_state.scroll_y);

                            let modifiers = Self::convert_modifiers(*modifiers);
                            let terminal_pixel = state.map_mouse_to_pixel(&render_info, position.x, position.y, scroll_state.scroll_x, scroll_state.scroll_y);
                            let evt = TerminalMouseEvent::new(pixel_pos, cell_pos, MouseButton::Middle, modifiers).with_terminal_pixel_position(terminal_pixel);

                            return Some(icy_ui::widget::Action::publish(TerminalMessage::Release(evt)));
                        }
                    // Left/Right single-click (only if not dragging)
                    } else if !state.dragging {
                        if let Some(position) = local_pos_in_bounds {
                            let pixel_pos = (position.x, position.y);
                            let cell_pos = state.map_mouse_to_cell(&render_info, position.x, position.y, scroll_state.scroll_x, scroll_state.scroll_y);

                            state.dragging = false;
                            state.drag_anchor = None;
                            state.last_drag_position = None;
                            state.last_drag_pixel_position = None;
                            state.last_move_position = None;
                            state.last_move_pixel_position = None;

                            let mouse_button = match button {
                                mouse::Button::Left => MouseButton::Left,
                                mouse::Button::Right => MouseButton::Right,
                                _ => return None,
                            };

                            let modifiers = Self::convert_modifiers(*modifiers);
                            let terminal_pixel = state.map_mouse_to_pixel(&render_info, position.x, position.y, scroll_state.scroll_x, scroll_state.scroll_y);
                            let evt = TerminalMouseEvent::new(pixel_pos, cell_pos, mouse_button, modifiers).with_terminal_pixel_position(terminal_pixel);

                            return Some(icy_ui::widget::Action::publish(TerminalMessage::Release(evt)));
                        }
                    }
                }

                mouse::Event::WheelScrolled { delta, modifiers } => {
                    // Check for Ctrl (or Cmd on macOS) for zoom
                    if modifiers.control() || modifiers.logo() {
                        return Some(icy_ui::widget::Action::publish(TerminalMessage::Zoom(crate::ZoomMessage::Wheel(*delta))));
                    }

                    if let Some(position) = local_pos_in_bounds {
                        let pixel_pos = (position.x, position.y);
                        let cell_pos = state.map_mouse_to_cell(&render_info, position.x, position.y, scroll_state.scroll_x, scroll_state.scroll_y);
                        let terminal_pixel = state.map_mouse_to_pixel(&render_info, position.x, position.y, scroll_state.scroll_x, scroll_state.scroll_y);
                        let event = TerminalMouseEvent::new(pixel_pos, cell_pos, MouseButton::None, Self::convert_modifiers(*modifiers))
                            .with_terminal_pixel_position(terminal_pixel);
                        return Some(icy_ui::widget::Action::publish(TerminalMessage::Scroll(*delta, event)));
                    }
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
        event: &icy_ui::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<icy_ui::widget::Action<TerminalMessage>> {
        self.internal_update(state, event, bounds, cursor)
    }

    fn mouse_interaction(&self, _state: &Self::State, bounds: Rectangle, cursor: mouse::Cursor) -> mouse::Interaction {
        if !cursor.is_over(bounds) {
            return mouse::Interaction::default();
        }

        self.term.cursor_icon.read().unwrap_or_default()
    }
}
