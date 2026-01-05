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

pub mod export_dialog;
mod icons;
mod messages;
mod playback_controls;

pub use export_dialog::*;
pub use messages::*;
pub use playback_controls::*;

use std::{path::PathBuf, sync::Arc, time::Instant};

use icy_engine_gui::{theme::main_area_background, ui::DialogStack, MonitorSettings, ScalingMode, Terminal, TerminalView};
use icy_engine_scripting::Animator;
use icy_ui::widget::canvas;
use icy_ui::widget::canvas::Canvas;
use icy_ui::{
    highlighter, mouse,
    widget::{column, container, pane_grid, row, rule, scrollable, stack, text, text_editor, Space},
    Background, Border, Element, Length, Task, Theme,
};
use parking_lot::Mutex;

use crate::fl;
use crate::ui::main_window::Message;

/// Default animation speed in milliseconds
#[allow(dead_code)]
const DEFAULT_FRAME_DELAY: u32 = 100;

/// Pane content types for the split view
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationPane {
    /// Code editor pane (left)
    CodeEditor,
    /// Preview pane (right)
    Preview,
}

/// Create monitor settings optimized for animation preview
/// Uses auto-scaling without integer scaling for smooth preview
fn create_preview_monitor_settings() -> Arc<MonitorSettings> {
    let mut settings = MonitorSettings::default();
    settings.scaling_mode = ScalingMode::Auto;
    settings.use_integer_scaling = false;
    Arc::new(settings)
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

    /// Whether the editor is dirty (unsaved changes)
    is_dirty: bool,

    /// Undo stack (text snapshots)
    undo_stack: Vec<String>,

    /// Redo stack (text snapshots)
    redo_stack: Vec<String>,

    /// Preview screen buffer
    preview_screen: Option<Arc<Mutex<Box<dyn icy_engine::Screen>>>>,

    /// Preview terminal for rendering
    preview_terminal: Option<Terminal>,

    /// Current monitor settings for preview
    preview_monitor: Arc<MonitorSettings>,

    /// Last displayed frame index (to detect changes)
    last_preview_frame: usize,

    /// Whether the log panel is visible
    log_panel_visible: bool,

    /// Pane grid state for resizable split view
    panes: pane_grid::State<AnimationPane>,
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

    /// Internal counter to keep redraw-driven playback alive.
    ///
    /// Without this, `Tick` may not mutate any visible state for most frames
    /// (because frame-delay hasn't elapsed yet), which can cause redraws to stop.
    tick_seq: u64,
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self {
            current_frame: 0,
            is_playing: false,
            is_loop: false,
            last_update: Instant::now(),
            speed: 1.0,
            tick_seq: 0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct PlaybackRedrawTicker {
    /// True if animation is playing
    playing: bool,
    /// True if we need ticks for initialization/compilation
    needs_update: bool,
}

#[derive(Debug)]
struct PlaybackRedrawTickerState {
    cache: canvas::Cache,
    last_redraw: Option<Instant>,
}

impl Default for PlaybackRedrawTickerState {
    fn default() -> Self {
        Self {
            cache: canvas::Cache::new(),
            last_redraw: None,
        }
    }
}

impl icy_ui::widget::canvas::Program<AnimationEditorMessage> for PlaybackRedrawTicker {
    type State = PlaybackRedrawTickerState;

    fn update(
        &self,
        state: &mut Self::State,
        event: &icy_ui::Event,
        _bounds: icy_ui::Rectangle,
        _cursor: mouse::Cursor,
    ) -> Option<icy_ui::widget::canvas::Action<AnimationEditorMessage>> {
        if let icy_ui::Event::Window(icy_ui::window::Event::RedrawRequested(now)) = event {
            if self.playing || self.needs_update {
                state.last_redraw = Some(*now);
                // Publishing `Tick` will schedule the next redraw as long as `Tick`
                // actually updates state (like the ColorSwitcher pattern).
                return Some(icy_ui::widget::canvas::Action::publish(AnimationEditorMessage::Tick));
            }

            state.last_redraw = None;
        }

        None
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &icy_ui::Renderer,
        _theme: &Theme,
        bounds: icy_ui::Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<icy_ui::widget::canvas::Geometry> {
        let geometry = _state.cache.draw(renderer, bounds.size(), |_| {
            // Intentionally draw nothing. A cached (empty) geometry keeps the widget
            // in the render tree so it can observe RedrawRequested events.
        });
        vec![geometry]
    }
}

impl AnimationEditor {
    /// Create a new animation editor with empty script
    pub fn new() -> Self {
        let animator = Arc::new(Mutex::new(Animator::default()));

        // Create pane layout: Code Editor | Preview
        let (mut panes, code_pane) = pane_grid::State::new(AnimationPane::CodeEditor);
        // Split vertically, preview on the right
        let _ = panes.split(pane_grid::Axis::Vertical, code_pane, AnimationPane::Preview);

        Self {
            script: text_editor::Content::with_text(""),
            animator,
            next_animator: None,
            parent_path: None,
            file_path: None,
            playback: PlaybackState::default(),
            scale: 1.0,
            needs_recompile: false,
            last_change: Instant::now(),
            first_frame: true,
            restore_frame: 0,
            is_dirty: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            preview_screen: None,
            preview_terminal: None,
            preview_monitor: create_preview_monitor_settings(),
            last_preview_frame: usize::MAX,
            log_panel_visible: false,
            panes,
        }
    }

    /// Create a new animation editor from a file
    pub fn from_file(path: PathBuf, content: String) -> Self {
        let parent_path = path.parent().map(|p| p.to_path_buf());
        let animator = Animator::run(&parent_path, content.clone());

        // Create pane layout: Code Editor | Preview
        let (mut panes, code_pane) = pane_grid::State::new(AnimationPane::CodeEditor);
        // Split vertically, preview on the right
        let _ = panes.split(pane_grid::Axis::Vertical, code_pane, AnimationPane::Preview);

        Self {
            script: text_editor::Content::with_text(&content),
            animator,
            next_animator: None,
            parent_path,
            file_path: Some(path),
            playback: PlaybackState::default(),
            scale: 1.0,
            needs_recompile: false,
            last_change: Instant::now(),
            first_frame: true,
            restore_frame: 0,
            is_dirty: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            preview_screen: None,
            preview_terminal: None,
            preview_monitor: create_preview_monitor_settings(),
            last_preview_frame: usize::MAX,
            log_panel_visible: false,
            panes,
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

    /// Check if we need recompile checks (for debounced recompilation)
    pub fn needs_recompile_check(&self) -> bool {
        self.needs_recompile || self.next_animator.is_some()
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

    /// Check if there's an error
    pub fn has_error(&self) -> bool {
        !self.animator.lock().error.is_empty()
    }

    /// Get the error message
    pub fn error_message(&self) -> String {
        self.animator.lock().error.clone()
    }

    /// Get the last log message up to current frame (for status bar)
    pub fn last_log_message(&self) -> Option<String> {
        let animator = self.animator.lock();
        let current_frame = self.playback.current_frame;

        // Find the last log entry that is <= current frame
        animator
            .log
            .iter()
            .filter(|entry| entry.frame <= current_frame)
            .last()
            .map(|entry| format!("[{}] {}", entry.frame, entry.text.clone()))
    }

    /// Check if log panel is visible
    pub fn is_log_visible(&self) -> bool {
        self.log_panel_visible
    }

    /// Get current time position and total duration in milliseconds
    /// Returns (current_time_ms, total_time_ms)
    pub fn get_time_info(&self) -> (u64, u64) {
        let animator = self.animator.lock();
        let frames = &animator.frames;

        if frames.is_empty() {
            return (0, 0);
        }

        let mut current_time: u64 = 0;
        let mut total_time: u64 = 0;
        let current_frame = self.playback.current_frame;

        for (i, (_, _, delay)) in frames.iter().enumerate() {
            if i < current_frame {
                current_time += *delay as u64;
            }
            total_time += *delay as u64;
        }

        (current_time, total_time)
    }

    /// Format milliseconds as MM:SS.s (e.g., "01:23.4")
    pub fn format_time(ms: u64) -> String {
        let total_seconds = ms / 1000;
        let tenths = (ms % 1000) / 100;
        let minutes = total_seconds / 60;
        let seconds = total_seconds % 60;
        format!("{:02}:{:02}.{}", minutes, seconds, tenths)
    }

    /// Check if dirty
    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    /// Get description of next undo operation (for menu display)
    pub fn undo_description(&self) -> Option<String> {
        if self.undo_stack.is_empty() {
            None
        } else {
            Some(crate::fl!("undo-animation-edit"))
        }
    }

    /// Get description of next redo operation (for menu display)
    pub fn redo_description(&self) -> Option<String> {
        if self.redo_stack.is_empty() {
            None
        } else {
            Some(crate::fl!("undo-animation-edit"))
        }
    }

    /// Get file path
    pub fn file_path(&self) -> Option<&PathBuf> {
        self.file_path.as_ref()
    }

    /// Set file path
    pub fn set_file_path(&mut self, path: PathBuf) {
        self.parent_path = path.parent().map(|p| p.to_path_buf());
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
        self.undo_stack.len()
    }

    /// Get session data for serialization
    pub fn get_session_data(&self) -> Option<crate::session::AnimationSessionState> {
        Some(crate::session::AnimationSessionState {
            version: 1,
            undo_stack: self.undo_stack.clone(),
            current_frame: self.playback.current_frame,
            playback_position: 0.0, // TODO: track actual playback position
            is_playing: self.playback.is_playing,
            script_scroll_offset: 0.0, // TODO: track scroll position
        })
    }

    /// Restore session data from serialization
    pub fn set_session_data(&mut self, state: crate::session::AnimationSessionState) {
        self.undo_stack = state.undo_stack;
        self.playback.current_frame = state.current_frame;
        self.playback.is_playing = state.is_playing;
    }

    /// Get bytes for autosave (returns the Lua script as bytes)
    pub fn get_autosave_bytes(&self) -> Result<Vec<u8>, String> {
        Ok(self.get_script().into_bytes())
    }

    /// Load from an autosave file, using the original path for file association
    pub fn load_from_autosave(autosave_path: &std::path::Path, original_path: PathBuf) -> Result<Self, String> {
        let content = std::fs::read_to_string(autosave_path).map_err(|e| format!("Failed to load autosave: {}", e))?;
        let mut editor = Self::from_file(original_path, content);
        editor.is_dirty = true; // Autosave means we have unsaved changes
        Ok(editor)
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
                // Immediately update preview after successful compile
                self.update_preview_terminal();
            }
        }

        // Check if initial animator is ready but preview not yet created
        if self.preview_terminal.is_none() && self.is_ready() && self.frame_count() > 0 {
            self.update_preview_terminal();
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
            terminal.set_fit_terminal_height_to_bounds(true);
            terminal.update_viewport_size();

            // Mark viewport as changed to force re-render
            terminal.mark_viewport_changed();

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

    /// Handle messages
    pub fn update(&mut self, message: AnimationEditorMessage, dialogs: &mut DialogStack<Message>) -> Task<AnimationEditorMessage> {
        match message {
            // ═══════════════════════════════════════════════════════════════
            // Dialog-related messages (moved from MainWindow)
            // ═══════════════════════════════════════════════════════════════
            AnimationEditorMessage::ShowExportDialog => {
                let animator = self.animator.clone();
                let source_path = self.file_path().cloned();
                dialogs.push(AnimationExportDialog::new(animator, source_path.as_ref()));
                Task::none()
            }
            AnimationEditorMessage::ExportDialog(_) => {
                // Handled by DialogStack
                Task::none()
            }

            // ═══════════════════════════════════════════════════════════════
            // Script editing
            // ═══════════════════════════════════════════════════════════════
            AnimationEditorMessage::ScriptAction(action) => {
                let is_edit = action.is_edit();
                if is_edit {
                    // Save current state before edit for undo
                    self.undo_stack.push(self.script.text());
                    self.redo_stack.clear(); // Clear redo on new edit
                }
                self.script.perform(action);
                if is_edit {
                    self.is_dirty = true;
                    self.schedule_recompile();
                }
                Task::none()
            }

            AnimationEditorMessage::TogglePlayback => {
                if self.playback.is_playing {
                    self.playback.is_playing = false;
                    Task::none()
                } else {
                    // Reset to beginning if at end
                    if self.playback.current_frame + 1 >= self.frame_count() {
                        self.playback.current_frame = 0;
                    }
                    self.playback.is_playing = true;
                    self.playback.last_update = Instant::now();
                    Task::none()
                }
            }

            AnimationEditorMessage::Stop => {
                self.playback.is_playing = false;
                self.playback.current_frame = 0;
                self.update_preview_terminal();
                Task::none()
            }

            AnimationEditorMessage::Restart => {
                self.playback.current_frame = 0;
                self.playback.is_playing = true;
                self.playback.last_update = Instant::now();
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

            AnimationEditorMessage::ToggleLogPanel => {
                self.log_panel_visible = !self.log_panel_visible;
                Task::none()
            }

            AnimationEditorMessage::PaneResized(pane_grid::ResizeEvent { split, ratio }) => {
                self.panes.resize(split, ratio);
                Task::none()
            }

            AnimationEditorMessage::Tick => {
                if self.playback.is_playing {
                    self.playback.tick_seq = self.playback.tick_seq.wrapping_add(1);
                }
                self.update_animator();
                Task::none()
            }

            AnimationEditorMessage::PreviewEvent(_) => Task::none(),

            AnimationEditorMessage::Recompile => {
                self.recompile();
                Task::none()
            }

            AnimationEditorMessage::Undo => {
                if let Some(prev_text) = self.undo_stack.pop() {
                    // Save current state to redo stack
                    self.redo_stack.push(self.script.text());
                    // Restore previous state
                    self.script = text_editor::Content::with_text(&prev_text);
                    self.schedule_recompile();
                }
                Task::none()
            }

            AnimationEditorMessage::Redo => {
                if let Some(next_text) = self.redo_stack.pop() {
                    // Save current state to undo stack
                    self.undo_stack.push(self.script.text());
                    // Restore next state
                    self.script = text_editor::Content::with_text(&next_text);
                    self.schedule_recompile();
                }
                Task::none()
            }
        }
    }

    /// Render the animation editor view
    ///
    /// The optional `chat_panel` parameter is accepted for API consistency but
    /// currently ignored since animation editor doesn't support collaboration.
    pub fn view(&self, _chat_panel: Option<Element<'_, AnimationEditorMessage>>) -> Element<'_, AnimationEditorMessage> {
        // Use pane_grid for resizable split view
        pane_grid::PaneGrid::new(&self.panes, |_id, pane, _is_maximized| {
            let content: Element<'_, AnimationEditorMessage> = match pane {
                AnimationPane::CodeEditor => self.view_code_editor_pane(),
                AnimationPane::Preview => self.view_preview_pane(),
            };
            pane_grid::Content::new(content)
        })
        .on_resize(10, AnimationEditorMessage::PaneResized)
        .spacing(1)
        .into()
    }

    /// Render the code editor pane (left side)
    fn view_code_editor_pane(&self) -> Element<'_, AnimationEditorMessage> {
        // Code editor with Lua syntax highlighting and monospace font
        // Note: text_editor has its own built-in scrollbar, don't wrap in scrollable!
        let code_editor = text_editor(&self.script)
            .on_action(AnimationEditorMessage::ScriptAction)
            .highlight("lua", highlighter::Theme::SolarizedDark)
            .font(icy_ui::Font::MONOSPACE)
            .padding(8)
            .height(Length::Fill);

        // Code panel - text_editor handles its own scrolling
        // Add left padding for visual separation from pane edge
        let code_panel = container(code_editor).width(Length::Fill).height(Length::Fill).padding(8); // top, right, bottom, left

        // Build code editor pane, optionally with log panel below
        if self.log_panel_visible {
            let log_panel = self.view_log_panel();
            column![code_panel, rule::horizontal(1), log_panel,]
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else {
            container(code_panel).width(Length::Fill).height(Length::Fill).into()
        }
    }

    /// Render the preview pane (right side)
    fn view_preview_pane(&self) -> Element<'_, AnimationEditorMessage> {
        // Get error message before building UI (to avoid lifetime issues)
        let error_msg = self.error_message();

        // Preview using TerminalView
        let preview_element: Element<'_, AnimationEditorMessage> = if self.has_error() {
            // Show error
            container(text(error_msg).size(14).style(|theme: &Theme| icy_ui::widget::text::Style {
                color: Some(theme.destructive.base),
            }))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
        } else if !self.is_ready() {
            // Show loading
            container(text(fl!("animation-compiling")).size(14))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into()
        } else if self.frame_count() == 0 {
            // No frames
            container(text(fl!("animation-no-frames")).size(14))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into()
        } else if let Some(terminal) = &self.preview_terminal {
            // Show terminal view with current frame
            // Enable auto-scaling for the preview (like terminal)
            let view = TerminalView::show_with_effects(terminal, self.preview_monitor.clone(), None).map(AnimationEditorMessage::PreviewEvent);
            view
        } else {
            // No terminal yet
            container(text(fl!("animation-preparing")).size(14))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into()
        };

        // Create frame info overlay (frame counter and time display)
        let frame_info_overlay = self.view_frame_info_overlay();

        // Playback needs continuous redraws while playing.
        // We follow the same RedrawRequested-driven pattern as ColorSwitcher:
        // publish `Tick` on redraw; handling `Tick` updates state and triggers the next redraw.
        // Also need ticks during initialization/compilation to update preview
        let needs_update = self.preview_terminal.is_none() || self.next_animator.is_some() || self.needs_recompile;
        let redraw_ticker: Element<'_, AnimationEditorMessage> = Canvas::new(PlaybackRedrawTicker {
            playing: self.playback.is_playing && self.is_ready() && !self.has_error() && self.frame_count() > 0,
            needs_update,
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .into();

        // Stack the terminal view with the frame info overlay
        // Put ticker FIRST so it can observe RedrawRequested even if the terminal shader
        // returns an Action for the same event.
        let preview_with_overlay = stack![redraw_ticker, preview_element, frame_info_overlay]
            .width(Length::Fill)
            .height(Length::Fill);

        // Preview container (takes most of the vertical space)
        // Use a distinct background color for the terminal area
        let preview_container = container(preview_with_overlay)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(4)
            .style(|theme: &Theme| container::Style {
                background: Some(Background::Color(main_area_background(theme))),
                border: Border {
                    color: theme.primary.divider,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            });

        // Player controls (below the terminal, like a video player)
        let player_controls = view_player_controls(self);

        // Preview pane: Preview on top, controls below
        column![preview_container, player_controls,]
            .spacing(4)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(8)
            .into()
    }

    /// Render the frame info overlay (positioned at top of preview)
    fn view_frame_info_overlay(&self) -> Element<'_, AnimationEditorMessage> {
        let frame_count = self.frame_count();
        let current_frame = self.current_frame();

        // Frame counter text
        let frame_text = if frame_count > 0 {
            fl!("animation-frame-display", current = ((current_frame + 1) as i32), total = (frame_count as i32))
        } else {
            fl!("animation-no-frames")
        };

        let frame_label = container(text(frame_text).size(12).font(icy_ui::Font::MONOSPACE))
            .padding([4, 10])
            .style(|_theme: &Theme| container::Style {
                background: Some(Background::Color(icy_ui::Color::from_rgba(0.0, 0.0, 0.0, 0.5))),
                border: Border {
                    color: icy_ui::Color::TRANSPARENT,
                    width: 0.0,
                    radius: 4.0.into(),
                },
                text_color: Some(icy_ui::Color::WHITE),
                ..Default::default()
            });

        // Time display
        let (current_time_ms, total_time_ms) = self.get_time_info();
        let time_text = format!(
            "{} / {}",
            AnimationEditor::format_time(current_time_ms),
            AnimationEditor::format_time(total_time_ms)
        );

        let time_label = container(text(time_text).size(12).font(icy_ui::Font::MONOSPACE))
            .padding([4, 10])
            .style(|_theme: &Theme| container::Style {
                background: Some(Background::Color(icy_ui::Color::from_rgba(0.0, 0.0, 0.0, 0.5))),
                border: Border {
                    color: icy_ui::Color::TRANSPARENT,
                    width: 0.0,
                    radius: 4.0.into(),
                },
                text_color: Some(icy_ui::Color::WHITE),
                ..Default::default()
            });

        // Row with frame info on left, time on right
        let info_row = row![frame_label, Space::new().width(Length::Fill), time_label,].padding(8).width(Length::Fill);

        // Position at top, let the rest be transparent/pass-through
        column![info_row, Space::new().height(Length::Fill),]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    /// Render the log panel (shown below editor when visible)
    fn view_log_panel(&self) -> Element<'_, AnimationEditorMessage> {
        let animator = self.animator.lock();

        let error_text = animator.error.clone();
        let current_frame = self.playback.current_frame;

        // Filter log entries to show only up to current frame
        let log_entries: Vec<_> = animator
            .log
            .iter()
            .filter(|entry| entry.frame <= current_frame)
            .map(|entry| (entry.frame, entry.text.clone()))
            .collect();
        drop(animator); // Release lock before building UI

        if !error_text.is_empty() {
            // Show error
            container(text(error_text).size(12).style(|theme: &Theme| icy_ui::widget::text::Style {
                color: Some(theme.destructive.base),
            }))
            .width(Length::Fill)
            .height(Length::Fixed(120.0))
            .padding(8)
            .into()
        } else if log_entries.is_empty() {
            // No log entries
            container(text(fl!("animation-no-log")).size(12).style(|theme: &Theme| icy_ui::widget::text::Style {
                color: Some(theme.primary.divider),
            }))
            .width(Length::Fill)
            .height(Length::Fixed(120.0))
            .padding(8)
            .into()
        } else {
            // Show log entries filtered by current frame
            let entries: Vec<Element<'_, AnimationEditorMessage>> = log_entries
                .into_iter()
                .map(|(frame, entry_text)| {
                    row![
                        text(format!("[{}]", frame)).size(11).style(|theme: &Theme| icy_ui::widget::text::Style {
                            color: Some(theme.accent.selected),
                        }),
                        Space::new().width(6),
                        text(entry_text).size(11),
                    ]
                    .spacing(2)
                    .into()
                })
                .collect();

            scrollable(column(entries).spacing(2).padding(4))
                .width(Length::Fill)
                .height(Length::Fixed(120.0))
                .into()
        }
    }

    // ═══════════════════════════════════════════════════════════════════════
    // MCP API methods
    // ═══════════════════════════════════════════════════════════════════════

    /// Get the script text (for MCP)
    pub fn get_script_text(&self) -> String {
        self.get_script()
    }

    /// Replace text in the script at given byte offset (for MCP)
    pub fn replace_script_text(&mut self, offset: usize, length: usize, new_text: &str) {
        let mut script = self.get_script();
        let end = (offset + length).min(script.len());
        script.replace_range(offset..end, new_text);

        // Save current script for undo
        self.push_undo_state();

        // Update the content
        self.script = text_editor::Content::with_text(&script);
        self.schedule_recompile();
        self.is_dirty = true;
    }

    /// Get script errors
    pub fn get_errors(&self) -> Vec<String> {
        let animator = self.animator.lock();
        if animator.error.is_empty() {
            vec![]
        } else {
            vec![animator.error.clone()]
        }
    }

    /// Whether animation is playing
    pub fn is_playing(&self) -> bool {
        self.playback.is_playing
    }

    /// Get a rendered frame as text (for MCP)
    pub fn get_frame_as_text(&self, frame: usize, format: &crate::mcp::types::ScreenCaptureFormat) -> Result<String, String> {
        let animator = self.animator.lock();

        if animator.frames.is_empty() {
            return Err("No frames rendered. Run the animation first.".to_string());
        }

        let frame_idx = if frame == 0 { 0 } else { frame.saturating_sub(1) };

        if frame_idx >= animator.frames.len() {
            return Err(format!("Frame {} out of range. Animation has {} frames.", frame, animator.frames.len()));
        }

        let (screen, _monitor_settings, _delay) = &animator.frames[frame_idx];

        // Convert screen to text using TextPane trait methods
        let width = screen.width() as usize;
        let height = screen.height() as usize;

        let mut result = String::new();

        match format {
            crate::mcp::types::ScreenCaptureFormat::Text => {
                // Plain text without colors
                for y in 0..height {
                    for x in 0..width {
                        let ch = screen.char_at(icy_engine::Position::new(x as i32, y as i32));
                        let c = if ch.ch == '\0' || ch.ch == ' ' { ' ' } else { ch.ch };
                        result.push(c);
                    }
                    result.push('\n');
                }
            }
            crate::mcp::types::ScreenCaptureFormat::Ansi => {
                // ANSI escape codes
                let mut last_fg = None;
                let mut last_bg = None;

                for y in 0..height {
                    for x in 0..width {
                        let ch = screen.char_at(icy_engine::Position::new(x as i32, y as i32));

                        // Check if colors changed
                        let fg = ch.attribute.foreground_color();
                        let bg = ch.attribute.background_color();

                        if last_fg != Some(fg) || last_bg != Some(bg) {
                            // Emit ANSI color codes (simplified - using palette indices)
                            let fg_idx = fg.as_palette_index().unwrap_or(7);
                            let bg_idx = bg.as_palette_index().unwrap_or(0);
                            result.push_str(&format!("\x1b[38;5;{}m\x1b[48;5;{}m", fg_idx, bg_idx));
                            last_fg = Some(fg);
                            last_bg = Some(bg);
                        }

                        let c = if ch.ch == '\0' { ' ' } else { ch.ch };
                        result.push(c);
                    }
                    result.push_str("\x1b[0m\n"); // Reset at end of line
                    last_fg = None;
                    last_bg = None;
                }
            }
        }

        Ok(result)
    }

    /// Push current state to undo stack
    fn push_undo_state(&mut self) {
        let current = self.get_script();
        self.undo_stack.push(current);
        self.redo_stack.clear();
    }
}

impl Default for AnimationEditor {
    fn default() -> Self {
        Self::new()
    }
}
