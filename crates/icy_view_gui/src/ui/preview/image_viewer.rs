//! Image viewer widget with zoom and scroll support
//! Provides similar UX to the Terminal view for a consistent experience

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use iced::advanced::image::Renderer as ImageRenderer;
use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer::{self, Renderer as _};
use iced::advanced::widget::{self, Widget};
use iced::mouse::ScrollDelta;
use iced::widget::{container, image as iced_image, stack};
use iced::{Element, Event, Length, Rectangle, Size, Theme, mouse};
use icy_engine_gui::{HorizontalScrollbarOverlay, ScrollbarInfo, ScrollbarOverlay};
use parking_lot::RwLock;

/// Minimum zoom level (25%)
pub const MIN_ZOOM: f32 = 0.25;
/// Maximum zoom level (800%)
pub const MAX_ZOOM: f32 = 8.0;
/// Zoom step for each zoom in/out action (25%)
pub const ZOOM_STEP: f32 = 0.25;
/// Scrollbar fade timing
const SCROLLBAR_FADE_DELAY: f32 = 1.5;
const SCROLLBAR_FADE_SPEED: f32 = 3.0;
/// Arrow key scroll step in pixels
const ARROW_SCROLL_STEP: f32 = 50.0;

/// Messages from the image viewer
#[derive(Debug, Clone)]
pub enum ImageViewerMessage {
    /// Scroll by delta
    Scroll(f32, f32),
    /// Scroll to absolute position in pixels
    ScrollTo(f32, f32),
    /// Scroll vertical to absolute position in pixels
    ScrollYTo(f32),
    /// Scroll horizontal to absolute position in pixels
    ScrollXTo(f32),
    /// Zoom in
    ZoomIn,
    /// Zoom out
    ZoomOut,
    /// Zoom to fit
    ZoomFit,
    /// Zoom to 100%
    Zoom100,
    /// Set specific zoom level
    SetZoom(f32),
    /// Vertical scrollbar hover state changed
    VScrollbarHovered(bool),
    /// Horizontal scrollbar hover state changed
    HScrollbarHovered(bool),
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
}

/// Image viewer state with zoom and scroll support
pub struct ImageViewer {
    /// The image handle
    handle: iced_image::Handle,
    /// Original image dimensions
    image_size: (u32, u32),
    /// Current zoom level (1.0 = 100%)
    zoom: f32,
    /// Scroll offset in zoomed pixels
    scroll_offset: (f32, f32),
    /// Last known viewport size (shared with widget for updates during render)
    viewport_size: Arc<RwLock<(f32, f32)>>,
    /// Scrollbar visibility (0.0 = hidden, 1.0 = visible)
    scrollbar_visibility_v: f32,
    scrollbar_visibility_h: f32,
    /// Time since last scroll activity (separate for each scrollbar)
    scrollbar_idle_time_v: f32,
    scrollbar_idle_time_h: f32,
    /// Vertical scrollbar hover state
    pub vscrollbar_hover_state: Arc<AtomicBool>,
    /// Horizontal scrollbar hover state
    pub hscrollbar_hover_state: Arc<AtomicBool>,
}

impl ImageViewer {
    /// Create a new image viewer
    pub fn new(handle: iced_image::Handle, width: u32, height: u32) -> Self {
        Self {
            handle,
            image_size: (width, height),
            zoom: 1.0,
            scroll_offset: (0.0, 0.0),
            viewport_size: Arc::new(RwLock::new((800.0, 600.0))),
            scrollbar_visibility_v: 0.0,
            scrollbar_visibility_h: 0.0,
            scrollbar_idle_time_v: 0.0,
            scrollbar_idle_time_h: 0.0,
            vscrollbar_hover_state: Arc::new(AtomicBool::new(false)),
            hscrollbar_hover_state: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Update the image
    pub fn set_image(&mut self, handle: iced_image::Handle, width: u32, height: u32) {
        self.handle = handle;
        self.image_size = (width, height);
        self.scroll_offset = (0.0, 0.0);
        // Auto-fit when loading new image
        self.zoom = 1.0;
    }

    /// Get current zoom level
    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    /// Get current viewport size
    fn get_viewport_size(&self) -> (f32, f32) {
        *self.viewport_size.read()
    }

    /// Set zoom level
    pub fn set_zoom(&mut self, zoom: f32) {
        let old_zoom = self.zoom;
        self.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);

        // Adjust scroll to keep center point stable
        if old_zoom != self.zoom {
            let vp = self.get_viewport_size();
            let ratio = self.zoom / old_zoom;
            let center_x = self.scroll_offset.0 + vp.0 / 2.0;
            let center_y = self.scroll_offset.1 + vp.1 / 2.0;
            self.scroll_offset.0 = center_x * ratio - vp.0 / 2.0;
            self.scroll_offset.1 = center_y * ratio - vp.1 / 2.0;
            self.clamp_scroll();
        }
    }

    /// Zoom in
    pub fn zoom_in(&mut self) {
        self.set_zoom(self.zoom + ZOOM_STEP);
        self.show_scrollbars();
    }

    /// Zoom out
    pub fn zoom_out(&mut self) {
        self.set_zoom(self.zoom - ZOOM_STEP);
        self.show_scrollbars();
    }

    /// Zoom to fit viewport
    pub fn zoom_fit(&mut self) {
        let vp = self.get_viewport_size();
        if vp.0 > 0.0 && vp.1 > 0.0 {
            let scale_x = vp.0 / self.image_size.0 as f32;
            let scale_y = vp.1 / self.image_size.1 as f32;
            self.zoom = scale_x.min(scale_y).clamp(MIN_ZOOM, MAX_ZOOM);
            self.scroll_offset = (0.0, 0.0);
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

    /// Get maximum scroll offsets
    fn max_scroll(&self) -> (f32, f32) {
        let zoomed = self.zoomed_size();
        let vp = self.get_viewport_size();
        ((zoomed.0 - vp.0).max(0.0), (zoomed.1 - vp.1).max(0.0))
    }

    /// Clamp scroll offset to valid range
    fn clamp_scroll(&mut self) {
        let max = self.max_scroll();
        self.scroll_offset.0 = self.scroll_offset.0.clamp(0.0, max.0);
        self.scroll_offset.1 = self.scroll_offset.1.clamp(0.0, max.1);
    }

    /// Scroll by delta (in screen pixels)
    pub fn scroll(&mut self, dx: f32, dy: f32) {
        self.scroll_offset.0 += dx;
        self.scroll_offset.1 += dy;
        self.clamp_scroll();
        // Show only the scrollbar(s) that are actually scrolling
        if dx.abs() > 0.01 {
            self.show_hscrollbar();
        }
        if dy.abs() > 0.01 {
            self.show_vscrollbar();
        }
    }

    /// Scroll to absolute position in pixels
    pub fn scroll_to(&mut self, x: f32, y: f32) {
        self.scroll_offset.0 = x;
        self.scroll_offset.1 = y;
        self.clamp_scroll();
        self.show_scrollbars();
    }

    /// Scroll vertical to absolute position in pixels
    pub fn scroll_y_to(&mut self, y: f32) {
        self.scroll_offset.1 = y;
        self.clamp_scroll();
        self.show_vscrollbar();
    }

    /// Scroll horizontal to absolute position in pixels
    pub fn scroll_x_to(&mut self, x: f32) {
        self.scroll_offset.0 = x;
        self.clamp_scroll();
        self.show_hscrollbar();
    }

    /// Scroll to home (top-left)
    pub fn scroll_home(&mut self) {
        self.scroll_offset = (0.0, 0.0);
        self.show_scrollbars();
    }

    /// Scroll to end (bottom-right)
    pub fn scroll_end(&mut self) {
        let max = self.max_scroll();
        self.scroll_offset = (max.0, max.1);
        self.show_scrollbars();
    }

    /// Scroll page up
    pub fn scroll_page_up(&mut self) {
        let vp = self.get_viewport_size();
        self.scroll_offset.1 -= vp.1 * 0.9;
        self.clamp_scroll();
        self.show_vscrollbar();
    }

    /// Scroll page down
    pub fn scroll_page_down(&mut self) {
        let vp = self.get_viewport_size();
        self.scroll_offset.1 += vp.1 * 0.9;
        self.clamp_scroll();
        self.show_vscrollbar();
    }

    /// Scroll arrow up
    pub fn scroll_arrow_up(&mut self) {
        self.scroll_offset.1 -= ARROW_SCROLL_STEP;
        self.clamp_scroll();
        self.show_vscrollbar();
    }

    /// Scroll arrow down
    pub fn scroll_arrow_down(&mut self) {
        self.scroll_offset.1 += ARROW_SCROLL_STEP;
        self.clamp_scroll();
        self.show_vscrollbar();
    }

    /// Scroll arrow left
    pub fn scroll_arrow_left(&mut self) {
        self.scroll_offset.0 -= ARROW_SCROLL_STEP;
        self.clamp_scroll();
        self.show_hscrollbar();
    }

    /// Scroll arrow right
    pub fn scroll_arrow_right(&mut self) {
        self.scroll_offset.0 += ARROW_SCROLL_STEP;
        self.clamp_scroll();
        self.show_hscrollbar();
    }

    /// Show scrollbars (reset fade timer)
    fn show_scrollbars(&mut self) {
        self.show_vscrollbar();
        self.show_hscrollbar();
    }

    /// Show vertical scrollbar
    fn show_vscrollbar(&mut self) {
        self.scrollbar_idle_time_v = 0.0;
        self.scrollbar_visibility_v = 1.0;
    }

    /// Show horizontal scrollbar
    fn show_hscrollbar(&mut self) {
        self.scrollbar_idle_time_h = 0.0;
        self.scrollbar_visibility_h = 1.0;
    }

    /// Update scrollbar fade animation
    pub fn update_scrollbars(&mut self, dt: f32) {
        let is_hovering_v = self.vscrollbar_hover_state.load(Ordering::Relaxed);
        let is_hovering_h = self.hscrollbar_hover_state.load(Ordering::Relaxed);

        // Update vertical scrollbar
        if is_hovering_v {
            self.scrollbar_idle_time_v = 0.0;
            self.scrollbar_visibility_v = 1.0;
        } else {
            self.scrollbar_idle_time_v += dt;
            if self.scrollbar_idle_time_v > SCROLLBAR_FADE_DELAY {
                let fade = (self.scrollbar_idle_time_v - SCROLLBAR_FADE_DELAY) * SCROLLBAR_FADE_SPEED;
                self.scrollbar_visibility_v = (1.0 - fade).max(0.0);
            }
        }

        // Update horizontal scrollbar
        if is_hovering_h {
            self.scrollbar_idle_time_h = 0.0;
            self.scrollbar_visibility_h = 1.0;
        } else {
            self.scrollbar_idle_time_h += dt;
            if self.scrollbar_idle_time_h > SCROLLBAR_FADE_DELAY {
                let fade = (self.scrollbar_idle_time_h - SCROLLBAR_FADE_DELAY) * SCROLLBAR_FADE_SPEED;
                self.scrollbar_visibility_h = (1.0 - fade).max(0.0);
            }
        }
    }

    /// Set viewport size
    pub fn set_viewport_size(&mut self, width: f32, height: f32) {
        let current = self.get_viewport_size();
        if (current.0 - width).abs() > 1.0 || (current.1 - height).abs() > 1.0 {
            *self.viewport_size.write() = (width, height);
            self.clamp_scroll();
        }
    }

    /// Check if the image viewer needs animation updates (for scrollbar fade)
    pub fn needs_animation(&self) -> bool {
        // Need animation if scrollbars are visible or fading
        let is_hovering_v = self.vscrollbar_hover_state.load(Ordering::Relaxed);
        let is_hovering_h = self.hscrollbar_hover_state.load(Ordering::Relaxed);

        let fade_duration = SCROLLBAR_FADE_DELAY + 1.0 / SCROLLBAR_FADE_SPEED;

        // Animation needed if:
        // - Scrollbars are still visible (fading out)
        // - User is hovering (keep visible)
        // - Within fade delay period
        is_hovering_v
            || is_hovering_h
            || self.scrollbar_visibility_v > 0.0
            || self.scrollbar_visibility_h > 0.0
            || self.scrollbar_idle_time_v < fade_duration
            || self.scrollbar_idle_time_h < fade_duration
    }

    /// Get scrollbar info for rendering scrollbars
    pub fn scrollbar_info(&self) -> ScrollbarInfo {
        let zoomed = self.zoomed_size();
        let max = self.max_scroll();
        let vp = self.get_viewport_size();

        let needs_vscrollbar = zoomed.1 > vp.1;
        let needs_hscrollbar = zoomed.0 > vp.0;

        let height_ratio = if zoomed.1 > 0.0 { (vp.1 / zoomed.1).min(1.0) } else { 1.0 };

        let width_ratio = if zoomed.0 > 0.0 { (vp.0 / zoomed.0).min(1.0) } else { 1.0 };

        let scroll_position_v = if max.1 > 0.0 { self.scroll_offset.1 / max.1 } else { 0.0 };

        let scroll_position_h = if max.0 > 0.0 { self.scroll_offset.0 / max.0 } else { 0.0 };

        ScrollbarInfo {
            needs_vscrollbar,
            needs_hscrollbar,
            visibility_v: self.scrollbar_visibility_v,
            visibility_h: self.scrollbar_visibility_h,
            scroll_position_v,
            scroll_position_h,
            height_ratio,
            width_ratio,
            max_scroll_y: max.1,
            max_scroll_x: max.0,
        }
    }

    /// Handle a message
    pub fn update(&mut self, message: ImageViewerMessage) {
        match message {
            ImageViewerMessage::Scroll(dx, dy) => self.scroll(dx, dy),
            ImageViewerMessage::ScrollTo(x, y) => self.scroll_to(x, y),
            ImageViewerMessage::ScrollYTo(y) => self.scroll_y_to(y),
            ImageViewerMessage::ScrollXTo(x) => self.scroll_x_to(x),
            ImageViewerMessage::ZoomIn => self.zoom_in(),
            ImageViewerMessage::ZoomOut => self.zoom_out(),
            ImageViewerMessage::ZoomFit => self.zoom_fit(),
            ImageViewerMessage::Zoom100 => self.zoom_100(),
            ImageViewerMessage::SetZoom(z) => self.set_zoom(z),
            ImageViewerMessage::VScrollbarHovered(h) => {
                self.vscrollbar_hover_state.store(h, Ordering::Relaxed);
            }
            ImageViewerMessage::HScrollbarHovered(h) => {
                self.hscrollbar_hover_state.store(h, Ordering::Relaxed);
            }
            ImageViewerMessage::Home => self.scroll_home(),
            ImageViewerMessage::End => self.scroll_end(),
            ImageViewerMessage::PageUp => self.scroll_page_up(),
            ImageViewerMessage::PageDown => self.scroll_page_down(),
            ImageViewerMessage::ArrowUp => self.scroll_arrow_up(),
            ImageViewerMessage::ArrowDown => self.scroll_arrow_down(),
            ImageViewerMessage::ArrowLeft => self.scroll_arrow_left(),
            ImageViewerMessage::ArrowRight => self.scroll_arrow_right(),
        }
    }

    /// Create the view element with scrollbars
    pub fn view<'a, Message: Clone + 'static>(&'a self, on_message: impl Fn(ImageViewerMessage) -> Message + 'static + Clone) -> Element<'a, Message> {
        // Create the image view widget
        let image_widget: Element<'a, Message> = ImageViewWidget::new(
            self.handle.clone(),
            self.image_size,
            self.zoom,
            self.scroll_offset,
            self.viewport_size.clone(),
            on_message.clone(),
        )
        .into();

        let scrollbar_info = self.scrollbar_info();

        if scrollbar_info.needs_any_scrollbar() {
            let mut layers: Vec<Element<'a, Message>> = vec![image_widget];

            // Add vertical scrollbar if needed
            if scrollbar_info.needs_vscrollbar {
                let on_msg = on_message.clone();
                let on_msg2 = on_message.clone();
                let vscrollbar = ScrollbarOverlay::new(
                    scrollbar_info.visibility_v,
                    scrollbar_info.scroll_position_v,
                    scrollbar_info.height_ratio,
                    scrollbar_info.max_scroll_y,
                    self.vscrollbar_hover_state.clone(),
                    move |_x, y| on_msg(ImageViewerMessage::ScrollYTo(y)),
                    move |h| on_msg2(ImageViewerMessage::VScrollbarHovered(h)),
                )
                .view();

                let vscrollbar_container = container(vscrollbar).width(Length::Fill).height(Length::Fill).align_x(iced::Alignment::End);
                layers.push(vscrollbar_container.into());
            }

            // Add horizontal scrollbar if needed
            if scrollbar_info.needs_hscrollbar {
                let on_msg = on_message.clone();
                let on_msg2 = on_message.clone();
                let hscrollbar = HorizontalScrollbarOverlay::new(
                    scrollbar_info.visibility_h,
                    scrollbar_info.scroll_position_h,
                    scrollbar_info.width_ratio,
                    scrollbar_info.max_scroll_x,
                    self.hscrollbar_hover_state.clone(),
                    move |x, _y| on_msg(ImageViewerMessage::ScrollXTo(x)),
                    move |h| on_msg2(ImageViewerMessage::HScrollbarHovered(h)),
                )
                .view();

                let hscrollbar_container = container(hscrollbar).width(Length::Fill).height(Length::Fill).align_y(iced::Alignment::End);
                layers.push(hscrollbar_container.into());
            }

            stack(layers).into()
        } else {
            image_widget
        }
    }
}

/// Custom widget for rendering the scrollable/zoomable image
struct ImageViewWidget<Message> {
    handle: iced_image::Handle,
    image_size: (u32, u32),
    zoom: f32,
    scroll_offset: (f32, f32),
    viewport_size: Arc<RwLock<(f32, f32)>>,
    on_message: Box<dyn Fn(ImageViewerMessage) -> Message>,
}

impl<Message> ImageViewWidget<Message> {
    fn new(
        handle: iced_image::Handle,
        image_size: (u32, u32),
        zoom: f32,
        scroll_offset: (f32, f32),
        viewport_size: Arc<RwLock<(f32, f32)>>,
        on_message: impl Fn(ImageViewerMessage) -> Message + 'static,
    ) -> Self {
        Self {
            handle,
            image_size,
            zoom,
            scroll_offset,
            viewport_size,
            on_message: Box::new(on_message),
        }
    }

    fn zoomed_size(&self) -> (f32, f32) {
        (self.image_size.0 as f32 * self.zoom, self.image_size.1 as f32 * self.zoom)
    }

    fn get_viewport_size(&self) -> (f32, f32) {
        *self.viewport_size.read()
    }
}

impl<Message: Clone> Widget<Message, Theme, iced::Renderer> for ImageViewWidget<Message> {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(&mut self, _tree: &mut widget::Tree, _renderer: &iced::Renderer, limits: &layout::Limits) -> layout::Node {
        let size = limits.max();
        // Update shared viewport size
        *self.viewport_size.write() = (size.width, size.height);
        layout::Node::new(size)
    }

    fn draw(
        &self,
        _tree: &widget::Tree,
        renderer: &mut iced::Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();

        // Draw dark background
        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: iced::Border::default(),
                shadow: iced::Shadow::default(),
                snap: true,
            },
            iced::Color::from_rgb(0.08, 0.08, 0.08),
        );

        let zoomed = self.zoomed_size();

        // Calculate image position (centered if smaller than viewport)
        let x = if zoomed.0 < bounds.width {
            bounds.x + (bounds.width - zoomed.0) / 2.0
        } else {
            bounds.x - self.scroll_offset.0
        };

        let y = if zoomed.1 < bounds.height {
            bounds.y + (bounds.height - zoomed.1) / 2.0
        } else {
            bounds.y - self.scroll_offset.1
        };

        // Create clipped region for the image
        let image_bounds = Rectangle {
            x,
            y,
            width: zoomed.0,
            height: zoomed.1,
        };

        // Use renderer's with_layer for clipping
        renderer.with_layer(bounds, |renderer| {
            // Draw the image using iced's image rendering
            let image = iced::advanced::image::Image::<iced_image::Handle> {
                handle: self.handle.clone(),
                filter_method: iced::advanced::image::FilterMethod::Linear,
                rotation: iced::Radians(0.0),
                opacity: 1.0,
                snap: true,
                border_radius: iced::border::Radius::default(),
            };

            renderer.draw_image(image, image_bounds, bounds);
        });
    }

    fn update(
        &mut self,
        _tree: &mut widget::Tree,
        event: &iced::Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &iced::Renderer,
        _clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();

        match event {
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                if cursor.is_over(bounds) {
                    // Check if Ctrl is held for zooming
                    let ctrl_pressed = icy_engine_gui::is_ctrl_pressed();
                    if ctrl_pressed {
                        let zoom_delta = match delta {
                            ScrollDelta::Lines { y, .. } => *y,
                            ScrollDelta::Pixels { y, .. } => *y / 100.0,
                        };
                        if zoom_delta > 0.0 {
                            shell.publish((self.on_message)(ImageViewerMessage::ZoomIn));
                        } else if zoom_delta < 0.0 {
                            shell.publish((self.on_message)(ImageViewerMessage::ZoomOut));
                        }
                        return;
                    }

                    // Normal scroll
                    let (dx, dy) = match delta {
                        ScrollDelta::Lines { x, y } => (*x * 50.0, *y * 50.0),
                        ScrollDelta::Pixels { x, y } => (*x, *y),
                    };
                    shell.publish((self.on_message)(ImageViewerMessage::Scroll(-dx, -dy)));
                    return;
                }
            }
            Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. }) => {
                use iced::keyboard::{Key, key::Named};
                match key {
                    Key::Named(Named::Home) => {
                        shell.publish((self.on_message)(ImageViewerMessage::Home));
                    }
                    Key::Named(Named::End) => {
                        shell.publish((self.on_message)(ImageViewerMessage::End));
                    }
                    Key::Named(Named::PageUp) => {
                        shell.publish((self.on_message)(ImageViewerMessage::PageUp));
                    }
                    Key::Named(Named::PageDown) => {
                        shell.publish((self.on_message)(ImageViewerMessage::PageDown));
                    }
                    Key::Named(Named::ArrowUp) => {
                        shell.publish((self.on_message)(ImageViewerMessage::ArrowUp));
                    }
                    Key::Named(Named::ArrowDown) => {
                        shell.publish((self.on_message)(ImageViewerMessage::ArrowDown));
                    }
                    Key::Named(Named::ArrowLeft) => {
                        shell.publish((self.on_message)(ImageViewerMessage::ArrowLeft));
                    }
                    Key::Named(Named::ArrowRight) => {
                        shell.publish((self.on_message)(ImageViewerMessage::ArrowRight));
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &widget::Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        let bounds = layout.bounds();
        if cursor.is_over(bounds) {
            let zoomed = self.zoomed_size();
            if zoomed.0 > bounds.width || zoomed.1 > bounds.height {
                return mouse::Interaction::Grab;
            }
        }
        mouse::Interaction::default()
    }
}

impl<'a, Message: Clone + 'static> From<ImageViewWidget<Message>> for Element<'a, Message> {
    fn from(widget: ImageViewWidget<Message>) -> Self {
        Element::new(widget)
    }
}
