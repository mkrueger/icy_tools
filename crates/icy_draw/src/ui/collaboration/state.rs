//! Collaboration state management
//!
//! Manages the state of a collaboration session including:
//! - Connection status
//! - Remote users and cursors
//! - Chat history
//! - Document synchronization

use icy_engine_edit::collaboration::{ChatMessage, ServerStatus, User};
use std::collections::HashMap;

use super::subscription::CollaborationClient;

/// ID for a remote user
pub type UserId = u32;

/// User status constants
pub mod user_status {
    /// User is actively editing
    pub const ACTIVE: u8 = 0;
    /// User is idle (no activity for a while)
    pub const IDLE: u8 = 1;
    /// User is away
    pub const AWAY: u8 = 2;
    /// User is connected via web client
    pub const WEB: u8 = 3;
}

/// Cursor mode for remote users
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorMode {
    /// Normal editing mode - show cursor at position
    #[default]
    Editing,
    /// Selection mode - show selection rectangle
    Selection,
    /// Operation mode - moving a floating selection block
    Operation,
    /// Cursor is hidden (user switched to a non-editing tool)
    Hidden,
}

/// Remote user state
#[derive(Debug, Clone)]
pub struct RemoteUser {
    /// User information
    pub user: User,
    /// Current cursor position
    pub cursor: Option<(i32, i32)>,
    /// Current selection state
    pub selection: Option<SelectionState>,
    /// Current operation state (floating selection position)
    pub operation: Option<OperationState>,
    /// Current cursor mode
    pub cursor_mode: CursorMode,
    /// User status (Active=0, Idle=1, Away=2, Web=3)
    pub status: u8,
}

/// Selection state for a user
#[derive(Debug, Clone)]
pub struct SelectionState {
    pub selecting: bool,
    pub col: i32,
    pub row: i32,
}

/// Operation state for a user (floating selection)
#[derive(Debug, Clone)]
pub struct OperationState {
    pub col: i32,
    pub row: i32,
}

/// Collaboration session state
#[derive(Debug, Default)]
pub struct CollaborationState {
    /// Whether we are in collaboration mode
    pub active: bool,
    /// Active client connection (if connected)
    pub client: Option<CollaborationClient>,
    /// Connection info (server URL) for reconnection
    pub server_url: Option<String>,
    /// Our nickname
    pub nick: Option<String>,
    /// Session password
    pub password: Option<String>,
    /// Our user ID (assigned by server)
    pub our_user_id: Option<UserId>,
    /// Remote users (excluding ourselves)
    pub remote_users: HashMap<UserId, RemoteUser>,
    /// Chat history
    pub chat_messages: Vec<ChatMessage>,
    /// Server status
    pub server_status: Option<ServerStatus>,
    /// Whether the chat panel is visible
    pub chat_visible: bool,
    /// Current chat input text
    pub chat_input: String,
    /// Document columns (as reported by server)
    pub columns: u32,
    /// Document rows (as reported by server)
    pub rows: u32,
    /// 9px mode
    pub use_9px: bool,
    /// Ice colors
    pub ice_colors: bool,
    /// Font name
    pub font: String,
    /// Whether we are currently connecting
    pub connecting: bool,
}

impl CollaborationState {
    /// Create new collaboration state
    pub fn new() -> Self {
        Self::default()
    }

    /// Start connecting to a server
    pub fn start_connecting(&mut self, url: String, nick: String, password: String) {
        self.connecting = true;
        self.server_url = Some(url);
        self.nick = Some(nick);
        self.password = Some(password);
    }

    /// Called when connection is established
    pub fn on_connected(&mut self, client: CollaborationClient) {
        self.connecting = false;
        self.active = true;
        self.client = Some(client);
        self.chat_visible = true; // Auto-show chat on connect
    }

    /// Start a new collaboration session (when we receive session info from server)
    pub fn start_session(&mut self, user_id: UserId, columns: u32, rows: u32, use_9px: bool, ice_colors: bool, font: String) {
        self.active = true;
        self.our_user_id = Some(user_id);
        self.columns = columns;
        self.rows = rows;
        self.use_9px = use_9px;
        self.ice_colors = ice_colors;
        self.font = font;
        self.remote_users.clear();
        self.chat_messages.clear();
        self.server_status = None;
    }

    /// End the collaboration session
    pub fn end_session(&mut self) {
        self.active = false;
        self.connecting = false;
        self.our_user_id = None;
        self.client = None;
        self.remote_users.clear();
        // Keep chat messages for reference
    }

    /// Check if we are connected and have an active client
    pub fn is_connected(&self) -> bool {
        self.active && self.client.is_some()
    }

    /// Add a remote user
    pub fn add_user(&mut self, user: User) {
        // Don't add ourselves
        if Some(user.id) != self.our_user_id {
            let status = user.status;
            self.remote_users.insert(
                user.id,
                RemoteUser {
                    user,
                    cursor: None,
                    selection: None,
                    operation: None,
                    cursor_mode: CursorMode::Editing,
                    status,
                },
            );
        }
    }

    /// Remove a remote user
    pub fn remove_user(&mut self, user_id: UserId) {
        self.remote_users.remove(&user_id);
    }

    /// Update user cursor position
    pub fn update_cursor(&mut self, user_id: UserId, col: i32, row: i32) {
        if let Some(user) = self.remote_users.get_mut(&user_id) {
            user.cursor = Some((col, row));
            user.cursor_mode = CursorMode::Editing;
        }
    }

    /// Update user selection
    pub fn update_selection(&mut self, user_id: UserId, selecting: bool, col: i32, row: i32) {
        if let Some(user) = self.remote_users.get_mut(&user_id) {
            user.selection = Some(SelectionState { selecting, col, row });
            user.cursor_mode = CursorMode::Selection;
        }
    }

    /// Update user operation state (floating selection)
    pub fn update_operation(&mut self, user_id: UserId, col: i32, row: i32) {
        if let Some(user) = self.remote_users.get_mut(&user_id) {
            user.operation = Some(OperationState { col, row });
            user.cursor_mode = CursorMode::Operation;
        }
    }

    /// Hide user cursor
    pub fn hide_user_cursor(&mut self, user_id: UserId) {
        if let Some(user) = self.remote_users.get_mut(&user_id) {
            user.cursor_mode = CursorMode::Hidden;
        }
    }

    /// Update user status
    pub fn update_user_status(&mut self, user_id: UserId, status: u8) {
        if let Some(user) = self.remote_users.get_mut(&user_id) {
            user.status = status;
        }
    }

    /// Send our status to the server
    pub fn send_status(&mut self, status: u8) -> Option<iced::Task<()>> {
        let client = self.client.as_ref()?;
        let handle = client.handle().clone();

        Some(iced::Task::future(async move {
            let _ = handle.send_status(status).await;
        }))
    }

    /// Add a chat message
    pub fn add_chat_message(&mut self, message: ChatMessage) {
        self.chat_messages.push(message);
    }

    /// Add a system message (e.g., for notifications)
    pub fn add_system_message(&mut self, text: &str) {
        self.chat_messages.push(ChatMessage {
            id: 0,
            nick: String::new(),
            group: String::new(),
            text: text.to_string(),
            time: 0,
        });
    }

    /// Update server status
    pub fn update_server_status(&mut self, status: ServerStatus) {
        self.server_status = Some(status);
    }

    /// Update canvas size
    pub fn update_canvas_size(&mut self, columns: u32, rows: u32) {
        self.columns = columns;
        self.rows = rows;
    }

    /// Toggle chat visibility
    pub fn toggle_chat(&mut self) {
        self.chat_visible = !self.chat_visible;
    }

    /// Send a chat message (returns a Task that should be spawned)
    pub fn send_chat(&mut self, text: String) -> Option<iced::Task<()>> {
        let client = self.client.as_ref()?;
        let handle = client.handle().clone();

        // Server broadcasts CHAT to everyone *except* the sender.
        // Therefore, the UI must show our own messages locally on send.
        if let (Some(id), Some(nick)) = (self.our_user_id, self.nick.as_ref()) {
            self.chat_messages.push(ChatMessage {
                id,
                nick: nick.clone(),
                group: String::new(),
                text: text.clone(),
                time: 0,
            });
        }

        Some(iced::Task::future(async move {
            let _ = handle.send_chat(text).await;
        }))
    }

    /// Send cursor position (returns a Task that should be spawned)
    pub fn send_cursor(&self, col: i32, row: i32) -> Option<iced::Task<()>> {
        if let Some(client) = &self.client {
            let handle = client.handle().clone();
            Some(iced::Task::future(async move {
                let _ = handle.send_cursor(col, row).await;
            }))
        } else {
            None
        }
    }

    /// Send selection update (returns a Task that should be spawned)
    pub fn send_selection(&self, selecting: bool, col: i32, row: i32) -> Option<iced::Task<()>> {
        if let Some(client) = &self.client {
            let handle = client.handle().clone();
            Some(iced::Task::future(async move {
                let _ = handle.send_selection(selecting, col, row).await;
            }))
        } else {
            None
        }
    }

    /// Send operation mode position (floating selection) (returns a Task that should be spawned)
    pub fn send_operation(&self, col: i32, row: i32) -> Option<iced::Task<()>> {
        if let Some(client) = &self.client {
            let handle = client.handle().clone();
            Some(iced::Task::future(async move {
                let _ = handle.send_operation(col, row).await;
            }))
        } else {
            None
        }
    }

    /// Send hide cursor command (returns a Task that should be spawned)
    pub fn send_hide_cursor(&self) -> Option<iced::Task<()>> {
        if let Some(client) = &self.client {
            let handle = client.handle().clone();
            Some(iced::Task::future(async move {
                let _ = handle.send_hide_cursor().await;
            }))
        } else {
            None
        }
    }

    /// Send a draw operation (returns a Task that should be spawned)
    pub fn send_draw(&self, col: i32, row: i32, block: icy_engine_edit::collaboration::Block) -> Option<iced::Task<()>> {
        if let Some(client) = &self.client {
            let handle = client.handle().clone();
            Some(iced::Task::future(async move {
                let _ = handle.draw(col, row, block).await;
            }))
        } else {
            None
        }
    }

    /// Send SAUCE metadata update (returns a Task that should be spawned)
    pub fn send_sauce(
        &self,
        title: String,
        author: String,
        group: String,
        comments: String,
    ) -> Option<iced::Task<()>> {
        if let Some(client) = &self.client {
            let handle = client.handle().clone();
            Some(iced::Task::future(async move {
                let _ = handle.send_sauce(title, author, group, comments).await;
            }))
        } else {
            None
        }
    }

    /// Get sorted list of remote users
    pub fn sorted_users(&self) -> Vec<&RemoteUser> {
        let mut users: Vec<_> = self.remote_users.values().collect();
        users.sort_by(|a, b| a.user.nick.cmp(&b.user.nick));
        users
    }

    /// Get user by ID
    pub fn get_user(&self, user_id: UserId) -> Option<&RemoteUser> {
        self.remote_users.get(&user_id)
    }

    /// Get user's color for rendering cursor (generated from user ID)
    pub fn user_color(&self, user_id: UserId) -> (u8, u8, u8) {
        // Generate a color from user ID
        // Use HSL with fixed saturation and lightness, vary hue by user ID
        let hue = ((user_id * 137) % 360) as f32; // Golden angle for good distribution
        let saturation: f32 = 0.7;
        let lightness: f32 = 0.5;

        // Convert HSL to RGB
        let c = (1.0_f32 - (2.0_f32 * lightness - 1.0_f32).abs()) * saturation;
        let x = c * (1.0_f32 - ((hue / 60.0_f32) % 2.0_f32 - 1.0_f32).abs());
        let m = lightness - c / 2.0_f32;

        let (r, g, b) = if hue < 60.0 {
            (c, x, 0.0_f32)
        } else if hue < 120.0 {
            (x, c, 0.0_f32)
        } else if hue < 180.0 {
            (0.0_f32, c, x)
        } else if hue < 240.0 {
            (0.0_f32, x, c)
        } else if hue < 300.0 {
            (x, 0.0_f32, c)
        } else {
            (c, 0.0_f32, x)
        };

        (((r + m) * 255.0_f32) as u8, ((g + m) * 255.0_f32) as u8, ((b + m) * 255.0_f32) as u8)
    }
}
