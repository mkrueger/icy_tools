mod ansi;
use std::error::Error;

pub use ansi::*;

mod bitfont;
pub use bitfont::*;

mod charfont;
pub use charfont::*;

mod animation;
pub use animation::*;

#[derive(Debug, Clone)]
pub enum SavingError {
    ErrorWritingFile(String),
    ErrorCreatingFile(String),
}

impl std::fmt::Display for SavingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SavingError::ErrorWritingFile(error) => {
                write!(f, "Error writing file: {}", error)
            }
            SavingError::ErrorCreatingFile(error) => {
                write!(f, "Error creating file: {}", error)
            }
        }
    }
}
impl Error for SavingError {
    fn description(&self) -> &str {
        "use std::display"
    }

    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }

    fn cause(&self) -> Option<&dyn Error> {
        self.source()
    }
}
