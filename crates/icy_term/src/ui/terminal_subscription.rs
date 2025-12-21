//! Terminal event subscription for iced
//!
//! Provides an async Subscription that wraps the terminal event receiver
//! and delivers events without polling, saving CPU.

use iced::Subscription;
use iced::futures::SinkExt;
use iced::futures::channel::mpsc::Sender;
use iced::stream::channel;
use std::sync::OnceLock;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::mcp::McpCommand;
use crate::terminal::terminal_thread::TerminalEvent;

/// Pending terminal receivers waiting to be connected to subscriptions
/// Key: window id, Value: receiver
static PENDING_RECEIVERS: OnceLock<std::sync::Mutex<Vec<(usize, UnboundedReceiver<TerminalEvent>)>>> = OnceLock::new();

fn get_pending_receivers() -> &'static std::sync::Mutex<Vec<(usize, UnboundedReceiver<TerminalEvent>)>> {
    PENDING_RECEIVERS.get_or_init(|| std::sync::Mutex::new(Vec::new()))
}

/// Register a terminal receiver for a window
pub fn register_terminal_receiver(window_id: usize, rx: UnboundedReceiver<TerminalEvent>) {
    if let Ok(mut pending) = get_pending_receivers().lock() {
        pending.push((window_id, rx));
    }
}

/// Take a pending receiver for a specific window
fn take_pending_receiver(window_id: usize) -> Option<UnboundedReceiver<TerminalEvent>> {
    if let Ok(mut pending) = get_pending_receivers().lock() {
        if let Some(pos) = pending.iter().position(|(id, _)| *id == window_id) {
            return Some(pending.remove(pos).1);
        }
    }
    None
}

/// Create a subscription for terminal events for a specific window
/// Uses window_id as the subscription identifier for deduplication
pub fn terminal_events(window_id: usize) -> Subscription<(usize, TerminalEvent)> {
    // Use run_with with window_id as the hashable data for subscription identity
    Subscription::run_with(window_id, |id: &usize| {
        let window_id = *id;
        channel(100, move |output: Sender<(usize, TerminalEvent)>| async move {
            run_terminal_subscription(window_id, output).await;
        })
    })
}

async fn run_terminal_subscription(window_id: usize, mut output: Sender<(usize, TerminalEvent)>) {
    // Try to get the receiver for this window
    // We poll a bit because the subscription may start before the receiver is registered
    let mut rx = None;
    for _ in 0..100 {
        if let Some(receiver) = take_pending_receiver(window_id) {
            rx = Some(receiver);
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    let Some(mut rx) = rx else {
        log::warn!("No terminal receiver found for window {}", window_id);
        // Keep subscription alive
        std::future::pending::<()>().await;
        return;
    };

    // Forward all events from the receiver
    while let Some(event) = rx.recv().await {
        if output.send((window_id, event)).await.is_err() {
            // Receiver dropped, exit
            break;
        }
    }

    // Keep the subscription alive for cleanup
    std::future::pending::<()>().await;
}

// ============================================================================
// MCP Subscription (single global subscription)
// ============================================================================

use std::sync::Arc;

/// Pending MCP receiver (only one, for the first window)
static PENDING_MCP_RECEIVER: OnceLock<std::sync::Mutex<Option<UnboundedReceiver<McpCommand>>>> = OnceLock::new();

fn get_pending_mcp_receiver() -> &'static std::sync::Mutex<Option<UnboundedReceiver<McpCommand>>> {
    PENDING_MCP_RECEIVER.get_or_init(|| std::sync::Mutex::new(None))
}

/// Register the MCP receiver (called once at startup if MCP is enabled)
pub fn register_mcp_receiver(rx: UnboundedReceiver<McpCommand>) {
    if let Ok(mut pending) = get_pending_mcp_receiver().lock() {
        *pending = Some(rx);
    }
}

/// Take the pending MCP receiver
fn take_mcp_receiver() -> Option<UnboundedReceiver<McpCommand>> {
    if let Ok(mut pending) = get_pending_mcp_receiver().lock() {
        pending.take()
    } else {
        None
    }
}

/// Check if MCP receiver is registered
pub fn has_mcp_receiver() -> bool {
    if let Ok(pending) = get_pending_mcp_receiver().lock() {
        pending.is_some()
    } else {
        false
    }
}

/// Create a subscription for MCP commands (single global subscription)
/// Returns Arc<McpCommand> for efficient cloning
pub fn mcp_events() -> Subscription<Arc<McpCommand>> {
    // Use Subscription::run for the single MCP subscription
    Subscription::run(mcp_stream)
}

fn mcp_stream() -> impl iced::futures::Stream<Item = Arc<McpCommand>> {
    channel(100, move |output: Sender<Arc<McpCommand>>| async move {
        run_mcp_subscription(output).await;
    })
}

async fn run_mcp_subscription(mut output: Sender<Arc<McpCommand>>) {
    // Try to get the MCP receiver
    let mut rx = None;
    for _ in 0..100 {
        if let Some(receiver) = take_mcp_receiver() {
            rx = Some(receiver);
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    let Some(mut rx) = rx else {
        log::debug!("No MCP receiver registered");
        // Keep subscription alive
        std::future::pending::<()>().await;
        return;
    };

    log::info!("MCP subscription started");

    // Forward all MCP commands wrapped in Arc
    while let Some(cmd) = rx.recv().await {
        if output.send(Arc::new(cmd)).await.is_err() {
            // Receiver dropped, exit
            break;
        }
    }

    // Keep the subscription alive for cleanup
    std::future::pending::<()>().await;
}
