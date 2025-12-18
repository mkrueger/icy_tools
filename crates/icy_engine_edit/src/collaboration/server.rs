//! WebSocket server for hosting Moebius-compatible collaboration sessions.
//!
//! This module provides a Tokio-based WebSocket server that hosts collaboration
//! sessions compatible with both Moebius clients and icy_draw clients.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast, mpsc};

use super::compression::compress_moebius_data;
use super::protocol::*;
use super::session::{Session, SessionEvent, SharedSession, UserId};

/// Error type for server operations.
#[derive(Debug)]
pub enum ServerError {
    /// Failed to bind to address
    BindFailed(String),
    /// Server error
    ServerError(String),
    /// Session error
    SessionError(String),
}

impl std::fmt::Display for ServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerError::BindFailed(msg) => write!(f, "Failed to bind: {}", msg),
            ServerError::ServerError(msg) => write!(f, "Server error: {}", msg),
            ServerError::SessionError(msg) => write!(f, "Session error: {}", msg),
        }
    }
}

impl std::error::Error for ServerError {}

/// Server configuration.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Address to bind to
    pub bind_addr: SocketAddr,
    /// Session password (empty for no password)
    pub password: String,
    /// Maximum users allowed (0 for unlimited)
    pub max_users: usize,
    /// Initial document columns
    pub columns: u32,
    /// Initial document rows
    pub rows: u32,
    /// Enable extended protocol features
    pub enable_extended_protocol: bool,
    /// Server status message
    pub status_message: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:8080".parse().unwrap(),
            password: String::new(),
            max_users: 0,
            columns: 80,
            rows: 25,
            enable_extended_protocol: true,
            status_message: String::new(),
        }
    }
}

/// Server state tracking connected clients.
#[derive(Debug)]
pub struct ServerState {
    /// The collaboration session
    pub session: SharedSession,
    /// Document data (column-major: blocks[col][row])
    document: RwLock<Vec<Vec<Block>>>,
    /// Connected client senders (for broadcasting)
    clients: RwLock<HashMap<UserId, mpsc::Sender<String>>>,
    /// Event broadcaster
    event_tx: broadcast::Sender<SessionEvent>,
    /// Server configuration
    config: ServerConfig,
}

impl ServerState {
    /// Create a new server state.
    pub fn new(config: ServerConfig) -> Arc<Self> {
        let session = Arc::new(Session::with_dimensions(config.password.clone(), config.columns, config.rows));

        // Initialize empty document
        let mut document = Vec::with_capacity(config.columns as usize);
        for _ in 0..config.columns {
            let column = vec![Block::default(); config.rows as usize];
            document.push(column);
        }

        let (event_tx, _) = broadcast::channel(1024);

        Arc::new(Self {
            session,
            document: RwLock::new(document),
            clients: RwLock::new(HashMap::new()),
            event_tx,
            config,
        })
    }

    /// Subscribe to server events.
    pub fn subscribe(&self) -> broadcast::Receiver<SessionEvent> {
        self.event_tx.subscribe()
    }

    /// Get the compressed document data.
    pub async fn get_compressed_document(&self) -> MoebiusDoc {
        let doc = self.document.read().await;
        let (columns, rows) = self.session.get_dimensions();

        // Flatten to row-major for compression (Moebius format)
        let mut blocks = Vec::with_capacity((columns * rows) as usize);
        for row in 0..rows as usize {
            for col in 0..columns as usize {
                if col < doc.len() && row < doc[col].len() {
                    blocks.push(doc[col][row].clone());
                } else {
                    blocks.push(Block::default());
                }
            }
        }
        let compressed = compress_moebius_data(&blocks);
        MoebiusDoc {
            columns,
            rows,
            data: None,
            compressed_data: Some(compressed),
            title: String::new(),
            author: String::new(),
            group: String::new(),
            date: String::new(),
            palette: serde_json::Value::Array(Vec::new()),
            font_name: self.session.font(),
            ice_colors: self.session.get_ice_colors(),
            use_9px_font: self.session.get_use_9px(),
            comments: String::new(),
            c64_background: None,
        }
    }

    /// Set a character in the document.
    pub async fn set_char(&self, col: i32, row: i32, block: Block) {
        let mut doc = self.document.write().await;
        let (columns, rows) = self.session.get_dimensions();

        if col >= 0 && col < columns as i32 && row >= 0 && row < rows as i32 {
            let col = col as usize;
            let row = row as usize;

            // Ensure document is large enough
            while doc.len() <= col {
                doc.push(vec![Block::default(); rows as usize]);
            }
            while doc[col].len() <= row {
                doc[col].push(Block::default());
            }

            doc[col][row] = block;
        }
    }

    /// Get a character from the document.
    pub async fn char_at(&self, col: i32, row: i32) -> Option<Block> {
        let doc = self.document.read().await;
        let (columns, rows) = self.session.get_dimensions();

        if col >= 0 && col < columns as i32 && row >= 0 && row < rows as i32 {
            let col = col as usize;
            let row = row as usize;
            doc.get(col).and_then(|c| c.get(row)).cloned()
        } else {
            None
        }
    }

    /// Resize the document.
    pub async fn resize(&self, new_columns: u32, new_rows: u32) {
        let mut doc = self.document.write().await;

        // Resize columns
        while doc.len() < new_columns as usize {
            doc.push(vec![Block::default(); new_rows as usize]);
        }
        doc.truncate(new_columns as usize);

        // Resize rows in each column
        for column in doc.iter_mut() {
            while column.len() < new_rows as usize {
                column.push(Block::default());
            }
            column.truncate(new_rows as usize);
        }

        self.session.set_dimensions(new_columns, new_rows);
    }

    /// Register a new client connection.
    pub async fn register_client(&self, user_id: UserId, sender: mpsc::Sender<String>) {
        self.clients.write().await.insert(user_id, sender);
    }

    /// Unregister a client connection.
    pub async fn unregister_client(&self, user_id: UserId) {
        self.clients.write().await.remove(&user_id);
        self.session.remove_user(user_id);
    }

    /// Broadcast a message to all clients except the sender.
    pub async fn broadcast(&self, message: &str, except: Option<UserId>) {
        let clients = self.clients.read().await;
        for (id, sender) in clients.iter() {
            if except != Some(*id) {
                let _ = sender.send(message.to_string()).await;
            }
        }
    }

    /// Broadcast a message to all clients.
    pub async fn broadcast_all(&self, message: &str) {
        self.broadcast(message, None).await;
    }

    /// Send a message to a specific client.
    pub async fn send_to(&self, user_id: UserId, message: &str) -> bool {
        let clients = self.clients.read().await;
        if let Some(sender) = clients.get(&user_id) {
            sender.send(message.to_string()).await.is_ok()
        } else {
            false
        }
    }

    /// Get the number of connected clients.
    pub async fn client_count(&self) -> usize {
        self.clients.read().await.len()
    }

    /// Emit a session event.
    pub fn emit_event(&self, event: SessionEvent) {
        let _ = self.event_tx.send(event);
    }

    /// Handle a connect request and return the response.
    pub async fn handle_connect(&self, nick: String, password: String) -> Result<(UserId, String), String> {
        // Check password
        if !self.session.check_password(&password) {
            let response = RefusedResponse {
                msg_type: ActionCode::Refused as u8,
                data: serde_json::json!({"reason": "Invalid password"}),
            };
            return Err(serde_json::to_string(&response).unwrap());
        }

        // Check max users
        if self.config.max_users > 0 && self.session.get_users().len() >= self.config.max_users {
            let response = RefusedResponse {
                msg_type: ActionCode::Refused as u8,
                data: serde_json::json!({"reason": "Server is full"}),
            };
            return Err(serde_json::to_string(&response).unwrap());
        }

        // Add user
        let user_id = self.session.add_user(nick);

        // Build connected response
        let response = ConnectedResponse {
            msg_type: ActionCode::Connected as u8,
            data: ConnectedData {
                id: user_id,
                doc: self.get_compressed_document().await,
                users: self.session.get_users(),
                chat_history: self.session.get_chat_history(),
                status: 0, // Default status
            },
        };

        let json = serde_json::to_string(&response).unwrap();
        Ok((user_id, json))
    }

    /// Handle a draw message.
    pub async fn handle_draw(&self, user_id: UserId, msg: DrawMessage) {
        // Update document
        self.set_char(msg.data.x, msg.data.y, msg.data.block.clone()).await;

        // Broadcast to other clients
        let broadcast_msg = serde_json::to_string(&msg).unwrap();
        self.broadcast(&broadcast_msg, Some(user_id)).await;

        // Emit event
        self.emit_event(SessionEvent::Draw {
            col: msg.data.x,
            row: msg.data.y,
            block: msg.data.block,
            layer: msg.data.layer,
        });
    }

    /// Handle a cursor message.
    pub async fn handle_cursor(&self, user_id: UserId, col: i32, row: i32) {
        self.session.update_cursor(user_id, col, row);

        // Broadcast cursor position with user ID
        #[derive(serde::Serialize)]
        struct CursorBroadcast {
            action: u8,
            id: u32,
            col: i32,
            row: i32,
        }

        let msg = CursorBroadcast {
            action: ActionCode::Cursor as u8,
            id: user_id,
            col,
            row,
        };
        let json = serde_json::to_string(&msg).unwrap();
        self.broadcast(&json, Some(user_id)).await;

        self.emit_event(SessionEvent::CursorMoved { id: user_id, col, row });
    }

    /// Handle a chat message.
    pub async fn handle_chat(&self, user_id: UserId, text: String) {
        let user = self.session.get_user(user_id);
        let nick = user.map(|u| u.nick).unwrap_or_else(|| "Unknown".to_string());

        self.session.add_chat_message(user_id, nick.clone(), text.clone());

        // Broadcast chat message
        let msg = ChatBroadcastMessage {
            msg_type: ActionCode::Chat as u8,
            data: ChatMessage {
                id: user_id,
                nick: nick.clone(),
                text: text.clone(),
                group: String::new(),
                time: 0,
            },
        };
        let json = serde_json::to_string(&msg).unwrap();
        self.broadcast_all(&json).await;

        self.emit_event(SessionEvent::Chat { nick, text });
    }

    /// Handle resize columns.
    pub async fn handle_resize_columns(&self, columns: u32) {
        let (_, rows) = self.session.get_dimensions();
        self.resize(columns, rows).await;

        // Broadcast
        let msg = ResizeColumnsMessage::new(columns);
        let json = serde_json::to_string(&msg).unwrap();
        self.broadcast_all(&json).await;

        self.emit_event(SessionEvent::Resized { columns, rows });
    }

    /// Handle resize rows.
    pub async fn handle_resize_rows(&self, rows: u32) {
        let (columns, _) = self.session.get_dimensions();
        self.resize(columns, rows).await;

        // Broadcast
        let msg = ResizeRowsMessage::new(rows);
        let json = serde_json::to_string(&msg).unwrap();
        self.broadcast_all(&json).await;

        self.emit_event(SessionEvent::Resized { columns, rows });
    }
}

/// Server handle for controlling the running server.
pub struct ServerHandle {
    /// Server state
    pub state: Arc<ServerState>,
    /// Shutdown signal sender
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl ServerHandle {
    /// Create a new server handle (server not started).
    pub fn new(config: ServerConfig) -> Self {
        Self {
            state: ServerState::new(config),
            shutdown_tx: None,
        }
    }

    /// Get the server state.
    pub fn state(&self) -> &Arc<ServerState> {
        &self.state
    }

    /// Request server shutdown.
    pub async fn shutdown(&self) {
        if let Some(tx) = &self.shutdown_tx {
            let _ = tx.send(()).await;
        }
    }

    /// Check if the server is running.
    pub fn is_running(&self) -> bool {
        self.shutdown_tx.is_some()
    }
}

/// Builder for creating collaboration servers.
pub struct ServerBuilder {
    config: ServerConfig,
}

impl ServerBuilder {
    /// Create a new server builder.
    pub fn new() -> Self {
        Self {
            config: ServerConfig::default(),
        }
    }

    /// Set the bind address.
    pub fn bind(mut self, addr: SocketAddr) -> Self {
        self.config.bind_addr = addr;
        self
    }

    /// Set the bind address from a string.
    pub fn bind_str(mut self, addr: &str) -> Result<Self, std::net::AddrParseError> {
        self.config.bind_addr = addr.parse()?;
        Ok(self)
    }

    /// Set the session password.
    pub fn password(mut self, password: impl Into<String>) -> Self {
        self.config.password = password.into();
        self
    }

    /// Set the maximum number of users.
    pub fn max_users(mut self, max: usize) -> Self {
        self.config.max_users = max;
        self
    }

    /// Set the document dimensions.
    pub fn dimensions(mut self, columns: u32, rows: u32) -> Self {
        self.config.columns = columns;
        self.config.rows = rows;
        self
    }

    /// Enable or disable extended protocol.
    pub fn enable_extended_protocol(mut self, enable: bool) -> Self {
        self.config.enable_extended_protocol = enable;
        self
    }

    /// Set the server status message.
    pub fn status_message(mut self, message: impl Into<String>) -> Self {
        self.config.status_message = message.into();
        self
    }

    /// Build the server handle (does not start listening).
    pub fn build(self) -> ServerHandle {
        ServerHandle::new(self.config)
    }
}

impl Default for ServerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_state_creation() {
        let config = ServerConfig::default();
        let state = ServerState::new(config);

        assert_eq!(state.client_count().await, 0);
        let (columns, rows) = state.session.get_dimensions();
        assert_eq!(columns, 80);
        assert_eq!(rows, 25);
    }

    #[tokio::test]
    async fn test_server_document_operations() {
        let config = ServerConfig::default();
        let state = ServerState::new(config);

        let block = Block { code: 65, fg: 7, bg: 0 };
        state.set_char(10, 5, block.clone()).await;

        let retrieved = state.char_at(10, 5).await.unwrap();
        assert_eq!(retrieved.code, 65);
        assert_eq!(retrieved.fg, 7);
        assert_eq!(retrieved.bg, 0);
    }

    #[tokio::test]
    async fn test_server_resize() {
        let config = ServerConfig::default();
        let state = ServerState::new(config);

        state.resize(100, 50).await;

        let (columns, rows) = state.session.get_dimensions();
        assert_eq!(columns, 100);
        assert_eq!(rows, 50);
    }

    #[tokio::test]
    async fn test_handle_connect() {
        let config = ServerConfig {
            password: "secret".to_string(),
            ..Default::default()
        };
        let state = ServerState::new(config);

        // Wrong password should fail
        let result = state.handle_connect("User".to_string(), "wrong".to_string()).await;
        assert!(result.is_err());

        // Correct password should succeed
        let result = state.handle_connect("User".to_string(), "secret".to_string()).await;
        assert!(result.is_ok());
        let (user_id, _) = result.unwrap();
        assert!(user_id > 0);
    }

    #[test]
    fn test_server_builder() {
        let handle = ServerBuilder::new()
            .bind_str("127.0.0.1:9000")
            .unwrap()
            .password("test")
            .max_users(10)
            .dimensions(160, 50)
            .build();

        assert!(!handle.is_running());
    }
}
