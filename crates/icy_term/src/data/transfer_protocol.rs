use i18n_embed_fl::fl;
use icy_net::modem::ModemCommand;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
    /// Creates a TransferProtocol from an internal protocol id.
    /// Returns None if the id is not a known internal protocol.
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
            _ => None,
        }
    }

    /// Returns true if this is an internal protocol (id starts with @)
    pub fn is_internal(&self) -> bool {
        self.id.starts_with('@')
    }

    /// Returns the display name for the protocol.
    /// For internal protocols, returns hardcoded names; for external protocols, returns the name field.
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
                _ => self.description.clone(),
            }
        } else {
            self.description.clone()
        }
    }

    /// Creates a Protocol instance for this transfer protocol.
    /// For internal protocols, creates the built-in implementation.
    /// For external protocols, creates an ExternalProtocol that runs the configured command.
    ///
    /// `download_dir` is used for external protocols to expand the `%D` placeholder.
    pub fn create(&self, download_dir: PathBuf) -> Option<Box<dyn icy_net::protocol::Protocol>> {
        use icy_net::protocol::TransferProtocolType;

        // Internal protocols start with @
        if self.id.starts_with('@') {
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
    ]
}

fn default_true() -> bool {
    true
}
