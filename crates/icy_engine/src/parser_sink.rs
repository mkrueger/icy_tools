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
    AnsiMode, AnsiMusic, Blink, Color, CommandSink, DecPrivateMode, DeviceControlString, Direction, EraseInDisplayMode, EraseInLineMode, IgsCommand, Intensity,
    OperatingSystemCommand, ParseError, RipCommand, SgrAttribute, SkypixCommand, TerminalCommand, Underline,
};

use crate::{AttributedChar, EditableScreen, Position, SavedCaretState};
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

    fn apply_sgr(&mut self, sgr: SgrAttribute) {
        let attr = &mut self.screen.caret_mut().attribute;

        match sgr {
            SgrAttribute::Reset => {
                self.screen.caret_default_colors();
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
                if on {
                    let fg = attr.get_foreground();
                    let bg = attr.get_background();
                    attr.set_foreground(bg);
                    attr.set_background(fg);
                }
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
                        attr.set_foreground(c as u32);
                    }
                    Color::Extended(c) => {
                        attr.set_foreground(c as u32);
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
                        attr.set_background(c as u32);
                    }
                    Color::Extended(c) => {
                        attr.set_background(c as u32);
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

    fn set_dec_private_mode(&mut self, mode: DecPrivateMode, enabled: bool) {
        match mode {
            DecPrivateMode::OriginMode => {
                self.screen.terminal_state_mut().origin_mode = if enabled {
                    crate::OriginMode::WithinMargins
                } else {
                    crate::OriginMode::UpperLeftCorner
                };
            }
            DecPrivateMode::AutoWrap => {
                self.screen.terminal_state_mut().auto_wrap_mode = if enabled {
                    crate::AutoWrapMode::AutoWrap
                } else {
                    crate::AutoWrapMode::NoWrap
                };
            }
            DecPrivateMode::CursorVisible => {
                self.screen.caret_mut().visible = enabled;
            }
            DecPrivateMode::Inverse => {
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
            DecPrivateMode::IceColors => {
                *self.screen.ice_mode_mut() = if enabled { crate::IceMode::Ice } else { crate::IceMode::Blink };
            }
            DecPrivateMode::CursorBlinking => {
                // Caret doesn't have a blinking field currently
                // Could be extended if needed
            }
            _ => {
                // Other modes not yet implemented
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
}

impl<'a> CommandSink for ScreenSink<'a> {
    fn print(&mut self, text: &[u8]) {
        for &byte in text {
            let ch = AttributedChar::new(byte as char, self.screen.caret().attribute);
            self.screen.print_char(ch);
        }
    }

    fn emit(&mut self, cmd: TerminalCommand) {
        match cmd {
            // Basic control characters
            TerminalCommand::CarriageReturn => {
                self.screen.cr();
            }
            TerminalCommand::LineFeed => {
                self.screen.lf();
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
                    Direction::Up => self.screen.up(n),
                    Direction::Down => self.screen.down(n),
                    Direction::Left => self.screen.left(n),
                    Direction::Right => self.screen.right(n),
                }
            }
            TerminalCommand::CsiCursorNextLine(n) => {
                for _ in 0..n {
                    self.screen.next_line();
                }
            }
            TerminalCommand::CsiCursorPreviousLine(n) => {
                self.screen.up(n as i32);
                self.screen.cr();
            }
            TerminalCommand::CsiCursorHorizontalAbsolute(col) => {
                let col = (col as i32).saturating_sub(1).max(0);
                self.screen.caret_mut().x = col;
                self.screen.limit_caret_pos();
            }
            TerminalCommand::CsiCursorPosition(row, col) => {
                let upper_left = self.screen.upper_left_position();
                let row = upper_left.y + (row as i32).saturating_sub(1).max(0);
                let col = upper_left.x + (col as i32).saturating_sub(1).max(0);
                self.screen.set_caret_position(Position::new(col, row));
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

            // Scrolling region
            TerminalCommand::CsiSetScrollingRegion(top, bottom) => {
                let top = (top as i32).saturating_sub(1).max(0);
                let bottom = (bottom as i32).saturating_sub(1).max(0);
                self.screen.terminal_state_mut().set_margins_top_bottom(top, bottom);
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
                let pos = self.screen.caret().position();
                let blank = AttributedChar::new(' ', self.screen.caret().attribute);
                for i in 0..n as i32 {
                    let x = pos.x + i;
                    if x < self.screen.get_width() {
                        self.screen.set_char(Position::new(x, pos.y), blank);
                    }
                }
            }
            TerminalCommand::CsiInsertLine(n) => {
                for _ in 0..n {
                    self.screen.insert_terminal_line(self.screen.caret().y);
                }
            }
            TerminalCommand::CsiDeleteLine(n) => {
                for _ in 0..n {
                    self.screen.remove_terminal_line(self.screen.caret().y);
                }
            }

            // Vertical positioning
            TerminalCommand::CsiLinePositionAbsolute(line) => {
                let upper_left = self.screen.upper_left_position();
                let line = upper_left.y + (line as i32).saturating_sub(1).max(0);
                self.screen.caret_mut().y = line;
                self.screen.limit_caret_pos();
            }
            TerminalCommand::CsiLinePositionForward(n) => {
                self.screen.down(n as i32);
            }
            TerminalCommand::CsiCharacterPositionForward(n) => {
                self.screen.right(n as i32);
            }
            TerminalCommand::CsiHorizontalPositionAbsolute(col) => {
                let upper_left = self.screen.upper_left_position();
                let col = upper_left.x + (col as i32).saturating_sub(1).max(0);
                self.screen.caret_mut().x = col;
                self.screen.limit_caret_pos();
            }

            // Tab operations
            TerminalCommand::CsiClearTabulation => {
                let col = self.screen.caret().x;
                self.screen.terminal_state_mut().remove_tab_stop(col);
            }
            TerminalCommand::CsiClearAllTabs => {
                self.screen.terminal_state_mut().clear_tab_stops();
            }
            TerminalCommand::CsiCursorLineTabulationForward(num) => {
                (0..num).for_each(|_| {
                    let x = self.screen.terminal_state().next_tab_stop(self.screen.caret().position().x);
                    self.screen.caret_mut().x = x;
                });
            }
            TerminalCommand::CsiCursorBackwardTabulation(num) => {
                (0..num).for_each(|_| {
                    let x = self.screen.terminal_state().prev_tab_stop(self.screen.caret().position().x);
                    self.screen.caret_mut().x = x;
                });
            }

            // Cursor save/restore
            TerminalCommand::CsiSaveCursorPosition => {
                *self.screen.saved_caret_pos() = self.screen.caret().position();
            }
            TerminalCommand::CsiRestoreCursorPosition => {
                let pos = *self.screen.saved_caret_pos();
                self.screen.caret_mut().set_position(pos);
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
            TerminalCommand::CsiDecPrivateModeSet(mode) => {
                self.set_dec_private_mode(mode, true);
            }
            TerminalCommand::CsiDecPrivateModeReset(mode) => {
                self.set_dec_private_mode(mode, false);
            }

            // ANSI Modes
            TerminalCommand::CsiSetMode(mode) => {
                self.set_ansi_mode(mode, true);
            }
            TerminalCommand::CsiResetMode(mode) => {
                self.set_ansi_mode(mode, false);
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
                self.screen.next_line();
            }
            TerminalCommand::EscSetTab => {
                let col = self.screen.caret().x;
                self.screen.terminal_state_mut().set_tab_at(col);
            }
            TerminalCommand::EscReverseIndex => {
                self.screen.reverse_index();
            }
            TerminalCommand::EscReset => {
                self.screen.reset_terminal();
            }

            // Commands not yet fully mapped
            TerminalCommand::CsiFontSelection(_, _) => {}
            TerminalCommand::CsiSelectCommunicationSpeed(_, _) => {}
            TerminalCommand::CsiFillRectangularArea(_, _, _, _, _) => {}
            TerminalCommand::CsiEraseRectangularArea(_, _, _, _) => {}
            TerminalCommand::CsiSelectiveEraseRectangularArea(_, _, _, _) => {}
            TerminalCommand::CsiEqualsSetMargins(top, bottom) => {
                // CSI = {top};{bottom}r - Set margins
                let top = (top as i32).saturating_sub(1).max(0);
                let bottom = (bottom as i32).saturating_sub(1).max(0);
                self.screen.terminal_state_mut().set_margins_top_bottom(top, bottom);
            }
            TerminalCommand::CsiEqualsSetSpecificMargins(top, bottom) => {
                // CSI = {top};{bottom}m - Set specific margins
                let top = (top as i32).saturating_sub(1).max(0);
                let bottom = (bottom as i32).saturating_sub(1).max(0);
                self.screen.terminal_state_mut().set_margins_top_bottom(top, bottom);
            }
        }
    }

    fn emit_rip(&mut self, cmd: RipCommand) {
        self.screen.handle_rip_command(cmd);
    }

    fn emit_skypix(&mut self, cmd: SkypixCommand) {
        // SkyPix commands need graphics integration
        log::debug!("Received SkyPix command: {:?}", cmd);
        // TODO: Convert SkypixCommand to RunSkypixSequence callback
    }

    fn emit_igs(&mut self, cmd: IgsCommand) {
        // IGS commands need graphics integration
        log::debug!("Received IGS command: {:?}", cmd);
        // TODO: Add IGS callback variant to CallbackAction
    }

    fn device_control(&mut self, dcs: DeviceControlString<'_>) {
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
            DeviceControlString::Sixel(vertical_scale, bg_color, sixel_data) => {
                // Parse and render sixel graphics
                let p = self.screen.caret().position();
                let bg = [0xff, bg_color.0, bg_color.1, bg_color.2];

                // Convert sixel_data to String for parsing
                if let Ok(sixel_str) = std::str::from_utf8(sixel_data) {
                    // Spawn thread to parse sixel data (as in original implementation)
                    let sixel_string = sixel_str.to_string();
                    let vertical_scale_i32 = vertical_scale as i32;
                    let handle = std::thread::spawn(move || crate::Sixel::parse_from(p, 1, vertical_scale_i32, bg, &sixel_string));

                    self.screen.push_sixel_thread(handle);
                    log::debug!(
                        "Started sixel parsing thread (scale={}, bg=({},{},{}))",
                        vertical_scale,
                        bg_color.0,
                        bg_color.1,
                        bg_color.2
                    );
                } else {
                    log::error!("Invalid UTF-8 in sixel data");
                }
            }
        }
    }

    fn operating_system_command(&mut self, osc: OperatingSystemCommand<'_>) {
        // OSC handling - typically for window title, hyperlinks, etc.
        match osc {
            OperatingSystemCommand::SetTitle(title) => {
                if let Ok(title_str) = std::str::from_utf8(title) {
                    log::debug!("OSC: Set title to '{}'", title_str);
                    // TODO: Add SetTitle callback variant
                }
            }
            OperatingSystemCommand::SetIconName(name) => {
                if let Ok(name_str) = std::str::from_utf8(name) {
                    log::debug!("OSC: Set icon name to '{}'", name_str);
                }
            }
            OperatingSystemCommand::SetWindowTitle(title) => {
                if let Ok(title_str) = std::str::from_utf8(title) {
                    log::debug!("OSC: Set window title to '{}'", title_str);
                }
            }
            OperatingSystemCommand::SetPaletteColor(index, r, g, b) => {
                // Set palette color
                self.screen.palette_mut().set_color_rgb(index as u32, r, g, b);
                log::debug!("OSC: Set palette color {} to RGB({}, {}, {})", index, r, g, b);
            }
            OperatingSystemCommand::Hyperlink { params, uri } => {
                if let (Ok(_params_str), Ok(uri_str)) = (std::str::from_utf8(params), std::str::from_utf8(uri)) {
                    /*
                    if uri_str.is_empty() {
                        self.screen.caret_mut().attribute.set_is_underlined(false);
                        let cp = self.screen.caret().position();
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
                        position: self.screen.caret().position(),
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

    fn report_error(&mut self, error: ParseError) {
        log::error!("Parser error: {:?}", error);
    }
}
