use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum FigError {
    #[error("Invalid FIG header")]
    InvalidHeader,

    #[error("Invalid FIG header hard blank")]
    InvalidHeaderHardBlank,

    #[error("Invalid FIG header print direction ({0})")]
    InvalidHeaderPrintDirection(usize),

    #[error("Invalid character tag ({0})")]
    InvalidCharTag(String),

    #[error("Invalid character line without EOL")]
    InvalidCharLine,

    #[error("Invalid FIGLET ZIP archive")]
    InvalidZIP,
}
