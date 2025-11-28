//! # SkyPix Graphics Commands
//!
//! SkyPix is an Amiga graphics protocol designed for BBS systems that allows
//! remote terminals to display high-resolution graphics. It operates on a
//! 640x200 screen and supports up to 16 colors.
//!
//! ## Protocol Overview
//!
//! All SkyPix commands use the ESC sequence format: `<ESC>[command_number;params!`
//! where parameters are separated by semicolons and the command is terminated
//! by an exclamation point (!).
//!
//! ## Screen Specifications
//!
//! - Resolution: 640 x 200 pixels
//! - Color modes: 8 colors (3 bitplanes) or 16 colors (4 bitplanes)
//! - Default mode: 16 colors
//!
//! ## Font Handling
//!
//! When any non-default font is in use:
//! - The cursor is turned OFF
//! - All text is rendered in JAM1 mode (transparent background)
//! - Carriage returns move to the beginning of the next line
//! - Linefeeds are ignored
//!
//! In the default font:
//! - Text is rendered in JAM2 mode (ANSI background colors work)
//! - Standard ANSI cursor behavior applies

use std::fmt;

/// Flood fill modes for Command 3 (AREA_FILL)
///
/// These modes correspond to the Amiga graphics.library Flood() function modes.
/// Flood fill searches outward from a starting point (x, y) and fills pixels
/// based on the selected mode.
///
/// # Amiga Graphics Reference
///
/// The Amiga Flood() routine uses two modes:
/// - **Outline Mode (0)**: Fills all pixels that are NOT the outline color (AOlPen).
///   The fill stops when it encounters pixels matching the outline color.
/// - **Color Mode (1)**: Fills all pixels that ARE the same color as the starting pixel.
///   The fill replaces all adjacent pixels of that color with the fill color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FillMode {
    /// Outline Mode (mode 0)
    ///
    /// Starting from (x, y), the system searches outward in all directions for pixels
    /// whose color matches the Area Outline Pen (AOlPen). All horizontally or vertically
    /// adjacent pixels that are NOT of that outline color are filled with the current
    /// pen color or pattern. The fill stops at the outline color boundary.
    ///
    /// Use case: Fill inside a shape drawn with a specific outline color.
    ///
    /// Example: Draw a triangle with color 3, then flood fill inside it.
    /// The fill will stop at the triangle's edges (color 3).
    #[default]
    Outline = 0,

    /// Color Mode (mode 1)
    ///
    /// Starting from (x, y), the system reads the pixel color at that position.
    /// It then searches for all horizontally or vertically adjacent pixels whose
    /// color is the SAME as this starting color and replaces them with the current
    /// pen color or pattern.
    ///
    /// Use case: Replace all connected pixels of a specific color with a new color.
    ///
    /// Example: Click on a blue area, and all connected blue pixels become red.
    Color = 1,
}

impl TryFrom<i32> for FillMode {
    type Error = String;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Outline),
            1 => Ok(Self::Color),
            _ => Err(format!("Invalid FillMode value: {}. Expected 0 (Outline) or 1 (Color)", value)),
        }
    }
}

impl From<FillMode> for i32 {
    fn from(mode: FillMode) -> Self {
        mode as i32
    }
}

impl fmt::Display for FillMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Outline => write!(f, "Outline"),
            Self::Color => write!(f, "Color"),
        }
    }
}

/// CRC Transfer modes for Command 16 (CRC XMODEM TRANSFER)
///
/// This command initiates a CRC XMODEM file transfer for various file types.
/// The transfer happens invisibly (no transfer window) and will abort after
/// only ONE retry.
///
/// # Syntax
///
/// `<ESC>[16;m;a;b!filename!`
///
/// Where:
/// - `m` is the transfer mode (see variants below)
/// - `a` and `b` are width and height for IFF brushes (redundant but helpful for visualization)
/// - `filename` is the file to transfer
///
/// # Brush Caching
///
/// Brushes are RETAINED in memory after use. If the named brush already exists
/// in memory, SkyPix terminals should abort the transfer by sending 5 CANCELs,
/// then decode the existing brush into a bitmap as usual. This optimization
/// helps with applications like SkyPix menus where brushes need only be sent
/// once per logon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrcTransferMode {
    /// Mode 1: IFF Brush format
    ///
    /// An IFF (Interchange File Format) brush is coming. The file should be
    /// saved to RAM:, decoded, and put in the brush buffer. After decoding,
    /// the file should be deleted from RAM:.
    ///
    /// The width and height parameters (a, b) in the command are redundant
    /// since they're also in the brush header, but they're included to help
    /// humans visualize the brush when writing SkyPix code in a text editor.
    IffBrush = 1,

    /// Mode 2: IFF Sound format
    ///
    /// An IFF sound sample is coming. This mode is treated the same as
    /// FutureSound mode. **Note: This was not yet implemented in the original
    /// SkyPix specification.**
    IffSound = 2,

    /// Mode 3: FutureSound™ format
    ///
    /// A FutureSound format sample is coming. Once unpacked and ready to use
    /// in the sample buffer, the file should be deleted from RAM:.
    FutureSound = 3,

    /// Mode 20: General purpose file transfer
    ///
    /// A general-purpose XMODEM download that goes to the user's default
    /// directory. An immediate download of the specified filename begins.
    GeneralPurpose = 20,
}

impl TryFrom<i32> for CrcTransferMode {
    type Error = String;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::IffBrush),
            2 => Ok(Self::IffSound),
            3 => Ok(Self::FutureSound),
            20 => Ok(Self::GeneralPurpose),
            _ => Err(format!("Invalid CrcTransferMode value: {}. Expected 1, 2, 3, or 20", value)),
        }
    }
}

impl From<CrcTransferMode> for i32 {
    fn from(mode: CrcTransferMode) -> Self {
        mode as i32
    }
}

impl fmt::Display for CrcTransferMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IffBrush => write!(f, "IFF Brush"),
            Self::IffSound => write!(f, "IFF Sound"),
            Self::FutureSound => write!(f, "FutureSound"),
            Self::GeneralPurpose => write!(f, "General Purpose"),
        }
    }
}

/// Display modes for Command 17 (SELECT DISPLAY MODE)
///
/// # Syntax
///
/// `<ESC>[17;m!`
///
/// SkyPix ALWAYS uses a 640 x 200 screen. The mode parameter controls the
/// number of bitplanes (and therefore colors) available.
///
/// # Palette Reset
///
/// The palette should be reset any time the display mode changes.
///
/// # Performance Note
///
/// At 9600 baud or higher, it's acceptable to use a 1 bitplane screen
/// (2 colors) for speed in the default screen. However, when a specific
/// mode is requested via this command, the system should change displays
/// as requested.
///
/// # Future Extensions
///
/// This command will eventually be expanded to include dual-playfield mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DisplayMode {
    /// Mode 1: 8 colors (3 bitplanes)
    ///
    /// Returns to a normal 3-bitplane display with 8 colors.
    /// If already in this mode, the command is ignored.
    #[default]
    EightColors = 1,

    /// Mode 2: 16 colors (4 bitplanes)
    ///
    /// Switches to a 4-bitplane display with 16 colors available.
    /// This provides the full SkyPix color palette.
    SixteenColors = 2,
}

impl TryFrom<i32> for DisplayMode {
    type Error = String;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::EightColors),
            2 => Ok(Self::SixteenColors),
            _ => Err(format!("Invalid DisplayMode value: {}. Expected 1 (8 colors) or 2 (16 colors)", value)),
        }
    }
}

impl From<DisplayMode> for i32 {
    fn from(mode: DisplayMode) -> Self {
        mode as i32
    }
}

impl fmt::Display for DisplayMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EightColors => write!(f, "8 colors"),
            Self::SixteenColors => write!(f, "16 colors"),
        }
    }
}

/// SkyPix command numbers
///
/// These constants define the numeric command codes used in the SkyPix protocol.
/// Commands are sent as `<ESC>[command_number;params!`
pub mod command_numbers {
    /// Command 0: Comment - all text until closing `!` is ignored
    pub const COMMENT: i32 = 0;
    /// Command 1: Set a single pixel
    pub const SET_PIXEL: i32 = 1;
    /// Command 2: Draw a line from current pen position
    pub const DRAW_LINE: i32 = 2;
    /// Command 3: Flood fill an area
    pub const AREA_FILL: i32 = 3;
    /// Command 4: Draw a filled rectangle
    pub const RECTANGLE_FILL: i32 = 4;
    /// Command 5: Draw an ellipse outline
    pub const ELLIPSE: i32 = 5;
    /// Command 6: Capture a screen region as a brush
    pub const GRAB_BRUSH: i32 = 6;
    /// Command 7: Blit a brush to the screen
    pub const USE_BRUSH: i32 = 7;
    /// Command 8: Move the drawing pen (not the cursor)
    pub const MOVE_PEN: i32 = 8;
    /// Command 9: Play a sound sample
    pub const PLAY_SAMPLE: i32 = 9;
    /// Command 10: Set the current font
    pub const SET_FONT: i32 = 10;
    /// Command 11: Load a new 16-color palette
    pub const NEW_PALETTE: i32 = 11;
    /// Command 12: Reset to the standard SkyPix palette
    pub const RESET_PALETTE: i32 = 12;
    /// Command 13: Draw a filled ellipse
    pub const FILLED_ELLIPSE: i32 = 13;
    /// Command 14: Pause execution for specified jiffies
    pub const DELAY: i32 = 14;
    /// Command 15: Set the foreground (A) pen color
    pub const SET_PEN_A: i32 = 15;
    /// Command 16: Initiate a CRC XMODEM file transfer
    pub const CRC_TRANSFER: i32 = 16;
    /// Command 17: Change the display mode (color depth)
    pub const SET_DISPLAY_MODE: i32 = 17;
    /// Command 18: Set the background (B) pen color
    pub const SET_PEN_B: i32 = 18;
    /// Command 19: Position the text cursor
    pub const POSITION_CURSOR: i32 = 19;
    /// Command 21: Send controller input back to host
    pub const CONTROLLER_RETURN: i32 = 21;
    /// Command 22: Define a clickable gadget region
    pub const DEFINE_GADGET: i32 = 22;
}

/// SkyPix graphics commands
///
/// These are the parsed SkyPix commands that can be executed by a terminal.
/// All SkyPix commands use the ESC sequence format: `<ESC>[command_number;params!`
///
/// # Text Handling
///
/// Any data in a SkyPix file that isn't interpreted as a command is assumed
/// to be text and is output to the screen in whatever font is active, from
/// wherever the cursor happens to be.
#[derive(Debug, Clone, PartialEq)]
pub enum SkypixCommand {
    /// Command 0: COMMENT
    ///
    /// # Syntax
    /// `<ESC>[0!comment!`
    ///
    /// All text after this command is discarded until a closing exclamation
    /// point is seen. Comments can be of any length.
    Comment { text: String },

    /// Command 1: SET PIXEL
    ///
    /// # Syntax
    /// `<ESC>[1;x;y!`
    ///
    /// Sets the pixel at the specified X and Y coordinate to whatever
    /// color is currently in Pen A.
    SetPixel { x: i32, y: i32 },

    /// Command 2: DRAW LINE
    ///
    /// # Syntax
    /// `<ESC>[2;x;y!`
    ///
    /// Draws a line in the current A pen color from the existing pen
    /// position to the point (x, y). The pen position is updated to (x, y)
    /// after the line is drawn.
    DrawLine { x: i32, y: i32 },

    /// Command 3: AREA FILL
    ///
    /// # Syntax
    /// `<ESC>[3;m;x;y!`
    ///
    /// Floods, in mode m, the area beginning at (x, y).
    ///
    /// Uses the Amiga Flood() algorithm with the specified mode:
    /// - `Outline` (0): Fill stops at pixels matching the outline color
    /// - `Color` (1): Fill replaces all connected pixels of the same color as (x, y)
    AreaFill { mode: FillMode, x: i32, y: i32 },

    /// Command 4: RECTANGLE FILL
    ///
    /// # Syntax
    /// `<ESC>[4;x1;y1;x2;y2!`
    ///
    /// Draws a filled rectangle in the current Pen A color.
    /// The numeric parameters plug directly into the Amiga RectFill() function.
    /// (x1, y1) is one corner and (x2, y2) is the opposite corner.
    RectangleFill { x1: i32, y1: i32, x2: i32, y2: i32 },

    /// Command 5: ELLIPSE
    ///
    /// # Syntax
    /// `<ESC>[5;x;y;a;b!`
    ///
    /// Draws an ellipse outline using DrawEllipse() with the supplied parameters:
    /// - (x, y): Center point of the ellipse
    /// - a: Horizontal radius (semi-major axis)
    /// - b: Vertical radius (semi-minor axis)
    Ellipse { x: i32, y: i32, a: i32, b: i32 },

    /// Command 6: GRAB BRUSH
    ///
    /// # Syntax
    /// `<ESC>[6;x1;y1;x2;y2!`
    ///
    /// Stores a piece of the screen as a brush in memory. From there it
    /// will behave exactly like a brush that has been received remotely.
    ///
    /// - (x1, y1): Starting point (top-left corner)
    /// - x2: Width of the brush
    /// - y2: Height of the brush
    GrabBrush { x1: i32, y1: i32, width: i32, height: i32 },

    /// Command 7: BLIT (BlitBitMap)
    ///
    /// # Syntax
    /// `<ESC>[7;a;b;c;d;e;f;g;h!`
    ///
    /// Blits from whatever is in the brush buffer. If nothing's there,
    /// the command is aborted. All the parameters are supplied so that
    /// frame animation can be performed.
    ///
    /// Parameters map directly to Amiga BlitBitMap():
    /// - (src_x, src_y): Source position within the brush
    /// - (dst_x, dst_y): Destination position on screen
    /// - (width, height): Size of the region to blit
    /// - minterm: Blitter minterm for logical operations
    /// - mask: Bitplane mask
    UseBrush {
        src_x: i32,
        src_y: i32,
        dst_x: i32,
        dst_y: i32,
        width: i32,
        height: i32,
        minterm: i32,
        mask: i32,
    },

    /// Command 8: MOVE PEN
    ///
    /// # Syntax
    /// `<ESC>[8;x;y!`
    ///
    /// Moves the drawing pen to (x, y). This is NOT the cursor - the cursor
    /// and pen are separate concepts in SkyPix. The pen position is used for
    /// line drawing operations.
    MovePen { x: i32, y: i32 },

    /// Command 9: PLAY SAMPLE
    ///
    /// # Syntax
    /// `<ESC>[9;a;b;c;d!`
    ///
    /// Plays a simple sample from the sample buffer.
    ///
    /// Parameters:
    /// - speed: The playback speed of the sample
    /// - start: Starting point in bytes within the sample
    /// - end: Ending point in bytes within the sample
    /// - loops: Number of iterations (0 = play once)
    ///
    /// If no sample is in memory, the command is aborted.
    PlaySample { speed: i32, start: i32, end: i32, loops: i32 },

    /// Command 10: SET FONT
    ///
    /// # Syntax
    /// `<ESC>[10;y!fontname.font!`
    ///
    /// Y is the Y size (height) for the font.
    ///
    /// The parser expects the font name as an ASCII string with the ".font"
    /// extension, terminated with an exclamation point. This name and Y are
    /// used to open the required font.
    ///
    /// If the font is not available, it should be noted to the user in an
    /// unobtrusive way, and the default font is used instead.
    ///
    /// ## Rendering Mode
    /// When ANY non-default font is in use:
    /// - The cursor is turned OFF
    /// - All text is rendered in JAM1 mode (transparent background)
    /// - Carriage returns move to the beginning of the next line
    /// - Linefeeds are ignored
    ///
    /// In the default font:
    /// - Text is rendered in JAM2 mode (ANSI background colors work)
    ///
    /// ## Standard Fonts
    /// SkyPix expects access to all standard Workbench fonts. Other fonts
    /// can be used but this is discouraged in SKYLINE because callers might
    /// not have the requested font.
    ///
    /// ## Font Reset
    /// To reset the font, use the [`ResetFont`](SkypixCommand::ResetFont) command
    /// instead (`<ESC>[10;0!`).
    SetFont { size: i32, name: String },

    /// Command 10 (Special Case): RESET FONT
    ///
    /// # Syntax
    /// `<ESC>[10;0!`
    ///
    /// Resets to the default font. This is a special case of the SET FONT
    /// command where Y (font size) is zero.
    ///
    /// When the font is reset:
    /// - The cursor is turned back ON
    /// - Text is rendered in JAM2 mode (ANSI background colors work)
    /// - Standard ANSI cursor behavior applies
    ///
    /// This command does NOT expect a font name - it aborts immediately
    /// after resetting to the default font.
    ResetFont,

    /// Command 11: NEW PALETTE
    ///
    /// # Syntax
    /// `<ESC>[11;c1;c2;c3;...;c16!`
    ///
    /// Sixteen packed color values ready to be put in a 16-element, 16-bit
    /// integer array and plugged into the Amiga LoadRGB4() function.
    ///
    /// Each color value is in Amiga 12-bit RGB format (0x0RGB) where each
    /// component (R, G, B) is a 4-bit value (0-15).
    NewPalette { colors: Vec<i32> },

    /// Command 12: RESET PALETTE
    ///
    /// # Syntax
    /// `<ESC>[12!`
    ///
    /// Resets to the SkyPix standard palette. The default palette colors
    /// (in decimal R, G, B format) are:
    ///
    /// | Index | R  | G  | B  | Description    |
    /// |-------|----|----|----|--------------  |
    /// |   0   |  0 |  0 |  0 | Black          |
    /// |   1   |  1 |  1 | 15 | Dark Blue      |
    /// |   2   | 13 | 13 | 13 | Light Gray     |
    /// |   3   | 15 |  0 |  0 | Red            |
    /// |   4   |  0 | 15 |  1 | Green          |
    /// |   5   |  3 | 10 | 15 | Light Blue     |
    /// |   6   | 15 | 15 |  2 | Yellow         |
    /// |   7   | 12 |  0 | 14 | Magenta        |
    /// |   8   |  0 | 11 |  6 | Teal           |
    /// |   9   |  0 | 13 | 13 | Cyan           |
    /// |  10   |  0 | 10 | 15 | Sky Blue       |
    /// |  11   |  0 |  7 | 12 | Dark Cyan      |
    /// |  12   |  0 |  0 | 15 | Blue           |
    /// |  13   |  7 |  0 | 15 | Purple         |
    /// |  14   | 12 |  0 | 14 | Magenta/Purple |
    /// |  15   | 12 |  0 |  8 | Dark Magenta   |
    ResetPalette,

    /// Command 13: FILLED ELLIPSE
    ///
    /// # Syntax
    /// `<ESC>[13;x;y;a;b!`
    ///
    /// Same as ELLIPSE (command 5), but fills the ellipse with the current
    /// Pen A color using an area fill operation.
    ///
    /// Parameters:
    /// - (x, y): Center point of the ellipse
    /// - a: Horizontal radius (semi-major axis)
    /// - b: Vertical radius (semi-minor axis)
    FilledEllipse { x: i32, y: i32, a: i32, b: i32 },

    /// Command 14: DELAY
    ///
    /// # Syntax
    /// `<ESC>[14;a!`
    ///
    /// A is a value in jiffies (1/60th of a second) ready to plug into
    /// the Amiga Delay() function. This pauses the display for the
    /// specified duration.
    ///
    /// Example: `<ESC>[14;60!` pauses for 1 second.
    Delay { jiffies: i32 },

    /// Command 15: SET A PEN
    ///
    /// # Syntax
    /// `<ESC>[15;a!`
    ///
    /// Sets the A Pen (foreground drawing color) to color index a.
    /// Valid values are 0-15 for 16-color mode, 0-7 for 8-color mode.
    SetPenA { color: i32 },

    /// Command 16: CRC XMODEM TRANSFER
    ///
    /// # Syntax
    /// `<ESC>[16;m;a;b!filename!`
    ///
    /// Initiates a CRC XMODEM file transfer. See [`CrcTransferMode`] for
    /// details on the available modes.
    ///
    /// Parameters:
    /// - mode: Transfer mode (1=IFF Brush, 2=IFF Sound, 3=FutureSound, 20=General)
    /// - width, height: Dimensions for brushes (redundant but helpful for visualization)
    /// - filename: Name of the file to transfer
    ///
    /// The transfer happens invisibly and aborts after only ONE retry.
    CrcTransfer {
        mode: CrcTransferMode,
        width: i32,
        height: i32,
        filename: String,
    },

    /// Command 17: SELECT DISPLAY MODE
    ///
    /// # Syntax
    /// `<ESC>[17;m!`
    ///
    /// Changes the display mode. See [`DisplayMode`] for available modes.
    /// The palette is reset whenever the display mode changes.
    SetDisplayMode { mode: DisplayMode },

    /// Command 18: SET B PEN
    ///
    /// # Syntax
    /// `<ESC>[18;b!`
    ///
    /// Sets the B Pen (background color) to color index b.
    /// This is useful mainly in the default font, to allow ANSI backgrounds
    /// access to more colors than the standard 8 ANSI background colors.
    SetPenB { color: i32 },

    /// Command 19: POSITION CURSOR
    ///
    /// # Syntax
    /// `<ESC>[19;x;y!`
    ///
    /// Moves the text cursor to pixel coordinates (x, y).
    /// This does NOT affect the position of the drawing pen - cursor and
    /// pen are separate concepts in SkyPix.
    PositionCursor { x: i32, y: i32 },

    /// Command 21: CONTROLLER RETURN
    ///
    /// # Syntax
    /// `<ESC>[21;c;x;y!`
    ///
    /// This is the ONLY SkyPix command that a terminal actually TRANSMITS
    /// back to the host. It reports user input events.
    ///
    /// Controller types (c):
    /// - 1 = LEFT MOUSE-BUTTON CLICK: X and Y are current mouse coordinates.
    ///       Only triggers on the DOWNWARD part of the click; the button
    ///       release message must be filtered out.
    /// - 2 = MENU SELECTION: X ranges from 0 (first item) through highest
    ///       menu number minus 1. Y is ignored.
    /// - 3 = JOYSTICK 1: X is 1-4 for up/down/right/left (0 = no direction).
    ///       Y is 1 if fire button held, 0 otherwise.
    /// - 4 = JOYSTICK 2: Same as joystick 1.
    ControllerReturn { c: i32, x: i32, y: i32 },

    /// Command 22: DEFINE A SKYPIX GADGET
    ///
    /// # Syntax
    /// `<ESC>[22;n;c;x1;y1;x2;y2!`
    ///
    /// Defines a clickable gadget region on screen.
    ///
    /// Parameters:
    /// - n: Gadget number (1-20)
    /// - c: Command NUMBER associated with this gadget
    /// - (x1, y1): Top-left corner of the hitbox
    /// - (x2, y2): Bottom-right corner of the hitbox
    ///
    /// The hitbox is automatically painted with the current A Pen color.
    ///
    /// ## Terminal Implementation Note
    /// In a terminal without auto-answer mode that supports SkyPix, this
    /// can simply be interpreted as a DrawFilledRectangle command, and
    /// the n and c parameters can be discarded.
    DefineGadget { num: i32, cmd: i32, x1: i32, y1: i32, x2: i32, y2: i32 },
}
