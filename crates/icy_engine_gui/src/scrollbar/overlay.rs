//! macOS-style overlay scrollbar widget that integrates directly with Viewport
//!
//! This scrollbar mutates the Viewport directly instead of publishing messages,
//! similar to how iced's built-in widgets (like text_input) work internally.
//!
//! Supports both `RefCell<Viewport>` (for dialogs) and `Arc<RwLock<Viewport>>` (for Terminal).
//! Also provides a callback-based variant for legacy code.
//!
//! Animation is self-driven: the widget listens for RedrawRequested events and
//! schedules the next redraw automatically when animating, like iced's text_input cursor blink.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use iced::{
    mouse,
    widget::canvas::{self, Cache, Canvas, Frame, Geometry, Path, Stroke},
    window, Color, Element, Length, Point, Rectangle, Renderer, Size, Theme,
};
use parking_lot::RwLock;

use crate::Viewport;

const MIN_WIDTH: f32 = 3.0;
const MAX_WIDTH: f32 = 9.0;
const MIN_ALPHA: f32 = 0.42;
const MAX_ALPHA: f32 = 0.87;
const RIGHT_OFFSET: f32 = 0.0;
const TOP_PADDING: f32 = 0.0;
const BOTTOM_PADDING: f32 = 0.0;

/// Animation frame interval (~60fps)
const ANIMATION_FRAME_MS: u64 = 16;

/// Trait for accessing Viewport in different container types
pub trait ViewportAccess {
    fn with_viewport<R>(&self, f: impl FnOnce(&Viewport) -> R) -> R;
    fn with_viewport_mut<R>(&self, f: impl FnOnce(&mut Viewport) -> R) -> R;
}

impl ViewportAccess for RefCell<Viewport> {
    fn with_viewport<R>(&self, f: impl FnOnce(&Viewport) -> R) -> R {
        f(&self.borrow())
    }
    fn with_viewport_mut<R>(&self, f: impl FnOnce(&mut Viewport) -> R) -> R {
        f(&mut self.borrow_mut())
    }
}

impl ViewportAccess for Arc<RwLock<Viewport>> {
    fn with_viewport<R>(&self, f: impl FnOnce(&Viewport) -> R) -> R {
        f(&self.read())
    }
    fn with_viewport_mut<R>(&self, f: impl FnOnce(&mut Viewport) -> R) -> R {
        f(&mut self.write())
    }
}

/// State for the scrollbar overlay canvas widget
#[derive(Debug, Default)]
pub struct ScrollbarOverlayState {
    /// Whether the user is currently dragging the scrollbar (for callback mode)
    pub is_dragging: bool,
}

// ============================================================================
// ViewportAccess-based scrollbar (new API)
// ============================================================================

/// Vertical scrollbar overlay that mutates Viewport directly
pub struct ScrollbarOverlay<'a, V: ViewportAccess> {
    viewport: &'a V,
}

impl<'a, V: ViewportAccess> ScrollbarOverlay<'a, V> {
    pub fn new(viewport: &'a V) -> Self {
        Self { viewport }
    }

    pub fn view(self) -> Element<'a, ()>
    where
        V: 'a,
    {
        Canvas::new(self).width(Length::Fixed(12.0)).height(Length::Fill).into()
    }

    fn draw_scrollbar(&self, frame: &mut Frame, size: Size) {
        let (visibility, scroll_position, height_ratio) = self
            .viewport
            .with_viewport(|vp| (vp.scrollbar.visibility, vp.scrollbar.scroll_position, vp.height_ratio()));

        draw_vertical_scrollbar(frame, size, visibility, scroll_position, height_ratio);
    }
}

impl<'a, V: ViewportAccess> canvas::Program<()> for ScrollbarOverlay<'a, V> {
    type State = ScrollbarOverlayState;

    fn draw(&self, _state: &Self::State, renderer: &Renderer, _theme: &Theme, bounds: Rectangle, _cursor: mouse::Cursor) -> Vec<Geometry> {
        let cache = Cache::new();
        let geometry = cache.draw(renderer, bounds.size(), |frame| {
            self.draw_scrollbar(frame, bounds.size());
        });

        vec![geometry]
    }

    fn update(&self, _state: &mut Self::State, event: &iced::Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<iced::widget::canvas::Action<()>> {
        let is_hovered = cursor.is_over(bounds);

        match event {
            // Handle animation: on each redraw, update scrollbar AND scroll animations
            // This makes the scrollbar self-driving - no manual animation calls needed
            iced::Event::Window(window::Event::RedrawRequested(now)) => {
                let next_redraw_at = self.viewport.with_viewport_mut(|vp| {
                    // Update scrollbar visibility animation
                    vp.scrollbar.update_animation();
                    // Update smooth scroll animation (PageUp/PageDown, Home/End, etc.)
                    vp.update_animation();

                    let mut next: Option<std::time::Instant> = None;

                    // Viewport smooth scrolling wants regular frames while active.
                    if vp.is_animating() {
                        next = Some(*now + Duration::from_millis(ANIMATION_FRAME_MS));
                    }

                    // Scrollbar schedules either the next animation frame or the fade-out start.
                    if let Some(sb_next) = vp.scrollbar.next_wakeup_instant(*now) {
                        next = Some(next.map_or(sb_next, |cur| cur.min(sb_next)));
                    }

                    next
                });

                if let Some(next_frame) = next_redraw_at {
                    return Some(iced::widget::canvas::Action::request_redraw_at(next_frame));
                }
            }
            iced::Event::Mouse(mouse_event) => match mouse_event {
                iced::mouse::Event::ButtonPressed {
                    button: iced::mouse::Button::Left,
                    ..
                } => {
                    if is_hovered {
                        if let Some(pos) = cursor.position_in(bounds) {
                            self.viewport.with_viewport_mut(|vp| {
                                vp.handle_vscrollbar_press(pos.y, bounds.height);
                            });
                            return Some(iced::widget::canvas::Action::request_redraw().and_capture());
                        }
                    }
                }
                iced::mouse::Event::ButtonReleased {
                    button: iced::mouse::Button::Left,
                    ..
                } => {
                    let released = self.viewport.with_viewport_mut(|vp| {
                        if vp.handle_vscrollbar_release() {
                            vp.handle_vscrollbar_hover(is_hovered);
                            true
                        } else {
                            false
                        }
                    });
                    if released {
                        return Some(iced::widget::canvas::Action::request_redraw());
                    }
                }
                iced::mouse::Event::CursorMoved { .. } => {
                    let result = self.viewport.with_viewport_mut(|vp| {
                        if vp.scrollbar.is_dragging {
                            // Continue dragging even outside bounds
                            if let Some(pos) = cursor.position() {
                                let relative_y = pos.y - bounds.y;
                                vp.handle_vscrollbar_drag(relative_y, bounds.height);
                                return Some(true); // dragging
                            }
                            None
                        } else {
                            // Update hover state
                            if vp.handle_vscrollbar_hover(is_hovered) {
                                Some(false) // hover changed
                            } else {
                                None
                            }
                        }
                    });
                    match result {
                        Some(true) => return Some(iced::widget::canvas::Action::request_redraw().and_capture()),
                        Some(false) => return Some(iced::widget::canvas::Action::request_redraw()),
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

/// Vertical scrollbar overlay with callbacks (legacy API)
/// Use this when you have a custom viewport that doesn't implement ViewportAccess
pub struct ScrollbarOverlayCallback<Message> {
    visibility: f32,
    scroll_position: f32,
    height_ratio: f32,
    max_scroll_y: f32,
    on_scroll: Box<dyn Fn(f32, f32) -> Message>,
    on_hover: Box<dyn Fn(bool) -> Message>,
    last_hover_state: Arc<AtomicBool>,
}

impl<Message> ScrollbarOverlayCallback<Message>
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
        Canvas::new(self).width(Length::Fixed(12.0)).height(Length::Fill).into()
    }

    fn calculate_scroll_from_position(&self, mouse_y: f32, height: f32) -> f32 {
        let available_height = height - TOP_PADDING - BOTTOM_PADDING;
        let thumb_height = (available_height * self.height_ratio).max(30.0);
        let click_offset = mouse_y - TOP_PADDING - (thumb_height / 2.0);
        let max_thumb_offset = available_height - thumb_height;
        if max_thumb_offset > 0.0 {
            (click_offset / max_thumb_offset).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

impl<Message> canvas::Program<Message> for ScrollbarOverlayCallback<Message>
where
    Message: Clone + 'static,
{
    type State = ScrollbarOverlayState;

    fn draw(&self, _state: &Self::State, renderer: &Renderer, _theme: &Theme, bounds: Rectangle, _cursor: mouse::Cursor) -> Vec<Geometry> {
        let cache = Cache::new();
        let geometry = cache.draw(renderer, bounds.size(), |frame| {
            draw_vertical_scrollbar(frame, bounds.size(), self.visibility, self.scroll_position, self.height_ratio);
        });
        vec![geometry]
    }

    fn update(&self, state: &mut Self::State, event: &iced::Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<iced::widget::canvas::Action<Message>> {
        let is_hovered = cursor.is_over(bounds);

        match event {
            iced::Event::Mouse(mouse_event) => match mouse_event {
                iced::mouse::Event::ButtonPressed {
                    button: iced::mouse::Button::Left,
                    ..
                } => {
                    if is_hovered {
                        state.is_dragging = true;
                        if let Some(pos) = cursor.position_in(bounds) {
                            let scroll_ratio = self.calculate_scroll_from_position(pos.y, bounds.height);
                            let absolute_y = scroll_ratio * self.max_scroll_y;
                            let msg = (self.on_scroll)(0.0, absolute_y);
                            return Some(iced::widget::canvas::Action::publish(msg).and_capture());
                        }
                    }
                }
                iced::mouse::Event::ButtonReleased {
                    button: iced::mouse::Button::Left,
                    ..
                } => {
                    if state.is_dragging {
                        state.is_dragging = false;
                        let msg = (self.on_hover)(is_hovered);
                        return Some(iced::widget::canvas::Action::publish(msg));
                    }
                }
                iced::mouse::Event::CursorMoved { .. } => {
                    if state.is_dragging {
                        if let Some(pos) = cursor.position() {
                            let relative_y = pos.y - bounds.y;
                            let scroll_ratio = self.calculate_scroll_from_position(relative_y, bounds.height);
                            let absolute_y = scroll_ratio * self.max_scroll_y;
                            let msg = (self.on_scroll)(0.0, absolute_y);
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

// ============================================================================
// Shared rendering code
// ============================================================================

fn draw_vertical_scrollbar(frame: &mut Frame, size: Size, visibility: f32, scroll_position: f32, height_ratio: f32) {
    let is_thin = visibility <= 0.3;

    if is_thin {
        // Thin line mode
        let line_x = size.width - MIN_WIDTH;
        let available_height = size.height - TOP_PADDING - BOTTOM_PADDING;
        let thumb_height = (available_height * height_ratio).max(40.0);
        let max_thumb_offset = available_height - thumb_height;
        let thumb_y = TOP_PADDING + (max_thumb_offset * scroll_position);

        let thin_thumb = Path::rounded_rectangle(Point::new(line_x, thumb_y), Size::new(MIN_WIDTH, thumb_height), 1.5.into());
        frame.fill(&thin_thumb, Color::from_rgba(1.0, 1.0, 1.0, MIN_ALPHA));
    } else {
        // Full scrollbar mode
        let scrollbar_width = MIN_WIDTH + (MAX_WIDTH - MIN_WIDTH) * visibility;
        let scrollbar_x = size.width - scrollbar_width - RIGHT_OFFSET;

        // Background track
        let track_height = size.height - TOP_PADDING - BOTTOM_PADDING;
        let track_path = Path::rectangle(Point::new(scrollbar_x, TOP_PADDING), Size::new(scrollbar_width, track_height));
        frame.fill(&track_path, Color::from_rgba(0.0, 0.0, 0.0, 0.2 * visibility));

        // Thumb
        let available_height = track_height;
        let thumb_height = (available_height * height_ratio).max(30.0);
        let max_thumb_offset = available_height - thumb_height;
        let thumb_y = TOP_PADDING + (max_thumb_offset * scroll_position);

        let thumb_path = Path::rounded_rectangle(Point::new(scrollbar_x, thumb_y), Size::new(scrollbar_width, thumb_height), 4.0.into());
        frame.fill(&thumb_path, Color::from_rgba(1.0, 1.0, 1.0, MIN_ALPHA + (MAX_ALPHA - MIN_ALPHA) * visibility));

        frame.stroke(
            &thumb_path,
            Stroke::default().with_width(0.5).with_color(Color::from_rgba(1.0, 1.0, 1.0, 0.3 * visibility)),
        );
    }
}
