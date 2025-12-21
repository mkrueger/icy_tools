use super::*;
use serde_json::json;

#[tokio::test]
async fn parse_connected_includes_chat_history_and_users() {
    let msg = json!({
        "type": 0,
        "data": {
            "chat_history": [
                {"group":"","id":8,"nick":"user1","text":"Test1","time":1766111994776u64},
                {"group":"","id":8,"nick":"user2","text":"Test3","time":1766111995508u64},
                {"group":"","id":16,"nick":"user3","text":"Hallo","time":1766132397192u64},
                {"group":"","id":22,"nick":"Anonymous","text":"Hallo","time":1766136724422u64},
                {"group":"","id":22,"nick":"Anonymous","text":"Welt","time":1766136726268u64}
            ],
            "id": 24,
            "status": 0,
            "users": [
                {"group":"","id":0,"nick":"User","status":2}
            ],
            "doc": {
                "columns": 1,
                "rows": 1,
                "title": "",
                "author": "",
                "group": "",
                "date": "",
                "palette": [],
                "font_name": "IBM VGA",
                "ice_colors": true,
                "use_9px_font": false,
                "comments": "",
                "data": [
                    {"code": 65, "fg": 7, "bg": 0}
                ]
            }
        }
    });

    let mut assigned_id: Option<UserId> = None;
    let event = parse_server_message(&msg, "", &mut assigned_id).await.expect("expected event");

    assert_eq!(assigned_id, Some(24));

    match event {
        CollaborationEvent::Connected(doc) => {
            assert_eq!(doc.user_id, 24);
            assert_eq!(doc.users.len(), 1);
            assert_eq!(doc.users[0].id, 0);
            assert_eq!(doc.users[0].nick, "User");

            assert_eq!(doc.chat_history.len(), 5);
            assert_eq!(doc.chat_history[0].nick, "user1");
            assert_eq!(doc.chat_history[0].text, "Test1");
            assert_eq!(doc.chat_history[4].text, "Welt");
        }
        other => panic!("Expected Connected, got: {other:?}"),
    }
}

#[tokio::test]
async fn parse_paste_as_selection() {
    let msg = json!({
        "type": 17,
        "data": {
            "id": 42,
            "blocks": {
                "columns": 2,
                "rows": 1,
                "data": [
                    {"code": 65, "fg": 7, "bg": 0},
                    {"code": 66, "fg": 2, "bg": 1}
                ]
            }
        }
    });

    let mut assigned_id: Option<UserId> = None;
    let event = parse_server_message(&msg, "", &mut assigned_id).await.expect("expected event");
    assert_eq!(assigned_id, None);

    match event {
        CollaborationEvent::PasteAsSelection { user_id, blocks } => {
            assert_eq!(user_id, 42);
            assert_eq!(blocks.columns, 2);
            assert_eq!(blocks.rows, 1);
            assert_eq!(blocks.data.len(), 2);
            assert_eq!(blocks.data[0].code, 65);
        }
        other => panic!("Expected PasteAsSelection, got: {other:?}"),
    }
}

#[tokio::test]
async fn parse_rotate_flip_set_bg() {
    let mut assigned_id: Option<UserId> = None;

    let rotate = json!({"type": 18, "data": {"id": 7}});
    let flipx = json!({"type": 19, "data": {"id": 7}});
    let flipy = json!({"type": 20, "data": {"id": 7}});
    let set_bg = json!({"type": 21, "data": {"id": 7, "value": 3}});

    assert!(matches!(
        parse_server_message(&rotate, "", &mut assigned_id).await,
        Some(CollaborationEvent::Rotate { user_id: 7 })
    ));
    assert!(matches!(
        parse_server_message(&flipx, "", &mut assigned_id).await,
        Some(CollaborationEvent::FlipX { user_id: 7 })
    ));
    assert!(matches!(
        parse_server_message(&flipy, "", &mut assigned_id).await,
        Some(CollaborationEvent::FlipY { user_id: 7 })
    ));
    assert!(matches!(
        parse_server_message(&set_bg, "", &mut assigned_id).await,
        Some(CollaborationEvent::BackgroundChanged { user_id: 7, value: 3 })
    ));
}

#[test]
fn serialize_paste_as_selection() {
    let blocks = Blocks {
        columns: 1,
        rows: 1,
        data: vec![Block { code: 65, fg: 7, bg: 0 }],
    };

    let msg = command_to_message(ClientCommand::PasteAsSelection { blocks }, Some(9), "n", "").expect("expected message");

    let v: Value = serde_json::from_str(&msg).expect("valid json");
    assert_eq!(v.get("type").and_then(|x| x.as_u64()), Some(17));
    let data = v.get("data").expect("data");
    assert_eq!(data.get("id").and_then(|x| x.as_u64()), Some(9));
    let blocks = data.get("blocks").expect("blocks");
    assert_eq!(blocks.get("columns").and_then(|x| x.as_u64()), Some(1));
    assert_eq!(blocks.get("rows").and_then(|x| x.as_u64()), Some(1));
}

// ========================================================================
// Group in Protocol Tests
// ========================================================================

#[test]
fn client_config_includes_group() {
    let config = ClientConfig {
        url: "ws://localhost:8000".to_string(),
        nick: "TestUser".to_string(),
        group: "TestGroup".to_string(),
        password: "secret".to_string(),
        ping_interval_secs: 30,
    };

    assert_eq!(config.nick, "TestUser");
    assert_eq!(config.group, "TestGroup");
    assert_eq!(config.password, "secret");
}

#[test]
fn client_config_default_has_empty_group() {
    let config = ClientConfig::default();

    assert_eq!(config.nick, "Anonymous");
    assert_eq!(config.group, "");
}

#[test]
fn chat_message_includes_group() {
    let msg = command_to_message(
        ClientCommand::Chat {
            text: "Hello, world!".to_string(),
        },
        Some(42),
        "TestUser",
        "TestGroup",
    )
    .expect("expected message");

    let v: Value = serde_json::from_str(&msg).expect("valid json");
    assert_eq!(v.get("type").and_then(|x| x.as_u64()), Some(10)); // CHAT = 10
    let data = v.get("data").expect("data");
    assert_eq!(data.get("id").and_then(|x| x.as_u64()), Some(42));
    assert_eq!(data.get("nick").and_then(|x| x.as_str()), Some("TestUser"));
    assert_eq!(data.get("group").and_then(|x| x.as_str()), Some("TestGroup"));
    assert_eq!(data.get("text").and_then(|x| x.as_str()), Some("Hello, world!"));
}

#[test]
fn chat_message_includes_empty_group() {
    let msg = command_to_message(ClientCommand::Chat { text: "Hi".to_string() }, Some(1), "User", "").expect("expected message");

    let v: Value = serde_json::from_str(&msg).expect("valid json");
    let data = v.get("data").expect("data");
    assert_eq!(data.get("group").and_then(|x| x.as_str()), Some(""));
}

#[test]
fn status_message_does_not_include_group() {
    // Status messages use only id and status, not nick/group
    let msg = command_to_message(ClientCommand::SetStatus { status: 1 }, Some(42), "TestUser", "TestGroup").expect("expected message");

    let v: Value = serde_json::from_str(&msg).expect("valid json");
    assert_eq!(v.get("type").and_then(|x| x.as_u64()), Some(11)); // STATUS = 11
    let data = v.get("data").expect("data");
    assert_eq!(data.get("id").and_then(|x| x.as_u64()), Some(42));
    assert_eq!(data.get("status").and_then(|x| x.as_u64()), Some(1));
    // Status message should NOT include nick/group per Moebius protocol
    assert!(data.get("nick").is_none());
    assert!(data.get("group").is_none());
}
