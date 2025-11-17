//! Core parser infrastructure: command emission traits and basic ASCII parser.

mod ascii;
mod errors;
use std::fmt::Display;

pub use ascii::AsciiParser;
pub use errors::{ErrorLevel, ParseError};

mod ansi;

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
pub use rip::{BlockTransferMode, FileQueryMode, ImagePasteMode, QueryMode, RipCommand, RipParser, WriteMode};

mod skypix;
pub use skypix::{SkypixCommand, SkypixParser};

mod igs;
pub use igs::*;

mod tables;

/// Special keys for CSI sequences
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpecialKey {
    Home = 1,
    Insert = 2,
    Delete = 3,
    End = 4,
    PageUp = 5,
    PageDown = 6,
}

impl SpecialKey {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            1 => Some(SpecialKey::Home),
            2 => Some(SpecialKey::Insert),
            3 => Some(SpecialKey::Delete),
            4 => Some(SpecialKey::End),
            5 => Some(SpecialKey::PageUp),
            6 => Some(SpecialKey::PageDown),
            _ => None,
        }
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

impl DecPrivateMode {
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
#[derive(Debug, PartialEq)]
pub enum DeviceControlString<'a> {
    /// Load custom font: ESC P CTerm:Font:{slot}:{base64_data} ESC \
    /// Parameters: font slot number, decoded font data (already base64-decoded by parser)
    LoadFont(usize, Vec<u8>),
    /// Sixel graphics: ESC P {params} q {data} ESC \
    /// Parameters: vertical_scale, background_color (r, g, b), sixel_data
    Sixel {
        aspect_ratio: Option<u16>,
        zero_color: Option<u16>,
        grid_size: Option<u16>,
        sixel_data: &'a [u8],
    },
}

/// Operating System Command (OSC) sequences: ESC ] ... BEL or ESC \
#[repr(u8)]
#[derive(Debug, PartialEq)]
pub enum OperatingSystemCommand<'a> {
    /// OSC 0 - Set Icon Name and Window Title: ESC]0;{text}BEL or ESC]0;{text}ESC\
    SetTitle(&'a [u8]),
    /// OSC 1 - Set Icon Name: ESC]1;{text}BEL
    SetIconName(&'a [u8]),
    /// OSC 2 - Set Window Title: ESC]2;{text}BEL
    SetWindowTitle(&'a [u8]),
    /// OSC 4 - Set Palette Color: ESC]4;{index};rgb:{rr}/{gg}/{bb}BEL
    /// Parameters: color_index, r, g, b
    SetPaletteColor(u8, u8, u8, u8),
    /// OSC 8 - Hyperlink: ESC]8;{params};{uri}BEL
    Hyperlink { params: &'a [u8], uri: &'a [u8] },
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
    CsiMoveCursor(Direction, u16),
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

    /// SU/SD/SR/SL - Scroll: ESC[{n}S/T/ A/ @
    CsiScroll(Direction, u16),

    /// SGR - Select Graphic Rendition: ESC[{param}m
    /// Text attributes like color, bold, underline, etc.
    /// Emitted once per attribute in a sequence (e.g., ESC[1;31m emits Bold then ForegroundRed)
    CsiSelectGraphicRendition(SgrAttribute),

    /// DECSTBM - Set Scrolling Region: ESC[{top};{bottom};{left};[{right}]r
    CsiSetScrollingRegion {
        top: u16,
        bottom: u16,
        left: u16,
        right: u16,
    },

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

    // CSI with intermediate bytes
    /// DECSCUSR - Set Caret Style: ESC[{Ps} q
    /// First parameter: blinking (true) or steady (false)
    /// Second parameter: shape (Block, Underline, or Bar)
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

    /// DECFRA - Fill Rectangular Area: ESC[{Pchar};{Pt};{Pl};{Pb};{Pr} $x
    CsiFillRectangularArea {
        char: u8,
        top: u16,
        left: u16,
        bottom: u16,
        right: u16,
    },

    /// DECERA - Erase Rectangular Area: ESC[{Pt};{Pl};{Pb};{Pr}$z
    CsiEraseRectangularArea {
        top: u16,
        left: u16,
        bottom: u16,
        right: u16,
    },

    /// DECSERA - Selective Erase Rectangular Area: ESC[{Pt};{Pl};{Pb};{Pr}${
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
    DecPrivateModeReport(DecPrivateMode),

    /// Request Checksum of Rectangular Area: ESC[{page};{top};{left};{bottom};{right}*y
    /// Terminal should respond with checksum in DCS format
    RequestChecksumRectangularArea(u8, u16, u16, u16, u16),

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

    /// IGS: Query version (0) or resolution (3)
    IgsQuery(u8),
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
    fn device_control(&mut self, _dcs: DeviceControlString<'_>) {}

    /// Emit an Operating System Command (OSC) sequence. Default implementation does nothing.
    fn operating_system_command(&mut self, _osc: OperatingSystemCommand<'_>) {}

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
    fn report_errror(&mut self, _error: ParseError, _level: ErrorLevel) {}
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
    pub const OPTIONS: [BaudEmulation; 12] = [
        BaudEmulation::Off,
        BaudEmulation::Rate(300),
        BaudEmulation::Rate(600),
        BaudEmulation::Rate(1200),
        BaudEmulation::Rate(2400),
        BaudEmulation::Rate(4800),
        BaudEmulation::Rate(9600),
        BaudEmulation::Rate(19200),
        BaudEmulation::Rate(38400),
        BaudEmulation::Rate(57600),
        BaudEmulation::Rate(76800),
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
