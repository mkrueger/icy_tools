use super::*;
use serde_json::json;
use std::net::SocketAddr;
use tokio::time::{Duration, timeout};

// ============================================================================
// Helper functions for broadcast testing
// ============================================================================

/// Helper struct for a connected test user
struct TestUser {
    id: UserId,
    nick: String,
    tx: mpsc::Sender<String>,
    rx: mpsc::Receiver<String>,
}

/// Set up a server with 3 connected registered users.
/// Returns (ServerState, user1, user2, user3).
async fn setup_server_with_3_users() -> (Arc<ServerState>, TestUser, TestUser, TestUser) {
    let state = ServerState::new(ServerConfig::default());
    let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();

    // Connect user 1
    let (tx1, mut rx1) = mpsc::channel::<String>(32);
    let mut id1: Option<UserId> = None;
    let mut nick1 = String::new();
    let c1 = json!({"type": 0, "data": {"nick": "User1", "group": "TestGroup", "pass": ""}}).to_string();
    handle_message(&state, &tx1, &mut id1, &mut nick1, &c1, addr).await.unwrap();
    let _ = recv_json(&mut rx1).await; // CONNECTED

    // Connect user 2
    let (tx2, mut rx2) = mpsc::channel::<String>(32);
    let mut id2: Option<UserId> = None;
    let mut nick2 = String::new();
    let c2 = json!({"type": 0, "data": {"nick": "User2", "group": "TestGroup", "pass": ""}}).to_string();
    handle_message(&state, &tx2, &mut id2, &mut nick2, &c2, addr).await.unwrap();
    let _ = recv_json(&mut rx2).await; // CONNECTED
    let _ = recv_json(&mut rx1).await; // JOIN for user2

    // Connect user 3
    let (tx3, mut rx3) = mpsc::channel::<String>(32);
    let mut id3: Option<UserId> = None;
    let mut nick3 = String::new();
    let c3 = json!({"type": 0, "data": {"nick": "User3", "group": "TestGroup", "pass": ""}}).to_string();
    handle_message(&state, &tx3, &mut id3, &mut nick3, &c3, addr).await.unwrap();
    let _ = recv_json(&mut rx3).await; // CONNECTED
    let _ = recv_json(&mut rx1).await; // JOIN for user3
    let _ = recv_json(&mut rx2).await; // JOIN for user3

    let user1 = TestUser {
        id: id1.unwrap(),
        nick: nick1,
        tx: tx1,
        rx: rx1,
    };
    let user2 = TestUser {
        id: id2.unwrap(),
        nick: nick2,
        tx: tx2,
        rx: rx2,
    };
    let user3 = TestUser {
        id: id3.unwrap(),
        nick: nick3,
        tx: tx3,
        rx: rx3,
    };

    (state, user1, user2, user3)
}

/// Receive JSON from channel with timeout.
async fn recv_json(rx: &mut mpsc::Receiver<String>) -> serde_json::Value {
    let msg = timeout(Duration::from_millis(200), rx.recv())
        .await
        .expect("timeout waiting for server message")
        .expect("channel closed");
    serde_json::from_str(&msg).expect("valid json")
}

/// Assert that no message is received within timeout.
async fn expect_no_message(rx: &mut mpsc::Receiver<String>) {
    let res = timeout(Duration::from_millis(50), rx.recv()).await;
    assert!(res.is_err(), "expected no message, but received one");
}

// ============================================================================
// Original unit tests
// ============================================================================

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
    let result = state.handle_connect("User".to_string(), "wrong".to_string(), false).await;
    assert!(result.is_err());

    // Correct password should succeed
    let result = state.handle_connect("User".to_string(), "secret".to_string(), false).await;
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

// ============================================================================
// Broadcast tests: verify messages are correctly forwarded to other users
// ============================================================================

#[tokio::test]
async fn test_broadcast_cursor() {
    let (state, mut user1, mut user2, mut user3) = setup_server_with_3_users().await;
    let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();

    // User 2 sends CURSOR
    let msg = r#"{"type":4,"data":{"x":18,"y":13,"id":2}}"#;
    handle_message(&state, &user2.tx, &mut Some(user2.id), &mut user2.nick, msg, addr)
        .await
        .unwrap();

    // User 1 and User 3 should receive the cursor update
    let received1 = recv_json(&mut user1.rx).await;
    assert_eq!(received1.get("type").and_then(|v| v.as_u64()), Some(4));
    let data1 = received1.get("data").unwrap();
    assert_eq!(data1.get("x").and_then(|v| v.as_i64()), Some(18));
    assert_eq!(data1.get("y").and_then(|v| v.as_i64()), Some(13));

    let received3 = recv_json(&mut user3.rx).await;
    assert_eq!(received3.get("type").and_then(|v| v.as_u64()), Some(4));

    // Sender should NOT receive their own message
    expect_no_message(&mut user2.rx).await;
}

#[tokio::test]
async fn test_broadcast_selection() {
    let (state, mut user1, mut user2, mut user3) = setup_server_with_3_users().await;
    let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();

    // User 2 sends SELECTION
    let msg = r#"{"type":5,"data":{"x":32,"y":11,"id":2}}"#;
    handle_message(&state, &user2.tx, &mut Some(user2.id), &mut user2.nick, msg, addr)
        .await
        .unwrap();

    // User 1 and User 3 should receive the selection update
    let received1 = recv_json(&mut user1.rx).await;
    assert_eq!(received1.get("type").and_then(|v| v.as_u64()), Some(5));
    let data1 = received1.get("data").unwrap();
    assert_eq!(data1.get("x").and_then(|v| v.as_i64()), Some(32));
    assert_eq!(data1.get("y").and_then(|v| v.as_i64()), Some(11));

    let received3 = recv_json(&mut user3.rx).await;
    assert_eq!(received3.get("type").and_then(|v| v.as_u64()), Some(5));

    // Sender should NOT receive their own message
    expect_no_message(&mut user2.rx).await;
}

#[tokio::test]
async fn test_broadcast_hide_cursor() {
    let (state, mut user1, mut user2, mut user3) = setup_server_with_3_users().await;
    let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();

    // User 2 sends HIDE_CURSOR
    let msg = r#"{"type":8,"data":{"id":2}}"#;
    handle_message(&state, &user2.tx, &mut Some(user2.id), &mut user2.nick, msg, addr)
        .await
        .unwrap();

    // User 1 and User 3 should receive hide cursor
    let received1 = recv_json(&mut user1.rx).await;
    assert_eq!(received1.get("type").and_then(|v| v.as_u64()), Some(8));

    let received3 = recv_json(&mut user3.rx).await;
    assert_eq!(received3.get("type").and_then(|v| v.as_u64()), Some(8));

    // Sender should NOT receive their own message
    expect_no_message(&mut user2.rx).await;
}

#[tokio::test]
async fn test_broadcast_draw() {
    let (state, mut user1, mut user2, mut user3) = setup_server_with_3_users().await;
    let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();

    // User 2 sends DRAW
    let msg = r#"{"type":9,"data":{"x":10,"y":12,"block":{"code":101,"fg":7,"bg":0},"id":2}}"#;
    handle_message(&state, &user2.tx, &mut Some(user2.id), &mut user2.nick, msg, addr)
        .await
        .unwrap();

    // User 1 and User 3 should receive the draw update
    let received1 = recv_json(&mut user1.rx).await;
    assert_eq!(received1.get("type").and_then(|v| v.as_u64()), Some(9));
    let data1 = received1.get("data").unwrap();
    assert_eq!(data1.get("x").and_then(|v| v.as_i64()), Some(10));
    assert_eq!(data1.get("y").and_then(|v| v.as_i64()), Some(12));
    let block = data1.get("block").unwrap();
    assert_eq!(block.get("code").and_then(|v| v.as_u64()), Some(101));
    assert_eq!(block.get("fg").and_then(|v| v.as_u64()), Some(7));
    assert_eq!(block.get("bg").and_then(|v| v.as_u64()), Some(0));

    let received3 = recv_json(&mut user3.rx).await;
    assert_eq!(received3.get("type").and_then(|v| v.as_u64()), Some(9));

    // Sender should NOT receive their own message
    expect_no_message(&mut user2.rx).await;
}

#[tokio::test]
async fn test_broadcast_chat() {
    let (state, mut user1, mut user2, mut user3) = setup_server_with_3_users().await;
    let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();

    // User 2 sends CHAT
    let msg = r#"{"type":10,"data":{"nick":"Test User","group":"Mafia","text":"hello","id":2}}"#;
    handle_message(&state, &user2.tx, &mut Some(user2.id), &mut user2.nick, msg, addr)
        .await
        .unwrap();

    // User 1 and User 3 should receive the chat message
    let received1 = recv_json(&mut user1.rx).await;
    assert_eq!(received1.get("type").and_then(|v| v.as_u64()), Some(10));
    let data1 = received1.get("data").unwrap();
    assert_eq!(data1.get("text").and_then(|v| v.as_str()), Some("hello"));
    // Nick may be updated from message or remain as registered
    assert!(data1.get("nick").and_then(|v| v.as_str()).is_some());
    assert!(data1.get("group").and_then(|v| v.as_str()).is_some());

    let received3 = recv_json(&mut user3.rx).await;
    assert_eq!(received3.get("type").and_then(|v| v.as_u64()), Some(10));

    // Sender should NOT receive their own message
    expect_no_message(&mut user2.rx).await;
}

#[tokio::test]
async fn test_broadcast_sauce() {
    let (state, mut user1, mut user2, mut user3) = setup_server_with_3_users().await;
    let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();

    // User 2 sends SAUCE
    let msg = r#"{"type":12,"data":{"title":"title                              ","author":"author              ","group":"group               ","comments":"cmt1\ncmt2                                                       ","id":2}}"#;
    handle_message(&state, &user2.tx, &mut Some(user2.id), &mut user2.nick, msg, addr)
        .await
        .unwrap();

    // User 1 and User 3 should receive the sauce update
    let received1 = recv_json(&mut user1.rx).await;
    assert_eq!(received1.get("type").and_then(|v| v.as_u64()), Some(12));
    let data1 = received1.get("data").unwrap();
    assert!(data1.get("title").and_then(|v| v.as_str()).unwrap().starts_with("title"));
    assert!(data1.get("author").and_then(|v| v.as_str()).unwrap().starts_with("author"));

    let received3 = recv_json(&mut user3.rx).await;
    assert_eq!(received3.get("type").and_then(|v| v.as_u64()), Some(12));

    // Sender should NOT receive their own message
    expect_no_message(&mut user2.rx).await;
}

#[tokio::test]
async fn test_broadcast_ice_colors() {
    let (state, mut user1, mut user2, mut user3) = setup_server_with_3_users().await;
    let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();

    // User 2 sends ICE_COLORS
    let msg = r#"{"type":13,"data":{"value":true,"id":2}}"#;
    handle_message(&state, &user2.tx, &mut Some(user2.id), &mut user2.nick, msg, addr)
        .await
        .unwrap();

    // User 1 and User 3 should receive the ice colors update
    let received1 = recv_json(&mut user1.rx).await;
    assert_eq!(received1.get("type").and_then(|v| v.as_u64()), Some(13));
    let data1 = received1.get("data").unwrap();
    assert_eq!(data1.get("value").and_then(|v| v.as_bool()), Some(true));

    let received3 = recv_json(&mut user3.rx).await;
    assert_eq!(received3.get("type").and_then(|v| v.as_u64()), Some(13));

    // Sender should NOT receive their own message
    expect_no_message(&mut user2.rx).await;
}

#[tokio::test]
async fn test_broadcast_use_9px_font() {
    let (state, mut user1, mut user2, mut user3) = setup_server_with_3_users().await;
    let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();

    // User 2 sends USE_9PX_FONT
    let msg = r#"{"type":14,"data":{"value":true,"id":2}}"#;
    handle_message(&state, &user2.tx, &mut Some(user2.id), &mut user2.nick, msg, addr)
        .await
        .unwrap();

    // User 1 and User 3 should receive the 9px font update
    let received1 = recv_json(&mut user1.rx).await;
    assert_eq!(received1.get("type").and_then(|v| v.as_u64()), Some(14));
    let data1 = received1.get("data").unwrap();
    assert_eq!(data1.get("value").and_then(|v| v.as_bool()), Some(true));

    let received3 = recv_json(&mut user3.rx).await;
    assert_eq!(received3.get("type").and_then(|v| v.as_u64()), Some(14));

    // Sender should NOT receive their own message
    expect_no_message(&mut user2.rx).await;
}

#[tokio::test]
async fn test_broadcast_change_font() {
    let (state, mut user1, mut user2, mut user3) = setup_server_with_3_users().await;
    let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();

    // User 2 sends CHANGE_FONT
    let msg = r#"{"type":15,"data":{"font_name":"IBM EGA 866","id":2}}"#;
    handle_message(&state, &user2.tx, &mut Some(user2.id), &mut user2.nick, msg, addr)
        .await
        .unwrap();

    // User 1 and User 3 should receive the font change
    let received1 = recv_json(&mut user1.rx).await;
    assert_eq!(received1.get("type").and_then(|v| v.as_u64()), Some(15));
    let data1 = received1.get("data").unwrap();
    assert_eq!(data1.get("font_name").and_then(|v| v.as_str()), Some("IBM EGA 866"));

    let received3 = recv_json(&mut user3.rx).await;
    assert_eq!(received3.get("type").and_then(|v| v.as_u64()), Some(15));

    // Sender should NOT receive their own message
    expect_no_message(&mut user2.rx).await;
}

#[tokio::test]
async fn test_broadcast_set_canvas_size() {
    let (state, mut user1, mut user2, mut user3) = setup_server_with_3_users().await;
    let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();

    // User 2 sends SET_CANVAS_SIZE
    let msg = r#"{"type":16,"data":{"columns":80,"rows":50,"id":2}}"#;
    handle_message(&state, &user2.tx, &mut Some(user2.id), &mut user2.nick, msg, addr)
        .await
        .unwrap();

    // User 1 and User 3 should receive the canvas size update
    let received1 = recv_json(&mut user1.rx).await;
    assert_eq!(received1.get("type").and_then(|v| v.as_u64()), Some(16));
    let data1 = received1.get("data").unwrap();
    assert_eq!(data1.get("columns").and_then(|v| v.as_u64()), Some(80));
    assert_eq!(data1.get("rows").and_then(|v| v.as_u64()), Some(50));

    let received3 = recv_json(&mut user3.rx).await;
    assert_eq!(received3.get("type").and_then(|v| v.as_u64()), Some(16));

    // Sender should NOT receive their own message
    expect_no_message(&mut user2.rx).await;
}

#[tokio::test]
async fn test_broadcast_paste_as_selection() {
    let (state, mut user1, mut user2, mut user3) = setup_server_with_3_users().await;
    let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();

    // User 2 sends PASTE_AS_SELECTION
    let msg = r#"{"type":17,"data":{"blocks":{"columns":4,"rows":1,"data":[{"code":116,"fg":7,"bg":0},{"code":101,"fg":7,"bg":0},{"code":115,"fg":7,"bg":0},{"code":116,"fg":7,"bg":0}],"is_move_operation":false},"id":2}}"#;
    handle_message(&state, &user2.tx, &mut Some(user2.id), &mut user2.nick, msg, addr)
        .await
        .unwrap();

    // User 1 and User 3 should receive the paste
    let received1 = recv_json(&mut user1.rx).await;
    assert_eq!(received1.get("type").and_then(|v| v.as_u64()), Some(17));
    let data1 = received1.get("data").unwrap();
    let blocks = data1.get("blocks").unwrap();
    assert_eq!(blocks.get("columns").and_then(|v| v.as_u64()), Some(4));
    assert_eq!(blocks.get("rows").and_then(|v| v.as_u64()), Some(1));

    let received3 = recv_json(&mut user3.rx).await;
    assert_eq!(received3.get("type").and_then(|v| v.as_u64()), Some(17));

    // Sender should NOT receive their own message
    expect_no_message(&mut user2.rx).await;
}

#[tokio::test]
async fn test_broadcast_rotate() {
    let (state, mut user1, mut user2, mut user3) = setup_server_with_3_users().await;
    let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();

    // User 2 sends ROTATE
    let msg = r#"{"type":18,"data":{"id":2}}"#;
    handle_message(&state, &user2.tx, &mut Some(user2.id), &mut user2.nick, msg, addr)
        .await
        .unwrap();

    // User 1 and User 3 should receive the rotate command
    let received1 = recv_json(&mut user1.rx).await;
    assert_eq!(received1.get("type").and_then(|v| v.as_u64()), Some(18));

    let received3 = recv_json(&mut user3.rx).await;
    assert_eq!(received3.get("type").and_then(|v| v.as_u64()), Some(18));

    // Sender should NOT receive their own message
    expect_no_message(&mut user2.rx).await;
}

#[tokio::test]
async fn test_broadcast_flip_x() {
    let (state, mut user1, mut user2, mut user3) = setup_server_with_3_users().await;
    let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();

    // User 2 sends FLIP_X
    let msg = r#"{"type":19,"data":{"id":2}}"#;
    handle_message(&state, &user2.tx, &mut Some(user2.id), &mut user2.nick, msg, addr)
        .await
        .unwrap();

    // User 1 and User 3 should receive the flip x command
    let received1 = recv_json(&mut user1.rx).await;
    assert_eq!(received1.get("type").and_then(|v| v.as_u64()), Some(19));

    let received3 = recv_json(&mut user3.rx).await;
    assert_eq!(received3.get("type").and_then(|v| v.as_u64()), Some(19));

    // Sender should NOT receive their own message
    expect_no_message(&mut user2.rx).await;
}

#[tokio::test]
async fn test_broadcast_flip_y() {
    let (state, mut user1, mut user2, mut user3) = setup_server_with_3_users().await;
    let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();

    // User 2 sends FLIP_Y
    let msg = r#"{"type":20,"data":{"id":2}}"#;
    handle_message(&state, &user2.tx, &mut Some(user2.id), &mut user2.nick, msg, addr)
        .await
        .unwrap();

    // User 1 and User 3 should receive the flip y command
    let received1 = recv_json(&mut user1.rx).await;
    assert_eq!(received1.get("type").and_then(|v| v.as_u64()), Some(20));

    let received3 = recv_json(&mut user3.rx).await;
    assert_eq!(received3.get("type").and_then(|v| v.as_u64()), Some(20));

    // Sender should NOT receive their own message
    expect_no_message(&mut user2.rx).await;
}
