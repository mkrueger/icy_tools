use super::TextAttribute;

#[derive(Clone, Copy, Debug)]
pub struct AttributedChar {
    pub ch: char,
    pub attribute: TextAttribute,
}

impl Default for AttributedChar {
    fn default() -> Self {
        AttributedChar {
            ch: ' ',
            attribute: super::TextAttribute::default(),
        }
    }
}

impl AttributedChar {
    pub fn invisible() -> Self {
        AttributedChar {
            ch: ' ',
            attribute: super::TextAttribute {
                attr: crate::attribute::INVISIBLE,
                foreground_color: TextAttribute::TRANSPARENT_COLOR,
                background_color: TextAttribute::TRANSPARENT_COLOR,
                ..Default::default()
            },
        }
    }
    pub fn is_visible(&self) -> bool {
        (self.attribute.attr & crate::attribute::INVISIBLE) == 0
    }

    #[must_use]
    pub fn new(ch: char, attribute: TextAttribute) -> Self {
        AttributedChar { ch, attribute }
    }

    pub fn is_transparent(self) -> bool {
        (self.ch == '\0' || self.ch == ' ') || self.attribute.get_background() == self.attribute.get_foreground()
    }

    pub fn get_font_page(&self) -> usize {
        self.attribute.get_font_page()
    }

    pub fn set_font_page(&mut self, page: usize) {
        self.attribute.set_font_page(page);
    }

    pub(crate) fn with_font_page(&self, font_page: usize) -> AttributedChar {
        AttributedChar {
            ch: self.ch,
            attribute: self.attribute.with_font_page(font_page),
        }
    }
}

impl PartialEq for AttributedChar {
    fn eq(&self, other: &AttributedChar) -> bool {
        self.ch == other.ch && self.attribute == other.attribute
    }
}

impl std::fmt::Display for AttributedChar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "(Char: {}/0x{0:X} '{}', Attr: {}, Font: {})",
            self.ch as u32,
            self.ch,
            self.attribute,
            self.get_font_page()
        )
    }
}

impl From<char> for AttributedChar {
    fn from(value: char) -> Self {
        AttributedChar {
            ch: value,
            attribute: TextAttribute::default(),
        }
    }
}

/*
pub fn get_color(color: u8) -> &'static str
{
    match color {
        0 => "Black",
        1 => "Blue",
        2 => "Green",
        3 => "Aqua",
        4 => "Red",
        5 => "Purple",
        6 => "Brown",
        7 => "Light Gray",
        8 => "Gray",
        9 => "Light Blue",
        10 => "Light Green",
        11 => "Light Aqua",
        12 => "Light Red",
        13 => "Light Purple",
        14 => "Light Yelllow",
        15 => "White",
        _ => "Unknown"
    }
}
*/
