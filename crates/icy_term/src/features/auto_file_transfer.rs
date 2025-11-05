use icy_net::protocol::TransferProtocolType;

use crate::util::PatternRecognizer;

pub struct AutoFileTransfer {
    zmodem_dl: PatternRecognizer,
    zmodem_ul: PatternRecognizer,
}

impl AutoFileTransfer {
    pub fn try_transfer(&mut self, ch: u8) -> Option<(TransferProtocolType, bool)> {
        if self.zmodem_dl.push_ch(ch) {
            return Some((TransferProtocolType::ZModem, true));
        }
        if self.zmodem_ul.push_ch(ch) {
            return Some((TransferProtocolType::ZModem, false));
        }
        None
    }
}

impl Default for AutoFileTransfer {
    fn default() -> Self {
        Self {
            zmodem_dl: PatternRecognizer::from(b"\x18B00", true), // ZRQINIT
            zmodem_ul: PatternRecognizer::from(b"\x18B01", true), // ZRINIT
        }
    }
}
