//! Real-time collaboration module for icy_engine_edit.
//!
//! This module provides Moebius-compatible real-time collaboration functionality,
//! allowing multiple users to edit ANSI art documents simultaneously.
//!
//! # Protocol Compatibility
//!
//! The implementation is bidirectionally compatible with Moebius:
//! - Can connect to Moebius servers
//! - Can host sessions that Moebius clients can join
//!
//! # Protocol Versioning
//!
//! - Version 1 (default, Moebius-compatible): Single layer support only
//! - Version 2+: Extended features including layer support
//!
//! Moebius clients ignore unknown fields, so we use an optional `protocol_version`
//! field in the CONNECTED message for feature negotiation.

// Re-export types needed by test modules
pub use icy_engine_edit::collaboration::*;
pub use icy_engine_edit::{EditorUndoOp, EditorUndoStack, Size};
pub use serde_json::Value;
pub use std::sync::Arc;
pub use tokio::sync::mpsc;

/// Default EGA palette for tests
pub const DEFAULT_EGA_PALETTE: [[u8; 3]; 16] = [
    [0, 0, 0],       // 0 - Black
    [0, 0, 170],     // 1 - Blue
    [0, 170, 0],     // 2 - Green
    [0, 170, 170],   // 3 - Cyan
    [170, 0, 0],     // 4 - Red
    [170, 0, 170],   // 5 - Magenta
    [170, 85, 0],    // 6 - Brown
    [170, 170, 170], // 7 - Light Gray
    [85, 85, 85],    // 8 - Dark Gray
    [85, 85, 255],   // 9 - Light Blue
    [85, 255, 85],   // 10 - Light Green
    [85, 255, 255],  // 11 - Light Cyan
    [255, 85, 85],   // 12 - Light Red
    [255, 85, 255],  // 13 - Light Magenta
    [255, 255, 85],  // 14 - Yellow
    [255, 255, 255], // 15 - White
];

mod client;
mod compression;
mod connector;
mod protocol;
mod server;
mod session;
mod state;
