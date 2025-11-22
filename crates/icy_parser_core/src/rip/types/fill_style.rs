/// Fill style patterns for RIPscrip graphics primitives.
///
/// Defines built-in 8x8 pixel fill patterns used by commands like BAR, FILLED_OVAL,
/// PIE_SLICE, etc. Each pattern is represented by an 8-byte bitmap where each byte
/// defines one row of the 8x8 pattern.
///
/// Pattern values correspond to the RIPscrip specification:
/// - `0x00` (00): Fill with background color (color #0)
/// - `0x01` (01): Solid fill (fill color)
/// - `0x02` (02): Horizontal line fill (thick lines)
/// - `0x03` (03): Light slash fill `/  /  /  /` (thin lines)
/// - `0x04` (04): Normal slash fill `// // // //` (thick lines)
/// - `0x05` (05): Normal backslash fill `\\ \\ \\ \\` (thick lines)
/// - `0x06` (06): Light backslash fill `\  \  \  \` (thin lines)
/// - `0x07` (07): Light hatch fill `###########` (thin lines)
/// - `0x08` (08): Heavy cross hatch fill `XXXXXXXXXXX` (thin lines)
/// - `0x09` (09): Interleaving line fill `+-+-+-+-+-+` (thin lines)
/// - `0x0A` (10): Widely spaced dot fill `. : . : . :` (pixels only)
/// - `0x0B` (11): Closely spaced dot fill `:::::::::::` (pixels only)
/// - Custom: User-defined pattern (specified via RIP_FILL_PATTERN command)
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FillStyle {
    /// Fill with background color (color #0)
    Empty = 0x00,
    /// Solid fill (fill color)
    Solid = 0x01,
    /// Horizontal line fill (thick lines): -----------
    Line = 0x02,
    /// Light slash fill (thin lines): /  /  /  /
    LtSlash = 0x03,
    /// Normal slash fill (thick lines): // // // //
    Slash = 0x04,
    /// Normal backslash fill (thick lines): \\ \\ \\ \\
    BkSlash = 0x05,
    /// Light backslash fill (thin lines): \  \  \  \
    LtBkSlash = 0x06,
    /// Light hatch fill (thin lines): ###########
    Hatch = 0x07,
    /// Heavy cross hatch fill (thin lines): XXXXXXXXXXX
    XHatch = 0x08,
    /// Interleaving line fill (thin lines): +-+-+-+-+-+
    Interleave = 0x09,
    /// Widely spaced dot fill (pixels only): . : . : . :
    WideDot = 0x0A,
    /// Closely spaced dot fill (pixels only): :::::::::::
    CloseDot = 0x0B,
    /// User-defined pattern (set via RIP_FILL_PATTERN)
    User = 0x0C,
}

impl FillStyle {
    pub const DEFAULT_FILL_PATTERNS: [[u8; 8]; 13] = [
        // Empty
        [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        // Solid
        [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
        // Line
        [0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        // LtSlash
        [0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80],
        // Slash
        [0xE0, 0xC1, 0x83, 0x07, 0x0E, 0x1C, 0x38, 0x70],
        // BkSlash
        [0xF0, 0x78, 0x3C, 0x1E, 0x0F, 0x87, 0xC3, 0xE1],
        // LtBkSlash
        [0xA5, 0xD2, 0x69, 0xB4, 0x5A, 0x2D, 0x96, 0x4B],
        // Hatch
        [0xFF, 0x88, 0x88, 0x88, 0xFF, 0x88, 0x88, 0x88],
        // XHatch
        [0x81, 0x42, 0x24, 0x18, 0x18, 0x24, 0x42, 0x81],
        // Interleave
        [0xCC, 0x33, 0xCC, 0x33, 0xCC, 0x33, 0xCC, 0x33],
        // WideDot
        [0x80, 0x00, 0x08, 0x00, 0x80, 0x00, 0x08, 0x00],
        // CloseDot
        [0x88, 0x00, 0x22, 0x00, 0x88, 0x00, 0x22, 0x00],
        // User
        [0xAA, 0x55, 0xAA, 0x55, 0xAA, 0x55, 0xAA, 0x55],
    ];

    pub fn get_fill_pattern(self, fill_user_pattern: &[u8]) -> &[u8] {
        match self {
            FillStyle::User => fill_user_pattern,
            _ => &FillStyle::DEFAULT_FILL_PATTERNS[self as usize],
        }
    }
}

impl TryFrom<u8> for FillStyle {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(FillStyle::Empty),
            0x01 => Ok(FillStyle::Solid),
            0x02 => Ok(FillStyle::Line),
            0x03 => Ok(FillStyle::LtSlash),
            0x04 => Ok(FillStyle::Slash),
            0x05 => Ok(FillStyle::BkSlash),
            0x06 => Ok(FillStyle::LtBkSlash),
            0x07 => Ok(FillStyle::Hatch),
            0x08 => Ok(FillStyle::XHatch),
            0x09 => Ok(FillStyle::Interleave),
            0x0A => Ok(FillStyle::WideDot),
            0x0B => Ok(FillStyle::CloseDot),
            0x0C => Ok(FillStyle::User),
            _ => Err(format!("Invalid FillStyle value: {:#04x}", value)),
        }
    }
}

impl TryFrom<i32> for FillStyle {
    type Error = String;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        if value < 0 || value > 255 {
            return Err(format!("FillStyle value out of range: {}", value));
        }
        FillStyle::try_from(value as u8)
    }
}
