//! Scrollbar widgets and state management
//!
//! This module provides macOS-style overlay scrollbars with animation.

pub mod state;
pub use state::*;

pub mod overlay;
pub use overlay::{ScrollbarOverlay, ScrollbarOverlayCallback, ScrollbarOverlayState, ViewportAccess};

pub mod horizontal_overlay;
pub use horizontal_overlay::{HorizontalScrollbarOverlay, HorizontalScrollbarOverlayCallback, HorizontalScrollbarOverlayState};

pub mod info;
pub use info::*;
