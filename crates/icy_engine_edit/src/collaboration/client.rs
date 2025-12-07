//! WebSocket client for connecting to Moebius-compatible collaboration servers.
//!
//! This module provides a Tokio-based WebSocket client that can connect to
//! both Moebius servers and icy_draw servers.

use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};

use super::protocol::*;
use super::session::{SessionEvent, UserId};

/// Error type for client operations.
#[derive(Debug)]
pub enum ClientError {
    /// Connection failed
    ConnectionFailed(String),
    /// Authentication failed (wrong password)
    AuthenticationFailed(String),
    /// WebSocket error
    WebSocketError(String),
    /// Protocol error
    ProtocolError(String),
    /// Connection closed
    Disconnected,
    /// Send failed
    SendFailed(String),
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientError::ConnectionFailed(msg) => write!(f, "Connection failed: {}", msg),
            ClientError::AuthenticationFailed(msg) => write!(f, "Authentication failed: {}", msg),
            ClientError::WebSocketError(msg) => write!(f, "WebSocket error: {}", msg),
            ClientError::ProtocolError(msg) => write!(f, "Protocol error: {}", msg),
            ClientError::Disconnected => write!(f, "Disconnected from server"),
            ClientError::SendFailed(msg) => write!(f, "Send failed: {}", msg),
        }
    }
}

impl std::error::Error for ClientError {}

/// Connection state for the client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    /// Not connected
    Disconnected,
    /// Connecting to server
    Connecting,
    /// Connected and authenticated
    Connected,
    /// Connection failed or lost
    Failed(String),
}

/// Client configuration.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Server URL (ws:// or wss://)
    pub url: String,
    /// User nickname
    pub nick: String,
    /// Session password
    pub password: String,
    /// Use extended protocol (V2) if supported
    pub use_extended_protocol: bool,
    /// Ping interval in seconds (0 to disable)
    pub ping_interval_secs: u64,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            nick: "Anonymous".to_string(),
            password: String::new(),
            use_extended_protocol: true,
            ping_interval_secs: 30,
        }
    }
}

/// Commands that can be sent to the client task.
#[derive(Debug)]
pub enum ClientCommand {
    /// Disconnect from server
    Disconnect,
    /// Send cursor position
    Cursor { col: i32, row: i32 },
    /// Send selection update
    Selection { selecting: bool, col: i32, row: i32 },
    /// Draw a character
    Draw { col: i32, row: i32, block: Block, layer: Option<usize> },
    /// Draw preview (temporary)
    DrawPreview { col: i32, row: i32, block: Block },
    /// Send chat message
    Chat { text: String },
    /// Resize columns
    ResizeColumns { columns: u32 },
    /// Resize rows
    ResizeRows { rows: u32 },
    /// Paste data
    Paste {
        data: String,
        col: i32,
        row: i32,
        columns: u32,
        rows: u32,
        layer: Option<usize>,
    },
    /// Set 9px mode
    SetUse9px { value: bool },
    /// Set ice colors
    SetIceColors { value: bool },
    /// Set font
    SetFont { font: String },
    /// Ping
    Ping,
}

/// Handle for interacting with the collaboration client.
///
/// This is the main interface for sending commands to the client task.
#[derive(Clone)]
pub struct ClientHandle {
    command_tx: mpsc::Sender<ClientCommand>,
    state: Arc<RwLock<ConnectionState>>,
    user_id: Arc<RwLock<Option<UserId>>>,
    protocol_version: Arc<RwLock<ProtocolVersion>>,
}

impl ClientHandle {
    /// Get the current connection state.
    pub async fn state(&self) -> ConnectionState {
        self.state.read().await.clone()
    }

    /// Check if connected.
    pub async fn is_connected(&self) -> bool {
        matches!(*self.state.read().await, ConnectionState::Connected)
    }

    /// Get the assigned user ID (if connected).
    pub async fn user_id(&self) -> Option<UserId> {
        *self.user_id.read().await
    }

    /// Get the negotiated protocol version.
    pub async fn protocol_version(&self) -> ProtocolVersion {
        *self.protocol_version.read().await
    }

    /// Disconnect from the server.
    pub async fn disconnect(&self) -> Result<(), ClientError> {
        self.command_tx
            .send(ClientCommand::Disconnect)
            .await
            .map_err(|e| ClientError::SendFailed(e.to_string()))
    }

    /// Send cursor position update.
    pub async fn send_cursor(&self, col: i32, row: i32) -> Result<(), ClientError> {
        self.command_tx
            .send(ClientCommand::Cursor { col, row })
            .await
            .map_err(|e| ClientError::SendFailed(e.to_string()))
    }

    /// Send selection update.
    pub async fn send_selection(&self, selecting: bool, col: i32, row: i32) -> Result<(), ClientError> {
        self.command_tx
            .send(ClientCommand::Selection { selecting, col, row })
            .await
            .map_err(|e| ClientError::SendFailed(e.to_string()))
    }

    /// Draw a character at the given position.
    pub async fn draw(&self, col: i32, row: i32, block: Block) -> Result<(), ClientError> {
        self.command_tx
            .send(ClientCommand::Draw { col, row, block, layer: None })
            .await
            .map_err(|e| ClientError::SendFailed(e.to_string()))
    }

    /// Draw a character at the given position on a specific layer (V2 only).
    pub async fn draw_on_layer(&self, col: i32, row: i32, block: Block, layer: usize) -> Result<(), ClientError> {
        self.command_tx
            .send(ClientCommand::Draw {
                col,
                row,
                block,
                layer: Some(layer),
            })
            .await
            .map_err(|e| ClientError::SendFailed(e.to_string()))
    }

    /// Draw a preview character (temporary).
    pub async fn draw_preview(&self, col: i32, row: i32, block: Block) -> Result<(), ClientError> {
        self.command_tx
            .send(ClientCommand::DrawPreview { col, row, block })
            .await
            .map_err(|e| ClientError::SendFailed(e.to_string()))
    }

    /// Send a chat message.
    pub async fn send_chat(&self, text: String) -> Result<(), ClientError> {
        self.command_tx
            .send(ClientCommand::Chat { text })
            .await
            .map_err(|e| ClientError::SendFailed(e.to_string()))
    }

    /// Resize columns.
    pub async fn resize_columns(&self, columns: u32) -> Result<(), ClientError> {
        self.command_tx
            .send(ClientCommand::ResizeColumns { columns })
            .await
            .map_err(|e| ClientError::SendFailed(e.to_string()))
    }

    /// Resize rows.
    pub async fn resize_rows(&self, rows: u32) -> Result<(), ClientError> {
        self.command_tx
            .send(ClientCommand::ResizeRows { rows })
            .await
            .map_err(|e| ClientError::SendFailed(e.to_string()))
    }

    /// Set 9px font mode.
    pub async fn set_use_9px(&self, value: bool) -> Result<(), ClientError> {
        self.command_tx
            .send(ClientCommand::SetUse9px { value })
            .await
            .map_err(|e| ClientError::SendFailed(e.to_string()))
    }

    /// Set ice colors mode.
    pub async fn set_ice_colors(&self, value: bool) -> Result<(), ClientError> {
        self.command_tx
            .send(ClientCommand::SetIceColors { value })
            .await
            .map_err(|e| ClientError::SendFailed(e.to_string()))
    }

    /// Set font.
    pub async fn set_font(&self, font: String) -> Result<(), ClientError> {
        self.command_tx
            .send(ClientCommand::SetFont { font })
            .await
            .map_err(|e| ClientError::SendFailed(e.to_string()))
    }
}

/// Builder for creating collaboration clients.
pub struct ClientBuilder {
    config: ClientConfig,
}

impl ClientBuilder {
    /// Create a new client builder.
    pub fn new() -> Self {
        Self {
            config: ClientConfig::default(),
        }
    }

    /// Set the server URL.
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.config.url = url.into();
        self
    }

    /// Set the user nickname.
    pub fn nick(mut self, nick: impl Into<String>) -> Self {
        self.config.nick = nick.into();
        self
    }

    /// Set the session password.
    pub fn password(mut self, password: impl Into<String>) -> Self {
        self.config.password = password.into();
        self
    }

    /// Enable or disable extended protocol.
    pub fn use_extended_protocol(mut self, enable: bool) -> Self {
        self.config.use_extended_protocol = enable;
        self
    }

    /// Set ping interval in seconds (0 to disable).
    pub fn ping_interval(mut self, seconds: u64) -> Self {
        self.config.ping_interval_secs = seconds;
        self
    }

    /// Get the configuration.
    pub fn config(&self) -> &ClientConfig {
        &self.config
    }

    /// Build the client (does not connect yet).
    ///
    /// Returns a handle for controlling the client and a receiver for events.
    /// The actual connection is established when you call `connect()` on the handle.
    pub fn build(self) -> (ClientHandle, mpsc::Receiver<SessionEvent>) {
        let (command_tx, _command_rx) = mpsc::channel(256);
        let (_event_tx, event_rx) = mpsc::channel(256);

        let state = Arc::new(RwLock::new(ConnectionState::Disconnected));
        let user_id = Arc::new(RwLock::new(None));
        let protocol_version = Arc::new(RwLock::new(ProtocolVersion::V1));

        let handle = ClientHandle {
            command_tx,
            state,
            user_id,
            protocol_version,
        };

        // Note: Actual connection logic will be implemented when tokio-tungstenite
        // is added as a dependency. For now, we just return the handle structure.

        (handle, event_rx)
    }
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Connect to a collaboration server.
///
/// This is a convenience function that creates a client and connects to the server.
///
/// # Arguments
///
/// * `url` - WebSocket URL of the server
/// * `nick` - User nickname
/// * `password` - Session password (empty string for no password)
///
/// # Returns
///
/// A tuple of (ClientHandle, event receiver)
pub fn connect(url: impl Into<String>, nick: impl Into<String>, password: impl Into<String>) -> (ClientHandle, mpsc::Receiver<SessionEvent>) {
    ClientBuilder::new().url(url).nick(nick).password(password).build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_builder() {
        let (handle, _events) = ClientBuilder::new().url("ws://localhost:8080").nick("TestUser").password("secret").build();

        assert!(!handle.is_connected().await);
        assert_eq!(handle.user_id().await, None);
    }

    #[test]
    fn test_client_config_default() {
        let config = ClientConfig::default();
        assert!(config.url.is_empty());
        assert_eq!(config.nick, "Anonymous");
        assert!(config.password.is_empty());
        assert!(config.use_extended_protocol);
        assert_eq!(config.ping_interval_secs, 30);
    }
}
