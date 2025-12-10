//! Animation editor messages

use std::path::PathBuf;

/// Messages for the animation editor
#[derive(Debug, Clone)]
pub enum AnimationEditorMessage {
    // === Script editing ===
    /// Script content changed
    ScriptAction(iced::widget::text_editor::Action),

    // === Playback controls ===
    /// Play/pause toggle
    TogglePlayback,
    /// Stop playback and reset to frame 0
    Stop,
    /// Go to previous frame
    PreviousFrame,
    /// Go to next frame
    NextFrame,
    /// Seek to first frame
    FirstFrame,
    /// Seek to last frame
    LastFrame,
    /// Seek to specific frame
    SeekFrame(usize),
    /// Toggle loop mode
    ToggleLoop,

    // === View controls ===
    /// Toggle scale (1x/2x)
    ToggleScale,
    /// Set custom scale
    SetScale(f32),
    /// Set playback speed multiplier
    SetPlaybackSpeed(f32),

    // === Export ===
    /// Browse for export path
    BrowseExportPath,
    /// Export path selected
    ExportPathSelected(Option<PathBuf>),
    /// Set export format
    SetExportFormat(usize),
    /// Start export
    StartExport,
    /// Export progress update
    ExportProgress(usize),
    /// Export completed
    ExportComplete,
    /// Export error
    ExportError(String),

    // === Animation update ===
    /// Tick for animation update
    Tick,
    /// Force recompile script
    Recompile,
}
