use i18n_embed_fl::fl;
use icy_net::modem::ModemCommand;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::protocol::CetProtocol;
use crate::protocol::ExternalProtocol;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TransferProtocol {
    #[serde(default = "default_true")]
    pub enabled: bool,

    #[serde(default)]
    pub id: String,

    #[serde(default)]
    pub name: String,

    #[serde(default)]
    pub description: String,

    /// Some old protocols require asking the user for a download location
    #[serde(default)]
    pub ask_for_download_location: bool,

    #[serde(default)]
    pub batch: bool,

    #[serde(default)]
    pub send_command: String,
    #[serde(default)]
    pub recv_command: String,

    /// Enable auto-transfer detection for this protocol
    #[serde(default)]
    pub auto_transfer: bool,

    /// Signature to detect when the remote initiates a download (we receive)
    #[serde(default)]
    pub download_signature: ModemCommand,

    /// Signature to detect when the remote initiates an upload (we send)
    #[serde(default)]
    pub upload_signature: ModemCommand,
}

impl TransferProtocol {
    /// Creates a `TransferProtocol` from an internal protocol id.
    /// Returns None if the id is not a known internal protocol.
    #[must_use]
    pub fn from_internal_id(id: &str) -> Option<Self> {
        match id {
            "@zmodem" => Some(Self {
                enabled: true,
                id: "@zmodem".to_string(),
                auto_transfer: true,
                batch: true,
                download_signature: "**\\x18B00".parse().unwrap_or_default(), // ZRQINIT
                upload_signature: "**\\x18B01".parse().unwrap_or_default(),   // ZRINIT
                ..Default::default()
            }),
            "@zmodem8k" => Some(Self {
                enabled: true,
                id: "@zmodem8k".to_string(),
                batch: true,
                ..Default::default()
            }),
            "@xmodem" => Some(Self {
                enabled: true,
                id: "@xmodem".to_string(),
                ask_for_download_location: true,
                ..Default::default()
            }),
            "@xmodem1k" => Some(Self {
                enabled: true,
                id: "@xmodem1k".to_string(),
                ask_for_download_location: true,
                ..Default::default()
            }),
            "@xmodem1kg" => Some(Self {
                enabled: true,
                id: "@xmodem1kg".to_string(),
                ask_for_download_location: true,
                ..Default::default()
            }),
            "@ymodem" => Some(Self {
                enabled: true,
                id: "@ymodem".to_string(),
                batch: true,
                ..Default::default()
            }),
            "@ymodemg" => Some(Self {
                enabled: true,
                id: "@ymodemg".to_string(),
                batch: true,
                ..Default::default()
            }),
            "@text" => Some(Self {
                enabled: true,
                id: "@text".to_string(),
                ..Default::default()
            }),
            "@cet" => Some(Self {
                enabled: true,
                id: "@cet".to_string(),
                ..Default::default()
            }),
            _ => None,
        }
    }

    /// Returns true if this is an internal protocol (id starts with @)
    #[must_use]
    pub fn is_internal(&self) -> bool {
        self.id.starts_with('@')
    }

    /// Returns the display name for the protocol.
    /// For internal protocols, returns hardcoded names; for external protocols, returns the name field.
    #[must_use]
    pub fn get_name(&self) -> String {
        if self.is_internal() {
            match self.id.as_str() {
                "@zmodem" => "Zmodem".to_string(),
                "@zmodem8k" => "ZedZap".to_string(),
                "@xmodem" => "Xmodem".to_string(),
                "@xmodem1k" => "Xmodem 1k".to_string(),
                "@xmodem1kg" => "Xmodem 1k-G".to_string(),
                "@ymodem" => "Ymodem".to_string(),
                "@ymodemg" => "Ymodem-G".to_string(),
                "@text" => "Text".to_string(),
                "@cet" => "CET Telesoftware".to_string(),
                _ => self.name.clone(),
            }
        } else {
            self.name.clone()
        }
    }

    /// Returns the description for the protocol.
    /// For internal protocols, uses i18n keys; for external protocols, returns the description field.
    pub fn get_description(&self) -> String {
        if self.is_internal() {
            match self.id.as_str() {
                "@zmodem" => fl!(crate::LANGUAGE_LOADER, "protocol-zmodem-description"),
                "@zmodem8k" => fl!(crate::LANGUAGE_LOADER, "protocol-zmodem8k-description"),
                "@xmodem" => fl!(crate::LANGUAGE_LOADER, "protocol-xmodem-description"),
                "@xmodem1k" => fl!(crate::LANGUAGE_LOADER, "protocol-xmodem1k-description"),
                "@xmodem1kg" => fl!(crate::LANGUAGE_LOADER, "protocol-xmodem1kG-description"),
                "@ymodem" => fl!(crate::LANGUAGE_LOADER, "protocol-ymodem-description"),
                "@ymodemg" => fl!(crate::LANGUAGE_LOADER, "protocol-ymodemg-description"),
                "@text" => fl!(crate::LANGUAGE_LOADER, "protocol-text-description"),
                "@cet" => fl!(crate::LANGUAGE_LOADER, "protocol-cet-description"),
                _ => self.description.clone(),
            }
        } else {
            self.description.clone()
        }
    }

    /// Creates a Protocol instance for this transfer protocol.
    /// For internal protocols, creates the built-in implementation.
    /// For external protocols, creates an `ExternalProtocol` that runs the configured command.
    ///
    /// `download_dir` is used for external protocols to expand the `%D` placeholder.
    #[must_use]
    pub fn create(&self, download_dir: PathBuf) -> Option<Box<dyn icy_net::protocol::Protocol>> {
        use icy_net::protocol::TransferProtocolType;

        // Internal protocols start with @
        if self.id.starts_with('@') {
            if self.id == "@cet" {
                return Some(Box::new(CetProtocol::new(download_dir)));
            }
            let protocol_type = match self.id.as_str() {
                "@zmodem" => TransferProtocolType::ZModem,
                "@zmodem8k" => TransferProtocolType::ZModem8k,
                "@xmodem" => TransferProtocolType::XModem,
                "@xmodem1k" => TransferProtocolType::XModem1k,
                "@xmodem1kg" => TransferProtocolType::XModem1kG,
                "@ymodem" => TransferProtocolType::YModem,
                "@ymodemg" => TransferProtocolType::YModemG,
                "@text" => TransferProtocolType::ASCII,
                _ => return None,
            };
            Some(protocol_type.create())
        } else {
            // External protocol - use configured commands
            if self.send_command.is_empty() && self.recv_command.is_empty() {
                return None;
            }
            Some(Box::new(ExternalProtocol::new(
                self.name.clone(),
                self.send_command.clone(),
                self.recv_command.clone(),
                download_dir,
            )))
        }
    }
}

/// Returns the default list of built-in transfer protocols
#[must_use]
pub fn default_protocols() -> Vec<TransferProtocol> {
    vec![
        TransferProtocol {
            enabled: true,
            id: "@zmodem".to_string(),
            name: String::new(),
            description: String::new(),
            ask_for_download_location: false,
            batch: true,
            send_command: String::new(),
            recv_command: String::new(),
            auto_transfer: true,
            download_signature: "**\\x18B00".parse().unwrap_or_default(), // ZRQINIT
            upload_signature: "**\\x18B01".parse().unwrap_or_default(),   // ZRINIT
        },
        TransferProtocol {
            enabled: true,
            id: "@zmodem8k".to_string(),
            name: String::new(),
            description: String::new(),
            ask_for_download_location: false,
            batch: true,
            send_command: String::new(),
            recv_command: String::new(),
            auto_transfer: false,
            download_signature: ModemCommand::default(),
            upload_signature: ModemCommand::default(),
        },
        TransferProtocol {
            enabled: true,
            id: "@xmodem".to_string(),
            name: String::new(),
            description: String::new(),
            ask_for_download_location: true,
            batch: false,
            send_command: String::new(),
            recv_command: String::new(),
            auto_transfer: false,
            download_signature: ModemCommand::default(),
            upload_signature: ModemCommand::default(),
        },
        TransferProtocol {
            enabled: true,
            id: "@xmodem1k".to_string(),
            name: String::new(),
            description: String::new(),
            ask_for_download_location: true,
            batch: false,
            send_command: String::new(),
            recv_command: String::new(),
            auto_transfer: false,
            download_signature: ModemCommand::default(),
            upload_signature: ModemCommand::default(),
        },
        TransferProtocol {
            enabled: true,
            id: "@xmodem1kg".to_string(),
            name: String::new(),
            description: String::new(),
            ask_for_download_location: true,
            batch: false,
            send_command: String::new(),
            recv_command: String::new(),
            auto_transfer: false,
            download_signature: ModemCommand::default(),
            upload_signature: ModemCommand::default(),
        },
        TransferProtocol {
            enabled: true,
            id: "@ymodem".to_string(),
            name: String::new(),
            description: String::new(),
            ask_for_download_location: false,
            batch: true,
            send_command: String::new(),
            recv_command: String::new(),
            auto_transfer: false,
            download_signature: ModemCommand::default(),
            upload_signature: ModemCommand::default(),
        },
        TransferProtocol {
            enabled: true,
            id: "@ymodemg".to_string(),
            name: String::new(),
            description: String::new(),
            ask_for_download_location: false,
            batch: true,
            send_command: String::new(),
            recv_command: String::new(),
            auto_transfer: false,
            download_signature: ModemCommand::default(),
            upload_signature: ModemCommand::default(),
        },
        TransferProtocol {
            enabled: true,
            id: "@text".to_string(),
            name: String::new(),
            description: String::new(),
            ask_for_download_location: false,
            batch: false,
            send_command: String::new(),
            recv_command: String::new(),
            auto_transfer: false,
            download_signature: ModemCommand::default(),
            upload_signature: ModemCommand::default(),
        },
        TransferProtocol {
            enabled: true,
            id: "@cet".to_string(),
            name: String::new(),
            description: String::new(),
            ask_for_download_location: false,
            batch: false,
            send_command: String::new(),
            recv_command: String::new(),
            auto_transfer: false,
            download_signature: ModemCommand::default(),
            upload_signature: ModemCommand::default(),
        },
    ]
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use icy_net::{
        connection::channel::ChannelConnection,
        protocol::{Header, HeaderType, ZFrameType, Zmodem, ZCRCE, ZCRCW},
        Connection,
    };

    #[tokio::test]
    async fn internal_zmodem_downloads_accept_large_subpackets() {
        for id in ["@zmodem", "@zmodem8k"] {
            for size in [1025, 8192] {
                tokio::time::timeout(std::time::Duration::from_secs(10), async {
                    let protocol = TransferProtocol::from_internal_id(id).unwrap();
                    let mut receiver = protocol.create(std::env::temp_dir()).unwrap();
                    let (mut conn, mut peer) = ChannelConnection::create_pair();
                    let mut state = receiver.initiate_recv(&mut conn).await.unwrap();
                    assert_eq!(Header::read(&mut peer, &mut 0).await.unwrap().unwrap().frame_type, ZFrameType::RIinit);

                    let data: Vec<u8> = (0..size).map(|index| (index % 256) as u8).collect();
                    let metadata = format!("download.bin\0{size}\0");
                    let mut bytes = Header::empty(ZFrameType::File).build(HeaderType::Bin32, false);
                    bytes.extend(Zmodem::encode_subpacket_crc32(ZCRCW, metadata.as_bytes(), false));
                    peer.send(&bytes).await.unwrap();
                    receiver.update_transfer(&mut conn, &mut state).await.unwrap();
                    assert_eq!(Header::read(&mut peer, &mut 0).await.unwrap().unwrap().frame_type, ZFrameType::RPos);

                    let mut bytes = Header::from_number(ZFrameType::Data, 0).build(HeaderType::Bin32, false);
                    bytes.extend(Zmodem::encode_subpacket_crc32(ZCRCE, &data, false));
                    peer.send(&bytes).await.unwrap();
                    while state.recieve_state.cur_bytes_transfered < size as u64 {
                        receiver.update_transfer(&mut conn, &mut state).await.unwrap();
                    }
                    assert_eq!(state.recieve_state.cur_bytes_transfered, size as u64);
                    assert_eq!(state.recieve_state.errors, 0);
                    assert_eq!(state.recieve_state.warnings, 0);
                    Header::from_number(ZFrameType::Eof, size as u32)
                        .write(&mut peer, HeaderType::Bin32, false)
                        .await
                        .unwrap();
                    while state.recieve_state.finished_files.is_empty() {
                        receiver.update_transfer(&mut conn, &mut state).await.unwrap();
                    }
                    let path = &state.recieve_state.finished_files[0].1;
                    let received = std::fs::read(path).unwrap();
                    std::fs::remove_file(path).unwrap();
                    assert_eq!(received, data, "{id} download with a {size}-byte subpacket");
                })
                .await
                .expect("ZMODEM download regression timed out");
            }
        }
    }
}
