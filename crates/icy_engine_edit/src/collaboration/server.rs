//! WebSocket server for hosting Moebius-compatible collaboration sessions.
//!
//! This module provides a Tokio-based WebSocket server that hosts collaboration
//! sessions compatible with both Moebius clients and icy_draw clients.

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

    println!("\x1b[1;36m╔═══════════════════════════════════════════════════════════════╗\x1b[0m");
    println!("\x1b[1;36m║\x1b[0m  \x1b[1;33micy_draw Collaboration Server\x1b[0m                              \x1b[1;36m║\x1b[0m");
    println!("\x1b[1;36m╠═══════════════════════════════════════════════════════════════╣\x1b[0m");
    println!("\x1b[1;36m║\x1b[0m  \x1b[1;32mBind Address:\x1b[0m  {:<44} \x1b[1;36m║\x1b[0m", local_addr);

    if !config.password.is_empty() {
        println!("\x1b[1;36m║\x1b[0m  \x1b[1;32mPassword:\x1b[0m      {:<44} \x1b[1;36m║\x1b[0m", "********");
    } else {
        println!("\x1b[1;36m║\x1b[0m  \x1b[1;32mPassword:\x1b[0m      {:<44} \x1b[1;36m║\x1b[0m", "(none)");
    }

    println!(
        "\x1b[1;36m║\x1b[0m  \x1b[1;32mDocument:\x1b[0m      {}x{} {:<34} \x1b[1;36m║\x1b[0m",
        config.columns, config.rows, ""
    );

    if config.max_users > 0 {
        println!(
            "\x1b[1;36m║\x1b[0m  \x1b[1;32mMax Users:\x1b[0m     {:<44} \x1b[1;36m║\x1b[0m",
            config.max_users
        );
    } else {
        println!("\x1b[1;36m║\x1b[0m  \x1b[1;32mMax Users:\x1b[0m     {:<44} \x1b[1;36m║\x1b[0m", "unlimited");
    }

    println!("\x1b[1;36m╠═══════════════════════════════════════════════════════════════╣\x1b[0m");
    println!("\x1b[1;36m║\x1b[0m  \x1b[1;37mConnect with:\x1b[0m  ws://{:<39} \x1b[1;36m║\x1b[0m", local_addr);
    println!("\x1b[1;36m╠═══════════════════════════════════════════════════════════════╣\x1b[0m");
    println!("\x1b[1;36m║\x1b[0m  \x1b[1;90mPress Ctrl+C to stop the server\x1b[0m                            \x1b[1;36m║\x1b[0m");
    println!("\x1b[1;36m╚═══════════════════════════════════════════════════════════════╝\x1b[0m");
    println!();

    log::info!("Server listening on {}", local_addr);

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

    println!("\x1b[1;34m[{}]\x1b[0m New connection", addr);

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
                println!("\x1b[1;33m[{}]\x1b[0m Client requested close", addr);
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

        // Broadcast leave message
        let leave_msg = LeaveMessage {
            msg_type: ActionCode::Leave as u8,
            data: LeaveData { id },
        };
        if let Ok(json) = serde_json::to_string(&leave_msg) {
            state.broadcast(&json, Some(id)).await;
        }

        println!(
            "\x1b[1;31m[{}]\x1b[0m \x1b[1m{}\x1b[0m left (users: {})",
            addr,
            user_nick,
            state.client_count().await
        );

        state.emit_event(SessionEvent::UserLeft(id));
    } else {
        println!("\x1b[1;31m[{}]\x1b[0m Disconnected (no login)", addr);
    }

    sender_task.abort();
    Ok(())
}

/// Handle a single JSON message from a client.
async fn handle_message(
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
            let nick = data.get("nick").and_then(|v| v.as_str()).unwrap_or("Guest").to_string();
            let password = data.get("pass").and_then(|v| v.as_str()).unwrap_or("").to_string();

            match state.handle_connect(nick.clone(), password).await {
                Ok((id, response)) => {
                    *user_id = Some(id);
                    *user_nick = nick.clone();

                    state.register_client(id, tx.clone()).await;

                    // Send CONNECTED response
                    let _ = tx.send(response).await;

                    // Broadcast JOIN to other clients
                    let user = state.session.get_user(id);
                    let join_msg = JoinMessage {
                        msg_type: ActionCode::Join as u8,
                        data: JoinData {
                            id,
                            nick: user.as_ref().map(|u| u.nick.clone()).unwrap_or_default(),
                            group: user.as_ref().map(|u| u.group.clone()).unwrap_or_default(),
                            status: user.as_ref().map(|u| u.status).unwrap_or(0),
                        },
                    };
                    if let Ok(json) = serde_json::to_string(&join_msg) {
                        state.broadcast(&json, Some(id)).await;
                    }

                    println!(
                        "\x1b[1;32m[{}]\x1b[0m \x1b[1m{}\x1b[0m joined (users: {})",
                        addr,
                        nick,
                        state.client_count().await
                    );

                    state.emit_event(SessionEvent::UserJoined(user.unwrap_or(User {
                        id,
                        nick: nick.clone(),
                        group: String::new(),
                        status: 0,
                        col: 0,
                        row: 0,
                        selecting: false,
                        selection_col: 0,
                        selection_row: 0,
                    })));
                }
                Err(refuse_json) => {
                    let _ = tx.send(refuse_json).await;
                    println!("\x1b[1;31m[{}]\x1b[0m Connection refused", addr);
                }
            }
        }

        Some(4) => {
            // CURSOR
            if let Some(id) = *user_id {
                let col = data.get("col").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let row = data.get("row").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                state.handle_cursor(id, col, row).await;
            }
        }

        Some(5) => {
            // SELECTION
            if let Some(id) = *user_id {
                let selecting = data.get("selecting").and_then(|v| v.as_bool()).unwrap_or(false);
                let col = data.get("col").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let row = data.get("row").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let start_col = data.get("start_col").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let start_row = data.get("start_row").and_then(|v| v.as_i64()).unwrap_or(0) as i32;

                state.session.update_selection_with_start(id, selecting, col, row, start_col, start_row);

                // Broadcast to others
                #[derive(serde::Serialize)]
                struct SelectionBroadcast {
                    #[serde(rename = "type")]
                    msg_type: u8,
                    data: SelectionBroadcastData,
                }
                #[derive(serde::Serialize)]
                struct SelectionBroadcastData {
                    id: u32,
                    selecting: bool,
                    col: i32,
                    row: i32,
                    start_col: i32,
                    start_row: i32,
                }
                let msg = SelectionBroadcast {
                    msg_type: ActionCode::Selection as u8,
                    data: SelectionBroadcastData {
                        id,
                        selecting,
                        col,
                        row,
                        start_col,
                        start_row,
                    },
                };
                if let Ok(json) = serde_json::to_string(&msg) {
                    state.broadcast(&json, Some(id)).await;
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
                    state.broadcast(&json, Some(id)).await;
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
                if !chat_text.is_empty() {
                    state.handle_chat(id, chat_text.clone()).await;
                    println!("\x1b[1;35m[{}]\x1b[0m <\x1b[1m{}\x1b[0m> {}", addr, user_nick, chat_text);
                }
            }
        }

        Some(11) => {
            // STATUS
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
                    state.broadcast(&json, Some(id)).await;
                }
            }
        }

        Some(12) => {
            // SAUCE
            if let Some(id) = *user_id {
                let title = data.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let author = data.get("author").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let group = data.get("group").and_then(|v| v.as_str()).unwrap_or("").to_string();

                state.session.update_sauce(title.clone(), author.clone(), group.clone());

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
                }
                let msg = SauceBroadcast {
                    msg_type: ActionCode::Sauce as u8,
                    data: SauceBroadcastData { id, title, author, group },
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
                let font_name = data.get("value").and_then(|v| v.as_str()).unwrap_or("").to_string();
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
                    value: String,
                }
                let msg = ChangeFontBroadcast {
                    msg_type: ActionCode::ChangeFont as u8,
                    data: ChangeFontBroadcastData { id, value: font_name },
                };
                if let Ok(json) = serde_json::to_string(&msg) {
                    state.broadcast(&json, Some(id)).await;
                }
            }
        }

        Some(16) => {
            // SET_CANVAS_SIZE
            if let Some(_id) = *user_id {
                let columns = data.get("columns").and_then(|v| v.as_u64()).unwrap_or(80) as u32;
                let rows = data.get("rows").and_then(|v| v.as_u64()).unwrap_or(25) as u32;

                state.resize(columns, rows).await;

                // Broadcast to all (including sender for confirmation)
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
                    state.broadcast_all(&json).await;
                }

                println!("\x1b[1;33m[{}]\x1b[0m Canvas resized to {}x{}", addr, columns, rows);
            }
        }

        _ => {
            // Unknown or unhandled action - log but don't fail
            if let Some(code) = action_code {
                log::debug!("[{}] Unhandled action code: {}", addr, code);
            }
        }
    }

    Ok(())
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
