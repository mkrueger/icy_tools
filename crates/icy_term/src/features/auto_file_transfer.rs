use crate::{protocol::TransferType, util::PatternRecognizer};

pub struct AutoFileTransfer {
    zmodem_dl: PatternRecognizer,
    zmodem_ul: PatternRecognizer,
}

impl AutoFileTransfer {
    pub fn reset(&mut self) {
        self.zmodem_dl.reset();
        self.zmodem_ul.reset();
    }

    pub fn try_transfer(&mut self, ch: u8) -> Option<(TransferType, bool)> {
        if self.zmodem_dl.push_ch(ch) {
            return Some((TransferType::ZModem, true));
        }
        if self.zmodem_ul.push_ch(ch) {
            return Some((TransferType::ZModem, false));
        }
        None
    }
}

impl Default for AutoFileTransfer {
    fn default() -> Self {
        Self {
            zmodem_dl: PatternRecognizer::from(b"\x18B00000000000000", true),
            zmodem_ul: PatternRecognizer::from(b"\x18B0100000023be50", true),
        }
    }
}
