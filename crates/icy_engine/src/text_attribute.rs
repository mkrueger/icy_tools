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

#[derive(Clone, Copy)]
pub struct TextAttribute {
    pub(super) font_page: usize,
    pub(super) foreground_color: u32,
    pub(super) background_color: u32,
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
            self.get_foreground(),
            self.get_background(),
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

    pub fn get_foreground(self) -> u32 {
        self.foreground_color
    }

    pub fn set_foreground(&mut self, color: u32) {
        self.foreground_color = color;
    }

    pub fn get_background(self) -> u32 {
        self.background_color
    }

    pub fn set_background(&mut self, color: u32) {
        self.background_color = color;
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
    pub fn get_font_page(&self) -> usize {
        self.font_page
    }

    pub fn set_font_page(&mut self, page: usize) {
        self.font_page = page;
    }

    pub fn with_font_page(&self, font_page: usize) -> TextAttribute {
        TextAttribute { font_page, ..*self }
    }
}

impl PartialEq for TextAttribute {
    fn eq(&self, other: &TextAttribute) -> bool {
        self.foreground_color == other.foreground_color && self.background_color == other.background_color && self.attr == other.attr
    }
}
