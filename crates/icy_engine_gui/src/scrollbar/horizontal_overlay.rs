//! macOS-style horizontal overlay scrollbar widget that integrates directly with Viewport
//!
//! This scrollbar mutates the Viewport directly instead of publishing messages,
//! similar to how iced's built-in widgets (like text_input) work internally.
//!
//! Supports both `RefCell<Viewport>` (for dialogs) and `Arc<RwLock<Viewport>>` (for Terminal).
//! Also provides a callback-based variant for legacy code.
//!
//! Animation is self-driven: the widget listens for RedrawRequested events and
//! schedules the next redraw automatically when animating.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use icy_ui::{
    mouse,
    widget::canvas::{self, Cache, Canvas, Frame, Geometry, Path, Stroke},
    window, Color, Element, Length, Point, Rectangle, Renderer, Size, Theme,
};

use super::overlay::ViewportAccess;

const MIN_HEIGHT: f32 = 3.0;
const MAX_HEIGHT: f32 = 9.0;
const MIN_ALPHA: f32 = 0.42;
const MAX_ALPHA: f32 = 0.87;
const BOTTOM_OFFSET: f32 = 0.0;
const LEFT_PADDING: f32 = 0.0;
const RIGHT_PADDING: f32 = 0.0;

/// Animation frame interval (~60fps)
const ANIMATION_FRAME_MS: u64 = 16;

/// State for the horizontal scrollbar overlay canvas widget
#[derive(Debug, Default)]
pub struct HorizontalScrollbarOverlayState {
    /// Whether the user is currently dragging the scrollbar (for callback mode)
    pub is_dragging: bool,
}

// ============================================================================
// ViewportAccess-based scrollbar (new API)
// ============================================================================

/// Horizontal scrollbar overlay that mutates Viewport directly
pub struct HorizontalScrollbarOverlay<'a, V: ViewportAccess> {
    viewport: &'a V,
}

impl<'a, V: ViewportAccess> HorizontalScrollbarOverlay<'a, V> {
    pub fn new(viewport: &'a V) -> Self {
        Self { viewport }
    }

    pub fn view(self) -> Element<'a, ()>
    where
        V: 'a,
    {
        Canvas::new(self).width(Length::Fill).height(Length::Fixed(12.0)).into()
    }

    fn draw_scrollbar(&self, frame: &mut Frame, size: Size) {
        let (visibility, scroll_position, width_ratio) = self
            .viewport
            .with_viewport(|vp| (vp.scrollbar.visibility_x, vp.scrollbar.scroll_position_x, vp.width_ratio()));

        draw_horizontal_scrollbar(frame, size, visibility, scroll_position, width_ratio);
    }
}

impl<'a, V: ViewportAccess> canvas::Program<()> for HorizontalScrollbarOverlay<'a, V> {
    type State = HorizontalScrollbarOverlayState;

    fn draw(&self, _state: &Self::State, renderer: &Renderer, _theme: &Theme, bounds: Rectangle, _cursor: mouse::Cursor) -> Vec<Geometry> {
        let cache = Cache::new();
        let geometry = cache.draw(renderer, bounds.size(), |frame| {
            self.draw_scrollbar(frame, bounds.size());
        });

        vec![geometry]
    }

    fn update(&self, _state: &mut Self::State, event: &icy_ui::Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<icy_ui::widget::canvas::Action<()>> {
        let is_hovered = cursor.is_over(bounds);

        match event {
            // Handle animation: on each redraw, update scrollbar AND scroll animations
            // This makes the scrollbar self-driving - no manual animation calls needed
            icy_ui::Event::Window(window::Event::RedrawRequested(now)) => {
                let next_redraw_at = self.viewport.with_viewport_mut(|vp| {
                    // Update scrollbar visibility animation
                    vp.scrollbar.update_animation();
                    // Update smooth scroll animation (PageUp/PageDown, Home/End, etc.)
                    vp.update_animation();

                    let mut next: Option<std::time::Instant> = None;

                    if vp.is_animating() {
                        next = Some(*now + Duration::from_millis(ANIMATION_FRAME_MS));
                    }

                    if let Some(sb_next) = vp.scrollbar.next_wakeup_instant(*now) {
                        next = Some(next.map_or(sb_next, |cur| cur.min(sb_next)));
                    }

                    next
                });

                if let Some(next_frame) = next_redraw_at {
                    return Some(icy_ui::widget::canvas::Action::request_redraw_at(next_frame));
                }
            }
            icy_ui::Event::Mouse(mouse_event) => match mouse_event {
                icy_ui::mouse::Event::ButtonPressed {
                    button: icy_ui::mouse::Button::Left,
                    ..
                } => {
                    if is_hovered {
                        if let Some(pos) = cursor.position_in(bounds) {
                            self.viewport.with_viewport_mut(|vp| {
                                vp.handle_hscrollbar_press(pos.x, bounds.width);
                            });
                            return Some(icy_ui::widget::canvas::Action::request_redraw().and_capture());
                        }
                    }
                }
                icy_ui::mouse::Event::ButtonReleased {
                    button: icy_ui::mouse::Button::Left,
                    ..
                } => {
                    let released = self.viewport.with_viewport_mut(|vp| {
                        if vp.handle_hscrollbar_release() {
                            vp.handle_hscrollbar_hover(is_hovered);
                            true
                        } else {
                            false
                        }
                    });
                    if released {
                        return Some(icy_ui::widget::canvas::Action::request_redraw());
                    }
                }
                icy_ui::mouse::Event::CursorMoved { .. } => {
                    let result = self.viewport.with_viewport_mut(|vp| {
                        if vp.scrollbar.is_dragging_x {
                            // Continue dragging even outside bounds
                            if let Some(pos) = cursor.position() {
                                let relative_x = pos.x - bounds.x;
                                vp.handle_hscrollbar_drag(relative_x, bounds.width);
                                return Some(true); // dragging
                            }
                            None
                        } else {
                            // Update hover state
                            if vp.handle_hscrollbar_hover(is_hovered) {
                                Some(false) // hover changed
                            } else {
                                None
                            }
                        }
                    });
                    match result {
                        Some(true) => return Some(icy_ui::widget::canvas::Action::request_redraw().and_capture()),
                        Some(false) => return Some(icy_ui::widget::canvas::Action::request_redraw()),
                        None => {}
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

// ============================================================================
// Callback-based scrollbar (legacy API for custom viewports)
// ============================================================================

/// Horizontal scrollbar overlay with callbacks (legacy API)
/// Use this when you have a custom viewport that doesn't implement ViewportAccess
pub struct HorizontalScrollbarOverlayCallback<Message> {
    visibility: f32,
    scroll_position: f32,
    width_ratio: f32,
    max_scroll_x: f32,
    on_scroll: Box<dyn Fn(f32, f32) -> Message>,
    on_hover: Box<dyn Fn(bool) -> Message>,
    last_hover_state: Arc<AtomicBool>,
}

impl<Message> HorizontalScrollbarOverlayCallback<Message>
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

    fn calculate_scroll_from_position(&self, mouse_x: f32, width: f32) -> f32 {
        let available_width = width - LEFT_PADDING - RIGHT_PADDING;
        let thumb_width = (available_width * self.width_ratio).max(30.0);
        let click_offset = mouse_x - LEFT_PADDING - (thumb_width / 2.0);
        let max_thumb_offset = available_width - thumb_width;
        if max_thumb_offset > 0.0 {
            (click_offset / max_thumb_offset).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

impl<Message> canvas::Program<Message> for HorizontalScrollbarOverlayCallback<Message>
where
    Message: Clone + 'static,
{
    type State = HorizontalScrollbarOverlayState;

    fn draw(&self, _state: &Self::State, renderer: &Renderer, _theme: &Theme, bounds: Rectangle, _cursor: mouse::Cursor) -> Vec<Geometry> {
        let cache = Cache::new();
        let geometry = cache.draw(renderer, bounds.size(), |frame| {
            draw_horizontal_scrollbar(frame, bounds.size(), self.visibility, self.scroll_position, self.width_ratio);
        });
        vec![geometry]
    }

    fn update(
        &self,
        state: &mut Self::State,
        event: &icy_ui::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<icy_ui::widget::canvas::Action<Message>> {
        let is_hovered = cursor.is_over(bounds);

        match event {
            icy_ui::Event::Mouse(mouse_event) => match mouse_event {
                icy_ui::mouse::Event::ButtonPressed {
                    button: icy_ui::mouse::Button::Left,
                    ..
                } => {
                    if is_hovered {
                        state.is_dragging = true;
                        if let Some(pos) = cursor.position_in(bounds) {
                            let scroll_ratio = self.calculate_scroll_from_position(pos.x, bounds.width);
                            let absolute_x = scroll_ratio * self.max_scroll_x;
                            let msg = (self.on_scroll)(absolute_x, 0.0);
                            return Some(icy_ui::widget::canvas::Action::publish(msg).and_capture());
                        }
                    }
                }
                icy_ui::mouse::Event::ButtonReleased {
                    button: icy_ui::mouse::Button::Left,
                    ..
                } => {
                    if state.is_dragging {
                        state.is_dragging = false;
                        let msg = (self.on_hover)(is_hovered);
                        return Some(icy_ui::widget::canvas::Action::publish(msg));
                    }
                }
                icy_ui::mouse::Event::CursorMoved { .. } => {
                    if state.is_dragging {
                        if let Some(pos) = cursor.position() {
                            let relative_x = pos.x - bounds.x;
                            let scroll_ratio = self.calculate_scroll_from_position(relative_x, bounds.width);
                            let absolute_x = scroll_ratio * self.max_scroll_x;
                            let msg = (self.on_scroll)(absolute_x, 0.0);
                            return Some(icy_ui::widget::canvas::Action::publish(msg).and_capture());
                        }
                    } else {
                        let last_hover = self.last_hover_state.swap(is_hovered, Ordering::Relaxed);
                        if last_hover != is_hovered {
                            let msg = (self.on_hover)(is_hovered);
                            return Some(icy_ui::widget::canvas::Action::publish(msg));
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

// ============================================================================
// Shared rendering code
// ============================================================================

fn draw_horizontal_scrollbar(frame: &mut Frame, size: Size, visibility: f32, scroll_position: f32, width_ratio: f32) {
    let is_thin = visibility <= 0.3;

    if is_thin {
        // Thin line mode
        let line_y = size.height - MIN_HEIGHT;
        let available_width = size.width - LEFT_PADDING - RIGHT_PADDING;
        let thumb_width = (available_width * width_ratio).max(40.0);
        let max_thumb_offset = available_width - thumb_width;
        let thumb_x = LEFT_PADDING + (max_thumb_offset * scroll_position);

        let thin_thumb = Path::rounded_rectangle(Point::new(thumb_x, line_y), Size::new(thumb_width, MIN_HEIGHT), 1.5.into());
        frame.fill(&thin_thumb, Color::from_rgba(1.0, 1.0, 1.0, MIN_ALPHA));
    } else {
        // Full scrollbar mode
        let scrollbar_height = MIN_HEIGHT + (MAX_HEIGHT - MIN_HEIGHT) * visibility;
        let scrollbar_y = size.height - scrollbar_height - BOTTOM_OFFSET;

        // Background track
        let track_path = Path::rectangle(
            Point::new(LEFT_PADDING, scrollbar_y),
            Size::new(size.width - LEFT_PADDING - RIGHT_PADDING, scrollbar_height),
        );
        frame.fill(&track_path, Color::from_rgba(0.0, 0.0, 0.0, 0.2 * visibility));

        // Thumb
        let available_width = size.width - LEFT_PADDING - RIGHT_PADDING;
        let thumb_width = (available_width * width_ratio).max(30.0);
        let max_thumb_offset = available_width - thumb_width;
        let thumb_x = LEFT_PADDING + (max_thumb_offset * scroll_position);

        let thumb_path = Path::rounded_rectangle(Point::new(thumb_x, scrollbar_y), Size::new(thumb_width, scrollbar_height), 4.0.into());
        frame.fill(&thumb_path, Color::from_rgba(1.0, 1.0, 1.0, MIN_ALPHA + (MAX_ALPHA - MIN_ALPHA) * visibility));

        frame.stroke(
            &thumb_path,
            Stroke::default().with_width(0.5).with_color(Color::from_rgba(1.0, 1.0, 1.0, 0.3 * visibility)),
        );
    }
}
