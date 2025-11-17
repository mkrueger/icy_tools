/// Cursor mode for Cursor command
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorMode {
    /// Cursor off
    Off = 0,
    /// Cursor on
    On = 1,
    /// Destructive backspace
    DestructiveBackspace = 2,
    /// Non-destructive backspace
    NonDestructiveBackspace = 3,
}

impl From<i32> for CursorMode {
    fn from(value: i32) -> Self {
        match value {
            0 => Self::Off,
            1 => Self::On,
            2 => Self::DestructiveBackspace,
            3 => Self::NonDestructiveBackspace,
            _ => Self::Off,
        }
    }
}
