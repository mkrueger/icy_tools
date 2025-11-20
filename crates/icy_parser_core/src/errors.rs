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
        value: String,
        /// Expected range or valid values (optional)
        expected: Option<String>,
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
                    format!("Invalid parameter value '{}' for command '{}' (expected: {})", value, command, exp)
                } else {
                    format!("Invalid parameter value '{}' for command '{}'", value, command)
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

/// Format a byte value for human-readable error messages.
///
/// Returns a string with both hex and human-readable representation:
/// - Printable ASCII (0x20-0x7E): "0x41 ('A')"
/// - Control characters with names: "0x0A (LF)", "0x09 (TAB)", etc.
/// - Other bytes: "0x00"
pub fn print_char_value(byte: u8) -> String {
    match byte {
        0x00 => "0x00 (NUL)".to_string(),
        0x01 => "0x01 (SOH)".to_string(),
        0x02 => "0x02 (STX)".to_string(),
        0x03 => "0x03 (ETX)".to_string(),
        0x04 => "0x04 (EOT)".to_string(),
        0x05 => "0x05 (ENQ)".to_string(),
        0x06 => "0x06 (ACK)".to_string(),
        0x07 => "0x07 (BEL)".to_string(),
        0x08 => "0x08 (BS)".to_string(),
        0x09 => "0x09 (TAB)".to_string(),
        0x0A => "0x0A (LF)".to_string(),
        0x0B => "0x0B (VT)".to_string(),
        0x0C => "0x0C (FF)".to_string(),
        0x0D => "0x0D (CR)".to_string(),
        0x0E => "0x0E (SO)".to_string(),
        0x0F => "0x0F (SI)".to_string(),
        0x10 => "0x10 (DLE)".to_string(),
        0x11 => "0x11 (DC1)".to_string(),
        0x12 => "0x12 (DC2)".to_string(),
        0x13 => "0x13 (DC3)".to_string(),
        0x14 => "0x14 (DC4)".to_string(),
        0x15 => "0x15 (NAK)".to_string(),
        0x16 => "0x16 (SYN)".to_string(),
        0x17 => "0x17 (ETB)".to_string(),
        0x18 => "0x18 (CAN)".to_string(),
        0x19 => "0x19 (EM)".to_string(),
        0x1A => "0x1A (SUB)".to_string(),
        0x1B => "0x1B (ESC)".to_string(),
        0x1C => "0x1C (FS)".to_string(),
        0x1D => "0x1D (GS)".to_string(),
        0x1E => "0x1E (RS)".to_string(),
        0x1F => "0x1F (US)".to_string(),
        0x20..=0x7E => format!("0x{:02X} ('{}')", byte, byte as char),
        0x7F => "0x7F (DEL)".to_string(),
        _ => format!("0x{:02X}", byte),
    }
}
