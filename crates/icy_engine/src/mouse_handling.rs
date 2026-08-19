use crate::{ExtMouseMode, MouseMode, MouseState, Position};

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum MouseButton {
    #[default]
    None = -1,
    Left = 0,
    Middle = 1,
    Right = 2,
    WheelUp = 3,
    WheelDown = 4,
    // Extended buttons
    Button6 = 5,
    Button7 = 6,
    Button8 = 7,
    Button9 = 8,
    Button10 = 9,
    Button11 = 10,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MouseEventType {
    Press,
    Release,
    Motion,

    FocusIn,
    FocusOut,
}

#[derive(Default, Clone, Debug, PartialEq)]
pub struct KeyModifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
}

#[derive(Debug, Clone)]
pub struct MouseEvent {
    pub mouse_state: MouseState,
    pub event_type: MouseEventType,
    pub position: Position,
    /// Terminal-document pixel position for DEC mode 1016.
    pub pixel_position: Option<Position>,
    pub button: MouseButton,
    pub modifiers: KeyModifiers,
}

impl MouseEvent {
    pub fn new(mouse_state: MouseState) -> Self {
        Self {
            mouse_state,
            event_type: MouseEventType::Press,
            position: Position::default(),
            pixel_position: None,
            button: MouseButton::default(),
            modifiers: KeyModifiers::default(),
        }
    }
    pub fn generate_mouse_report(&self) -> Option<String> {
        match self.event_type {
            MouseEventType::FocusIn => {
                if self.mouse_state.focus_out_event_enabled {
                    Some(generate_focus_event(true))
                } else {
                    None
                }
            }
            MouseEventType::FocusOut => {
                if self.mouse_state.focus_out_event_enabled {
                    Some(generate_focus_event(false))
                } else {
                    None
                }
            }
            _ => self.generate_button_mouse_report(),
        }
    }

    fn generate_button_mouse_report(&self) -> Option<String> {
        // Convert to 1-based terminal coordinates
        let x = self.position.x + 1;
        let y = self.position.y + 1;

        if self.mouse_state.alternate_scroll_enabled
            && matches!(self.event_type, MouseEventType::Press)
            && (self.button == MouseButton::WheelUp || self.button == MouseButton::WheelDown)
        {
            // Standard (non-application) cursor key sequences:
            // Up: ESC [ A   Down: ESC [ B
            // If in the future you track application cursor mode (DECCKM), switch to ESC O A / ESC O B.
            let seq = if self.button == MouseButton::WheelUp { "\x1B[A" } else { "\x1B[B" };
            return Some(seq.to_string());
        }

        if self.mouse_state.mouse_mode == MouseMode::OFF {
            return None;
        }

        let mut cb = if matches!(self.mouse_state.extended_mode, ExtMouseMode::SGR | ExtMouseMode::PixelPosition) {
            encode_sgr_button(self.button, &self.modifiers)
        } else {
            encode_vt200_button(self.button, self.event_type, &self.modifiers)
        };
        if matches!(self.event_type, MouseEventType::Motion) {
            cb |= 32;
        }

        match self.mouse_state.extended_mode {
            ExtMouseMode::SGR | ExtMouseMode::PixelPosition => {
                let (x, y) = if self.mouse_state.extended_mode == ExtMouseMode::PixelPosition {
                    let pixel = self.pixel_position?;
                    (pixel.x + 1, pixel.y + 1)
                } else {
                    (x, y)
                };
                let final_byte = if matches!(self.event_type, MouseEventType::Release) { 'm' } else { 'M' };
                return Some(format!("\x1B[<{cb};{x};{y}{final_byte}"));
            }
            ExtMouseMode::URXVT => return Some(format!("\x1B[{};{x};{y}M", cb + 32)),
            ExtMouseMode::ExtendedUTF8 => return Some(encode_utf8_mouse(cb, x, y)),
            ExtMouseMode::None => {}
        }

        match self.mouse_state.mouse_mode {
            MouseMode::OFF => None,

            MouseMode::X10 => {
                // X10 only reports button press
                if matches!(self.event_type, MouseEventType::Press) {
                    let cb = encode_x10_button(self.button, &self.modifiers);
                    Some(format!(
                        "\x1B[M{}{}{}",
                        char::from(cb + 32),
                        char::from((x.min(223) + 32) as u8),
                        char::from((y.min(223) + 32) as u8)
                    ))
                } else {
                    None
                }
            }

            MouseMode::VT200 | MouseMode::VT200_Highlight => {
                let cb = encode_vt200_button(self.button, self.event_type, &self.modifiers);
                Some(format!(
                    "\x1B[M{}{}{}",
                    char::from(cb + 32),
                    char::from((x.min(223) + 32) as u8),
                    char::from((y.min(223) + 32) as u8)
                ))
            }

            MouseMode::ButtonEvents => {
                if self.button == MouseButton::None && matches!(self.event_type, MouseEventType::Motion) {
                    return None;
                }
                let mut cb = encode_vt200_button(self.button, self.event_type, &self.modifiers);
                if matches!(self.event_type, MouseEventType::Motion) {
                    cb += 32; // Add motion indicator
                }
                Some(format!(
                    "\x1B[M{}{}{}",
                    char::from(cb + 32),
                    char::from((x.min(223) + 32) as u8),
                    char::from((y.min(223) + 32) as u8)
                ))
            }

            MouseMode::AnyEvents => {
                // Reports all motion events
                let mut cb = encode_vt200_button(self.button, self.event_type, &self.modifiers);
                if matches!(self.event_type, MouseEventType::Motion) {
                    cb += 32;
                }
                Some(format!(
                    "\x1B[M{}{}{}",
                    char::from(cb + 32),
                    char::from((x.min(223) + 32) as u8),
                    char::from((y.min(223) + 32) as u8)
                ))
            }
        }
    }
}

fn encode_utf8_mouse(cb: u8, x: i32, y: i32) -> String {
    let encode = |value: i32| char::from_u32((value.clamp(0, 2015) + 32) as u32).unwrap_or('\u{fffd}');
    format!("\x1B[M{}{}{}", char::from(cb + 32), encode(x), encode(y))
}

fn encode_x10_button(button: MouseButton, modifiers: &KeyModifiers) -> u8 {
    let mut cb = match button {
        MouseButton::Left => 0,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
        _ => return 3, // Not supported in X10
    };

    if modifiers.shift {
        cb |= 4;
    }
    if modifiers.alt || modifiers.meta {
        cb |= 8;
    }
    if modifiers.ctrl {
        cb |= 16;
    }

    cb
}

fn encode_vt200_button(button: MouseButton, event_type: MouseEventType, modifiers: &KeyModifiers) -> u8 {
    let mut cb = match event_type {
        MouseEventType::Release => 3,
        _ => match button {
            MouseButton::None => 3,
            MouseButton::Left => 0,
            MouseButton::Middle => 1,
            MouseButton::Right => 2,
            MouseButton::WheelUp => 64,
            MouseButton::WheelDown => 65,
            MouseButton::Button6 => 66,
            MouseButton::Button7 => 67,
            MouseButton::Button8 => 128,
            MouseButton::Button9 => 129,
            MouseButton::Button10 => 130,
            MouseButton::Button11 => 131,
        },
    };

    if modifiers.shift {
        cb |= 4;
    }
    if modifiers.alt || modifiers.meta {
        cb |= 8;
    }
    if modifiers.ctrl {
        cb |= 16;
    }

    cb
}

fn encode_sgr_button(button: MouseButton, modifiers: &KeyModifiers) -> u8 {
    let mut cb = match button {
        MouseButton::Left => 0,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
        MouseButton::WheelUp => 64,
        MouseButton::WheelDown => 65,
        MouseButton::Button6 => 66,
        MouseButton::Button7 => 67,
        MouseButton::Button8 => 128,
        MouseButton::Button9 => 129,
        MouseButton::Button10 => 130,
        MouseButton::Button11 => 131,
        MouseButton::None => 3,
    };
    if modifiers.shift {
        cb |= 4;
    }
    if modifiers.alt || modifiers.meta {
        cb |= 8;
    }
    if modifiers.ctrl {
        cb |= 16;
    }
    cb
}
/*
fn encode_sgr_button(button: MouseButton, modifiers: &KeyModifiers) -> u8 {
    // SGR doesn't add 32 to the button code
    let mut cb = match button {
        MouseButton::Left => 0,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
        MouseButton::WheelUp => 64,
        MouseButton::WheelDown => 65,
        _ => 3,
    };

    if modifiers.shift { cb |= 4; }
    if modifiers.alt || modifiers.meta { cb |= 8; }
    if modifiers.ctrl { cb |= 16; }

    cb
}

fn encode_utf8_mouse(cb: u8, x: i32, y: i32) -> String {
    let mut result = String::from("\x1B[M");

    // Encode button
    if cb < 128 {
        result.push(char::from(cb + 32));
    } else {
        // UTF-8 encode values >= 128
        result.push_str(&to_utf8_mouse_coord(cb as i32));
    }

    // Encode coordinates
    result.push_str(&to_utf8_mouse_coord(x));
    result.push_str(&to_utf8_mouse_coord(y));

    result
}

fn to_utf8_mouse_coord(val: i32) -> String {
    if val < 96 {
        String::from(char::from((val + 32) as u8))
    } else if val < 2048 {
        // 2-byte UTF-8
        let b1 = 0xC0 | ((val >> 6) & 0x1F);
        let b2 = 0x80 | (val & 0x3F);
        String::from_utf8(vec![b1 as u8, b2 as u8]).unwrap_or_default()
    } else {
        // Clamp to max supported
        String::from(char::from(255))
    }
}
*/
fn generate_focus_event(focused: bool) -> String {
    if focused {
        "\x1B[I".to_string()
    } else {
        "\x1B[O".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn motion_event(mode: MouseMode, button: MouseButton) -> MouseEvent {
        let mut mouse_state = MouseState::default();
        mouse_state.mouse_mode = mode;
        MouseEvent {
            mouse_state,
            event_type: MouseEventType::Motion,
            position: Position::new(0, 0),
            pixel_position: None,
            button,
            modifiers: KeyModifiers::default(),
        }
    }

    #[test]
    fn any_event_motion_distinguishes_unpressed_and_dragged() {
        assert_eq!(
            motion_event(MouseMode::AnyEvents, MouseButton::None).generate_mouse_report(),
            Some("\x1B[MC!!".to_string())
        );
        assert_eq!(
            motion_event(MouseMode::AnyEvents, MouseButton::Left).generate_mouse_report(),
            Some("\x1B[M@!!".to_string())
        );
    }

    #[test]
    fn button_event_mode_ignores_unpressed_motion() {
        assert_eq!(motion_event(MouseMode::ButtonEvents, MouseButton::None).generate_mouse_report(), None);
    }

    #[test]
    fn sgr_reports_press_release_motion_and_wheel() {
        let mut event = motion_event(MouseMode::AnyEvents, MouseButton::None);
        event.mouse_state.extended_mode = ExtMouseMode::SGR;
        event.position = Position::new(9, 19);
        assert_eq!(event.generate_mouse_report(), Some("\x1B[<35;10;20M".to_string()));

        event.event_type = MouseEventType::Press;
        event.button = MouseButton::Left;
        assert_eq!(event.generate_mouse_report(), Some("\x1B[<0;10;20M".to_string()));

        event.event_type = MouseEventType::Release;
        assert_eq!(event.generate_mouse_report(), Some("\x1B[<0;10;20m".to_string()));

        event.button = MouseButton::Right;
        assert_eq!(event.generate_mouse_report(), Some("\x1B[<2;10;20m".to_string()));

        event.event_type = MouseEventType::Press;
        event.button = MouseButton::WheelDown;
        assert_eq!(event.generate_mouse_report(), Some("\x1B[<65;10;20M".to_string()));
    }

    #[test]
    fn pixel_mode_uses_terminal_pixels_not_cells() {
        let mut event = motion_event(MouseMode::AnyEvents, MouseButton::None);
        event.mouse_state.extended_mode = ExtMouseMode::PixelPosition;
        event.position = Position::new(9, 19);
        event.pixel_position = Some(Position::new(123, 77));
        assert_eq!(event.generate_mouse_report(), Some("\x1B[<35;124;78M".to_string()));
    }
}
