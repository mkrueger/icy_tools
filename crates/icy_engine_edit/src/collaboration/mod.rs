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

mod client;
mod compression;
mod connector;
mod protocol;
mod server;
mod session;
mod state;

pub use client::*;
pub use compression::*;
pub use connector::*;
pub use protocol::*;
pub use server::*;
pub use session::*;
pub use state::*;
