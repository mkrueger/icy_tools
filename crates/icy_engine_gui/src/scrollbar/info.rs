//! Scrollbar information and helper functions for Terminal-based views
//!
//! This module provides utilities for computing scrollbar state from Terminal
//! and creating scrollbar overlay elements.

use icy_ui::{
    widget::{container, stack},
    Alignment, Element, Length,
};

use super::{overlay::ViewportAccess, HorizontalScrollbarOverlay, ScrollbarOverlay};
use crate::Terminal;

/// Information needed to render scrollbars
/// Computed from Terminal state, shared between icy_term and icy_view
#[derive(Debug, Clone)]
pub struct ScrollbarInfo {
    /// Whether a vertical scrollbar is needed
    pub needs_vscrollbar: bool,
    /// Whether a horizontal scrollbar is needed
    pub needs_hscrollbar: bool,
}

impl ScrollbarInfo {
    /// Compute scrollbar info from a Terminal
    pub fn from_terminal(terminal: &Terminal) -> Self {
        let vp = terminal.viewport.read();
        let content_height = vp.content_height;
        let content_width = vp.content_width;

        // Use viewport methods which use shader-computed values if available
        let visible_height = vp.visible_content_height();
        let visible_width = vp.visible_content_width();

        drop(vp);

        // Scrollbar ratio: how much of the content is visible (in content units)
        let height_ratio = visible_height / content_height.max(1.0);
        let needs_vscrollbar = height_ratio < 1.0;

        let width_ratio = visible_width / content_width.max(1.0);
        let needs_hscrollbar = width_ratio < 1.0;

        Self {
            needs_vscrollbar,
            needs_hscrollbar,
        }
    }

    /// Check if any scrollbar is needed
    pub fn needs_any_scrollbar(&self) -> bool {
        self.needs_vscrollbar || self.needs_hscrollbar
    }
}

/// Create a vertical scrollbar overlay element that mutates viewport directly
pub fn create_vscrollbar<'a, V: ViewportAccess + 'a>(viewport: &'a V) -> Element<'a, ()> {
    ScrollbarOverlay::new(viewport).view()
}

/// Create a horizontal scrollbar overlay element that mutates viewport directly
pub fn create_hscrollbar<'a, V: ViewportAccess + 'a>(viewport: &'a V) -> Element<'a, ()> {
    HorizontalScrollbarOverlay::new(viewport).view()
}

/// Wrap content with scrollbar overlays that mutate viewport directly
/// Returns a stack element with content and scrollbars overlaid
/// The scrollbars produce () messages (no external messages needed)
pub fn wrap_with_scrollbars<'a, Message: 'a, V: ViewportAccess + 'a>(
    content: Element<'a, Message>,
    viewport: &'a V,
    needs_vscrollbar: bool,
    needs_hscrollbar: bool,
) -> Element<'a, Message> {
    if !needs_vscrollbar && !needs_hscrollbar {
        return content;
    }

    let mut layers: Vec<Element<'a, Message>> = vec![content];

    // Add vertical scrollbar if needed
    if needs_vscrollbar {
        let vscrollbar_view: Element<'a, ()> = create_vscrollbar(viewport);
        // Map () to Message - this is a no-op since scrollbar mutates viewport directly
        let vscrollbar_mapped: Element<'a, Message> = vscrollbar_view.map(|_| unreachable!());
        let vscrollbar_container: container::Container<'a, Message> =
            container(vscrollbar_mapped).width(Length::Fill).height(Length::Fill).align_x(Alignment::End);
        layers.push(vscrollbar_container.into());
    }

    // Add horizontal scrollbar if needed
    if needs_hscrollbar {
        let hscrollbar_view: Element<'a, ()> = create_hscrollbar(viewport);
        let hscrollbar_mapped: Element<'a, Message> = hscrollbar_view.map(|_| unreachable!());
        let hscrollbar_container: container::Container<'a, Message> =
            container(hscrollbar_mapped).width(Length::Fill).height(Length::Fill).align_y(Alignment::End);
        layers.push(hscrollbar_container.into());
    }

    stack(layers).into()
}

impl Terminal {
    /// Compute scrollbar info from current terminal state
    pub fn scrollbar_info(&self) -> ScrollbarInfo {
        ScrollbarInfo::from_terminal(self)
    }
}
