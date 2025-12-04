use iced::{
    Alignment, Element, Length,
    widget::{container, stack},
};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::{HorizontalScrollbarOverlay, ScrollbarOverlay, Terminal};

/// Information needed to render scrollbars
/// Computed from Terminal state, shared between icy_term and icy_view
#[derive(Debug, Clone)]
pub struct ScrollbarInfo {
    /// Whether a vertical scrollbar is needed
    pub needs_vscrollbar: bool,
    /// Whether a horizontal scrollbar is needed
    pub needs_hscrollbar: bool,
    /// Vertical scrollbar visibility (0.0 = invisible, 1.0 = fully visible)
    pub visibility_v: f32,
    /// Horizontal scrollbar visibility
    pub visibility_h: f32,
    /// Vertical scroll position (0.0 = top, 1.0 = bottom)
    pub scroll_position_v: f32,
    /// Horizontal scroll position (0.0 = left, 1.0 = right)
    pub scroll_position_h: f32,
    /// Ratio of visible height to content height (for thumb size)
    pub height_ratio: f32,
    /// Ratio of visible width to content width (for thumb size)
    pub width_ratio: f32,
    /// Maximum scroll Y in content units
    pub max_scroll_y: f32,
    /// Maximum scroll X in content units
    pub max_scroll_x: f32,
}

impl ScrollbarInfo {
    /// Compute scrollbar info from a Terminal
    pub fn from_terminal(terminal: &Terminal) -> Self {
        let vp = terminal.viewport.read();
        let zoom = vp.zoom;
        let content_height = vp.content_height;
        let content_width = vp.content_width;

        // Use computed visible dimensions from shader if available
        // These already account for zoom (at 200% zoom, we see half as much content)
        let computed_height = terminal.computed_visible_height.load(std::sync::atomic::Ordering::Relaxed) as f32;
        let computed_width = terminal.computed_visible_width.load(std::sync::atomic::Ordering::Relaxed) as f32;
        let visible_height = if computed_height > 0.0 { computed_height } else { vp.visible_height / zoom };
        let visible_width = if computed_width > 0.0 { computed_width } else { vp.visible_width / zoom };

        drop(vp);

        // Scrollbar ratio: how much of the content is visible (in content units)
        let height_ratio = visible_height / content_height.max(1.0);
        let needs_vscrollbar = height_ratio < 1.0;

        let width_ratio = visible_width / content_width.max(1.0);
        let needs_hscrollbar = width_ratio < 1.0;

        // Max scroll in CONTENT units (content_size - visible_content_size)
        let max_scroll_y = (content_height - visible_height).max(0.0);
        let max_scroll_x = (content_width - visible_width).max(0.0);

        Self {
            needs_vscrollbar,
            needs_hscrollbar,
            visibility_v: terminal.scrollbar.visibility,
            visibility_h: terminal.scrollbar.visibility_x,
            scroll_position_v: terminal.scrollbar.scroll_position,
            scroll_position_h: terminal.scrollbar.scroll_position_x,
            height_ratio,
            width_ratio,
            max_scroll_y,
            max_scroll_x,
        }
    }

    /// Check if any scrollbar is needed
    pub fn needs_any_scrollbar(&self) -> bool {
        self.needs_vscrollbar || self.needs_hscrollbar
    }

    /// Create a vertical scrollbar overlay element
    pub fn create_vscrollbar<Message: Clone + 'static>(
        &self,
        hover_state: Arc<AtomicBool>,
        on_scroll: impl Fn(f32, f32) -> Message + 'static,
        on_hover: impl Fn(bool) -> Message + 'static,
    ) -> Element<'static, Message> {
        ScrollbarOverlay::new(
            self.visibility_v,
            self.scroll_position_v,
            self.height_ratio,
            self.max_scroll_y,
            hover_state,
            on_scroll,
            on_hover,
        )
        .view()
    }

    /// Create a horizontal scrollbar overlay element
    pub fn create_hscrollbar<Message: Clone + 'static>(
        &self,
        hover_state: Arc<AtomicBool>,
        on_scroll: impl Fn(f32, f32) -> Message + 'static,
        on_hover: impl Fn(bool) -> Message + 'static,
    ) -> Element<'static, Message> {
        HorizontalScrollbarOverlay::new(
            self.visibility_h,
            self.scroll_position_h,
            self.width_ratio,
            self.max_scroll_x,
            hover_state,
            on_scroll,
            on_hover,
        )
        .view()
    }

    /// Create both scrollbars as a stack with the terminal content
    /// Returns a stack element with terminal view and scrollbars overlaid
    pub fn wrap_with_scrollbars<'a, Message: Clone + 'static>(
        &self,
        content: Element<'a, Message>,
        vscrollbar_hover_state: Arc<AtomicBool>,
        hscrollbar_hover_state: Arc<AtomicBool>,
        on_scroll_v: impl Fn(f32, f32) -> Message + 'static,
        on_hover_v: impl Fn(bool) -> Message + 'static,
        on_scroll_h: impl Fn(f32, f32) -> Message + 'static,
        on_hover_h: impl Fn(bool) -> Message + 'static,
    ) -> Element<'a, Message> {
        if !self.needs_any_scrollbar() {
            return content;
        }

        let mut layers: Vec<Element<'a, Message>> = vec![content];

        // Add vertical scrollbar if needed
        if self.needs_vscrollbar {
            let vscrollbar_view = self.create_vscrollbar(vscrollbar_hover_state, on_scroll_v, on_hover_v);
            let vscrollbar_container: container::Container<'a, Message> =
                container(vscrollbar_view).width(Length::Fill).height(Length::Fill).align_x(Alignment::End);
            layers.push(vscrollbar_container.into());
        }

        // Add horizontal scrollbar if needed
        if self.needs_hscrollbar {
            let hscrollbar_view = self.create_hscrollbar(hscrollbar_hover_state, on_scroll_h, on_hover_h);
            let hscrollbar_container: container::Container<'a, Message> =
                container(hscrollbar_view).width(Length::Fill).height(Length::Fill).align_y(Alignment::End);
            layers.push(hscrollbar_container.into());
        }

        stack(layers).into()
    }
}

impl Terminal {
    /// Compute scrollbar info from current terminal state
    pub fn scrollbar_info(&self) -> ScrollbarInfo {
        ScrollbarInfo::from_terminal(self)
    }
}
