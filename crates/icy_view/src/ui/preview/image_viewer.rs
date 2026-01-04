//! Image viewer widget with zoom and scroll support using scroll_area
//! Provides similar UX to the Terminal view for a consistent experience

use std::sync::Arc;

use icy_engine_gui::{ScalingMode, ZoomMessage};
use icy_ui::mouse::ScrollDelta;
use icy_ui::widget::{container, image as iced_image, scroll_area};
use icy_ui::{mouse, Element, Length, Task};
use parking_lot::RwLock;

/// Scroll state updated from scroll_area callback
#[derive(Default, Clone)]
pub struct ScrollState {
    /// Current X scroll position (from scroll_area)
    pub scroll_x: f32,
    /// Current Y scroll position (from scroll_area)
    pub scroll_y: f32,
    /// Visible width (from scroll_area)
    pub visible_width: f32,
    /// Visible height (from scroll_area)
    pub visible_height: f32,
}

/// Messages from the image viewer
#[derive(Debug, Clone)]
pub enum ImageViewerMessage {
    /// Unified zoom message
    Zoom(ZoomMessage),
    /// Keyboard navigation: Home
    Home,
    /// Keyboard navigation: End
    End,
    /// Keyboard navigation: Page Up
    PageUp,
    /// Keyboard navigation: Page Down
    PageDown,
    /// Keyboard navigation: Arrow Up
    ArrowUp,
    /// Keyboard navigation: Arrow Down
    ArrowDown,
    /// Keyboard navigation: Arrow Left
    ArrowLeft,
    /// Keyboard navigation: Arrow Right
    ArrowRight,
    /// Mouse pressed at position (for drag start)
    Press((f32, f32)),
    /// Mouse released (for drag end)
    Release,
    /// Mouse moved (for cursor updates)
    Move(Option<(f32, f32)>),
    /// Scroll event (dx, dy)
    Scroll(f32, f32),
}

/// Image viewer state with zoom support (scrolling handled by scroll_area)
pub struct ImageViewer {
    /// The image handle
    handle: iced_image::Handle,
    /// Original image dimensions
    image_size: (u32, u32),
    /// Current zoom level (1.0 = 100%)
    zoom: f32,
    /// Scroll state (updated from scroll_area callback)
    pub scroll_state: Arc<RwLock<ScrollState>>,
    /// Current cursor icon override (for drag state)
    pub cursor_icon: Arc<RwLock<Option<mouse::Interaction>>>,
}

impl ImageViewer {
    /// Create a new image viewer
    pub fn new(handle: iced_image::Handle, width: u32, height: u32) -> Self {
        Self {
            handle,
            image_size: (width, height),
            zoom: 1.0,
            scroll_state: Arc::new(RwLock::new(ScrollState::default())),
            cursor_icon: Arc::new(RwLock::new(None)),
        }
    }

    /// Get current zoom level
    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    /// Set zoom level
    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = ScalingMode::clamp_zoom(zoom);
    }

    /// Zoom in
    pub fn zoom_in(&mut self) {
        self.set_zoom(ScalingMode::zoom_in(self.zoom, false));
    }

    /// Zoom out
    pub fn zoom_out(&mut self) {
        self.set_zoom(ScalingMode::zoom_out(self.zoom, false));
    }

    /// Zoom to fit viewport
    pub fn zoom_fit(&mut self) {
        let state = self.scroll_state.read();
        let vp_w = state.visible_width;
        let vp_h = state.visible_height;
        drop(state);

        if vp_w > 0.0 && vp_h > 0.0 {
            let scale_x = vp_w / self.image_size.0 as f32;
            let scale_y = vp_h / self.image_size.1 as f32;
            self.zoom = ScalingMode::clamp_zoom(scale_x.min(scale_y));
        }
    }

    /// Zoom to 100%
    pub fn zoom_100(&mut self) {
        self.set_zoom(1.0);
    }

    /// Get the zoomed image size
    pub fn zoomed_size(&self) -> (f32, f32) {
        (self.image_size.0 as f32 * self.zoom, self.image_size.1 as f32 * self.zoom)
    }

    /// Get current scroll Y position
    pub fn scroll_y(&self) -> f32 {
        self.scroll_state.read().scroll_y
    }

    /// Get current scroll X position
    pub fn scroll_x(&self) -> f32 {
        self.scroll_state.read().scroll_x
    }

    /// Get maximum scroll Y
    pub fn max_scroll_y(&self) -> f32 {
        let state = self.scroll_state.read();
        let zoomed = self.zoomed_size();
        (zoomed.1 - state.visible_height).max(0.0)
    }

    /// Get maximum scroll X
    pub fn max_scroll_x(&self) -> f32 {
        let state = self.scroll_state.read();
        let zoomed = self.zoomed_size();
        (zoomed.0 - state.visible_width).max(0.0)
    }

    /// Get visible height
    pub fn visible_height(&self) -> f32 {
        self.scroll_state.read().visible_height
    }

    /// Get visible width
    pub fn visible_width(&self) -> f32 {
        self.scroll_state.read().visible_width
    }

    /// Check if the image viewer needs animation updates
    pub fn needs_animation(&self) -> bool {
        false // scroll_area handles animations now
    }

    /// Handle a message, returns optional scroll task
    pub fn update<Message: 'static>(&mut self, message: ImageViewerMessage) -> Task<Message> {
        match message {
            ImageViewerMessage::Zoom(zoom_msg) => match zoom_msg {
                ZoomMessage::In => self.zoom_in(),
                ZoomMessage::Out => self.zoom_out(),
                ZoomMessage::Reset => self.zoom_100(),
                ZoomMessage::AutoFit => self.zoom_fit(),
                ZoomMessage::Set(z) => self.set_zoom(z),
                ZoomMessage::Wheel(delta) => {
                    let (y_delta, is_smooth) = match delta {
                        ScrollDelta::Lines { y, .. } => {
                            let sign = if y > 0.0 {
                                1.0
                            } else if y < 0.0 {
                                -1.0
                            } else {
                                0.0
                            };
                            (sign, false)
                        }
                        ScrollDelta::Pixels { y, .. } => (y / 200.0, true),
                    };
                    if y_delta != 0.0 {
                        if is_smooth {
                            self.set_zoom(self.zoom + y_delta)
                        } else if y_delta > 0.0 {
                            self.zoom_in()
                        } else {
                            self.zoom_out()
                        }
                    }
                }
            },
            // Keyboard navigation - scroll_area handles scrolling
            // These are kept for potential future programmatic scrolling needs
            ImageViewerMessage::Home
            | ImageViewerMessage::End
            | ImageViewerMessage::PageUp
            | ImageViewerMessage::PageDown
            | ImageViewerMessage::ArrowUp
            | ImageViewerMessage::ArrowDown
            | ImageViewerMessage::ArrowLeft
            | ImageViewerMessage::ArrowRight => {
                // TODO: Implement programmatic scrolling if needed
            }
            // Scroll event - no-op, scroll_area handles this
            ImageViewerMessage::Scroll(_dx, _dy) => {}
            // Drag messages are handled by PreviewView, not here
            ImageViewerMessage::Press(_) | ImageViewerMessage::Release | ImageViewerMessage::Move(_) => {}
        }
        Task::none()
    }

    /// Create the view element with scroll_area
    pub fn view<'a, Message: Clone + 'static>(&'a self, _on_message: impl Fn(ImageViewerMessage) -> Message + 'static + Clone) -> Element<'a, Message> {
        let zoomed = self.zoomed_size();
        let content_size = icy_ui::Size::new(zoomed.0, zoomed.1);
        let scroll_state = self.scroll_state.clone();
        let handle = self.handle.clone();

        // Use scroll_area with show_viewport to get scroll position
        scroll_area()
            .width(Length::Fill)
            .height(Length::Fill)
            .show_viewport(content_size, move |viewport| {
                // Update scroll state from scroll_area callback
                {
                    let mut state = scroll_state.write();
                    state.scroll_x = viewport.x;
                    state.scroll_y = viewport.y;
                    state.visible_width = viewport.width;
                    state.visible_height = viewport.height;
                }

                // Create the image element at zoomed size inside the callback
                let image_element = iced_image(handle.clone())
                    .width(Length::Fixed(content_size.width))
                    .height(Length::Fixed(content_size.height))
                    .content_fit(icy_ui::ContentFit::Fill);

                // Wrap in container with dark background
                container(image_element)
                    .width(Length::Shrink)
                    .height(Length::Shrink)
                    .style(|_theme: &icy_ui::Theme| container::Style {
                        background: Some(icy_ui::Background::Color(icy_ui::Color::from_rgb(0.08, 0.08, 0.08))),
                        ..Default::default()
                    })
                    .into()
            })
            .into()
    }
}
