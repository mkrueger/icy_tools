use bstr::BString;

use crate::{header::SauceHeader, SauceDataType};

/// | Field    | Type | Size | Descritption
/// |----------|------|------|-------------
/// | ID       | char | 5    | SAUCE comment block ID. This should be equal to "COMNT".
/// | Line 1   | char | 64   | Line of text.
/// | ...      |      |      |
/// | Line n   | char | 64   | Last line of text
const ANSI_FLAG_NON_BLINK_MODE: u8 = 0b0000_0001;
const ANSI_MASK_LETTER_SPACING: u8 = 0b0000_0110;
const ANSI_LETTER_SPACING_LEGACY: u8 = 0b0000_0000;
const ANSI_LETTER_SPACING_8PX: u8 = 0b0000_0010;
const ANSI_LETTER_SPACING_9PX: u8 = 0b0000_0100;

const ANSI_MASK_ASPECT_RATIO: u8 = 0b0001_1000;
const ANSI_ASPECT_RATIO_LEGACY: u8 = 0b0000_0000;
const ANSI_ASPECT_RATIO_STRETCH: u8 = 0b0000_1000;
const ANSI_ASPECT_RATIO_SQUARE: u8 = 0b0001_0000;

mod sauce_file_type {
    pub const ASCII: u8 = 0;
    pub const ANSI: u8 = 1;
    pub const ANSIMATION: u8 = 2;
    pub const PCBOARD: u8 = 4;
    pub const AVATAR: u8 = 5;
    pub const TUNDRA_DRAW: u8 = 8;
}

pub struct CharCaps {
    pub content_type: ContentType,
    pub width: u16,
    pub height: u16,
    pub use_ice: bool,
    pub use_letter_spacing: bool,
    pub use_aspect_ratio: bool,
    pub font_opt: Option<BString>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ContentType {
    Unknown(u8),
    Ascii,
    Ansi,
    AnsiMation,
    RipScript,
    PCBoard,
    Avatar,
    Html,
    Source,
    TundraDraw,
}

impl From<u8> for ContentType {
    fn from(byte: u8) -> ContentType {
        match byte {
            0 => ContentType::Ascii,
            1 => ContentType::Ansi,
            2 => ContentType::AnsiMation,
            3 => ContentType::RipScript,
            4 => ContentType::PCBoard,
            5 => ContentType::Avatar,
            6 => ContentType::Html,
            7 => ContentType::Source,
            8 => ContentType::TundraDraw,
            unknown => ContentType::Unknown(unknown),
        }
    }
}

impl From<ContentType> for u8 {
    fn from(content_type: ContentType) -> u8 {
        match content_type {
            ContentType::Ascii => 0,
            ContentType::Ansi => 1,
            ContentType::AnsiMation => 2,
            ContentType::RipScript => 3,
            ContentType::PCBoard => 4,
            ContentType::Avatar => 5,
            ContentType::Html => 6,
            ContentType::Source => 7,
            ContentType::TundraDraw => 8,
            ContentType::Unknown(byte) => byte,
        }
    }
}

impl CharCaps {
    pub(crate) fn from(header: &SauceHeader) -> crate::Result<Self> {
        let mut width = 80;
        let mut height = 25;
        let mut use_ice = false;
        let mut use_letter_spacing = false;
        let mut use_aspect_ratio = false;
        let mut font_opt = None;

        match header.data_type {
            SauceDataType::BinaryText => {
                width = (header.file_type as u16) << 1;
                use_ice = (header.t_flags & ANSI_FLAG_NON_BLINK_MODE) == ANSI_FLAG_NON_BLINK_MODE;
                font_opt = Some(header.t_info_s.clone());
            }
            SauceDataType::XBin => {
                width = header.t_info1;
                height = header.t_info2;
                // no flags according to spec
            }

            SauceDataType::Character => {
                match header.file_type {
                    sauce_file_type::ASCII | sauce_file_type::ANSIMATION | sauce_file_type::ANSI => {
                        width = header.t_info1;
                        height = header.t_info2;
                        use_ice = (header.t_flags & ANSI_FLAG_NON_BLINK_MODE) == ANSI_FLAG_NON_BLINK_MODE;

                        match header.t_flags & ANSI_MASK_LETTER_SPACING {
                            ANSI_LETTER_SPACING_LEGACY | ANSI_LETTER_SPACING_8PX => {
                                use_letter_spacing = false;
                            }
                            ANSI_LETTER_SPACING_9PX => use_letter_spacing = true,
                            _ => {}
                        }
                        match header.t_flags & ANSI_MASK_ASPECT_RATIO {
                            ANSI_ASPECT_RATIO_SQUARE | ANSI_ASPECT_RATIO_LEGACY => {
                                use_aspect_ratio = false;
                            }
                            ANSI_ASPECT_RATIO_STRETCH => use_aspect_ratio = true,
                            _ => {}
                        }
                        font_opt = Some(header.t_info_s.clone());
                    }

                    sauce_file_type::PCBOARD | sauce_file_type::AVATAR | sauce_file_type::TUNDRA_DRAW => {
                        width = header.t_info1;
                        height = header.t_info2;
                        // no flags according to spec
                    }
                    _ => {
                        // ignore other char types.
                    }
                }
            }
            _ => {
                unreachable!("This should never happen")
            }
        }

        Ok(CharCaps {
            content_type: ContentType::from(header.file_type),
            width,
            height,
            use_ice,
            use_letter_spacing,
            use_aspect_ratio,
            font_opt,
        })
    }

    pub(crate) fn write_to_header(&self, header: &mut SauceHeader) {
        match header.data_type {
            SauceDataType::BinaryText => {
                header.file_type = sauce_file_type::ASCII;
                header.t_flags = if self.use_ice { ANSI_FLAG_NON_BLINK_MODE } else { 0 };
                if let Some(font) = &self.font_opt {
                    header.t_info_s.clone_from(font);
                } else {
                    header.t_info_s.clear();
                }
            }
            SauceDataType::XBin => {
                header.t_info1 = self.width;
                header.t_info2 = self.height;
            }
            SauceDataType::Character => {
                header.file_type = u8::from(self.content_type);
                match self.content_type {
                    ContentType::Ascii | ContentType::AnsiMation | ContentType::Ansi => {
                        header.t_info1 = self.width;
                        header.t_info2 = self.height;

                        header.t_flags = if self.use_ice { ANSI_FLAG_NON_BLINK_MODE } else { 0 };
                        header.t_flags |= if self.use_letter_spacing {
                            ANSI_LETTER_SPACING_9PX
                        } else {
                            ANSI_LETTER_SPACING_LEGACY
                        };
                        header.t_flags |= if self.use_aspect_ratio {
                            ANSI_ASPECT_RATIO_STRETCH
                        } else {
                            ANSI_ASPECT_RATIO_LEGACY
                        };

                        if let Some(font) = &self.font_opt {
                            header.t_info_s.clone_from(font);
                        } else {
                            header.t_info_s.clear();
                        }
                    }

                    ContentType::PCBoard | ContentType::Avatar | ContentType::TundraDraw => {
                        header.t_info1 = self.width;
                        header.t_info2 = self.height;
                        // no flags according to spec
                    }
                    _ => {
                        // ignore other char types.
                    }
                }
            }
            _ => {
                unreachable!("This should never happen")
            }
        }
    }
}
