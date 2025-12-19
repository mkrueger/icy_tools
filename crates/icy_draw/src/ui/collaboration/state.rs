//! Collaboration state management
//!
//! `icy_draw` owns the UI/connection wrapper state.
//! The UI-free collaboration core lives in `icy_engine_edit` for unit testing.

use icy_engine_edit::EditorUndoStack;
use icy_engine_edit::collaboration::{ChatMessage, CollaborationCoreState, ConnectedDocument, ServerStatus, User};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use super::subscription::CollaborationClient;

pub use icy_engine_edit::collaboration::{CursorMode, RemoteUser, UserId};

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
    /// Our group
    pub group: Option<String>,
    /// Session password
    pub password: Option<String>,
    /// UI-free collaboration data + sync logic
    pub core: CollaborationCoreState,
    /// Whether the chat panel is visible
    pub chat_visible: bool,
    /// Current chat input text
    pub chat_input: String,
    /// Whether the chat input field is focused
    pub chat_input_focused: bool,
    /// Whether we are currently connecting
    pub connecting: bool,
}

impl CollaborationState {
    /// Create new collaboration state
    pub fn new() -> Self {
        Self::default()
    }

    /// Start connecting to a server
    pub fn start_connecting(&mut self, url: String, nick: String, group: String, password: String) {
        self.connecting = true;
        self.server_url = Some(url);
        self.nick = Some(nick);
        self.group = Some(group);
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
    pub fn start_session(&mut self, doc: &ConnectedDocument) {
        self.active = true;
        self.core.start_session(doc);
    }

    /// End the collaboration session
    pub fn end_session(&mut self) {
        self.active = false;
        self.connecting = false;
        self.client = None;
        self.core.end_session();
        // Keep chat messages for reference
    }

    /// Check if we are connected and have an active client
    pub fn is_connected(&self) -> bool {
        self.active && self.client.is_some()
    }

    /// Add a remote user
    ///
    /// `show_join` controls whether a system message is shown.
    /// Set to `false` for users that were already connected when we joined.
    pub fn add_user(&mut self, user: User, show_join: bool) {
        // Show system message for user joining (unless it's us or a guest)
        if show_join && Some(user.id) != self.core.our_user_id && !user.nick.is_empty() {
            let msg = if user.group.is_empty() {
                format!("{} has joined", user.nick)
            } else {
                format!("{} <{}> has joined", user.nick, user.group)
            };
            self.core.add_system_message(&msg);
        }
        self.core.add_user(user);
    }

    /// Remove a remote user
    ///
    /// `show_leave` controls whether a system message is shown.
    /// Set to `false` when disconnecting (bulk user removal).
    pub fn remove_user(&mut self, user_id: UserId, show_leave: bool) {
        // Show system message for user leaving
        if show_leave {
            if let Some(remote_user) = self.core.get_user(user_id) {
                if !remote_user.user.nick.is_empty() {
                    let msg = if remote_user.user.group.is_empty() {
                        format!("{} has left", remote_user.user.nick)
                    } else {
                        format!("{} <{}> has left", remote_user.user.nick, remote_user.user.group)
                    };
                    self.core.add_system_message(&msg);
                }
            }
        }
        self.core.remove_user(user_id);
    }

    /// Update user cursor position
    pub fn update_cursor(&mut self, user_id: UserId, col: i32, row: i32) {
        self.core.update_cursor(user_id, col, row);
    }

    /// Update user selection
    pub fn update_selection(&mut self, user_id: UserId, selecting: bool, col: i32, row: i32) {
        self.core.update_selection(user_id, selecting, col, row);
    }

    /// Update user operation state (floating selection)
    pub fn update_operation(&mut self, user_id: UserId, col: i32, row: i32) {
        self.core.update_operation(user_id, col, row);
    }

    /// Hide user cursor
    pub fn hide_user_cursor(&mut self, user_id: UserId) {
        self.core.hide_user_cursor(user_id);
    }

    /// Update user status
    pub fn update_user_status(&mut self, user_id: UserId, status: u8) {
        self.core.update_user_status(user_id, status);
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
        self.core.add_chat_message(message);
    }

    /// Add a system message (e.g., for notifications)
    pub fn add_system_message(&mut self, text: &str) {
        self.core.add_system_message(text);
    }

    /// Update server status
    pub fn update_server_status(&mut self, status: ServerStatus) {
        self.core.update_server_status(status);
    }

    /// Update canvas size
    pub fn update_canvas_size(&mut self, columns: u32, rows: u32) {
        self.core.update_canvas_size(columns, rows);
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
        if let (Some(id), Some(nick)) = (self.core.our_user_id, self.nick.as_ref()) {
            let group = self.group.clone().unwrap_or_default();
            let time = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
            self.core.add_chat_message(ChatMessage {
                id,
                nick: nick.clone(),
                group,
                text: text.clone(),
                time,
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

    /// Send paste-as-selection (floating layer blocks) for collaboration
    pub fn send_paste_as_selection(&self, blocks: icy_engine_edit::collaboration::Blocks) -> Option<iced::Task<()>> {
        if let Some(client) = &self.client {
            let handle = client.handle().clone();
            Some(iced::Task::future(async move {
                let _ = handle.paste_as_selection(blocks).await;
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
    pub fn send_sauce(&self, title: String, author: String, group: String, comments: String) -> Option<iced::Task<()>> {
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
        self.core.sorted_users()
    }

    /// Get user by ID
    pub fn get_user(&self, user_id: UserId) -> Option<&RemoteUser> {
        self.core.get_user(user_id)
    }

    /// Get user's color for rendering cursor (generated from user ID)
    pub fn user_color(&self, user_id: UserId) -> (u8, u8, u8) {
        self.core.user_color(user_id)
    }

    /// Get user's avatar (deterministically assigned from user ID)
    pub fn user_avatar(&self, user_id: UserId) -> super::icons::Avatar {
        super::icons::Avatar::from_user_id(user_id)
    }

    /// Get our own user ID
    pub fn our_user_id(&self) -> Option<UserId> {
        self.core.our_user_id
    }

    /// Get our own nickname
    pub fn our_nick(&self) -> Option<&str> {
        self.nick.as_deref()
    }

    /// Get our own group
    pub fn our_group(&self) -> Option<&str> {
        self.group.as_deref()
    }

    /// Synchronize with the undo stack and send pending operations to the server.
    ///
    /// This method tracks a sync_pointer into the undo stack. When called:
    /// - If undo_stack.len() > sync_pointer: new operations were pushed (redo direction)
    ///   -> collect redo_client_commands for ops [sync_pointer..len]
    /// - If undo_stack.len() < sync_pointer: operations were undone
    ///   -> collect undo_client_commands for ops that moved to redo stack
    ///
    /// Also syncs cursor position and selection state.
    ///
    /// Returns a Task that sends all collected commands to the server.
    pub fn sync_from_undo_stack(&mut self, undo_stack: &EditorUndoStack, caret_pos: (i32, i32), selecting: bool) -> Option<iced::Task<()>> {
        // Skip if not connected
        let client = self.client.as_ref()?;
        let handle = client.handle().clone();

        let commands = self.core.sync_from_undo_stack(undo_stack, caret_pos, selecting);

        if commands.is_empty() {
            return None;
        }

        // Send all commands
        Some(iced::Task::future(async move {
            for cmd in commands {
                // Use the send_command method which handles all command types
                let _ = handle.send_command(cmd).await;
            }
        }))
    }

    /// Reset sync pointer (call when loading a new document or disconnecting)
    pub fn reset_sync_pointer(&mut self) {
        self.core.reset_sync_pointer();
    }

    /// Set sync pointer to current undo stack length (call after initial sync)
    pub fn set_sync_pointer(&mut self, len: usize) {
        self.core.set_sync_pointer(len);
    }

    pub fn remote_users(&self) -> &HashMap<UserId, RemoteUser> {
        &self.core.remote_users
    }

    pub fn chat_messages(&self) -> &[ChatMessage] {
        &self.core.chat_messages
    }
}
