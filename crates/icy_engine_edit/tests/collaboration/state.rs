use super::*;

fn dummy_doc(user_id: u32) -> ConnectedDocument {
    ConnectedDocument {
        user_id,
        document: vec![],
        columns: 80,
        rows: 25,
        users: vec![],
        chat_history: vec![],
        use_9px: false,
        ice_colors: true,
        font: "IBM VGA".to_string(),
        palette: [[0; 3]; 16], // Default black palette for tests
        title: String::new(),
        author: String::new(),
        group: String::new(),
        comments: String::new(),
    }
}

fn make_user(id: u32, nick: &str, group: &str) -> User {
    User {
        id,
        nick: nick.to_string(),
        group: group.to_string(),
        status: user_status::ACTIVE,
        col: 0,
        row: 0,
        selecting: false,
        selection_col: 0,
        selection_row: 0,
    }
}

// ========================================================================
// User Management Tests
// ========================================================================

#[test]
fn add_user_does_not_add_self() {
    let mut core = CollaborationCoreState::new();
    core.start_session(&dummy_doc(10));

    core.add_user(make_user(10, "me", ""));
    assert!(core.remote_users.is_empty());
}

#[test]
fn add_and_remove_users() {
    let mut core = CollaborationCoreState::new();
    core.start_session(&dummy_doc(1));

    // Add users
    core.add_user(make_user(2, "Alice", "Group1"));
    core.add_user(make_user(3, "Bob", "Group2"));

    assert_eq!(core.remote_users.len(), 2);
    assert!(core.get_user(2).is_some());
    assert!(core.get_user(3).is_some());

    // Verify user data
    let alice = core.get_user(2).unwrap();
    assert_eq!(alice.user.nick, "Alice");
    assert_eq!(alice.user.group, "Group1");

    // Remove user
    core.remove_user(2);
    assert_eq!(core.remote_users.len(), 1);
    assert!(core.get_user(2).is_none());
    assert!(core.get_user(3).is_some());
}

#[test]
fn sorted_users_returns_alphabetical() {
    let mut core = CollaborationCoreState::new();
    core.start_session(&dummy_doc(1));

    core.add_user(make_user(2, "Zoe", ""));
    core.add_user(make_user(3, "Alice", ""));
    core.add_user(make_user(4, "Mike", ""));

    let sorted = core.sorted_users();
    assert_eq!(sorted.len(), 3);
    assert_eq!(sorted[0].user.nick, "Alice");
    assert_eq!(sorted[1].user.nick, "Mike");
    assert_eq!(sorted[2].user.nick, "Zoe");
}

// ========================================================================
// Cursor & Selection Tests
// ========================================================================

#[test]
fn update_cursor_position() {
    let mut core = CollaborationCoreState::new();
    core.start_session(&dummy_doc(1));
    core.add_user(make_user(2, "User", ""));

    core.update_cursor(2, 10, 20);

    let user = core.get_user(2).unwrap();
    assert_eq!(user.cursor, Some((10, 20)));
    assert_eq!(user.cursor_mode, CursorMode::Editing);
}

#[test]
fn update_selection() {
    let mut core = CollaborationCoreState::new();
    core.start_session(&dummy_doc(1));
    core.add_user(make_user(2, "User", ""));

    core.update_selection(2, true, 5, 10);

    let user = core.get_user(2).unwrap();
    let selection = user.selection.as_ref().unwrap();
    assert!(selection.selecting);
    assert_eq!(selection.col, 5);
    assert_eq!(selection.row, 10);
    assert_eq!(user.cursor_mode, CursorMode::Selection);
}

#[test]
fn hide_cursor() {
    let mut core = CollaborationCoreState::new();
    core.start_session(&dummy_doc(1));
    core.add_user(make_user(2, "User", ""));

    core.update_cursor(2, 10, 20);
    core.hide_user_cursor(2);

    let user = core.get_user(2).unwrap();
    assert_eq!(user.cursor_mode, CursorMode::Hidden);
}

// ========================================================================
// Status Tests
// ========================================================================

#[test]
fn update_user_status() {
    let mut core = CollaborationCoreState::new();
    core.start_session(&dummy_doc(1));
    core.add_user(make_user(2, "User", ""));

    core.update_user_status(2, user_status::AWAY);

    let user = core.get_user(2).unwrap();
    assert_eq!(user.status, user_status::AWAY);
}

// ========================================================================
// Chat Message Tests
// ========================================================================

#[test]
fn add_chat_messages() {
    let mut core = CollaborationCoreState::new();

    core.add_chat_message(ChatMessage {
        id: 1,
        nick: "Alice".to_string(),
        group: "Group1".to_string(),
        text: "Hello!".to_string(),
        time: 1234567890,
    });

    core.add_chat_message(ChatMessage {
        id: 2,
        nick: "Bob".to_string(),
        group: "".to_string(),
        text: "Hi there!".to_string(),
        time: 1234567891,
    });

    assert_eq!(core.chat_messages.len(), 2);
    assert_eq!(core.chat_messages[0].nick, "Alice");
    assert_eq!(core.chat_messages[0].group, "Group1");
    assert_eq!(core.chat_messages[1].text, "Hi there!");
}

#[test]
fn add_system_message() {
    let mut core = CollaborationCoreState::new();

    core.add_system_message("Alice has joined");
    core.add_system_message("Bob has left");

    assert_eq!(core.chat_messages.len(), 2);
    // System messages have id=0 and empty nick
    assert_eq!(core.chat_messages[0].id, 0);
    assert!(core.chat_messages[0].nick.is_empty());
    assert_eq!(core.chat_messages[0].text, "Alice has joined");
    assert_eq!(core.chat_messages[1].text, "Bob has left");
}

// ========================================================================
// Canvas Size Tests
// ========================================================================

#[test]
fn update_canvas_size() {
    let mut core = CollaborationCoreState::new();
    core.start_session(&dummy_doc(1));

    assert_eq!(core.columns, 80);
    assert_eq!(core.rows, 25);

    core.update_canvas_size(160, 50);

    assert_eq!(core.columns, 160);
    assert_eq!(core.rows, 50);
}

// ========================================================================
// Session Lifecycle Tests
// ========================================================================

#[test]
fn start_and_end_session() {
    let mut core = CollaborationCoreState::new();

    // Start session
    let doc = dummy_doc(42);
    core.start_session(&doc);

    assert_eq!(core.our_user_id, Some(42));
    assert_eq!(core.columns, 80);
    assert_eq!(core.rows, 25);

    // Add some state
    core.add_user(make_user(2, "User", ""));
    core.add_chat_message(ChatMessage {
        id: 2,
        nick: "User".to_string(),
        group: "".to_string(),
        text: "Hello".to_string(),
        time: 0,
    });

    // End session
    core.end_session();

    assert!(core.our_user_id.is_none());
    assert!(core.remote_users.is_empty());
    // Chat messages are preserved for reference
    assert_eq!(core.chat_messages.len(), 1);
}

#[test]
fn start_session_with_users_and_chat() {
    let mut core = CollaborationCoreState::new();

    let doc = ConnectedDocument {
        user_id: 1,
        document: vec![],
        columns: 100,
        rows: 40,
        users: vec![make_user(2, "Alice", "G1"), make_user(3, "Bob", "G2")],
        chat_history: vec![
            ChatMessage {
                id: 2,
                nick: "Alice".to_string(),
                group: "G1".to_string(),
                text: "Hi".to_string(),
                time: 100,
            },
            ChatMessage {
                id: 3,
                nick: "Bob".to_string(),
                group: "G2".to_string(),
                text: "Hello".to_string(),
                time: 200,
            },
        ],
        use_9px: true,
        ice_colors: false,
        font: "Topaz".to_string(),
        palette: [[0; 3]; 16], // Default black palette for tests
        title: "Test".to_string(),
        author: "Author".to_string(),
        group: "Group".to_string(),
        comments: "Comments".to_string(),
    };

    core.start_session(&doc);
    // Manually add users as they would come via Join events
    for user in &doc.users {
        core.add_user(user.clone());
    }

    assert_eq!(core.our_user_id, Some(1));
    assert_eq!(core.columns, 100);
    assert_eq!(core.rows, 40);
    assert_eq!(core.remote_users.len(), 2);
    assert_eq!(core.chat_messages.len(), 2);
    assert_eq!(core.chat_messages[0].nick, "Alice");
    assert_eq!(core.chat_messages[1].nick, "Bob");
}

// ========================================================================
// User Color Generation Tests
// ========================================================================

#[test]
fn user_color_is_deterministic() {
    let core = CollaborationCoreState::new();

    let color1 = core.user_color(42);
    let color2 = core.user_color(42);

    assert_eq!(color1, color2);
}

#[test]
fn different_users_have_different_colors() {
    let core = CollaborationCoreState::new();

    let color1 = core.user_color(1);
    let color2 = core.user_color(2);
    let color3 = core.user_color(100);

    // Colors should generally differ (not always guaranteed, but highly likely)
    assert!(color1 != color2 || color2 != color3);
}

// ========================================================================
// Sync Tests
// ========================================================================

#[test]
fn sync_initializes_without_emitting_existing_ops() {
    let mut core = CollaborationCoreState::new();
    let mut undo = EditorUndoStack::new();
    undo.push(EditorUndoOp::SetUseLetterSpacing { new_ls: true });

    // When selecting=false, only Cursor is sent (Moebius protocol)
    let cmds = core.sync_from_undo_stack(&undo, (1, 2), false);

    assert_eq!(cmds, vec![ClientCommand::Cursor { col: 1, row: 2 }]);
}

#[test]
fn sync_emits_redo_for_new_ops_and_undo_for_reverted_ops() {
    let mut core = CollaborationCoreState::new();
    let mut undo = EditorUndoStack::new();

    // Initial call sets pointer
    undo.push(EditorUndoOp::SetUseLetterSpacing { new_ls: true });
    let _ = core.sync_from_undo_stack(&undo, (0, 0), false);

    // Push new op -> redo commands
    undo.push(EditorUndoOp::SetUseLetterSpacing { new_ls: false });
    let cmds = core.sync_from_undo_stack(&undo, (0, 0), false);
    assert_eq!(cmds, vec![ClientCommand::SetUse9px { value: false }]);

    // Simulate undo of last op by moving it to redo stack
    let op = undo.pop_undo().unwrap();
    undo.push_redo(op);

    let cmds = core.sync_from_undo_stack(&undo, (0, 0), false);
    assert_eq!(cmds, vec![ClientCommand::SetUse9px { value: true }]);
}

#[test]
fn sync_tracks_resize_ops() {
    let mut core = CollaborationCoreState::new();
    let mut undo = EditorUndoStack::new();

    let _ = core.sync_from_undo_stack(&undo, (0, 0), false);

    undo.push(EditorUndoOp::ResizeBuffer {
        orig_size: Size { width: 80, height: 25 },
        size: Size { width: 100, height: 30 },
    });

    let cmds = core.sync_from_undo_stack(&undo, (0, 0), false);
    assert_eq!(cmds, vec![ClientCommand::SetCanvasSize { columns: 100, rows: 30 }]);
}

#[test]
fn sync_cursor_only_emits_on_change() {
    let mut core = CollaborationCoreState::new();
    let undo = EditorUndoStack::new();

    // First call should emit cursor
    let cmds = core.sync_from_undo_stack(&undo, (5, 10), false);
    assert!(cmds.contains(&ClientCommand::Cursor { col: 5, row: 10 }));

    // Same position should not emit cursor again
    let cmds = core.sync_from_undo_stack(&undo, (5, 10), false);
    assert!(cmds.is_empty());

    // Different position should emit
    let cmds = core.sync_from_undo_stack(&undo, (6, 10), false);
    assert!(cmds.contains(&ClientCommand::Cursor { col: 6, row: 10 }));
}

#[test]
fn sync_selection_only_emits_on_change() {
    let mut core = CollaborationCoreState::new();
    let undo = EditorUndoStack::new();

    // First call should emit selection
    let cmds = core.sync_from_undo_stack(&undo, (0, 0), true);
    assert!(cmds.iter().any(|c| matches!(c, ClientCommand::Selection { selecting: true, .. })));

    // Same selection state should not emit again
    let cmds = core.sync_from_undo_stack(&undo, (0, 0), true);
    assert!(cmds.is_empty());

    // Changed selection state should emit
    let cmds = core.sync_from_undo_stack(&undo, (0, 0), false);
    assert!(cmds.iter().any(|c| matches!(c, ClientCommand::Selection { selecting: false, .. })));
}

#[test]
fn reset_sync_pointer() {
    let mut core = CollaborationCoreState::new();
    let mut undo = EditorUndoStack::new();

    undo.push(EditorUndoOp::SetUseLetterSpacing { new_ls: true });
    let _ = core.sync_from_undo_stack(&undo, (5, 5), false);

    core.reset_sync_pointer();

    // After reset, sync should re-initialize and emit cursor again
    let cmds = core.sync_from_undo_stack(&undo, (5, 5), false);
    assert!(cmds.contains(&ClientCommand::Cursor { col: 5, row: 5 }));
}
