use super::*;

#[test]
fn test_connector_creation() {
    let (connector, _rx) = CollaborationConnector::new(ConnectorConfig::default());
    assert!(!connector.is_connected());
}

#[test]
fn test_block_conversion() {
    let block = Block { code: 65, fg: 7, bg: 1 };
    let ch = block_to_attributed_char(&block);
    assert_eq!(ch.ch, 'A');

    let back = attributed_char_to_block(&ch);
    assert_eq!(back.code, 65);
    assert_eq!(back.fg, 7);
    assert_eq!(back.bg, 1);
}

#[tokio::test]
async fn test_connector_events() {
    let (mut connector, mut rx) = CollaborationConnector::new(ConnectorConfig::default());
    connector.set_connected(true);

    connector.on_char_drawn(10, 20, 65, 7, 0);

    let event = rx.recv().await.unwrap();
    match event {
        ConnectorEvent::Draw { col, row, block, .. } => {
            assert_eq!(col, 10);
            assert_eq!(row, 20);
            assert_eq!(block.code, 65);
        }
        _ => panic!("Expected Draw event"),
    }
}

#[tokio::test]
async fn test_cursor_debounce() {
    let (mut connector, mut rx) = CollaborationConnector::new(ConnectorConfig::default());
    connector.set_connected(true);

    // First cursor update should be sent
    connector.on_cursor_moved(10, 20);
    let _ = rx.recv().await.unwrap();

    // Same position should be debounced
    connector.on_cursor_moved(10, 20);

    // Different position should be sent
    connector.on_cursor_moved(15, 25);
    let event = rx.recv().await.unwrap();
    match event {
        ConnectorEvent::Cursor { col, row } => {
            assert_eq!(col, 15);
            assert_eq!(row, 25);
        }
        _ => panic!("Expected Cursor event"),
    }
}

#[test]
fn test_not_connected_no_events() {
    let (connector, mut rx) = CollaborationConnector::new(ConnectorConfig::default());
    // Not connected - should not send events
    connector.on_char_drawn(10, 20, 65, 7, 0);

    // Channel should be empty
    assert!(rx.try_recv().is_err());
}
