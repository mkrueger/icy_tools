use std::error::Error;

use crate::ansi::fmt_error_string;

#[derive(Debug, Clone)]
pub enum ParserError {
    InvalidChar(char),
    InvalidBuffer,
    UnsupportedEscapeSequence(String),
    UnsupportedDCSSequence(String),
    UnsupportedOSCSequence(String),
    UnsupportedCustomCommand(i32),
    Description(&'static str),
    UnsupportedControlCode(u32),
    UnsupportedFont(usize),
    UnsupportedSauceFont(String),
    UnexpectedSixelEnd(char),
    InvalidColorInSixelSequence,
    NumberMissingInSixelRepeat,
    InvalidSixelChar(char),
    UnsupportedSixelColorformat(i32),
    ErrorInSixelEngine(&'static str),
    InvalidPictureSize,

    InvalidRipAnsiQuery(i32),

    Error(String),
}

impl std::fmt::Display for ParserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParserError::InvalidChar(ch) => write!(f, "invalid character {ch}"),
            ParserError::UnsupportedEscapeSequence(seq) => {
                write!(f, "unsupported escape sequence {}", fmt_error_string(&seq))
            }
            ParserError::UnsupportedDCSSequence(seq) => {
                write!(f, "unsupported DCS sequence {}", fmt_error_string(&seq))
            }
            ParserError::UnsupportedOSCSequence(seq) => {
                write!(f, "unsupported OSC sequence {}", fmt_error_string(&seq))
            }
            ParserError::Description(str) => write!(f, "{str}"),
            ParserError::UnsupportedControlCode(code) => {
                write!(f, "unsupported control code {}", *code)
            }
            ParserError::UnsupportedCustomCommand(code) => {
                write!(f, "unsupported custom ansi command: {}", *code)
            }
            ParserError::UnsupportedFont(code) => write!(f, "font {} not supported", *code),
            ParserError::UnsupportedSauceFont(name) => write!(f, "font {name} not supported"),
            ParserError::UnexpectedSixelEnd(ch) => {
                write!(f, "sixel sequence ended with <esc>{ch} expected '\\'")
            }
            ParserError::InvalidBuffer => write!(f, "output buffer is invalid"),
            ParserError::InvalidColorInSixelSequence => {
                write!(f, "invalid color in sixel sequence")
            }
            ParserError::NumberMissingInSixelRepeat => {
                write!(f, "sixel repeat sequence is missing number")
            }
            ParserError::InvalidSixelChar(ch) => write!(f, "{ch} invalid in sixel data"),
            ParserError::UnsupportedSixelColorformat(i) => {
                write!(f, "{i} invalid color format in sixel data")
            }
            ParserError::ErrorInSixelEngine(err) => write!(f, "sixel engine error: {err}"),
            ParserError::InvalidPictureSize => write!(f, "invalid sixel picture size description"),
            ParserError::InvalidRipAnsiQuery(i) => write!(f, "invalid rip ansi query <esc>[{i}!"),
            ParserError::Error(err) => write!(f, "Parse error: {err}"),
        }
    }
}

impl Error for ParserError {
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
