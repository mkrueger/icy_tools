use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use i18n_embed_fl::fl;
use iced::{
    Alignment, Element, Length, Task,
    widget::{container, image as iced_image, stack},
};
use icy_engine::{Position, Screen, Selection, Size, TextScreen};
use icy_engine_gui::{HorizontalScrollbarOverlay, MonitorSettings, ScrollbarOverlay, Terminal, TerminalView};
use icy_parser_core::BaudEmulation;
use icy_sauce::SauceRecord;
use parking_lot::Mutex;
use tokio::sync::mpsc;

use super::view_thread::{ScrollMode, ViewCommand, ViewEvent, create_view_thread};
use crate::EXT_IMAGE_LIST;
use crate::ui::options::ScrollSpeed;
use crate::ui::theme;

/// Counter for image load requests to handle cancellation
static IMAGE_LOAD_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Preview mode - either terminal or image
#[derive(Debug, Clone)]
pub enum PreviewMode {
    /// No preview loaded
    None,
    /// Terminal/ANSI preview
    Terminal,
    /// Image preview with handle
    Image(iced_image::Handle),
    /// Loading indicator
    Loading,
    /// Error message
    Error(String),
}

/// Messages for the preview view
#[derive(Clone)]
pub enum PreviewMessage {
    /// Load data for preview (path is for display/type detection, data is the content)
    LoadData(PathBuf, Vec<u8>),
    /// View thread event received
    ViewEvent(ViewEvent),
    /// Sauce information received (sauce record, content size without SAUCE)
    SauceInfoReceived(Option<SauceRecord>, usize),
    /// Animation tick for terminal rendering
    AnimationTick(f32),
    /// Reset animation timer (to prevent jumps when starting auto-scroll)
    ResetAnimationTimer,
    /// Scroll viewport
    ScrollViewport(f32, f32),
    /// Scroll viewport to absolute position
    ScrollViewportTo(f32, f32),
    /// Scroll viewport to absolute position immediately (no animation)
    ScrollViewportToImmediate(f32, f32),
    /// Scroll horizontal only to absolute X position immediately
    ScrollViewportXToImmediate(f32),
    /// Scrollbar hover state changed
    ScrollbarHovered(bool),
    /// Horizontal scrollbar hover state changed
    HScrollbarHovered(bool),
    /// Set baud emulation
    SetBaudEmulation(BaudEmulation),
    /// Terminal view message
    TerminalMessage(icy_engine_gui::Message),
    /// Image loaded from background thread
    ImageLoaded(u64, Result<iced_image::Handle, String>),
    /// Start selection
    StartSelection(Selection),
    /// Update selection position
    UpdateSelection(Position),
    /// End selection (lock it)
    EndSelection,
    /// Clear selection
    ClearSelection,
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
    monitor_settings: MonitorSettings,
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
        }
    }

    /// Check if a file is an image based on extension
    fn is_image_file(path: &PathBuf) -> bool {
        if let Some(ext) = path.extension() {
            let ext_lower = ext.to_ascii_lowercase();
            let ext_str = ext_lower.to_str().unwrap_or("");
            return EXT_IMAGE_LIST.contains(&ext_str);
        }
        false
    }

    /// Load data for preview
    pub fn load_data(&mut self, path: PathBuf, data: Vec<u8>) -> Task<PreviewMessage> {
        self.current_file = Some(path.clone());
        self.is_loading = true;

        // Reset scroll position to top when loading new file
        self.terminal.scroll_to_immediate(0.0, 0.0);
        self.terminal.sync_scrollbar_with_viewport();

        // Reset scroll mode (background thread will set it)
        self.scroll_mode = ScrollMode::Off;

        if Self::is_image_file(&path) {
            // Load image in background thread
            self.preview_mode = PreviewMode::Loading;

            // Cancel any previous image load by incrementing the counter
            let load_id = IMAGE_LOAD_COUNTER.fetch_add(1, Ordering::SeqCst) + 1;
            self.current_image_load_id = load_id;

            // Spawn background task to load image from data
            Task::perform(
                async move {
                    // Load image in blocking thread pool
                    tokio::task::spawn_blocking(move || match image::load_from_memory(&data) {
                        Ok(img) => {
                            let rgba = img.to_rgba8();
                            let (width, height) = rgba.dimensions();
                            let handle = iced_image::Handle::from_rgba(width, height, rgba.into_raw());
                            (load_id, Ok(handle))
                        }
                        Err(e) => (load_id, Err(format!("Failed to load image: {}", e))),
                    })
                    .await
                    .unwrap_or_else(|e| (load_id, Err(format!("Task error: {}", e))))
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

    /// Get the current buffer size (width x height) from the terminal screen
    pub fn get_buffer_size(&self) -> Option<(i32, i32)> {
        if matches!(self.preview_mode, PreviewMode::Terminal) {
            let screen = self.terminal.screen.lock();
            return Some((screen.get_width(), screen.get_height()));
        }
        None
    }

    /// Get the content size (file size without SAUCE record)
    pub fn get_content_size(&self) -> Option<usize> {
        self.content_size
    }

    /// Get maximum scroll Y using computed visible height for accuracy
    /// This accounts for the actual rendered height rather than just viewport settings
    fn get_max_scroll_y(&self) -> f32 {
        let vp = self.terminal.viewport.read();
        let computed_height = self.terminal.computed_visible_height.load(std::sync::atomic::Ordering::Relaxed) as f32;
        let visible_height = if computed_height > 0.0 { computed_height } else { vp.visible_height };
        (vp.content_height * vp.zoom - visible_height).max(0.0)
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
        }
    }

    /// Check if animation is needed
    pub fn needs_animation(&self) -> bool {
        self.is_loading || self.terminal.needs_animation() || self.scroll_mode != ScrollMode::Off
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
            PreviewMessage::LoadData(path, data) => self.load_data(path, data),
            PreviewMessage::ViewEvent(event) => {
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
                            return Task::done(PreviewMessage::ResetAnimationTimer);
                        }
                    }
                    ViewEvent::SauceInfo(sauce_opt, content_size) => {
                        // Forward sauce info as separate message for parent to handle
                        return Task::done(PreviewMessage::SauceInfoReceived(sauce_opt, content_size));
                    }
                    ViewEvent::Error(msg) => {
                        log::error!("Preview error: {}", msg);
                        self.is_loading = false;
                        self.preview_mode = PreviewMode::Error(msg);
                    }
                }
                Task::none()
            }
            PreviewMessage::ImageLoaded(load_id, result) => {
                // Only accept if this is the current load request
                if load_id == self.current_image_load_id {
                    self.is_loading = false;
                    match result {
                        Ok(handle) => {
                            self.preview_mode = PreviewMode::Image(handle);
                        }
                        Err(msg) => {
                            self.preview_mode = PreviewMode::Error(msg);
                        }
                    }
                }
                // Ignore outdated load results
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

                // Update viewport size to match current buffer size
                // This is important during baud emulation when buffer grows
                self.terminal.update_viewport_size();

                // Handle scroll mode (decided by background thread)
                match self.scroll_mode {
                    ScrollMode::ClampToBottom => {
                        // Clamp to bottom during baud emulation loading
                        // Use computed visible height for accurate max scroll calculation
                        let max_scroll_y = self.get_max_scroll_y();
                        self.terminal.scroll_to_immediate(0.0, max_scroll_y);
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

                        self.terminal.scroll_to_immediate(0.0, new_y);
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

                // Return any extra tasks
                if extra_tasks.is_empty() { Task::none() } else { Task::batch(extra_tasks) }
            }
            PreviewMessage::ScrollViewport(dx, dy) => {
                // User is scrolling manually, disable auto-scroll modes
                self.scroll_mode = ScrollMode::Off;
                self.terminal.scroll_by(dx, dy);
                self.terminal.sync_scrollbar_with_viewport();
                Task::none()
            }
            PreviewMessage::ScrollViewportTo(x, y) => {
                // User is scrolling manually, disable auto-scroll modes
                self.scroll_mode = ScrollMode::Off;
                self.terminal.scroll_to(x, y);
                self.terminal.sync_scrollbar_with_viewport();
                Task::none()
            }
            PreviewMessage::ScrollViewportToImmediate(x, y) => {
                // User is scrolling via scrollbar, disable auto-scroll modes
                self.scroll_mode = ScrollMode::Off;
                self.terminal.scroll_to_immediate(x, y);
                self.terminal.sync_scrollbar_with_viewport();
                Task::none()
            }
            PreviewMessage::ScrollViewportXToImmediate(x) => {
                // User is scrolling horizontally via scrollbar
                self.scroll_mode = ScrollMode::Off;
                let current_y = self.terminal.viewport.read().scroll_y;
                self.terminal.scroll_to_immediate(x, current_y);
                self.terminal.sync_scrollbar_with_viewport();
                Task::none()
            }
            PreviewMessage::ScrollbarHovered(is_hovered) => {
                self.terminal.scrollbar.set_hovered(is_hovered);
                Task::none()
            }
            PreviewMessage::HScrollbarHovered(is_hovered) => {
                // Horizontal scrollbar uses the same hover state for animation
                self.terminal.scrollbar.set_hovered(is_hovered);
                Task::none()
            }
            PreviewMessage::SetBaudEmulation(baud) => {
                self.set_baud_emulation(baud);
                Task::none()
            }
            PreviewMessage::TerminalMessage(msg) => {
                match msg {
                    icy_engine_gui::Message::ScrollViewport(dx, dy) => {
                        // User is scrolling manually, disable auto-scroll modes
                        self.scroll_mode = ScrollMode::Off;
                        self.terminal.scroll_by(dx, dy);
                        self.terminal.sync_scrollbar_with_viewport();
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
            PreviewMessage::StartSelection(sel) => {
                // Selection coordinates already include scroll offset from map_mouse_to_cell
                let mut screen = self.terminal.screen.lock();
                let _ = screen.set_selection(sel);
                Task::none()
            }
            PreviewMessage::UpdateSelection(pos) => {
                // Position already includes scroll offset from map_mouse_to_cell
                let mut screen = self.terminal.screen.lock();
                if let Some(mut sel) = screen.get_selection().clone() {
                    if !sel.locked {
                        sel.lead = pos;
                        let _ = screen.set_selection(sel);
                    }
                }
                Task::none()
            }
            PreviewMessage::EndSelection => {
                let mut screen = self.terminal.screen.lock();
                if let Some(mut sel) = screen.get_selection().clone() {
                    sel.locked = true;
                    let _ = screen.set_selection(sel);
                }
                Task::none()
            }
            PreviewMessage::ClearSelection => {
                let mut screen = self.terminal.screen.lock();
                let _ = screen.clear_selection();
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
            PreviewMode::Error(msg) => container(iced::widget::text(fl!(crate::LANGUAGE_LOADER, "preview-error", message = msg.clone())))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .style(|theme: &iced::Theme| container::Style {
                    background: Some(iced::Background::Color(theme::main_area_background(theme))),
                    ..Default::default()
                })
                .into(),
            PreviewMode::Image(handle) => {
                let img = iced_image::Image::new(handle.clone()).content_fit(iced::ContentFit::Contain);

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
            PreviewMode::Terminal => {
                let terminal_view = TerminalView::show_with_effects(&self.terminal, monitor_settings).map(PreviewMessage::TerminalMessage);

                let vp = self.terminal.viewport.read();

                // Use computed visible height from shader if available, otherwise fall back to viewport
                let computed_height = self.terminal.computed_visible_height.load(std::sync::atomic::Ordering::Relaxed) as f32;
                let visible_height = if computed_height > 0.0 { computed_height } else { vp.visible_height };

                // Calculate if we need vertical scrollbar (content taller than visible area)
                let scrollbar_height_ratio = visible_height / vp.content_height.max(1.0);
                let needs_vscrollbar = scrollbar_height_ratio < 1.0;

                // Calculate if we need horizontal scrollbar (content wider than visible area)
                let visible_width = vp.visible_width;
                let scrollbar_width_ratio = visible_width / vp.content_width.max(1.0);
                let needs_hscrollbar = scrollbar_width_ratio < 1.0;

                let content_height = vp.content_height;
                let content_width = vp.content_width;
                drop(vp);

                if needs_vscrollbar || needs_hscrollbar {
                    let scrollbar_visibility = self.terminal.scrollbar.visibility;

                    let mut layers: Vec<Element<'_, PreviewMessage>> = vec![terminal_view];

                    // Add vertical scrollbar if needed
                    if needs_vscrollbar {
                        let scrollbar_position = self.terminal.scrollbar.scroll_position;
                        let max_scroll_y = (content_height - visible_height).max(0.0);

                        let vscrollbar_view = ScrollbarOverlay::new(
                            scrollbar_visibility,
                            scrollbar_position,
                            scrollbar_height_ratio,
                            max_scroll_y,
                            self.terminal.scrollbar_hover_state.clone(),
                            |x, y| PreviewMessage::ScrollViewportToImmediate(x, y),
                            |is_hovered| PreviewMessage::ScrollbarHovered(is_hovered),
                        )
                        .view();

                        let vscrollbar_container: container::Container<'_, PreviewMessage> =
                            container(vscrollbar_view).width(Length::Fill).height(Length::Fill).align_x(Alignment::End);
                        layers.push(vscrollbar_container.into());
                    }

                    // Add horizontal scrollbar if needed
                    if needs_hscrollbar {
                        let scrollbar_position_x = self.terminal.scrollbar.scroll_position_x;
                        let max_scroll_x = (content_width - visible_width).max(0.0);

                        let hscrollbar_view = HorizontalScrollbarOverlay::new(
                            scrollbar_visibility,
                            scrollbar_position_x,
                            scrollbar_width_ratio,
                            max_scroll_x,
                            self.terminal.scrollbar_hover_state.clone(),
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

    /// Create the view using internal monitor settings
    pub fn view(&self) -> Element<'_, PreviewMessage> {
        self.view_with_settings(None)
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
