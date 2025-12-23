//! WebSocket client for connecting to Moebius-compatible collaboration servers.
//!
//! This module provides a Tokio-based WebSocket client that can connect to
//! both Moebius servers and icy_draw servers.

use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::{mpsc, RwLock};
use tokio::time::MissedTickBehavior;
use tokio::time::{Duration, Instant};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use super::protocol::*;
use super::session::UserId;
use super::user_status;

fn collab_dbg(prefix: &str, color_ansi: &str, payload: &str) {
    // `anstream` translates ANSI sequences on Windows when needed.
    anstream::eprintln!("{color}{prefix}\x1b[0m {payload}", color = color_ansi, prefix = prefix, payload = payload);
}

/// Error type for client operations.
#[derive(Debug, Clone)]
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
    /// User group (optional, for display in chat)
    pub group: String,
    /// Session password
    pub password: String,
    /// Ping interval in seconds (0 to disable)
    pub ping_interval_secs: u64,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            nick: "Anonymous".to_string(),
            group: String::new(),
            password: String::new(),
            ping_interval_secs: 30,
        }
    }
}

/// Commands that can be sent to the client task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientCommand {
    /// Disconnect from server
    Disconnect,
    /// Send cursor position
    Cursor { col: i32, row: i32 },
    /// Send selection update
    Selection { selecting: bool, col: i32, row: i32 },
    /// Send operation mode (floating selection) position
    Operation { col: i32, row: i32 },
    /// Hide cursor (when switching to non-editing tools)
    HideCursor,
    /// Draw a character
    Draw { col: i32, row: i32, block: Block },
    /// Draw preview (temporary)
    DrawPreview { col: i32, row: i32, block: Block },
    /// Send chat message
    Chat { text: String },
    /// Resize columns
    ResizeColumns { columns: u32 },
    /// Resize rows
    ResizeRows { rows: u32 },
    /// Set canvas size (columns and rows together)
    SetCanvasSize { columns: u32, rows: u32 },
    /// Set 9px mode
    SetUse9px { value: bool },
    /// Set ice colors
    SetIceColors { value: bool },
    /// Set font
    SetFont { font: String },
    /// Set user status (Active=0, Idle=1, Away=2, Web=3)
    SetStatus { status: u8 },
    /// Set SAUCE metadata
    SetSauce {
        title: String,
        author: String,
        group: String,
        comments: String,
    },
    /// Set background color
    SetBackground { value: u32 },
    /// Rotate layer/selection
    Rotate,
    /// Flip horizontally
    FlipX,
    /// Flip vertically
    FlipY,
    /// Paste a block rectangle as a floating selection (Moebius PASTE_AS_SELECTION=17)
    PasteAsSelection { blocks: Blocks },
    /// Ping
    Ping,
}

/// Events received from the server.
#[derive(Debug, Clone)]
pub enum CollaborationEvent {
    /// Successfully connected to server
    Connected(Box<ConnectedDocument>),
    /// Connection refused (wrong password)
    Refused { reason: String },
    /// A user joined
    UserJoined(User),
    /// A user left
    UserLeft { user_id: UserId, nick: String },
    /// Cursor position updated
    CursorMoved { user_id: UserId, col: i32, row: i32 },
    /// Selection updated
    SelectionChanged { user_id: UserId, selecting: bool, col: i32, row: i32 },
    /// Operation mode started (floating selection)
    OperationStarted { user_id: UserId, col: i32, row: i32 },
    /// Cursor hidden (user switched to non-editing tool)
    CursorHidden { user_id: UserId },
    /// Character drawn
    Draw { col: i32, row: i32, block: Block },
    /// Preview character drawn
    DrawPreview { col: i32, row: i32, block: Block },
    /// Chat message received
    Chat(ChatMessage),
    /// Server status updated
    StatusChanged(ServerStatus),
    /// SAUCE metadata changed
    SauceChanged(SauceData),
    /// Canvas resized
    CanvasResized { user_id: UserId, columns: u32, rows: u32 },
    /// 9px mode changed
    Use9pxChanged { user_id: UserId, value: bool },
    /// Ice colors changed
    IceColorsChanged { user_id: UserId, value: bool },
    /// Font changed
    FontChanged { user_id: UserId, font_name: String },
    /// Paste-as-selection received (Moebius PASTE_AS_SELECTION=17)
    PasteAsSelection { user_id: UserId, blocks: Blocks },
    /// Rotate received (Moebius ROTATE=18)
    Rotate { user_id: UserId },
    /// Flip X received (Moebius FLIP_X=19)
    FlipX { user_id: UserId },
    /// Flip Y received (Moebius FLIP_Y=20)
    FlipY { user_id: UserId },
    /// Background color changed (Moebius SET_BG=21)
    BackgroundChanged { user_id: UserId, value: u32 },
    /// Connection lost
    Disconnected,
    /// Error occurred
    Error(ClientError),
}

/// Handle for interacting with the collaboration client.
///
/// This is the main interface for sending commands to the client task.
#[derive(Clone)]
pub struct ClientHandle {
    command_tx: mpsc::Sender<ClientCommand>,
    state: Arc<RwLock<ConnectionState>>,
    user_id: Arc<RwLock<Option<UserId>>>,
    nick: String,
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

    /// Get the nickname.
    pub fn nick(&self) -> &str {
        &self.nick
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

    /// Send operation mode position (floating selection).
    pub async fn send_operation(&self, col: i32, row: i32) -> Result<(), ClientError> {
        self.command_tx
            .send(ClientCommand::Operation { col, row })
            .await
            .map_err(|e| ClientError::SendFailed(e.to_string()))
    }

    /// Hide cursor (when switching to non-editing tools).
    pub async fn send_hide_cursor(&self) -> Result<(), ClientError> {
        self.command_tx
            .send(ClientCommand::HideCursor)
            .await
            .map_err(|e| ClientError::SendFailed(e.to_string()))
    }

    /// Set user status (Active=0, Idle=1, Away=2, Web=3).
    pub async fn send_status(&self, status: u8) -> Result<(), ClientError> {
        self.command_tx
            .send(ClientCommand::SetStatus { status })
            .await
            .map_err(|e| ClientError::SendFailed(e.to_string()))
    }

    /// Set SAUCE metadata.
    pub async fn send_sauce(&self, title: String, author: String, group: String, comments: String) -> Result<(), ClientError> {
        self.command_tx
            .send(ClientCommand::SetSauce {
                title,
                author,
                group,
                comments,
            })
            .await
            .map_err(|e| ClientError::SendFailed(e.to_string()))
    }

    /// Draw a character at the given position.
    pub async fn draw(&self, col: i32, row: i32, block: Block) -> Result<(), ClientError> {
        self.command_tx
            .send(ClientCommand::Draw { col, row, block })
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

    /// Set canvas size (columns and rows).
    pub async fn set_canvas_size(&self, columns: u32, rows: u32) -> Result<(), ClientError> {
        self.command_tx
            .send(ClientCommand::SetCanvasSize { columns, rows })
            .await
            .map_err(|e| ClientError::SendFailed(e.to_string()))
    }

    /// Set background color.
    pub async fn set_background(&self, value: u32) -> Result<(), ClientError> {
        self.command_tx
            .send(ClientCommand::SetBackground { value })
            .await
            .map_err(|e| ClientError::SendFailed(e.to_string()))
    }

    /// Rotate layer/selection.
    pub async fn rotate(&self) -> Result<(), ClientError> {
        self.command_tx
            .send(ClientCommand::Rotate)
            .await
            .map_err(|e| ClientError::SendFailed(e.to_string()))
    }

    /// Flip horizontally.
    pub async fn flip_x(&self) -> Result<(), ClientError> {
        self.command_tx
            .send(ClientCommand::FlipX)
            .await
            .map_err(|e| ClientError::SendFailed(e.to_string()))
    }

    /// Flip vertically.
    pub async fn flip_y(&self) -> Result<(), ClientError> {
        self.command_tx
            .send(ClientCommand::FlipY)
            .await
            .map_err(|e| ClientError::SendFailed(e.to_string()))
    }

    /// Paste blocks as a floating selection (Moebius PASTE_AS_SELECTION=17).
    pub async fn paste_as_selection(&self, blocks: Blocks) -> Result<(), ClientError> {
        self.command_tx
            .send(ClientCommand::PasteAsSelection { blocks })
            .await
            .map_err(|e| ClientError::SendFailed(e.to_string()))
    }

    /// Send ping (keepalive).
    pub async fn ping(&self) -> Result<(), ClientError> {
        self.command_tx
            .send(ClientCommand::Ping)
            .await
            .map_err(|e| ClientError::SendFailed(e.to_string()))
    }

    /// Send a generic command.
    /// This is useful for sending commands collected from UndoOp mappings.
    pub async fn send_command(&self, cmd: ClientCommand) -> Result<(), ClientError> {
        self.command_tx.send(cmd).await.map_err(|e| ClientError::SendFailed(e.to_string()))
    }
}

/// Start the collaboration client and connect to a server.
///
/// Returns a handle for sending commands and a receiver for events.
pub async fn connect(config: ClientConfig) -> Result<(ClientHandle, mpsc::Receiver<CollaborationEvent>), ClientError> {
    let (command_tx, command_rx) = mpsc::channel(256);
    let (event_tx, event_rx) = mpsc::channel(256);

    let state = Arc::new(RwLock::new(ConnectionState::Connecting));
    let user_id = Arc::new(RwLock::new(None));

    let handle = ClientHandle {
        command_tx,
        state: state.clone(),
        user_id: user_id.clone(),
        nick: config.nick.clone(),
    };

    // Spawn the client task
    tokio::spawn(run_client(config, command_rx, event_tx, state, user_id));

    Ok((handle, event_rx))
}

/// Main client task that handles WebSocket communication.
async fn run_client(
    config: ClientConfig,
    mut command_rx: mpsc::Receiver<ClientCommand>,
    event_tx: mpsc::Sender<CollaborationEvent>,
    state: Arc<RwLock<ConnectionState>>,
    user_id_storage: Arc<RwLock<Option<UserId>>>,
) {
    let nick = config.nick.clone();
    let group = config.group.clone();
    let password = config.password.clone();

    // Parse URL - Moebius format: host:port/path or just host:port
    // Default port is 8000 (Moebius standard)
    let url = if config.url.starts_with("ws://") || config.url.starts_with("wss://") {
        config.url.clone()
    } else {
        // Check if port is specified
        let has_port = config.url.split('/').next().map_or(false, |host_part| {
            host_part.contains(':') && host_part.split(':').last().map_or(false, |p| p.parse::<u16>().is_ok())
        });
        if has_port {
            format!("ws://{}", config.url)
        } else {
            // Insert default port 8000 before any path
            if let Some(slash_pos) = config.url.find('/') {
                format!("ws://{}:8000{}", &config.url[..slash_pos], &config.url[slash_pos..])
            } else {
                format!("ws://{}:8000", config.url)
            }
        }
    };

    log::info!("Connecting to collaboration server: {}", url);

    // Connect to WebSocket
    let ws_stream = match connect_async(&url).await {
        Ok((stream, _)) => stream,
        Err(e) => {
            let error = ClientError::ConnectionFailed(e.to_string());
            *state.write().await = ConnectionState::Failed(e.to_string());
            let _ = event_tx.send(CollaborationEvent::Error(error)).await;
            return;
        }
    };

    let (mut write, mut read) = ws_stream.split();

    // Send connect message with password
    // Moebius protocol: Client sends CONNECTED (0) to initiate, server responds with CONNECTED (0)
    let connect_msg = json!({
        "type": ActionCode::Connected as u8,
        "data": {
            "nick": nick.clone(),
            "group": group.clone(),
            "pass": password.clone(),
        }
    });

    if let Err(e) = write.send(Message::Text(connect_msg.to_string().into())).await {
        let error = ClientError::WebSocketError(e.to_string());
        *state.write().await = ConnectionState::Failed(e.to_string());
        let _ = event_tx.send(CollaborationEvent::Error(error)).await;
        return;
    }

    let mut assigned_user_id: Option<UserId> = None;

    // Moebius-like away timers: ACTIVE immediately on activity, then IDLE after 60s, AWAY after 5min.
    // Important: sending STATUS itself must NOT count as activity, otherwise we'd bounce ACTIVE<->IDLE.
    let mut last_activity: Option<Instant> = None;
    let mut current_status: Option<u8> = None;
    let mut status_tick = tokio::time::interval(Duration::from_secs(1));
    status_tick.set_missed_tick_behavior(MissedTickBehavior::Delay);

    // Check if debug logging is enabled via environment variable
    let debug_messages = std::env::var("ICY_COLLAB_DEBUG").is_ok();

    // Main event loop
    loop {
        tokio::select! {
            // Presence timer handling
            _ = status_tick.tick() => {
                const IDLE_AFTER_SECS: u64 = 60;
                const AWAY_AFTER_SECS: u64 = 5 * 60;

                let Some(last) = last_activity else { continue; };
                let elapsed = last.elapsed();
                let desired_status = if elapsed >= Duration::from_secs(AWAY_AFTER_SECS) {
                    user_status::AWAY
                } else if elapsed >= Duration::from_secs(IDLE_AFTER_SECS) {
                    user_status::IDLE
                } else {
                    user_status::ACTIVE
                };

                if current_status != Some(desired_status) {
                    if let Some(msg) = command_to_message(ClientCommand::SetStatus { status: desired_status }, assigned_user_id, &nick, &group) {
                        if debug_messages {
                            if let Ok(parsed) = serde_json::from_str::<Value>(&msg) {
                                if let Ok(formatted) = serde_json::to_string_pretty(&parsed) {
                                    collab_dbg("[COLLAB TX]", "\x1b[36m", &formatted);
                                } else {
                                    collab_dbg("[COLLAB TX]", "\x1b[36m", &msg);
                                }
                            } else {
                                collab_dbg("[COLLAB TX]", "\x1b[36m", &msg);
                            }
                        }

                        if let Err(e) = write.send(Message::Text(msg.into())).await {
                            log::error!("Failed to send status message: {}", e);
                        } else {
                            current_status = Some(desired_status);
                        }
                    }
                }
            }

            // Handle incoming WebSocket messages
            msg = read.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(parsed) = serde_json::from_str::<Value>(&text) {
                            // Log raw JSON if debug mode is enabled
                            if debug_messages {
                                if let Ok(formatted) = serde_json::to_string_pretty(&parsed) {
                                    collab_dbg("[COLLAB RX]", "\x1b[32m", &formatted);
                                } else {
                                    collab_dbg("[COLLAB RX]", "\x1b[32m", &text);
                                }
                            }

                            if let Some(event) = parse_server_message(&parsed, &nick, &mut assigned_user_id).await {
                                // Update state on connected
                                if let CollaborationEvent::Connected(ref doc) = event {
                                    *state.write().await = ConnectionState::Connected;
                                    *user_id_storage.write().await = Some(doc.user_id);
                                }
                                if matches!(&event, CollaborationEvent::Refused { .. }) {
                                    *state.write().await = ConnectionState::Failed("Authentication failed".to_string());
                                }
                                let _ = event_tx.send(event).await;
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        *state.write().await = ConnectionState::Disconnected;
                        let _ = event_tx.send(CollaborationEvent::Disconnected).await;
                        break;
                    }
                    Some(Err(e)) => {
                        let error = ClientError::WebSocketError(e.to_string());
                        *state.write().await = ConnectionState::Failed(e.to_string());
                        let _ = event_tx.send(CollaborationEvent::Error(error)).await;
                        break;
                    }
                    _ => {}
                }
            }

            // Handle commands from the application
            cmd = command_rx.recv() => {
                match cmd {
                    Some(ClientCommand::Disconnect) => {
                        let _ = write.close().await;
                        *state.write().await = ConnectionState::Disconnected;
                        let _ = event_tx.send(CollaborationEvent::Disconnected).await;
                        break;
                    }
                    Some(cmd) => {
                        // Track activity like Moebius: any user action (except CONNECTED/STATUS/PING) resets timers.
                        let is_activity = !matches!(
                            cmd,
                            ClientCommand::Disconnect | ClientCommand::Ping | ClientCommand::SetStatus { .. }
                        );

                        // If user explicitly sets status, track it as current (but don't treat as activity).
                        if let ClientCommand::SetStatus { status } = cmd {
                            current_status = Some(status);
                        }

                        if let Some(msg) = command_to_message(cmd, assigned_user_id, &nick, &group) {
                            // Log outgoing message if debug mode is enabled
                            if debug_messages {
                                if let Ok(parsed) = serde_json::from_str::<Value>(&msg) {
                                    if let Ok(formatted) = serde_json::to_string_pretty(&parsed) {
                                        collab_dbg("[COLLAB TX]", "\x1b[36m", &formatted);
                                    } else {
                                        collab_dbg("[COLLAB TX]", "\x1b[36m", &msg);
                                    }
                                } else {
                                    collab_dbg("[COLLAB TX]", "\x1b[36m", &msg);
                                }
                            }

                            if let Err(e) = write.send(Message::Text(msg.into())).await {
                                log::error!("Failed to send message: {}", e);
                            } else if is_activity {
                                last_activity = Some(Instant::now());

                                // Immediately set ACTIVE on activity (Moebius behavior)
                                if current_status != Some(user_status::ACTIVE) {
                                    if let Some(status_msg) = command_to_message(
                                        ClientCommand::SetStatus { status: user_status::ACTIVE },
                                        assigned_user_id,
                                        &nick,
                                        &group,
                                    ) {
                                        if debug_messages {
                                            if let Ok(parsed) = serde_json::from_str::<Value>(&status_msg) {
                                                if let Ok(formatted) = serde_json::to_string_pretty(&parsed) {
                                                    collab_dbg("[COLLAB TX]", "\x1b[36m", &formatted);
                                                } else {
                                                    collab_dbg("[COLLAB TX]", "\x1b[36m", &status_msg);
                                                }
                                            } else {
                                                collab_dbg("[COLLAB TX]", "\x1b[36m", &status_msg);
                                            }
                                        }

                                        if let Err(e) = write.send(Message::Text(status_msg.into())).await {
                                            log::error!("Failed to send active status: {}", e);
                                        } else {
                                            current_status = Some(user_status::ACTIVE);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    None => break,
                }
            }
        }
    }
}

/// Parse a server message and convert to CollaborationEvent.
#[doc(hidden)]
pub async fn parse_server_message(msg: &Value, _nick: &str, assigned_id: &mut Option<UserId>) -> Option<CollaborationEvent> {
    let msg_type = msg.get("type")?.as_u64()? as u8;

    let data = msg.get("data");

    match msg_type {
        0 => {
            // CONNECTED
            let resp: ConnectedResponse = serde_json::from_value(msg.clone()).ok()?;
            let user_id = resp.data.id as UserId;
            *assigned_id = Some(user_id);

            let mut connected = resp.data.doc.into_connected_document(user_id, resp.data.users).ok()?;
            connected.chat_history = resp.data.chat_history;

            Some(CollaborationEvent::Connected(Box::new(connected)))
        }
        1 => {
            // REFUSED
            Some(CollaborationEvent::Refused {
                reason: "Wrong password".to_string(),
            })
        }
        2 => {
            // JOIN
            let data = data?;
            let user = serde_json::from_value(data.clone()).ok()?;
            Some(CollaborationEvent::UserJoined(user))
        }
        3 => {
            // LEAVE
            let data = data?;
            let user_id = data.get("id")?.as_u64()? as UserId;
            Some(CollaborationEvent::UserLeft { user_id, nick: String::new() })
        }
        4 => {
            // CURSOR
            let data = data?;
            let user_id = data.get("id")?.as_u64()? as UserId;
            let col = data.get("x").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let row = data.get("y").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            Some(CollaborationEvent::CursorMoved { user_id, col, row })
        }
        5 => {
            // SELECTION
            let data = data?;
            let user_id = data.get("id")?.as_u64()? as UserId;
            let col = data.get("x").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let row = data.get("y").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            Some(CollaborationEvent::SelectionChanged {
                user_id,
                selecting: true,
                col,
                row,
            })
        }
        7 => {
            // OPERATION
            let data = data?;
            let user_id = data.get("id")?.as_u64()? as UserId;
            let col = data.get("x").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let row = data.get("y").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            Some(CollaborationEvent::OperationStarted { user_id, col, row })
        }
        8 => {
            // HIDE_CURSOR
            let data = data?;
            let user_id = data.get("id")?.as_u64()? as UserId;
            Some(CollaborationEvent::CursorHidden { user_id })
        }
        9 => {
            // DRAW
            let data = data?;
            let col = data.get("x")?.as_i64()? as i32;
            let row = data.get("y")?.as_i64()? as i32;
            let block_data = data.get("block")?;
            let block = Block {
                code: block_data.get("code").and_then(|v| v.as_u64()).unwrap_or(32) as u32,
                fg: block_data.get("fg").and_then(|v| v.as_u64()).unwrap_or(7) as u8,
                bg: block_data.get("bg").and_then(|v| v.as_u64()).unwrap_or(0) as u8,
            };
            Some(CollaborationEvent::Draw { col, row, block })
        }
        10 => {
            // CHAT
            let data = data?;
            let id = data.get("id").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let chat_nick = data.get("nick").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let text = data.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let group = data.get("group").and_then(|v| v.as_str()).unwrap_or("").to_string();
            // Use server timestamp if available, otherwise use current time
            let time = data.get("time").and_then(|v| v.as_u64()).filter(|&t| t > 0).unwrap_or_else(|| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0)
            });
            Some(CollaborationEvent::Chat(ChatMessage {
                id,
                nick: chat_nick,
                text,
                group,
                time,
            }))
        }
        11 => {
            // STATUS
            let data = data?;
            let status: ServerStatus = serde_json::from_value(data.clone()).ok()?;
            Some(CollaborationEvent::StatusChanged(status))
        }
        12 => {
            // SAUCE
            let data = data?;
            let sauce: SauceData = serde_json::from_value(data.clone()).ok()?;
            Some(CollaborationEvent::SauceChanged(sauce))
        }
        13 => {
            // ICE_COLORS
            let data = data?;
            let user_id = data.get("id").and_then(|v| v.as_u64()).unwrap_or(0) as UserId;
            let value = data.get("value")?.as_bool()?;
            Some(CollaborationEvent::IceColorsChanged { user_id, value })
        }
        14 => {
            // USE_9PX_FONT
            let data = data?;
            let user_id = data.get("id").and_then(|v| v.as_u64()).unwrap_or(0) as UserId;
            let value = data.get("value")?.as_bool()?;
            Some(CollaborationEvent::Use9pxChanged { user_id, value })
        }
        15 => {
            // CHANGE_FONT
            let data = data?;
            let user_id = data.get("id").and_then(|v| v.as_u64()).unwrap_or(0) as UserId;
            let font_name = data.get("font_name")?.as_str()?.to_string();
            Some(CollaborationEvent::FontChanged { user_id, font_name })
        }
        16 => {
            // SET_CANVAS_SIZE
            let data = data?;
            let user_id = data.get("id").and_then(|v| v.as_u64()).unwrap_or(0) as UserId;
            let columns = data.get("columns")?.as_u64()? as u32;
            let rows = data.get("rows")?.as_u64()? as u32;
            Some(CollaborationEvent::CanvasResized { user_id, columns, rows })
        }
        17 => {
            // PASTE_AS_SELECTION
            let data = data?;
            let user_id = data.get("id")?.as_u64()? as UserId;
            let blocks_val = data.get("blocks")?;
            let blocks: Blocks = serde_json::from_value(blocks_val.clone()).ok()?;
            Some(CollaborationEvent::PasteAsSelection { user_id, blocks })
        }
        18 => {
            // ROTATE
            let data = data?;
            let user_id = data.get("id")?.as_u64()? as UserId;
            Some(CollaborationEvent::Rotate { user_id })
        }
        19 => {
            // FLIP_X
            let data = data?;
            let user_id = data.get("id")?.as_u64()? as UserId;
            Some(CollaborationEvent::FlipX { user_id })
        }
        20 => {
            // FLIP_Y
            let data = data?;
            let user_id = data.get("id")?.as_u64()? as UserId;
            Some(CollaborationEvent::FlipY { user_id })
        }
        21 => {
            // SET_BG
            let data = data?;
            let user_id = data.get("id")?.as_u64()? as UserId;
            let value = data.get("value")?.as_u64()? as u32;
            Some(CollaborationEvent::BackgroundChanged { user_id, value })
        }
        _ => None,
    }
}

/// Convert a ClientCommand to a JSON message string.
/// Uses Moebius-compatible action codes.
#[doc(hidden)]
pub fn command_to_message(cmd: ClientCommand, user_id: Option<UserId>, nick: &str, group: &str) -> Option<String> {
    let id = user_id?;

    let msg = match cmd {
        ClientCommand::Cursor { col, row } => json!({
            "type": ActionCode::Cursor as u8,
            "data": { "id": id, "x": col, "y": row }
        }),
        ClientCommand::Selection { selecting, col, row } => {
            if selecting {
                json!({
                    "type": ActionCode::Selection as u8,
                    "data": { "id": id, "x": col, "y": row }
                })
            } else {
                json!({
                    "type": ActionCode::Cursor as u8,
                    "data": { "id": id, "x": col, "y": row }
                })
            }
        }
        ClientCommand::Operation { col, row } => json!({
            "type": ActionCode::Operation as u8,
            "data": { "id": id, "x": col, "y": row }
        }),
        ClientCommand::HideCursor => json!({
            "type": ActionCode::HideCursor as u8,
            "data": { "id": id }
        }),
        ClientCommand::Draw { col, row, block } => json!({
            "type": ActionCode::Draw as u8,  // Moebius DRAW = 9
            "data": {
                "id": id,
                "x": col,
                "y": row,
                "block": { "code": block.code, "fg": block.fg, "bg": block.bg }
            }
        }),
        // DrawPreview is not part of Moebius; do not send over the network.
        ClientCommand::DrawPreview { .. } => return None,
        ClientCommand::Chat { text } => json!({
            "type": ActionCode::Chat as u8,
            "data": { "id": id, "nick": nick, "group": group, "text": text }
        }),
        // Moebius SET_CANVAS_SIZE = 16 requires both columns and rows
        // For now, we don't support partial resize - skip these
        ClientCommand::ResizeColumns { columns: _ } => return None,
        ClientCommand::ResizeRows { rows: _ } => return None,
        ClientCommand::SetUse9px { value } => json!({
            "type": ActionCode::Use9pxFont as u8,  // Moebius USE_9PX_FONT = 14
            "data": { "id": id, "value": value }
        }),
        ClientCommand::SetIceColors { value } => json!({
            "type": ActionCode::IceColors as u8,  // Moebius ICE_COLORS = 13
            "data": { "id": id, "value": value }
        }),
        ClientCommand::SetFont { font } => json!({
            "type": ActionCode::ChangeFont as u8,  // Moebius CHANGE_FONT = 15
            "data": { "id": id, "font_name": font }
        }),
        ClientCommand::SetStatus { status } => json!({
            "type": ActionCode::Status as u8,  // Moebius STATUS = 11
            "data": { "id": id, "status": status }
        }),
        ClientCommand::SetSauce {
            title,
            author,
            group,
            comments,
        } => json!({
            "type": ActionCode::Sauce as u8,  // Moebius SAUCE = 12
            "data": { "id": id, "title": title, "author": author, "group": group, "comments": comments }
        }),
        ClientCommand::SetCanvasSize { columns, rows } => json!({
            "type": ActionCode::SetCanvasSize as u8,  // Moebius SET_CANVAS_SIZE = 16
            "data": { "id": id, "columns": columns, "rows": rows }
        }),
        ClientCommand::SetBackground { value } => json!({
            "type": ActionCode::SetBackground as u8,  // Moebius SET_BG = 21
            "data": { "id": id, "value": value }
        }),
        ClientCommand::Rotate => json!({
            "type": ActionCode::Rotate as u8,  // Moebius ROTATE = 18
            "data": { "id": id }
        }),
        ClientCommand::FlipX => json!({
            "type": ActionCode::FlipX as u8,  // Moebius FLIP_X = 19
            "data": { "id": id }
        }),
        ClientCommand::FlipY => json!({
            "type": ActionCode::FlipY as u8,  // Moebius FLIP_Y = 20
            "data": { "id": id }
        }),
        ClientCommand::PasteAsSelection { blocks } => json!({
            "type": ActionCode::PasteAsSelection as u8, // Moebius PASTE_AS_SELECTION = 17
            "data": { "id": id, "blocks": blocks }
        }),
        // Ping not in Moebius protocol - skip
        ClientCommand::Ping => return None,
        ClientCommand::Disconnect => return None,
    };

    Some(msg.to_string())
}
