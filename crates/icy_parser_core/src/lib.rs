//! Core parser infrastructure: command emission traits and basic ASCII parser.

mod ascii;
pub use ascii::AsciiParser;

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

mod atascii;
pub use atascii::AtasciiParser;

mod petscii;
pub use petscii::PetsciiParser;

mod viewdata;
pub use viewdata::ViewdataParser;

mod mode7;
pub use mode7::Mode7Parser;

mod rip;
pub use rip::{RipCommand, RipParser};

mod skypix;
pub use skypix::{SkypixCommand, SkypixParser};

mod igs;
pub use igs::{IgsCommand, IgsParser};

mod vt52;
pub use vt52::Vt52Parser;

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

/// Direction for cursor movement and scrolling commands
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
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
    Sixel(u8, (u8, u8, u8), &'a [u8]),
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
    /// OSC 8 - Hyperlink: ESC]8;{params};{uri}BEL
    Hyperlink { params: &'a [u8], uri: &'a [u8] },
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
    /// 1=Home, 2=Insert, 3=Delete, 4=End, 5=PageUp, 6=PageDown
    CsiSpecialKey(u16),

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

    // CSI with intermediate bytes
    /// DECSCUSR - Set Caret Style: ESC[{Ps} q
    /// First parameter: blinking (true) or steady (false)
    /// Second parameter: shape (Block, Underline, or Bar)
    CsiSetCaretStyle(bool, CaretShape),

    /// Font Selection: ESC[{Ps1};{Ps2} D
    /// Ps1 = slot (0-3), Ps2 = font number
    CsiFontSelection(u16, u16),

    /// Select Communication Speed: ESC[{Ps1};{Ps2}*r
    CsiSelectCommunicationSpeed(u16, u16),

    /// Request Checksum of Rectangular Area: ESC[{Ppage};{Pt};{Pl};{Pb};{Pr}*y
    /// (Pid parameter ignored)
    CsiRequestChecksumRectangularArea(u8, u16, u16, u16, u16),

    /// DECRQTSR - Request Tab Stop Report: ESC[{Ps}$w
    CsiRequestTabStopReport(u16),

    /// DECFRA - Fill Rectangular Area: ESC[{Pchar};{Pt};{Pl};{Pb};{Pr}$x
    CsiFillRectangularArea(u16, u16, u16, u16, u16),

    /// DECERA - Erase Rectangular Area: ESC[{Pt};{Pl};{Pb};{Pr}$z
    CsiEraseRectangularArea(u16, u16, u16, u16),

    /// DECSERA - Selective Erase Rectangular Area: ESC[{Pt};{Pl};{Pb};{Pr}${
    CsiSelectiveEraseRectangularArea(u16, u16, u16, u16),

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
}

/// Music style for ANSI music playback
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MusicStyle {
    /// Play music in foreground (blocks)
    Foreground,
    /// Play music in background (non-blocking)
    Background,
    /// Normal note articulation (7/8 of note duration)
    Normal,
    /// Legato articulation (full note duration, no pause between notes)
    Legato,
    /// Staccato articulation (3/4 of note duration, 1/4 pause)
    Staccato,
}

impl MusicStyle {
    /// Calculate the pause length after a note based on the music style
    pub fn get_pause_length(&self, duration: i32) -> i32 {
        match self {
            MusicStyle::Legato => 0,
            MusicStyle::Staccato => duration / 4,
            _ => duration / 8,
        }
    }
}

/// ANSI music action - a single music command
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MusicAction {
    /// Play a note: frequency (Hz), tempo * length, is_dotted
    PlayNote(f32, i32, bool),
    /// Pause for given tempo * length
    Pause(i32),
    /// Change music style
    SetStyle(MusicStyle),
}

impl MusicAction {
    /// Get the duration of this music action in milliseconds
    pub fn get_duration(&self) -> i32 {
        match self {
            MusicAction::PlayNote(_, len, dotted) => {
                if *dotted {
                    360000 / *len
                } else {
                    240000 / *len
                }
            }
            MusicAction::Pause(len) => 240000 / *len,
            _ => 0,
        }
    }
}

/// ANSI music sequence - a collection of music actions
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AnsiMusic {
    /// The music actions to perform
    pub music_actions: Vec<MusicAction>,
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

    /// Emit a Device Control String (DCS) sequence. Default implementation does nothing.
    fn device_control(&mut self, _dcs: DeviceControlString<'_>) {}

    /// Emit an Operating System Command (OSC) sequence. Default implementation does nothing.
    fn operating_system_command(&mut self, _osc: OperatingSystemCommand<'_>) {}

    /// Emit an Application Program String (APS) sequence: ESC _ ... ESC \
    /// Default implementation does nothing.
    fn aps(&mut self, _data: &[u8]) {}

    /// Play ANSI music sequence. Default implementation does nothing.
    fn play_music(&mut self, _music: AnsiMusic) {}

    /// Report a parsing error. Default implementation does nothing.
    fn report_error(&mut self, _error: ParseError) {}
}

pub trait CommandParser {
    fn parse(&mut self, input: &[u8], sink: &mut dyn CommandSink);
    fn flush(&mut self, _sink: &mut dyn CommandSink) {}
}
