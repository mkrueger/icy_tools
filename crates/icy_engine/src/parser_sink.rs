//! CommandSink implementation for EditableScreen
//!
//! This module provides `ScreenSink`, an adapter that implements the `CommandSink` trait
//! from `icy_parser_core` for any type implementing `EditableScreen`. This allows the new
//! parser infrastructure to drive icy_engine's terminal emulation.
//!
//! # Example
//!
//! ```no_run
//! use icy_engine::{ScreenSink, TextScreen, Size};
//! use icy_parser_core::{AnsiParser, CommandParser};
//!
//! let mut screen = TextScreen::new(Size::new(80, 25));
//! let mut sink = ScreenSink::new(&mut screen);
//! let mut parser = AnsiParser::new();
//!
//! parser.parse(b"\x1b[1;32mHello, World!\x1b[0m", &mut sink);
//! ```

use icy_parser_core::{
    AnsiMode, AnsiMusic, Blink, Color, CommandSink, DecMode, DeviceControlString, Direction, EraseInDisplayMode, EraseInLineMode, ErrorLevel, IgsCommand,
    Intensity, OperatingSystemCommand, ParseError, RipCommand, SgrAttribute, SkypixCommand, TerminalCommand, Underline, ViewDataCommand,
};

use crate::{AttributedChar, BitFont, BufferType, EditableScreen, FontSelectionState, MouseMode, Position, SavedCaretState, Sixel, XTERM_256_PALETTE};
/// Adapter that implements CommandSink for any type implementing EditableScreen.
/// This allows icy_parser_core parsers to drive icy_engine's terminal emulation.
pub struct ScreenSink<'a> {
    screen: &'a mut dyn EditableScreen,
}

impl<'a> ScreenSink<'a> {
    pub fn new(screen: &'a mut dyn EditableScreen) -> Self {
        Self { screen }
    }

    /// Get mutable reference to the underlying screen
    pub fn screen_mut(&mut self) -> &mut dyn EditableScreen {
        self.screen
    }

    /// Get reference to the underlying screen
    pub fn screen(&self) -> &dyn EditableScreen {
        self.screen
    }

    /// Get the current caret attribute with inverse_video applied if active
    fn get_display_attribute(&self) -> crate::TextAttribute {
        let mut attr = if self.screen.terminal_state().inverse_video {
            let mut attr = self.screen.caret().attribute;
            let fg = attr.get_foreground();
            let bg = attr.get_background();
            attr.set_foreground(bg);
            attr.set_background(fg);
            attr
        } else {
            self.screen.caret().attribute
        };

        if self.screen.terminal_state().ice_colors && attr.is_blinking() && attr.background_color < 8 {
            attr.set_is_blinking(false);
            attr.background_color += 8;
        }
        attr
    }

    fn set_font_selection_success(&mut self, slot: usize) {
        self.screen.terminal_state_mut().font_selection_state = FontSelectionState::Success;
        self.screen.caret_mut().set_font_page(slot);

        if self.screen.caret().attribute.is_blinking() && self.screen.caret().attribute.is_bold() {
            self.screen.terminal_state_mut().high_intensity_blink_attribute_font_slot = slot;
        } else if self.screen.caret().attribute.is_blinking() {
            self.screen.terminal_state_mut().blink_attribute_font_slot = slot;
        } else if self.screen.caret().attribute.is_bold() {
            self.screen.terminal_state_mut().high_intensity_attribute_font_slot = slot;
        } else {
            self.screen.terminal_state_mut().normal_attribute_font_slot = slot;
        }
    }

    fn apply_sgr(&mut self, sgr: SgrAttribute) {
        let attr = &mut self.screen.caret_mut().attribute;
        match sgr {
            SgrAttribute::Reset => {
                self.screen.sgr_reset();
            }
            SgrAttribute::Intensity(intensity) => match intensity {
                Intensity::Normal => {
                    attr.set_is_bold(false);
                    attr.set_is_faint(false);
                }
                Intensity::Bold => {
                    attr.set_is_bold(true);
                    attr.set_is_faint(false);
                }
                Intensity::Faint => {
                    attr.set_is_bold(false);
                    attr.set_is_faint(true);
                }
            },
            SgrAttribute::Italic(on) => attr.set_is_italic(on),
            SgrAttribute::Fraktur => {
                // Fraktur not directly supported, treat as italic
                attr.set_is_italic(true);
            }
            SgrAttribute::Underline(underline) => match underline {
                Underline::Off => attr.set_is_underlined(false),
                Underline::Single | Underline::Double => attr.set_is_underlined(true),
            },
            SgrAttribute::CrossedOut(on) => attr.set_is_crossed_out(on),
            SgrAttribute::Blink(blink) => match blink {
                Blink::Off => attr.set_is_blinking(false),
                Blink::Slow | Blink::Rapid => attr.set_is_blinking(true),
            },
            SgrAttribute::Inverse(on) => {
                // Inverse video: swap foreground and background colors
                self.screen.terminal_state_mut().inverse_video = on;
                // Note: turning off inverse would require saving the original colors
                // This is a limitation of the current attribute system
            }
            SgrAttribute::Concealed(on) => attr.set_is_concealed(on),
            SgrAttribute::Frame(frame) => {
                // Frame not directly supported in TextAttribute
                // Could be extended if needed
                let _ = frame;
            }
            SgrAttribute::Overlined(on) => {
                // Overline not directly supported in TextAttribute
                let _ = on;
            }
            SgrAttribute::Font(font) => {
                attr.set_font_page(font as usize);
            }
            SgrAttribute::Foreground(color) => {
                match color {
                    Color::Base(c) => {
                        let col = {
                            let _ = attr;
                            (c as u32) % self.screen.max_base_colors()
                        };
                        self.screen.caret_mut().attribute.set_foreground(col);
                        return;
                    }
                    Color::Extended(c) => {
                        let color_val = {
                            let _ = attr;
                            let pal = XTERM_256_PALETTE[c as usize].1.clone();
                            self.screen.palette_mut().insert_color(pal)
                        };
                        self.screen.caret_mut().set_foreground(color_val);
                        return;
                    }
                    Color::Rgb(r, g, b) => {
                        // Need to release attr borrow before calling palette_mut
                        let color_val = {
                            // This scope ends the attr borrow
                            let _ = attr;
                            self.screen.palette_mut().insert_color_rgb(r, g, b)
                        };
                        // Re-borrow attr
                        self.screen.caret_mut().attribute.set_foreground(color_val);
                        return; // Early return to avoid code after match
                    }
                    Color::Default => {
                        attr.set_foreground(7);
                    }
                };
            }
            SgrAttribute::Background(color) => {
                match color {
                    Color::Base(c) => {
                        let col = {
                            let _ = attr;
                            (c as u32) % self.screen.max_base_colors()
                        };
                        self.screen.caret_mut().attribute.set_background(col);
                    }
                    Color::Extended(c) => {
                        let color_val = {
                            let _ = attr;
                            let pal = XTERM_256_PALETTE[c as usize].1.clone();
                            self.screen.palette_mut().insert_color(pal)
                        };
                        self.screen.caret_mut().set_background(color_val);
                        return;
                    }
                    Color::Rgb(r, g, b) => {
                        // Need to release attr borrow before calling palette_mut
                        let color_val = {
                            // This scope ends the attr borrow
                            let _ = attr;
                            self.screen.palette_mut().insert_color_rgb(r, g, b)
                        };
                        // Re-borrow attr
                        self.screen.caret_mut().attribute.set_background(color_val);
                        return; // Early return to avoid code after match
                    }
                    Color::Default => {
                        attr.set_background(0);
                    }
                };
            }
            SgrAttribute::IdeogramUnderline
            | SgrAttribute::IdeogramDoubleUnderline
            | SgrAttribute::IdeogramOverline
            | SgrAttribute::IdeogramDoubleOverline
            | SgrAttribute::IdeogramStress
            | SgrAttribute::IdeogramAttributesOff => {
                // Ideogram attributes not supported
            }
        }
    }

    fn set_dec_private_mode(&mut self, mode: DecMode, enabled: bool) {
        match mode {
            DecMode::OriginMode => {
                self.screen.terminal_state_mut().origin_mode = if enabled {
                    crate::OriginMode::WithinMargins
                } else {
                    crate::OriginMode::UpperLeftCorner
                };
            }
            DecMode::AutoWrap => {
                self.screen.terminal_state_mut().auto_wrap_mode = if enabled {
                    crate::AutoWrapMode::AutoWrap
                } else {
                    crate::AutoWrapMode::NoWrap
                };
            }
            DecMode::CursorVisible => {
                self.screen.caret_mut().visible = enabled;
            }
            DecMode::Inverse => {
                // Screen-wide inverse mode: swap foreground and background
                // Note: This is a simplified implementation
                // A full implementation would need to track this mode separately
                let attr = &mut self.screen.caret_mut().attribute;
                if enabled {
                    let fg = attr.get_foreground();
                    let bg = attr.get_background();
                    attr.set_foreground(bg);
                    attr.set_background(fg);
                }
            }
            DecMode::IceColors => {
                self.screen.terminal_state_mut().ice_colors = enabled;
            }
            DecMode::CursorBlinking => {
                self.screen.caret_mut().blinking = enabled;
            }
            DecMode::SmoothScroll => {
                self.screen.terminal_state_mut().scroll_state = if enabled {
                    crate::TerminalScrolling::Smooth
                } else {
                    crate::TerminalScrolling::Fast
                };
            }
            DecMode::LeftRightMargin => {
                self.screen.terminal_state_mut().set_dec_left_right_margins(enabled);
            }
            DecMode::X10Mouse => {
                self.screen.terminal_state_mut().set_mouse_mode(MouseMode::X10);
            }
            DecMode::VT200Mouse => {
                self.screen.terminal_state_mut().set_mouse_mode(MouseMode::VT200);
            }
            DecMode::VT200HighlightMouse => {
                self.screen.terminal_state_mut().set_mouse_mode(MouseMode::VT200_Highlight);
            }
            DecMode::ButtonEventMouse => {
                self.screen.terminal_state_mut().set_mouse_mode(MouseMode::ButtonEvents);
            }
            DecMode::AnyEventMouse => {
                self.screen.terminal_state_mut().set_mouse_mode(MouseMode::AnyEvents);
            }
            DecMode::FocusEvent => {
                self.screen.terminal_state_mut().mouse_state.focus_out_event_enabled = enabled;
            }
            DecMode::AlternateScroll => {
                self.screen.terminal_state_mut().mouse_state.alternate_scroll_enabled = enabled;
            }
            DecMode::ExtendedMouseUTF8 => {
                self.screen.terminal_state_mut().mouse_state.extended_mode = crate::ExtMouseMode::ExtendedUTF8;
            }
            DecMode::ExtendedMouseSGR => {
                self.screen.terminal_state_mut().mouse_state.extended_mode = crate::ExtMouseMode::SGR;
            }
            DecMode::ExtendedMouseURXVT => {
                self.screen.terminal_state_mut().mouse_state.extended_mode = crate::ExtMouseMode::URXVT;
            }
            DecMode::ExtendedMousePixel => {
                self.screen.terminal_state_mut().mouse_state.extended_mode = crate::ExtMouseMode::PixelPosition;
            }
        }
    }

    fn set_ansi_mode(&mut self, mode: AnsiMode, enabled: bool) {
        match mode {
            AnsiMode::InsertReplace => {
                self.screen.caret_mut().insert_mode = enabled;
            }
        }
    }

    fn vd_fill_to_eol(&mut self) {
        if self.screen.caret_position().x <= 0 {
            return;
        }
        let sx = self.screen.caret_position().x;
        let sy = self.screen.caret_position().y;

        let prev_attr = self.screen.get_char((sx, sy).into()).attribute;

        // Fill remaining characters on the line that match the previous attribute
        // This handles cases like double-height where we need to update all following characters
        for x in sx..self.screen.terminal_state().get_width() {
            let p = Position::new(x, sy);
            let mut ch = self.screen.get_char(p);

            // Stop if we hit a character with a different attribute
            // (this means a new color/style command was encountered)
            if ch.attribute != prev_attr {
                break;
            }

            // Update this character with the new caret attribute
            ch.attribute = self.screen.caret().attribute;
            self.screen.set_char(p, ch);
        }
    }
}

impl<'a> CommandSink for ScreenSink<'a> {
    fn print(&mut self, text: &[u8]) {
        match self.screen.buffer_type() {
            BufferType::Unicode => {
                // UTF-8 mode: decode multi-byte sequences
                // Get any incomplete sequence from previous call and append new data
                let utf8_buf = &mut self.screen.terminal_state_mut().utf8_buffer;
                utf8_buf.extend_from_slice(text);

                // Take ownership of buffer to avoid borrow issues
                let buffer = std::mem::take(utf8_buf);
                let mut i = 0;

                while i < buffer.len() {
                    let remaining = &buffer[i..];

                    // Try to decode UTF-8 starting at position i
                    match std::str::from_utf8(remaining) {
                        Ok(valid_str) => {
                            // All remaining bytes form valid UTF-8
                            for ch in valid_str.chars() {
                                let attr_char = AttributedChar::new(ch, self.get_display_attribute());
                                self.screen.print_char(attr_char);
                            }
                            i = buffer.len(); // Consumed everything
                        }
                        Err(e) => {
                            // Process valid part first
                            if e.valid_up_to() > 0 {
                                let valid_str = std::str::from_utf8(&remaining[..e.valid_up_to()]).unwrap();
                                for ch in valid_str.chars() {
                                    let attr_char = AttributedChar::new(ch, self.get_display_attribute());
                                    self.screen.print_char(attr_char);
                                }
                                i += e.valid_up_to();
                            }

                            if let Some(error_len) = e.error_len() {
                                // Invalid UTF-8 sequence - output replacement char
                                let attr_char = AttributedChar::new('\u{FFFD}', self.get_display_attribute());
                                self.screen.print_char(attr_char);
                                i += error_len;
                            } else {
                                // Incomplete sequence at end - keep for next call
                                break;
                            }
                        }
                    }
                }

                // Keep any incomplete sequence for next call
                let utf8_buf = &mut self.screen.terminal_state_mut().utf8_buffer;
                if i < buffer.len() {
                    *utf8_buf = buffer[i..].to_vec();
                } else {
                    utf8_buf.clear();
                }
            }
            _ => {
                // Legacy mode: treat each byte as a character (CP437, Petscii, Atascii, Viewdata)
                for &byte in text {
                    let ch = AttributedChar::new(byte as char, self.get_display_attribute());
                    self.screen.print_char(ch);
                }
            }
        }
    }

    fn emit(&mut self, cmd: TerminalCommand) {
        match cmd {
            // Basic control characters
            TerminalCommand::CarriageReturn => {
                if self.screen.terminal_state().cr_is_if {
                    self.screen.lf();
                } else {
                    self.screen.cr();
                }
            }
            TerminalCommand::LineFeed => {
                if !self.screen.terminal_state().cr_is_if {
                    self.screen.lf();
                }
            }
            TerminalCommand::Backspace => {
                self.screen.bs();
            }
            TerminalCommand::Tab => {
                self.screen.tab_forward();
            }
            TerminalCommand::FormFeed => {
                self.screen.ff();
            }
            TerminalCommand::Bell => {
                // Bell is typically handled by the application layer
            }
            TerminalCommand::Delete => {
                self.screen.del();
            }

            // Cursor movement
            TerminalCommand::CsiMoveCursor(direction, n) => {
                let n = n as i32;
                match direction {
                    Direction::Up => self.screen.up(n, false),
                    Direction::Down => self.screen.down(n, false),
                    Direction::Left => self.screen.left(n, false),
                    Direction::Right => self.screen.right(n, false),
                }
            }
            TerminalCommand::CsiCursorNextLine(n) => {
                for _ in 0..n {
                    self.screen.next_line(false);
                }
            }
            TerminalCommand::CsiCursorPreviousLine(n) => {
                self.screen.up(n as i32, false);
                self.screen.cr();
            }
            TerminalCommand::CsiCursorHorizontalAbsolute(col) => {
                let col = (col as i32).saturating_sub(1).max(0);
                let mut pos = self.screen.caret_position();
                pos.x = col;
                self.screen.set_caret_position(pos);
                self.screen.limit_caret_pos(false);
            }
            TerminalCommand::CsiCursorPosition(row, col) => {
                let upper_left = self.screen.upper_left_position();
                let row = upper_left.y + (row as i32).saturating_sub(1).max(0);
                let col = upper_left.x + (col as i32).saturating_sub(1).max(0);
                self.screen.set_caret_position(Position::new(col, row));
                self.screen.limit_caret_pos(false);
            }

            // Erase operations
            TerminalCommand::CsiEraseInDisplay(mode) => match mode {
                EraseInDisplayMode::CursorToEnd => {
                    self.screen.clear_buffer_down();
                }
                EraseInDisplayMode::StartToCursor => {
                    self.screen.clear_buffer_up();
                }
                EraseInDisplayMode::All => {
                    self.screen.clear_screen();
                }
                EraseInDisplayMode::AllAndScrollback => {
                    self.screen.clear_screen();
                    self.screen.clear_scrollback();
                }
            },
            TerminalCommand::CsiEraseInLine(mode) => match mode {
                EraseInLineMode::CursorToEnd => {
                    self.screen.clear_line_end();
                }
                EraseInLineMode::StartToCursor => {
                    self.screen.clear_line_start();
                }
                EraseInLineMode::All => {
                    self.screen.clear_line();
                }
            },

            // Scrolling
            TerminalCommand::CsiScroll(direction, n) => {
                for _ in 0..n {
                    match direction {
                        Direction::Up => self.screen.scroll_up(),
                        Direction::Down => self.screen.scroll_down(),
                        Direction::Left => self.screen.scroll_left(),
                        Direction::Right => self.screen.scroll_right(),
                    }
                }
            }

            // Attributes
            TerminalCommand::CsiSelectGraphicRendition(sgr) => {
                self.apply_sgr(sgr);
            }

            // Character/Line operations
            TerminalCommand::CsiInsertCharacter(n) => {
                for _ in 0..n {
                    self.screen.ins();
                }
            }
            TerminalCommand::CsiDeleteCharacter(n) => {
                for _ in 0..n {
                    self.screen.del();
                }
            }
            TerminalCommand::CsiEraseCharacter(n) => {
                let pos = self.screen.caret_position();
                let blank = AttributedChar::new(' ', self.get_display_attribute());
                for i in 0..n as i32 {
                    let x = pos.x + i;
                    if x < self.screen.get_width() {
                        self.screen.set_char(Position::new(x, pos.y), blank);
                    }
                }
            }
            TerminalCommand::CsiInsertLine(n) => {
                for _ in 0..n {
                    self.screen.insert_terminal_line(self.screen.caret_position().y);
                }
            }
            TerminalCommand::CsiDeleteLine(n) => {
                for _ in 0..n {
                    self.screen.remove_terminal_line(self.screen.caret_position().y);
                }
            }

            // Vertical positioning
            TerminalCommand::CsiLinePositionAbsolute(line) => {
                let upper_left = self.screen.upper_left_position();
                let line = upper_left.y + (line as i32).saturating_sub(1).max(0);
                let mut pos = self.screen.caret_position();
                pos.y = line;
                self.screen.set_caret_position(pos);
                self.screen.limit_caret_pos(false);
            }
            TerminalCommand::CsiLinePositionForward(n) => {
                self.screen.down(n as i32, false);
            }
            TerminalCommand::CsiCharacterPositionForward(n) => {
                self.screen.right(n as i32, false);
            }
            TerminalCommand::CsiHorizontalPositionAbsolute(col) => {
                let upper_left = self.screen.upper_left_position();
                let col = upper_left.x + (col as i32).saturating_sub(1).max(0);
                let mut pos = self.screen.caret_position();
                pos.x = col;
                self.screen.set_caret_position(pos);
                self.screen.limit_caret_pos(false);
            }

            // Tab operations
            TerminalCommand::CsiClearTabulation => {
                let col = self.screen.caret_position().x;
                self.screen.terminal_state_mut().remove_tab_stop(col);
            }
            TerminalCommand::CsiClearAllTabs => {
                self.screen.terminal_state_mut().clear_tab_stops();
            }
            TerminalCommand::CsiCursorLineTabulationForward(num) => {
                (0..num).for_each(|_| {
                    let x = self.screen.terminal_state().next_tab_stop(self.screen.caret_position().x);
                    let mut pos = self.screen.caret_position();
                    pos.x = x;
                    self.screen.set_caret_position(pos);
                });
            }
            TerminalCommand::CsiCursorBackwardTabulation(num) => {
                (0..num).for_each(|_| {
                    let x = self.screen.terminal_state().prev_tab_stop(self.screen.caret_position().x);
                    let mut pos = self.screen.caret_position();
                    pos.x = x;
                    self.screen.set_caret_position(pos);
                });
            }

            // Cursor save/restore
            TerminalCommand::CsiSaveCursorPosition => {
                *self.screen.saved_caret_pos() = self.screen.caret_position();
            }
            TerminalCommand::CsiRestoreCursorPosition => {
                let pos = *self.screen.saved_caret_pos();
                self.screen.set_caret_position(pos);
            }

            TerminalCommand::EscSaveCursor => {
                // DECSC - Save Cursor
                *self.screen.saved_cursor_state() = SavedCaretState {
                    caret: self.screen.caret().clone(),
                    origin_mode: self.screen.terminal_state().origin_mode,
                    auto_wrap_mode: self.screen.terminal_state().auto_wrap_mode,
                };
            }

            TerminalCommand::EscRestoreCursor => {
                let state = self.screen.saved_cursor_state().clone();
                self.screen.terminal_state_mut().origin_mode = state.origin_mode;
                self.screen.terminal_state_mut().auto_wrap_mode = state.auto_wrap_mode;
                *self.screen.caret_mut() = state.caret;
            }

            // Terminal resize
            TerminalCommand::CsiResizeTerminal(height, width) => {
                self.screen.set_size(crate::Size::new(width as i32, height as i32));
            }

            // Special keys (typically handled by application)
            TerminalCommand::CsiSpecialKey(_key) => {}

            // DEC Private Modes
            TerminalCommand::CsiDecSetMode(mode, enabled) => {
                self.set_dec_private_mode(mode, enabled);
            }

            // ANSI Modes
            TerminalCommand::CsiSetMode(mode, enabled) => {
                self.set_ansi_mode(mode, enabled);
            }

            // Caret style
            TerminalCommand::CsiSetCaretStyle(blinking, shape) => {
                let caret = self.screen.caret_mut();
                caret.blinking = blinking;
                caret.shape = shape;
            }

            // ESC sequences (non-CSI)
            TerminalCommand::EscIndex => {
                self.screen.index();
            }
            TerminalCommand::EscNextLine => {
                self.screen.next_line(true);
            }
            TerminalCommand::EscSetTab => {
                let col = self.screen.caret().x;
                self.screen.terminal_state_mut().set_tab_at(col);
            }
            TerminalCommand::EscReverseIndex => {
                self.screen.up(1, true);
            }
            TerminalCommand::EscReset => {
                self.screen.reset_terminal();
            }

            // Commands not yet fully mapped
            TerminalCommand::SetFontPage(page) => {
                self.screen.caret_mut().set_font_page(page);
            }
            TerminalCommand::CsiFontSelection { slot: _slot, font_number } => {
                let nr = font_number as usize;
                if self.screen().get_font(nr).is_some() {
                    self.set_font_selection_success(nr);
                }
                match BitFont::from_ansi_font_page(nr) {
                    Ok(font) => {
                        self.screen_mut().set_font(nr, font);
                        self.set_font_selection_success(nr);
                    }
                    Err(err) => {
                        log::error!("failed font selection: {}", err);
                        self.screen_mut().terminal_state_mut().font_selection_state = FontSelectionState::Failure;
                    }
                }
            }
            TerminalCommand::CsiSelectCommunicationSpeed(_, _) => {}
            TerminalCommand::CsiFillRectangularArea { .. } => {}
            TerminalCommand::CsiEraseRectangularArea { .. } => {}
            TerminalCommand::CsiSelectiveEraseRectangularArea { .. } => {}
            TerminalCommand::CsiSetScrollingRegion { top, bottom, left, right } => {
                let top = (top as i32).saturating_sub(1).max(0);
                let bottom = (bottom as i32).saturating_sub(1).max(0);
                let left = (left as i32).saturating_sub(1).max(0);
                let right = (right as i32).saturating_sub(1).max(0);

                self.screen.terminal_state_mut().set_margins_top_bottom(top, bottom);
                self.screen.terminal_state_mut().set_margins_left_right(left, right);
                let pos = self.screen.upper_left_position();
                self.screen.set_caret_position(pos);
            }
            TerminalCommand::SetTopBottomMargin { top, bottom } => {
                // CSI = {top};{bottom}r - Set margins
                let top = (top as i32).saturating_sub(1).max(0);
                let bottom = (bottom as i32).saturating_sub(1).max(0);
                self.screen.terminal_state_mut().set_margins_top_bottom(top, bottom);
                let pos = self.screen.upper_left_position();
                self.screen.set_caret_position(pos);
            }
            TerminalCommand::ResetMargins => {
                self.screen.terminal_state_mut().clear_margins_left_right();
                self.screen.terminal_state_mut().clear_margins_top_bottom();
                self.screen.set_caret_position(Position::default());
            }
            TerminalCommand::ResetLeftAndRightMargin { left, right } => {
                let width = self.screen.get_width();

                let (current_left, current_right) = self.screen.terminal_state().get_margins_left_right().unwrap_or((0, width - 1));

                let mut new_left_1b = left as i32;
                let mut new_right_1b = right as i32;

                if new_left_1b == 0 {
                    new_left_1b = current_left + 1;
                }
                if new_right_1b == 0 {
                    new_right_1b = current_right + 1;
                }

                let new_left = new_left_1b.saturating_sub(1);
                let new_right = new_right_1b.saturating_sub(1);

                if new_left >= 0 && new_right >= 0 && new_left < width && new_right < width && new_left < new_right {
                    self.screen.terminal_state_mut().set_margins_left_right(new_left, new_right);
                    let pos = self.screen.upper_left_position();
                    self.screen.set_caret_position(pos);
                }
            }

            TerminalCommand::CsiEqualsSetSpecificMargins(margin_type, value) => {
                // CSI = {margin_type};{value}m - Set specific margin
                let n = (value as i32).saturating_sub(1).max(0);

                use icy_parser_core::MarginType;
                match margin_type {
                    MarginType::Top => {
                        let bottom = if let Some((_, b)) = self.screen.terminal_state().get_margins_top_bottom() {
                            b
                        } else {
                            self.screen.get_height() - 1
                        };
                        self.screen.terminal_state_mut().set_margins_top_bottom(n, bottom);
                    }
                    MarginType::Bottom => {
                        let top = if let Some((t, _)) = self.screen.terminal_state().get_margins_top_bottom() {
                            t
                        } else {
                            0
                        };
                        self.screen.terminal_state_mut().set_margins_top_bottom(top, n);
                    }
                    MarginType::Left => {
                        let right = if let Some((_, r)) = self.screen.terminal_state().get_margins_left_right() {
                            r
                        } else {
                            self.screen.get_width() - 1
                        };
                        self.screen.terminal_state_mut().set_margins_left_right(n, right);
                    }
                    MarginType::Right => {
                        let left = if let Some((l, _)) = self.screen.terminal_state().get_margins_left_right() {
                            l
                        } else {
                            0
                        };
                        self.screen.terminal_state_mut().set_margins_left_right(left, n);
                    }
                }
            }
            TerminalCommand::ScrollArea {
                direction,
                num_lines,
                top,
                left,
                bottom,
                right,
            } => {
                // Scroll a rectangular area
                let _ = (direction, num_lines, top, left, bottom, right);
                // TODO:
                //self.screen.scroll_area(direction, num_lines as i32, top as i32, left as i32, bottom as i32, right as i32);
            }
            TerminalCommand::AvatarClearArea { attr, lines, columns } => {
                // Avatar clear area
                let _ = (attr, lines, columns);
                // TODO: Implement Avatar clear area
            }
            TerminalCommand::AvatarInitArea { attr, ch, lines, columns } => {
                // Avatar init area
                let _ = (attr, ch, lines, columns);
                // TODO: Implement Avatar init area
            }
        }
    }

    fn emit_rip(&mut self, cmd: RipCommand) {
        self.screen.handle_rip_command(cmd);
    }

    fn emit_skypix(&mut self, cmd: SkypixCommand) {
        self.screen.handle_skypix_command(cmd);
    }

    fn emit_igs(&mut self, cmd: IgsCommand) {
        self.screen.handle_igs_command(cmd);
    }

    fn emit_view_data(&mut self, cmd: ViewDataCommand) -> bool {
        let current_row = self.screen.caret_position().y;
        if current_row != self.screen.terminal_state_mut().vd_last_row {
            // For Viewdata, default foreground is white (color 7), not black
            self.screen.caret_mut().attribute.set_foreground(7);
            self.screen.caret_mut().attribute.set_background(0);
            self.screen.terminal_state_mut().vd_last_row = current_row;
        }

        match cmd {
            ViewDataCommand::ViewDataClearScreen => {
                // Preserve caret visibility (e.g., if hidden by 0x14)
                let was_visible = self.screen.caret().visible;
                self.screen.reset_terminal();
                self.screen.caret_mut().visible = was_visible;
                self.screen.clear_screen();
                // For Viewdata, default foreground is white (color 7), not black
                self.screen.caret_mut().attribute.set_foreground(7);
                self.screen.caret_mut().attribute.set_background(0);
                self.screen.terminal_state_mut().vd_last_row = 0;
            }
            ViewDataCommand::FillToEol => {
                self.vd_fill_to_eol();
            }
            ViewDataCommand::DoubleHeight(enabled) => {
                self.screen.caret_mut().attribute.set_is_double_height(enabled);
                self.vd_fill_to_eol();
            }
            ViewDataCommand::ResetRowColors => {
                // For Viewdata, default foreground is white (color 7), not black
                self.screen.caret_mut().attribute.set_foreground(7);
                self.screen.caret_mut().attribute.set_background(0);
                self.screen.terminal_state_mut().vd_last_row = self.screen.caret_position().y;
            }
            ViewDataCommand::CheckAndResetOnRowChange => {}
            ViewDataCommand::MoveCaret(direction) => match direction {
                Direction::Up => {
                    let current_y = self.screen.caret_position().y;
                    let y = if current_y > 0 {
                        current_y.saturating_sub(1)
                    } else {
                        self.screen.terminal_state().get_height() - 1
                    };
                    let mut pos = self.screen.caret_position();
                    pos.y = y;
                    self.screen.set_caret_position(pos);
                }
                Direction::Down => {
                    let mut pos = self.screen.caret_position();
                    pos.y = pos.y + 1;
                    if pos.y >= self.screen.terminal_state().get_height() {
                        pos.y = 0;
                    }
                    self.screen.set_caret_position(pos);
                }
                Direction::Left => {
                    let pos = self.screen.caret_position();
                    if pos.x > 0 {
                        let mut new_pos = pos;
                        new_pos.x = pos.x.saturating_sub(1);
                        self.screen.set_caret_position(new_pos);
                    } else {
                        let x = self.screen.terminal_state().get_width().saturating_sub(1);
                        self.screen.caret_mut().x = x;
                        self.emit_view_data(ViewDataCommand::MoveCaret(Direction::Up));
                    }
                }
                Direction::Right => {
                    let x = self.screen.caret().x;
                    self.screen.caret_mut().x = x + 1;
                    if self.screen.caret().x >= self.screen.terminal_state().get_width() {
                        self.screen.caret_mut().x = 0;
                        self.emit_view_data(ViewDataCommand::MoveCaret(Direction::Down));
                        return true;
                    }
                }
            },
            ViewDataCommand::SetBgToFg => {
                let fg = self.screen.caret_mut().attribute.get_foreground();
                self.screen.caret_mut().attribute.set_background(fg);
            }
            ViewDataCommand::SetChar(ch) => {
                let ch = AttributedChar::new(ch as char, self.get_display_attribute());
                self.screen.set_char(self.screen.caret_position(), ch);
            }
        }
        false
    }

    fn device_control(&mut self, dcs: DeviceControlString) {
        // DCS handling for font loading and sixel
        match dcs {
            DeviceControlString::LoadFont(slot, data) => {
                // Load custom font from decoded base64 data
                match crate::BitFont::from_bytes(format!("custom font {}", slot), &data) {
                    Ok(font) => {
                        log::info!("Loaded custom font into slot {}", slot);
                        self.screen.set_font(slot, font);
                    }
                    Err(err) => {
                        log::error!("Failed to load custom font: {}", err);
                    }
                }
            }
            DeviceControlString::Sixel {
                aspect_ratio,
                zero_color,
                grid_size,
                sixel_data,
            } => match Sixel::parse_from(aspect_ratio, zero_color, grid_size, &sixel_data) {
                Ok(sixel) => {
                    let pos = self.screen.caret_position();
                    self.screen.add_sixel(pos, sixel);
                }
                Err(err) => {
                    log::error!("Error loading sixel: {}", err);
                }
            },
        }
    }

    fn operating_system_command(&mut self, osc: OperatingSystemCommand) {
        // OSC handling - typically for window title, hyperlinks, etc.
        match osc {
            OperatingSystemCommand::SetTitle(title) => {
                if let Ok(title_str) = std::str::from_utf8(&title) {
                    log::debug!("OSC: Set title to '{}'", title_str);
                    // TODO: Add SetTitle callback variant
                }
            }
            OperatingSystemCommand::SetIconName(name) => {
                if let Ok(name_str) = std::str::from_utf8(&name) {
                    log::debug!("OSC: Set icon name to '{}'", name_str);
                }
            }
            OperatingSystemCommand::SetWindowTitle(title) => {
                if let Ok(title_str) = std::str::from_utf8(&title) {
                    log::debug!("OSC: Set window title to '{}'", title_str);
                }
            }
            OperatingSystemCommand::SetPaletteColor(index, r, g, b) => {
                // Set palette color
                self.screen.palette_mut().set_color_rgb(index as u32, r, g, b);
                log::debug!("OSC: Set palette color {} to RGB({}, {}, {})", index, r, g, b);
            }
            OperatingSystemCommand::Hyperlink { params, uri } => {
                if let (Ok(_params_str), Ok(uri_str)) = (std::str::from_utf8(&params), std::str::from_utf8(&uri)) {
                    /*
                    if uri_str.is_empty() {
                        self.screen.caret_mut().attribute.set_is_underlined(false);
                        let cp = self.screen.caret_position();
                        if cp.y == p.position.y {
                            p.length = cp.x - p.position.x;
                        } else {
                            p.length = self.screen.terminal_state().get_width() - p.position.x + (cp.y - p.position.y) * self.screen.terminal_state().get_width() + p.position.x;
                        }
                        self.screen.add_hyperlink(p);
                    } else {*/
                    self.screen.caret_mut().attribute.set_is_underlined(true);
                    self.screen.add_hyperlink(crate::HyperLink {
                        url: Some(uri_str.to_string()),
                        position: self.screen.caret_position(),
                        length: 0,
                    });
                }
            }
        }
    }

    fn aps(&mut self, _data: &[u8]) {
        // APS sequences not commonly used
    }

    fn play_music(&mut self, _music: AnsiMusic) {
        // Push music playback callback to be handled by application layer
    }

    fn report_error(&mut self, error: ParseError, _level: ErrorLevel) {
        log::error!("Parser error: {:?}", error);
    }
}
