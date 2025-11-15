use crate::{MouseMode, MouseState, Position};

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
    pub button: MouseButton,
    pub modifiers: KeyModifiers,
}

impl MouseEvent {
    pub fn new(mouse_state: MouseState) -> Self {
        Self {
            mouse_state,
            event_type: MouseEventType::Press,
            position: Position::default(),
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

        match self.mouse_state.mouse_mode {
            MouseMode::OFF => None,

            MouseMode::X10 => {
                // X10 only reports button press
                if matches!(self.event_type, MouseEventType::Press) {
                    let cb = encode_x10_button(self.button, &self.modifiers);
                    Some(format!(
                        "\x1B[M{}{}{}",
                        char::from((cb + 32) as u8),
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
                    char::from((cb + 32) as u8),
                    char::from((x.min(223) + 32) as u8),
                    char::from((y.min(223) + 32) as u8)
                ))
            }

            MouseMode::ButtonEvents => {
                let mut cb = encode_vt200_button(self.button, self.event_type, &self.modifiers);
                if matches!(self.event_type, MouseEventType::Motion) {
                    cb += 32; // Add motion indicator
                }
                Some(format!(
                    "\x1B[M{}{}{}",
                    char::from((cb + 32) as u8),
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
                    char::from((cb + 32) as u8),
                    char::from((x.min(223) + 32) as u8),
                    char::from((y.min(223) + 32) as u8)
                ))
            } /*
              MouseMode::SGRExtendedMode => {
                  // SGR format: CSI < Cb ; Cx ; Cy M (press) or m (release)
                  let cb = encode_sgr_button(button, &modifiers);
                  let end_char = if matches!(event_type, MouseEventType::Release) { 'm' } else { 'M' };
                  Some(format!("\x1B[<{};{};{}{}", cb, x, y, end_char))
              }

              MouseMode::URXVTExtendedMode => {
                  // URXVT format: CSI Cb ; Cx ; Cy M
                  let cb = encode_vt200_button(button, event_type, &modifiers) + 32;
                  Some(format!("\x1B[{};{};{}M", cb, x, y))
              }

              MouseMode::ExtendedMode => {
                  // UTF-8 encoding for coordinates > 223
                  let cb = encode_vt200_button(button, event_type, &modifiers);
                  Some(encode_utf8_mouse(cb, x, y))
              }

              MouseMode::PixelPosition => {
                  // Similar to SGR but reports pixel position
                  // This would need pixel coordinates from the rendering layer
                  None // TODO: Implement when pixel coordinates are available
              }

              MouseMode::FocusEvent | MouseMode::AlternateScroll |
              MouseMode::VT200_Highlight => {
                  // These have special handling
                  None
              }*/
        }
    }
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
            MouseButton::None | MouseButton::Left => 0,
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
    if focused { "\x1B[I".to_string() } else { "\x1B[O".to_string() }
}
