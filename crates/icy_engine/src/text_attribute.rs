use crate::IceMode;

/// Attribute flags for text styling (bold, italic, blink, etc.)
/// Note: Transparency is now part of AttributeColor, not these flags.
pub mod attribute {
    pub const NONE: u16 = 0;
    pub const BOLD: u16 = 0b0000_0000_0000_0001;
    pub const FAINT: u16 = 0b0000_0000_0000_0010;
    pub const ITALIC: u16 = 0b0000_0000_0000_0100;
    pub const BLINK: u16 = 0b0000_0000_0000_1000;

    pub const UNDERLINE: u16 = 0b0000_0000_0001_0000;
    pub const DOUBLE_UNDERLINE: u16 = 0b0000_0000_0010_0000;
    pub const CONCEAL: u16 = 0b0000_0000_0100_0000;
    pub const CROSSED_OUT: u16 = 0b0000_0000_1000_0000;
    pub const DOUBLE_HEIGHT: u16 = 0b0000_0001_0000_0000;
    pub const OVERLINE: u16 = 0b0000_0010_0000_0000;

    // Marker for I/O: invisible cell / end-of-visible-line
    pub const INVISIBLE: u16 = 0b1000_0000_0000_0000;
    // Short marker for skipping rest of line in wire format
    pub const INVISIBLE_SHORT: u16 = 0b1100_0000_0000_0000;
}

/// Color representation for text foreground/background.
/// Each color can be a palette index, extended (xterm 256) index, RGB, or fully transparent.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttributeColor {
    /// Standard palette index (0-15 typically, but can be higher for custom palettes)
    Palette(u8),
    /// Extended palette index (xterm 256 colors: 0-255)
    ExtendedPalette(u8),
    /// Direct RGB color
    Rgb(u8, u8, u8),
    /// Fully transparent (alpha = 0)
    Transparent,
}

impl Default for AttributeColor {
    fn default() -> Self {
        AttributeColor::Palette(0)
    }
}

impl AttributeColor {
    /// Pack this color into a u32 for wire/storage format.
    /// Layout:
    /// - Transparent: 0xFF_00_00_00
    /// - Palette(n):  0x00_00_00_nn
    /// - ExtendedPalette(n): 0x01_00_00_nn
    /// - Rgb(r,g,b): 0x02_rr_gg_bb
    pub fn to_u32(self) -> u32 {
        match self {
            AttributeColor::Transparent => 0xFF_00_00_00,
            AttributeColor::Palette(n) => n as u32,
            AttributeColor::ExtendedPalette(n) => 0x01_00_00_00 | (n as u32),
            AttributeColor::Rgb(r, g, b) => 0x02_00_00_00 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32),
        }
    }

    /// Unpack a u32 from wire/storage format into AttributeColor.
    pub fn from_u32(val: u32) -> Self {
        let tag = (val >> 24) as u8;
        match tag {
            0xFF => AttributeColor::Transparent,
            0x02 => {
                let r = ((val >> 16) & 0xFF) as u8;
                let g = ((val >> 8) & 0xFF) as u8;
                let b = (val & 0xFF) as u8;
                AttributeColor::Rgb(r, g, b)
            }
            0x01 => AttributeColor::ExtendedPalette((val & 0xFF) as u8),
            _ => AttributeColor::Palette((val & 0xFF) as u8),
        }
    }

    /// Check if this color is transparent
    pub fn is_transparent(self) -> bool {
        matches!(self, AttributeColor::Transparent)
    }

    /// Get palette index if this is a Palette color, None otherwise
    pub fn as_palette_index(self) -> Option<u8> {
        match self {
            AttributeColor::Palette(n) => Some(n),
            _ => None,
        }
    }

    /// Get extended palette index if this is ExtendedPalette, None otherwise
    pub fn as_extended_index(self) -> Option<u8> {
        match self {
            AttributeColor::ExtendedPalette(n) => Some(n),
            _ => None,
        }
    }

    /// Get RGB values if this is an Rgb color, None otherwise
    pub fn as_rgb(self) -> Option<(u8, u8, u8)> {
        match self {
            AttributeColor::Rgb(r, g, b) => Some((r, g, b)),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
pub struct TextAttribute {
    font_page: u8,
    foreground_color: AttributeColor,
    background_color: AttributeColor,
    pub attr: u16,
}

impl std::fmt::Debug for TextAttribute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextAttribute")
            .field("foreground_color", &self.foreground_color)
            .field("background_color", &self.background_color)
            .field("attr", &format!("{:016b}", self.attr))
            .field("font_page", &self.font_page)
            .finish()
    }
}

impl Default for TextAttribute {
    fn default() -> Self {
        Self {
            foreground_color: AttributeColor::Palette(7),
            background_color: AttributeColor::Palette(0),
            attr: attribute::NONE,
            font_page: 0,
        }
    }
}

impl std::fmt::Display for TextAttribute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "(Attr: {:X}, fg {:?}, bg {:?}, blink {})",
            self.as_u8(IceMode::Blink),
            self.foreground_color,
            self.background_color,
            self.is_blinking()
        )
    }
}

impl TextAttribute {
    /// Create a new TextAttribute with palette colors
    pub fn new(foreground: u32, background: u32) -> Self {
        TextAttribute {
            foreground_color: AttributeColor::Palette(foreground as u8),
            background_color: AttributeColor::Palette(background as u8),
            ..Default::default()
        }
    }

    /// Create from AttributeColor values
    pub fn from_colors(foreground: AttributeColor, background: AttributeColor) -> Self {
        TextAttribute {
            foreground_color: foreground,
            background_color: background,
            ..Default::default()
        }
    }

    /// Create from legacy DOS-style attribute byte
    pub fn from_u8(attr: u8, ice_mode: IceMode) -> Self {
        let mut blink = false;
        let background = if let IceMode::Ice = ice_mode {
            attr >> 4
        } else {
            blink = attr & 0b1000_0000 != 0;
            (attr >> 4) & 0b0111
        };
        let foreground = attr & 0b1111;

        let mut result = TextAttribute {
            foreground_color: AttributeColor::Palette(foreground),
            background_color: AttributeColor::Palette(background),
            ..Default::default()
        };
        result.set_is_blinking(blink);
        result
    }

    /// Create from legacy color bytes (with bold/blink encoding)
    pub fn from_color(fg: u8, bg: u8) -> Self {
        let mut res = TextAttribute {
            foreground_color: AttributeColor::Palette(fg & 0x7),
            background_color: AttributeColor::Palette(bg & 0x7),
            ..Default::default()
        };
        res.set_is_bold((fg & 0b1000) != 0);
        res.set_is_blinking((bg & 0b1000) != 0);
        res
    }

    /// Convert to legacy DOS-style attribute byte
    pub fn as_u8(self, ice_mode: IceMode) -> u8 {
        let fg_idx = match self.foreground_color {
            AttributeColor::Palette(n) | AttributeColor::ExtendedPalette(n) => n as u32,
            AttributeColor::Rgb(_, _, _) => 7, // fallback
            AttributeColor::Transparent => 0,
        };
        let bg_idx = match self.background_color {
            AttributeColor::Palette(n) | AttributeColor::ExtendedPalette(n) => n as u32,
            AttributeColor::Rgb(_, _, _) => 0, // fallback
            AttributeColor::Transparent => 0,
        };

        let mut fg = fg_idx & 0b_1111;
        if self.is_bold() {
            fg |= 0b_1000;
        }
        let bg = match ice_mode {
            IceMode::Blink => bg_idx & 0b_0111 | if self.is_blinking() { 0b_1000 } else { 0 },
            IceMode::Unlimited | IceMode::Ice => bg_idx & 0b_1111,
        };
        (fg | bg << 4) as u8
    }

    // === Foreground color accessors ===

    /// Get the foreground color
    pub fn foreground_color(&self) -> AttributeColor {
        self.foreground_color
    }

    /// Set the foreground color
    pub fn set_foreground_color(&mut self, color: AttributeColor) {
        self.foreground_color = color;
    }

    /// Check if foreground is transparent
    pub fn is_foreground_transparent(&self) -> bool {
        self.foreground_color.is_transparent()
    }

    /// Set foreground as palette index
    pub fn set_foreground(&mut self, index: u32) {
        self.foreground_color = AttributeColor::Palette(index as u8);
    }

    /// Set foreground as extended palette index
    pub fn set_foreground_ext(&mut self, index: u8) {
        self.foreground_color = AttributeColor::ExtendedPalette(index);
    }

    /// Set foreground as RGB
    pub fn set_foreground_rgb(&mut self, r: u8, g: u8, b: u8) {
        self.foreground_color = AttributeColor::Rgb(r, g, b);
    }

    /// Set foreground as transparent
    pub fn set_foreground_transparent(&mut self) {
        self.foreground_color = AttributeColor::Transparent;
    }

    /// Legacy: get foreground as u32 (palette index or 0 for non-palette)
    pub fn foreground(&self) -> u32 {
        match self.foreground_color {
            AttributeColor::Palette(n) => n as u32,
            AttributeColor::ExtendedPalette(n) => n as u32,
            AttributeColor::Rgb(_, _, _) => 0,
            AttributeColor::Transparent => 0,
        }
    }

    /// Check if foreground is RGB
    pub fn is_foreground_rgb(&self) -> bool {
        matches!(self.foreground_color, AttributeColor::Rgb(_, _, _))
    }

    /// Check if foreground is extended palette
    pub fn is_foreground_ext(&self) -> bool {
        matches!(self.foreground_color, AttributeColor::ExtendedPalette(_))
    }

    /// Get foreground RGB values (returns (0,0,0) if not RGB)
    pub fn foreground_rgb(&self) -> (u8, u8, u8) {
        match self.foreground_color {
            AttributeColor::Rgb(r, g, b) => (r, g, b),
            _ => (0, 0, 0),
        }
    }

    /// Get foreground extended palette index (returns 0 if not ext)
    pub fn foreground_ext(&self) -> u8 {
        match self.foreground_color {
            AttributeColor::ExtendedPalette(n) => n,
            _ => 0,
        }
    }

    // === Background color accessors ===

    /// Get the background color
    pub fn background_color(&self) -> AttributeColor {
        self.background_color
    }

    /// Set the background color
    pub fn set_background_color(&mut self, color: AttributeColor) {
        self.background_color = color;
    }

    /// Check if background is transparent
    pub fn is_background_transparent(&self) -> bool {
        self.background_color.is_transparent()
    }

    /// Set background as palette index
    pub fn set_background(&mut self, index: u32) {
        self.background_color = AttributeColor::Palette(index as u8);
    }

    /// Set background as extended palette index
    pub fn set_background_ext(&mut self, index: u8) {
        self.background_color = AttributeColor::ExtendedPalette(index);
    }

    /// Set background as RGB
    pub fn set_background_rgb(&mut self, r: u8, g: u8, b: u8) {
        self.background_color = AttributeColor::Rgb(r, g, b);
    }

    /// Set background as transparent
    pub fn set_background_transparent(&mut self) {
        self.background_color = AttributeColor::Transparent;
    }

    /// Legacy: get background as u32 (palette index or 0 for non-palette)
    pub fn background(&self) -> u32 {
        match self.background_color {
            AttributeColor::Palette(n) => n as u32,
            AttributeColor::ExtendedPalette(n) => n as u32,
            AttributeColor::Rgb(_, _, _) => 0,
            AttributeColor::Transparent => 0,
        }
    }

    /// Check if background is RGB
    pub fn is_background_rgb(&self) -> bool {
        matches!(self.background_color, AttributeColor::Rgb(_, _, _))
    }

    /// Check if background is extended palette
    pub fn is_background_ext(&self) -> bool {
        matches!(self.background_color, AttributeColor::ExtendedPalette(_))
    }

    /// Get background RGB values (returns (0,0,0) if not RGB)
    pub fn background_rgb(&self) -> (u8, u8, u8) {
        match self.background_color {
            AttributeColor::Rgb(r, g, b) => (r, g, b),
            _ => (0, 0, 0),
        }
    }

    /// Get background extended palette index (returns 0 if not ext)
    pub fn background_ext(&self) -> u8 {
        match self.background_color {
            AttributeColor::ExtendedPalette(n) => n,
            _ => 0,
        }
    }

    // === Style attribute accessors ===

    pub fn is_bold(&self) -> bool {
        (self.attr & attribute::BOLD) == attribute::BOLD
    }

    pub fn set_is_bold(&mut self, is_bold: bool) {
        if is_bold {
            self.attr |= attribute::BOLD;
        } else {
            self.attr &= !attribute::BOLD;
        }
    }

    pub fn is_faint(&self) -> bool {
        (self.attr & attribute::FAINT) == attribute::FAINT
    }

    pub fn set_is_faint(&mut self, is_faint: bool) {
        if is_faint {
            self.attr |= attribute::FAINT;
        } else {
            self.attr &= !attribute::FAINT;
        }
    }

    pub fn is_italic(&self) -> bool {
        (self.attr & attribute::ITALIC) == attribute::ITALIC
    }

    pub fn set_is_italic(&mut self, is_italic: bool) {
        if is_italic {
            self.attr |= attribute::ITALIC;
        } else {
            self.attr &= !attribute::ITALIC;
        }
    }

    pub fn is_blinking(&self) -> bool {
        (self.attr & attribute::BLINK) == attribute::BLINK
    }

    pub fn set_is_blinking(&mut self, is_blink: bool) {
        if is_blink {
            self.attr |= attribute::BLINK;
        } else {
            self.attr &= !attribute::BLINK;
        }
    }

    pub fn is_double_height(&self) -> bool {
        (self.attr & attribute::DOUBLE_HEIGHT) == attribute::DOUBLE_HEIGHT
    }

    pub fn set_is_double_height(&mut self, is_double_height: bool) {
        if is_double_height {
            self.attr |= attribute::DOUBLE_HEIGHT;
        } else {
            self.attr &= !attribute::DOUBLE_HEIGHT;
        }
    }

    pub fn is_crossed_out(&self) -> bool {
        (self.attr & attribute::CROSSED_OUT) == attribute::CROSSED_OUT
    }

    pub fn set_is_crossed_out(&mut self, is_crossed_out: bool) {
        if is_crossed_out {
            self.attr |= attribute::CROSSED_OUT;
        } else {
            self.attr &= !attribute::CROSSED_OUT;
        }
    }

    pub fn is_underlined(&self) -> bool {
        (self.attr & attribute::UNDERLINE) == attribute::UNDERLINE
    }

    pub fn set_is_underlined(&mut self, is_underline: bool) {
        if is_underline {
            self.attr |= attribute::UNDERLINE;
        } else {
            self.attr &= !attribute::UNDERLINE;
        }
    }

    pub fn is_double_underlined(&self) -> bool {
        (self.attr & attribute::DOUBLE_UNDERLINE) == attribute::DOUBLE_UNDERLINE
    }

    pub fn set_is_double_underlined(&mut self, is_double_underline: bool) {
        if is_double_underline {
            self.attr |= attribute::DOUBLE_UNDERLINE;
        } else {
            self.attr &= !attribute::DOUBLE_UNDERLINE;
        }
    }

    pub fn is_concealed(&self) -> bool {
        (self.attr & attribute::CONCEAL) == attribute::CONCEAL
    }

    pub fn set_is_concealed(&mut self, is_concealed: bool) {
        if is_concealed {
            self.attr |= attribute::CONCEAL;
        } else {
            self.attr &= !attribute::CONCEAL;
        }
    }

    pub fn reset(&mut self) {
        self.attr = 0;
    }

    pub fn is_overlined(&self) -> bool {
        (self.attr & attribute::OVERLINE) == attribute::OVERLINE
    }

    pub fn set_is_overlined(&mut self, arg: bool) {
        if arg {
            self.attr |= attribute::OVERLINE;
        } else {
            self.attr &= !attribute::OVERLINE;
        }
    }

    // === Font page ===

    #[must_use]
    pub fn font_page(&self) -> usize {
        self.font_page as usize
    }

    pub fn set_font_page(&mut self, page: usize) {
        self.font_page = page as u8;
    }

    pub fn with_font_page(&self, font_page: usize) -> TextAttribute {
        TextAttribute {
            font_page: font_page as u8,
            ..*self
        }
    }

    // === Legacy compatibility for palette-based operations ===

    /// Set foreground from legacy palette color (clears other modes)
    pub fn set_foreground_palette(&mut self, color: u32) {
        self.foreground_color = AttributeColor::Palette(color as u8);
    }

    /// Set background from legacy palette color (clears other modes)
    pub fn set_background_palette(&mut self, color: u32) {
        self.background_color = AttributeColor::Palette(color as u8);
    }
}

impl PartialEq for TextAttribute {
    fn eq(&self, other: &TextAttribute) -> bool {
        self.foreground_color == other.foreground_color
            && self.background_color == other.background_color
            && self.attr == other.attr
            && self.font_page == other.font_page
    }
}
