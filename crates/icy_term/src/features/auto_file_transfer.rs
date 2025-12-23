use crate::util::PatternRecognizer;
use crate::TransferProtocol;

/// A scanner for a single protocol's signature
struct SignatureScanner {
    protocol_id: String,
    is_download: bool,
    recognizer: PatternRecognizer,
}

/// Scans incoming data for auto-transfer signatures.
/// Built from the protocol list at session start.
pub struct AutoTransferScanner {
    scanners: Vec<SignatureScanner>,
}

impl AutoTransferScanner {
    /// Creates a new scanner from a list of transfer protocols.
    /// Only includes protocols that have auto_transfer enabled and non-empty signatures.
    pub fn from_protocols(protocols: &[TransferProtocol]) -> Self {
        let mut scanners = Vec::new();

        for protocol in protocols {
            if !protocol.enabled || !protocol.auto_transfer {
                continue;
            }

            // Add download signature scanner if non-empty
            let dl_bytes = protocol.download_signature.to_bytes();
            if !dl_bytes.is_empty() {
                scanners.push(SignatureScanner {
                    protocol_id: protocol.id.clone(),
                    is_download: true,
                    recognizer: PatternRecognizer::from(&dl_bytes, false),
                });
            }

            // Add upload signature scanner if non-empty
            let ul_bytes = protocol.upload_signature.to_bytes();
            if !ul_bytes.is_empty() {
                scanners.push(SignatureScanner {
                    protocol_id: protocol.id.clone(),
                    is_download: false,
                    recognizer: PatternRecognizer::from(&ul_bytes, false),
                });
            }
        }

        Self { scanners }
    }

    /// Returns (protocol_id, is_download) if a transfer signature is detected
    pub fn try_transfer(&mut self, ch: u8) -> Option<(String, bool)> {
        for scanner in &mut self.scanners {
            if scanner.recognizer.push_ch(ch) {
                return Some((scanner.protocol_id.clone(), scanner.is_download));
            }
        }
        None
    }
}

impl Default for AutoTransferScanner {
    fn default() -> Self {
        Self { scanners: Vec::new() }
    }
}
