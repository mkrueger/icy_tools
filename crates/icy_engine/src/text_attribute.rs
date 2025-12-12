use crate::IceMode;

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
    pub const INVISIBLE: u16 = 0b1000_0000_0000_0000;

    /// This is a special attribute that is used to indicate that the character data
    /// can be represented as u8. For loading & saving only.
    pub const SHORT_DATA: u16 = 0b0100_0000_0000_0000;

    // Flag for loading indicating end of line
    pub const INVISIBLE_SHORT: u16 = 0b1100_0000_0000_0000;
}

pub mod extended_attribute {
    pub const NONE: u8 = 0;
    /// Foreground color is stored as direct RGB (r, g, b encoded in foreground_color)
    pub const FG_RGBA: u8 = 0b0000_0001;
    /// Background color is stored as direct RGB (r, g, b encoded in background_color)  
    pub const BG_RGBA: u8 = 0b0000_0010;

    /// Foreground extended palette index
    pub const FG_EXT: u8 = 0b0000_0100;

    /// Background extended palette index
    pub const BG_EXT: u8 = 0b0000_1000;
}

#[derive(Clone, Copy)]
pub struct TextAttribute {
    pub(super) font_page: u8,
    pub(super) foreground_color: u32,
    pub(super) background_color: u32,

    pub ext_attr: u8,
    pub attr: u16,
}

impl std::fmt::Debug for TextAttribute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextAttribute")
            .field("foreground_color", &self.foreground_color)
            .field("background_color", &self.background_color)
            .field("attr", &format!("{:08b}", self.attr))
            .field("font_page", &self.font_page)
            .finish()
    }
}

impl Default for TextAttribute {
    fn default() -> Self {
        Self {
            foreground_color: 7,
            background_color: 0,
            ext_attr: 0,

            attr: attribute::NONE,
            font_page: 0,
        }
    }
}

impl std::fmt::Display for TextAttribute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "(Attr: {:X}, fg {}, bg {}, blink {})",
            self.as_u8(IceMode::Blink),
            self.foreground(),
            self.background(),
            self.is_blinking()
        )
    }
}

impl TextAttribute {
    pub const TRANSPARENT_COLOR: u32 = 1 << 31;

    pub fn new(foreground_color: u32, background_color: u32) -> Self {
        TextAttribute {
            foreground_color,
            background_color,
            ..Default::default()
        }
    }

    pub fn from_u8(attr: u8, ice_mode: IceMode) -> Self {
        let mut blink = false;
        let background_color = if let IceMode::Ice = ice_mode {
            attr >> 4
        } else {
            blink = attr & 0b1000_0000 != 0;
            (attr >> 4) & 0b0111
        } as u32;
        let foreground_color = (attr & 0b1111) as u32;

        let mut attr = TextAttribute {
            foreground_color,
            background_color,
            ..Default::default()
        };

        attr.set_is_blinking(blink);

        attr
    }

    pub fn from_color(fg: u8, bg: u8) -> Self {
        let mut res = TextAttribute {
            foreground_color: fg as u32 & 0x7,
            background_color: bg as u32 & 0x7,
            ..Default::default()
        };
        res.set_is_bold((fg & 0b1000) != 0);
        res.set_is_blinking((bg & 0b1000) != 0);
        res
    }

    pub fn as_u8(self, ice_mode: IceMode) -> u8 {
        let mut fg = self.foreground_color & 0b_1111;
        if self.is_bold() {
            fg |= 0b_1000;
        }
        let bg = match ice_mode {
            IceMode::Blink => self.background_color & 0b_0111 | if self.is_blinking() { 0b_1000 } else { 0 },
            IceMode::Unlimited | IceMode::Ice => self.background_color & 0b_1111,
        };
        (fg | bg << 4) as u8
    }

    pub fn foreground(self) -> u32 {
        self.foreground_color
    }

    /// Set foreground color (as palette index).
    /// Note: This clears the RGB and EXT flags. Use set_foreground_rgb() or set_foreground_ext() for other color types.
    pub fn set_foreground(&mut self, color: u32) {
        self.foreground_color = color;
        self.ext_attr &= !(extended_attribute::FG_RGBA | extended_attribute::FG_EXT);
    }

    pub fn background(self) -> u32 {
        self.background_color
    }

    /// Set background color (as palette index).
    /// Note: This clears the RGB and EXT flags. Use set_background_rgb() or set_background_ext() for other color types.
    pub fn set_background(&mut self, color: u32) {
        self.background_color = color;
        self.ext_attr &= !(extended_attribute::BG_RGBA | extended_attribute::BG_EXT);
    }

    pub fn is_bold(self) -> bool {
        (self.attr & attribute::BOLD) == attribute::BOLD
    }

    pub fn set_is_bold(&mut self, is_bold: bool) {
        if is_bold {
            self.attr |= attribute::BOLD;
        } else {
            self.attr &= !attribute::BOLD;
        }
    }

    pub fn is_faint(self) -> bool {
        (self.attr & attribute::FAINT) == attribute::FAINT
    }

    pub fn set_is_faint(&mut self, is_faint: bool) {
        if is_faint {
            self.attr |= attribute::FAINT;
        } else {
            self.attr &= !attribute::FAINT;
        }
    }

    pub fn is_italic(self) -> bool {
        (self.attr & attribute::ITALIC) == attribute::ITALIC
    }

    pub fn set_is_italic(&mut self, is_italic: bool) {
        if is_italic {
            self.attr |= attribute::ITALIC;
        } else {
            self.attr &= !attribute::ITALIC;
        }
    }

    pub fn is_blinking(self) -> bool {
        (self.attr & attribute::BLINK) == attribute::BLINK
    }

    pub fn set_is_blinking(&mut self, is_blink: bool) {
        if is_blink {
            self.attr |= attribute::BLINK;
        } else {
            self.attr &= !attribute::BLINK;
        }
    }

    pub fn is_double_height(self) -> bool {
        (self.attr & attribute::DOUBLE_HEIGHT) == attribute::DOUBLE_HEIGHT
    }

    pub fn set_is_double_height(&mut self, is_double_height: bool) {
        if is_double_height {
            self.attr |= attribute::DOUBLE_HEIGHT;
        } else {
            self.attr &= !attribute::DOUBLE_HEIGHT;
        }
    }

    pub fn is_crossed_out(self) -> bool {
        (self.attr & attribute::CROSSED_OUT) == attribute::CROSSED_OUT
    }

    pub fn set_is_crossed_out(&mut self, is_crossed_out: bool) {
        if is_crossed_out {
            self.attr |= attribute::CROSSED_OUT;
        } else {
            self.attr &= !attribute::CROSSED_OUT;
        }
    }

    pub fn is_underlined(self) -> bool {
        (self.attr & attribute::UNDERLINE) == attribute::UNDERLINE
    }

    pub fn set_is_underlined(&mut self, is_underline: bool) {
        if is_underline {
            self.attr |= attribute::UNDERLINE;
        } else {
            self.attr &= !attribute::UNDERLINE;
        }
    }

    pub fn is_double_underlined(self) -> bool {
        (self.attr & attribute::DOUBLE_UNDERLINE) == attribute::DOUBLE_UNDERLINE
    }

    pub fn set_is_double_underlined(&mut self, is_double_underline: bool) {
        if is_double_underline {
            self.attr |= attribute::DOUBLE_UNDERLINE;
        } else {
            self.attr &= !attribute::DOUBLE_UNDERLINE;
        }
    }

    pub fn is_concealed(self) -> bool {
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

    pub fn is_overlined(self) -> bool {
        (self.attr & attribute::OVERLINE) == attribute::OVERLINE
    }

    pub fn set_is_overlined(&mut self, arg: bool) {
        if arg {
            self.attr |= attribute::OVERLINE;
        } else {
            self.attr &= !attribute::OVERLINE;
        }
    }

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

    /// Returns true if the foreground color is stored as direct RGB
    pub fn is_foreground_rgb(self) -> bool {
        (self.ext_attr & extended_attribute::FG_RGBA) != 0
    }

    /// Returns true if the background color is stored as direct RGB
    pub fn is_background_rgb(self) -> bool {
        (self.ext_attr & extended_attribute::BG_RGBA) != 0
    }

    /// Set foreground color as palette index
    pub fn set_foreground_palette(&mut self, color: u32) {
        self.foreground_color = color;
        self.ext_attr &= !extended_attribute::FG_RGBA;
    }

    /// Set foreground color as direct RGB
    pub fn set_foreground_rgb(&mut self, r: u8, g: u8, b: u8) {
        // Pack RGB into u32: 0x00RRGGBB
        self.foreground_color = ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
        self.ext_attr |= extended_attribute::FG_RGBA;
    }

    /// Set background color as palette index
    pub fn set_background_palette(&mut self, color: u32) {
        self.background_color = color;
        self.ext_attr &= !extended_attribute::BG_RGBA;
    }

    /// Set background color as direct RGB
    pub fn set_background_rgb(&mut self, r: u8, g: u8, b: u8) {
        // Pack RGB into u32: 0x00RRGGBB
        self.background_color = ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
        self.ext_attr |= extended_attribute::BG_RGBA;
    }

    /// Get foreground RGB values (only valid if is_foreground_rgb() is true)
    pub fn foreground_rgb(self) -> (u8, u8, u8) {
        let r = (self.foreground_color >> 16) as u8;
        let g = (self.foreground_color >> 8) as u8;
        let b = self.foreground_color as u8;
        (r, g, b)
    }

    /// Get background RGB values (only valid if is_background_rgb() is true)
    pub fn background_rgb(self) -> (u8, u8, u8) {
        let r = (self.background_color >> 16) as u8;
        let g = (self.background_color >> 8) as u8;
        let b = self.background_color as u8;
        (r, g, b)
    }

    /// Returns true if the foreground color is an extended palette index (0-255)
    pub fn is_foreground_ext(self) -> bool {
        (self.ext_attr & extended_attribute::FG_EXT) != 0
    }

    /// Returns true if the background color is an extended palette index (0-255)
    pub fn is_background_ext(self) -> bool {
        (self.ext_attr & extended_attribute::BG_EXT) != 0
    }

    /// Set foreground color as extended palette index (0-255)
    pub fn set_foreground_ext(&mut self, index: u8) {
        self.foreground_color = index as u32;
        // Clear RGB flag, set EXT flag
        self.ext_attr &= !extended_attribute::FG_RGBA;
        self.ext_attr |= extended_attribute::FG_EXT;
    }

    /// Set background color as extended palette index (0-255)
    pub fn set_background_ext(&mut self, index: u8) {
        self.background_color = index as u32;
        // Clear RGB flag, set EXT flag
        self.ext_attr &= !extended_attribute::BG_RGBA;
        self.ext_attr |= extended_attribute::BG_EXT;
    }

    /// Get foreground extended palette index (only valid if is_foreground_ext() is true)
    pub fn foreground_ext(self) -> u8 {
        self.foreground_color as u8
    }

    /// Get background extended palette index (only valid if is_background_ext() is true)
    pub fn background_ext(self) -> u8 {
        self.background_color as u8
    }
}

impl PartialEq for TextAttribute {
    fn eq(&self, other: &TextAttribute) -> bool {
        self.foreground_color == other.foreground_color
            && self.background_color == other.background_color
            && self.attr == other.attr
            && self.ext_attr == other.ext_attr
    }
}
