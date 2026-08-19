//! `CommandSink` implementation for `EditableScreen`
//!
//! This module provides `ScreenSink`, an adapter that implements the `CommandSink` trait
//! from `icy_parser_core` for any type implementing `EditableScreen`. This allows the new
//! parser infrastructure to drive `icy_engine`'s terminal emulation.
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

use base64::{engine::general_purpose, Engine as _};
use icy_parser_core::{
    AnsiMode, AnsiMusic, Blink, Color, CommandSink, DecMode, DeviceControlString, Direction, EraseInDisplayMode, EraseInLineMode, ErrorLevel, IgsCommand,
    Intensity, OperatingSystemCommand, ParseError, RipCommand, SgrAttribute, SkypixCommand, TerminalCommand, Underline, ViewDataCommand, Wrapping,
};
use image::imageops::FilterType;

use crate::{AttributedChar, BitFont, BufferType, EditableScreen, FontSelectionState, MouseMode, Position, SavedCaretState, Sixel, Size};

const MAX_ENCODED_SIZE: usize = 16 * 1024 * 1024;
const MAX_PIXELS: u64 = 16_000_000;

#[derive(Default)]
struct ImageApcOptions {
    sx: u32,
    sy: u32,
    sw: Option<u32>,
    sh: Option<u32>,
    dx: i32,
    dy: i32,
    dw: Option<u32>,
    dh: Option<u32>,
    flip_x: bool,
    flip_y: bool,
    zoom_x: u32,
    zoom_y: u32,
}

fn parse_image_apc_options<'a>(parts: impl Iterator<Item = &'a str>) -> ImageApcOptions {
    let mut options = ImageApcOptions {
        zoom_x: 1,
        zoom_y: 1,
        ..Default::default()
    };
    for part in parts {
        if part.eq_ignore_ascii_case("FX") {
            options.flip_x = true;
            continue;
        }
        if part.eq_ignore_ascii_case("FY") {
            options.flip_y = true;
            continue;
        }
        let Some((key, value)) = part.split_once('=') else { continue };
        match key.to_ascii_uppercase().as_str() {
            "SX" => options.sx = value.parse().unwrap_or(0),
            "SY" => options.sy = value.parse().unwrap_or(0),
            "SW" => options.sw = value.parse().ok(),
            "SH" => options.sh = value.parse().ok(),
            "DX" => options.dx = value.parse().unwrap_or(0),
            "DY" => options.dy = value.parse().unwrap_or(0),
            "DW" => options.dw = value.parse().ok(),
            "DH" => options.dh = value.parse().ok(),
            "ZX" => options.zoom_x = value.parse().unwrap_or(0),
            "ZY" => options.zoom_y = value.parse().unwrap_or(0),
            _ => {}
        }
    }
    options
}

/// Decodes and places an inline image payload.
///
/// Takes no screen reference so callers can run the decode without holding the render lock.
/// `screen_size` is in characters; `options` is the `;`-separated argument list without the payload.
pub fn decode_image_blob(bytes: &[u8], is_jxl: bool, options: &str, font: Size, screen_size: Size) -> Option<(Position, Sixel)> {
    let options = parse_image_apc_options(options.split([';', ' ']).filter(|part| !part.is_empty()));

    let mut image = if is_jxl {
        let decoder = jxl_oxide::integration::JxlDecoder::new(std::io::Cursor::new(bytes)).ok()?;
        image::DynamicImage::from_decoder(decoder).ok()?
    } else {
        image::load_from_memory_with_format(bytes, image::ImageFormat::Pnm).ok()?
    };
    if u64::from(image.width()) * u64::from(image.height()) > MAX_PIXELS {
        log::warn!("Ignoring oversized inline image");
        return None;
    }

    if options.sx >= image.width() || options.sy >= image.height() {
        return None;
    }
    let width = options.sw.unwrap_or(image.width() - options.sx).min(image.width() - options.sx);
    let height = options.sh.unwrap_or(image.height() - options.sy).min(image.height() - options.sy);
    if width == 0 || height == 0 {
        return None;
    }
    image = image.crop_imm(options.sx, options.sy, width, height);

    if options.flip_x {
        image = image.fliph();
    }
    if options.flip_y {
        image = image.flipv();
    }

    if let (Some(width), Some(height)) = (options.dw, options.dh) {
        if width == 0 || height == 0 || u64::from(width) * u64::from(height) > MAX_PIXELS {
            return None;
        }
        image = image.resize_exact(width, height, FilterType::Nearest);
    } else {
        if options.zoom_x == 0 || options.zoom_y == 0 {
            return None;
        }
        let width = image.width().checked_mul(options.zoom_x)?;
        let height = image.height().checked_mul(options.zoom_y)?;
        if u64::from(width) * u64::from(height) > MAX_PIXELS {
            return None;
        }
        if options.zoom_x != 1 || options.zoom_y != 1 {
            image = image.resize_exact(width, height, FilterType::Nearest);
        }
    }

    let (mut dx, mut dy) = (options.dx, options.dy);
    let screen_width = screen_size.width * font.width;
    let screen_height = screen_size.height * font.height;
    if dx < 0 {
        let skip = dx.unsigned_abs();
        if skip >= image.width() {
            return None;
        }
        image = image.crop_imm(skip, 0, image.width() - skip, image.height());
        dx = 0;
    }
    if dy < 0 {
        let skip = dy.unsigned_abs();
        if skip >= image.height() {
            return None;
        }
        image = image.crop_imm(0, skip, image.width(), image.height() - skip);
        dy = 0;
    }
    if image.width() == 0 || image.height() == 0 || dx >= screen_width || dy >= screen_height {
        return None;
    }
    let visible_width = image.width().min((screen_width - dx) as u32);
    let visible_height = image.height().min((screen_height - dy) as u32);
    if visible_width != image.width() || visible_height != image.height() {
        image = image.crop_imm(0, 0, visible_width, visible_height);
    }

    let rgba = image.to_rgba8();
    let position = Position::new(dx / font.width.max(1), dy / font.height.max(1));
    let mut sixel = Sixel::from_data((rgba.width() as i32, rgba.height() as i32), 1, 1, rgba.into_raw());
    sixel.pixel_offset = Position::new(dx % font.width.max(1), dy % font.height.max(1));
    Some((position, sixel))
}

/// Decodes a base64 `DrawPPMBlob` / `DrawJXLBlob` APC payload.
pub fn decode_image_apc(data: &[u8], font: Size, screen_size: Size) -> Option<(Position, Sixel)> {
    const PPM_PREFIX: &str = "SyncTERM:C;DrawPPMBlob";
    const JXL_PREFIX: &str = "SyncTERM:C;DrawJXLBlob";

    let command = std::str::from_utf8(data).ok()?;
    let (arguments, is_jxl) = if let Some(arguments) = command.strip_prefix(PPM_PREFIX) {
        (arguments, false)
    } else if let Some(arguments) = command.strip_prefix(JXL_PREFIX) {
        (arguments, true)
    } else {
        return None;
    };

    let arguments = arguments.trim_start_matches([';', ' ']);
    let (options, encoded) = arguments.rsplit_once([';', ' ']).unwrap_or(("", arguments));
    if encoded.is_empty() || encoded.len() > MAX_ENCODED_SIZE {
        if !encoded.is_empty() {
            log::warn!("Ignoring oversized inline image payload");
        }
        return None;
    }
    let bytes = general_purpose::STANDARD.decode(encoded).ok()?;
    decode_image_blob(&bytes, is_jxl, options, font, screen_size)
}

/// Adapter that implements `CommandSink` for any type implementing `EditableScreen`.
/// This allows `icy_parser_core` parsers to drive `icy_engine`'s terminal emulation.
pub struct ScreenSink<'a> {
    screen: &'a mut dyn EditableScreen,
    diagnostics: Vec<(ParseError, ErrorLevel)>,
}

impl<'a> ScreenSink<'a> {
    pub fn new(screen: &'a mut dyn EditableScreen) -> Self {
        Self {
            screen,
            diagnostics: Vec::new(),
        }
    }

    /// Get mutable reference to the underlying screen
    pub fn screen_mut(&mut self) -> &mut dyn EditableScreen {
        self.screen
    }

    /// Get reference to the underlying screen
    pub fn screen(&self) -> &dyn EditableScreen {
        self.screen
    }

    pub fn diagnostics(&self) -> &[(ParseError, ErrorLevel)] {
        &self.diagnostics
    }

    /// Get the current caret attribute with `inverse_video` applied if active
    fn display_attribute(&self) -> crate::TextAttribute {
        let mut attr = if self.screen.terminal_state().inverse_video {
            let mut attr = self.screen.caret().attribute;
            let fg = attr.foreground();
            let bg = attr.background();
            attr.set_foreground(bg);
            attr.set_background(fg);
            attr
        } else {
            self.screen.caret().attribute
        };

        if self.screen.terminal_state().ice_colors && attr.is_blinking() && attr.background() < 8 {
            attr.set_is_blinking(false);
            attr.set_background(attr.background() + 8);
        }
        attr
    }

    fn dec_rectangle(&self, top: u16, left: u16, bottom: u16, right: u16) -> Option<(Position, Position)> {
        let (origin_x, origin_y, max_x, max_y) = match self.screen.terminal_state().origin_mode {
            crate::OriginMode::UpperLeftCorner => (0, self.screen.first_visible_line(), self.screen.width() - 1, self.screen.last_visible_line()),
            crate::OriginMode::WithinMargins => (
                self.screen.first_editable_column(),
                self.screen.first_editable_line(),
                self.screen.last_editable_column(),
                self.screen.last_editable_line(),
            ),
        };
        let start = Position::new(origin_x + i32::from(left.saturating_sub(1)), origin_y + i32::from(top.saturating_sub(1)));
        let end = Position::new(origin_x + i32::from(right.saturating_sub(1)), origin_y + i32::from(bottom.saturating_sub(1)));
        let start = Position::new(start.x.clamp(origin_x, max_x), start.y.clamp(origin_y, max_y));
        let end = Position::new(end.x.clamp(origin_x, max_x), end.y.clamp(origin_y, max_y));
        (start.x <= end.x && start.y <= end.y).then_some((start, end))
    }

    fn fill_dec_rectangle(&mut self, top: u16, left: u16, bottom: u16, right: u16, ch: AttributedChar) {
        let Some((start, end)) = self.dec_rectangle(top, left, bottom, right) else {
            return;
        };
        for y in start.y..=end.y {
            for x in start.x..=end.x {
                self.screen.set_char(Position::new(x, y), ch);
            }
        }
    }

    fn set_font_selection_success(&mut self, slot: u8) {
        self.screen.terminal_state_mut().font_selection_state = FontSelectionState::Success;
        self.screen.caret_mut().set_font_page(slot);

        if self.screen.caret().attribute.is_blinking() && self.screen.caret().attribute.is_bold() {
            self.screen.terminal_state_mut().high_intensity_blink_attribute_font_slot = slot as usize;
        } else if self.screen.caret().attribute.is_blinking() {
            self.screen.terminal_state_mut().blink_attribute_font_slot = slot as usize;
        } else if self.screen.caret().attribute.is_bold() {
            self.screen.terminal_state_mut().high_intensity_attribute_font_slot = slot as usize;
        } else {
            self.screen.terminal_state_mut().normal_attribute_font_slot = slot as usize;
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
                attr.set_font_page(font);
            }
            SgrAttribute::Foreground(color) => {
                match color {
                    Color::Base(c) => {
                        let col = {
                            let _ = attr;
                            (c as u32) % self.screen.max_base_colors()
                        };
                        self.screen.caret_mut().attribute.set_foreground(col);
                    }
                    Color::Extended(c) => {
                        // Extended colors (256-color palette) - store as extended palette index
                        self.screen.caret_mut().attribute.set_foreground_ext(c);
                    }
                    Color::Rgb(r, g, b) => {
                        self.screen.caret_mut().attribute.set_foreground_rgb(r, g, b);
                    }
                    Color::Default => {
                        attr.set_foreground(7);
                    }
                }
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
                        // Extended colors (256-color palette) - store as extended palette index
                        self.screen.caret_mut().attribute.set_background_ext(c);
                    }
                    Color::Rgb(r, g, b) => {
                        self.screen.caret_mut().attribute.set_background_rgb(r, g, b);
                    }
                    Color::Default => {
                        attr.set_background(0);
                    }
                }
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
                self.screen.terminal_state_mut().wrap_pending = false;
                self.screen.terminal_state_mut().origin_mode = if enabled {
                    crate::OriginMode::WithinMargins
                } else {
                    crate::OriginMode::UpperLeftCorner
                };
            }
            DecMode::AutoWrap => {
                self.screen.terminal_state_mut().wrap_pending = false;
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
                    let fg = attr.foreground();
                    let bg = attr.background();
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
            DecMode::BracketedPaste => {
                self.screen.terminal_state_mut().bracketed_paste_mode = enabled;
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

        let prev_attr = self.screen.char_at((sx, sy).into()).attribute;

        // Fill remaining characters on the line that match the previous attribute
        // This handles cases like double-height where we need to update all following characters
        for x in sx..self.screen.terminal_state().width() {
            let p = Position::new(x, sy);
            let mut ch = self.screen.char_at(p);

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

impl CommandSink for ScreenSink<'_> {
    fn print(&mut self, text: &[u8]) {
        match self.screen.buffer_type() {
            BufferType::Unicode => {
                // UTF-8 mode: use utf8parse for proper multi-byte sequence handling
                // Collect decoded characters first to avoid borrow issues
                let mut chars = Vec::new();
                {
                    struct CharCollector<'b>(&'b mut Vec<char>);
                    impl utf8parse::Receiver for CharCollector<'_> {
                        fn codepoint(&mut self, c: char) {
                            self.0.push(c);
                        }
                        fn invalid_sequence(&mut self) {
                            self.0.push('\u{FFFD}');
                        }
                    }

                    let parser = &mut self.screen.terminal_state_mut().utf8_parser;
                    let mut receiver = CharCollector(&mut chars);
                    for &byte in text {
                        parser.advance(&mut receiver, byte);
                    }
                }

                // Now output the collected characters
                for ch in chars {
                    if self.screen.height() >= crate::limits::MAX_BUFFER_HEIGHT || self.screen.width() >= crate::limits::MAX_BUFFER_WIDTH {
                        // Prevent excessive buffer growth
                        break;
                    }
                    let attr_char = AttributedChar::new(ch, self.display_attribute());
                    self.screen.print_char(attr_char);
                }
            }
            _ => {
                // Legacy mode: treat each byte as a character (CP437, Petscii, Atascii, Viewdata)
                for &byte in text {
                    if self.screen.height() >= crate::limits::MAX_BUFFER_HEIGHT || self.screen.width() >= crate::limits::MAX_BUFFER_WIDTH {
                        // Prevent excessive buffer growth
                        break;
                    }
                    let ch = AttributedChar::new(byte as char, self.display_attribute());
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
            TerminalCommand::CsiMoveCursor(direction, n, wrapping) => {
                let n = n as i32;
                let auto_wrap = match wrapping {
                    Wrapping::Never => false,
                    // Use terminal's auto_wrap_mode setting
                    Wrapping::Always | Wrapping::Setting => true,
                };
                match direction {
                    Direction::Up => self.screen.up(n, false, auto_wrap),
                    Direction::Down => self.screen.down(n, false, auto_wrap),
                    Direction::Left => self.screen.left(n, false, auto_wrap),
                    Direction::Right => self.screen.right(n, false, auto_wrap),
                }
            }
            TerminalCommand::CsiCursorNextLine(n) => {
                for _ in 0..n {
                    self.screen.next_line(false);
                }
            }
            TerminalCommand::CsiCursorPreviousLine(n) => {
                self.screen.up(n as i32, false, false);
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
                let blank = AttributedChar::new(' ', self.display_attribute());
                for i in 0..n as i32 {
                    let x = pos.x + i;
                    if x < self.screen.width() {
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
                self.screen.down(n as i32, false, false);
            }
            TerminalCommand::CsiCharacterPositionForward(n) => {
                self.screen.right(n as i32, false, false);
            }
            TerminalCommand::CsiHorizontalPositionAbsolute(col) => {
                let upper_left = self.screen.upper_left_position();
                let col = upper_left.x + (col as i32).saturating_sub(1).max(0);
                let mut pos = self.screen.caret_position();
                pos.x = col;
                self.screen.set_caret_position(pos);
                self.screen.limit_caret_pos(false);
            }
            TerminalCommand::CsiSetLastColumnFlag { enabled, forced } => {
                let state = self.screen.terminal_state_mut();
                if forced {
                    state.last_column_flag_forced = true;
                    state.last_column_flag_mode = true;
                } else if !state.last_column_flag_forced {
                    state.last_column_flag_mode = enabled;
                }
                state.wrap_pending = false;
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

            // Kitty keyboard protocol
            TerminalCommand::PushKittyKeyboardFlags(flags) => {
                self.screen.terminal_state_mut().kitty_keyboard.push(flags);
            }
            TerminalCommand::PopKittyKeyboardFlags(count) => {
                self.screen.terminal_state_mut().kitty_keyboard.pop(count as usize);
            }
            TerminalCommand::SetKittyKeyboardFlags(flags, mode) => {
                self.screen.terminal_state_mut().kitty_keyboard.set(flags, mode);
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
                self.screen.up(1, true, false);
            }
            TerminalCommand::EscReset => {
                self.screen.reset_terminal();
            }

            // Commands not yet fully mapped
            TerminalCommand::SetFontPage(page) => {
                self.screen.caret_mut().set_font_page(page as u8);
            }
            TerminalCommand::CsiFontSelection { slot: _slot, font_number } => {
                let nr = font_number as u8;
                if self.screen().font(nr as usize).is_some() {
                    self.set_font_selection_success(nr);
                }
                match BitFont::from_ansi_font_page(nr, self.screen().font_dimensions().height as u8) {
                    Some(font) => {
                        self.screen_mut().set_font(nr as usize, font.clone());
                        self.set_font_selection_success(nr);
                    }
                    None => {
                        self.screen_mut().terminal_state_mut().font_selection_state = FontSelectionState::Failure;
                    }
                }
            }
            TerminalCommand::CsiSelectCommunicationSpeed(_, _) => {}
            TerminalCommand::CsiFillRectangularArea {
                char,
                top,
                left,
                bottom,
                right,
            } => {
                self.fill_dec_rectangle(top, left, bottom, right, AttributedChar::new(char as char, self.display_attribute()));
            }
            TerminalCommand::CsiEraseRectangularArea { top, left, bottom, right }
            | TerminalCommand::CsiSelectiveEraseRectangularArea { top, left, bottom, right } => {
                self.fill_dec_rectangle(top, left, bottom, right, AttributedChar::new(' ', crate::TextAttribute::default()));
            }
            TerminalCommand::CsiSetStatusDisplayType(_) | TerminalCommand::CsiSelectActiveStatusDisplay(_) => {}
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
                let width = self.screen.width();

                let (current_left, current_right) = self.screen.terminal_state().margins_left_right().unwrap_or((0, width - 1));

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
                        let bottom = if let Some((_, b)) = self.screen.terminal_state().margins_top_bottom() {
                            b
                        } else {
                            self.screen.height() - 1
                        };
                        self.screen.terminal_state_mut().set_margins_top_bottom(n, bottom);
                    }
                    MarginType::Bottom => {
                        let top = if let Some((t, _)) = self.screen.terminal_state().margins_top_bottom() {
                            t
                        } else {
                            0
                        };
                        self.screen.terminal_state_mut().set_margins_top_bottom(top, n);
                    }
                    MarginType::Left => {
                        let right = if let Some((_, r)) = self.screen.terminal_state().margins_left_right() {
                            r
                        } else {
                            self.screen.width() - 1
                        };
                        self.screen.terminal_state_mut().set_margins_left_right(n, right);
                    }
                    MarginType::Right => {
                        let left = if let Some((l, _)) = self.screen.terminal_state().margins_left_right() {
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
                        self.screen.terminal_state().height() - 1
                    };
                    let mut pos = self.screen.caret_position();
                    pos.y = y;
                    self.screen.set_caret_position(pos);
                }
                Direction::Down => {
                    let mut pos = self.screen.caret_position();
                    pos.y += 1;
                    if pos.y >= self.screen.terminal_state().height() {
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
                        let x = self.screen.terminal_state().width().saturating_sub(1);
                        self.screen.caret_mut().x = x;
                        self.emit_view_data(ViewDataCommand::MoveCaret(Direction::Up));
                    }
                }
                Direction::Right => {
                    let x = self.screen.caret().x;
                    self.screen.caret_mut().x = x + 1;
                    if self.screen.caret().x >= self.screen.terminal_state().width() {
                        self.screen.caret_mut().x = 0;
                        self.emit_view_data(ViewDataCommand::MoveCaret(Direction::Down));
                        return true;
                    }
                }
            },
            ViewDataCommand::SetBgToFg => {
                let fg = self.screen.caret_mut().attribute.foreground();
                self.screen.caret_mut().attribute.set_background(fg);
            }
            ViewDataCommand::SetChar(ch) => {
                let ch = AttributedChar::new(ch as char, self.display_attribute());
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
                match crate::BitFont::from_bytes(format!("custom font {slot}"), &data) {
                    Ok(font) => {
                        log::info!("Loaded custom font into slot {slot}");
                        self.screen.set_font(slot, font);
                    }
                    Err(err) => {
                        log::error!("Failed to load custom font: {err}");
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
                    log::error!("Error loading sixel: {err}");
                }
            },
        }
    }

    fn operating_system_command(&mut self, osc: OperatingSystemCommand) {
        // OSC handling - typically for window title, hyperlinks, etc.
        match osc {
            OperatingSystemCommand::SetTitle(title) => {
                if let Ok(title_str) = std::str::from_utf8(&title) {
                    log::debug!("OSC: Set title to '{title_str}'");
                    // TODO: Add SetTitle callback variant
                }
            }
            OperatingSystemCommand::SetIconName(name) => {
                if let Ok(name_str) = std::str::from_utf8(&name) {
                    log::debug!("OSC: Set icon name to '{name_str}'");
                }
            }
            OperatingSystemCommand::SetWindowTitle(title) => {
                if let Ok(title_str) = std::str::from_utf8(&title) {
                    log::debug!("OSC: Set window title to '{title_str}'");
                }
            }
            OperatingSystemCommand::SetPaletteColor(index, r, g, b) => {
                // Set palette color
                self.screen.palette_mut().set_color_rgb(index as u32, r, g, b);
                log::debug!("OSC: Set palette color {index} to RGB({r}, {g}, {b})");
            }
            OperatingSystemCommand::ResetPaletteColors(indices) => {
                if indices.is_empty() {
                    for (index, (_, color)) in crate::XTERM_256_PALETTE.iter().enumerate() {
                        self.screen.palette_mut().set_color(index as u32, color.clone());
                    }
                } else {
                    for index in indices {
                        let color = crate::XTERM_256_PALETTE[index as usize].1.clone();
                        self.screen.palette_mut().set_color(index as u32, color);
                    }
                }
            }
            OperatingSystemCommand::Hyperlink { params, uri } => {
                if let (Ok(_params_str), Ok(uri_str)) = (std::str::from_utf8(&params), std::str::from_utf8(&uri)) {
                    if uri_str.is_empty() {
                        self.screen.caret_mut().attribute.set_is_underlined(false);
                        if let Some((url, position)) = self.screen.terminal_state_mut().active_hyperlink.take() {
                            let end = self.screen.caret_position();
                            let length = (end.y - position.y) * self.screen.width() + end.x - position.x;
                            if length > 0 {
                                self.screen.add_hyperlink(crate::HyperLink {
                                    url: Some(url),
                                    position,
                                    length,
                                });
                            }
                        }
                    } else {
                        self.screen.caret_mut().attribute.set_is_underlined(true);
                        self.screen.terminal_state_mut().active_hyperlink = Some((uri_str.to_string(), self.screen.caret_position()));
                    }
                }
            }
        }
    }

    fn aps(&mut self, data: &[u8]) {
        let font = self.screen.font_dimensions();
        let screen_size = Size::new(self.screen.width(), self.screen.height());
        if let Some((position, sixel)) = decode_image_apc(data, font, screen_size) {
            self.screen.add_sixel(position, sixel);
        }
    }

    fn play_music(&mut self, _music: AnsiMusic) {
        // Push music playback callback to be handled by application layer
    }

    fn report_error(&mut self, error: ParseError, level: ErrorLevel) {
        match level {
            ErrorLevel::Error => log::error!("Parser error: {error:?}"),
            ErrorLevel::Warning => log::warn!("Parser warning: {error:?}"),
            ErrorLevel::Info => log::info!("Parser info: {error:?}"),
        }
        self.diagnostics.push((error, level));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use icy_parser_core::{AnsiParser, CommandParser};

    #[test]
    fn draws_inline_ppm_apc() {
        let ppm = b"P3\n2 1\n255\n255 0 0  0 255 0\n";
        let encoded = general_purpose::STANDARD.encode(ppm);
        let sequence = format!("\x1B_SyncTERM:C;DrawPPMBlob;DW=4;DH=2;{}\x1B\\", encoded);
        let mut screen = crate::TextScreen::new((80, 25));
        let mut parser = AnsiParser::new();

        parser.parse(sequence.as_bytes(), &mut ScreenSink::new(&mut screen));

        assert_eq!(screen.buffer.layers[0].sixels.len(), 1);
        assert_eq!(screen.buffer.layers[0].sixels[0].size(), crate::Size::new(4, 2));
        assert_eq!(screen.buffer.layers[0].sixels[0].picture_data.len(), 4 * 2 * 4);
    }

    #[test]
    fn transforms_and_positions_inline_ppm_apc() {
        let ppm = b"P3\n2 1\n255\n255 0 0  0 255 0\n";
        let encoded = general_purpose::STANDARD.encode(ppm);
        let sequence = format!("\x1B_SyncTERM:C;DrawPPMBlob;FX;ZX=2;ZY=2;DX=3;DY=5;{}\x1B\\", encoded);
        let mut screen = crate::TextScreen::new((80, 25));
        let mut parser = AnsiParser::new();
        parser.parse(sequence.as_bytes(), &mut ScreenSink::new(&mut screen));

        let sixel = &screen.buffer.layers[0].sixels[0];
        assert_eq!(sixel.size(), crate::Size::new(4, 2));
        assert_eq!(sixel.pixel_offset, Position::new(3, 5));
        assert_eq!(&sixel.picture_data[..4], &[0, 255, 0, 255]);
    }

    #[test]
    fn clips_negative_inline_ppm_destination() {
        let ppm = b"P3\n2 1\n255\n255 0 0  0 255 0\n";
        let encoded = general_purpose::STANDARD.encode(ppm);
        let sequence = format!("\x1B_SyncTERM:C;DrawPPMBlob;DX=-1;{}\x1B\\", encoded);
        let mut screen = crate::TextScreen::new((80, 25));
        let mut parser = AnsiParser::new();
        parser.parse(sequence.as_bytes(), &mut ScreenSink::new(&mut screen));

        let sixel = &screen.buffer.layers[0].sixels[0];
        assert_eq!(sixel.size(), crate::Size::new(1, 1));
        assert_eq!(sixel.pixel_offset, Position::default());
        assert_eq!(&sixel.picture_data[..4], &[0, 255, 0, 255]);
    }
}
