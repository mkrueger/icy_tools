//! Image viewer widget with zoom and scroll support
//! Provides similar UX to the Terminal view for a consistent experience

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use iced::advanced::image::Renderer as ImageRenderer;
use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer::{self, Renderer as _};
use iced::advanced::widget::{self, Widget};
use iced::mouse::ScrollDelta;
use iced::widget::{container, image as iced_image, stack};
use iced::{Element, Event, Length, Rectangle, Size, Theme, mouse};
use icy_engine_gui::{HorizontalScrollbarOverlayCallback, ScalingMode, ScrollbarOverlayCallback, ScrollbarState, Viewport, ZoomMessage};
use parking_lot::RwLock;

/// Arrow key scroll step in pixels
const ARROW_SCROLL_STEP: f32 = 50.0;
/// Page scroll factor (percentage of viewport)
const PAGE_SCROLL_FACTOR: f32 = 0.9;

/// Messages from the image viewer
#[derive(Debug, Clone)]
pub enum ImageViewerMessage {
    /// Scroll by delta (direct, no animation - for mouse wheel)
    Scroll(f32, f32),
    /// Scroll by delta with smooth animation (for PageUp/PageDown)
    ScrollSmooth(f32, f32),
    /// Scroll to absolute position (direct, no animation)
    ScrollTo(f32, f32),
    /// Scroll to absolute position with smooth animation (for Home/End)
    ScrollToSmooth(f32, f32),
    /// Scroll vertical to absolute position in pixels (scrollbar)
    ScrollYTo(f32),
    /// Scroll horizontal to absolute position in pixels (scrollbar)
    ScrollXTo(f32),
    /// Unified zoom message
    Zoom(ZoomMessage),
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
    /// Mouse pressed at position (for drag start)
    Press((f32, f32)),
    /// Mouse released (for drag end)
    Release,
    /// Mouse dragged to position
    Drag((f32, f32)),
    /// Mouse moved (for cursor updates)
    Move(Option<(f32, f32)>),
}

/// Local scrollbar info for ImageViewer (uses custom viewport, not Terminal)
struct ImageScrollbarInfo {
    needs_vscrollbar: bool,
    needs_hscrollbar: bool,
    visibility_v: f32,
    visibility_h: f32,
    scroll_position_v: f32,
    scroll_position_h: f32,
    height_ratio: f32,
    width_ratio: f32,
    max_scroll_y: f32,
    max_scroll_x: f32,
}

impl ImageScrollbarInfo {
    fn needs_any_scrollbar(&self) -> bool {
        self.needs_vscrollbar || self.needs_hscrollbar
    }
}

/// Image viewer state with zoom and scroll support
pub struct ImageViewer {
    /// The image handle
    handle: iced_image::Handle,
    /// Original image dimensions
    image_size: (u32, u32),
    /// Current zoom level (1.0 = 100%)
    zoom: f32,
    /// Viewport for scroll management (uses Viewport from icy_engine_gui)
    pub viewport: Viewport,
    /// Scrollbar state (uses ScrollbarState from icy_engine_gui)
    pub scrollbar: ScrollbarState,
    /// Vertical scrollbar hover state
    pub vscrollbar_hover_state: Arc<AtomicBool>,
    /// Horizontal scrollbar hover state
    pub hscrollbar_hover_state: Arc<AtomicBool>,
    /// Current cursor icon override (for drag state)
    pub cursor_icon: Arc<parking_lot::RwLock<Option<mouse::Interaction>>>,
}

impl ImageViewer {
    /// Create a new image viewer
    pub fn new(handle: iced_image::Handle, width: u32, height: u32) -> Self {
        let mut viewport = Viewport::default();
        viewport.set_content_size(width as f32, height as f32);

        Self {
            handle,
            image_size: (width, height),
            zoom: 1.0,
            viewport,
            scrollbar: ScrollbarState::new(),
            vscrollbar_hover_state: Arc::new(AtomicBool::new(false)),
            hscrollbar_hover_state: Arc::new(AtomicBool::new(false)),
            cursor_icon: Arc::new(RwLock::new(None)),
        }
    }

    /// Update the image
    pub fn set_image(&mut self, handle: iced_image::Handle, width: u32, height: u32) {
        self.handle = handle;
        self.image_size = (width, height);
        self.zoom = 1.0;
        // Reset viewport for new image
        self.viewport.scroll_x_to(0.0);
        self.viewport.scroll_y_to(0.0);
        self.update_content_size();
    }

    /// Get current zoom level
    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    /// Update content size based on zoom
    fn update_content_size(&mut self) {
        let zoomed = self.zoomed_size();
        self.viewport.set_content_size(zoomed.0, zoomed.1);
    }

    /// Set zoom level
    pub fn set_zoom(&mut self, zoom: f32) {
        let old_zoom = self.zoom;
        self.zoom = ScalingMode::clamp_zoom(zoom);

        // Adjust scroll to keep center point stable
        if old_zoom != self.zoom {
            let ratio = self.zoom / old_zoom;
            let center_x = self.viewport.scroll_x + self.viewport.visible_width / 2.0;
            let center_y = self.viewport.scroll_y + self.viewport.visible_height / 2.0;
            let new_x = center_x * ratio - self.viewport.visible_width / 2.0;
            let new_y = center_y * ratio - self.viewport.visible_height / 2.0;

            self.update_content_size();
            self.viewport.scroll_x_to(new_x);
            self.viewport.scroll_y_to(new_y);
        }
        self.scrollbar.mark_interaction(true);
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
        let vp_w = self.viewport.visible_width;
        let vp_h = self.viewport.visible_height;
        if vp_w > 0.0 && vp_h > 0.0 {
            let scale_x = vp_w / self.image_size.0 as f32;
            let scale_y = vp_h / self.image_size.1 as f32;
            self.zoom = ScalingMode::clamp_zoom(scale_x.min(scale_y));
            self.update_content_size();
            self.viewport.scroll_x_to(0.0);
            self.viewport.scroll_y_to(0.0);
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

    /// Scroll by delta (direct, no animation - for mouse wheel/arrows)
    pub fn scroll(&mut self, dx: f32, dy: f32) {
        if dx.abs() > 0.01 {
            self.viewport.scroll_x_by(dx);
        }
        if dy.abs() > 0.01 {
            self.viewport.scroll_y_by(dy);
        }
        self.sync_scrollbar();
    }

    /// Scroll by delta with smooth animation (for PageUp/PageDown)
    pub fn scroll_smooth(&mut self, dx: f32, dy: f32) {
        if dx.abs() > 0.01 {
            self.viewport.scroll_x_by_smooth(dx);
        }
        if dy.abs() > 0.01 {
            self.viewport.scroll_y_by_smooth(dy);
        }
        self.sync_scrollbar();
    }

    /// Scroll to absolute position (direct, no animation)
    pub fn scroll_to(&mut self, x: f32, y: f32) {
        self.viewport.scroll_x_to(x);
        self.viewport.scroll_y_to(y);
        self.sync_scrollbar();
    }

    /// Scroll to absolute position with smooth animation (for Home/End)
    pub fn scroll_to_smooth(&mut self, x: f32, y: f32) {
        self.viewport.scroll_x_to_smooth(x);
        self.viewport.scroll_y_to_smooth(y);
        self.sync_scrollbar();
    }

    /// Scroll vertical to absolute position (scrollbar)
    pub fn scroll_y_to(&mut self, y: f32) {
        self.viewport.scroll_y_to(y);
        self.sync_scrollbar();
    }

    /// Scroll horizontal to absolute position (scrollbar)
    pub fn scroll_x_to(&mut self, x: f32) {
        self.viewport.scroll_x_to(x);
        self.sync_scrollbar();
    }

    /// Synchronize scrollbar with viewport
    fn sync_scrollbar(&mut self) {
        let max_x = self.viewport.max_scroll_x();
        let max_y = self.viewport.max_scroll_y();

        if max_y > 0.0 {
            self.scrollbar.set_scroll_position(self.viewport.scroll_y / max_y);
            self.scrollbar.mark_interaction(true);
        }
        if max_x > 0.0 {
            self.scrollbar.set_scroll_position_x(self.viewport.scroll_x / max_x);
            self.scrollbar.mark_interaction_x(true);
        }
    }

    /// Scroll to home (top-left) with animation
    pub fn scroll_home(&mut self) {
        self.scroll_to_smooth(0.0, 0.0);
    }

    /// Scroll to end (bottom-right) with animation
    pub fn scroll_end(&mut self) {
        let max_x = self.viewport.max_scroll_x();
        let max_y = self.viewport.max_scroll_y();
        self.scroll_to_smooth(max_x, max_y);
    }

    /// Scroll page up with animation
    pub fn scroll_page_up(&mut self) {
        let page_height = self.viewport.visible_height * PAGE_SCROLL_FACTOR;
        self.scroll_smooth(0.0, -page_height);
    }

    /// Scroll page down with animation
    pub fn scroll_page_down(&mut self) {
        let page_height = self.viewport.visible_height * PAGE_SCROLL_FACTOR;
        self.scroll_smooth(0.0, page_height);
    }

    /// Scroll arrow up (direct)
    pub fn scroll_arrow_up(&mut self) {
        self.scroll(0.0, -ARROW_SCROLL_STEP);
    }

    /// Scroll arrow down (direct)
    pub fn scroll_arrow_down(&mut self) {
        self.scroll(0.0, ARROW_SCROLL_STEP);
    }

    /// Scroll arrow left (direct)
    pub fn scroll_arrow_left(&mut self) {
        self.scroll(-ARROW_SCROLL_STEP, 0.0);
    }

    /// Scroll arrow right (direct)
    pub fn scroll_arrow_right(&mut self) {
        self.scroll(ARROW_SCROLL_STEP, 0.0);
    }

    /// Update animations (viewport smooth scroll + scrollbar fade)
    pub fn update_scrollbars(&mut self, _dt: f32) {
        // Update viewport smooth scroll animation
        self.viewport.update_animation();

        // Sync scrollbar position after animation update
        self.sync_scrollbar();

        // Update scrollbar fade animation
        self.scrollbar.update_animation();
    }

    /// Set viewport size
    pub fn set_viewport_size(&mut self, width: f32, height: f32) {
        self.viewport.set_visible_size(width, height);
        self.update_content_size();
    }

    /// Check if the image viewer needs animation updates
    pub fn needs_animation(&self) -> bool {
        let vp = self.viewport.is_animating();
        let sb = self.scrollbar.needs_animation();
        let result = vp || sb;
        if sb {
            log::info!("ImageViewer needs_animation: vp={}, sb={} -> {}", vp, sb, result);
        }
        result
    }

    /// Get scrollbar info for rendering scrollbars
    fn scrollbar_info(&self) -> ImageScrollbarInfo {
        let zoomed = self.zoomed_size();
        let vp_w = self.viewport.visible_width;
        let vp_h = self.viewport.visible_height;

        let needs_vscrollbar = zoomed.1 > vp_h;
        let needs_hscrollbar = zoomed.0 > vp_w;

        let height_ratio = if zoomed.1 > 0.0 { (vp_h / zoomed.1).min(1.0) } else { 1.0 };
        let width_ratio = if zoomed.0 > 0.0 { (vp_w / zoomed.0).min(1.0) } else { 1.0 };

        let max_x = self.viewport.max_scroll_x();
        let max_y = self.viewport.max_scroll_y();

        let scroll_position_v = if max_y > 0.0 { self.viewport.scroll_y / max_y } else { 0.0 };
        let scroll_position_h = if max_x > 0.0 { self.viewport.scroll_x / max_x } else { 0.0 };

        ImageScrollbarInfo {
            needs_vscrollbar,
            needs_hscrollbar,
            visibility_v: self.scrollbar.visibility,
            visibility_h: self.scrollbar.visibility_x,
            scroll_position_v,
            scroll_position_h,
            height_ratio,
            width_ratio,
            max_scroll_y: max_y,
            max_scroll_x: max_x,
        }
    }

    /// Handle a message
    pub fn update(&mut self, message: ImageViewerMessage) {
        match message {
            ImageViewerMessage::Scroll(dx, dy) => self.scroll(dx, dy),
            ImageViewerMessage::ScrollSmooth(dx, dy) => self.scroll_smooth(dx, dy),
            ImageViewerMessage::ScrollTo(x, y) => self.scroll_to(x, y),
            ImageViewerMessage::ScrollToSmooth(x, y) => self.scroll_to_smooth(x, y),
            ImageViewerMessage::ScrollYTo(y) => self.scroll_y_to(y),
            ImageViewerMessage::ScrollXTo(x) => self.scroll_x_to(x),
            ImageViewerMessage::Zoom(zoom_msg) => {
                match zoom_msg {
                    ZoomMessage::In => self.zoom_in(),
                    ZoomMessage::Out => self.zoom_out(),
                    ZoomMessage::Reset => self.zoom_100(),
                    ZoomMessage::AutoFit => self.zoom_fit(),
                    ZoomMessage::Set(z) => self.set_zoom(z),
                    ZoomMessage::Wheel(delta) => {
                        // Extract y-axis delta and determine zoom behavior
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
                }
            }
            ImageViewerMessage::VScrollbarHovered(h) => {
                self.scrollbar.set_hovered(h);
            }
            ImageViewerMessage::HScrollbarHovered(h) => {
                self.scrollbar.set_hovered_x(h);
            }
            ImageViewerMessage::Home => self.scroll_home(),
            ImageViewerMessage::End => self.scroll_end(),
            ImageViewerMessage::PageUp => self.scroll_page_up(),
            ImageViewerMessage::PageDown => self.scroll_page_down(),
            ImageViewerMessage::ArrowUp => self.scroll_arrow_up(),
            ImageViewerMessage::ArrowDown => self.scroll_arrow_down(),
            ImageViewerMessage::ArrowLeft => self.scroll_arrow_left(),
            ImageViewerMessage::ArrowRight => self.scroll_arrow_right(),
            // Drag messages are handled by PreviewView, not here
            ImageViewerMessage::Press(_) | ImageViewerMessage::Release | ImageViewerMessage::Drag(_) | ImageViewerMessage::Move(_) => {}
        }
    }

    /// Create the view element with scrollbars
    pub fn view<'a, Message: Clone + 'static>(&'a self, on_message: impl Fn(ImageViewerMessage) -> Message + 'static + Clone) -> Element<'a, Message> {
        // Create the image view widget
        let image_widget: Element<'a, Message> = ImageViewWidget::new(
            self.handle.clone(),
            self.image_size,
            self.zoom,
            (self.viewport.scroll_x, self.viewport.scroll_y),
            on_message.clone(),
            self.cursor_icon.clone(),
        )
        .into();

        let scrollbar_info = self.scrollbar_info();

        if scrollbar_info.needs_any_scrollbar() {
            let mut layers: Vec<Element<'a, Message>> = vec![image_widget];

            // Add vertical scrollbar if needed
            if scrollbar_info.needs_vscrollbar {
                let on_msg = on_message.clone();
                let on_msg2 = on_message.clone();
                let vscrollbar = ScrollbarOverlayCallback::new(
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
                let hscrollbar = HorizontalScrollbarOverlayCallback::new(
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
    on_message: Box<dyn Fn(ImageViewerMessage) -> Message>,
    cursor_icon: Arc<RwLock<Option<mouse::Interaction>>>,
}

impl<Message> ImageViewWidget<Message> {
    fn new(
        handle: iced_image::Handle,
        image_size: (u32, u32),
        zoom: f32,
        scroll_offset: (f32, f32),
        on_message: impl Fn(ImageViewerMessage) -> Message + 'static,
        cursor_icon: Arc<RwLock<Option<mouse::Interaction>>>,
    ) -> Self {
        Self {
            handle,
            image_size,
            zoom,
            scroll_offset,
            on_message: Box::new(on_message),
            cursor_icon,
        }
    }

    fn zoomed_size(&self) -> (f32, f32) {
        (self.image_size.0 as f32 * self.zoom, self.image_size.1 as f32 * self.zoom)
    }
}

impl<Message: Clone> Widget<Message, Theme, iced::Renderer> for ImageViewWidget<Message> {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(&mut self, _tree: &mut widget::Tree, _renderer: &iced::Renderer, limits: &layout::Limits) -> layout::Node {
        let size = limits.max();
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
        let zoomed = self.zoomed_size();
        let can_scroll = zoomed.0 > bounds.width || zoomed.1 > bounds.height;

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if can_scroll {
                    if let Some(pos) = cursor.position_in(bounds) {
                        shell.publish((self.on_message)(ImageViewerMessage::Press((pos.x, pos.y))));
                    }
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                // Always send release to handle drags that end outside bounds
                shell.publish((self.on_message)(ImageViewerMessage::Release));
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                // Always send Move with global position for drag tracking
                // preview_view decides if it's a drag based on is_dragging state
                let global_pos = (position.x - bounds.x, position.y - bounds.y);
                shell.publish((self.on_message)(ImageViewerMessage::Move(Some(global_pos))));
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                if cursor.is_over(bounds) {
                    // Check if Cmd/Ctrl is held for zooming (Cmd on macOS, Ctrl on Windows/Linux)
                    let command_pressed = icy_engine_gui::is_command_pressed();
                    if command_pressed {
                        // Pass the scroll delta directly to ZoomMessage::Wheel
                        shell.publish((self.on_message)(ImageViewerMessage::Zoom(ZoomMessage::Wheel(*delta))));
                        return;
                    }

                    // Normal scroll (direct, no animation)
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
        // Check for cursor override (e.g., during drag)
        if let Some(cursor_override) = *self.cursor_icon.read() {
            return cursor_override;
        }

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
