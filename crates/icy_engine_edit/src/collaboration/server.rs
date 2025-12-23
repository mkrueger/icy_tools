//! WebSocket server for hosting Moebius-compatible collaboration sessions.
//!
//! This module provides a Tokio-based WebSocket server that hosts collaboration
//! sessions compatible with both Moebius clients and icy_draw clients.
//!
//! # Broadcast Behavior (Moebius Compatibility)
//!
//! The server implements three broadcast methods matching Moebius behavior:
//!
//! | Method | Moebius Equivalent | Description |
//! |--------|-------------------|-------------|
//! | `broadcast()` | `send_all_including_guests` | All clients except sender (incl. web guests) |
//! | `broadcast_to_registered()` | `send_all` | Registered users only, except sender |
//! | `broadcast_to_registered_including_self()` | `send_all_including_self` | Registered users including sender |
//!
//! ## Action-specific broadcast rules:
//!
//! - **DRAW**: `broadcast()` - All clients see drawing updates (including web viewers)
//! - **CHAT**: `broadcast_to_registered()` - Only registered users receive chat messages
//! - **JOIN/LEAVE**: `broadcast_to_registered()` - Only registered users get notified
//! - **STATUS**: `broadcast_to_registered_including_self()` - Status echoed back to sender
//! - **SAUCE, ICE_COLORS, USE_9PX_FONT, CHANGE_FONT, SET_CANVAS_SIZE**: `broadcast()` - Document settings to all
//!
//! Web clients (guests) are identified by an empty nickname and receive status `WEB=3`.
//! They receive drawing updates but not chat, join/leave notifications, or status changes.

use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{RwLock, broadcast, mpsc};
use tokio_tungstenite::tungstenite::Message;

use super::compression::compress_moebius_data;
use super::protocol::*;
use super::session::{Session, SessionEvent, SharedSession, UserId};
use crate::SauceMetaData;

// ANSI color codes for server output
pub(crate) mod colors {
    pub const RESET: &str = "\x1b[0m";
    pub const BOLD: &str = "\x1b[1m";
    pub const CYAN: &str = "\x1b[1;36m";
    pub const GREEN: &str = "\x1b[1;32m";
    pub const YELLOW: &str = "\x1b[1;33m";
    pub const RED: &str = "\x1b[1;31m";
    pub const BLUE: &str = "\x1b[1;34m";
    pub const MAGENTA: &str = "\x1b[1;35m";
    pub const WHITE: &str = "\x1b[1;37m";
    pub const GRAY: &str = "\x1b[1;90m";
}

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
    /// Initial document content (column-major: blocks[col][row])
    /// If None, creates empty document
    pub initial_document: Option<Vec<Vec<Block>>>,
    /// Initial ICE colors setting
    pub ice_colors: bool,
    /// Initial 9px font setting
    pub use_9px_font: bool,
    /// Initial font name
    pub font_name: String,
    /// Color palette (exactly 16 RGB colors)
    /// Each color is [r, g, b] with values 0-255
    pub palette: [[u8; 3]; 16],
    /// SAUCE metadata
    pub sauce: SauceMetaData,

    /// Autosave configuration
    pub autosave: super::autosave::AutosaveConfig,

    // UI strings for localization (defaults to English)
    /// Server banner title (default: "icy_draw Collaboration Server")
    pub ui_title: String,
    /// Label for bind address (default: "Bind Address")
    pub ui_bind_address: String,
    /// Label for password (default: "Password")
    pub ui_password: String,
    /// Label for document size (default: "Document")
    pub ui_document: String,
    /// Label for max users (default: "Max Users")
    pub ui_max_users: String,
    /// Label for connect URL (default: "Connect with")
    pub ui_connect_with: String,
    /// Stop server hint (default: "Press Ctrl+C to stop the server")
    pub ui_stop_hint: String,
    /// "none" placeholder for empty password
    pub ui_none: String,
    /// "unlimited" for max_users = 0
    pub ui_unlimited: String,
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
            initial_document: None,
            ice_colors: false,
            use_9px_font: false,
            font_name: "IBM VGA".to_string(),
            // Standard EGA palette (16 colors, 8-bit RGB)
            palette: [
                [0x00, 0x00, 0x00], // 0: Black
                [0x00, 0x00, 0xAA], // 1: Blue
                [0x00, 0xAA, 0x00], // 2: Green
                [0x00, 0xAA, 0xAA], // 3: Cyan
                [0xAA, 0x00, 0x00], // 4: Red
                [0xAA, 0x00, 0xAA], // 5: Magenta
                [0xAA, 0x55, 0x00], // 6: Brown/Yellow
                [0xAA, 0xAA, 0xAA], // 7: Light Gray
                [0x55, 0x55, 0x55], // 8: Dark Gray
                [0x55, 0x55, 0xFF], // 9: Light Blue
                [0x55, 0xFF, 0x55], // 10: Light Green
                [0x55, 0xFF, 0xFF], // 11: Light Cyan
                [0xFF, 0x55, 0x55], // 12: Light Red
                [0xFF, 0x55, 0xFF], // 13: Light Magenta
                [0xFF, 0xFF, 0x55], // 14: Yellow
                [0xFF, 0xFF, 0xFF], // 15: White
            ],
            sauce: SauceMetaData::default(),
            autosave: super::autosave::AutosaveConfig::default(),
            // Default English UI strings
            ui_title: "icy_draw Collaboration Server".to_string(),
            ui_bind_address: "Bind Address".to_string(),
            ui_password: "Password".to_string(),
            ui_document: "Document".to_string(),
            ui_max_users: "Max Users".to_string(),
            ui_connect_with: "Connect with".to_string(),
            ui_stop_hint: "Press Ctrl+C to stop the server".to_string(),
            ui_none: "(none)".to_string(),
            ui_unlimited: "unlimited".to_string(),
        }
    }
}

/// Server state tracking connected clients.
#[derive(Debug)]
pub struct ServerState {
    /// The collaboration session
    pub session: SharedSession,
    /// Document data (column-major: blocks[col][row])
    pub(crate) document: RwLock<Vec<Vec<Block>>>,
    /// Connected client senders (for broadcasting)
    clients: RwLock<HashMap<UserId, mpsc::Sender<String>>>,
    /// Event broadcaster
    event_tx: broadcast::Sender<SessionEvent>,
    /// Server configuration
    pub(crate) config: ServerConfig,
}

impl ServerState {
    /// Create a new server state.
    pub fn new(config: ServerConfig) -> Arc<Self> {
        let session = Arc::new(Session::with_dimensions(config.password.clone(), config.columns, config.rows));

        // Apply initial settings
        session.set_ice_colors(config.ice_colors);
        session.set_use_9px(config.use_9px_font);
        session.set_font(config.font_name.clone());
        session.update_sauce(config.sauce.clone());

        // Initialize document from config or create empty
        let document = if let Some(initial_doc) = &config.initial_document {
            initial_doc.clone()
        } else {
            let mut doc = Vec::with_capacity(config.columns as usize);
            for _ in 0..config.columns {
                let column = vec![Block::default(); config.rows as usize];
                doc.push(column);
            }
            doc
        };

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
        let sauce = self.session.get_sauce();
        MoebiusDoc {
            columns,
            rows,
            data: None,
            compressed_data: Some(compressed),
            title: sauce.title,
            author: sauce.author,
            group: sauce.group,
            date: String::new(),
            palette: self.get_palette_json(),
            font_name: self.session.font(),
            ice_colors: self.session.get_ice_colors(),
            use_9px_font: self.session.get_use_9px(),
            comments: sauce.comments,
            c64_background: None,
        }
    }

    /// Convert the palette to JSON format for Moebius protocol.
    /// Returns an array of 16 {r, g, b} objects.
    fn get_palette_json(&self) -> serde_json::Value {
        let palette_array: Vec<serde_json::Value> = self
            .config
            .palette
            .iter()
            .map(|[r, g, b]| {
                serde_json::json!({
                    "r": *r,
                    "g": *g,
                    "b": *b
                })
            })
            .collect();
        serde_json::Value::Array(palette_array)
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
    ///
    /// Equivalent to Moebius `send_all_including_guests(ws, type, data)`.
    ///
    /// Used for: DRAW, SAUCE, ICE_COLORS, USE_9PX_FONT, CHANGE_FONT, SET_CANVAS_SIZE
    ///
    /// Web clients (guests with status=WEB) receive these messages so they can
    /// see real-time drawing updates even without being logged in.
    pub async fn broadcast(&self, message: &str, except: Option<UserId>) {
        let clients = self.clients.read().await;
        for (id, sender) in clients.iter() {
            if except != Some(*id) {
                let _ = sender.send(message.to_string()).await;
            }
        }
    }

    /// Broadcast a message to registered users only (not web clients).
    ///
    /// Equivalent to Moebius `send_all(ws, type, data)`.
    ///
    /// Used for: CHAT, JOIN, LEAVE
    ///
    /// Web clients (guests with status=WEB) do NOT receive these messages.
    /// Only users with a valid nickname (status != WEB) are included.
    pub async fn broadcast_to_registered(&self, message: &str, except: Option<UserId>) {
        let registered_ids = self.session.get_registered_user_ids();
        let clients = self.clients.read().await;
        for (id, sender) in clients.iter() {
            if except != Some(*id) && registered_ids.contains(id) {
                let _ = sender.send(message.to_string()).await;
            }
        }
    }

    /// Broadcast a message to all registered users including the sender.
    ///
    /// Equivalent to Moebius `send_all_including_self(type, data)`.
    ///
    /// Used for: STATUS
    ///
    /// The sender receives their own message back as confirmation.
    /// Web clients (guests with status=WEB) do NOT receive these messages.
    pub async fn broadcast_to_registered_including_self(&self, message: &str) {
        let registered_ids = self.session.get_registered_user_ids();
        let clients = self.clients.read().await;
        for (id, sender) in clients.iter() {
            if registered_ids.contains(id) {
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
    ///
    /// Moebius compatibility note:
    /// - Web viewers are identified by a missing `nick` field (server-side `is_web_client=true`).
    /// - Guests can have an empty nickname (""), but are still registered users.
    pub async fn handle_connect(&self, nick: String, password: String, is_web_client: bool) -> Result<(UserId, String), String> {
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

        // Get existing users BEFORE adding the new user (Moebius behavior)
        // This way the new user doesn't see themselves in the initial user list
        let existing_users = self.session.get_users();
        let chat_history = self.session.get_chat_history();

        // Add user
        let user_id = self.session.add_user(nick.clone());

        // Build connected response
        // Moebius sends different payloads for web clients vs regular clients.
        let response_json = if is_web_client {
            // Web client: minimal response (only {id, doc})
            serde_json::json!({
                "type": ActionCode::Connected as u8,
                "data": {
                    "id": user_id,
                    "doc": self.get_compressed_document().await
                }
            })
        } else {
            // Regular client: full response
            let response = ConnectedResponse {
                msg_type: ActionCode::Connected as u8,
                data: ConnectedData {
                    id: user_id,
                    doc: self.get_compressed_document().await,
                    users: existing_users,
                    chat_history,
                    status: 0, // ACTIVE status
                },
            };
            serde_json::to_value(&response).unwrap()
        };

        Ok((user_id, serde_json::to_string(&response_json).unwrap()))
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

        // Moebius forwards cursor updates via `send_all` (registered users only, excluding sender)
        // Wire format: {"type": 4, "data": {"id": <u32>, "x": <i32>, "y": <i32>}}
        #[derive(serde::Serialize)]
        struct CursorBroadcast {
            #[serde(rename = "type")]
            msg_type: u8,
            data: CursorBroadcastData,
        }
        #[derive(serde::Serialize)]
        struct CursorBroadcastData {
            id: u32,
            x: i32,
            y: i32,
        }

        let msg = CursorBroadcast {
            msg_type: ActionCode::Cursor as u8,
            data: CursorBroadcastData { id: user_id, x: col, y: row },
        };
        let json = serde_json::to_string(&msg).unwrap();
        self.broadcast_to_registered(&json, Some(user_id)).await;

        self.emit_event(SessionEvent::CursorMoved { id: user_id, col, row });
    }

    /// Handle a chat message.
    pub async fn handle_chat(&self, user_id: UserId, text: String) {
        let user = self.session.get_user(user_id);
        let nick = user.as_ref().map(|u| u.nick.clone()).unwrap_or_else(|| "Unknown".to_string());
        let group = user.map(|u| u.group).unwrap_or_default();

        self.session.add_chat_message(user_id, nick.clone(), text.clone());

        // Broadcast chat message to registered users only (not web clients)
        // Moebius uses send_all which excludes sender and web guests
        let msg = ChatBroadcastMessage {
            msg_type: ActionCode::Chat as u8,
            data: ChatMessage {
                id: user_id,
                nick: nick.clone(),
                text: text.clone(),
                group,
                time: 0,
            },
        };
        let json = serde_json::to_string(&msg).unwrap();
        self.broadcast_to_registered(&json, Some(user_id)).await;

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

/// Run the collaboration server.
///
/// This function starts the WebSocket server and handles incoming connections.
/// It will run until the shutdown signal is received or an error occurs.
pub async fn run_server(config: ServerConfig) -> Result<(), ServerError> {
    let state = ServerState::new(config.clone());

    let listener = TcpListener::bind(&config.bind_addr).await.map_err(|e| ServerError::BindFailed(e.to_string()))?;

    // Print server info
    let local_addr = listener.local_addr().map_err(|e| ServerError::ServerError(e.to_string()))?;

    use anstream::println;
    use colors::*;

    // Calculate field widths for proper alignment
    // Find the longest label to determine label_width
    let labels = [
        &config.ui_bind_address,
        &config.ui_password,
        &config.ui_document,
        &config.ui_max_users,
        &config.ui_connect_with,
    ];
    let label_width = labels.iter().map(|s| s.chars().count()).max().unwrap_or(13);

    // Calculate box dimensions based on content
    // Row format: "║  {label}{label_pad}:  {value}{value_pad} ║"
    // Title/hint format: "║  {text}{pad} ║"

    let title_len = config.ui_title.chars().count();
    let hint_len = config.ui_stop_hint.chars().count();

    // Calculate the longest possible row content
    let ws_url = format!("ws://{}", local_addr);
    let doc_size = format!("{}x{}", config.columns, config.rows);
    let max_users_display = if config.max_users > 0 {
        config.max_users.to_string()
    } else {
        config.ui_unlimited.clone()
    };
    let password_display = if config.password.is_empty() {
        config.ui_none.clone()
    } else {
        "********".to_string()
    };

    let values = [
        local_addr.to_string(),
        password_display.clone(),
        doc_size.clone(),
        max_users_display.clone(),
        ws_url.clone(),
    ];
    let max_value_len = values.iter().map(|s| s.chars().count()).max().unwrap_or(20);

    // Inner content width = label_width + ":" (1) + "  " (2) + max_value_len
    let labeled_row_width = label_width + 3 + max_value_len;
    let inner_width = title_len.max(hint_len).max(labeled_row_width);

    // Box content: "  " (2) + inner_width
    let box_width = inner_width + 2;
    let border = "═".repeat(box_width);

    println!("{CYAN}╔{border}╗{RESET}", CYAN = CYAN, border = border, RESET = RESET);

    // Title row
    let title_pad = box_width - config.ui_title.chars().count() - 2;
    println!(
        "{CYAN}║{RESET}  {YELLOW}{title}{RESET}{pad}{CYAN}║{RESET}",
        CYAN = CYAN,
        YELLOW = YELLOW,
        RESET = RESET,
        title = config.ui_title,
        pad = " ".repeat(title_pad)
    );

    println!("{CYAN}╠{border}╣{RESET}", CYAN = CYAN, border = border, RESET = RESET);

    // Print labeled rows with proper alignment
    let print_row = |label: &str, value: &str, color: &str| {
        let label_pad = label_width - label.chars().count();
        // Content: label + label_pad + ":" (1) + "  " (2) + value
        // Padding: box_width - 2 (prefix) - label - label_pad - 1 - 2 - value
        let content_len = label.chars().count() + label_pad + 3 + value.chars().count();
        let value_pad = box_width - 2 - content_len;
        println!(
            "{CYAN}║{RESET}  {color}{label}{label_spaces}:{RESET}  {value}{value_spaces}{CYAN}║{RESET}",
            CYAN = CYAN,
            RESET = RESET,
            color = color,
            label = label,
            label_spaces = " ".repeat(label_pad),
            value = value,
            value_spaces = " ".repeat(value_pad)
        );
    };

    print_row(&config.ui_bind_address, &local_addr.to_string(), GREEN);
    print_row(&config.ui_password, &password_display, GREEN);
    print_row(&config.ui_document, &doc_size, GREEN);
    print_row(&config.ui_max_users, &max_users_display, GREEN);

    println!("{CYAN}╠{border}╣{RESET}", CYAN = CYAN, border = border, RESET = RESET);
    print_row(&config.ui_connect_with, &ws_url, WHITE);
    println!("{CYAN}╠{border}╣{RESET}", CYAN = CYAN, border = border, RESET = RESET);

    // Hint row
    let hint_pad = box_width - config.ui_stop_hint.chars().count() - 2;
    println!(
        "{CYAN}║{RESET}  {GRAY}{hint}{RESET}{pad}{CYAN}║{RESET}",
        CYAN = CYAN,
        GRAY = GRAY,
        RESET = RESET,
        hint = config.ui_stop_hint,
        pad = " ".repeat(hint_pad)
    );

    println!("{CYAN}╚{border}╝{RESET}", CYAN = CYAN, border = border, RESET = RESET);
    println!();

    log::info!("Server listening on {}", local_addr);

    // Start autosave manager (always created, saves at least on shutdown)
    let autosave_manager = Arc::new(super::autosave::AutosaveManager::new(config.autosave.clone(), state.clone()));

    // Print autosave status
    if config.autosave.has_periodic_saves() {
        println!(
            "{GREEN}[Autosave]{RESET} Enabled, saving to {:?} every {:?}",
            config.autosave.backup_folder,
            config.autosave.interval.unwrap(),
            GREEN = GREEN,
            RESET = RESET
        );
    } else {
        println!(
            "{GREEN}[Autosave]{RESET} Saving to {:?} on shutdown",
            config.autosave.backup_folder,
            GREEN = GREEN,
            RESET = RESET
        );
    }

    // Start periodic autosave task if interval is configured
    let _autosave_handle = autosave_manager.clone().start();

    // Set up Ctrl+C handler for graceful shutdown
    let _shutdown_state = state.clone(); // Reserved for future use (e.g., notifying clients)
    let shutdown_autosave = autosave_manager.clone();

    tokio::spawn(async move {
        if let Ok(()) = tokio::signal::ctrl_c().await {
            println!();
            println!("{YELLOW}[Server]{RESET} Shutting down...", YELLOW = YELLOW, RESET = RESET);

            // Always save on shutdown
            let shutdown_path = shutdown_autosave.generate_filename();
            match shutdown_autosave.save_to(&shutdown_path).await {
                Ok(()) => {
                    println!("{GREEN}[Autosave]{RESET} Final save to {:?}", shutdown_path, GREEN = GREEN, RESET = RESET);
                }
                Err(e) => {
                    println!("{RED}[Autosave]{RESET} Failed to save on shutdown: {}", e, RED = RED, RESET = RESET);
                }
            }

            log::info!("Server shutdown complete");
            std::process::exit(0);
        }
    });

    loop {
        let (stream, addr) = listener.accept().await.map_err(|e| ServerError::ServerError(e.to_string()))?;

        let state = state.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(state, stream, addr).await {
                log::error!("[{}] Connection error: {}", addr, e);
            }
        });
    }
}

/// Handle a single WebSocket connection.
async fn handle_connection(state: Arc<ServerState>, stream: TcpStream, addr: SocketAddr) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use anstream::println;
    use colors::*;

    println!("{BLUE}[{addr}]{RESET} New connection", BLUE = BLUE, addr = addr, RESET = RESET);

    let ws_stream = tokio_tungstenite::accept_async(stream).await?;
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    // Channel for outgoing messages
    let (tx, mut rx) = mpsc::channel::<String>(256);

    // User ID will be assigned on CONNECT message
    let mut user_id: Option<UserId> = None;
    let mut user_nick = String::new();

    // Spawn task to forward messages from channel to WebSocket
    let sender_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    // Handle incoming messages
    while let Some(msg_result) = ws_receiver.next().await {
        let msg = match msg_result {
            Ok(m) => m,
            Err(e) => {
                log::warn!("[{}] WebSocket error: {}", addr, e);
                break;
            }
        };

        match msg {
            Message::Text(text) => {
                let text_str: &str = text.as_ref();
                if let Err(e) = handle_message(&state, &tx, &mut user_id, &mut user_nick, text_str, addr).await {
                    log::warn!("[{}] Message handling error: {}", addr, e);
                }
            }
            Message::Close(_) => {
                println!("{YELLOW}[{addr}]{RESET} Client requested close", YELLOW = YELLOW, addr = addr, RESET = RESET);
                break;
            }
            Message::Ping(_data) => {
                // Pong is handled automatically by tungstenite
            }
            _ => {}
        }
    }

    // Cleanup on disconnect
    if let Some(id) = user_id {
        state.unregister_client(id).await;

        // Broadcast leave message to registered users only (Moebius uses send_all)
        let leave_msg = LeaveMessage {
            msg_type: ActionCode::Leave as u8,
            data: LeaveData { id },
        };
        if let Ok(json) = serde_json::to_string(&leave_msg) {
            state.broadcast_to_registered(&json, Some(id)).await;
        }

        let user_count = state.client_count().await;
        println!(
            "{RED}[{addr}]{RESET} {BOLD}{nick}{RESET} left (users: {count})",
            RED = RED,
            addr = addr,
            RESET = RESET,
            BOLD = BOLD,
            nick = user_nick,
            count = user_count
        );

        state.emit_event(SessionEvent::UserLeft(id));
    } else {
        println!("{RED}[{addr}]{RESET} Disconnected (no login)", RED = RED, addr = addr, RESET = RESET);
    }

    sender_task.abort();
    Ok(())
}

/// Handle a single JSON message from a client.
pub async fn handle_message(
    state: &Arc<ServerState>,
    tx: &mpsc::Sender<String>,
    user_id: &mut Option<UserId>,
    user_nick: &mut String,
    text: &str,
    addr: SocketAddr,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use anstream::println;

    let json: serde_json::Value = serde_json::from_str(text)?;

    // Determine action from either "type" (Moebius) or "action" field
    let action_code = json.get("type").or_else(|| json.get("action")).and_then(|v| v.as_u64()).map(|v| v as u8);

    let data = json.get("data").cloned().unwrap_or(serde_json::Value::Null);

    match action_code {
        Some(0) => {
            // CONNECT request (client wants to join)
            // Extract nick and password from data
            // Moebius: nick == undefined means web client, nick == "" means guest
            let nick_opt = data.get("nick").and_then(|v| v.as_str());
            let is_web_client = nick_opt.is_none();
            let nick = nick_opt.unwrap_or("").to_string();
            let password = data.get("pass").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let group = data.get("group").and_then(|v| v.as_str()).unwrap_or("").to_string();

            match state.handle_connect(nick.clone(), password, is_web_client).await {
                Ok((id, response)) => {
                    *user_id = Some(id);
                    *user_nick = nick.clone();

                    // Update user with group if provided
                    if !group.is_empty() {
                        state.session.update_group(id, group.clone());
                    }

                    // Set status based on client type (Moebius: WEB=3 for web clients, ACTIVE=0 for regular)
                    let user_status = if is_web_client { 3 } else { 0 };
                    state.session.update_status(id, user_status);

                    state.register_client(id, tx.clone()).await;

                    // Send CONNECTED response
                    let _ = tx.send(response).await;

                    // Broadcast JOIN to other registered clients only (Moebius uses send_all)
                    let join_msg = JoinMessage {
                        msg_type: ActionCode::Join as u8,
                        data: JoinData {
                            id,
                            nick: nick.clone(),
                            group: group.clone(),
                            status: user_status,
                        },
                    };
                    if let Ok(json) = serde_json::to_string(&join_msg) {
                        state.broadcast_to_registered(&json, Some(id)).await;
                    }

                    let user_count = state.client_count().await;
                    let display_name = if is_web_client {
                        "web client"
                    } else if nick.is_empty() {
                        "Guest"
                    } else {
                        &nick
                    };
                    println!(
                        "{GREEN}[{addr}]{RESET} {BOLD}{name}{RESET} joined (users: {count})",
                        GREEN = colors::GREEN,
                        addr = addr,
                        RESET = colors::RESET,
                        BOLD = colors::BOLD,
                        name = display_name,
                        count = user_count
                    );

                    state.emit_event(SessionEvent::UserJoined(User {
                        id,
                        nick: nick.clone(),
                        group,
                        status: user_status,
                        col: 0,
                        row: 0,
                        selecting: false,
                        selection_col: 0,
                        selection_row: 0,
                    }));
                }
                Err(refuse_json) => {
                    let _ = tx.send(refuse_json).await;
                    println!("{RED}[{addr}]{RESET} Connection refused", RED = colors::RED, addr = addr, RESET = colors::RESET);
                }
            }
        }

        Some(4) => {
            // CURSOR
            if let Some(id) = *user_id {
                let col = data.get("x").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let row = data.get("y").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                state.handle_cursor(id, col, row).await;
            }
        }

        Some(5) => {
            // SELECTION
            if let Some(id) = *user_id {
                let col = data.get("x").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let row = data.get("y").and_then(|v| v.as_i64()).unwrap_or(0) as i32;

                // Moebius selection updates are forwarded with just {id, x, y}
                state.session.update_selection(id, true, col, row);

                #[derive(serde::Serialize)]
                struct SelectionBroadcast {
                    #[serde(rename = "type")]
                    msg_type: u8,
                    data: SelectionBroadcastData,
                }
                #[derive(serde::Serialize)]
                struct SelectionBroadcastData {
                    id: u32,
                    x: i32,
                    y: i32,
                }
                let msg = SelectionBroadcast {
                    msg_type: ActionCode::Selection as u8,
                    data: SelectionBroadcastData { id, x: col, y: row },
                };
                if let Ok(json) = serde_json::to_string(&msg) {
                    // Moebius default forwarding uses send_all (registered only, excluding sender)
                    state.broadcast_to_registered(&json, Some(id)).await;
                }
            }
        }

        Some(8) => {
            // HIDE_CURSOR
            if let Some(id) = *user_id {
                #[derive(serde::Serialize)]
                struct HideCursorBroadcast {
                    #[serde(rename = "type")]
                    msg_type: u8,
                    data: HideCursorData,
                }
                #[derive(serde::Serialize)]
                struct HideCursorData {
                    id: u32,
                }
                let msg = HideCursorBroadcast {
                    msg_type: ActionCode::HideCursor as u8,
                    data: HideCursorData { id },
                };
                if let Ok(json) = serde_json::to_string(&msg) {
                    // Moebius default forwarding uses send_all (registered only, excluding sender)
                    state.broadcast_to_registered(&json, Some(id)).await;
                }
            }
        }

        Some(9) => {
            // DRAW
            if let Some(id) = *user_id {
                let x = data.get("x").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let y = data.get("y").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let block_data = data.get("block").cloned().unwrap_or_default();
                let block: Block = serde_json::from_value(block_data).unwrap_or_default();
                let layer = data.get("layer").and_then(|v| v.as_u64()).map(|v| v as usize);

                let msg = DrawMessage {
                    msg_type: ActionCode::Draw as u8,
                    data: DrawData { id, x, y, block, layer },
                };
                state.handle_draw(id, msg).await;
            }
        }

        Some(10) => {
            // CHAT
            if let Some(id) = *user_id {
                let chat_text = data.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string();
                // Moebius also sends nick and group in chat messages
                let nick = data.get("nick").and_then(|v| v.as_str()).map(|s| s.to_string());
                let group = data.get("group").and_then(|v| v.as_str()).unwrap_or("").to_string();

                if !chat_text.is_empty() {
                    // Update nick/group if changed (Moebius behavior)
                    if let Some(ref new_nick) = nick {
                        if new_nick != user_nick {
                            *user_nick = new_nick.clone();
                        }
                    }
                    if !group.is_empty() {
                        state.session.update_group(id, group);
                    }

                    state.handle_chat(id, chat_text.clone()).await;
                    println!(
                        "{MAGENTA}[{addr}]{RESET} <{BOLD}{nick}{RESET}> {text}",
                        MAGENTA = colors::MAGENTA,
                        addr = addr,
                        RESET = colors::RESET,
                        BOLD = colors::BOLD,
                        nick = user_nick,
                        text = chat_text
                    );
                }
            }
        }

        Some(11) => {
            // STATUS - Moebius uses send_all_including_self (registered users including sender)
            if let Some(id) = *user_id {
                let status = data.get("status").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
                state.session.update_status(id, status);

                #[derive(serde::Serialize)]
                struct StatusBroadcast {
                    #[serde(rename = "type")]
                    msg_type: u8,
                    data: StatusBroadcastData,
                }
                #[derive(serde::Serialize)]
                struct StatusBroadcastData {
                    id: u32,
                    status: u8,
                }
                let msg = StatusBroadcast {
                    msg_type: ActionCode::Status as u8,
                    data: StatusBroadcastData { id, status },
                };
                if let Ok(json) = serde_json::to_string(&msg) {
                    state.broadcast_to_registered_including_self(&json).await;
                }
            }
        }

        Some(12) => {
            // SAUCE
            if let Some(id) = *user_id {
                let title = data.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let author = data.get("author").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let group = data.get("group").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let comments_str = data.get("comments").and_then(|v| v.as_str()).unwrap_or("");

                let meta = SauceMetaData {
                    title: title.clone().into(),
                    author: author.clone().into(),
                    group: group.clone().into(),
                    comments: if comments_str.is_empty() {
                        vec![]
                    } else {
                        comments_str.lines().map(|l| l.into()).collect()
                    },
                };
                state.session.update_sauce(meta);

                #[derive(serde::Serialize)]
                struct SauceBroadcast {
                    #[serde(rename = "type")]
                    msg_type: u8,
                    data: SauceBroadcastData,
                }
                #[derive(serde::Serialize)]
                struct SauceBroadcastData {
                    id: u32,
                    title: String,
                    author: String,
                    group: String,
                    comments: String,
                }
                let msg = SauceBroadcast {
                    msg_type: ActionCode::Sauce as u8,
                    data: SauceBroadcastData {
                        id,
                        title,
                        author,
                        group,
                        comments: comments_str.to_string(),
                    },
                };
                if let Ok(json) = serde_json::to_string(&msg) {
                    state.broadcast(&json, Some(id)).await;
                }
            }
        }

        Some(13) => {
            // ICE_COLORS
            if let Some(id) = *user_id {
                let ice_colors = data.get("value").and_then(|v| v.as_bool()).unwrap_or(false);
                state.session.set_ice_colors(ice_colors);

                #[derive(serde::Serialize)]
                struct IceColorsBroadcast {
                    #[serde(rename = "type")]
                    msg_type: u8,
                    data: IceColorsBroadcastData,
                }
                #[derive(serde::Serialize)]
                struct IceColorsBroadcastData {
                    id: u32,
                    value: bool,
                }
                let msg = IceColorsBroadcast {
                    msg_type: ActionCode::IceColors as u8,
                    data: IceColorsBroadcastData { id, value: ice_colors },
                };
                if let Ok(json) = serde_json::to_string(&msg) {
                    state.broadcast(&json, Some(id)).await;
                }
            }
        }

        Some(14) => {
            // USE_9PX_FONT
            if let Some(id) = *user_id {
                let use_9px = data.get("value").and_then(|v| v.as_bool()).unwrap_or(false);
                state.session.set_use_9px(use_9px);

                #[derive(serde::Serialize)]
                struct Use9pxBroadcast {
                    #[serde(rename = "type")]
                    msg_type: u8,
                    data: Use9pxBroadcastData,
                }
                #[derive(serde::Serialize)]
                struct Use9pxBroadcastData {
                    id: u32,
                    value: bool,
                }
                let msg = Use9pxBroadcast {
                    msg_type: ActionCode::Use9pxFont as u8,
                    data: Use9pxBroadcastData { id, value: use_9px },
                };
                if let Ok(json) = serde_json::to_string(&msg) {
                    state.broadcast(&json, Some(id)).await;
                }
            }
        }

        Some(15) => {
            // CHANGE_FONT
            if let Some(id) = *user_id {
                // Moebius uses `font_name`; accept legacy `value` too.
                let font_name = data
                    .get("font_name")
                    .and_then(|v| v.as_str())
                    .or_else(|| data.get("value").and_then(|v| v.as_str()))
                    .unwrap_or("")
                    .to_string();
                state.session.set_font(font_name.clone());

                #[derive(serde::Serialize)]
                struct ChangeFontBroadcast {
                    #[serde(rename = "type")]
                    msg_type: u8,
                    data: ChangeFontBroadcastData,
                }
                #[derive(serde::Serialize)]
                struct ChangeFontBroadcastData {
                    id: u32,
                    font_name: String,
                }
                let msg = ChangeFontBroadcast {
                    msg_type: ActionCode::ChangeFont as u8,
                    data: ChangeFontBroadcastData { id, font_name },
                };
                if let Ok(json) = serde_json::to_string(&msg) {
                    state.broadcast(&json, Some(id)).await;
                }
            }
        }

        Some(16) => {
            // SET_CANVAS_SIZE
            if let Some(id) = *user_id {
                let columns = data.get("columns").and_then(|v| v.as_u64()).unwrap_or(80) as u32;
                let rows = data.get("rows").and_then(|v| v.as_u64()).unwrap_or(25) as u32;

                state.resize(columns, rows).await;

                // Broadcast to all including guests, except sender (Moebius behavior)
                #[derive(serde::Serialize)]
                struct CanvasSizeBroadcast {
                    #[serde(rename = "type")]
                    msg_type: u8,
                    data: CanvasSizeData,
                }
                #[derive(serde::Serialize)]
                struct CanvasSizeData {
                    columns: u32,
                    rows: u32,
                }
                let msg = CanvasSizeBroadcast {
                    msg_type: ActionCode::SetCanvasSize as u8,
                    data: CanvasSizeData { columns, rows },
                };
                if let Ok(json) = serde_json::to_string(&msg) {
                    state.broadcast(&json, Some(id)).await;
                }

                println!(
                    "{YELLOW}[{addr}]{RESET} Canvas resized to {cols}x{rows}",
                    YELLOW = colors::YELLOW,
                    addr = addr,
                    RESET = colors::RESET,
                    cols = columns,
                    rows = rows
                );
            }
        }

        _ => {
            // Moebius behavior: forward unhandled actions via `send_all` (registered users only, excluding sender).
            // Exception: SET_BG is ignored in Moebius.
            if let Some(code) = action_code {
                if code == ActionCode::SetBackground as u8 {
                    return Ok(());
                }

                if let Some(id) = *user_id {
                    state.broadcast_to_registered(text, Some(id)).await;
                }
            }
        }
    }

    Ok(())
}
