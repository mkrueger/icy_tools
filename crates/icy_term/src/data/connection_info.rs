use icy_net::ConnectionType;
use std::fmt;

use crate::{Address, Res};

#[derive(Debug, Default, Clone)]
pub struct ConnectionInformation {
    pub protocol: Option<ConnectionType>,
    pub host: String,
    pub port: Option<u16>,
    pub user_name: Option<String>,
    pub password: Option<String>,
}

impl From<Address> for ConnectionInformation {
    fn from(address: Address) -> Self {
        if address.protocol == ConnectionType::Modem {
            return ConnectionInformation {
                protocol: Some(ConnectionType::Modem),
                host: address.address,
                port: None,
                user_name: None,
                password: None,
            };
        }

        let mut result = ConnectionInformation::parse(&address.address).unwrap_or_default();

        if result.protocol.is_none() {
            result.protocol = Some(address.protocol);
        }

        if result.user_name.is_none() && !address.user_name.is_empty() {
            result.user_name = Some(address.user_name);
        }

        if result.password.is_none() && !address.password.is_empty() {
            result.password = Some(address.password);
        }

        result
    }
}

impl fmt::Display for ConnectionInformation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Build the URL format: [protocol://][user[:password]@]host[:port]

        // Protocol
        if let Some(protocol) = &self.protocol {
            write!(
                f,
                "{}://",
                match protocol {
                    ConnectionType::Telnet => "telnet",
                    ConnectionType::SSH => "ssh",
                    ConnectionType::Raw => "raw",
                    ConnectionType::Websocket => "ws",
                    ConnectionType::SecureWebsocket => "wss",
                    ConnectionType::Modem => "modem",
                    _ => "unknown",
                }
            )?;
        }

        // Username and password
        if let Some(username) = &self.user_name {
            write!(f, "{}", username)?;
        }
        if let Some(password) = &self.password {
            write!(f, ":{}", password)?;
        }
        if self.user_name.is_some() || self.password.is_some() {
            write!(f, "@")?;
        }

        // Host
        write!(f, "{}", self.host)?;

        if let Some(port) = &self.port {
            write!(f, ":{}", port)?;
        }

        Ok(())
    }
}

impl ConnectionInformation {
    pub fn parse(url: &str) -> Res<Self> {
        let url = url.trim();

        match url::Url::parse(&url) {
            Err(err) => {
                if err == url::ParseError::RelativeUrlWithoutBase {
                    return Self::parse_address_string(&url);
                }
                Err(Box::new(err))
            }
            Ok(parsed) => {
                if parsed.scheme() == "modem" {
                    return Ok(Self {
                        protocol: Some(ConnectionType::Modem),
                        host: parsed.path().to_string(),
                        port: None,
                        user_name: None,
                        password: None,
                    });
                }

                // Determine protocol from scheme
                let protocol = match parsed.scheme() {
                    "telnet" => Some(ConnectionType::Telnet),
                    "ssh" => Some(ConnectionType::SSH),
                    "raw" => Some(ConnectionType::Raw),
                    "ws" => Some(ConnectionType::Websocket),
                    "wss" => Some(ConnectionType::SecureWebsocket),
                    "modem" => Some(ConnectionType::Modem),
                    _ => None,
                };

                // Extract username and password if present
                let user_name = if !parsed.username().is_empty() {
                    Some(parsed.username().to_string())
                } else {
                    None
                };

                let password = if let Some(password) = parsed.password() {
                    Some(password.to_string())
                } else {
                    None
                };

                let (host, port) = if parsed.has_host() {
                    (parsed.host_str().unwrap_or("").to_string(), parsed.port())
                } else {
                    return Self::parse_address_string(&url);
                };

                Ok(Self {
                    protocol,
                    host,
                    port,
                    user_name,
                    password,
                })
            }
        }
    }

    fn parse_address_string(address: &str) -> Res<Self> {
        let mut user_name = None;
        let mut password = None;
        let mut remaining = address;

        // Check for credentials (user[:password]@)
        if let Some(at_pos) = remaining.rfind('@') {
            let credentials = &remaining[..at_pos];
            remaining = &remaining[at_pos + 1..];

            if let Some(colon_pos) = credentials.find(':') {
                let user = &credentials[..colon_pos];
                let pass = &credentials[colon_pos + 1..];
                if !user.is_empty() {
                    user_name = Some(user.to_string());
                }
                if !pass.is_empty() {
                    password = Some(pass.to_string());
                }
            } else if !credentials.is_empty() {
                user_name = Some(credentials.to_string());
            }
        }

        // Parse host[:port]
        let (host, port) = if let Some(colon_pos) = remaining.rfind(':') {
            let host_part = &remaining[..colon_pos];
            let port_part = &remaining[colon_pos + 1..];
            let port_num = port_part.parse::<u16>()?;
            (host_part.to_string(), Some(port_num))
        } else {
            (remaining.to_string(), None)
        };

        Ok(Self {
            protocol: None,
            host,
            port,
            user_name,
            password,
        })
    }

    pub fn protocol(&self) -> ConnectionType {
        self.protocol.unwrap_or(ConnectionType::Telnet)
    }

    pub fn port(&self) -> u16 {
        self.port.unwrap_or_else(|| match self.protocol() {
            ConnectionType::Telnet => 23,
            ConnectionType::SSH => 22,
            ConnectionType::Raw => 23,
            ConnectionType::Websocket => 80,
            ConnectionType::SecureWebsocket => 443,
            _ => 23,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Address, ConnectionInformation};
    use icy_net::ConnectionType;

    #[test]
    fn test_parse_telnet_with_port() {
        let url = "telnet://bbs.example.com:2323";
        let conn_info = ConnectionInformation::parse(url).unwrap();

        assert_eq!(conn_info.protocol, Some(ConnectionType::Telnet));
        assert_eq!(conn_info.host, "bbs.example.com");
        assert_eq!(conn_info.port, Some(2323));
        assert_eq!(conn_info.user_name, None);
        assert_eq!(conn_info.password, None);
    }

    #[test]
    fn test_parse_ssh_with_credentials() {
        let url = "ssh://user:pass@bbs.example.com:2222";
        let conn_info = ConnectionInformation::parse(url).unwrap();

        assert_eq!(conn_info.protocol, Some(ConnectionType::SSH));
        assert_eq!(conn_info.host, "bbs.example.com");
        assert_eq!(conn_info.port, Some(2222));
        assert_eq!(conn_info.user_name, Some("user".to_string()));
        assert_eq!(conn_info.password, Some("pass".to_string()));
    }

    #[test]
    fn test_parse_base_with_credentials() {
        let url = "user:pass@bbs.example.com:2222";
        let conn_info = ConnectionInformation::parse(url).unwrap();

        assert_eq!(conn_info.protocol, None);
        assert_eq!(conn_info.host, "bbs.example.com");
        assert_eq!(conn_info.port, Some(2222));
        assert_eq!(conn_info.user_name, Some("user".to_string()));
        assert_eq!(conn_info.password, Some("pass".to_string()));
    }

    #[test]
    fn test_parse_websocket() {
        let url = "ws://bbs.example.com:8080";
        let conn_info = ConnectionInformation::parse(url).unwrap();

        assert_eq!(conn_info.protocol, Some(ConnectionType::Websocket));
        assert_eq!(conn_info.host, "bbs.example.com");
        assert_eq!(conn_info.port, Some(8080));
    }

    #[test]
    fn test_parse_secure_websocket() {
        let url = "wss://secure.bbs.example.com";
        let conn_info = ConnectionInformation::parse(url).unwrap();

        assert_eq!(conn_info.protocol, Some(ConnectionType::SecureWebsocket));
        assert_eq!(conn_info.host, "secure.bbs.example.com");
        assert_eq!(conn_info.port, None); // Default port
    }

    #[test]
    fn test_parse_raw_protocol() {
        let url = "raw://bbs.example.com:23";
        let conn_info = ConnectionInformation::parse(url).unwrap();

        assert_eq!(conn_info.protocol, Some(ConnectionType::Raw));
        assert_eq!(conn_info.host, "bbs.example.com");
        assert_eq!(conn_info.port, Some(23));
    }

    #[test]
    fn test_parse_with_username_only() {
        let url = "telnet://sysop@bbs.example.com";
        let conn_info = ConnectionInformation::parse(url).unwrap();

        assert_eq!(conn_info.protocol, Some(ConnectionType::Telnet));
        assert_eq!(conn_info.host, "bbs.example.com");
        assert_eq!(conn_info.user_name, Some("sysop".to_string()));
        assert_eq!(conn_info.password, None);
    }

    #[test]
    fn test_display_telnet_default_port() {
        let conn_info = ConnectionInformation {
            protocol: Some(ConnectionType::Telnet),
            host: "bbs.example.com".to_string(),
            port: None,
            user_name: None,
            password: None,
        };

        let display = format!("{}", conn_info);
        assert_eq!(display, "telnet://bbs.example.com");
    }

    #[test]
    fn test_display_ssh_with_credentials() {
        let conn_info = ConnectionInformation {
            protocol: Some(ConnectionType::SSH),
            host: "bbs.example.com".to_string(),
            port: Some(2222),
            user_name: Some("sysop".to_string()),
            password: Some("secret".to_string()),
        };

        let display = format!("{}", conn_info);
        assert_eq!(display, "ssh://sysop:secret@bbs.example.com:2222");
    }

    #[test]
    fn test_display_no_protocol() {
        let conn_info = ConnectionInformation {
            protocol: None,
            host: "bbs.example.com".to_string(),
            port: Some(8888),
            user_name: None,
            password: None,
        };

        let display = format!("{}", conn_info);
        assert_eq!(display, "bbs.example.com:8888");
    }

    #[test]
    fn test_protocol_method_default() {
        let conn_info = ConnectionInformation {
            protocol: None,
            host: "bbs.example.com".to_string(),
            port: None,
            user_name: None,
            password: None,
        };

        // Should default to Telnet
        assert_eq!(conn_info.protocol(), ConnectionType::Telnet);
    }

    #[test]
    fn test_port_method_defaults() {
        // Telnet default
        let telnet = ConnectionInformation {
            protocol: Some(ConnectionType::Telnet),
            host: "bbs.example.com".to_string(),
            port: None,
            user_name: None,
            password: None,
        };
        assert_eq!(telnet.port(), 23);

        // SSH default
        let ssh = ConnectionInformation {
            protocol: Some(ConnectionType::SSH),
            host: "bbs.example.com".to_string(),
            port: None,
            user_name: None,
            password: None,
        };
        assert_eq!(ssh.port(), 22);

        // Websocket default
        let ws = ConnectionInformation {
            protocol: Some(ConnectionType::Websocket),
            host: "bbs.example.com".to_string(),
            port: None,
            user_name: None,
            password: None,
        };
        assert_eq!(ws.port(), 80);

        // Secure Websocket default
        let wss = ConnectionInformation {
            protocol: Some(ConnectionType::SecureWebsocket),
            host: "bbs.example.com".to_string(),
            port: None,
            user_name: None,
            password: None,
        };
        assert_eq!(wss.port(), 443);
    }

    #[test]
    fn test_from_address() {
        use chrono::{Duration, Utc};
        use icy_engine::ansi::{BaudEmulation, MusicOption};
        use icy_net::telnet::TerminalEmulation;

        let address = Address {
            system_name: "Test BBS".to_string(),
            address: "bbs.example.com:2323".to_string(),
            user_name: "testuser".to_string(),
            password: "testpass".to_string(),
            protocol: ConnectionType::SSH,
            terminal_type: TerminalEmulation::Ansi,
            comment: String::new(),
            font_name: None,
            screen_mode: Default::default(),
            auto_login: String::new(),
            proxy_command: String::new(),
            ansi_music: MusicOption::default(),
            ice_mode: true,
            is_favored: false,
            created: Utc::now(),
            updated: Utc::now(),
            overall_duration: Duration::zero(),
            number_of_calls: 0,
            last_call: None,
            last_call_duration: Duration::zero(),
            uploaded_bytes: 0,
            downloaded_bytes: 0,
            baud_emulation: BaudEmulation::default(),
        };

        let conn_info: ConnectionInformation = ConnectionInformation::from(address);

        assert_eq!(conn_info.protocol, Some(ConnectionType::SSH));
        assert_eq!(conn_info.host, "bbs.example.com");
        assert_eq!(conn_info.port, Some(2323));
        assert_eq!(conn_info.user_name, Some("testuser".to_string()));
        assert_eq!(conn_info.password, Some("testpass".to_string()));
    }

    #[test]
    fn test_from_address_no_port() {
        use chrono::{Duration, Utc};
        use icy_engine::ansi::{BaudEmulation, MusicOption};
        use icy_net::telnet::TerminalEmulation;

        let address = Address {
            system_name: "Test BBS".to_string(),
            address: "bbs.example.com".to_string(),
            user_name: String::new(),
            password: String::new(),
            protocol: ConnectionType::Telnet,
            terminal_type: TerminalEmulation::Ansi,
            comment: String::new(),
            font_name: None,
            screen_mode: Default::default(),
            auto_login: String::new(),
            proxy_command: String::new(),
            ansi_music: MusicOption::default(),
            ice_mode: true,
            is_favored: false,
            created: Utc::now(),
            updated: Utc::now(),
            overall_duration: Duration::zero(),
            number_of_calls: 0,
            last_call: None,
            last_call_duration: Duration::zero(),
            uploaded_bytes: 0,
            downloaded_bytes: 0,
            baud_emulation: BaudEmulation::default(),
        };

        let conn_info = ConnectionInformation::from(address);
        assert_eq!(conn_info.protocol, Some(ConnectionType::Telnet));
        assert_eq!(conn_info.host, "bbs.example.com");
        assert_eq!(conn_info.port, None);
        assert_eq!(conn_info.user_name, None);
        assert_eq!(conn_info.password, None);
    }

    #[test]
    fn test_from_address_user_override() {
        use chrono::{Duration, Utc};
        use icy_engine::ansi::{BaudEmulation, MusicOption};
        use icy_net::telnet::TerminalEmulation;

        let address = Address {
            system_name: "Test BBS".to_string(),
            address: "foo:bar@bbs.example.com".to_string(),
            user_name: "override_user".to_string(),
            password: "override_pass".to_string(),
            protocol: ConnectionType::Telnet,
            terminal_type: TerminalEmulation::Ansi,
            comment: String::new(),
            font_name: None,
            screen_mode: Default::default(),
            auto_login: String::new(),
            proxy_command: String::new(),
            ansi_music: MusicOption::default(),
            ice_mode: true,
            is_favored: false,
            created: Utc::now(),
            updated: Utc::now(),
            overall_duration: Duration::zero(),
            number_of_calls: 0,
            last_call: None,
            last_call_duration: Duration::zero(),
            uploaded_bytes: 0,
            downloaded_bytes: 0,
            baud_emulation: BaudEmulation::default(),
        };

        let conn_info = ConnectionInformation::from(address);

        assert_eq!(conn_info.protocol, Some(ConnectionType::Telnet));
        assert_eq!(conn_info.host, "bbs.example.com");
        assert_eq!(conn_info.port, None);
        assert_eq!(conn_info.user_name, Some("foo".to_string()));
        assert_eq!(conn_info.password, Some("bar".to_string()));
    }

    #[test]
    fn test_roundtrip_parse_display() {
        let original_url = "ssh://user:pass@bbs.example.com:2222";
        let conn_info = ConnectionInformation::parse(original_url).unwrap();
        let display = format!("{}", conn_info);

        // Parse the displayed string again
        let reparsed = ConnectionInformation::parse(&display).unwrap();

        assert_eq!(conn_info.protocol, reparsed.protocol);
        assert_eq!(conn_info.host, reparsed.host);
        assert_eq!(conn_info.port, reparsed.port);
        assert_eq!(conn_info.user_name, reparsed.user_name);
        assert_eq!(conn_info.password, reparsed.password);
    }

    #[test]
    fn test_default() {
        let conn_info = ConnectionInformation::default();

        assert_eq!(conn_info.protocol, None);
        assert_eq!(conn_info.host, "");
        assert_eq!(conn_info.port, None);
        assert_eq!(conn_info.user_name, None);
        assert_eq!(conn_info.password, None);
    }

    #[test]
    fn test_parse_invalid_url() {
        // Invalid URL should return an error
        let result = ConnectionInformation::parse("invalid:-1");
        assert!(result.is_err());
        let result = ConnectionInformation::parse("invalid:url");
        assert!(result.is_err());
        let result = ConnectionInformation::parse("invalid:5945549423");
        assert!(result.is_err());
    }
}
