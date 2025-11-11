#![allow(clippy::match_same_arms)]
use super::BufferParser;
use crate::{AttributedChar, CallbackAction, EditableScreen, EngineResult};

mod constants;

#[cfg(test)]
mod tests;

/// BBC MODE 7 implementation. Spec here:
/// <https://www.bbcbasic.co.uk/bbcwin/manual/bbcwin8.html>
/// <https://central.kaserver5.org/Kasoft/Typeset/BBC/Ch28.html>
/// <https://www.bbcbasic.co.uk/bbcwin/manual/bbcwinh.html>
pub struct Parser {
    // Escape sequence handling
    got_esc: bool,
    vdu_queue: Vec<u8>,
    vdu_expected: usize,

    // Graphics mode state
    hold_graphics: bool,
    held_graphics_character: u8,
    is_contiguous: bool,
    is_in_graphic_mode: bool,

    // Double height state
    double_height_top_row: Option<i32>,
    double_height_bottom_row: Option<i32>,

    // VDU mode states
    vdu_disabled: bool,
    graphics_cursor_mode: bool,

    // Current colors
    current_fg: u32,
    current_bg: u32,
}

impl Default for Parser {
    fn default() -> Self {
        Self {
            got_esc: false,
            vdu_queue: Vec::new(),
            vdu_expected: 0,
            hold_graphics: false,
            held_graphics_character: b' ',
            is_contiguous: true,
            is_in_graphic_mode: false,
            double_height_top_row: None,
            double_height_bottom_row: None,
            vdu_disabled: false,
            graphics_cursor_mode: false,
            current_fg: 7,
            current_bg: 0,
        }
    }
}

impl Parser {
    #[inline]
    fn ascii_cycle_remap(ch: u8) -> u8 {
        // Prestel/BBC Mode 7 remap cycle for '#', '_', '`' characters.
        // Some services rotate these three to access mosaic variants.
        // Cycle: '#' (35) -> '_' (95) -> '`' (96) -> '#' (35)
        match ch {
            b'#' => b'_',
            b'_' => b'`',
            b'`' => b'#',
            _ => ch,
        }
    }

    #[inline]
    fn mosaic_upgrade(&self, ch: u8) -> u8 {
        // When in graphic (mosaic) mode some hosts "upgrade" ASCII ranges
        // into the mosaic block set by setting bit7. Rough heuristic applied
        // to ranges 32..=63 and 96..=127 (space/punct + lowercase) if in graphics.
        // This is a best-effort implementation; separated/contiguous differences
        // are still handled later in display_graphics_char.
        if self.is_in_graphic_mode {
            if (32..=63).contains(&ch) || (96..=127).contains(&ch) {
                return ch | 0x80; // set bit7 to enter 128+ range
            }
        }
        ch
    }

    fn destructive_backspace(&mut self, buf: &mut dyn EditableScreen) {
        // Teletext destructive backspace semantics: BS, write space, BS again.
        self.caret_left(buf); // move left
        let ach = AttributedChar::new(' ', buf.caret().attribute);
        buf.set_char(buf.caret().position(), ach); // erase previous cell
        self.caret_left(buf); // position caret on erased cell ready for overwrite
    }
    fn reset_line_state(&mut self) {
        // Reset per-line state when moving to a new line
        self.is_in_graphic_mode = false;
        self.hold_graphics = false;
        self.held_graphics_character = b' ';
        self.is_contiguous = true;
        self.current_fg = 7;
        self.current_bg = 0;
    }

    fn caret_down(&mut self, buf: &mut dyn EditableScreen) {
        let old_y = buf.caret().y;
        buf.index();
        if buf.caret().y != old_y {
            self.reset_line_state();
            buf.caret_mut().attribute.set_foreground(self.current_fg);
            buf.caret_mut().attribute.set_background(self.current_bg);
        }
    }

    fn caret_up(&mut self, buf: &mut dyn EditableScreen) {
        let old_y = buf.caret().y;
        if buf.caret().y > 0 {
            buf.caret_mut().y = buf.caret().y - 1;
        } else {
            buf.caret_mut().y = buf.terminal_state().get_height() - 1;
        }
        if buf.caret().y != old_y {
            self.reset_line_state();
            buf.caret_mut().attribute.set_foreground(self.current_fg);
            buf.caret_mut().attribute.set_background(self.current_bg);
        }
    }

    fn caret_right(&mut self, buf: &mut dyn EditableScreen) {
        buf.caret_mut().x = buf.caret().x + 1;
        if buf.caret().x >= buf.terminal_state().get_width() {
            buf.caret_mut().x = 0;
            self.caret_down(buf);
        }
    }

    fn caret_left(&mut self, buf: &mut dyn EditableScreen) {
        if buf.caret().x > 0 {
            buf.caret_mut().x = buf.caret().x - 1;
        } else {
            buf.caret_mut().x = buf.terminal_state().get_width() - 1;
            self.caret_up(buf);
        }
    }

    fn set_at_attributes(&mut self, buf: &mut dyn EditableScreen) {
        // Mode 7 "set-at" behavior: attributes take effect from next char position
        buf.caret_mut().attribute.set_foreground(self.current_fg);
        buf.caret_mut().attribute.set_background(self.current_bg);
    }

    fn display_control_char(&mut self, buf: &mut dyn EditableScreen) {
        // Display space or held graphics for control positions
        let display_ch = if self.hold_graphics && self.is_in_graphic_mode {
            self.held_graphics_character
        } else {
            b' '
        };

        let ach = AttributedChar::new(display_ch as char, buf.caret().attribute);
        buf.set_char(buf.caret().position(), ach);
        self.caret_right(buf);
    }

    fn display_graphics_char(&mut self, buf: &mut dyn EditableScreen, ch: u8) {
        if !self.is_in_graphic_mode {
            // In alpha mode, graphics chars display as spaces
            let ach = AttributedChar::new(' ', buf.caret().attribute);
            buf.set_char(buf.caret().position(), ach);
        } else {
            // Store as held graphics if in range
            if (160..=191).contains(&ch) || (224..=255).contains(&ch) {
                self.held_graphics_character = ch;
            }

            // Map to block graphics character
            let mapped_ch = if self.is_contiguous {
                // Contiguous graphics mapping
                if (160..=191).contains(&ch) {
                    ch - 32 // Map to 128-159
                } else if (224..=255).contains(&ch) {
                    ch - 64 // Map to 160-191
                } else {
                    ch
                }
            } else {
                // Separated graphics mapping
                if (160..=191).contains(&ch) {
                    ch + 32 // Map to 192-223
                } else if (224..=255).contains(&ch) {
                    ch // Already in 224-255
                } else {
                    ch
                }
            };

            let ach = AttributedChar::new(mapped_ch as char, buf.caret().attribute);
            buf.set_char(buf.caret().position(), ach);
        }
        self.caret_right(buf);
    }

    fn handle_vdu_sequence(&mut self, buf: &mut dyn EditableScreen, ch: u8) -> CallbackAction {
        self.vdu_queue.push(ch);

        if self.vdu_queue.len() >= self.vdu_expected {
            // Process complete VDU sequence
            match self.vdu_queue[0] {
                17 if self.vdu_queue.len() >= 2 => {
                    // VDU 17,n - COLOUR n
                    let color = self.vdu_queue[1];
                    if color < 128 {
                        // Foreground
                        self.current_fg = (color & 15) as u32;
                    } else {
                        // Background
                        self.current_bg = ((color - 128) & 15) as u32;
                    }
                    self.set_at_attributes(buf);
                }
                22 if self.vdu_queue.len() >= 2 => {
                    // VDU 22,n - MODE n
                    // Reset parser state for new mode
                    *self = Self::default();
                    buf.reset_terminal();
                }
                31 if self.vdu_queue.len() >= 3 => {
                    // VDU 31,x,y - TAB(x,y)
                    let x = self.vdu_queue[1] as i32;
                    let y = self.vdu_queue[2] as i32;
                    if x < buf.terminal_state().get_width() && y < buf.terminal_state().get_height() {
                        buf.caret_mut().x = x;
                        buf.caret_mut().y = y;
                    }
                }
                _ => {}
            }

            self.vdu_queue.clear();
            self.vdu_expected = 0;
        }

        CallbackAction::Update
    }
}

impl BufferParser for Parser {
    fn print_char(&mut self, buf: &mut dyn EditableScreen, ch: char) -> EngineResult<CallbackAction> {
        let ch = ch as u8;

        // Handle VDU disabled state
        if self.vdu_disabled && ch != 6 {
            return Ok(CallbackAction::None);
        }

        // Handle escape sequences
        if self.got_esc {
            self.got_esc = false;
            // VDU 27 - next character goes directly to screen
            let ach = AttributedChar::new(ch as char, buf.caret().attribute);
            buf.set_char(buf.caret().position(), ach);
            self.caret_right(buf);
            return Ok(CallbackAction::Update);
        }

        // Handle multi-byte VDU sequences
        if self.vdu_expected > 0 {
            return Ok(self.handle_vdu_sequence(buf, ch));
        }

        match ch {
            0 => {} // Null - does nothing

            1 => {} // Send next to printer only - not implemented
            2 => {} // Enable printer - not implemented
            3 => {} // Disable printer - not implemented

            4 => {
                // Write text at text cursor
                self.graphics_cursor_mode = false;
            }
            5 => {
                // Write text at graphics cursor (does nothing in Mode 7)
                self.graphics_cursor_mode = true;
            }
            6 => {
                // Enable screen output
                self.vdu_disabled = false;
            }
            7 => {
                // Bell
                return Ok(CallbackAction::Beep);
            }
            8 => {
                // Cursor left
                self.caret_left(buf);
            }
            9 => {
                // Cursor right
                self.caret_right(buf);
            }
            10 => {
                // Cursor down
                self.caret_down(buf);
            }
            11 => {
                // Cursor up
                self.caret_up(buf);
            }
            12 => {
                // Clear screen (CLS)
                buf.clear_screen();
                buf.home();
                self.reset_line_state();
            }
            13 => {
                // Carriage return
                buf.cr();
                self.reset_line_state();
            }
            14 => {} // Enable auto-paging - not implemented
            15 => {} // Disable auto-paging - not implemented
            16 => {} // Clear graphics area (CLG) - does nothing in Mode 7

            17 => {
                // COLOUR n - expect 1 more byte
                self.vdu_expected = 2;
                self.vdu_queue.clear();
                self.vdu_queue.push(ch);
            }
            18 => {
                // GCOL mode,colour - expect 2 more bytes (ignored in Mode 7)
                self.vdu_expected = 3;
                self.vdu_queue.clear();
                self.vdu_queue.push(ch);
            }
            19 => {
                // VDU 19 - palette - expect 5 more bytes
                self.vdu_expected = 6;
                self.vdu_queue.clear();
                self.vdu_queue.push(ch);
            }
            20 => {
                // Restore default colors
                self.current_fg = 7;
                self.current_bg = 0;
                self.set_at_attributes(buf);
            }
            21 => {
                // Disable screen output
                self.vdu_disabled = true;
            }
            22 => {
                // MODE - expect 1 more byte
                self.vdu_expected = 2;
                self.vdu_queue.clear();
                self.vdu_queue.push(ch);
            }
            23 => {
                // Various - expect 9 more bytes
                self.vdu_expected = 10;
                self.vdu_queue.clear();
                self.vdu_queue.push(ch);
            }
            24 => {
                // Graphics viewport - expect 8 more bytes (ignored in Mode 7)
                self.vdu_expected = 9;
                self.vdu_queue.clear();
                self.vdu_queue.push(ch);
            }
            25 => {
                // PLOT - expect 4 more bytes (ignored in Mode 7)
                self.vdu_expected = 5;
                self.vdu_queue.clear();
                self.vdu_queue.push(ch);
            }
            26 => {
                // Reset viewports
                buf.home();
                self.reset_line_state();
            }
            27 => {
                // Next char to screen
                self.got_esc = true;
            }
            28 => {
                // Text viewport - expect 4 more bytes
                self.vdu_expected = 5;
                self.vdu_queue.clear();
                self.vdu_queue.push(ch);
            }
            29 => {
                // Graphics origin - expect 4 more bytes (ignored in Mode 7)
                self.vdu_expected = 5;
                self.vdu_queue.clear();
                self.vdu_queue.push(ch);
            }
            30 => {
                // Home cursor
                buf.home();
                self.reset_line_state();
            }
            31 => {
                // TAB(x,y) - expect 2 more bytes
                self.vdu_expected = 3;
                self.vdu_queue.clear();
                self.vdu_queue.push(ch);
            }
            127 => {
                // Destructive backspace (erase and stay on erased cell)
                self.destructive_backspace(buf);
            }

            // Mode 7 control codes
            129..=135 => {
                // Alpha colors: Red, Green, Yellow, Blue, Magenta, Cyan, White
                self.is_in_graphic_mode = false;
                self.current_fg = 1 + (ch - 129) as u32;
                self.set_at_attributes(buf);
                buf.caret_mut().attribute.set_is_concealed(false);
                self.display_control_char(buf);
            }
            136 => {
                // Flash
                buf.caret_mut().attribute.set_is_blinking(true);
                self.display_control_char(buf);
            }
            137 => {
                // Steady
                buf.caret_mut().attribute.set_is_blinking(false);
                self.display_control_char(buf);
            }
            140 => {
                // Normal height
                buf.caret_mut().attribute.set_is_double_height(false);
                self.double_height_top_row = None;
                self.double_height_bottom_row = None;
                self.display_control_char(buf);
            }
            141 => {
                // Double height
                buf.caret_mut().attribute.set_is_double_height(true);
                let y = buf.caret().y;
                if self.double_height_top_row.is_none() {
                    self.double_height_top_row = Some(y);
                    self.double_height_bottom_row = Some(y + 1);
                }
                self.display_control_char(buf);
            }
            145..=151 => {
                // Graphics colors: Red, Green, Yellow, Blue, Magenta, Cyan, White
                self.is_in_graphic_mode = true;
                self.current_fg = 1 + (ch - 145) as u32;
                self.set_at_attributes(buf);
                buf.caret_mut().attribute.set_is_concealed(false);
                self.display_control_char(buf);
            }
            152 => {
                // Conceal display
                buf.caret_mut().attribute.set_is_concealed(true);
                self.display_control_char(buf);
            }
            153 => {
                // Contiguous graphics
                self.is_contiguous = true;
                self.display_control_char(buf);
            }
            154 => {
                // Separated graphics
                self.is_contiguous = false;
                self.display_control_char(buf);
            }
            156 => {
                // Black background
                self.current_bg = 0;
                self.set_at_attributes(buf);
                self.display_control_char(buf);
            }
            157 => {
                // New background (use current foreground color)
                self.current_bg = self.current_fg;
                self.set_at_attributes(buf);
                self.display_control_char(buf);
            }
            158 => {
                // Hold graphics
                self.hold_graphics = true;
                self.display_control_char(buf);
            }
            159 => {
                // Release graphics
                self.hold_graphics = false;
                self.display_control_char(buf);
            }

            // Printable characters and graphics
            32..=126 => {
                // Normal ASCII printable with optional remap & mosaic upgrade
                let mut mapped = Self::ascii_cycle_remap(ch);
                mapped = self.mosaic_upgrade(mapped);
                let ach = AttributedChar::new(mapped as char, buf.caret().attribute);
                buf.set_char(buf.caret().position(), ach);
                self.caret_right(buf);
            }
            160..=255 => {
                // Graphics characters
                self.display_graphics_char(buf, ch);
            }

            _ => {
                // Raw C1 (0x80..0x9F) or other values: forward as-is for now.
                // Future: hook into dedicated escape/VDU handler for Prestel specifics.
                let ach = AttributedChar::new(ch as char, buf.caret().attribute);
                buf.set_char(buf.caret().position(), ach);
                self.caret_right(buf);
            }
        }

        Ok(CallbackAction::Update)
    }
}
