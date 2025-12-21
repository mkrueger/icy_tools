use super::*;
use serde_json::json;

// ========================================================================
// ActionCode Tests
// ========================================================================

#[test]
fn action_code_try_from_valid() {
    assert_eq!(ActionCode::try_from(0), Ok(ActionCode::Connected));
    assert_eq!(ActionCode::try_from(1), Ok(ActionCode::Refused));
    assert_eq!(ActionCode::try_from(2), Ok(ActionCode::Join));
    assert_eq!(ActionCode::try_from(3), Ok(ActionCode::Leave));
    assert_eq!(ActionCode::try_from(4), Ok(ActionCode::Cursor));
    assert_eq!(ActionCode::try_from(5), Ok(ActionCode::Selection));
    assert_eq!(ActionCode::try_from(9), Ok(ActionCode::Draw));
    assert_eq!(ActionCode::try_from(10), Ok(ActionCode::Chat));
    assert_eq!(ActionCode::try_from(11), Ok(ActionCode::Status));
    assert_eq!(ActionCode::try_from(12), Ok(ActionCode::Sauce));
    assert_eq!(ActionCode::try_from(16), Ok(ActionCode::SetCanvasSize));
    assert_eq!(ActionCode::try_from(21), Ok(ActionCode::SetBackground));
}

#[test]
fn action_code_try_from_invalid() {
    assert!(ActionCode::try_from(22).is_err());
    assert!(ActionCode::try_from(100).is_err());
    assert!(ActionCode::try_from(255).is_err());
}

// ========================================================================
// User Serialization Tests
// ========================================================================

#[test]
fn user_serialization() {
    let user = User {
        id: 42,
        nick: "TestUser".to_string(),
        group: "TestGroup".to_string(),
        status: user_status::ACTIVE,
        col: 10,
        row: 20,
        selecting: true,
        selection_col: 5,
        selection_row: 15,
    };

    let json = serde_json::to_string(&user).unwrap();
    let parsed: User = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.id, 42);
    assert_eq!(parsed.nick, "TestUser");
    assert_eq!(parsed.group, "TestGroup");
    assert_eq!(parsed.status, user_status::ACTIVE);
}

#[test]
fn user_deserialization_with_defaults() {
    // Moebius sends minimal user data
    let json = json!({
        "id": 5,
        "nick": "Guest",
        "status": 1
    });

    let user: User = serde_json::from_value(json).unwrap();

    assert_eq!(user.id, 5);
    assert_eq!(user.nick, "Guest");
    assert_eq!(user.group, ""); // default
    assert_eq!(user.status, user_status::IDLE);
    assert_eq!(user.col, 0); // default
    assert_eq!(user.row, 0); // default
}

// ========================================================================
// ChatMessage Serialization Tests
// ========================================================================

#[test]
fn chat_message_serialization() {
    let msg = ChatMessage {
        id: 1,
        nick: "Alice".to_string(),
        text: "Hello, world!".to_string(),
        group: "Blocktronics".to_string(),
        time: 1234567890,
    };

    let json = serde_json::to_string(&msg).unwrap();
    let parsed: ChatMessage = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.id, 1);
    assert_eq!(parsed.nick, "Alice");
    assert_eq!(parsed.text, "Hello, world!");
    assert_eq!(parsed.group, "Blocktronics");
    assert_eq!(parsed.time, 1234567890);
}

#[test]
fn chat_message_deserialization_with_defaults() {
    let json = json!({
        "id": 2,
        "nick": "Bob",
        "text": "Hi!"
    });

    let msg: ChatMessage = serde_json::from_value(json).unwrap();

    assert_eq!(msg.id, 2);
    assert_eq!(msg.nick, "Bob");
    assert_eq!(msg.text, "Hi!");
    assert_eq!(msg.group, ""); // default
    assert_eq!(msg.time, 0); // default
}

// ========================================================================
// Block Serialization Tests
// ========================================================================

#[test]
fn block_serialization() {
    let block = Block {
        code: 65, // 'A'
        fg: 7,
        bg: 0,
    };

    let json = serde_json::to_string(&block).unwrap();
    let parsed: Block = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.code, 65);
    assert_eq!(parsed.fg, 7);
    assert_eq!(parsed.bg, 0);
}

#[test]
fn block_with_extended_colors() {
    let block = Block {
        code: 219, // Full block
        fg: 15,    // Bright white
        bg: 4,     // Red
    };

    let json = serde_json::to_string(&block).unwrap();
    assert!(json.contains("\"code\":219"));
    assert!(json.contains("\"fg\":15"));
    assert!(json.contains("\"bg\":4"));
}

// ========================================================================
// Blocks Collection Serialization Tests
// ========================================================================

#[test]
fn blocks_serialization() {
    let blocks = Blocks {
        columns: 2,
        rows: 2,
        data: vec![
            Block { code: 65, fg: 7, bg: 0 },
            Block { code: 66, fg: 2, bg: 1 },
            Block { code: 67, fg: 3, bg: 2 },
            Block { code: 68, fg: 4, bg: 3 },
        ],
    };

    let json = serde_json::to_string(&blocks).unwrap();
    let parsed: Blocks = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.columns, 2);
    assert_eq!(parsed.rows, 2);
    assert_eq!(parsed.data.len(), 4);
    assert_eq!(parsed.data[0].code, 65);
    assert_eq!(parsed.data[3].code, 68);
}

// ========================================================================
// IncomingMessage Tests
// ========================================================================

#[test]
fn incoming_message_action_code() {
    let msg: IncomingMessage = serde_json::from_value(json!({
        "type": 10,
        "data": { "id": 1, "nick": "Test", "text": "Hello" }
    }))
    .unwrap();

    assert_eq!(msg.action_code(), Some(ActionCode::Chat));
}

#[test]
fn incoming_message_unknown_action() {
    let msg: IncomingMessage = serde_json::from_value(json!({
        "type": 99,
        "data": {}
    }))
    .unwrap();

    assert_eq!(msg.action_code(), None);
}

// ========================================================================
// ClientCommand Serialization Tests
// ========================================================================

#[test]
fn client_command_cursor() {
    let cmd = ClientCommand::Cursor { col: 10, row: 20 };

    if let ClientCommand::Cursor { col, row } = cmd {
        assert_eq!(col, 10);
        assert_eq!(row, 20);
    } else {
        panic!("Expected Cursor command");
    }
}

#[test]
fn client_command_draw() {
    let block = Block { code: 65, fg: 7, bg: 0 };
    let cmd = ClientCommand::Draw {
        col: 5,
        row: 10,
        block: block.clone(),
    };

    if let ClientCommand::Draw { col, row, block: b } = cmd {
        assert_eq!(col, 5);
        assert_eq!(row, 10);
        assert_eq!(b.code, 65);
    } else {
        panic!("Expected Draw command");
    }
}

#[test]
fn client_command_chat() {
    let cmd = ClientCommand::Chat {
        text: "Hello, world!".to_string(),
    };

    if let ClientCommand::Chat { text } = cmd {
        assert_eq!(text, "Hello, world!");
    } else {
        panic!("Expected Chat command");
    }
}

#[test]
fn client_command_set_canvas_size() {
    let cmd = ClientCommand::SetCanvasSize { columns: 160, rows: 50 };

    if let ClientCommand::SetCanvasSize { columns, rows } = cmd {
        assert_eq!(columns, 160);
        assert_eq!(rows, 50);
    } else {
        panic!("Expected SetCanvasSize command");
    }
}

// ========================================================================
// User Status Constants Tests
// ========================================================================

#[test]
fn user_status_constants() {
    assert_eq!(user_status::ACTIVE, 0);
    assert_eq!(user_status::IDLE, 1);
    assert_eq!(user_status::AWAY, 2);
    assert_eq!(user_status::WEB, 3);
}

// ========================================================================
// SauceData Serialization Tests
// ========================================================================

#[test]
fn sauce_data_serialization() {
    let sauce = SauceData {
        id: 1,
        title: "My Art".to_string(),
        author: "Artist".to_string(),
        group: "Crew".to_string(),
        comments: "Line 1\nLine 2".to_string(),
    };

    let json = serde_json::to_string(&sauce).unwrap();
    let parsed: SauceData = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.id, 1);
    assert_eq!(parsed.title, "My Art");
    assert_eq!(parsed.author, "Artist");
    assert_eq!(parsed.group, "Crew");
    assert_eq!(parsed.comments, "Line 1\nLine 2");
}

// ========================================================================
// ConnectedDocument Tests
// ========================================================================

#[test]
fn connected_document_fields() {
    let doc = ConnectedDocument {
        user_id: 42,
        document: vec![vec![Block { code: 65, fg: 7, bg: 0 }]],
        columns: 80,
        rows: 25,
        users: vec![User {
            id: 1,
            nick: "Other".to_string(),
            group: "G".to_string(),
            status: 0,
            col: 0,
            row: 0,
            selecting: false,
            selection_col: 0,
            selection_row: 0,
        }],
        chat_history: vec![ChatMessage {
            id: 1,
            nick: "Other".to_string(),
            text: "Hi".to_string(),
            group: "G".to_string(),
            time: 123,
        }],
        use_9px: false,
        ice_colors: true,
        font: "IBM VGA".to_string(),
        palette: DEFAULT_EGA_PALETTE,
        title: "Test".to_string(),
        author: "Author".to_string(),
        group: "Group".to_string(),
        comments: "Comments".to_string(),
    };

    assert_eq!(doc.user_id, 42);
    assert_eq!(doc.columns, 80);
    assert_eq!(doc.rows, 25);
    assert_eq!(doc.users.len(), 1);
    assert_eq!(doc.chat_history.len(), 1);
    assert!(doc.ice_colors);
    assert!(!doc.use_9px);
}

#[test]
fn parse_moebius_palette_accepts_6bit_and_scales_to_8bit() {
    // 6-bit white (63) should become 8-bit 255 via Moebius expansion.
    let palette_json = serde_json::json!([
        {"r": 63, "g": 63, "b": 63},
        {"r": 0, "g": 0, "b": 0}
    ]);
    let palette = parse_moebius_palette(&palette_json);

    assert_eq!(palette[0], [255, 255, 255]);
    assert_eq!(palette[1], [0, 0, 0]);
}

/// Test that buffer and layer sizes remain consistent after applying a remote document.
/// This is a regression test for a bug where the buffer would show only 25 rows
/// when the document was 50 rows, because layer 0 size was not properly synchronized.
#[test]
fn buffer_layer_size_consistency_after_remote_document() {
    use icy_engine::{AttributedChar, Position, TextBuffer, TextPane};

    // Create a "remote" document with 80x50 (50 rows - more than default 25)
    let mut document = Vec::new();
    for _col in 0..80 {
        let mut column = Vec::new();
        for row in 0..50 {
            column.push(Block {
                code: if row == 49 { 'X' as u32 } else { ' ' as u32 },
                fg: 7,
                bg: 0,
            });
        }
        document.push(column);
    }

    let remote_doc = ConnectedDocument {
        user_id: 1,
        document,
        columns: 80,
        rows: 50, // 50 rows, not 25!
        users: vec![],
        chat_history: vec![],
        use_9px: false,
        ice_colors: false,
        font: String::new(),
        palette: DEFAULT_EGA_PALETTE,
        title: String::new(),
        author: String::new(),
        group: String::new(),
        comments: String::new(),
    };

    // Simulate what icy_draw does: create a buffer with default 80x25
    let mut buffer = TextBuffer::new((80, 25));
    buffer.terminal_state.is_terminal_buffer = false;

    // Apply the remote document (mimics apply_remote_document)
    let cols_i32 = remote_doc.columns as i32;
    let rows_i32 = remote_doc.rows as i32;

    // Set document size
    buffer.set_size((cols_i32, rows_i32));
    buffer.layers[0].set_size((cols_i32, rows_i32));

    // Resize and preallocate layer 0 for fast bulk writes
    let layer = &mut buffer.layers[0];
    layer.preallocate_lines(cols_i32, rows_i32);

    for col in 0..(remote_doc.columns as usize) {
        for row in 0..(remote_doc.rows as usize) {
            let block = remote_doc.document.get(col).and_then(|c| c.get(row)).cloned().unwrap_or_default();

            let mut ch = AttributedChar::default();
            ch.ch = char::from_u32(block.code).unwrap_or(' ');
            ch.attribute.set_foreground(block.fg as u32);
            ch.attribute.set_background(block.bg as u32);

            layer.set_char_unchecked(Position::new(col as i32, row as i32), ch);
        }
    }

    // Now verify consistency
    assert_eq!(buffer.width(), 80, "Buffer width should be 80");
    assert_eq!(buffer.height(), 50, "Buffer height should be 50");

    assert_eq!(buffer.layers[0].size().width, 80, "Layer 0 width should be 80");
    assert_eq!(buffer.layers[0].size().height, 50, "Layer 0 height should be 50");

    // Most importantly: the actual line count should match!
    assert_eq!(buffer.layers[0].lines.len(), 50, "Layer 0 should have 50 lines allocated");

    // Verify we can access the last row
    let last_char = buffer.char_at(Position::new(0, 49));
    assert_eq!(last_char.ch, 'X', "Should be able to read character at row 49");
}

/// Test that a Moebius CONNECTED response with 50 rows is correctly parsed.
/// This tests the full deserialization pipeline.
#[test]
fn connected_response_50_rows_parsing() {
    // Simulate compressed data for 80x50 document (4000 blocks)
    // All blocks are the same: code=32 (space), fg=7, bg=0
    let compressed = MoebiusCompressedData {
        code: vec![[32, 3999]], // one run of 4000 spaces
        fg: vec![[7, 3999]],    // one run of 4000 foreground=7
        bg: vec![[0, 3999]],    // one run of 4000 background=0
    };

    let moebius_doc = MoebiusDoc {
        columns: 80,
        rows: 50, // 50 rows, not 25!
        title: "Test".to_string(),
        author: "Author".to_string(),
        group: "Group".to_string(),
        date: "".to_string(),
        palette: serde_json::Value::Null,
        font_name: "IBM VGA".to_string(),
        ice_colors: false,
        use_9px_font: false,
        comments: "".to_string(),
        c64_background: None,
        compressed_data: Some(compressed),
        data: None,
    };

    let result = moebius_doc.into_connected_document(1, vec![]).unwrap();

    assert_eq!(result.columns, 80, "Document columns should be 80");
    assert_eq!(result.rows, 50, "Document rows should be 50");
    assert_eq!(result.document.len(), 80, "Document should have 80 columns");
    assert_eq!(result.document[0].len(), 50, "Each column should have 50 rows");
}
