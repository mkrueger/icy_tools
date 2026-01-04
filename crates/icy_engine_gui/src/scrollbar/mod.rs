//! Scrollbar widgets and state management
//!
//! This module provides macOS-style overlay scrollbars with animation.

pub mod state;
pub use state::*;

// Overlay scrollbars are deprecated - use scroll_area with show_viewport instead
// Keeping source files for reference/helper methods
// pub mod overlay;
// pub use overlay::{ScrollbarOverlay, ScrollbarOverlayCallback, ScrollbarOverlayState, ViewportAccess};

// pub mod horizontal_overlay;
// pub use horizontal_overlay::{HorizontalScrollbarOverlay, HorizontalScrollbarOverlayCallback, HorizontalScrollbarOverlayState};

// pub mod info;
// pub use info::*;

use icy_ui::Element;

/// Deprecated: Wrap content with scrollbar overlays
/// This is now a no-op - use scroll_area with show_viewport instead for native scrollbars
#[deprecated(note = "Use scroll_area with show_viewport instead")]
pub fn wrap_with_scrollbars<'a, Message: 'a>(
    content: Element<'a, Message>,
    _needs_vscrollbar: bool,
    _needs_hscrollbar: bool,
) -> Element<'a, Message> {
    // Just return content - scrollbars should be handled by scroll_area now
    content
}
