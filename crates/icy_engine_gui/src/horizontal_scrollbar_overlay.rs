use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use iced::{
    Color, Element, Length, Point, Rectangle, Renderer, Size, Theme, mouse,
    widget::canvas::{self, Cache, Canvas, Frame, Geometry, Path, Stroke},
};

/// State for the horizontal scrollbar overlay canvas widget
#[derive(Debug, Default)]
pub struct HorizontalScrollbarOverlayState {
    /// Whether the user is currently dragging the scrollbar
    pub is_dragging: bool,
}

pub struct HorizontalScrollbarOverlay<Message> {
    visibility: f32,
    scroll_position: f32,
    width_ratio: f32,
    max_scroll_x: f32,                           // Maximum scroll value in pixels
    on_scroll: Box<dyn Fn(f32, f32) -> Message>, // Callback for scroll (x, y)
    on_hover: Box<dyn Fn(bool) -> Message>,      // Callback for hover state changes
    last_hover_state: Arc<AtomicBool>,           // Shared atomic state to track hover
}

const MIN_HEIGHT: f32 = 3.0;
const MAX_HEIGHT: f32 = 9.0;
const MIN_ALPHA: f32 = 0.42;
const MAX_ALPHA: f32 = 0.87;
// Offset from bottom edge when fully expanded
const BOTTOM_OFFSET: f32 = 0.0;
// Left/right padding
const LEFT_PADDING: f32 = 0.0;
// Right padding to leave room for vertical scrollbar
const RIGHT_PADDING: f32 = 0.0;

impl<Message> HorizontalScrollbarOverlay<Message>
where
    Message: Clone + 'static,
{
    pub fn new(
        visibility: f32,
        scroll_position: f32,
        width_ratio: f32,
        max_scroll_x: f32,
        last_hover_state: Arc<AtomicBool>,
        on_scroll: impl Fn(f32, f32) -> Message + 'static,
        on_hover: impl Fn(bool) -> Message + 'static,
    ) -> Self {
        Self {
            visibility,
            scroll_position,
            width_ratio,
            max_scroll_x,
            on_scroll: Box::new(on_scroll),
            on_hover: Box::new(on_hover),
            last_hover_state,
        }
    }

    pub fn view(self) -> Element<'static, Message> {
        Canvas::new(self).width(Length::Fill).height(Length::Fixed(12.0)).into()
    }

    fn draw_scrollbar(&self, frame: &mut Frame, size: Size, animated_visibility: f32) {
        // Use smoothly animated visibility value
        let is_thin = animated_visibility <= 0.3;

        if is_thin {
            // Draw just a thin line at the bottom edge - always visible
            let line_y = size.height - MIN_HEIGHT; // Bottom edge of canvas

            // Calculate thumb position
            let available_width = size.width - LEFT_PADDING - RIGHT_PADDING;
            let thumb_width = (available_width * self.width_ratio).max(40.0);
            let max_thumb_offset = available_width - thumb_width;
            let thumb_x = LEFT_PADDING + (max_thumb_offset * self.scroll_position);

            // Draw thin thumb indicator - bright white with alpha
            let thin_thumb = Path::rounded_rectangle(Point::new(thumb_x, line_y), Size::new(thumb_width, MIN_HEIGHT), 1.5.into());
            frame.fill(&thin_thumb, Color::from_rgba(1.0, 1.0, 1.0, MIN_ALPHA));
        } else {
            // Full scrollbar mode with smooth transition
            let scrollbar_height = MIN_HEIGHT + (MAX_HEIGHT - MIN_HEIGHT) * self.visibility;
            // Position scrollbar at bottom edge with offset
            let scrollbar_y = size.height - scrollbar_height - BOTTOM_OFFSET;

            // Draw background track - subtle dark background
            let track_path = Path::rectangle(
                Point::new(LEFT_PADDING, scrollbar_y),
                Size::new(size.width - LEFT_PADDING - RIGHT_PADDING, scrollbar_height),
            );
            frame.fill(&track_path, Color::from_rgba(0.0, 0.0, 0.0, 0.2 * animated_visibility));

            // Calculate thumb position and size
            let available_width = size.width - LEFT_PADDING - RIGHT_PADDING;
            let thumb_width = (available_width * self.width_ratio).max(30.0);
            let max_thumb_offset = available_width - thumb_width;
            let thumb_x = LEFT_PADDING + (max_thumb_offset * self.scroll_position);

            // Draw thumb - bright white
            let thumb_path = Path::rounded_rectangle(Point::new(thumb_x, scrollbar_y), Size::new(thumb_width, scrollbar_height), 4.0.into());
            frame.fill(
                &thumb_path,
                Color::from_rgba(1.0, 1.0, 1.0, MIN_ALPHA + (MAX_ALPHA - MIN_ALPHA) * animated_visibility),
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

    // Calculate scroll ratio (0.0-1.0) from mouse X position
    fn calculate_scroll_from_position(&self, mouse_x: f32, width: f32) -> f32 {
        // Account for padding
        let available_width = width - LEFT_PADDING - RIGHT_PADDING;
        let thumb_width = (available_width * self.width_ratio).max(30.0);

        // Position the center of the thumb at the mouse position
        let click_offset = mouse_x - LEFT_PADDING - (thumb_width / 2.0);

        // Calculate position accounting for thumb size
        let max_thumb_offset = available_width - thumb_width;

        // Convert to scroll ratio
        if max_thumb_offset > 0.0 {
            (click_offset / max_thumb_offset).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

impl<Message> canvas::Program<Message> for HorizontalScrollbarOverlay<Message>
where
    Message: Clone + 'static,
{
    type State = HorizontalScrollbarOverlayState;

    fn draw(&self, _state: &Self::State, renderer: &Renderer, _theme: &Theme, bounds: Rectangle, _cursor: mouse::Cursor) -> Vec<Geometry> {
        let animated_visibility = self.visibility;

        let cache = Cache::new();
        let geometry = cache.draw(renderer, bounds.size(), |frame| {
            self.draw_scrollbar(frame, bounds.size(), animated_visibility);
        });

        vec![geometry]
    }

    fn update(&self, state: &mut Self::State, event: &iced::Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<iced::widget::canvas::Action<Message>> {
        // Canvas is fixed height (12px), so just check if mouse is over bounds
        let is_hovered = cursor.is_over(bounds);

        match event {
            iced::Event::Mouse(mouse_event) => match mouse_event {
                iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left) => {
                    if is_hovered {
                        state.is_dragging = true;
                        if let Some(pos) = cursor.position_in(bounds) {
                            let scroll_ratio = self.calculate_scroll_from_position(pos.x, bounds.width);
                            let absolute_x = scroll_ratio * self.max_scroll_x;
                            let msg = (self.on_scroll)(absolute_x, 0.0);
                            return Some(iced::widget::canvas::Action::publish(msg).and_capture());
                        }
                    }
                }
                iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left) => {
                    if state.is_dragging {
                        state.is_dragging = false;
                        let msg = (self.on_hover)(is_hovered);
                        return Some(iced::widget::canvas::Action::publish(msg));
                    }
                }
                iced::mouse::Event::CursorMoved { .. } => {
                    if state.is_dragging {
                        if let Some(pos) = cursor.position() {
                            let relative_x = pos.x - bounds.x;
                            let scroll_ratio = self.calculate_scroll_from_position(relative_x, bounds.width);
                            let absolute_x = scroll_ratio * self.max_scroll_x;
                            let msg = (self.on_scroll)(absolute_x, 0.0);
                            return Some(iced::widget::canvas::Action::publish(msg).and_capture());
                        }
                    } else {
                        let last_hover = self.last_hover_state.swap(is_hovered, Ordering::Relaxed);
                        if last_hover != is_hovered {
                            let msg = (self.on_hover)(is_hovered);
                            return Some(iced::widget::canvas::Action::publish(msg));
                        }
                    }
                }
                _ => {}
            },
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
