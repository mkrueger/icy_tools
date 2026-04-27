/// Cursor mode for Cursor command
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorMode {
    /// Cursor off
    #[default]
    Off = 0,
    /// Cursor on
    On = 1,
    /// Destructive backspace
    DestructiveBackspace = 2,
    /// Non-destructive backspace
    NonDestructiveBackspace = 3,
}

impl TryFrom<i32> for CursorMode {
    type Error = String;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Off),
            1 => Ok(Self::On),
            2 => Ok(Self::DestructiveBackspace),
            3 => Ok(Self::NonDestructiveBackspace),
            _ => Err(format!("Invalid CursorMode value: {}", value)),
        }
    }
}
