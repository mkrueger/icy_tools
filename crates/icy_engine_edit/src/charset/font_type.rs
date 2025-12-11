//! Font types supported by TheDraw fonts

/// Font types supported by TheDraw fonts
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontType {
    /// Outline font - rendered with the current outline style
    Outline,
    /// Block font - solid block characters
    Block,
    /// Color font - full color characters with attributes
    #[default]
    Color,
}

impl std::fmt::Display for FontType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FontType::Outline => write!(f, "Outline"),
            FontType::Block => write!(f, "Block"),
            FontType::Color => write!(f, "Color"),
        }
    }
}

impl From<retrofont::tdf::TdfFontType> for FontType {
    fn from(t: retrofont::tdf::TdfFontType) -> Self {
        use retrofont::tdf::TdfFontType;
        match t {
            TdfFontType::Outline => FontType::Outline,
            TdfFontType::Block => FontType::Block,
            TdfFontType::Color => FontType::Color,
        }
    }
}

impl From<FontType> for retrofont::tdf::TdfFontType {
    fn from(t: FontType) -> Self {
        use retrofont::tdf::TdfFontType;
        match t {
            FontType::Outline => TdfFontType::Outline,
            FontType::Block => TdfFontType::Block,
            FontType::Color => TdfFontType::Color,
        }
    }
}
