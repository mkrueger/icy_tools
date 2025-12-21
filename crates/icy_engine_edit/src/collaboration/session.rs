//! Collaboration session state management.
//!
//! This module provides the shared session state for both client and server,
//! including user management, chat history, and document synchronization.

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

use super::protocol::{Block, ChatMessage, SauceData, ServerStatus, User};
use crate::SauceMetaData;

/// Unique identifier for a user in the session.
pub type UserId = u32;

/// Collaboration session state shared between connections.
#[derive(Debug)]
pub struct Session {
    /// Session password (empty string means no password)
    pub password: String,
    /// Users currently in the session
    users: RwLock<HashMap<UserId, User>>,
    /// Next user ID to assign
    next_user_id: RwLock<UserId>,
    /// Chat history
    chat_history: RwLock<Vec<ChatMessage>>,
    /// Current server status message
    status: RwLock<ServerStatus>,
    /// SAUCE metadata
    sauce: RwLock<SauceData>,
    /// Document dimensions
    columns: RwLock<u32>,
    rows: RwLock<u32>,
    /// Document settings
    use_9px: RwLock<bool>,
    ice_colors: RwLock<bool>,
    font: RwLock<String>,
    /// Protocol version supported by this session
    protocol_version: u8,
}

impl Session {
    /// Create a new session with the given password.
    pub fn new(password: String) -> Self {
        Self {
            password,
            users: RwLock::new(HashMap::new()),
            next_user_id: RwLock::new(1),
            chat_history: RwLock::new(Vec::new()),
            status: RwLock::new(ServerStatus::default()),
            sauce: RwLock::new(SauceData::default()),
            columns: RwLock::new(80),
            rows: RwLock::new(25),
            use_9px: RwLock::new(false),
            ice_colors: RwLock::new(false),
            font: RwLock::new(String::new()),
            protocol_version: 2,
        }
    }

    /// Create a session with custom dimensions.
    pub fn with_dimensions(password: String, columns: u32, rows: u32) -> Self {
        let session = Self::new(password);
        *session.columns.write() = columns;
        *session.rows.write() = rows;
        session
    }

    /// Check if the provided password matches.
    pub fn check_password(&self, password: &str) -> bool {
        self.password.is_empty() || self.password == password
    }

    /// Add a new user to the session.
    /// Returns the assigned user ID.
    pub fn add_user(&self, nick: String) -> UserId {
        let mut next_id = self.next_user_id.write();
        *next_id += 1;
        let id = *next_id;

        let user = User {
            id,
            nick,
            group: String::new(),
            status: 0,
            col: 0,
            row: 0,
            selecting: false,
            selection_col: 0,
            selection_row: 0,
        };

        self.users.write().insert(id, user);
        id
    }

    /// Remove a user from the session.
    pub fn remove_user(&self, id: UserId) -> Option<User> {
        self.users.write().remove(&id)
    }

    /// Get a copy of a user by ID.
    pub fn get_user(&self, id: UserId) -> Option<User> {
        self.users.read().get(&id).cloned()
    }

    /// Get all users in the session.
    pub fn get_users(&self) -> Vec<User> {
        self.users.read().values().cloned().collect()
    }

    /// Update a user's cursor position.
    pub fn update_cursor(&self, id: UserId, col: i32, row: i32) {
        if let Some(user) = self.users.write().get_mut(&id) {
            user.col = col;
            user.row = row;
        }
    }

    /// Update a user's selection state.
    pub fn update_selection(&self, id: UserId, selecting: bool, col: i32, row: i32) {
        if let Some(user) = self.users.write().get_mut(&id) {
            user.selecting = selecting;
            user.selection_col = col;
            user.selection_row = row;
        }
    }

    /// Update a user's selection state with start position.
    pub fn update_selection_with_start(&self, id: UserId, selecting: bool, col: i32, row: i32, _start_col: i32, _start_row: i32) {
        // For now, we only track the current selection position
        // Start position could be added to User struct if needed
        self.update_selection(id, selecting, col, row);
    }

    /// Update a user's status (ACTIVE=0, IDLE=1, AWAY=2, WEB=3).
    pub fn update_status(&self, id: UserId, status: u8) {
        if let Some(user) = self.users.write().get_mut(&id) {
            user.status = status;
        }
    }

    /// Update a user's group.
    pub fn update_group(&self, id: UserId, group: String) {
        if let Some(user) = self.users.write().get_mut(&id) {
            user.group = group;
        }
    }

    /// Get IDs of all registered users (not web clients).
    ///
    /// Web clients are identified by status == WEB (3) and have an empty nickname.
    /// They connect to view the document but don't participate in chat or
    /// receive join/leave notifications.
    ///
    /// This is used by the server to implement Moebius-compatible broadcast behavior:
    /// - `send_all`: Only sends to registered users (status != WEB)
    /// - `send_all_including_guests`: Sends to all users including web clients
    pub fn get_registered_user_ids(&self) -> Vec<UserId> {
        use super::user_status;
        self.users
            .read()
            .iter()
            .filter(|(_, user)| user.status != user_status::WEB)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Update SAUCE metadata.
    pub fn update_sauce(&self, meta: SauceMetaData) {
        let mut sauce = self.sauce.write();
        sauce.title = meta.title.to_string();
        sauce.author = meta.author.to_string();
        sauce.group = meta.group.to_string();
        sauce.comments = meta.comments.iter().map(|c| c.to_string()).collect::<Vec<_>>().join("\n");
    }

    /// Get SAUCE metadata.
    pub fn get_sauce(&self) -> SauceData {
        self.sauce.read().clone()
    }

    /// Add a chat message to the history.
    /// Moebius limits chat history to 32 messages.
    pub fn add_chat_message(&self, id: UserId, nick: String, text: String) {
        let msg = ChatMessage {
            id,
            nick,
            text,
            group: String::new(),
            time: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
        };
        let mut history = self.chat_history.write();
        history.push(msg);
        // Moebius limits chat history to 32 messages
        if history.len() > 32 {
            history.remove(0);
        }
    }

    /// Get the chat history.
    pub fn get_chat_history(&self) -> Vec<ChatMessage> {
        self.chat_history.read().clone()
    }

    /// Set the server status.
    pub fn set_status(&self, id: u32, status: u8) {
        let mut s = self.status.write();
        s.id = id;
        s.status = status;
    }

    /// Get the current server status.
    pub fn get_status(&self) -> ServerStatus {
        self.status.read().clone()
    }

    /// Get document dimensions.
    pub fn get_dimensions(&self) -> (u32, u32) {
        (*self.columns.read(), *self.rows.read())
    }

    /// Set document dimensions.
    pub fn set_dimensions(&self, columns: u32, rows: u32) {
        *self.columns.write() = columns;
        *self.rows.write() = rows;
    }

    /// Get use 9px font setting.
    pub fn get_use_9px(&self) -> bool {
        *self.use_9px.read()
    }

    /// Set use 9px font setting.
    pub fn set_use_9px(&self, value: bool) {
        *self.use_9px.write() = value;
    }

    /// Get ice colors setting.
    pub fn get_ice_colors(&self) -> bool {
        *self.ice_colors.read()
    }

    /// Set ice colors setting.
    pub fn set_ice_colors(&self, value: bool) {
        *self.ice_colors.write() = value;
    }

    /// Get font name.
    pub fn font(&self) -> String {
        self.font.read().clone()
    }

    /// Set font name.
    pub fn set_font(&self, font: String) {
        *self.font.write() = font;
    }

    /// Get protocol version.
    pub fn get_protocol_version(&self) -> u8 {
        self.protocol_version
    }
}

/// Shared session reference for use across async tasks.
pub type SharedSession = Arc<Session>;

/// Events that can occur in a collaboration session.
#[derive(Debug, Clone)]
pub enum SessionEvent {
    /// A user joined the session
    UserJoined(User),
    /// A user left the session
    UserLeft(UserId),
    /// A user's cursor moved
    CursorMoved { id: UserId, col: i32, row: i32 },
    /// A user's selection changed
    SelectionChanged { id: UserId, selecting: bool, col: i32, row: i32 },
    /// A character was drawn
    Draw { col: i32, row: i32, block: Block, layer: Option<usize> },
    /// A preview character was drawn (temporary)
    DrawPreview { col: i32, row: i32, block: Block },
    /// A chat message was sent
    Chat { nick: String, text: String },
    /// Server status changed
    StatusChanged(String),
    /// Document was resized
    Resized { columns: u32, rows: u32 },
    /// Use 9px setting changed
    Use9pxChanged(bool),
    /// Ice colors setting changed
    IceColorsChanged(bool),
    /// Font changed
    FontChanged(String),
    /// A paste operation occurred
    Paste {
        data: String,
        col: i32,
        row: i32,
        columns: u32,
        rows: u32,
        layer: Option<usize>,
    },
}

/// Callback type for session events.
pub type SessionEventHandler = Box<dyn Fn(SessionEvent) + Send + Sync>;
