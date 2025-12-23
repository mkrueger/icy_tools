use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use i18n_embed_fl::fl;
use iced::Event;
use iced::{
    widget::{column, container, image as iced_image, stack, text, Space},
    Alignment, Element, Length, Task,
};
use icy_engine::{Screen, Size, TextScreen};
use icy_engine_gui::{command_handler, HorizontalScrollbarOverlay, MonitorSettings, ScrollbarOverlay, Terminal, TerminalView};
use icy_parser_core::BaudEmulation;
use icy_sauce::SauceRecord;
use parking_lot::Mutex;
use tokio::sync::mpsc;

use super::content_view::ContentView;
use super::drag_scroll::DragScrollState;
use super::image_content_view::ImageContentView;
use super::image_viewer::{ImageViewer, ImageViewerMessage};
use super::terminal_content_view::TerminalContentView;
use super::view_thread::{create_view_thread, ScrollMode, ViewCommand, ViewEvent};
use crate::commands::{cmd, create_icy_view_commands};
use crate::ui::theme;
use crate::ScrollSpeed;
use icy_engine::formats::{FileFormat, ImageFormat};

/// Counter for image load requests to handle cancellation
static IMAGE_LOAD_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Check if a file is an image based on extension
pub fn is_image_file(path: &PathBuf) -> bool {
    if let Some(ext) = path.extension() {
        let ext_str = ext.to_ascii_lowercase();
        if let Some(format) = FileFormat::from_extension(ext_str.to_str().unwrap_or("")) {
            return format.is_image();
        }
    }
    false
}

/// Check if a file is a Sixel image
pub fn is_sixel_file(path: &PathBuf) -> bool {
    if let Some(ext) = path.extension() {
        let ext_str = ext.to_ascii_lowercase();
        if let Some(format) = FileFormat::from_extension(ext_str.to_str().unwrap_or("")) {
            return matches!(format, FileFormat::Image(ImageFormat::Sixel));
        }
    }
    false
}

// Command handler for PreviewView
command_handler!(PreviewCommands, create_icy_view_commands(), => PreviewMessage {
    cmd::VIEW_ZOOM_IN => PreviewMessage::Zoom(icy_engine_gui::ZoomMessage::In),
    cmd::VIEW_ZOOM_OUT => PreviewMessage::Zoom(icy_engine_gui::ZoomMessage::Out),
    cmd::VIEW_ZOOM_RESET => PreviewMessage::Zoom(icy_engine_gui::ZoomMessage::Reset),
    cmd::VIEW_ZOOM_FIT => PreviewMessage::Zoom(icy_engine_gui::ZoomMessage::AutoFit),
});

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
    /// Scroll viewport to absolute position with smooth animation (for Home/End)
    ScrollViewportToSmooth(f32, f32),
    /// Terminal view message
    TerminalMessage(icy_engine_gui::TerminalMessage),
    /// Image loaded from background thread (with dimensions)
    ImageLoaded(u64, Result<(iced_image::Handle, u32, u32), String>),
    /// Image viewer message
    ImageViewerMessage(ImageViewerMessage),
    /// Unified zoom message
    Zoom(icy_engine_gui::ZoomMessage),
}

/// Preview view for displaying ANSI files and images
pub struct PreviewView {
    /// Command handler for preview shortcuts
    commands: PreviewCommands,
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
    /// Monitor settings for CRT effects (cached as Arc for efficient rendering)
    pub monitor_settings: Arc<MonitorSettings>,
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

    /// Drag scrolling state (shared between terminal and image viewer)
    drag_scroll: DragScrollState,
}

impl PreviewView {
    pub fn new() -> Self {
        // Create a default screen (80x25 terminal)
        let screen: Box<dyn Screen> = Box::new(TextScreen::new(Size::new(80, 25)));
        let screen = Arc::new(Mutex::new(screen));

        // Create terminal widget in Viewer mode (for file viewing with scrolling)
        let mut terminal = Terminal::new(screen.clone());
        terminal.set_fit_terminal_height_to_bounds(true);

        // Create view thread
        let (command_tx, event_rx) = create_view_thread(screen);

        Self {
            commands: PreviewCommands::new(),
            terminal,
            command_tx,
            event_rx: Arc::new(Mutex::new(event_rx)),
            current_file: None,
            is_loading: false,
            monitor_settings: Arc::new(MonitorSettings::default()),
            baud_emulation: BaudEmulation::Off,
            preview_mode: PreviewMode::None,
            current_image_load_id: 0,
            sauce_info: None,
            content_size: None,
            scroll_mode: ScrollMode::Off,
            auto_scroll_enabled: false,
            scroll_speed: ScrollSpeed::Medium,
            image_viewer: None,
            // Drag scrolling
            drag_scroll: DragScrollState::new(),
        }
    }

    /// Handle an event and return the corresponding message if it matches a command
    pub fn handle_event(&self, event: &Event) -> Option<PreviewMessage> {
        self.commands.handle(event)
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

        // Reset drag/inertia state
        self.drag_scroll = DragScrollState::new();

        // Clear image viewer when loading new file
        self.image_viewer = None;

        if is_image_file(&path) {
            // Load image in background thread
            self.preview_mode = PreviewMode::Loading;

            // Cancel any previous image load by incrementing the counter
            let load_id = IMAGE_LOAD_COUNTER.fetch_add(1, Ordering::SeqCst) + 1;
            self.current_image_load_id = load_id;

            // Check if it's a Sixel file (needs special handling)
            let is_sixel = is_sixel_file(&path);

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
                                Ok(image) => {
                                    let w = image.width as u32;
                                    let h = image.height as u32;
                                    let handle = iced_image::Handle::from_rgba(w, h, image.pixels);
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

    /// Load a pre-decoded image directly (used for preloaded shuffle items)
    /// This skips the background decoding step since the image is already decoded
    /// Takes raw RGBA pixel data and creates the handle immediately
    pub fn load_decoded_image(&mut self, path: PathBuf, rgba: Vec<u8>, width: u32, height: u32) -> Task<PreviewMessage> {
        self.current_file = Some(path);
        self.is_loading = false;

        // Reset scroll position to top when loading new file
        self.terminal.scroll_x_to(0.0);
        self.terminal.scroll_y_to(0.0);
        self.terminal.sync_scrollbar_with_viewport();

        // Clear image viewer when loading new file
        self.image_viewer = None;

        // Increment load counter to invalidate any pending background loads
        self.current_image_load_id = IMAGE_LOAD_COUNTER.fetch_add(1, Ordering::SeqCst) + 1;

        // Create handle from rgba data
        let handle = iced_image::Handle::from_rgba(width, height, rgba);

        // Create image viewer with the pre-decoded image
        let mut viewer = ImageViewer::new(handle.clone(), width, height);

        // If auto-scroll is enabled (e.g., shuffle mode), reset scroll to top
        if self.auto_scroll_enabled {
            viewer.scroll_to(0.0, 0.0);
            self.scroll_mode = ScrollMode::AutoScroll;
        } else {
            self.scroll_mode = ScrollMode::Off;
        }

        self.image_viewer = Some(viewer);
        self.preview_mode = PreviewMode::Image(handle, width, height);

        log::debug!("Loaded pre-decoded image {}x{}", width, height);
        Task::none()
    }

    /// Set baud emulation rate
    pub fn set_baud_emulation(&mut self, baud: BaudEmulation) {
        self.baud_emulation = baud;
        // Note: scroll mode is now decided by background thread based on baud emulation
        let _ = self.command_tx.send(ViewCommand::SetBaudEmulation(baud));
    }

    /// Get current baud emulation setting
    pub fn baud_emulation(&self) -> BaudEmulation {
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

    /// Set the preview to error state
    pub fn set_error(&mut self, path: PathBuf, message: String) {
        self.current_file = Some(path);
        self.is_loading = false;
        self.preview_mode = PreviewMode::Error(message);
    }

    /// Get the current buffer size (width x height) from the terminal screen
    pub fn get_buffer_size(&self) -> Option<(i32, i32)> {
        if matches!(self.preview_mode, PreviewMode::Terminal) {
            let screen = self.terminal.screen.lock();
            return Some((screen.width(), screen.height()));
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

    /// Execute a closure with the active content view (image or terminal)
    /// This provides unified access to scroll operations
    fn with_content_view<R>(&mut self, f: impl FnOnce(&mut dyn ContentView) -> R) -> R {
        if let Some(ref mut viewer) = self.image_viewer {
            let mut content_view = ImageContentView::new(viewer);
            f(&mut content_view)
        } else {
            let mut content_view = TerminalContentView::new(&mut self.terminal);
            f(&mut content_view)
        }
    }

    /// Get maximum scroll Y using the active content view
    fn get_max_scroll_y(&self) -> f32 {
        if let Some(ref viewer) = self.image_viewer {
            viewer.viewport.max_scroll_y()
        } else {
            self.terminal.viewport.read().max_scroll_y()
        }
    }

    /// Get current scroll Y position using the active content view
    fn get_current_scroll_y(&self) -> f32 {
        if let Some(ref viewer) = self.image_viewer {
            viewer.viewport.scroll_y
        } else {
            self.terminal.viewport.read().scroll_y
        }
    }

    /// Check if scroll has reached the bottom
    fn is_at_bottom(&self) -> bool {
        let max_y = self.get_max_scroll_y();
        let current_y = self.get_current_scroll_y();
        max_y <= 0.0 || current_y >= max_y - 1.0
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
            || self.scroll_mode != ScrollMode::Off
            || self.image_viewer.as_ref().map_or(false, |v| v.needs_animation())
            || self.drag_scroll.needs_animation()
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
        // Reset scroll position to top using unified API
        self.with_content_view(|cv| {
            cv.scroll_to(0.0, 0.0);
            cv.sync_scrollbar();
        });
    }

    /// Check if scroll has completed (reached bottom)
    pub fn is_scroll_complete(&self) -> bool {
        if self.is_loading {
            return false;
        }
        self.is_at_bottom()
    }

    /// Get visible height in pixels (for shuffle mode overlay positioning)
    pub fn get_visible_height(&self) -> f32 {
        if let Some(ref viewer) = self.image_viewer {
            viewer.viewport.visible_height
        } else {
            self.terminal.viewport.read().visible_height
        }
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
                            let mut viewer = ImageViewer::new(handle.clone(), width, height);

                            // If auto-scroll is enabled (e.g., shuffle mode), reset scroll to top
                            if self.auto_scroll_enabled {
                                viewer.scroll_to(0.0, 0.0);
                                self.scroll_mode = ScrollMode::AutoScroll;
                            }

                            self.image_viewer = Some(viewer);
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
                use super::image_viewer::ImageViewerMessage;
                use iced::mouse;

                // Handle drag messages with shared drag_scroll state
                match &msg {
                    ImageViewerMessage::Press(pos) => {
                        let viewport_pos = self.with_content_view(|cv| (cv.scroll_x(), cv.scroll_y()));
                        self.drag_scroll.start_drag(*pos, viewport_pos);
                        self.scroll_mode = ScrollMode::Off;
                        // Set grabbing cursor
                        if let Some(ref viewer) = self.image_viewer {
                            *viewer.cursor_icon.write() = Some(mouse::Interaction::Grabbing);
                        }
                    }
                    ImageViewerMessage::Release => {
                        self.drag_scroll.end_drag();
                        // Reset cursor
                        if let Some(ref viewer) = self.image_viewer {
                            *viewer.cursor_icon.write() = None;
                        }
                    }
                    ImageViewerMessage::Move(Some(pos)) => {
                        // If dragging, process as drag event
                        if self.drag_scroll.is_dragging {
                            // ImageViewer uses zoom=1.0 since content_size already includes zoom
                            if let Some((x, y)) = self.drag_scroll.process_drag(*pos, 1.0) {
                                self.with_content_view(|cv| {
                                    cv.scroll_to(x, y);
                                    cv.sync_scrollbar();
                                });
                            }
                        }
                        // Cursor is handled by the widget based on cursor_icon state
                    }
                    ImageViewerMessage::Move(None) => {
                        // Mouse left bounds - nothing to do
                    }
                    ImageViewerMessage::Scroll(_, _) => {
                        // Stop inertia when user scrolls manually
                        self.scroll_mode = ScrollMode::Off;
                        self.drag_scroll.stop();
                    }
                    _ => {}
                }

                // Forward remaining messages to viewer
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
                    }
                }

                // Update animations using unified API
                self.with_content_view(|cv| {
                    cv.update_animations(delta_seconds);
                });

                // Update viewport size to match current buffer size
                // This is important during baud emulation when buffer grows
                self.terminal.update_viewport_size();

                // Handle scroll mode using unified content view API
                match self.scroll_mode {
                    ScrollMode::ClampToBottom => {
                        // Clamp to bottom during baud emulation loading
                        let max_scroll_y = self.get_max_scroll_y();
                        self.with_content_view(|cv| {
                            cv.scroll_y_to(max_scroll_y);
                            cv.sync_scrollbar();
                        });
                    }
                    ScrollMode::AutoScroll => {
                        // Animated scrolling after loading completes
                        let scroll_speed = self.scroll_speed.get_speed();
                        let scroll_delta = scroll_speed * delta_seconds;
                        let max_scroll_y = self.get_max_scroll_y();
                        let current_y = self.get_current_scroll_y();
                        let new_y = (current_y + scroll_delta).min(max_scroll_y);

                        self.with_content_view(|cv| {
                            cv.scroll_y_to(new_y);
                            cv.sync_scrollbar();
                        });

                        // Stop auto-scroll when we reach the bottom
                        if new_y >= max_scroll_y {
                            self.scroll_mode = ScrollMode::Off;
                        }
                    }
                    ScrollMode::Off => {
                        // No automatic scrolling - but handle inertia
                    }
                }

                // Handle inertia scrolling (after drag release)
                if let Some((dx, dy)) = self.drag_scroll.update_inertia(delta_seconds) {
                    self.with_content_view(|cv| {
                        cv.scroll_by(dx, dy);
                        cv.sync_scrollbar();
                    });
                }

                // Return any extra tasks
                if extra_tasks.is_empty() {
                    Task::none()
                } else {
                    Task::batch(extra_tasks)
                }
            }
            PreviewMessage::ScrollViewport(dx, dy) => {
                // User is scrolling manually, disable auto-scroll modes
                self.scroll_mode = ScrollMode::Off;
                self.with_content_view(|cv| {
                    cv.scroll_by(dx, dy);
                    cv.sync_scrollbar();
                });
                Task::none()
            }
            PreviewMessage::ScrollViewportSmooth(dx, dy) => {
                // User is scrolling with animation (PageUp/PageDown)
                self.scroll_mode = ScrollMode::Off;
                self.with_content_view(|cv| {
                    cv.scroll_by_smooth(dx, dy);
                    cv.sync_scrollbar();
                });
                Task::none()
            }
            PreviewMessage::ScrollViewportToSmooth(x, y) => {
                // User is scrolling to absolute position with animation (Home/End)
                self.scroll_mode = ScrollMode::Off;
                self.with_content_view(|cv| {
                    cv.scroll_to_smooth(x, y);
                    cv.sync_scrollbar();
                });
                Task::none()
            }
            PreviewMessage::TerminalMessage(msg) => {
                use iced::mouse;

                match msg {
                    icy_engine_gui::TerminalMessage::Press(evt) => {
                        // Only start drag if clicking on the actual content (not border)
                        if evt.text_position.is_some() {
                            let viewport_pos = self.with_content_view(|cv| (cv.scroll_x(), cv.scroll_y()));
                            self.drag_scroll.start_drag(evt.pixel_position, viewport_pos);
                            // Disable auto-scroll when user starts dragging
                            self.scroll_mode = ScrollMode::Off;
                            // Set grabbing cursor
                            *self.terminal.cursor_icon.write() = Some(mouse::Interaction::Grabbing);
                        }
                    }
                    icy_engine_gui::TerminalMessage::Release(_) => {
                        // End drag scrolling, start inertia if we have velocity
                        self.drag_scroll.end_drag();
                        // Reset cursor to default (will be set to Grab on next Move if over content)
                        *self.terminal.cursor_icon.write() = None;
                    }
                    icy_engine_gui::TerminalMessage::Move(evt) => {
                        // Show grab cursor only when over the actual terminal content area
                        if !self.drag_scroll.is_dragging {
                            if evt.text_position.is_some() {
                                *self.terminal.cursor_icon.write() = Some(mouse::Interaction::Grab);
                            } else {
                                *self.terminal.cursor_icon.write() = None;
                            }
                        }
                    }
                    icy_engine_gui::TerminalMessage::Drag(evt) => {
                        let zoom = self.terminal.get_zoom();
                        if let Some((x, y)) = self.drag_scroll.process_drag(evt.pixel_position, zoom) {
                            self.with_content_view(|cv| {
                                cv.scroll_to(x, y);
                                cv.sync_scrollbar();
                            });
                        }
                    }
                    icy_engine_gui::TerminalMessage::Scroll(delta) => {
                        // User is scrolling manually, disable auto-scroll modes
                        self.scroll_mode = ScrollMode::Off;
                        self.drag_scroll.stop();
                        let (dx, dy) = match delta {
                            icy_engine_gui::WheelDelta::Lines { x, y } => (x * 10.0, y * 20.0),
                            icy_engine_gui::WheelDelta::Pixels { x, y } => (x, y),
                        };
                        self.with_content_view(|cv| {
                            cv.scroll_by(-dx, -dy);
                            cv.sync_scrollbar();
                        });
                    }
                    icy_engine_gui::TerminalMessage::Zoom(zoom_msg) => {
                        // Handle zoom via unified ZoomMessage
                        // Create a new Arc with updated scaling_mode
                        let current_zoom = self.terminal.get_zoom();
                        let use_integer = self.monitor_settings.use_integer_scaling;
                        let mut new_settings = (*self.monitor_settings).clone();
                        new_settings.scaling_mode = new_settings.scaling_mode.apply_zoom(zoom_msg, current_zoom, use_integer);
                        if let icy_engine_gui::ScalingMode::Manual(z) = new_settings.scaling_mode {
                            self.terminal.set_zoom(z);
                        }
                        self.monitor_settings = Arc::new(new_settings);
                    }
                }
                Task::none()
            }
            PreviewMessage::Zoom(zoom_msg) => {
                // Unified zoom handling for both terminal and image viewer
                // Create a new Arc with updated scaling_mode
                let use_integer = self.monitor_settings.use_integer_scaling;
                let mut new_settings = (*self.monitor_settings).clone();
                if let Some(ref mut viewer) = self.image_viewer {
                    // Apply zoom to image viewer
                    let current_zoom = viewer.zoom();
                    let new_scaling = new_settings.scaling_mode.apply_zoom(zoom_msg, current_zoom, use_integer);
                    match new_scaling {
                        icy_engine_gui::ScalingMode::Auto => viewer.zoom_fit(),
                        icy_engine_gui::ScalingMode::Manual(z) => viewer.set_zoom(z),
                    }
                    new_settings.scaling_mode = new_scaling;
                } else {
                    // Apply zoom to terminal
                    let current_zoom = self.terminal.get_zoom();
                    new_settings.scaling_mode = new_settings.scaling_mode.apply_zoom(zoom_msg, current_zoom, use_integer);
                    match new_settings.scaling_mode {
                        icy_engine_gui::ScalingMode::Auto => {
                            self.terminal.zoom_auto_fit(use_integer);
                        }
                        icy_engine_gui::ScalingMode::Manual(z) => {
                            self.terminal.set_zoom(z);
                        }
                    }
                }
                self.monitor_settings = Arc::new(new_settings);
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
    pub fn view_with_settings(&self, settings: Option<Arc<MonitorSettings>>) -> Element<'_, PreviewMessage> {
        let monitor_settings = settings.unwrap_or_else(|| self.monitor_settings.clone());

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
                let terminal_view = TerminalView::show_with_effects(&self.terminal, monitor_settings, None).map(PreviewMessage::TerminalMessage);

                // Get scrollbar info using shared logic from icy_engine_gui
                let scrollbar_info = self.terminal.scrollbar_info();

                if scrollbar_info.needs_any_scrollbar() {
                    let mut layers: Vec<Element<'_, PreviewMessage>> = vec![terminal_view];

                    // Add vertical scrollbar if needed - uses ViewportAccess to mutate viewport directly
                    if scrollbar_info.needs_vscrollbar {
                        let vscrollbar_view: Element<'_, ()> = ScrollbarOverlay::new(&self.terminal.viewport).view();
                        let vscrollbar_mapped: Element<'_, PreviewMessage> = vscrollbar_view.map(|_| unreachable!());
                        let vscrollbar_container: container::Container<'_, PreviewMessage> =
                            container(vscrollbar_mapped).width(Length::Fill).height(Length::Fill).align_x(Alignment::End);
                        layers.push(vscrollbar_container.into());
                    }

                    // Add horizontal scrollbar if needed - uses ViewportAccess to mutate viewport directly
                    if scrollbar_info.needs_hscrollbar {
                        let hscrollbar_view: Element<'_, ()> = HorizontalScrollbarOverlay::new(&self.terminal.viewport).view();
                        let hscrollbar_mapped: Element<'_, PreviewMessage> = hscrollbar_view.map(|_| unreachable!());
                        let hscrollbar_container: container::Container<'_, PreviewMessage> =
                            container(hscrollbar_mapped).width(Length::Fill).height(Length::Fill).align_y(Alignment::End);
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
