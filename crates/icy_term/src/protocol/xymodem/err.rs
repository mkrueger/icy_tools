use std::error::Error;

#[derive(Debug, Clone, Copy)]
pub enum TransmissionError {
    Cancel,
    InvalidMode(u8),
    TooManyRetriesSendingHeader,
    XModem1File,
}

impl std::fmt::Display for TransmissionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransmissionError::Cancel => write!(f, "transmission canceled"),
            TransmissionError::InvalidMode(m) => write!(f, "invalid x/y modem mode: {m}"),
            TransmissionError::TooManyRetriesSendingHeader => {
                write!(f, "too many retries sending ymodem header")
            }
            TransmissionError::XModem1File => write!(f, "Only 1 file can be send with x-modem"),
        }
    }
}

impl Error for TransmissionError {
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
