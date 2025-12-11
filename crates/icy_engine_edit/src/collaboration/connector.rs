//! Collaboration connector for EditState.
//!
//! This module provides the integration layer between EditState and the
//! collaboration protocol. It handles:
//!
//! - Converting local edits to network messages
//! - Applying remote edits to the local buffer
//! - Managing undo/redo with network synchronization
//!
//! # Undo Behavior
//!
//! Following the Moebius approach: when undoing locally, we send DRAW messages
//! for all affected cells rather than an UNDO message. This ensures all clients
//! stay in sync and avoids complex conflict resolution.

use tokio::sync::mpsc;

use super::protocol::{Block, DrawMessage};

/// Events emitted by the connector for network transmission.
#[derive(Debug, Clone)]
pub enum ConnectorEvent {
    /// A character was drawn (send to network)
    Draw { col: i32, row: i32, block: Block, layer: Option<usize> },
    /// Multiple characters were drawn (e.g., during undo)
    DrawBatch(Vec<DrawMessage>),
    /// Cursor moved
    Cursor { col: i32, row: i32 },
    /// Selection changed
    Selection { selecting: bool, col: i32, row: i32 },
    /// Canvas resized
    Resize { columns: u32, rows: u32 },
}

/// Configuration for the collaboration connector.
#[derive(Debug, Clone, Default)]
pub struct ConnectorConfig {
    /// Whether to send preview draws (temporary, not saved)
    pub send_previews: bool,
    /// Layer index for operations (None = layer 0, Moebius compatible)
    pub active_layer: Option<usize>,
    /// Debounce cursor updates (ms)
    pub cursor_debounce_ms: u64,
}

/// Collaboration connector that bridges EditState and network.
///
/// This is designed to be used alongside EditState, intercepting changes
/// and converting them to network messages.
pub struct CollaborationConnector {
    /// Configuration
    config: ConnectorConfig,
    /// Outgoing event channel
    event_tx: mpsc::Sender<ConnectorEvent>,
    /// Whether currently connected
    connected: bool,
    /// Last cursor position sent (for debouncing)
    last_cursor: Option<(i32, i32)>,
}

impl CollaborationConnector {
    /// Create a new collaboration connector.
    pub fn new(config: ConnectorConfig) -> (Self, mpsc::Receiver<ConnectorEvent>) {
        let (event_tx, event_rx) = mpsc::channel(256);

        let connector = Self {
            config,
            event_tx,
            connected: false,
            last_cursor: None,
        };

        (connector, event_rx)
    }

    /// Mark as connected to a session.
    pub fn set_connected(&mut self, connected: bool) {
        self.connected = connected;
    }

    /// Check if connected.
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// Set the active layer for operations.
    pub fn set_active_layer(&mut self, layer: Option<usize>) {
        self.config.active_layer = layer;
    }

    /// Notify that a character was drawn.
    ///
    /// Call this after a successful set_char operation on the buffer.
    pub fn on_char_drawn(&self, col: i32, row: i32, code: u32, fg: u8, bg: u8) {
        if !self.connected {
            return;
        }

        let block = Block { code, fg, bg };
        let event = ConnectorEvent::Draw {
            col,
            row,
            block,
            layer: self.config.active_layer,
        };

        // Use try_send to avoid blocking
        let _ = self.event_tx.try_send(event);
    }

    /// Notify that multiple characters were drawn (e.g., during undo/redo).
    ///
    /// This is used when undoing an operation - we send individual DRAW
    /// messages for each affected cell, following the Moebius approach.
    pub fn on_chars_drawn(&self, changes: Vec<(i32, i32, u32, u8, u8)>) {
        if !self.connected || changes.is_empty() {
            return;
        }

        let messages: Vec<DrawMessage> = changes
            .into_iter()
            .map(|(col, row, code, fg, bg)| {
                let block = Block { code, fg, bg };
                DrawMessage {
                    action: 8, // Draw
                    col,
                    row,
                    block,
                    layer: self.config.active_layer,
                }
            })
            .collect();

        let _ = self.event_tx.try_send(ConnectorEvent::DrawBatch(messages));
    }

    /// Notify cursor position change.
    pub fn on_cursor_moved(&mut self, col: i32, row: i32) {
        if !self.connected {
            return;
        }

        // Simple debouncing: only send if position changed
        if self.last_cursor == Some((col, row)) {
            return;
        }
        self.last_cursor = Some((col, row));

        let _ = self.event_tx.try_send(ConnectorEvent::Cursor { col, row });
    }

    /// Notify selection change.
    pub fn on_selection_changed(&self, selecting: bool, col: i32, row: i32) {
        if !self.connected {
            return;
        }

        let _ = self.event_tx.try_send(ConnectorEvent::Selection { selecting, col, row });
    }

    /// Notify canvas resize.
    pub fn on_resize(&self, columns: u32, rows: u32) {
        if !self.connected {
            return;
        }

        let _ = self.event_tx.try_send(ConnectorEvent::Resize { columns, rows });
    }
}

/// Helper to convert AttributedChar to Block.
pub fn attributed_char_to_block(ch: &crate::AttributedChar) -> Block {
    Block {
        code: ch.ch as u32,
        fg: ch.attribute.foreground() as u8,
        bg: ch.attribute.background() as u8,
    }
}

/// Helper to convert Block to AttributedChar.
pub fn block_to_attributed_char(block: &Block) -> crate::AttributedChar {
    let mut attr = crate::TextAttribute::default();
    attr.set_foreground(block.fg as u32);
    attr.set_background(block.bg as u32);
    crate::AttributedChar::new(char::from_u32(block.code).unwrap_or(' '), attr)
}

/// Apply a remote draw event to an EditState buffer.
///
/// This bypasses the undo system since remote changes shouldn't be undoable locally.
pub fn apply_remote_draw(buffer: &mut crate::TextBuffer, col: i32, row: i32, block: &Block, layer: Option<usize>) {
    let ch = block_to_attributed_char(block);
    let pos = crate::Position::new(col, row);
    let layer_idx = layer.unwrap_or(0);

    if layer_idx < buffer.layers.len() {
        buffer.layers[layer_idx].set_char(pos, ch);
    }
}

/// Apply multiple remote draw events.
pub fn apply_remote_draws(buffer: &mut crate::TextBuffer, draws: &[DrawMessage]) {
    for draw in draws {
        apply_remote_draw(buffer, draw.col, draw.row, &draw.block, draw.layer);
    }
}

#[cfg(test)]
mod tests {
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
}
