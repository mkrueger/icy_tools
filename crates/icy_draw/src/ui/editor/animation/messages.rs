//! Animation editor messages

use iced::widget::pane_grid;

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
    /// Restart from beginning and play
    Restart,
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
    /// Toggle log panel visibility
    ToggleLogPanel,

    // === Pane grid ===
    /// Pane resized
    PaneResized(pane_grid::ResizeEvent),

    // === Animation update ===
    /// Tick for animation update
    Tick,
    /// Force recompile script
    Recompile,

    // === Undo/Redo ===
    /// Undo last edit
    Undo,
    /// Redo last undone edit
    Redo,

    // === Export ===
    /// Show export dialog
    ShowExportDialog,
    /// Export dialog messages
    ExportDialog(super::AnimationExportMessage),
}
