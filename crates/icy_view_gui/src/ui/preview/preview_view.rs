use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use i18n_embed_fl::fl;
use iced::{
    Alignment, Element, Length, Task,
    widget::{Space, column, container, image as iced_image, stack, text},
};
use icy_engine::{Screen, Size, TextScreen};
use icy_engine_gui::{HorizontalScrollbarOverlay, MonitorSettings, ScrollbarOverlay, Terminal, TerminalView};
use icy_parser_core::BaudEmulation;
use icy_sauce::SauceRecord;
use parking_lot::Mutex;
use tokio::sync::mpsc;

use super::image_viewer::{ImageViewer, ImageViewerMessage};
use super::view_thread::{ScrollMode, ViewCommand, ViewEvent, create_view_thread};
use crate::ui::options::ScrollSpeed;
use crate::ui::theme;
use icy_engine::formats::{FileFormat, ImageFormat};

/// Counter for image load requests to handle cancellation
static IMAGE_LOAD_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Preview mode - either terminal or image
#[derive(Debug, Clone)]
pub enum PreviewMode {
    /// No preview loaded
    None,
    /// Terminal/ANSI preview
    Terminal,
    /// Image preview with dimensions (width, height)
    Image(iced_image::Handle, u32, u32),
    /// Loading indicator
    Loading,
    /// Error message
    Error(String),
}

/// Messages for the preview view
#[derive(Clone)]
pub enum PreviewMessage {
    /// Sauce information received (sauce record, content size without SAUCE)
    SauceInfoReceived(Option<SauceRecord>, usize),
    /// Animation tick for terminal rendering
    AnimationTick(f32),
    /// Reset animation timer (to prevent jumps when starting auto-scroll)
    ResetAnimationTimer,
    /// Scroll viewport (direct, no animation - for mouse wheel)
    ScrollViewport(f32, f32),
    /// Scroll viewport with smooth animation (for PageUp/PageDown)
    ScrollViewportSmooth(f32, f32),
    /// Scroll viewport to absolute position (direct, no animation)
    ScrollViewportTo(f32, f32),
    /// Scroll viewport to absolute position with smooth animation (for Home/End)
    ScrollViewportToSmooth(f32, f32),
    /// Scroll vertical only to absolute Y position immediately
    ScrollViewportYToImmediate(f32),
    /// Scroll horizontal only to absolute X position immediately
    ScrollViewportXToImmediate(f32),
    /// Scrollbar hover state changed
    ScrollbarHovered(bool),
    /// Horizontal scrollbar hover state changed
    HScrollbarHovered(bool),
    /// Terminal view message
    TerminalMessage(icy_engine_gui::Message),
    /// Image loaded from background thread (with dimensions)
    ImageLoaded(u64, Result<(iced_image::Handle, u32, u32), String>),
    /// Image viewer message
    ImageViewerMessage(ImageViewerMessage),
    /// Zoom in (increase zoom level)
    ZoomIn,
    /// Zoom out (decrease zoom level)
    ZoomOut,
    /// Reset zoom to 100%
    ZoomReset,
    /// Zoom to fit
    ZoomFit,
}

/// Preview view for displaying ANSI files and images
pub struct PreviewView {
    /// Terminal widget for rendering
    pub terminal: Terminal,
    /// Command sender to view thread
    command_tx: mpsc::UnboundedSender<ViewCommand>,
    /// Event receiver from view thread (wrapped for polling)
    event_rx: Arc<Mutex<mpsc::UnboundedReceiver<ViewEvent>>>,
    /// Current file being previewed
    current_file: Option<PathBuf>,
    /// Whether file is currently loading
    is_loading: bool,
    /// Monitor settings for CRT effects
    pub monitor_settings: MonitorSettings,
    /// Current baud emulation setting
    baud_emulation: BaudEmulation,
    /// Current preview mode
    preview_mode: PreviewMode,
    /// Current image load request ID (for cancellation)
    current_image_load_id: u64,
    /// Current sauce information (if any)
    sauce_info: Option<SauceRecord>,
    /// Content size (file size without SAUCE record)
    content_size: Option<usize>,
    /// Current scroll mode (set by background thread)
    scroll_mode: ScrollMode,
    /// Auto-scroll enabled (setting - sent to background thread)
    auto_scroll_enabled: bool,
    /// Scroll speed for auto-scroll mode
    scroll_speed: ScrollSpeed,
    /// Image viewer (created when viewing images)
    image_viewer: Option<ImageViewer>,
}

impl PreviewView {
    pub fn new() -> Self {
        // Create a default screen (80x25 terminal)
        let screen: Box<dyn Screen> = Box::new(TextScreen::new(Size::new(80, 25)));
        let screen = Arc::new(Mutex::new(screen));

        // Create terminal widget in Viewer mode (for file viewing with scrolling)
        let terminal = Terminal::new(screen.clone());

        // Create view thread
        let (command_tx, event_rx) = create_view_thread(screen);

        Self {
            terminal,
            command_tx,
            event_rx: Arc::new(Mutex::new(event_rx)),
            current_file: None,
            is_loading: false,
            monitor_settings: MonitorSettings::default(),
            baud_emulation: BaudEmulation::Off,
            preview_mode: PreviewMode::None,
            current_image_load_id: 0,
            sauce_info: None,
            content_size: None,
            scroll_mode: ScrollMode::Off,
            auto_scroll_enabled: false,
            scroll_speed: ScrollSpeed::Medium,
            image_viewer: None,
        }
    }

    /// Check if a file is an image based on extension
    fn is_image_file(path: &PathBuf) -> bool {
        if let Some(ext) = path.extension() {
            let ext_str = ext.to_ascii_lowercase();
            if let Some(format) = FileFormat::from_extension(ext_str.to_str().unwrap_or("")) {
                return format.is_image();
            }
        }
        false
    }

    /// Check if a file is a Sixel image
    fn is_sixel_file(path: &PathBuf) -> bool {
        if let Some(ext) = path.extension() {
            let ext_str = ext.to_ascii_lowercase();
            if let Some(format) = FileFormat::from_extension(ext_str.to_str().unwrap_or("")) {
                return matches!(format, FileFormat::Image(ImageFormat::Sixel));
            }
        }
        false
    }

    /// Load data for preview
    pub fn load_data(&mut self, path: PathBuf, data: Vec<u8>) -> Task<PreviewMessage> {
        self.current_file = Some(path.clone());
        self.is_loading = true;

        // Reset scroll position to top when loading new file
        self.terminal.scroll_x_to(0.0);
        self.terminal.scroll_y_to(0.0);
        self.terminal.sync_scrollbar_with_viewport();

        // Reset scroll mode (background thread will set it)
        self.scroll_mode = ScrollMode::Off;

        // Clear image viewer when loading new file
        self.image_viewer = None;

        if Self::is_image_file(&path) {
            // Load image in background thread
            self.preview_mode = PreviewMode::Loading;

            // Cancel any previous image load by incrementing the counter
            let load_id = IMAGE_LOAD_COUNTER.fetch_add(1, Ordering::SeqCst) + 1;
            self.current_image_load_id = load_id;

            // Check if it's a Sixel file (needs special handling)
            let is_sixel = Self::is_sixel_file(&path);

            // Spawn background task to load image from data
            Task::perform(
                async move {
                    log::info!("Loading image for preview: {:?}", path);
                    // Load image in blocking thread pool
                    tokio::task::spawn_blocking(move || {
                        let data = icy_sauce::strip_sauce(&data, icy_sauce::StripMode::All);

                        if is_sixel {
                            // Use icy_sixel for Sixel files
                            match icy_sixel::sixel_decode(&data) {
                                Ok((rgba, width, height)) => {
                                    let w = width as u32;
                                    let h = height as u32;
                                    let handle = iced_image::Handle::from_rgba(w, h, rgba);
                                    (load_id, Ok((handle, w, h)))
                                }
                                Err(e) => (load_id, Err(format!("Failed to decode Sixel: {}", e))),
                            }
                        } else {
                            // Use image crate for other formats
                            match image::load_from_memory(&data) {
                                Ok(img) => {
                                    let rgba = img.to_rgba8();
                                    let (width, height) = rgba.dimensions();
                                    let handle = iced_image::Handle::from_rgba(width, height, rgba.into_raw());
                                    (load_id, Ok((handle, width, height)))
                                }
                                Err(e) => (load_id, Err(format!("Failed to load image: {}", e))),
                            }
                        }
                    })
                    .await
                    .unwrap_or_else(|e: tokio::task::JoinError| (load_id, Err(format!("Task error: {}", e))))
                },
                |(id, result)| PreviewMessage::ImageLoaded(id, result),
            )
        } else {
            // Load in terminal view thread (ANSI, etc.)
            self.preview_mode = PreviewMode::Terminal;

            // Invalidate any pending image load by incrementing the counter
            // This ensures old image loads don't overwrite the terminal view
            self.current_image_load_id = IMAGE_LOAD_COUNTER.fetch_add(1, Ordering::SeqCst) + 1;

            // Send auto_scroll_enabled to background thread so it can decide scroll mode
            let _ = self.command_tx.send(ViewCommand::LoadData(path, data, self.auto_scroll_enabled));
            Task::none()
        }
    }

    /// Set baud emulation rate
    pub fn set_baud_emulation(&mut self, baud: BaudEmulation) {
        self.baud_emulation = baud;
        // Note: scroll mode is now decided by background thread based on baud emulation
        let _ = self.command_tx.send(ViewCommand::SetBaudEmulation(baud));
    }

    /// Get current baud emulation setting
    pub fn get_baud_emulation(&self) -> BaudEmulation {
        self.baud_emulation
    }

    /// Get current sauce information (if any)
    pub fn get_sauce_info(&self) -> Option<&SauceRecord> {
        self.sauce_info.as_ref()
    }

    /// Get the screen for export (only available in terminal preview mode)
    pub fn get_screen(&self) -> Option<Arc<Mutex<Box<dyn Screen>>>> {
        if matches!(self.preview_mode, PreviewMode::Terminal) {
            Some(self.terminal.screen.clone())
        } else {
            None
        }
    }

    /// Get the buffer type from the current screen
    pub fn get_buffer_type(&self) -> Option<icy_engine::BufferType> {
        if matches!(self.preview_mode, PreviewMode::Terminal) {
            Some(self.terminal.screen.lock().buffer_type())
        } else {
            None
        }
    }

    /// Set monitor settings for CRT effects
    pub fn set_monitor_settings(&mut self, settings: MonitorSettings) {
        self.monitor_settings = settings;
    }

    /// Set the preview to error state
    pub fn set_error(&mut self, path: PathBuf, message: String) {
        self.current_file = Some(path);
        self.is_loading = false;
        self.preview_mode = PreviewMode::Error(message);
    }

    /// Zoom in by one step
    pub fn zoom_in(&mut self) {
        self.terminal.zoom_in();
    }

    /// Zoom in by integer step (for integer scaling mode)
    pub fn zoom_in_int(&mut self) {
        self.terminal.zoom_in_int();
    }

    /// Zoom out by one step
    pub fn zoom_out(&mut self) {
        self.terminal.zoom_out();
    }

    /// Zoom out by integer step (for integer scaling mode)
    pub fn zoom_out_int(&mut self) {
        self.terminal.zoom_out_int();
    }

    /// Reset zoom to 100% (1:1 pixel mapping)
    pub fn zoom_reset(&mut self) {
        self.terminal.zoom_reset();
    }

    /// Calculate and set auto-fit zoom
    pub fn zoom_auto_fit(&mut self, use_integer_scaling: bool) -> f32 {
        self.terminal.zoom_auto_fit(use_integer_scaling)
    }

    /// Get current zoom level
    pub fn get_zoom(&self) -> f32 {
        if let Some(ref viewer) = self.image_viewer {
            viewer.zoom()
        } else {
            self.terminal.get_zoom()
        }
    }

    /// Get image dimensions if viewing an image
    pub fn get_image_size(&self) -> Option<(u32, u32)> {
        if let Some(ref viewer) = self.image_viewer {
            let (w, h) = viewer.zoomed_size();
            Some((w as u32, h as u32))
        } else {
            None
        }
    }

    /// Get the current buffer size (width x height) from the terminal screen
    pub fn get_buffer_size(&self) -> Option<(i32, i32)> {
        if matches!(self.preview_mode, PreviewMode::Terminal) {
            let screen = self.terminal.screen.lock();
            return Some((screen.get_width(), screen.get_height()));
        }
        if let PreviewMode::Image(_, w, h) = self.preview_mode {
            return Some((w as i32, h as i32));
        }
        None
    }

    /// Get the content size (file size without SAUCE record)
    pub fn get_content_size(&self) -> Option<usize> {
        self.content_size
    }

    /// Get maximum scroll Y using computed visible height for accuracy
    /// This accounts for the actual rendered height rather than just viewport settings
    /// Returns max scroll in CONTENT coordinates (consistent with scroll_y)
    fn get_max_scroll_y(&self) -> f32 {
        // Viewport.max_scroll_y() now uses shader-computed visible_content_height if available
        self.terminal.viewport.read().max_scroll_y()
    }

    /// Cancel any ongoing loading operation
    /// Call this when selection is cleared or changed before a new file is selected
    pub fn cancel_loading(&mut self) {
        if self.is_loading {
            // Cancel the image load counter
            IMAGE_LOAD_COUNTER.fetch_add(1, Ordering::SeqCst);
            // Stop the view thread's current operation
            let _ = self.command_tx.send(ViewCommand::Stop);
            self.is_loading = false;
            self.current_file = None;
            self.preview_mode = PreviewMode::None;
            self.image_viewer = None;
        }
    }

    /// Check if animation is needed
    pub fn needs_animation(&self) -> bool {
        self.is_loading
            || self.terminal.needs_animation()
            || self.scroll_mode != ScrollMode::Off
            || self.image_viewer.as_ref().map_or(false, |v| v.needs_animation())
    }

    /// Start auto-scroll mode (animated scrolling to bottom)
    /// Returns a Task to reset the animation timer to prevent jumps
    pub fn start_auto_scroll(&mut self) -> Task<PreviewMessage> {
        // Only start if not currently loading and there's content to scroll
        if !self.is_loading {
            let max_scroll_y = self.get_max_scroll_y();
            if max_scroll_y > 0.0 {
                self.scroll_mode = ScrollMode::AutoScroll;
                // Reset timer to prevent jump on first frame
                return Task::done(PreviewMessage::ResetAnimationTimer);
            }
        }
        Task::none()
    }

    /// Stop auto-scroll mode
    pub fn stop_auto_scroll(&mut self) {
        self.scroll_mode = ScrollMode::Off;
    }

    /// Check if auto-scroll is active
    pub fn is_auto_scroll_active(&self) -> bool {
        self.scroll_mode == ScrollMode::AutoScroll
    }

    /// Set auto-scroll enabled (will auto-start on new file load)
    pub fn set_auto_scroll_enabled(&mut self, enabled: bool) {
        self.auto_scroll_enabled = enabled;
    }

    /// Check if auto-scroll is enabled (setting)
    pub fn is_auto_scroll_enabled(&self) -> bool {
        self.auto_scroll_enabled
    }

    /// Set scroll speed for auto-scroll mode
    pub fn set_scroll_speed(&mut self, speed: ScrollSpeed) {
        self.scroll_speed = speed;
    }

    /// Get current scroll speed
    pub fn get_scroll_speed(&self) -> ScrollSpeed {
        self.scroll_speed
    }

    /// Enable auto-scroll mode (for shuffle mode)
    pub fn enable_auto_scroll(&mut self) {
        self.auto_scroll_enabled = true;
        self.scroll_mode = ScrollMode::AutoScroll;
        // Reset scroll position to top
        self.terminal.scroll_y_to(0.0);
        self.terminal.sync_scrollbar_with_viewport();
    }

    /// Check if scroll has completed (reached bottom)
    pub fn is_scroll_complete(&self) -> bool {
        if self.is_loading {
            return false;
        }

        // Check if we're at the bottom
        let max_scroll_y = self.get_max_scroll_y();
        let current_y = self.terminal.viewport.read().scroll_y;

        // Consider scroll complete if we're at or very close to bottom
        // or if there's no scrollable content
        max_scroll_y <= 0.0 || current_y >= max_scroll_y - 1.0
    }

    /// Get visible height in pixels (for shuffle mode overlay positioning)
    pub fn get_visible_height(&self) -> f32 {
        let vp = self.terminal.viewport.read();
        let bounds_height = vp.bounds_height() as f32;
        if bounds_height > 0.0 { bounds_height } else { vp.visible_height }
    }

    /// Poll for events from view thread
    pub fn poll_events(&mut self) -> Vec<ViewEvent> {
        let mut events = Vec::new();
        let mut rx = self.event_rx.lock();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }
        events
    }

    /// Update with a message
    pub fn update(&mut self, message: PreviewMessage) -> Task<PreviewMessage> {
        match message {
            PreviewMessage::ImageLoaded(load_id, result) => {
                // Only accept if this is the current load request
                if load_id == self.current_image_load_id {
                    self.is_loading = false;
                    match result {
                        Ok((handle, width, height)) => {
                            // Create image viewer with the loaded image
                            self.image_viewer = Some(ImageViewer::new(handle.clone(), width, height));
                            self.preview_mode = PreviewMode::Image(handle, width, height);
                        }
                        Err(msg) => {
                            self.preview_mode = PreviewMode::Error(msg);
                        }
                    }
                }
                // Ignore outdated load results
                Task::none()
            }
            PreviewMessage::ImageViewerMessage(msg) => {
                if let Some(ref mut viewer) = self.image_viewer {
                    viewer.update(msg);
                }
                Task::none()
            }
            PreviewMessage::AnimationTick(delta_seconds) => {
                // Poll and process view thread events FIRST to update state
                let events = self.poll_events();
                let mut extra_tasks = Vec::new();

                for event in events {
                    match event {
                        ViewEvent::LoadingStarted(_path) => {
                            self.is_loading = true;
                        }
                        ViewEvent::LoadingCompleted => {
                            self.is_loading = false;
                            // Update terminal viewport after loading
                            self.terminal.update_viewport_size();
                        }
                        ViewEvent::SetScrollMode(mode) => {
                            self.scroll_mode = mode;
                            // Reset timer when starting auto-scroll to prevent jumps
                            if mode == ScrollMode::AutoScroll {
                                extra_tasks.push(Task::done(PreviewMessage::ResetAnimationTimer));
                            }
                        }
                        ViewEvent::SauceInfo(sauce_opt, content_size) => {
                            extra_tasks.push(Task::done(PreviewMessage::SauceInfoReceived(sauce_opt, content_size)));
                        }
                        ViewEvent::Error(msg) => {
                            log::error!("Preview error: {}", msg);
                            self.is_loading = false;
                            self.preview_mode = PreviewMode::Error(msg);
                        }
                    }
                }

                // Update terminal animations
                self.terminal.update_animations();

                // Update image viewer scrollbar animations if active
                if let Some(ref mut viewer) = self.image_viewer {
                    viewer.update_scrollbars(delta_seconds);
                }

                // Update viewport size to match current buffer size
                // This is important during baud emulation when buffer grows
                self.terminal.update_viewport_size();

                // Handle scroll mode (decided by background thread)
                match self.scroll_mode {
                    ScrollMode::ClampToBottom => {
                        // Clamp to bottom during baud emulation loading
                        // Use computed visible height for accurate max scroll calculation
                        let max_scroll_y = self.get_max_scroll_y();
                        self.terminal.scroll_y_to(max_scroll_y);
                        self.terminal.sync_scrollbar_with_viewport();
                    }
                    ScrollMode::AutoScroll => {
                        // Animated scrolling after loading completes
                        let scroll_speed = self.scroll_speed.get_speed();
                        let scroll_delta = scroll_speed * delta_seconds;
                        let current_y = self.terminal.viewport.read().scroll_y;
                        // Use computed visible height for accurate max scroll calculation
                        let max_scroll_y = self.get_max_scroll_y();
                        let new_y = (current_y + scroll_delta).min(max_scroll_y);

                        self.terminal.scroll_y_to(new_y);
                        self.terminal.sync_scrollbar_with_viewport();

                        // Stop auto-scroll when we reach the bottom
                        if new_y >= max_scroll_y {
                            self.scroll_mode = ScrollMode::Off;
                        }
                    }
                    ScrollMode::Off => {
                        // No automatic scrolling
                    }
                }

                // Update image viewer scrollbar animations
                if let Some(ref mut viewer) = self.image_viewer {
                    viewer.update_scrollbars(delta_seconds);
                }

                // Return any extra tasks
                if extra_tasks.is_empty() { Task::none() } else { Task::batch(extra_tasks) }
            }
            PreviewMessage::ScrollViewport(dx, dy) => {
                // User is scrolling manually, disable auto-scroll modes
                self.scroll_mode = ScrollMode::Off;
                self.terminal.scroll_x_by(dx);
                self.terminal.scroll_y_by(dy);
                self.terminal.sync_scrollbar_with_viewport();
                Task::none()
            }
            PreviewMessage::ScrollViewportSmooth(dx, dy) => {
                // User is scrolling with animation (PageUp/PageDown)
                self.scroll_mode = ScrollMode::Off;
                self.terminal.scroll_x_by_smooth(dx);
                self.terminal.scroll_y_by_smooth(dy);
                self.terminal.sync_scrollbar_with_viewport();
                Task::none()
            }
            PreviewMessage::ScrollViewportTo(x, y) => {
                // User is scrolling manually, disable auto-scroll modes
                self.scroll_mode = ScrollMode::Off;
                self.terminal.scroll_x_to(x);
                self.terminal.scroll_y_to(y);
                self.terminal.sync_scrollbar_with_viewport();
                Task::none()
            }
            PreviewMessage::ScrollViewportToSmooth(x, y) => {
                // User is scrolling to absolute position with animation (Home/End)
                self.scroll_mode = ScrollMode::Off;
                self.terminal.scroll_x_to_smooth(x);
                self.terminal.scroll_y_to_smooth(y);
                self.terminal.sync_scrollbar_with_viewport();
                Task::none()
            }
            PreviewMessage::ScrollViewportYToImmediate(y) => {
                // User is scrolling vertically via scrollbar
                self.scroll_mode = ScrollMode::Off;
                self.terminal.scroll_y_to(y);
                self.terminal.sync_scrollbar_with_viewport();
                Task::none()
            }
            PreviewMessage::ScrollViewportXToImmediate(x) => {
                // User is scrolling horizontally via scrollbar
                self.scroll_mode = ScrollMode::Off;
                self.terminal.scroll_x_to(x);
                self.terminal.sync_scrollbar_with_viewport();
                Task::none()
            }
            PreviewMessage::ScrollbarHovered(is_hovered) => {
                self.terminal.scrollbar.set_hovered(is_hovered);
                Task::none()
            }
            PreviewMessage::HScrollbarHovered(is_hovered) => {
                // Horizontal scrollbar uses separate hover state for animation
                self.terminal.scrollbar.set_hovered_x(is_hovered);
                Task::none()
            }
            PreviewMessage::TerminalMessage(msg) => {
                match msg {
                    icy_engine_gui::Message::ScrollViewport(dx, dy) => {
                        // User is scrolling manually, disable auto-scroll modes
                        self.scroll_mode = ScrollMode::Off;
                        self.terminal.scroll_x_by(dx);
                        self.terminal.scroll_y_by(dy);
                        self.terminal.sync_scrollbar_with_viewport();
                    }
                    icy_engine_gui::Message::ZoomWheel(delta) => {
                        // Handle Ctrl+wheel zoom
                        let use_integer = self.monitor_settings.use_integer_scaling;
                        let current_zoom = self.terminal.get_zoom();
                        let new_zoom = if delta > 0.0 {
                            icy_engine_gui::ScalingMode::zoom_in(current_zoom, use_integer)
                        } else {
                            icy_engine_gui::ScalingMode::zoom_out(current_zoom, use_integer)
                        };
                        self.terminal.set_zoom(new_zoom);
                        self.monitor_settings.scaling_mode = icy_engine_gui::ScalingMode::Manual(new_zoom);
                    }
                    icy_engine_gui::Message::StartSelection(sel) => {
                        // Selection coordinates already include scroll offset from map_mouse_to_cell
                        let mut screen = self.terminal.screen.lock();
                        let _ = screen.set_selection(sel);
                    }
                    icy_engine_gui::Message::UpdateSelection(pos) => {
                        // Position already includes scroll offset from map_mouse_to_cell
                        let mut screen = self.terminal.screen.lock();
                        if let Some(mut sel) = screen.get_selection().clone() {
                            if !sel.locked {
                                sel.lead = pos;
                                let _ = screen.set_selection(sel);
                            }
                        }
                    }
                    icy_engine_gui::Message::EndSelection => {
                        let mut screen = self.terminal.screen.lock();
                        if let Some(mut sel) = screen.get_selection().clone() {
                            sel.locked = true;
                            let _ = screen.set_selection(sel);
                        }
                    }
                    icy_engine_gui::Message::ClearSelection => {
                        let mut screen = self.terminal.screen.lock();
                        let _ = screen.clear_selection();
                    }
                    _ => {}
                }
                Task::none()
            }
            PreviewMessage::ZoomIn => {
                if let Some(ref mut viewer) = self.image_viewer {
                    viewer.zoom_in();
                } else {
                    self.terminal.zoom_in();
                }
                Task::none()
            }
            PreviewMessage::ZoomOut => {
                if let Some(ref mut viewer) = self.image_viewer {
                    viewer.zoom_out();
                } else {
                    self.terminal.zoom_out();
                }
                Task::none()
            }
            PreviewMessage::ZoomReset => {
                if let Some(ref mut viewer) = self.image_viewer {
                    viewer.zoom_100();
                } else {
                    self.terminal.zoom_reset();
                }
                Task::none()
            }
            PreviewMessage::ZoomFit => {
                if let Some(ref mut viewer) = self.image_viewer {
                    viewer.zoom_fit();
                } else {
                    self.terminal.zoom_auto_fit(self.monitor_settings.use_integer_scaling);
                }
                Task::none()
            }
            PreviewMessage::SauceInfoReceived(sauce_opt, content_size) => {
                // Store sauce info and content size for status bar display
                self.sauce_info = sauce_opt;
                self.content_size = Some(content_size);
                Task::none()
            }
            PreviewMessage::ResetAnimationTimer => {
                // This message is handled by MainWindow to reset last_tick
                // Just return none here, parent will handle it
                Task::none()
            }
        }
    }

    /// Create the view with optional monitor settings override
    /// If settings are provided, they override the internal monitor_settings
    pub fn view_with_settings(&self, settings: Option<&MonitorSettings>) -> Element<'_, PreviewMessage> {
        let monitor_settings = settings.cloned().unwrap_or_else(|| self.monitor_settings.clone());

        match &self.preview_mode {
            PreviewMode::None => container(iced::widget::text(fl!(crate::LANGUAGE_LOADER, "preview-no-file-selected")))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .style(|theme: &iced::Theme| container::Style {
                    background: Some(iced::Background::Color(theme::main_area_background(theme))),
                    ..Default::default()
                })
                .into(),
            PreviewMode::Loading => container(iced::widget::text(fl!(crate::LANGUAGE_LOADER, "preview-loading")))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .style(|theme: &iced::Theme| container::Style {
                    background: Some(iced::Background::Color(theme::main_area_background(theme))),
                    ..Default::default()
                })
                .into(),
            PreviewMode::Error(msg) => {
                // Create a nice error display with icon and styled text
                let error_icon = text("⚠").size(48).color(iced::Color::from_rgb(0.9, 0.3, 0.3));

                let error_title = text(fl!(crate::LANGUAGE_LOADER, "preview-error-title"))
                    .size(20)
                    .color(iced::Color::from_rgb(0.9, 0.3, 0.3));

                let error_message = text(msg.clone()).size(14).color(iced::Color::from_rgb(0.7, 0.7, 0.7));

                let file_hint = if let Some(ref path) = self.current_file {
                    let filename = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    text(filename).size(12).color(iced::Color::from_rgb(0.5, 0.5, 0.5))
                } else {
                    text("").size(12)
                };

                let error_content = column![
                    error_icon,
                    Space::new().height(16),
                    error_title,
                    Space::new().height(8),
                    error_message,
                    Space::new().height(4),
                    file_hint,
                ]
                .align_x(Alignment::Center)
                .width(Length::Shrink);

                container(error_content)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x(Length::Fill)
                    .center_y(Length::Fill)
                    .style(|theme: &iced::Theme| container::Style {
                        background: Some(iced::Background::Color(theme::main_area_background(theme))),
                        ..Default::default()
                    })
                    .into()
            }
            PreviewMode::Image(_handle, _width, _height) => {
                // Use the ImageViewer if available
                if let Some(ref viewer) = self.image_viewer {
                    container(viewer.view(PreviewMessage::ImageViewerMessage))
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .style(|theme: &iced::Theme| container::Style {
                            background: Some(iced::Background::Color(theme::main_area_background(theme))),
                            ..Default::default()
                        })
                        .into()
                } else {
                    // Fallback: simple centered image (shouldn't happen normally)
                    let img = iced_image::Image::new(_handle.clone()).content_fit(iced::ContentFit::Contain);
                    container(img)
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .center_x(Length::Fill)
                        .center_y(Length::Fill)
                        .style(|theme: &iced::Theme| container::Style {
                            background: Some(iced::Background::Color(theme::main_area_background(theme))),
                            ..Default::default()
                        })
                        .into()
                }
            }
            PreviewMode::Terminal => {
                let terminal_view = TerminalView::show_with_effects(&self.terminal, monitor_settings).map(PreviewMessage::TerminalMessage);

                // Get scrollbar info using shared logic from icy_engine_gui
                let scrollbar_info = self.terminal.scrollbar_info();

                if scrollbar_info.needs_any_scrollbar() {
                    let mut layers: Vec<Element<'_, PreviewMessage>> = vec![terminal_view];

                    // Add vertical scrollbar if needed
                    if scrollbar_info.needs_vscrollbar {
                        let vscrollbar_view = ScrollbarOverlay::new(
                            scrollbar_info.visibility_v,
                            scrollbar_info.scroll_position_v,
                            scrollbar_info.height_ratio,
                            scrollbar_info.max_scroll_y,
                            self.terminal.scrollbar_hover_state.clone(),
                            |_x, y| PreviewMessage::ScrollViewportYToImmediate(y),
                            |is_hovered| PreviewMessage::ScrollbarHovered(is_hovered),
                        )
                        .view();

                        let vscrollbar_container: container::Container<'_, PreviewMessage> =
                            container(vscrollbar_view).width(Length::Fill).height(Length::Fill).align_x(Alignment::End);
                        layers.push(vscrollbar_container.into());
                    }

                    // Add horizontal scrollbar if needed
                    if scrollbar_info.needs_hscrollbar {
                        let hscrollbar_view = HorizontalScrollbarOverlay::new(
                            scrollbar_info.visibility_h,
                            scrollbar_info.scroll_position_h,
                            scrollbar_info.width_ratio,
                            scrollbar_info.max_scroll_x,
                            self.terminal.hscrollbar_hover_state.clone(),
                            |x, _y| PreviewMessage::ScrollViewportXToImmediate(x),
                            |is_hovered| PreviewMessage::HScrollbarHovered(is_hovered),
                        )
                        .view();

                        let hscrollbar_container: container::Container<'_, PreviewMessage> =
                            container(hscrollbar_view).width(Length::Fill).height(Length::Fill).align_y(Alignment::End);
                        layers.push(hscrollbar_container.into());
                    }

                    container(stack(layers))
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .style(|theme: &iced::Theme| container::Style {
                            background: Some(iced::Background::Color(theme::main_area_background(theme))),
                            ..Default::default()
                        })
                        .into()
                } else {
                    container(terminal_view)
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .style(|theme: &iced::Theme| container::Style {
                            background: Some(iced::Background::Color(theme::main_area_background(theme))),
                            ..Default::default()
                        })
                        .into()
                }
            }
        }
    }
}

impl Default for PreviewView {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for PreviewView {
    fn drop(&mut self) {
        // Signal the view thread to shutdown
        let _ = self.command_tx.send(ViewCommand::Shutdown);
    }
}
