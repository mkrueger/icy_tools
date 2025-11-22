mod fill_style;
pub use fill_style::FillStyle;

mod line_style;
pub use line_style::LineStyle;

/// File query mode for RIP_FILE_QUERY command.
///
/// Determines the format of the response returned to the host when querying
/// file existence and metadata.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileQueryMode {
    /// Simply query existence: returns "1" if exists, "0" otherwise (no CR).
    FileExists = 0,
    /// Same as FileExists, but adds a carriage return after the response.
    FileExistsWithCR = 1,
    /// Query with file size: returns "0\r\n" if missing, or "1.{size}\r\n" if present.
    /// Example: "1.20345\r\n"
    QueryWithSize = 2,
    /// Extended info with date/time: returns "0\r\n" or "1.{size}.{date}.{time}\r\n".
    /// Example: "1.20345.01/02/93.03:04:30\r\n"
    QueryExtended = 3,
    /// Extended info including filename: "0\r\n" or "1.{filename}.{size}.{date}.{time}\r\n".
    /// Example: "1.MYFILE.RIP.20345.01/02/93.03:04:30\r\n"
    QueryWithFilename = 4,
}

impl TryFrom<u16> for FileQueryMode {
    type Error = String;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::FileExists),
            1 => Ok(Self::FileExistsWithCR),
            2 => Ok(Self::QueryWithSize),
            3 => Ok(Self::QueryExtended),
            4 => Ok(Self::QueryWithFilename),
            _ => Err(format!("Invalid FileQueryMode value: {}", value)),
        }
    }
}

/// Write mode for RIP drawing operations.
///
/// Determines how drawing operations interact with existing screen content.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteMode {
    /// Normal drawing mode (overwrite existing content).
    Normal = 0,
    /// XOR (complimentary) mode - allows rubber banding and temporary drawings.
    Xor = 1,
}

impl TryFrom<u16> for WriteMode {
    type Error = String;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Normal),
            1 => Ok(Self::Xor),
            _ => Err(format!("Invalid WriteMode value: {}", value)),
        }
    }
}

/// Image paste mode for RIP_PUT_IMAGE and RIP_LOAD_ICON commands.
///
/// Determines how pasted images interact with existing screen content using
/// logical operations.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImagePasteMode {
    /// Paste the image on-screen normally (COPY).
    Copy = 0,
    /// Exclusive-OR image with the one already on screen (XOR).
    Xor = 1,
    /// Logically OR image with the one already on screen (OR).
    Or = 2,
    /// Logically AND image with the one already on screen (AND).
    And = 3,
    /// Paste the inverse of the image on the screen (NOT).
    Not = 4,
}

impl TryFrom<u16> for ImagePasteMode {
    type Error = String;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Copy),
            1 => Ok(Self::Xor),
            2 => Ok(Self::Or),
            3 => Ok(Self::And),
            4 => Ok(Self::Not),
            _ => Err(format!("Invalid ImagePasteMode value: {}", value)),
        }
    }
}

/// Query processing mode for RIP_QUERY command.
///
/// Determines when and how query commands are processed.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryMode {
    /// Process the query command NOW (upon receipt).
    ProcessNow = 0,
    /// Process when mouse clicked in Graphics Window.
    OnClickGraphics = 1,
    /// Process when mouse clicked in Text Window.
    /// Mouse coordinates return TEXT coordinates (2 digits), not graphics (4 digits).
    OnClickText = 2,
}

impl TryFrom<u16> for QueryMode {
    type Error = String;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::ProcessNow),
            1 => Ok(Self::OnClickGraphics),
            2 => Ok(Self::OnClickText),
            _ => Err(format!("Invalid QueryMode value: {}", value)),
        }
    }
}

/// Block transfer protocol for RIP_ENTER_BLOCK_MODE command.
///
/// Specifies which file transfer protocol to use for block/file transfers.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockTransferMode {
    /// Xmodem (checksum) - requires filename.
    XmodemChecksum = 0,
    /// Xmodem (CRC) - requires filename.
    XmodemCrc = 1,
    /// Xmodem-1K - requires filename.
    Xmodem1K = 2,
    /// Xmodem-1K (G) - requires filename.
    Xmodem1KG = 3,
    /// Kermit - requires filename.
    Kermit = 4,
    /// Ymodem (batch) - filename not required.
    Ymodem = 5,
    /// Ymodem-G - filename not required.
    YmodemG = 6,
    /// Zmodem (crash recovery) - filename not required.
    Zmodem = 7,
}

impl BlockTransferMode {
    /// Convert to i32 value.
    pub fn to_i32(self) -> u16 {
        self as u16
    }
}

impl TryFrom<u16> for BlockTransferMode {
    type Error = String;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::XmodemChecksum),
            1 => Ok(Self::XmodemCrc),
            2 => Ok(Self::Xmodem1K),
            3 => Ok(Self::Xmodem1KG),
            4 => Ok(Self::Kermit),
            5 => Ok(Self::Ymodem),
            6 => Ok(Self::YmodemG),
            7 => Ok(Self::Zmodem),
            _ => Err(format!("Invalid BlockTransferMode value: {}", value)),
        }
    }
}
