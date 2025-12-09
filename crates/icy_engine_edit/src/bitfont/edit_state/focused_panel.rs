//! BitFont focused panel enumeration

/// Which panel currently has focus in the BitFont editor
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BitFontFocusedPanel {
    /// Edit grid has focus - operations work on pixel selection
    #[default]
    EditGrid,
    /// Character set has focus - operations work on selected characters
    CharSet,
}
