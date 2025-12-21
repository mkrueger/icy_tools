use crate::EditorUndoStack;
use std::collections::HashMap;

use super::{ChatMessage, ClientCommand, ConnectedDocument, ServerStatus, User, UserId};

/// User status constants.
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

/// Cursor mode for remote users.
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

/// Selection state for a user.
#[derive(Debug, Clone)]
pub struct SelectionState {
    pub selecting: bool,
    pub col: i32,
    pub row: i32,
}

/// Operation state for a user (floating selection).
#[derive(Debug, Clone)]
pub struct OperationState {
    pub col: i32,
    pub row: i32,
}

/// Remote user state.
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

/// UI-free collaboration state.
///
/// Keeps the data model for collaboration sessions (users/chat/doc fields) and
/// provides a testable undo-stack-to-command sync.
#[derive(Debug, Default)]
pub struct CollaborationCoreState {
    /// Our user ID (assigned by server)
    pub our_user_id: Option<UserId>,
    /// Remote users (excluding ourselves)
    pub remote_users: HashMap<UserId, RemoteUser>,
    /// Chat history
    pub chat_messages: Vec<ChatMessage>,
    /// Server status
    pub server_status: Option<ServerStatus>,
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

    /// Sync pointer into undo stack - tracks which operations have been synced
    /// This points to the undo_stack length at last sync
    sync_pointer: usize,

    /// Whether sync has been initialized (first call sets pointer without sending ops)
    sync_initialized: bool,

    /// Last sent cursor position (to avoid sending duplicates)
    last_cursor: Option<(i32, i32)>,
    /// Last sent selection state (to avoid sending duplicates)
    last_selection: Option<(bool, i32, i32)>,
}

impl CollaborationCoreState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start_session(&mut self, doc: &ConnectedDocument) {
        self.our_user_id = Some(doc.user_id);
        self.columns = doc.columns;
        self.rows = doc.rows;
        self.use_9px = doc.use_9px;
        self.ice_colors = doc.ice_colors;
        self.font = doc.font.clone();
        self.remote_users.clear();
        self.chat_messages = doc.chat_history.clone();
        self.server_status = None;
        self.reset_sync_pointer();
    }

    pub fn end_session(&mut self) {
        self.our_user_id = None;
        self.remote_users.clear();
        self.server_status = None;
        self.reset_sync_pointer();
        // Keep chat messages for reference
    }

    pub fn update_server_status(&mut self, status: ServerStatus) {
        self.server_status = Some(status);
    }

    pub fn update_canvas_size(&mut self, columns: u32, rows: u32) {
        self.columns = columns;
        self.rows = rows;
    }

    pub fn add_user(&mut self, user: User) {
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

    pub fn remove_user(&mut self, user_id: UserId) {
        self.remote_users.remove(&user_id);
    }

    pub fn update_cursor(&mut self, user_id: UserId, col: i32, row: i32) {
        if let Some(user) = self.remote_users.get_mut(&user_id) {
            user.cursor = Some((col, row));
            user.cursor_mode = CursorMode::Editing;
        }
    }

    pub fn update_selection(&mut self, user_id: UserId, selecting: bool, col: i32, row: i32) {
        if let Some(user) = self.remote_users.get_mut(&user_id) {
            user.selection = Some(SelectionState { selecting, col, row });
            user.cursor_mode = CursorMode::Selection;
        }
    }

    pub fn update_operation(&mut self, user_id: UserId, col: i32, row: i32) {
        if let Some(user) = self.remote_users.get_mut(&user_id) {
            user.operation = Some(OperationState { col, row });
            user.cursor_mode = CursorMode::Operation;
        }
    }

    pub fn hide_user_cursor(&mut self, user_id: UserId) {
        if let Some(user) = self.remote_users.get_mut(&user_id) {
            user.cursor_mode = CursorMode::Hidden;
        }
    }

    pub fn update_user_status(&mut self, user_id: UserId, status: u8) {
        if let Some(user) = self.remote_users.get_mut(&user_id) {
            user.status = status;
            user.user.status = status;
        }
    }

    pub fn add_chat_message(&mut self, message: ChatMessage) {
        self.chat_messages.push(message);
    }

    pub fn add_system_message(&mut self, text: &str) {
        self.chat_messages.push(ChatMessage {
            id: 0,
            nick: String::new(),
            group: String::new(),
            text: text.to_string(),
            time: 0,
        });
    }

    pub fn sorted_users(&self) -> Vec<&RemoteUser> {
        let mut users: Vec<_> = self.remote_users.values().collect();
        users.sort_by(|a, b| a.user.nick.cmp(&b.user.nick));
        users
    }

    pub fn get_user(&self, user_id: UserId) -> Option<&RemoteUser> {
        self.remote_users.get(&user_id)
    }

    pub fn user_color(&self, user_id: UserId) -> (u8, u8, u8) {
        let hue = ((user_id * 137) % 360) as f32;
        let saturation: f32 = 0.7;
        let lightness: f32 = 0.5;

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

    /// Synchronize with the undo stack and return pending operations to send to the server.
    ///
    /// See `icy_draw::CollaborationState::sync_from_undo_stack()` for the sending wrapper.
    pub fn sync_from_undo_stack(&mut self, undo_stack: &EditorUndoStack, caret_pos: (i32, i32), selecting: bool) -> Vec<ClientCommand> {
        let current_len = undo_stack.undo_stack().len();
        let mut commands: Vec<ClientCommand> = Vec::new();

        if !self.sync_initialized {
            self.sync_initialized = true;
            self.sync_pointer = current_len;
        } else if current_len > self.sync_pointer {
            for op in &undo_stack.undo_stack()[self.sync_pointer..current_len] {
                if let Some(cmds) = op.redo_client_commands() {
                    commands.extend(cmds);
                }
            }
            self.sync_pointer = current_len;
        } else if current_len < self.sync_pointer {
            let undone_count = self.sync_pointer - current_len;
            let redo_stack = undo_stack.redo_stack();
            let redo_len = redo_stack.len();
            if undone_count <= redo_len {
                for op in redo_stack.iter().rev().take(undone_count) {
                    if let Some(cmds) = op.undo_client_commands() {
                        commands.extend(cmds);
                    }
                }
            }
            self.sync_pointer = current_len;
        }

        // Moebius protocol: Send either CURSOR or SELECTION, not both.
        // - SELECTION (5): when actively creating/extending a selection (selecting=true)
        // - CURSOR (4): when in normal editing mode (selecting=false)
        let sel_state = (selecting, caret_pos.0, caret_pos.1);
        if selecting {
            // Selection mode: only send Selection events
            if self.last_selection != Some(sel_state) {
                self.last_selection = Some(sel_state);
                commands.push(ClientCommand::Selection {
                    selecting: true,
                    col: caret_pos.0,
                    row: caret_pos.1,
                });
            }
            // Update cursor tracking but don't send cursor command
            self.last_cursor = Some(caret_pos);
        } else {
            // Editing mode: only send Cursor events
            if self.last_cursor != Some(caret_pos) {
                self.last_cursor = Some(caret_pos);
                commands.push(ClientCommand::Cursor {
                    col: caret_pos.0,
                    row: caret_pos.1,
                });
            }
            // If we were selecting before, send a final Selection(false) to exit selection mode
            if self.last_selection.map(|s| s.0).unwrap_or(false) {
                self.last_selection = Some(sel_state);
                commands.push(ClientCommand::Selection {
                    selecting: false,
                    col: caret_pos.0,
                    row: caret_pos.1,
                });
            } else {
                self.last_selection = Some(sel_state);
            }
        }

        commands
    }

    pub fn reset_sync_pointer(&mut self) {
        self.sync_pointer = 0;
        self.sync_initialized = false;
        self.last_cursor = None;
        self.last_selection = None;
    }

    pub fn set_sync_pointer(&mut self, len: usize) {
        self.sync_pointer = len;
    }
}
