//! Collaboration module for icy_draw
//!
//! This module provides real-time collaboration:
//! - WebSocket client connection to collaboration servers
//! - iced Subscription for async event handling
//! - Chat panel widget
//! - Remote cursor rendering
//! - Connection dialog

pub mod chat_panel;
pub mod icons;
pub mod state;
pub mod subscription;

pub use chat_panel::{ChatPanelMessage, view_chat_panel};
pub use icons::{Avatar, UserStatus};
pub use state::*;
pub use subscription::*;
