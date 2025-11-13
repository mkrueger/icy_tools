//! Core parser infrastructure: command emission traits and basic ASCII parser.

#![cfg_attr(feature = "simd", feature(portable_simd))]

#[cfg(feature = "simd")]
use std::simd::{cmp::SimdPartialEq, *};

mod ansi;
pub use ansi::AnsiParser;

mod avatar;
pub use avatar::AvatarParser;

mod pcboard;
pub use pcboard::PcBoardParser;

mod ctrla;
pub use ctrla::CtrlAParser;

mod renegade;
pub use renegade::RenegadeParser;

/// Erase in Display mode for ED command (ESC[nJ)
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EraseInDisplayMode {
    /// Clear from cursor to end of display
    CursorToEnd = 0,
    /// Clear from start of display to cursor
    StartToCursor = 1,
    /// Clear entire display
    All = 2,
    /// Clear entire display and scrollback buffer
    AllAndScrollback = 3,
}

impl EraseInDisplayMode {
    fn from_u16(n: u16) -> Option<Self> {
        match n {
            0 => Some(Self::CursorToEnd),
            1 => Some(Self::StartToCursor),
            2 => Some(Self::All),
            3 => Some(Self::AllAndScrollback),
            _ => None,
        }
    }
}

/// Erase in Line mode for EL command (ESC[nK)
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EraseInLineMode {
    /// Clear from cursor to end of line
    CursorToEnd = 0,
    /// Clear from start of line to cursor
    StartToCursor = 1,
    /// Clear entire line
    All = 2,
}

impl EraseInLineMode {
    fn from_u16(n: u16) -> Option<Self> {
        match n {
            0 => Some(Self::CursorToEnd),
            1 => Some(Self::StartToCursor),
            2 => Some(Self::All),
            _ => None,
        }
    }
}

/// Device Status Report type for DSR command (ESC[nn)
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceStatusReport {
    /// Report operating status (reply: ESC[0n = OK)
    OperatingStatus = 5,
    /// Report cursor position (reply: ESC[{row};{col}R)
    CursorPosition = 6,
}

impl DeviceStatusReport {
    fn from_u16(n: u16) -> Option<Self> {
        match n {
            5 => Some(Self::OperatingStatus),
            6 => Some(Self::CursorPosition),
            _ => None,
        }
    }
}

/// ANSI Mode for SM/RM commands (ESC[nh / ESC[nl)
/// Standard ANSI modes - distinct from DEC private modes (which use ESC[?nh)
/// Currently only IRM (Insert/Replace Mode) is used by icy_engine
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnsiMode {
    /// IRM - Insert/Replace Mode (Mode 4)
    /// When set: newly received characters are inserted, pushing existing characters to the right
    /// When reset: newly received characters replace (overwrite) existing characters
    InsertReplace = 4,
}

impl AnsiMode {
    fn from_u16(n: u16) -> Option<Self> {
        match n {
            4 => Some(Self::InsertReplace),
            _ => None,
        }
    }
}

/// DEC Private Mode for DECSET/DECRST commands (ESC[?nh / ESC[?nl)
/// These are terminal-specific modes distinct from standard ANSI modes
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecPrivateMode {
    // Scrolling and Display Modes
    /// DECSCLM - Smooth Scroll (Mode 4)
    SmoothScroll = 4,
    /// DECOM - Origin Mode (Mode 6)
    /// When set: cursor addressing is relative to scrolling region
    /// When reset: cursor addressing is absolute (relative to upper-left corner)
    OriginMode = 6,
    /// DECAWM - Auto Wrap Mode (Mode 7)
    /// When set: cursor wraps to next line at right margin
    /// When reset: cursor stops at right margin
    AutoWrap = 7,

    // Cursor Modes
    /// DECTCEM - Text Cursor Enable Mode (Mode 25)
    /// When set: cursor is visible
    /// When reset: cursor is invisible
    CursorVisible = 25,
    /// ATT610 - Blinking Cursor (Mode 35)
    /// When set: cursor stops blinking
    /// When reset: cursor blinks
    CursorBlinking = 35,

    // iCE Colors / Blink Mode
    /// iCE Colors (Mode 33)
    /// When set: enable iCE colors (use background intensity instead of blink)
    /// When reset: standard blink attribute
    IceColors = 33,

    // Margin Modes
    /// DECLRMM - Left/Right Margin Mode (Mode 69)
    /// When set: left and right margins are enabled
    /// When reset: left and right margins are disabled
    LeftRightMargin = 69,

    // Mouse Tracking Modes
    /// X10 Mouse (Mode 9)
    X10Mouse = 9,
    /// VT200 Mouse (Mode 1000)
    VT200Mouse = 1000,
    /// VT200 Highlight Mouse (Mode 1001)
    VT200HighlightMouse = 1001,
    /// Button Event Mouse (Mode 1002)
    ButtonEventMouse = 1002,
    /// Any Event Mouse (Mode 1003)
    AnyEventMouse = 1003,
    /// Focus Event (Mode 1004)
    /// When set: report focus in/out events
    FocusEvent = 1004,
    /// Alternate Scroll (Mode 1007)
    /// When set: use alternate scroll mode
    AlternateScroll = 1007,

    // Mouse Extended Modes
    /// UTF-8 Extended Mouse Mode (Mode 1005)
    ExtendedMouseUTF8 = 1005,
    /// SGR Extended Mouse Mode (Mode 1006)
    ExtendedMouseSGR = 1006,
    /// URXVT Extended Mouse Mode (Mode 1015)
    ExtendedMouseURXVT = 1015,
    /// Pixel Position Mouse Mode (Mode 1016)
    ExtendedMousePixel = 1016,
}

impl DecPrivateMode {
    fn from_u16(n: u16) -> Option<Self> {
        match n {
            4 => Some(Self::SmoothScroll),
            6 => Some(Self::OriginMode),
            7 => Some(Self::AutoWrap),
            25 => Some(Self::CursorVisible),
            33 => Some(Self::IceColors),
            35 => Some(Self::CursorBlinking),
            69 => Some(Self::LeftRightMargin),
            9 => Some(Self::X10Mouse),
            1000 => Some(Self::VT200Mouse),
            1001 => Some(Self::VT200HighlightMouse),
            1002 => Some(Self::ButtonEventMouse),
            1003 => Some(Self::AnyEventMouse),
            1004 => Some(Self::FocusEvent),
            1007 => Some(Self::AlternateScroll),
            1005 => Some(Self::ExtendedMouseUTF8),
            1006 => Some(Self::ExtendedMouseSGR),
            1015 => Some(Self::ExtendedMouseURXVT),
            1016 => Some(Self::ExtendedMousePixel),
            _ => None,
        }
    }
}

/// Color values for foreground and background SGR attributes
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    /// Base 16 colors (0-15: Black, Red, Green, Yellow, Blue, Magenta, Cyan, White, and bright variants)
    Base(u8),
    /// Extended 256-color palette (colors 0-255)
    Extended(u8),
    /// RGB color (red, green, blue)
    Rgb(u8, u8, u8),
    /// Default/terminal color
    Default,
}

/// Intensity level for text display
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intensity {
    /// Normal intensity (default)
    Normal,
    /// Bold or increased intensity
    Bold,
    /// Faint, decreased intensity or second color
    Faint,
}

/// Underline style for text
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Underline {
    /// Not underlined
    Off,
    /// Single underline
    Single,
    /// Double underline
    Double,
}

/// Blink rate for text
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blink {
    /// Not blinking (steady)
    Off,
    /// Slowly blinking (less than 150 per minute)
    Slow,
    /// Rapidly blinking (150 per minute or more)
    Rapid,
}

/// Frame or encircle style for text
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frame {
    /// Not framed or encircled
    Off,
    /// Framed text
    Framed,
    /// Encircled text
    Encircled,
}

/// SGR (Select Graphic Rendition) attributes for ESC[...m sequences
/// These control text appearance: colors, intensity, underline, etc.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SgrAttribute {
    // Reset
    /// Reset all attributes to default
    Reset,

    // Intensity
    /// Set text intensity (normal, bold, or faint)
    Intensity(Intensity),

    // Style attributes with boolean values
    /// Italic text (true = italic, false = not italic)
    Italic(bool),

    /// Fraktur (Gothic) font
    Fraktur,

    /// Underline style (off, single, or double)
    Underline(Underline),

    /// Crossed out / strike-through (true = crossed, false = not crossed)
    CrossedOut(bool),

    // Blinking
    /// Blink rate (off, slow, or rapid)
    Blink(Blink),

    // Display modes with boolean values
    /// Inverse/reverse video (true = inverse, false = normal)
    Inverse(bool),
    /// Concealed/hidden (true = concealed, false = revealed)
    Concealed(bool),

    // Framing and encircling
    /// Frame or encircle style (off, framed, or encircled)
    Frame(Frame),
    /// Overlined text (true = overlined, false = not overlined)
    Overlined(bool),

    // Font selection (0-9)
    /// Font selection (0 = primary/default, 1-9 = alternative fonts)
    Font(u8), // 0-9

    // Colors
    /// Set foreground color
    Foreground(Color),
    /// Set background color
    Background(Color),

    // Ideogram attributes (60-65)
    /// Ideogram underline or right side line
    IdeogramUnderline,
    /// Ideogram double underline or double line on right side
    IdeogramDoubleUnderline,
    /// Ideogram overline or left side line
    IdeogramOverline,
    /// Ideogram double overline or double line on left side
    IdeogramDoubleOverline,
    /// Ideogram stress marking
    IdeogramStress,
    /// Cancel ideogram attributes
    IdeogramAttributesOff,
}

/// Parser error types
#[repr(u8)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// Invalid parameter value for a command
    InvalidParameter { command: &'static str, value: u16 },
    /// Incomplete sequence (parser state at end of input)
    IncompleteSequence,
    /// Malformed escape sequence
    MalformedSequence { description: &'static str },
}

#[repr(u8)]
#[derive(Debug, PartialEq)]
pub enum TerminalCommand<'a> {
    /// A contiguous run of displayable bytes (any byte not a handled control).
    Printable(&'a [u8]),

    // Basic control characters (C0 controls)
    CarriageReturn,
    LineFeed,
    Backspace,
    Tab,
    FormFeed,
    Bell,
    Delete,

    // ANSI CSI (Control Sequence Introducer) sequences
    /// CUU - Cursor Up: ESC[{n}A
    CsiCursorUp(u16),
    /// CUD - Cursor Down: ESC[{n}B
    CsiCursorDown(u16),
    /// CUF - Cursor Forward: ESC[{n}C
    CsiCursorForward(u16),
    /// CUB - Cursor Back: ESC[{n}D
    CsiCursorBack(u16),
    /// CNL - Cursor Next Line: ESC[{n}E
    CsiCursorNextLine(u16),
    /// CPL - Cursor Previous Line: ESC[{n}F
    CsiCursorPreviousLine(u16),
    /// CHA - Cursor Horizontal Absolute: ESC[{n}G
    CsiCursorHorizontalAbsolute(u16),
    /// CUP - Cursor Position: ESC[{row};{col}H or ESC[{row};{col}f
    CsiCursorPosition(u16, u16),

    /// ED - Erase in Display: ESC[{n}J
    CsiEraseInDisplay(EraseInDisplayMode),
    /// EL - Erase in Line: ESC[{n}K
    CsiEraseInLine(EraseInLineMode),

    /// SU - Scroll Up: ESC[{n}S
    CsiScrollUp(u16),
    /// SD - Scroll Down: ESC[{n}T
    CsiScrollDown(u16),

    /// SGR - Select Graphic Rendition: ESC[{param}m
    /// Text attributes like color, bold, underline, etc.
    /// Emitted once per attribute in a sequence (e.g., ESC[1;31m emits Bold then ForegroundRed)
    CsiSelectGraphicRendition(SgrAttribute),

    /// DECSTBM - Set Scrolling Region: ESC[{top};{bottom}r
    CsiSetScrollingRegion(u16, u16),

    /// ICH - Insert Character: ESC[{n}@
    CsiInsertCharacter(u16),
    /// DCH - Delete Character: ESC[{n}P
    CsiDeleteCharacter(u16),
    /// ECH - Erase Character: ESC[{n}X
    CsiEraseCharacter(u16),
    /// IL - Insert Line: ESC[{n}L
    CsiInsertLine(u16),
    /// DL - Delete Line: ESC[{n}M
    CsiDeleteLine(u16),

    /// REP - Repeat preceding character: ESC[{n}b
    CsiRepeatPrecedingCharacter(u16),

    /// DA - Device Attributes: ESC[c or ESC[0c
    CsiDeviceAttributes,
    /// DSR - Device Status Report: ESC[{n}n
    CsiDeviceStatusReport(DeviceStatusReport),

    /// DECSET - DEC Private Mode Set: ESC[?{n}h
    /// Emitted once per mode (e.g., ESC[?25;1000h emits two commands)
    CsiDecPrivateModeSet(DecPrivateMode),
    /// DECRST - DEC Private Mode Reset: ESC[?{n}l
    /// Emitted once per mode (e.g., ESC[?25;1000l emits two commands)
    CsiDecPrivateModeReset(DecPrivateMode),

    /// SM - Set Mode: ESC[{n}h
    /// Emitted once per mode
    CsiSetMode(AnsiMode),
    /// RM - Reset Mode: ESC[{n}l
    /// Emitted once per mode
    CsiResetMode(AnsiMode),

    // ANSI ESC sequences (non-CSI)
    /// IND - Index: ESC D (move cursor down, scroll if at bottom)
    EscIndex,
    /// NEL - Next Line: ESC E
    EscNextLine,
    /// HTS - Horizontal Tab Set: ESC H
    EscSetTab,
    /// RI - Reverse Index: ESC M (move cursor up, scroll if at top)
    EscReverseIndex,
    /// DECSC - Save Cursor: ESC 7
    EscSaveCursor,
    /// DECRC - Restore Cursor: ESC 8
    EscRestoreCursor,
    /// RIS - Reset to Initial State: ESC c
    EscReset,

    // OSC (Operating System Command) sequences
    /// OSC 0 - Set Icon Name and Window Title: ESC]0;{text}BEL or ESC]0;{text}ESC\
    OscSetTitle(&'a [u8]),
    /// OSC 1 - Set Icon Name: ESC]1;{text}BEL
    OscSetIconName(&'a [u8]),
    /// OSC 2 - Set Window Title: ESC]2;{text}BEL
    OscSetWindowTitle(&'a [u8]),
    /// OSC 8 - Hyperlink: ESC]8;{params};{uri}BEL
    OscHyperlink {
        params: &'a [u8],
        uri: &'a [u8],
    },

    // Avatar (Advanced Video Attribute Terminal Assembler and Recreator) commands
    /// AVT Repeat Character: ^Y{char}{count}
    AvtRepeatChar(u8, u8),

    /// Unknown or unsupported escape sequence
    /// Contains the raw bytes for potential logging/debugging
    Unknown(&'a [u8]),
}

pub trait CommandSink {
    fn emit(&mut self, cmd: TerminalCommand<'_>);

    /// Report a parsing error. Default implementation does nothing.
    fn report_error(&mut self, _error: ParseError) {}
}

pub trait CommandParser {
    fn parse(&mut self, input: &[u8], sink: &mut dyn CommandSink);
    fn flush(&mut self, _sink: &mut dyn CommandSink) {}
}

#[derive(Default)]
pub struct AsciiParser {}

impl AsciiParser {
    pub fn new() -> Self {
        Self::default()
    }
}

#[repr(u8)]
#[derive(Copy, Clone)]
enum ControlKind {
    Bell = 1,
    Backspace = 2,
    Tab = 3,
    LineFeed = 4,
    FormFeed = 5,
    CarriageReturn = 6,
    Delete = 7,
}

const fn build_control_lut() -> [u8; 256] {
    let mut lut = [0u8; 256];
    lut[0x07] = ControlKind::Bell as u8;
    lut[0x08] = ControlKind::Backspace as u8;
    lut[0x09] = ControlKind::Tab as u8;
    lut[0x0A] = ControlKind::LineFeed as u8;
    lut[0x0C] = ControlKind::FormFeed as u8;
    lut[0x0D] = ControlKind::CarriageReturn as u8;
    lut[0x7F] = ControlKind::Delete as u8;
    lut
}

const CONTROL_LUT: [u8; 256] = build_control_lut();

#[cfg(feature = "simd")]
impl CommandParser for AsciiParser {
    fn parse(&mut self, input: &[u8], sink: &mut dyn CommandSink) {
        let mut i = 0;
        let mut printable_start = 0;

        // SIMD fast path: check 16-byte chunks for controls
        while i + 16 <= input.len() {
            let chunk = u8x16::from_slice(&input[i..i + 16]);

            // Check for each control byte type
            let has_bell = chunk.simd_eq(u8x16::splat(0x07));
            let has_bs = chunk.simd_eq(u8x16::splat(0x08));
            let has_tab = chunk.simd_eq(u8x16::splat(0x09));
            let has_lf = chunk.simd_eq(u8x16::splat(0x0A));
            let has_ff = chunk.simd_eq(u8x16::splat(0x0C));
            let has_cr = chunk.simd_eq(u8x16::splat(0x0D));
            let has_del = chunk.simd_eq(u8x16::splat(0x7F));

            // Combine all control masks
            let has_control = has_bell | has_bs | has_tab | has_lf | has_ff | has_cr | has_del;

            // If no controls in this 16-byte chunk, skip ahead
            if !has_control.any() {
                i += 16;
                continue;
            }

            // Found control(s) - process byte-by-byte until we hit one
            while i < input.len() {
                let b = unsafe { *input.get_unchecked(i) };
                let code = unsafe { *CONTROL_LUT.get_unchecked(b as usize) };
                if code != 0 {
                    if i > printable_start {
                        sink.emit(TerminalCommand::Printable(&input[printable_start..i]));
                    }
                    match code {
                        c if c == ControlKind::Bell as u8 => sink.emit(TerminalCommand::Bell),
                        c if c == ControlKind::Backspace as u8 => sink.emit(TerminalCommand::Backspace),
                        c if c == ControlKind::Tab as u8 => sink.emit(TerminalCommand::Tab),
                        c if c == ControlKind::LineFeed as u8 => sink.emit(TerminalCommand::LineFeed),
                        c if c == ControlKind::FormFeed as u8 => sink.emit(TerminalCommand::FormFeed),
                        c if c == ControlKind::CarriageReturn as u8 => sink.emit(TerminalCommand::CarriageReturn),
                        c if c == ControlKind::Delete as u8 => sink.emit(TerminalCommand::Delete),
                        _ => unreachable!(),
                    }
                    i += 1;
                    printable_start = i;
                    break;
                } else {
                    i += 1;
                }
            }
        }

        // Tail: handle remaining bytes (< 16) with scalar loop
        while i < input.len() {
            let b = unsafe { *input.get_unchecked(i) };
            let code = unsafe { *CONTROL_LUT.get_unchecked(b as usize) };
            if code != 0 {
                if i > printable_start {
                    sink.emit(TerminalCommand::Printable(&input[printable_start..i]));
                }
                match code {
                    c if c == ControlKind::Bell as u8 => sink.emit(TerminalCommand::Bell),
                    c if c == ControlKind::Backspace as u8 => sink.emit(TerminalCommand::Backspace),
                    c if c == ControlKind::Tab as u8 => sink.emit(TerminalCommand::Tab),
                    c if c == ControlKind::LineFeed as u8 => sink.emit(TerminalCommand::LineFeed),
                    c if c == ControlKind::FormFeed as u8 => sink.emit(TerminalCommand::FormFeed),
                    c if c == ControlKind::CarriageReturn as u8 => sink.emit(TerminalCommand::CarriageReturn),
                    c if c == ControlKind::Delete as u8 => sink.emit(TerminalCommand::Delete),
                    _ => unreachable!(),
                }
                i += 1;
                printable_start = i;
            } else {
                i += 1;
            }
        }

        // Emit final printable run if any
        if i > printable_start {
            sink.emit(TerminalCommand::Printable(&input[printable_start..i]));
        }
    }
}

#[cfg(not(feature = "simd"))]
impl CommandParser for AsciiParser {
    fn parse(&mut self, input: &[u8], sink: &mut dyn CommandSink) {
        let mut i = 0;
        while i < input.len() {
            let b = unsafe { *input.get_unchecked(i) }; // unchecked; guarded by outer bound
            let code = unsafe { *CONTROL_LUT.get_unchecked(b as usize) };
            if code != 0 {
                match code {
                    c if c == ControlKind::Bell as u8 => sink.emit(TerminalCommand::Bell),
                    c if c == ControlKind::Backspace as u8 => sink.emit(TerminalCommand::Backspace),
                    c if c == ControlKind::Tab as u8 => sink.emit(TerminalCommand::Tab),
                    c if c == ControlKind::LineFeed as u8 => sink.emit(TerminalCommand::LineFeed),
                    c if c == ControlKind::FormFeed as u8 => sink.emit(TerminalCommand::FormFeed),
                    c if c == ControlKind::CarriageReturn as u8 => sink.emit(TerminalCommand::CarriageReturn),
                    c if c == ControlKind::Delete as u8 => sink.emit(TerminalCommand::Delete),
                    _ => unreachable!(),
                }
                i += 1;
            } else {
                let start = i;
                i += 1;
                while i < input.len() {
                    let nb = unsafe { *input.get_unchecked(i) };
                    if unsafe { *CONTROL_LUT.get_unchecked(nb as usize) } != 0 {
                        break;
                    }
                    i += 1;
                }
                sink.emit(TerminalCommand::Printable(&input[start..i]));
            }
        }
    }
}
