use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use iced::{
    Color, Element, Length, Point, Rectangle, Renderer, Size, Theme, mouse,
    widget::canvas::{self, Cache, Canvas, Frame, Geometry, Path, Stroke},
};

/// State for the scrollbar overlay canvas widget
#[derive(Debug, Default)]
pub struct ScrollbarOverlayState {
    /// Whether the user is currently dragging the scrollbar
    pub is_dragging: bool,
}

pub struct ScrollbarOverlay<Message> {
    visibility: f32,
    scroll_position: f32,
    height_ratio: f32,
    max_scroll_y: f32,                           // Maximum scroll value in pixels
    on_scroll: Box<dyn Fn(f32, f32) -> Message>, // Callback for scroll (x, y)
    on_hover: Box<dyn Fn(bool) -> Message>,      // Callback for hover state changes
    last_hover_state: Arc<AtomicBool>,           // Shared atomic state to track hover
}

const MIN_WITH: f32 = 3.0;
const MAX_WIDTH: f32 = 9.0;
const MIN_ALPHA: f32 = 0.42;
const MAX_ALPHA: f32 = 0.87;
// Offset from right edge when fully expanded
const RIGHT_OFFSET: f32 = 0.0;
// Top/bottom padding
const TOP_PADDING: f32 = 0.0;
// Bottom padding to leave room for horizontal scrollbar
const BOTTOM_PADDING: f32 = 0.0;

impl<Message> ScrollbarOverlay<Message>
where
    Message: Clone + 'static,
{
    pub fn new(
        visibility: f32,
        scroll_position: f32,
        height_ratio: f32,
        max_scroll_y: f32,
        last_hover_state: Arc<AtomicBool>,
        on_scroll: impl Fn(f32, f32) -> Message + 'static,
        on_hover: impl Fn(bool) -> Message + 'static,
    ) -> Self {
        Self {
            visibility,
            scroll_position,
            height_ratio,
            max_scroll_y,
            on_scroll: Box::new(on_scroll),
            on_hover: Box::new(on_hover),
            last_hover_state,
        }
    }

    pub fn view(self) -> Element<'static, Message> {
        Canvas::new(self)
            .width(Length::Fixed(12.0)) // Match scrollbar width
            .height(Length::Fill)
            .into()
    }

    fn draw_scrollbar(&self, frame: &mut Frame, size: Size, animated_visibility: f32) {
        // Use smoothly animated visibility value
        let is_thin = animated_visibility <= 0.3;

        if is_thin {
            // Draw just a thin line at the right edge - always visible
            let line_x = size.width - MIN_WITH; // Right edge of canvas

            // Calculate thumb position
            let available_height = size.height - TOP_PADDING - BOTTOM_PADDING;
            let thumb_height = (available_height * self.height_ratio).max(40.0);
            let max_thumb_offset = available_height - thumb_height;
            let thumb_y = TOP_PADDING + (max_thumb_offset * self.scroll_position);

            // Draw thin thumb indicator - bright white with alpha
            let thin_thumb = Path::rounded_rectangle(Point::new(line_x, thumb_y), Size::new(MIN_WITH, thumb_height), 1.5.into());
            frame.fill(&thin_thumb, Color::from_rgba(1.0, 1.0, 1.0, MIN_ALPHA));
        } else {
            // Full scrollbar mode with smooth transition
            let scrollbar_width = MIN_WITH + (MAX_WIDTH - MIN_WITH) * self.visibility;
            // Position scrollbar at right edge with offset
            let scrollbar_x = size.width - scrollbar_width - RIGHT_OFFSET;

            // Draw background track - subtle dark background
            let track_height = size.height - TOP_PADDING - BOTTOM_PADDING;
            let track_path = Path::rectangle(Point::new(scrollbar_x, TOP_PADDING), Size::new(scrollbar_width, track_height));
            frame.fill(&track_path, Color::from_rgba(0.0, 0.0, 0.0, 0.2 * animated_visibility));

            // Calculate thumb position and size
            let available_height = track_height;
            let thumb_height = (available_height * self.height_ratio).max(30.0);
            let max_thumb_offset = available_height - thumb_height;
            let thumb_y = TOP_PADDING + (max_thumb_offset * self.scroll_position);

            // Draw thumb - bright white
            let thumb_path = Path::rounded_rectangle(Point::new(scrollbar_x, thumb_y), Size::new(scrollbar_width, thumb_height), 4.0.into());
            frame.fill(
                &thumb_path,
                Color::from_rgba(1.0, 1.0, 1.0, MIN_ALPHA + (MAX_ALPHA - MIN_ALPHA) * animated_visibility), // White
            );

            // Add subtle border for depth
            frame.stroke(
                &thumb_path,
                Stroke::default()
                    .with_width(0.5)
                    .with_color(Color::from_rgba(1.0, 1.0, 1.0, 0.3 * animated_visibility)),
            );
        }
    }

    // Calculate scroll ratio (0.0-1.0) from mouse Y position
    fn calculate_scroll_from_position(&self, mouse_y: f32, height: f32) -> f32 {
        // Account for padding
        let available_height = height - TOP_PADDING - BOTTOM_PADDING;
        let thumb_height = (available_height * self.height_ratio).max(30.0);

        // Position the center of the thumb at the mouse position
        // Subtract half thumb height so the thumb center aligns with click
        let click_offset = mouse_y - TOP_PADDING - (thumb_height / 2.0);

        // Calculate position accounting for thumb size
        let max_thumb_offset = available_height - thumb_height;

        // Convert to scroll ratio
        if max_thumb_offset > 0.0 {
            (click_offset / max_thumb_offset).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

impl<Message> canvas::Program<Message> for ScrollbarOverlay<Message>
where
    Message: Clone + 'static,
{
    type State = ScrollbarOverlayState;

    fn draw(&self, _state: &Self::State, renderer: &Renderer, _theme: &Theme, bounds: Rectangle, _cursor: mouse::Cursor) -> Vec<Geometry> {
        // Use visibility from ScrollbarState (animated by ViewportTick)
        let animated_visibility = self.visibility;

        // Always draw with animated visibility
        let cache = Cache::new();
        let geometry = cache.draw(renderer, bounds.size(), |frame| {
            self.draw_scrollbar(frame, bounds.size(), animated_visibility);
        });

        vec![geometry]
    }

    fn update(&self, state: &mut Self::State, event: &iced::Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<iced::widget::canvas::Action<Message>> {
        let is_hovered = cursor.is_over(bounds);

        // Handle mouse events for scrolling
        match event {
            iced::Event::Mouse(mouse_event) => {
                match mouse_event {
                    iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left) => {
                        if is_hovered {
                            state.is_dragging = true;
                            // Calculate scroll position from click
                            if let Some(pos) = cursor.position_in(bounds) {
                                let scroll_ratio = self.calculate_scroll_from_position(pos.y, bounds.height);
                                let absolute_y = scroll_ratio * self.max_scroll_y;
                                let msg = (self.on_scroll)(0.0, absolute_y);
                                return Some(iced::widget::canvas::Action::publish(msg));
                            }
                        }
                    }
                    iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left) => {
                        if state.is_dragging {
                            state.is_dragging = false;
                            // Update hover state after releasing
                            let msg = (self.on_hover)(is_hovered);
                            return Some(iced::widget::canvas::Action::publish(msg));
                        }
                    }
                    iced::mouse::Event::CursorMoved { .. } => {
                        if state.is_dragging {
                            // Continue scrolling even if cursor moves outside bounds (mouse capture effect)
                            if let Some(pos) = cursor.position() {
                                // Calculate position relative to bounds
                                let relative_y = pos.y - bounds.y;
                                let scroll_ratio = self.calculate_scroll_from_position(relative_y, bounds.height);
                                let absolute_y = scroll_ratio * self.max_scroll_y;
                                let msg = (self.on_scroll)(0.0, absolute_y);
                                return Some(iced::widget::canvas::Action::publish(msg));
                            }
                        } else {
                            // Check if hover state actually changed to avoid unnecessary messages
                            // Use swap to atomically get old value and set new one
                            let last_hover = self.last_hover_state.swap(is_hovered, Ordering::Relaxed);
                            if last_hover != is_hovered {
                                let msg = (self.on_hover)(is_hovered);
                                return Some(iced::widget::canvas::Action::publish(msg));
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }

        None
    }

    fn mouse_interaction(&self, _state: &Self::State, bounds: Rectangle, cursor: mouse::Cursor) -> mouse::Interaction {
        if cursor.is_over(bounds) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }
}
