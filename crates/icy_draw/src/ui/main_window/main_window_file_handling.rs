//! File handling methods for MainWindow
//!
//! Contains all file operations: save, save-as, open, close, etc.

use std::path::PathBuf;

use icy_ui::Task;
use icy_engine::formats::FileFormat;
use icy_engine_gui::ui::{confirm_yes_no_cancel, error_dialog, DialogResult};

use super::main_window::{enforce_extension, MainWindow, Message, ModeState};
use crate::fl;
use crate::ui::editor::animation::AnimationEditor;
use crate::ui::editor::bitfont::BitFontEditor;

impl MainWindow {
    // ═══════════════════════════════════════════════════════════════════════════
    // Save operations
    // ═══════════════════════════════════════════════════════════════════════════

    /// Handle SaveFile message - save to current path or trigger SaveAs
    /// Disabled in collaboration mode (server handles persistence)
    pub(super) fn save_file(&mut self) -> Task<Message> {
        // In collaboration mode, don't save - the server handles persistence
        if self.collaboration_state.is_connected() {
            return Task::none();
        }

        if let Some(path) = self.mode_state.file_path().cloned() {
            // Save to the current path in its original format.
            // The format is determined by the file extension.
            self.save_to_path(path)
        } else {
            // No file path - trigger SaveAs
            self.save_file_as()
        }
    }

    /// Handle SaveFileAs message - show save dialog
    /// Disabled in collaboration mode (use Export instead)
    pub(super) fn save_file_as(&mut self) -> Task<Message> {
        // In collaboration mode, don't save - use Export instead
        if self.collaboration_state.is_connected() {
            return Task::none();
        }

        let (default_ext, filter_name) = self.mode_state.file_format();
        let all_files = fl!("file-dialog-filter-all-files");
        let title = fl!("file-dialog-save-as-title");
        let default_name = self
            .mode_state
            .file_path()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Untitled".to_string());
        let default_file_name = format!("{}.{}", default_name, default_ext);

        Task::perform(
            async move {
                rfd::AsyncFileDialog::new()
                    .add_filter(&filter_name, &[default_ext])
                    .add_filter(&all_files, &["*"])
                    .set_title(&title)
                    .set_file_name(default_file_name)
                    .save_file()
                    .await
                    .map(|f| f.path().to_path_buf())
            },
            |result| {
                if let Some(path) = result {
                    Message::FileSaved(enforce_extension(path, default_ext))
                } else {
                    Message::Noop // Cancelled
                }
            },
        )
    }

    /// Handle FileSaved message - save to selected path from SaveAs dialog
    pub(super) fn file_saved(&mut self, path: PathBuf) -> Task<Message> {
        match self.mode_state.save(&path) {
            Ok(()) => {
                self.mode_state.set_file_path(path.clone());
                self.mark_saved();
                self.options.write().recent_files.add_recent_file(&path);

                let saved_msg = Task::done(Message::SaveSucceeded(path.clone()));

                if self.close_after_save {
                    self.close_after_save = false;
                    self.pending_open_path = None;
                    return Task::batch([saved_msg, Task::done(Message::ForceCloseFile)]);
                }

                if let Some(pending) = self.pending_open_path.take() {
                    let next = match pending {
                        None => self.update(Message::ForceNewFile),
                        Some(open_path) if open_path.as_os_str().is_empty() => self.update(Message::OpenFile),
                        Some(open_path) => self.update(Message::FileOpened(open_path)),
                    };
                    return Task::batch([saved_msg, next]);
                }

                saved_msg
            }
            Err(e) => {
                self.close_after_save = false;
                self.pending_open_path = None;
                self.dialogs.push(error_dialog("Error Saving File", e, |_| Message::CloseDialog));
                Task::none()
            }
        }
    }

    /// Internal: save to a specific path and handle post-save actions
    fn save_to_path(&mut self, path: PathBuf) -> Task<Message> {
        match self.mode_state.save(&path) {
            Ok(()) => {
                self.mode_state.set_file_path(path.clone());
                self.mark_saved();
                self.options.write().recent_files.add_recent_file(&path);

                let saved_msg = Task::done(Message::SaveSucceeded(path.clone()));

                if self.close_after_save {
                    self.close_after_save = false;
                    self.pending_open_path = None;
                    return Task::batch([saved_msg, Task::done(Message::ForceCloseFile)]);
                }

                if let Some(pending) = self.pending_open_path.take() {
                    let next = match pending {
                        None => self.update(Message::ForceNewFile),
                        Some(open_path) if open_path.as_os_str().is_empty() => self.update(Message::ForceShowOpenDialog),
                        Some(open_path) => self.update(Message::FileOpened(open_path)),
                    };
                    return Task::batch([saved_msg, next]);
                }

                saved_msg
            }
            Err(e) => {
                self.close_after_save = false;
                self.pending_open_path = None;
                self.dialogs.push(error_dialog("Error Saving File", e, |_| Message::CloseDialog));
                Task::none()
            }
        }
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Close operations
    // ═══════════════════════════════════════════════════════════════════════════

    /// Handle CloseFile message - check for unsaved changes
    pub(super) fn close_file(&mut self) -> Task<Message> {
        if self.is_modified() {
            let filename = self
                .file_path()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| fl!("unsaved-title"));

            self.dialogs.push(confirm_yes_no_cancel(
                fl!("save-changes-title", filename = filename),
                fl!("save-changes-description"),
                |result| match result {
                    DialogResult::Yes => Message::SaveAndCloseFile,
                    DialogResult::No => Message::ForceCloseFile,
                    _ => Message::CloseDialog,
                },
            ));
            Task::none()
        } else {
            Task::done(Message::ForceCloseFile)
        }
    }

    /// Handle SaveAndCloseFile message - save then close
    pub(super) fn save_and_close_file(&mut self) -> Task<Message> {
        self.dialogs.pop();

        if let Some(path) = self.file_path().cloned() {
            match self.mode_state.save(&path) {
                Ok(()) => {
                    self.mark_saved();
                    Task::done(Message::ForceCloseFile)
                }
                Err(e) => {
                    self.dialogs.push(error_dialog("Error Saving File", e, |_| Message::CloseDialog));
                    Task::none()
                }
            }
        } else {
            self.close_after_save = true;
            self.save_file_as()
        }
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Open operations
    // ═══════════════════════════════════════════════════════════════════════════

    /// Handle OpenFile message - check for unsaved changes then show dialog
    pub(super) fn open_file(&mut self) -> Task<Message> {
        if self.is_modified() {
            let filename = self
                .file_path()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| fl!("unsaved-title"));

            self.pending_open_path = Some(Some(PathBuf::new()));

            self.dialogs.push(confirm_yes_no_cancel(
                fl!("save-changes-title", filename = filename),
                fl!("save-changes-description"),
                |result| match result {
                    DialogResult::Yes => Message::SaveFile,
                    DialogResult::No => Message::ForceShowOpenDialog,
                    _ => Message::CloseDialog,
                },
            ));
            Task::none()
        } else {
            self.show_open_dialog()
        }
    }

    /// Handle ForceShowOpenDialog message - show file picker
    pub(super) fn show_open_dialog(&mut self) -> Task<Message> {
        self.dialogs.pop();
        self.pending_open_path = None;

        let extensions: Vec<&str> = FileFormat::ALL
            .iter()
            .filter(|f| f.is_supported() || f.is_bitfont())
            .flat_map(|f| f.all_extensions())
            .copied()
            .collect();

        // Get the default directory: current file's directory or most recent file's directory
        let default_directory = self.file_path().and_then(|p| p.parent().map(|p| p.to_path_buf())).or_else(|| {
            self.options
                .read()
                .recent_files
                .files()
                .first()
                .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        });

        Task::perform(
            async move {
                let mut dialog = rfd::AsyncFileDialog::new()
                    .add_filter("Supported Files", &extensions)
                    .add_filter("All Files", &["*"])
                    .set_title("Open File");

                if let Some(dir) = default_directory {
                    dialog = dialog.set_directory(dir);
                }

                dialog.pick_file().await.map(|f| f.path().to_path_buf())
            },
            |result| {
                if let Some(path) = result {
                    Message::FileOpened(path)
                } else {
                    Message::Noop
                }
            },
        )
    }

    /// Handle OpenRecentFile message - check unsaved changes then open
    pub(super) fn open_recent_file(&mut self, path: PathBuf) -> Task<Message> {
        if self.is_modified() {
            let filename = self
                .file_path()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| fl!("unsaved-title"));

            let open_path = path.clone();
            self.dialogs.push(confirm_yes_no_cancel(
                fl!("save-changes-title", filename = filename),
                fl!("save-changes-description"),
                move |result| match result {
                    DialogResult::Yes => Message::SaveAndOpenFile(open_path.clone()),
                    DialogResult::No => Message::ForceOpenFile(open_path.clone()),
                    _ => Message::CloseDialog,
                },
            ));
            Task::none()
        } else {
            self.file_opened(path)
        }
    }

    /// Handle SaveAndOpenFile message - save then open file
    pub(super) fn save_and_open_file(&mut self, path: PathBuf) -> Task<Message> {
        self.dialogs.pop();

        if let Some(current_path) = self.file_path().cloned() {
            match self.mode_state.save(&current_path) {
                Ok(()) => {
                    self.mark_saved();
                    self.file_opened(path)
                }
                Err(e) => {
                    self.dialogs.push(error_dialog("Error Saving File", e, |_| Message::CloseDialog));
                    Task::none()
                }
            }
        } else {
            self.pending_open_path = Some(Some(path));
            self.save_file_as()
        }
    }

    /// Handle ForceOpenFile message - open without saving
    pub(super) fn force_open_file(&mut self, path: PathBuf) -> Task<Message> {
        self.dialogs.pop();
        self.file_opened(path)
    }

    /// Handle FileOpened message - load file into appropriate editor
    pub(super) fn file_opened(&mut self, path: PathBuf) -> Task<Message> {
        let format = FileFormat::from_path(&path);

        match format {
            Some(FileFormat::BitFont(_)) => match BitFontEditor::from_file(path.clone()) {
                Ok(editor) => {
                    self.mode_state = ModeState::BitFont(editor);
                    self.mark_saved();
                    self.options.write().recent_files.add_recent_file(&path);
                }
                Err(e) => {
                    self.dialogs.push(error_dialog(
                        "Error Loading Font",
                        format!("Failed to load '{}': {}", path.display(), e),
                        |_| Message::CloseDialog,
                    ));
                }
            },
            Some(FileFormat::IcyAnim) => match AnimationEditor::load_file(path.clone()) {
                Ok(editor) => {
                    self.mode_state = ModeState::Animation(editor);
                    self.mark_saved();
                    self.options.write().recent_files.add_recent_file(&path);
                }
                Err(e) => {
                    self.dialogs.push(error_dialog(
                        "Error Loading Animation",
                        format!("Failed to load '{}': {}", path.display(), e),
                        |_| Message::CloseDialog,
                    ));
                }
            },
            Some(FileFormat::CharacterFont(_)) => {
                match crate::ui::editor::charfont::CharFontEditor::with_file(path.clone(), self.options.clone(), self.font_library.clone()) {
                    Ok(editor) => {
                        self.mode_state = ModeState::CharFont(editor);
                        self.mark_saved();
                        self.options.write().recent_files.add_recent_file(&path);
                    }
                    Err(e) => {
                        self.dialogs.push(error_dialog(
                            "Error Loading TDF Font",
                            format!("Failed to load '{}': {}", path.display(), e),
                            |_| Message::CloseDialog,
                        ));
                    }
                }
            }
            _ => match crate::ui::editor::ansi::AnsiEditorMainArea::with_file(path.clone(), self.options.clone(), self.font_library.clone()) {
                Ok(editor) => {
                    self.mode_state = ModeState::Ansi(editor);
                    self.mark_saved();
                    self.options.write().recent_files.add_recent_file(&path);
                }
                Err(e) => {
                    self.dialogs.push(error_dialog(
                        "Error Loading File",
                        format!("Failed to load '{}': {}", path.display(), e),
                        |_| Message::CloseDialog,
                    ));
                }
            },
        }
        Task::none()
    }
}
