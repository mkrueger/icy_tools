//! Collaboration subscription for iced
//!
//! Provides an iced Subscription that wraps the collaboration client
//! and delivers events to the main application.

use icy_engine_edit::collaboration::{ClientConfig, ClientHandle, CollaborationEvent};
use icy_ui::futures::channel::mpsc::Sender;
use icy_ui::futures::SinkExt;
use icy_ui::stream::channel;
use icy_ui::Subscription;
use std::sync::OnceLock;

/// Message type for the collaboration subscription
#[derive(Debug, Clone)]
pub enum CollaborationMessage {
    /// Collaboration event from server
    Event(CollaborationEvent),
    /// Client handle is ready
    Ready(CollaborationClient),
}

/// Wrapper around ClientHandle that can be cloned and sent as a message
#[derive(Clone)]
pub struct CollaborationClient {
    handle: ClientHandle,
}

impl std::fmt::Debug for CollaborationClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CollaborationClient").field("nick", &self.handle.nick()).finish()
    }
}

impl CollaborationClient {
    /// Create new collaboration client wrapper
    pub fn new(handle: ClientHandle) -> Self {
        Self { handle }
    }

    /// Get the inner handle
    pub fn handle(&self) -> &ClientHandle {
        &self.handle
    }

    /// Send cursor position
    pub fn send_cursor(&self, col: i32, row: i32) {
        let handle = self.handle.clone();
        tokio::spawn(async move {
            let _ = handle.send_cursor(col, row).await;
        });
    }

    /// Send selection update
    pub fn send_selection(&self, selecting: bool, col: i32, row: i32) {
        let handle = self.handle.clone();
        tokio::spawn(async move {
            let _ = handle.send_selection(selecting, col, row).await;
        });
    }

    /// Draw a character
    pub fn draw(&self, col: i32, row: i32, block: icy_engine_edit::collaboration::Block) {
        let handle = self.handle.clone();
        tokio::spawn(async move {
            let _ = handle.draw(col, row, block).await;
        });
    }

    /// Draw a preview character
    pub fn draw_preview(&self, col: i32, row: i32, block: icy_engine_edit::collaboration::Block) {
        let handle = self.handle.clone();
        tokio::spawn(async move {
            let _ = handle.draw_preview(col, row, block).await;
        });
    }

    /// Send chat message
    pub fn send_chat(&self, text: String) {
        let handle = self.handle.clone();
        tokio::spawn(async move {
            let _ = handle.send_chat(text).await;
        });
    }

    /// Resize columns
    pub fn resize_columns(&self, columns: u32) {
        let handle = self.handle.clone();
        tokio::spawn(async move {
            let _ = handle.resize_columns(columns).await;
        });
    }

    /// Resize rows
    pub fn resize_rows(&self, rows: u32) {
        let handle = self.handle.clone();
        tokio::spawn(async move {
            let _ = handle.resize_rows(rows).await;
        });
    }

    /// Set 9px mode
    pub fn set_use_9px(&self, value: bool) {
        let handle = self.handle.clone();
        tokio::spawn(async move {
            let _ = handle.set_use_9px(value).await;
        });
    }

    /// Set ice colors
    pub fn set_ice_colors(&self, value: bool) {
        let handle = self.handle.clone();
        tokio::spawn(async move {
            let _ = handle.set_ice_colors(value).await;
        });
    }

    /// Set font
    pub fn set_font(&self, font: String) {
        let handle = self.handle.clone();
        tokio::spawn(async move {
            let _ = handle.set_font(font).await;
        });
    }

    /// Disconnect from server
    pub fn disconnect(&self) {
        let handle = self.handle.clone();
        tokio::spawn(async move {
            let _ = handle.disconnect().await;
        });
    }

    /// Get nickname
    pub fn nick(&self) -> &str {
        self.handle.nick()
    }
}

/// Global config storage for subscription (needed because run() takes fn pointer)
static PENDING_CONFIG: OnceLock<std::sync::Mutex<Option<ClientConfig>>> = OnceLock::new();

/// Set the config for the next connection
fn set_pending_config(config: ClientConfig) {
    let mutex = PENDING_CONFIG.get_or_init(|| std::sync::Mutex::new(None));
    *mutex.lock().unwrap() = Some(config);
}

/// Take the pending config
fn take_pending_config() -> Option<ClientConfig> {
    PENDING_CONFIG.get().and_then(|m| m.lock().ok()).and_then(|mut guard| guard.take())
}

/// Create a collaboration subscription that connects to a server
pub fn connect(config: ClientConfig) -> Subscription<CollaborationMessage> {
    set_pending_config(config);
    Subscription::run(collaboration_stream)
}

fn collaboration_stream() -> impl icy_ui::futures::Stream<Item = CollaborationMessage> {
    channel(100, |output: Sender<CollaborationMessage>| async move {
        if let Some(config) = take_pending_config() {
            run_collaboration(config, output).await;
        }
    })
}

async fn run_collaboration(config: ClientConfig, mut output: Sender<CollaborationMessage>) {
    // Connect to server
    match icy_engine_edit::collaboration::connect(config).await {
        Ok((handle, mut event_rx)) => {
            // Send the handle first
            let client = CollaborationClient::new(handle);
            let _ = output.send(CollaborationMessage::Ready(client)).await;

            // Then forward all events
            while let Some(event) = event_rx.recv().await {
                let is_disconnect = matches!(event, CollaborationEvent::Disconnected);
                let _ = output.send(CollaborationMessage::Event(event)).await;
                if is_disconnect {
                    break;
                }
            }
        }
        Err(e) => {
            // Send error event
            let _ = output.send(CollaborationMessage::Event(CollaborationEvent::Error(e))).await;
        }
    }

    // Keep the subscription alive for cleanup
    std::future::pending::<()>().await;
}
