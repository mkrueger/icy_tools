/// Fill pattern type for AttributeForFills command
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternType {
    /// Hollow (no fill)
    Hollow,
    /// Solid color fill
    Solid,
    /// Pattern fill (uses pattern index 1-24)
    Pattern(u8),
    /// Hatch fill (uses pattern index 1-12)
    Hatch(u8),
    /// User defined fill (uses pattern index 0-9)
    UserDefined(u8),
}
