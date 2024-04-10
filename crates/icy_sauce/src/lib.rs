use bstr::BString;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, SauceError>;

pub mod char_caps;
pub mod header;
pub mod info;
pub use info::*;

pub mod builder;
pub use builder::*;

#[derive(Error, Debug)]
pub enum SauceError {
    #[error("Unsupported SAUCE version: {0}")]
    UnsupportedSauceVersion(BString),

    #[error("Invalid comment block")]
    InvalidCommentBlock,

    #[error("Invalid comment ID: {0}")]
    InvalidCommentId(BString),

    #[error("Unsupported SAUCE date: {0}")]
    UnsupportedSauceDate(BString),

    #[error("Binary file width limit exceeded: {0}")]
    BinFileWidthLimitExceeded(i32),

    #[error("Wrong data type for operation: {0:?}")]
    WrongDataType(SauceDataType),

    #[error("IO error: {0}")]
    IoError(std::io::Error),

    #[error("Comment limit exceeded (255)")]
    CommentLimitExceeded,

    #[error("Comment too long: {0} bytes only up to 64 bytes are allowed.")]
    CommentTooLong(usize),

    #[error("Title too long: {0} bytes only up to 35 bytes are allowed.")]
    TitleTooLong(usize),

    #[error("Author too long: {0} bytes only up to 20 bytes are allowed.")]
    AuthorTooLong(usize),

    #[error("Group too long: {0} bytes only up to 20 bytes are allowed.")]
    GroupTooLong(usize),
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum SauceDataType {
    /// Undefined filetype.
    /// You could use this to add SAUCE to a custom or proprietary file, without giving it any particular meaning or interpretation.
    Undefined(u8),

    /// A character based file.
    /// These files are typically interpreted sequentially. Also known as streams.  
    #[default]
    Character = 1,

    /// Bitmap graphic and animation files.
    Bitmap = 2,

    /// A vector graphic file.
    Vector = 3,

    /// An audio file.
    Audio = 4,

    /// This is a raw memory copy of a text mode screen. Also known as a .BIN file.
    /// This is essentially a collection of character and attribute pairs.
    BinaryText = 5,

    /// An XBin or eXtended BIN file.
    XBin = 6,

    /// An archive file.
    Archive = 7,

    ///  A executable file.
    Executable = 8,
}

impl From<u8> for SauceDataType {
    fn from(byte: u8) -> SauceDataType {
        match byte {
            1 => SauceDataType::Character,
            2 => SauceDataType::Bitmap,
            3 => SauceDataType::Vector,
            4 => SauceDataType::Audio,
            5 => SauceDataType::BinaryText,
            6 => SauceDataType::XBin,
            7 => SauceDataType::Archive,
            8 => SauceDataType::Executable,
            undefined => SauceDataType::Undefined(undefined),
        }
    }
}

impl From<SauceDataType> for u8 {
    fn from(data_type: SauceDataType) -> u8 {
        match data_type {
            SauceDataType::Character => 1,
            SauceDataType::Bitmap => 2,
            SauceDataType::Vector => 3,
            SauceDataType::Audio => 4,
            SauceDataType::BinaryText => 5,
            SauceDataType::XBin => 6,
            SauceDataType::Archive => 7,
            SauceDataType::Executable => 8,
            SauceDataType::Undefined(byte) => byte,
        }
    }
}

/// Trims the trailing whitespace and null bytes from the data.
/// This is sauce specific - no other thing than space should be trimmed, however some implementations use null bytes instead of spaces.
pub(crate) fn sauce_trim(data: &[u8]) -> BString {
    let end = sauce_len_rev(data);
    BString::new(data[..end].to_vec())
}

fn sauce_len_rev(data: &[u8]) -> usize {
    let mut end = data.len();
    while end > 0 {
        let b = data[end - 1];
        if b != 0 && b != b' ' {
            break;
        }
        end -= 1;
    }
    end
}

/// Pads trailing whitespaces or cut too long data.
pub(crate) fn sauce_pad(str: &BString, len: usize) -> Vec<u8> {
    let mut data = str.to_vec();
    data.resize(len, b' ');
    data
}

#[cfg(test)]
mod tests {
    use crate::{sauce_pad, sauce_trim};
    use bstr::BString;

    #[test]
    fn test_sauce_trim() {
        let data = b"Hello World  ";
        assert_eq!(sauce_trim(data), BString::from("Hello World"));
        let data = b"Hello World\0\0";
        assert_eq!(sauce_trim(data), BString::from("Hello World"));

        let data = b"Hello World\t\0";
        assert_eq!(sauce_trim(data), BString::from("Hello World\t"));
        let data = b"Hello World\n ";
        assert_eq!(sauce_trim(data), BString::from("Hello World\n"));
        let data = b"    \0   ";
        assert_eq!(sauce_trim(data), BString::from(""));
    }

    #[test]
    fn test_sauce_pad() {
        let data = BString::from(b"Hello World");
        assert_eq!(sauce_pad(&data, 15), b"Hello World    ");

        let data = BString::from(b"Hello World");
        assert_eq!(sauce_pad(&data, 5), b"Hello");

        let data = BString::from(b"");
        assert_eq!(sauce_pad(&data, 1), b" ");
    }
}
