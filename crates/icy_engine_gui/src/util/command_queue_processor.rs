use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;

use icy_engine::{Screen, ScreenSink};
use parking_lot::Mutex;

use super::QueuedCommand;

/// Maximum lock duration in milliseconds for screen updates
pub const MAX_LOCK_DURATION_MS: u64 = 10;

/// Result of async command processing
pub enum AsyncCommandResult {
    /// Command was handled, continue to next
    Handled,
    /// Command was not handled, needs screen processing
    NotHandled,
    /// GrabScreen was encountered during processing
    GrabScreen,
}

/// Trait for handling async commands that don't need screen access.
/// Implement this for your specific thread (TerminalThread, ViewThread).
pub trait AsyncCommandHandler {
    /// Try to process an async command. Returns the result of processing.
    fn try_handle_async(&mut self, cmd: &QueuedCommand) -> impl std::future::Future<Output = AsyncCommandResult> + Send;

    /// Called when GrabScreen is processed (for double-stepping).
    /// Returns the delay in milliseconds to apply, if any.
    fn on_grab_screen(&mut self) -> impl std::future::Future<Output = Option<u64>> + Send {
        async { None }
    }
}

/// Helper struct for processing command queues with granular locking.
///
/// This can be used by both TerminalThread and ViewThread to share
/// the common queue processing logic.
pub struct CommandQueueProcessor {
    /// The shared screen
    screen: Arc<Mutex<Box<dyn Screen>>>,
    /// The command queue
    command_queue: VecDeque<QueuedCommand>,
}

impl CommandQueueProcessor {
    pub fn new(screen: Arc<Mutex<Box<dyn Screen>>>) -> Self {
        Self {
            screen,
            command_queue: VecDeque::new(),
        }
    }

    /// Get mutable access to the command queue (for QueueingSink)
    pub fn queue_mut(&mut self) -> &mut VecDeque<QueuedCommand> {
        &mut self.command_queue
    }

    /// Get the shared screen reference
    pub fn screen(&self) -> &Arc<Mutex<Box<dyn Screen>>> {
        &self.screen
    }

    /// Replace the screen (for mode changes)
    pub fn set_screen(&mut self, screen: Arc<Mutex<Box<dyn Screen>>>) {
        self.screen = screen;
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.command_queue.is_empty()
    }

    /// Clear the command queue
    pub fn clear(&mut self) {
        self.command_queue.clear();
    }

    /// Process the command queue with granular locking.
    ///
    /// - Async commands are processed via the handler trait
    /// - Screen commands are batched with time-limited locks
    pub async fn process_queue<H: AsyncCommandHandler>(&mut self, handler: &mut H) {
        loop {
            // Get next command
            let Some(cmd) = self.command_queue.pop_front() else {
                break;
            };

            // Try to process as async command first
            match handler.try_handle_async(&cmd).await {
                AsyncCommandResult::Handled => continue,
                AsyncCommandResult::GrabScreen => {
                    // Handle grab screen with potential delay
                    if let Some(delay_ms) = handler.on_grab_screen().await {
                        tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                    }
                    continue;
                }
                AsyncCommandResult::NotHandled => {
                    // Fall through to screen processing
                }
            }

            // Process commands that need screen lock
            let mut had_grab_screen = false;
            {
                let lock_start = Instant::now();
                let mut screen = self.screen.lock();

                if let Some(editable) = screen.as_editable() {
                    let mut screen_sink = ScreenSink::new(editable);

                    // Process first command
                    had_grab_screen |= cmd.process_screen_command(&mut screen_sink);

                    // Process more commands while within time budget
                    while lock_start.elapsed().as_millis() < MAX_LOCK_DURATION_MS as u128 {
                        // Check if next command needs async processing (without removing)
                        match self.command_queue.front() {
                            None => break,
                            Some(cmd) if cmd.needs_async_processing() => break,
                            _ => {}
                        }

                        // Safe to pop - we know it exists and doesn't need async
                        let next_cmd = self.command_queue.pop_front().unwrap();

                        had_grab_screen |= next_cmd.process_screen_command(&mut screen_sink);

                        // Break early on GrabScreen for double-stepping handlers
                        if had_grab_screen {
                            break;
                        }
                    }

                    // Update hyperlinks before releasing lock
                    editable.update_hyperlinks();
                }
            }

            // Notify handler about GrabScreen (for double-stepping)
            if had_grab_screen {
                if let Some(delay_ms) = handler.on_grab_screen().await {
                    tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                }
            }
        }
    }
}
