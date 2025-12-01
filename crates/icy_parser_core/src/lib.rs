//! Core parser infrastructure: command emission traits and basic ASCII parser.

mod ascii;
mod errors;
use std::fmt::Display;

pub use ascii::AsciiParser;
pub use errors::{ErrorLevel, ParseError, print_char_value};

mod ansi;

mod control_codes;
pub use control_codes::*;

pub use ansi::music::*;
pub use ansi::{AnsiParser, sgr::ANSI_COLOR_OFFSETS};

mod avatar;
pub use avatar::{AvatarParser, constants as avatar_constants};

mod pcboard;
pub use pcboard::PcBoardParser;

mod ctrla;
pub use ctrla::{BG_CODES as ctrla_bg, CtrlAParser, FG_CODES as ctrla_fg};

mod renegade;
pub use renegade::RenegadeParser;

mod atascii;
pub use atascii::AtasciiParser;

mod petscii;
pub use petscii::{C64_TERMINAL_SIZE, PetsciiParser};

mod viewdata;
use serde::Deserialize;
use serde::Serialize;
pub use viewdata::ViewdataParser;

mod mode7;
pub use mode7::Mode7Parser;

mod rip;
pub use rip::{BlockTransferMode, FileQueryMode, FillStyle, ImagePasteMode, LineStyle, QueryMode, WriteMode};
pub use rip::{RipCommand, RipParser};

mod skypix;
pub use skypix::*;

mod vt52;
pub use vt52::{VT52Mode, Vt52Parser};

mod igs;
pub use igs::*;

mod tables;

/// Special keys for CSI sequences
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpecialKey {
    Insert = 2,
    Delete = 3,
    PageUp = 5,
    PageDown = 6,
    Home = 7,
    End = 8,
    F1 = 11,
    F2 = 12,
    F3 = 13,
    F4 = 14,
    F5 = 15,
    F6 = 17,
    F7 = 18,
    F8 = 19,
    F9 = 20,
    F10 = 21,
    F11 = 23,
    F12 = 24,
}

impl TryFrom<u16> for SpecialKey {
    type Error = ();

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            2 => Ok(SpecialKey::Insert),
            3 => Ok(SpecialKey::Delete),
            5 => Ok(SpecialKey::PageUp),
            6 => Ok(SpecialKey::PageDown),
            7 => Ok(SpecialKey::Home),
            8 => Ok(SpecialKey::End),
            11 => Ok(SpecialKey::F1),
            12 => Ok(SpecialKey::F2),
            13 => Ok(SpecialKey::F3),
            14 => Ok(SpecialKey::F4),
            15 => Ok(SpecialKey::F5),
            17 => Ok(SpecialKey::F6),
            18 => Ok(SpecialKey::F7),
            19 => Ok(SpecialKey::F8),
            20 => Ok(SpecialKey::F9),
            21 => Ok(SpecialKey::F10),
            23 => Ok(SpecialKey::F11),
            24 => Ok(SpecialKey::F12),
            _ => Err(()),
        }
    }
}

impl SpecialKey {
    /// Returns the ANSI escape sequence for this special key (CSI [number] ~)
    pub fn to_sequence(&self) -> String {
        format!("\x1B[{}~", *self as u16)
    }
}
pub use tables::*;

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

/// Margin type for Set Specific Margin command (ESC[={type};{value}m)
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarginType {
    /// Top margin (Ps=0)
    Top = 0,
    /// Bottom margin (Ps=1)
    Bottom = 1,
    /// Left margin (Ps=2)
    Left = 2,
    /// Right margin (Ps=3)
    Right = 3,
}

impl MarginType {
    pub fn from_u16(n: u16) -> Option<Self> {
        match n {
            0 => Some(Self::Top),
            1 => Some(Self::Bottom),
            2 => Some(Self::Left),
            3 => Some(Self::Right),
            _ => None,
        }
    }
}

/// Communication line type for Select Communication Speed command
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommunicationLine {
    /// Host Transmit (default)
    HostTransmit = 0,
    /// Host Receive
    HostReceive = 2,
    /// Printer
    Printer = 3,
    /// Modem Hi
    ModemHi = 4,
    /// Modem Lo
    ModemLo = 5,
}

impl CommunicationLine {
    pub fn from_u16(n: u16) -> Self {
        match n {
            0 | 1 => Self::HostTransmit,
            2 => Self::HostReceive,
            3 => Self::Printer,
            4 => Self::ModemHi,
            5 => Self::ModemLo,
            _ => Self::HostTransmit, // Default to Host Transmit
        }
    }
}

/// Direction for cursor movement and scrolling commands
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// Wrapping behavior for CSI cursor movement commands
///
/// Controls whether cursor movement commands (CUU/CUD/CUF/CUB) should wrap
/// at line boundaries like printable characters, or stop at the edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Wrapping {
    /// CSI cursor commands never wrap at line boundaries (standard ANSI behavior)
    #[default]
    Never,
    /// Always wrap like printable characters (for terminals like PETscii, VT52, Atascii)
    Always,
    /// Use the terminal's current wrap mode setting
    Setting,
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
///
/// These are DEC terminal-specific modes distinct from standard ANSI modes.
/// Use DECSET (ESC[?{n}h) to enable and DECRST (ESC[?{n}l) to disable.
///
/// # Examples
/// - `ESC[?25h` - Show cursor (DECTCEM)
/// - `ESC[?25l` - Hide cursor
/// - `ESC[?7h` - Enable auto-wrap (DECAWM)
/// - `ESC[?1000h` - Enable VT200 mouse tracking
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecMode {
    // Scrolling and Display Modes
    /// DECSCLM - Smooth Scroll Mode (Mode 4)
    ///
    /// When set: Smooth scrolling (slower, animated)
    /// When reset: Jump scrolling (instant, default)
    SmoothScroll = 4,

    /// DECOM - Origin Mode (Mode 6)
    ///
    /// Controls how cursor positioning commands interpret coordinates.
    ///
    /// When set: Cursor addressing is relative to the scroll region.
    ///   - Row 1 refers to the first line of the scroll region
    ///   - Cursor cannot move outside the scroll region
    /// When reset: Cursor addressing is absolute (relative to screen origin 1,1).
    ///   - Row 1 refers to the first line of the screen
    ///   - Cursor can move anywhere on screen
    OriginMode = 6,

    /// DECAWM - Auto Wrap Mode (Mode 7)
    ///
    /// Controls cursor behavior when writing past the right margin.
    ///
    /// When set: Cursor wraps to the first column of the next line.
    ///   - At bottom-right, causes scroll before wrap
    /// When reset: Cursor stays at the right margin.
    ///   - Characters overwrite the last column
    AutoWrap = 7,

    // Video Modes
    /// Inverse Video Mode (VT-52 compatibility)
    /// When set: enable inverse/reverse video
    /// When reset: normal video
    Inverse = 5,

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

impl DecMode {
    fn from_u16(n: u16) -> Option<Self> {
        match n {
            4 => Some(Self::SmoothScroll),
            5 => Some(Self::Inverse),
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

/// Caret (cursor) shape for DECSCUSR
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CaretShape {
    /// Block cursor (default)
    #[default]
    Block,
    /// Underline cursor
    Underline,
    /// Bar/vertical line cursor
    Bar,
}

/// Device Control String (DCS) sequences: ESC P ... ESC \
#[repr(u8)]
#[derive(Debug, Clone, PartialEq)]
pub enum DeviceControlString {
    /// Load custom font: ESC P CTerm:Font:{slot}:{base64_data} ESC \
    /// Parameters: font slot number, decoded font data (already base64-decoded by parser)
    LoadFont(usize, Vec<u8>),
    /// Sixel graphics: ESC P {params} q {data} ESC \
    /// Parameters: vertical_scale, background_color (r, g, b), sixel_data
    Sixel {
        aspect_ratio: Option<u16>,
        zero_color: Option<u16>,
        grid_size: Option<u16>,
        sixel_data: Vec<u8>,
    },
}

/// Operating System Command (OSC) sequences: ESC ] ... BEL or ESC \
#[repr(u8)]
#[derive(Debug, Clone, PartialEq)]
pub enum OperatingSystemCommand {
    /// OSC 0 - Set Icon Name and Window Title: ESC]0;{text}BEL or ESC]0;{text}ESC\
    SetTitle(Vec<u8>),
    /// OSC 1 - Set Icon Name: ESC]1;{text}BEL
    SetIconName(Vec<u8>),
    /// OSC 2 - Set Window Title: ESC]2;{text}BEL
    SetWindowTitle(Vec<u8>),
    /// OSC 4 - Set Palette Color: ESC]4;{index};rgb:{rr}/{gg}/{bb}BEL
    /// Parameters: color_index, r, g, b
    SetPaletteColor(u8, u8, u8, u8),
    /// OSC 8 - Hyperlink: ESC]8;{params};{uri}BEL
    Hyperlink { params: Vec<u8>, uri: Vec<u8> },
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TerminalCommand {
    // Basic control characters (C0 controls)
    CarriageReturn,
    LineFeed,
    Backspace,
    Tab,
    FormFeed,
    Bell,
    Delete,

    // ANSI CSI (Control Sequence Introducer) sequences
    /// CUU/CUD/CUF/CUB - Cursor Movement: ESC[{n}A/B/C/D
    ///
    /// The Wrapping parameter controls whether cursor movement wraps at line boundaries:
    /// - Never: Standard ANSI behavior, stops at edges
    /// - Always: Wraps like printable characters (for PETscii, VT52, Atascii)
    /// - Setting: Uses the terminal's current wrap mode
    CsiMoveCursor(Direction, u16, Wrapping),

    /// CNL - Cursor Next Line: ESC[{n}E
    CsiCursorNextLine(u16),
    /// CPL - Cursor Previous Line: ESC[{n}F
    CsiCursorPreviousLine(u16),
    /// CHA - Cursor Horizontal Absolute: ESC[{n}G
    CsiCursorHorizontalAbsolute(u16),
    /// CUP - Cursor Position: ESC[{row};{col}H or ESC[{row};{col}f
    ///
    /// Moves the cursor to the specified row and column position.
    /// Row and column are 1-based (origin is 1,1 at top-left).
    /// If DECOM (Origin Mode) is set, row is relative to the scroll region.
    /// Parameters: (row, column)
    CsiCursorPosition(u16, u16),

    /// ED - Erase in Display: ESC[{n}J
    CsiEraseInDisplay(EraseInDisplayMode),
    /// EL - Erase in Line: ESC[{n}K
    CsiEraseInLine(EraseInLineMode),

    /// SU/SD/SR/SL - Scroll: ESC[{n}S/T/ A/ @
    CsiScroll(Direction, u16),

    /// SGR - Select Graphic Rendition: ESC[{param}m
    /// Text attributes like color, bold, underline, etc.
    /// Emitted once per attribute in a sequence (e.g., ESC[1;31m emits Bold then ForegroundRed)
    CsiSelectGraphicRendition(SgrAttribute),

    /// DECSTBM - Set Scrolling Region: ESC[{top};{bottom}r
    /// DECSLRM - Set Left and Right Margins: ESC[{left};{right}s (when DECLRMM is set)
    ///
    /// Defines the scrolling region boundaries. When the cursor reaches
    /// the bottom of the scroll region, content scrolls up.
    ///
    /// # Behavior
    /// - `top` and `bottom` define vertical scroll boundaries (1-based, inclusive)
    /// - `left` and `right` define horizontal margins (only when DECLRMM mode 69 is enabled)
    /// - After setting, cursor moves to origin (home position)
    /// - If DECOM is set, cursor moves to top-left of scroll region
    /// - Default values: top=1, bottom=screen_height, left=1, right=screen_width
    ///
    /// # Parameters
    /// - `top`: First row of scrolling region (1-based)
    /// - `bottom`: Last row of scrolling region (1-based)
    /// - `left`: Left margin column (1-based, requires DECLRMM)
    /// - `right`: Right margin column (1-based, requires DECLRMM)
    CsiSetScrollingRegion {
        top: u16,
        bottom: u16,
        left: u16,
        right: u16,
    },

    /// ICH - Insert Character: ESC[{n}@
    ///
    /// Inserts `n` blank characters at the cursor position.
    ///
    /// # Behavior
    /// - Shifts existing characters to the right
    /// - Characters that shift past the right margin are lost
    /// - Cursor position does NOT change
    /// - New characters have current attributes
    /// - Default n=1 if omitted
    CsiInsertCharacter(u16),

    /// DCH - Delete Character: ESC[{n}P
    ///
    /// Deletes `n` characters starting at the cursor position.
    ///
    /// # Behavior
    /// - Characters to the right shift left to fill the gap
    /// - Blank characters (with current attributes) fill from the right
    /// - Cursor position does NOT change
    /// - Default n=1 if omitted
    CsiDeleteCharacter(u16),

    /// ECH - Erase Character: ESC[{n}X
    ///
    /// Erases `n` characters starting at the cursor position.
    ///
    /// # Behavior
    /// - Replaces characters with blanks (spaces)
    /// - Does NOT shift remaining characters
    /// - Cursor position does NOT change
    /// - Uses current background color for erased cells
    /// - Default n=1 if omitted
    CsiEraseCharacter(u16),

    /// IL - Insert Line: ESC[{n}L
    ///
    /// Inserts `n` blank lines at the cursor row.
    ///
    /// # Behavior
    /// - Cursor row and lines below shift down within scroll region
    /// - Lines that shift past the bottom scroll margin are lost
    /// - New lines are blank with current attributes
    /// - Cursor moves to column 1 (left margin)
    /// - Only affects lines within the current scroll region
    /// - Default n=1 if omitted
    CsiInsertLine(u16),

    /// DL - Delete Line: ESC[{n}M
    ///
    /// Deletes `n` lines starting at the cursor row.
    ///
    /// # Behavior
    /// - Lines below shift up to fill the gap within scroll region
    /// - Blank lines (with current attributes) fill from the bottom
    /// - Cursor moves to column 1 (left margin)
    /// - Only affects lines within the current scroll region
    /// - Default n=1 if omitted
    CsiDeleteLine(u16),

    /// VPA - Line Position Absolute: ESC[{n}d
    CsiLinePositionAbsolute(u16),
    /// VPR - Line Position Forward: ESC[{n}e
    CsiLinePositionForward(u16),
    /// HPR - Character Position Forward: ESC[{n}a
    CsiCharacterPositionForward(u16),
    /// HPA - Horizontal Position Absolute: ESC[{n}'
    CsiHorizontalPositionAbsolute(u16),

    /// TBC - Tabulation Clear: ESC[0g (clear tab at current position)
    CsiClearTabulation,
    /// TBC - Tabulation Clear All: ESC[3g or ESC[5g (clear all tabs)
    CsiClearAllTabs,
    /// CVT - Cursor Line Tabulation: ESC[{n}Y (forward to next tab)
    CsiCursorLineTabulationForward(u16),
    /// CBT - Cursor Backward Tabulation: ESC[{n}Z
    CsiCursorBackwardTabulation(u16),

    /// SCOSC - Save Cursor Position: ESC[s
    CsiSaveCursorPosition,
    /// SCORC - Restore Cursor Position: ESC[u
    CsiRestoreCursorPosition,

    /// Resize Terminal: ESC[8;{height};{width}t
    CsiResizeTerminal(u16, u16),

    /// Special keys: ESC[{n}~
    CsiSpecialKey(SpecialKey),

    /// DECSET/DECRST - DEC Private Mode Set/Reset: ESC[?{n}h or ESC[?{n}l
    ///
    /// Enables or disables DEC terminal-specific modes.
    ///
    /// # Common Modes
    /// - Mode 6 (DECOM): Origin mode - relative cursor positioning
    /// - Mode 7 (DECAWM): Auto-wrap at right margin
    /// - Mode 25 (DECTCEM): Cursor visibility
    /// - Mode 33: iCE colors (background intensity instead of blink)
    /// - Mode 69 (DECLRMM): Enable left/right margins
    /// - Mode 1000+: Mouse tracking modes
    ///
    /// # Parameters
    /// - First: The DEC mode to modify
    /// - Second: `true` = set/enable (h), `false` = reset/disable (l)
    ///
    /// # Notes
    /// Emitted once per mode (e.g., ESC[?25;1000h emits two commands)
    CsiDecSetMode(DecMode, bool),

    /// SM - Set Mode: ESC[{n}h / RM - Reset Mode: ESC[{n}l
    /// Emitted once per mode
    CsiSetMode(AnsiMode, bool),

    // CSI with intermediate bytes
    /// DECSCUSR - Set Cursor Style: ESC[{Ps} SP q
    ///
    /// Changes the cursor (caret) appearance.
    ///
    /// # Ps Values
    /// - 0 or 1: Blinking block (default)
    /// - 2: Steady block
    /// - 3: Blinking underline
    /// - 4: Steady underline  
    /// - 5: Blinking bar (vertical line)
    /// - 6: Steady bar
    ///
    /// # Parameters
    /// - First: `true` if blinking, `false` if steady
    /// - Second: cursor shape (Block, Underline, or Bar)
    CsiSetCaretStyle(bool, CaretShape),

    /// Font Selection: ESC[{Ps1};{Ps2} D
    CsiFontSelection {
        slot: u16,
        font_number: u16,
    },

    /// Set Font Page (for PETSCII/ATASCII character set switching)
    /// Direct font page selection without font loading
    SetFontPage(usize),

    /// Select Communication Speed: ESC[{Ps1};{Ps2}*r
    /// Ps1 = communication line type, Ps2 = baud rate
    CsiSelectCommunicationSpeed(CommunicationLine, BaudEmulation),

    /// DECFRA - Fill Rectangular Area: ESC[{Pch};{Pt};{Pl};{Pb};{Pr}$x
    ///
    /// Fills a rectangular region with a specified character.
    ///
    /// # Behavior
    /// - Fills all character positions within the rectangle with `char`
    /// - Uses current text attributes (colors, bold, etc.)
    /// - Does NOT move the cursor position
    /// - Coordinates are 1-based and inclusive
    /// - If DECOM is set, coordinates are relative to scroll region
    ///
    /// # Parameters
    /// - `char`: ASCII code of fill character (32-126 for printable)
    /// - `top`: Top row of rectangle (1-based)
    /// - `left`: Left column of rectangle (1-based)
    /// - `bottom`: Bottom row of rectangle (1-based)
    /// - `right`: Right column of rectangle (1-based)
    CsiFillRectangularArea {
        char: u8,
        top: u16,
        left: u16,
        bottom: u16,
        right: u16,
    },

    /// DECERA - Erase Rectangular Area: ESC[{Pt};{Pl};{Pb};{Pr}$z
    ///
    /// Erases all characters in a rectangular region, replacing them with spaces.
    ///
    /// # Behavior
    /// - Replaces all characters in the rectangle with space (0x20)
    /// - Resets character attributes to default within the area
    /// - Does NOT move the cursor position
    /// - Coordinates are 1-based and inclusive
    /// - If DECOM is set, coordinates are relative to scroll region
    /// - Erases ALL characters regardless of protection attribute (DECSCA)
    ///
    /// # Parameters
    /// - `top`: Top row of rectangle (1-based)
    /// - `left`: Left column of rectangle (1-based)
    /// - `bottom`: Bottom row of rectangle (1-based)
    /// - `right`: Right column of rectangle (1-based)
    CsiEraseRectangularArea {
        top: u16,
        left: u16,
        bottom: u16,
        right: u16,
    },

    /// DECSERA - Selective Erase Rectangular Area: ESC[{Pt};{Pl};{Pb};{Pr}${
    ///
    /// Selectively erases characters in a rectangular region.
    /// Unlike DECERA, this only erases characters that are NOT protected.
    ///
    /// # Behavior
    /// - Replaces unprotected characters in the rectangle with space (0x20)
    /// - Characters marked as protected (via DECSCA SGR 1) are preserved
    /// - Resets character attributes to default for erased cells
    /// - Does NOT move the cursor position
    /// - Coordinates are 1-based and inclusive
    /// - If DECOM is set, coordinates are relative to scroll region
    ///
    /// # Parameters
    /// - `top`: Top row of rectangle (1-based)
    /// - `left`: Left column of rectangle (1-based)
    /// - `bottom`: Bottom row of rectangle (1-based)
    /// - `right`: Right column of rectangle (1-based)
    ///
    /// # Related
    /// - DECSCA (ESC[{Ps}"q) sets character protection attribute
    CsiSelectiveEraseRectangularArea {
        top: u16,
        left: u16,
        bottom: u16,
        right: u16,
    },

    // CSI = sequences (extended terminal functions)
    /// Set Margins: ESC[={top};{bottom}r
    SetTopBottomMargin {
        top: u16,
        bottom: u16,
    },
    /// Set Specific Margins: ESC[={type};{value}m
    CsiEqualsSetSpecificMargins(MarginType, u16),

    // ANSI ESC sequences (non-CSI)
    /// IND - Index: ESC D
    ///
    /// Moves the cursor down one line.
    ///
    /// # Behavior
    /// - If cursor is at the bottom margin of scroll region: scroll content up
    /// - If cursor is not at bottom margin: move cursor down one line
    /// - Column position is preserved
    /// - A new blank line appears at the bottom when scrolling
    EscIndex,

    /// NEL - Next Line: ESC E
    ///
    /// Moves the cursor to the first column of the next line.
    ///
    /// # Behavior
    /// - Equivalent to CR + LF (carriage return + line feed)
    /// - If at bottom of scroll region: scroll content up
    /// - Cursor column is set to 1 (or left margin)
    EscNextLine,

    /// HTS - Horizontal Tab Set: ESC H
    ///
    /// Sets a tab stop at the current cursor column.
    ///
    /// # Behavior
    /// - Adds current column to the tab stop list
    /// - Future TAB (0x09) characters will move to this column
    /// - Use TBC (ESC[0g or ESC[3g) to clear tab stops
    EscSetTab,

    /// RI - Reverse Index: ESC M
    ///
    /// Moves the cursor up one line.
    ///
    /// # Behavior
    /// - If cursor is at the top margin of scroll region: scroll content down
    /// - If cursor is not at top margin: move cursor up one line
    /// - Column position is preserved
    /// - A new blank line appears at the top when scrolling
    EscReverseIndex,
    /// DECSC - Save Cursor: ESC 7
    ///
    /// Saves the current cursor state to memory.
    ///
    /// # Saved State Includes
    /// - Cursor position (row, column)
    /// - Character attributes (SGR: colors, bold, italic, etc.)
    /// - Character set designations (G0-G3)
    /// - Autowrap mode (DECAWM)
    /// - Origin mode (DECOM)
    /// - Selective erase attribute (DECSCA)
    ///
    /// # Notes
    /// - Only one cursor state can be saved at a time
    /// - Saving again overwrites the previous saved state
    /// - Use DECRC (ESC 8) to restore
    EscSaveCursor,

    /// DECRC - Restore Cursor: ESC 8
    ///
    /// Restores the cursor state previously saved with DECSC.
    ///
    /// # Restored State Includes
    /// - Cursor position (row, column)
    /// - Character attributes (SGR)
    /// - Character set designations (G0-G3)
    /// - Autowrap mode (DECAWM)
    /// - Origin mode (DECOM)
    /// - Selective erase attribute (DECSCA)
    ///
    /// # Notes
    /// - If no state was saved, behavior is undefined (typically resets to defaults)
    /// - Cursor is constrained to screen boundaries after restore
    EscRestoreCursor,
    /// RIS - Reset to Initial State: ESC c
    EscReset,

    ScrollArea {
        direction: Direction,
        num_lines: u16,
        top: u16,
        left: u16,
        bottom: u16,
        right: u16,
    },

    AvatarClearArea {
        attr: u8,
        lines: u8,
        columns: u8,
    },
    AvatarInitArea {
        attr: u8,
        ch: u8,
        lines: u8,
        columns: u8,
    },

    /// ANSI Music sequence
    /// Conflicting CSI M/N commands can trigger music playback
    ResetMargins,
    ResetLeftAndRightMargin {
        left: u16,
        right: u16,
    },
}

/// Terminal requests that expect a response from the terminal emulator.
/// These are commands that query terminal state and require the terminal
/// to send data back to the host.
#[derive(Debug, Clone, PartialEq)]
pub enum TerminalRequest {
    /// Primary Device Attributes (DA): ESC[c or ESC[0c
    /// Terminal should respond with its capabilities
    DeviceAttributes,

    /// Secondary Device Attributes: ESC[>c
    /// Terminal should respond with version and hardware info
    SecondaryDeviceAttributes,

    /// Extended Device Attributes: ESC[<c
    /// Terminal should respond with extended capabilities
    ExtendedDeviceAttributes,

    /// Device Status Report (DSR): ESC[5n
    /// Terminal should respond with "\x1b[0n" (terminal OK)
    DeviceStatusReport,

    /// Cursor Position Report (CPR): ESC[6n
    /// Terminal should respond with "\x1b[{row};{col}R"
    CursorPositionReport,

    /// Current Screen Size: ESC[255n
    /// Terminal should respond with "\x1b[{height};{width}R"
    ScreenSizeReport,

    /// ANSI Mode Report: ESC[{mode}$p
    /// Terminal should respond with current mode status
    AnsiModeReport(AnsiMode),

    /// DEC Private Mode Report: ESC[?{mode}$p
    /// Terminal should respond with current DEC mode status
    DecPrivateModeReport(DecMode),

    /// Request Checksum of Rectangular Area: ESC[{id};{page};{top};{left};{bottom};{right}*y
    /// Terminal should respond with checksum in DCS format
    RequestChecksumRectangularArea {
        id: u8,
        page: u8,
        top: u16,
        left: u16,
        bottom: u16,
        right: u16,
    },

    /// Request Tab Stop Report: ESC[2$w
    /// Terminal should respond with current tab stops in DCS format
    RequestTabStopReport,

    /// Font State Report: ESC[=1n
    /// Terminal should respond with font selection state and current slots
    FontStateReport,

    /// Font Mode Report: ESC[=2n
    /// Terminal should respond with current terminal modes
    FontModeReport,

    /// Font Dimension Report: ESC[=3n
    /// Terminal should respond with current font dimensions
    FontDimensionReport,

    /// Macro Space Report: ESC[?62n
    /// Terminal should respond with available macro space
    MacroSpaceReport,

    /// Memory Checksum Report: ESC[?63;{Pid}n
    /// Terminal should respond with memory checksum
    /// Parameters: (pid, checksum)
    MemoryChecksumReport(u16, u16),

    /// RIPscrip: Request terminal ID
    /// Should respond with RIP terminal ID string
    RipRequestTerminalId,

    /// RIPscrip: Query file - check if file exists
    RipQueryFile(String),

    /// RIPscrip: Query file size
    RipQueryFileSize(String),

    /// RIPscrip: Query file date
    RipQueryFileDate(String),

    /// RIPscrip: Read file data
    RipReadFile(String),
}

pub trait CommandSink {
    /// Output printable text data
    fn print(&mut self, text: &[u8]);

    fn emit(&mut self, cmd: TerminalCommand);

    /// Emit a RIPscrip command. Default implementation does nothing.
    fn emit_rip(&mut self, _cmd: RipCommand) {}

    /// Emit a SkyPix command. Default implementation does nothing.
    fn emit_skypix(&mut self, _cmd: SkypixCommand) {}

    /// Emit an IGS (Interactive Graphics System) command. Default implementation does nothing.
    fn emit_igs(&mut self, _cmd: IgsCommand) {}

    /// if true, reset on row change should be called.
    fn emit_view_data(&mut self, _cmd: ViewDataCommand) -> bool {
        false
    }

    /// Emit a Device Control String (DCS) sequence. Default implementation does nothing.
    fn device_control(&mut self, _dcs: DeviceControlString) {}

    /// Emit an Operating System Command (OSC) sequence. Default implementation does nothing.
    fn operating_system_command(&mut self, _osc: OperatingSystemCommand) {}

    /// Emit an Application Program String (APS) sequence: ESC _ ... ESC \
    /// Default implementation does nothing.
    fn aps(&mut self, _data: &[u8]) {}

    /// Play ANSI music sequence. Default implementation does nothing.
    fn play_music(&mut self, _music: AnsiMusic) {}

    /// Handle a terminal request that expects a response.
    /// The implementation should send appropriate data back to the host.
    /// Default implementation does nothing.
    fn request(&mut self, _request: TerminalRequest) {}

    /// Report a parsing error with context information.
    /// The error includes a severity level and detailed diagnostic information.
    /// Default implementation does nothing.
    ///
    /// # Arguments
    /// * `error` - The parse error with context
    ///
    /// # Examples
    /// Implementers can filter by error level:
    /// ```ignore
    /// fn report_error(&mut self, error: ParseError, _level: ErrorLevel) {
    ///     if error.level() >= ErrorLevel::Warning {
    ///         eprintln!("Parse error: {}", error);
    ///     }
    /// }
    /// ```
    fn report_error(&mut self, _error: ParseError, _level: ErrorLevel) {}

    /// Begin XOR drawing mode for IGS loop commands with XOR stepping modifier.
    /// This is used by IGS `&` loops with the `|` modifier (e.g., `G|`).
    /// Default implementation does nothing.
    ///
    /// # Example
    /// Used in loop sequences like `G#&>198,0,2,0,G|4,2,6,x,x:` where shapes
    /// are drawn in XOR mode, allowing animation effects by overlaying/removing.
    fn begin_igs_xor_mode(&mut self) {}

    /// End XOR drawing mode and restore normal drawing.
    /// Called after an IGS loop with XOR stepping completes.
    /// Default implementation does nothing.
    fn end_igs_xor_mode(&mut self) {}
}

pub trait CommandParser: Send {
    fn parse(&mut self, input: &[u8], sink: &mut dyn CommandSink);
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BaudEmulation {
    #[default]
    Off,
    Rate(u32),
}

impl Display for BaudEmulation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Off => write!(f, "Off"),
            Self::Rate(v) => write!(f, "{v}"),
        }
    }
}

impl BaudEmulation {
    pub const OPTIONS: [BaudEmulation; 13] = [
        BaudEmulation::Off,
        BaudEmulation::Rate(300),
        BaudEmulation::Rate(600),
        BaudEmulation::Rate(1200),
        BaudEmulation::Rate(2400),
        BaudEmulation::Rate(4800),
        BaudEmulation::Rate(9600),
        BaudEmulation::Rate(14400),
        BaudEmulation::Rate(19200),
        BaudEmulation::Rate(28800),
        BaudEmulation::Rate(38400),
        BaudEmulation::Rate(57600),
        BaudEmulation::Rate(115_200),
    ];

    pub fn get_baud_rate(&self) -> u32 {
        match self {
            BaudEmulation::Off => 0,
            BaudEmulation::Rate(baud) => *baud,
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewDataCommand {
    /// preserves caret visibilty
    ViewDataClearScreen,
    FillToEol,
    DoubleHeight(bool),
    /// Reset colors to default on row change
    ResetRowColors,
    /// Check if row changed and reset colors if it did
    CheckAndResetOnRowChange,

    MoveCaret(Direction),

    SetBgToFg,
    SetChar(u8),
}

#[inline(always)]
pub(crate) fn flush_input(input: &[u8], sink: &mut dyn CommandSink, i: usize, start: usize) {
    if i > start {
        sink.print(&input[start..i]);
    }
}
