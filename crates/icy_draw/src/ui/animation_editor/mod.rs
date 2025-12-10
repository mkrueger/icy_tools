//! Animation Editor for icy_draw
//!
//! Provides a Lua-scripted animation editor for creating ANSI animations.
//! Features:
//! - Lua code editor with syntax highlighting
//! - Live preview of animation frames
//! - Playback controls (play, pause, seek, loop)
//! - Export to GIF and Asciicast formats
//! - Monitor settings per frame
//! - Log output from scripts

mod encoding;
pub mod export_dialog;
mod icons;
mod lua_highlighting;
pub mod menu_bar;
mod messages;
mod playback_controls;

pub use export_dialog::*;
pub use messages::*;
pub use playback_controls::*;

use std::{path::PathBuf, sync::Arc, time::Instant};

use iced::{
    Element, Length, Task, Theme, highlighter,
    widget::{Space, column, container, row, rule, scrollable, text, text_editor},
};
use icy_engine_gui::{MonitorSettings, ScalingMode, Terminal, TerminalView, set_default_auto_scaling_xy};
use icy_engine_scripting::Animator;
use parking_lot::Mutex;

/// Default animation speed in milliseconds
#[allow(dead_code)]
const DEFAULT_FRAME_DELAY: u32 = 100;

/// Create monitor settings optimized for animation preview
/// Uses auto-scaling without integer scaling for smooth preview
fn create_preview_monitor_settings() -> MonitorSettings {
    let mut settings = MonitorSettings::default();
    settings.scaling_mode = ScalingMode::Auto;
    settings.use_integer_scaling = false;
    settings
}

/// Animation editor state
pub struct AnimationEditor {
    /// Lua script source code
    script: text_editor::Content,

    /// The animator running the Lua script
    pub animator: Arc<Mutex<Animator>>,

    /// Next animator being computed (for live preview updates)
    next_animator: Option<Arc<Mutex<Animator>>>,

    /// Parent path for relative file loading
    parent_path: Option<PathBuf>,

    /// File path for saving
    file_path: Option<PathBuf>,

    /// Export settings
    export_path: PathBuf,
    export_format: usize,

    /// Playback state
    playback: PlaybackState,

    /// Preview scale factor
    scale: f32,

    /// Whether script needs recompilation
    needs_recompile: bool,

    /// Last script change time (for debouncing)
    last_change: Instant,

    /// Whether this is the first frame render
    first_frame: bool,

    /// Current frame to restore after recompile
    restore_frame: usize,

    /// Encoding state
    encoding: Option<EncodingState>,

    /// Whether the editor is dirty (unsaved changes)
    is_dirty: bool,

    /// Undo stack depth
    undo_depth: usize,

    /// Preview screen buffer
    preview_screen: Option<Arc<Mutex<Box<dyn icy_engine::Screen>>>>,

    /// Preview terminal for rendering
    preview_terminal: Option<Terminal>,

    /// Current monitor settings for preview
    preview_monitor: MonitorSettings,

    /// Last displayed frame index (to detect changes)
    last_preview_frame: usize,
}

/// Playback control state
pub struct PlaybackState {
    /// Current frame index
    pub current_frame: usize,
    /// Whether animation is playing
    pub is_playing: bool,
    /// Whether animation loops
    pub is_loop: bool,
    /// Last playback update time
    pub last_update: Instant,
    /// Playback speed multiplier (0.25, 0.5, 1.0, 2.0, 4.0)
    pub speed: f32,
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self {
            current_frame: 0,
            is_playing: false,
            is_loop: false,
            last_update: Instant::now(),
            speed: 1.0,
        }
    }
}

/// Encoding progress state
pub struct EncodingState {
    pub current_frame: usize,
    pub total_frames: usize,
    pub error: Option<String>,
}

impl AnimationEditor {
    /// Create a new animation editor with empty script
    pub fn new() -> Self {
        let animator = Arc::new(Mutex::new(Animator::default()));
        Self {
            script: text_editor::Content::with_text(""),
            animator,
            next_animator: None,
            parent_path: None,
            file_path: None,
            export_path: PathBuf::from("animation.gif"),
            export_format: 0,
            playback: PlaybackState::default(),
            scale: 1.0,
            needs_recompile: false,
            last_change: Instant::now(),
            first_frame: true,
            restore_frame: 0,
            encoding: None,
            is_dirty: false,
            undo_depth: 0,
            preview_screen: None,
            preview_terminal: None,
            preview_monitor: create_preview_monitor_settings(),
            last_preview_frame: usize::MAX,
        }
    }

    /// Create a new animation editor from a file
    pub fn from_file(path: PathBuf, content: String) -> Self {
        let parent_path = path.parent().map(|p| p.to_path_buf());
        let animator = Animator::run(&parent_path, content.clone());
        let export_path = path.with_extension("gif");

        Self {
            script: text_editor::Content::with_text(&content),
            animator,
            next_animator: None,
            parent_path,
            file_path: Some(path),
            export_path,
            export_format: 0,
            playback: PlaybackState::default(),
            scale: 1.0,
            needs_recompile: false,
            last_change: Instant::now(),
            first_frame: true,
            restore_frame: 0,
            encoding: None,
            is_dirty: false,
            undo_depth: 0,
            preview_screen: None,
            preview_terminal: None,
            preview_monitor: create_preview_monitor_settings(),
            last_preview_frame: usize::MAX,
        }
    }

    /// Get the script content as a string
    pub fn get_script(&self) -> String {
        self.script.text()
    }

    /// Check if the animation is ready (script finished executing)
    pub fn is_ready(&self) -> bool {
        self.animator.lock().success()
    }

    /// Get the number of frames
    pub fn frame_count(&self) -> usize {
        self.animator.lock().frames.len()
    }

    /// Get the current frame index
    pub fn current_frame(&self) -> usize {
        self.playback.current_frame
    }

    /// Get the playback speed multiplier
    pub fn playback_speed(&self) -> f32 {
        self.playback.speed
    }

    /// Get cursor position (line, column) - 0-indexed
    pub fn cursor_position(&self) -> (usize, usize) {
        let cursor = self.script.cursor();
        (cursor.position.line, cursor.position.column)
    }

    /// Get total line count
    pub fn line_count(&self) -> usize {
        self.script.line_count()
    }

    /// Check if there's an error
    pub fn has_error(&self) -> bool {
        !self.animator.lock().error.is_empty()
    }

    /// Get the error message
    pub fn error_message(&self) -> String {
        self.animator.lock().error.clone()
    }

    /// Check if dirty
    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    /// Mark as saved
    pub fn mark_saved(&mut self) {
        self.is_dirty = false;
    }

    /// Get file path
    pub fn file_path(&self) -> Option<&PathBuf> {
        self.file_path.as_ref()
    }

    /// Set file path
    pub fn set_file_path(&mut self, path: PathBuf) {
        self.parent_path = path.parent().map(|p| p.to_path_buf());
        self.export_path = path.with_extension("gif");
        self.file_path = Some(path);
    }

    /// Save the animation script to a file
    pub fn save(&mut self, path: &std::path::Path) -> Result<(), String> {
        let content = self.get_script();
        std::fs::write(path, content).map_err(|e| format!("Failed to save file: {}", e))?;
        self.is_dirty = false;
        Ok(())
    }

    /// Load an animation script from a file
    pub fn load_file(path: PathBuf) -> Result<Self, String> {
        let content = std::fs::read_to_string(&path).map_err(|e| format!("Failed to load file: {}", e))?;
        Ok(Self::from_file(path, content))
    }

    /// Get undo stack length (for dirty tracking)
    pub fn undo_stack_len(&self) -> usize {
        self.undo_depth
    }

    /// Check if the animation needs animation updates (for timer subscription)
    pub fn needs_animation(&self) -> bool {
        // Need animation ticks when:
        // - Animation is playing
        // - Script needs recompilation (checking debounce)
        // - Next animator is being computed
        // - Animator is running (not yet ready) - for initial load
        // - Preview terminal not yet created but animator is ready
        let animator_running = !self.is_ready();
        let needs_preview_update = self.is_ready() && self.preview_terminal.is_none();

        self.playback.is_playing || self.needs_recompile || self.next_animator.is_some() || animator_running || needs_preview_update
    }

    /// Schedule a script recompilation
    fn schedule_recompile(&mut self) {
        self.needs_recompile = true;
        self.last_change = Instant::now();
    }

    /// Check if we should recompile (debounced)
    fn should_recompile(&self) -> bool {
        self.needs_recompile && self.last_change.elapsed().as_millis() > 1000
    }

    /// Recompile the script
    fn recompile(&mut self) {
        self.restore_frame = self.playback.current_frame;
        let script = self.get_script();
        self.next_animator = Some(Animator::run(&self.parent_path, script));
        self.needs_recompile = false;
    }

    /// Update the animator state
    pub fn update_animator(&mut self) {
        // Check if next animator is ready
        if let Some(next) = &self.next_animator {
            let next_lock = next.lock();
            if next_lock.success() || !next_lock.error.is_empty() {
                drop(next_lock);
                self.animator = self.next_animator.take().unwrap();
                self.playback.current_frame = self.restore_frame.min(self.animator.lock().frames.len().saturating_sub(1));
                self.first_frame = true;
                // Force preview rebuild
                self.last_preview_frame = usize::MAX;
            }
        }

        // Check if we need to recompile
        if self.should_recompile() {
            self.recompile();
        }

        // Update playback
        if self.playback.is_playing && self.is_ready() {
            let animator = self.animator.lock();
            if !animator.frames.is_empty() {
                let delay = animator
                    .frames
                    .get(self.playback.current_frame)
                    .map(|(_, _, d)| *d)
                    .unwrap_or(DEFAULT_FRAME_DELAY);

                // Apply speed multiplier (higher speed = shorter delay)
                let adjusted_delay = (delay as f32 / self.playback.speed) as u128;

                if self.playback.last_update.elapsed().as_millis() > adjusted_delay {
                    drop(animator);
                    self.next_frame();
                    self.playback.last_update = Instant::now();
                }
            }
        }

        // Update preview terminal if frame changed
        self.update_preview_terminal();
    }

    /// Update the preview terminal with current frame data
    fn update_preview_terminal(&mut self) {
        let current_frame = self.playback.current_frame;

        // Only update if frame changed
        if current_frame == self.last_preview_frame && self.preview_terminal.is_some() {
            return;
        }

        let animator = self.animator.lock();
        if let Some((buffer, _settings, _delay)) = animator.frames.get(current_frame) {
            // Clone the buffer using clone_box()
            let boxed = buffer.clone_box();

            // Create screen arc and terminal
            let screen_arc: Arc<Mutex<Box<dyn icy_engine::Screen>>> = Arc::new(Mutex::new(boxed));
            let mut terminal = Terminal::new(screen_arc.clone());
            terminal.update_viewport_size();

            // Mark viewport as changed to force re-render
            terminal.viewport.write().changed.store(true, std::sync::atomic::Ordering::Relaxed);

            self.preview_screen = Some(screen_arc);
            self.preview_terminal = Some(terminal);

            self.last_preview_frame = current_frame;
        }
    }

    /// Move to next frame
    fn next_frame(&mut self) {
        let frame_count = self.frame_count();
        if frame_count == 0 {
            return;
        }

        self.playback.current_frame += 1;
        if self.playback.current_frame >= frame_count {
            if self.playback.is_loop {
                self.playback.current_frame = 0;
            } else {
                self.playback.current_frame = frame_count - 1;
                self.playback.is_playing = false;
            }
        }
    }

    /// Get current frame delay
    pub fn get_current_frame_delay(&self) -> Option<u32> {
        let animator = self.animator.lock();
        animator.frames.get(self.playback.current_frame).map(|(_, _, delay)| *delay)
    }

    /// Handle messages
    pub fn update(&mut self, message: AnimationEditorMessage) -> Task<AnimationEditorMessage> {
        match message {
            AnimationEditorMessage::ScriptAction(action) => {
                let is_edit = action.is_edit();
                self.script.perform(action);
                if is_edit {
                    self.is_dirty = true;
                    self.undo_depth += 1;
                    self.schedule_recompile();
                }
                Task::none()
            }

            AnimationEditorMessage::TogglePlayback => {
                if self.playback.is_playing {
                    self.playback.is_playing = false;
                } else {
                    // Reset to beginning if at end
                    if self.playback.current_frame + 1 >= self.frame_count() {
                        self.playback.current_frame = 0;
                    }
                    self.playback.is_playing = true;
                    self.playback.last_update = Instant::now();
                }
                Task::none()
            }

            AnimationEditorMessage::Stop => {
                self.playback.is_playing = false;
                self.playback.current_frame = 0;
                self.update_preview_terminal();
                Task::none()
            }

            AnimationEditorMessage::PreviousFrame => {
                if self.playback.current_frame > 0 {
                    self.playback.current_frame -= 1;
                    self.update_preview_terminal();
                }
                Task::none()
            }

            AnimationEditorMessage::NextFrame => {
                let frame_count = self.frame_count();
                if self.playback.current_frame + 1 < frame_count {
                    self.playback.current_frame += 1;
                    self.update_preview_terminal();
                }
                Task::none()
            }

            AnimationEditorMessage::FirstFrame => {
                self.playback.current_frame = 0;
                self.update_preview_terminal();
                Task::none()
            }

            AnimationEditorMessage::LastFrame => {
                let frame_count = self.frame_count();
                if frame_count > 0 {
                    self.playback.current_frame = frame_count - 1;
                    self.update_preview_terminal();
                }
                Task::none()
            }

            AnimationEditorMessage::SeekFrame(frame) => {
                let frame_count = self.frame_count();
                self.playback.current_frame = frame.min(frame_count.saturating_sub(1));
                self.update_preview_terminal();
                Task::none()
            }

            AnimationEditorMessage::ToggleLoop => {
                self.playback.is_loop = !self.playback.is_loop;
                Task::none()
            }

            AnimationEditorMessage::ToggleScale => {
                self.scale = if self.scale < 2.0 { 2.0 } else { 1.0 };
                Task::none()
            }

            AnimationEditorMessage::SetScale(scale) => {
                self.scale = scale;
                Task::none()
            }

            AnimationEditorMessage::SetPlaybackSpeed(speed) => {
                self.playback.speed = speed;
                Task::none()
            }

            AnimationEditorMessage::BrowseExportPath => {
                // File dialog would be opened here
                Task::none()
            }

            AnimationEditorMessage::ExportPathSelected(path) => {
                if let Some(p) = path {
                    self.export_path = p;
                }
                Task::none()
            }

            AnimationEditorMessage::SetExportFormat(format) => {
                self.export_format = format;
                let ext = encoding::get_encoder_extension(format);
                self.export_path.set_extension(ext);
                Task::none()
            }

            AnimationEditorMessage::StartExport => {
                // Start encoding in background thread
                Task::none()
            }

            AnimationEditorMessage::ExportProgress(frame) => {
                if let Some(ref mut enc) = self.encoding {
                    enc.current_frame = frame;
                }
                Task::none()
            }

            AnimationEditorMessage::ExportComplete => {
                self.encoding = None;
                Task::none()
            }

            AnimationEditorMessage::ExportError(error) => {
                if let Some(ref mut enc) = self.encoding {
                    enc.error = Some(error);
                }
                Task::none()
            }

            AnimationEditorMessage::Tick => {
                self.update_animator();
                Task::none()
            }

            AnimationEditorMessage::Recompile => {
                self.recompile();
                Task::none()
            }
        }
    }

    /// Render the animation editor view
    pub fn view(&self) -> Element<'_, AnimationEditorMessage> {
        // Left panel: Code editor with Lua syntax highlighting and monospace font
        let code_editor = text_editor(&self.script)
            .on_action(AnimationEditorMessage::ScriptAction)
            .highlight("lua", highlighter::Theme::SolarizedDark)
            .font(iced::Font::MONOSPACE)
            .height(Length::Fill);

        // Wrap code editor in scrollable container
        let code_panel = scrollable(container(code_editor).width(Length::Fill).height(Length::Fill).padding(8))
            .width(Length::FillPortion(1))
            .height(Length::Fill);

        // Right panel: Preview and controls
        let playback_controls = view_playback_controls(self);
        let frame_slider = view_frame_slider(self);

        // Get error message before building UI (to avoid lifetime issues)
        let error_msg = self.error_message();

        // Preview using TerminalView
        let preview_element: Element<'_, AnimationEditorMessage> = if self.has_error() {
            // Show error
            container(text(error_msg).size(14).style(|theme: &Theme| iced::widget::text::Style {
                color: Some(theme.extended_palette().danger.base.color),
            }))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
        } else if !self.is_ready() {
            // Show loading
            container(text("Compiling script...").size(14))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into()
        } else if self.frame_count() == 0 {
            // No frames
            container(text("No frames generated").size(14))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into()
        } else if let Some(terminal) = &self.preview_terminal {
            // Show terminal view with current frame
            // Enable auto-scaling for the preview (like terminal)
            set_default_auto_scaling_xy(true);
            let view = TerminalView::show_with_effects(terminal, self.preview_monitor.clone()).map(|_| AnimationEditorMessage::Tick);
            set_default_auto_scaling_xy(false);
            view
        } else {
            // No terminal yet
            container(text("Preparing preview...").size(14))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into()
        };

        let preview_container = container(preview_element).width(Length::Fill).height(Length::FillPortion(3)).padding(8);

        // Log panel
        let log_panel = self.view_log_panel();

        let right_panel = column![playback_controls, frame_slider, preview_container, rule::horizontal(1), log_panel,]
            .spacing(4)
            .width(Length::FillPortion(1))
            .height(Length::Fill);

        let right_container = container(right_panel).width(Length::FillPortion(1)).height(Length::Fill).padding(8);

        // Main layout: code editor | preview
        row![code_panel, rule::vertical(1), right_container,]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    /// Render the log panel
    fn view_log_panel(&self) -> Element<'_, AnimationEditorMessage> {
        let animator = self.animator.lock();

        let error_text = animator.error.clone();
        let log_entries: Vec<_> = animator.log.iter().map(|entry| (entry.frame, entry.text.clone())).collect();
        drop(animator); // Release lock before building UI

        if !error_text.is_empty() {
            // Show error
            container(text(error_text).size(12).style(|theme: &Theme| iced::widget::text::Style {
                color: Some(theme.extended_palette().danger.base.color),
            }))
            .width(Length::Fill)
            .height(Length::Fixed(100.0))
            .padding(8)
            .into()
        } else if log_entries.is_empty() {
            // No log entries
            container(text("No log entries").size(12).style(|theme: &Theme| iced::widget::text::Style {
                color: Some(theme.extended_palette().background.strong.color),
            }))
            .width(Length::Fill)
            .height(Length::Fixed(100.0))
            .padding(8)
            .into()
        } else {
            // Show log entries
            let entries: Vec<Element<'_, AnimationEditorMessage>> = log_entries
                .into_iter()
                .map(|(frame, entry_text)| {
                    row![text(format!("Frame {}:", frame)).size(12), Space::new().width(8), text(entry_text).size(12),]
                        .spacing(4)
                        .into()
                })
                .collect();

            scrollable(column(entries).spacing(2)).width(Length::Fill).height(Length::Fixed(100.0)).into()
        }
    }
}

impl Default for AnimationEditor {
    fn default() -> Self {
        Self::new()
    }
}
