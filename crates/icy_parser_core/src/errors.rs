//! Parser error types and error level definitions

use std::fmt::Display;

/// Error severity level for diagnostic reporting
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ErrorLevel {
    /// Informational message (e.g., unrecognized but harmless sequences)
    Info = 0,
    /// Warning about potentially problematic input (parsing continues)
    Warning = 1,
    /// Error in parsing that may cause incorrect behavior
    Error = 2,
}

impl ErrorLevel {
    /// Returns a human-readable string for the error level
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

impl Display for ErrorLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Parser error types with context information
#[repr(u8)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// Invalid parameter value for a command
    InvalidParameter {
        command: &'static str,
        value: u16,
        /// Expected range or valid values (optional)
        expected: Option<&'static str>,
    },
    /// Incomplete sequence (parser state at end of input)
    IncompleteSequence {
        /// Description of what was expected
        context: &'static str,
    },
    /// Malformed escape sequence
    MalformedSequence {
        description: &'static str,
        /// The problematic byte or sequence (for debugging)
        sequence: Option<String>,
    },
    /// Unsupported feature or command (not an error, but worth reporting)
    UnsupportedFeature { description: &'static str },
    /// Out of range value that was clamped or ignored
    OutOfRange { parameter: &'static str, value: i32, min: i32, max: i32 },
}

impl ParseError {
    /// Returns the suggested error level for this error type
    pub fn level(&self) -> ErrorLevel {
        match self {
            Self::InvalidParameter { .. } => ErrorLevel::Error,
            Self::IncompleteSequence { .. } => ErrorLevel::Warning,
            Self::MalformedSequence { .. } => ErrorLevel::Error,
            Self::UnsupportedFeature { .. } => ErrorLevel::Info,
            Self::OutOfRange { .. } => ErrorLevel::Warning,
        }
    }

    /// Returns a human-readable description of the error
    pub fn description(&self) -> String {
        match self {
            Self::InvalidParameter { command, value, expected } => {
                if let Some(exp) = expected {
                    format!("Invalid parameter value {} for command '{}' (expected: {})", value, command, exp)
                } else {
                    format!("Invalid parameter value {} for command '{}'", value, command)
                }
            }
            Self::IncompleteSequence { context } => {
                format!("Incomplete sequence: {}", context)
            }
            Self::MalformedSequence { description, sequence } => {
                if let Some(seq) = sequence {
                    format!("Malformed sequence: {} (sequence: {})", description, seq)
                } else {
                    format!("Malformed sequence: {}", description)
                }
            }
            Self::UnsupportedFeature { description } => {
                format!("Unsupported feature: {}", description)
            }
            Self::OutOfRange { parameter, value, min, max } => {
                format!("Parameter '{}' value {} out of range [{}, {}]", parameter, value, min, max)
            }
        }
    }
}

impl Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.level(), self.description())
    }
}
