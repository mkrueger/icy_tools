//! Export dialog for saving buffers to various file formats.
//!
//! This dialog provides a UI for exporting screen buffers to different formats
//! including ANSI, ASCII, XBin, and image formats (PNG, GIF, etc.).

use i18n_embed_fl::fl;
use icy_ui::{
    widget::{checkbox, column, container, pick_list, row, scrollable, text, text_input, Space},
    Alignment, Element, Length,
};
use icy_engine::{
    formats::{FileFormat, FormatOptions, ImageFormat, SauceMetaData, SixelSettings},
    AnsiCompatibilityLevel, BufferType, SaveOptions, Screen, ScreenPreperation,
};
use parking_lot::Mutex;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::dialog::StateResult;
use super::{
    browse_button, button_row_with_left, dialog_area, error_tooltip, left_label_small, modal_container, primary_button, restore_defaults_button,
    secondary_button, separator, DIALOG_SPACING, DIALOG_WIDTH_LARGE, LABEL_SMALL_WIDTH, TEXT_SIZE_NORMAL, TEXT_SIZE_SMALL,
};
use crate::dialog_wrapper;
use crate::settings::effect_box;
use crate::LANGUAGE_LOADER;

/// Messages for the export dialog
#[derive(Debug, Clone)]
pub enum ExportDialogMessage {
    /// Perform the export
    Export,
    /// Change the export directory
    ChangeDirectory(String),
    /// Change the export filename
    ChangeFileName(String),
    /// Change the export format
    ChangeFormat(FileFormat),
    /// Open directory browser
    BrowseDirectory,
    /// Restore default settings
    RestoreDefaults,
    /// Cancel the dialog
    Cancel,
    // Format-specific options
    /// Toggle SAUCE saving
    ToggleSaveSauce(bool),
    /// Set ANSI compatibility level
    SetAnsiLevel(AnsiCompatibilityLevel),
    /// Set screen preparation
    SetScreenPrep(ScreenPreperation),
    /// Toggle max line length limit
    ToggleMaxLineLength(bool),
    /// Set max line length value
    SetMaxLineLength(String),
    /// Toggle UTF-8 output (for character formats)
    ToggleUtf8Output(bool),
    /// Toggle compression (for formats that support it)
    ToggleCompress(bool),
    // Sixel settings
    /// Set sixel max colors
    SetSixelMaxColors(String),
    /// Set sixel diffusion
    SetSixelDiffusion(String),
    /// Toggle sixel k-means
    ToggleSixelKmeans(bool),
}

/// Format category for UI grouping
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FormatCategory {
    /// ANSI format - full options (level, screen prep, line length, sixel)
    Ansi,
    /// Avatar format - screen prep only
    Avatar,
    /// PCBoard format - screen prep + UTF-8
    PCBoard,
    /// CtrlA/Renegade - screen prep only
    CtrlA,
    /// Binary formats - no options
    Binary,
    /// IcyDraw native format
    IcyDraw,
    /// Image formats
    Image,
    /// Other formats - no options
    Other,
}

/// State for the export dialog
#[dialog_wrapper(result_type = PathBuf)]
pub struct ExportDialogState {
    /// The screen buffer to export
    pub screen: Arc<Mutex<Box<dyn Screen>>>,
    /// Current export directory
    pub export_directory: String,
    /// Current export filename (without extension)
    pub export_filename: String,
    /// Current export format
    pub export_format: FileFormat,
    /// Temporary directory (edited in dialog)
    temp_directory: String,
    /// Temporary filename (edited in dialog)
    temp_filename: String,
    /// Temporary format (edited in dialog)
    temp_format: FileFormat,
    /// The buffer type determines which export formats are available
    #[allow(dead_code)]
    buffer_type: BufferType,
    /// Cached list of available formats for this buffer type
    available_formats: Vec<FileFormat>,
    /// Default directory provider function
    default_directory_fn: Option<Box<dyn Fn() -> PathBuf + Send + Sync>>,

    // SAUCE metadata
    /// Optional SAUCE metadata to include
    sauce_metadata: Option<SauceMetaData>,
    /// Whether to save SAUCE record
    save_sauce: bool,

    // Compatibility
    /// Whether buffer has sixels
    has_sixels: bool,

    // Format-specific options
    /// ANSI compatibility level
    ansi_level: AnsiCompatibilityLevel,
    /// Screen preparation
    screen_prep: ScreenPreperation,
    /// Max line length enabled
    max_line_length_enabled: bool,
    /// Max line length value
    max_line_length: u16,
    /// UTF-8 output (for character formats)
    utf8_output: bool,
    /// Compression enabled
    compress: bool,
    /// Sixel settings
    sixel_settings: SixelSettings,
}

impl ExportDialogState {
    /// Create a new export dialog state
    ///
    /// # Arguments
    /// * `initial_path` - Initial file path (can include directory and filename)
    /// * `buffer_type` - The type of buffer being exported (determines available formats)
    /// * `screen` - The screen buffer to export
    pub fn new(initial_path: String, buffer_type: BufferType, screen: Arc<Mutex<Box<dyn Screen>>>) -> Self {
        let path: &Path = Path::new(&initial_path);

        // Get available formats for this buffer type (including image formats)
        let available_formats = FileFormat::save_formats_with_images_for_buffer_type(buffer_type);

        // Determine format from extension if present, or use first available
        let format = path
            .extension()
            .and_then(|ext| ext.to_str())
            .and_then(|ext| FileFormat::from_extension(&ext.to_lowercase()))
            .and_then(|fmt| {
                // Only use the detected format if it's compatible with the buffer type
                if available_formats.contains(&fmt) {
                    Some(fmt)
                } else {
                    None
                }
            })
            .unwrap_or_else(|| available_formats.first().copied().unwrap_or(FileFormat::Image(ImageFormat::Png)));

        let (dir, file) = if path.is_absolute() {
            (
                path.parent().and_then(|p| p.to_str()).unwrap_or("").to_string(),
                path.file_stem().and_then(|f| f.to_str()).unwrap_or("export").to_string(),
            )
        } else {
            (
                std::env::current_dir()
                    .ok()
                    .and_then(|p| p.to_str().map(|s| s.to_string()))
                    .unwrap_or_else(|| ".".to_string()),
                path.file_stem().and_then(|f| f.to_str()).unwrap_or("export").to_string(),
            )
        };

        Self {
            screen,
            export_directory: dir.clone(),
            export_filename: file.clone(),
            export_format: format,
            temp_directory: dir,
            temp_filename: file,
            temp_format: format,
            buffer_type,
            available_formats,
            default_directory_fn: None,
            // SAUCE metadata
            sauce_metadata: None,
            save_sauce: true,
            // Compatibility
            has_sixels: false,
            // Format options
            ansi_level: AnsiCompatibilityLevel::default(),
            screen_prep: ScreenPreperation::None,
            max_line_length_enabled: false,
            max_line_length: 80,
            utf8_output: false,
            compress: false,
            sixel_settings: SixelSettings::default(),
        }
    }

    /// Set the SAUCE metadata
    pub fn with_sauce_metadata(mut self, metadata: SauceMetaData) -> Self {
        self.sauce_metadata = Some(metadata);
        self
    }

    /// Set whether the buffer has sixels
    pub fn with_has_sixels(mut self, has_sixels: bool) -> Self {
        self.has_sixels = has_sixels;
        self
    }

    /// Set a function that provides the default directory
    pub fn with_default_directory_fn<F>(mut self, f: F) -> Self
    where
        F: Fn() -> PathBuf + Send + Sync + 'static,
    {
        self.default_directory_fn = Some(Box::new(f));
        self
    }

    /// Get the format category for the current format
    fn format_category(&self) -> FormatCategory {
        match self.temp_format {
            FileFormat::Ansi => FormatCategory::Ansi,
            FileFormat::Avatar => FormatCategory::Avatar,
            FileFormat::PCBoard => FormatCategory::PCBoard,
            FileFormat::CtrlA | FileFormat::Renegade => FormatCategory::CtrlA,
            FileFormat::IcyDraw => FormatCategory::IcyDraw,
            FileFormat::Image(_) => FormatCategory::Image,
            // Binary formats - no options
            FileFormat::Artworx | FileFormat::IceDraw | FileFormat::TundraDraw | FileFormat::XBin | FileFormat::Bin => FormatCategory::Binary,
            // ASCII, PETSCII, Atascii, etc. - no special options
            _ => FormatCategory::Other,
        }
    }

    /// Get the full export path
    pub fn get_full_path(&self) -> PathBuf {
        let filename_with_ext = format!("{}.{}", self.export_filename, self.export_format.primary_extension());
        Path::new(&self.export_directory).join(filename_with_ext)
    }

    /// Get the temporary full path (while editing)
    pub fn get_temp_full_path(&self) -> PathBuf {
        let filename_with_ext = format!("{}.{}", self.temp_filename, self.temp_format.primary_extension());
        Path::new(&self.temp_directory).join(filename_with_ext)
    }

    /// Export the buffer to file
    pub fn export_buffer(&self) -> Result<PathBuf, String> {
        let full_path = self.get_full_path();

        // Create directory if it doesn't exist
        if let Err(e) = std::fs::create_dir_all(&self.export_directory) {
            return Err(format!("Failed to create directory: {}", e));
        }

        // Get the buffer from edit state
        let mut screen = self.screen.lock();

        // Handle image export using ImageFormat from icy_engine
        if let FileFormat::Image(img_format) = self.export_format {
            img_format
                .save_screen(screen.as_ref(), &full_path)
                .map_err(|e| format!("Failed to save image: {}", e))?;
            return Ok(full_path);
        }

        // Get the file extension for format
        let ext = self.export_format.primary_extension();

        // Create save options based on format-specific settings
        let options = self.build_save_options();

        // Convert buffer to bytes based on format
        let content = screen.to_bytes(ext, &options).map_err(|e| format!("Failed to convert buffer: {}", e))?;

        // Write the bytes to file
        std::fs::write(&full_path, &content).map_err(|e| format!("Failed to write file: {}", e))?;

        Ok(full_path)
    }

    /// Build SaveOptions from current dialog state
    fn build_save_options(&self) -> SaveOptions {
        let mut options = SaveOptions::new();

        // Set SAUCE metadata if enabled
        if self.save_sauce {
            options.sauce = self.sauce_metadata.clone();
        }

        // Apply format-specific options
        match self.format_category() {
            FormatCategory::Ansi => {
                options.format = FormatOptions::Ansi(icy_engine::AnsiFormatOptions {
                    level: self.ansi_level,
                    screen_prep: self.screen_prep,
                    line_length: if self.max_line_length_enabled {
                        icy_engine::LineLength::Maximum(self.max_line_length)
                    } else {
                        icy_engine::LineLength::Default
                    },
                    sixel: self.sixel_settings.clone(),
                    ..Default::default()
                });
            }
            FormatCategory::Avatar | FormatCategory::CtrlA => {
                // Screen prep only
                options.format = FormatOptions::Ansi(icy_engine::AnsiFormatOptions {
                    screen_prep: self.screen_prep,
                    ..Default::default()
                });
            }
            FormatCategory::PCBoard => {
                // Screen prep + UTF-8
                options.format = FormatOptions::Ansi(icy_engine::AnsiFormatOptions {
                    level: if self.utf8_output {
                        AnsiCompatibilityLevel::Utf8Terminal
                    } else {
                        AnsiCompatibilityLevel::default()
                    },
                    screen_prep: self.screen_prep,
                    ..Default::default()
                });
            }
            // Binary, IcyDraw, Image, Other - no special options
            _ => {}
        }

        options
    }

    /// Handle a dialog message
    ///
    /// Returns the dialog result:
    /// - `StateResult::Success(path)` if export was successful
    /// - `StateResult::Close` if cancelled
    /// - `StateResult::None` to keep dialog open
    pub fn handle_message(&mut self, message: ExportDialogMessage) -> StateResult<PathBuf> {
        match message {
            ExportDialogMessage::Export => {
                // Update the actual values
                self.export_directory = self.temp_directory.clone();
                self.export_filename = self.temp_filename.clone();
                self.export_format = self.temp_format;

                // Perform the export
                match self.export_buffer() {
                    Ok(path) => {
                        log::info!("Successfully exported to: {}", path.display());
                        StateResult::Success(path)
                    }
                    Err(e) => {
                        log::error!("Export failed: {}", e);
                        // Keep dialog open on error
                        StateResult::None
                    }
                }
            }
            ExportDialogMessage::ChangeDirectory(dir) => {
                self.temp_directory = dir;
                StateResult::None
            }
            ExportDialogMessage::ChangeFileName(name) => {
                self.temp_filename = name;
                StateResult::None
            }
            ExportDialogMessage::ChangeFormat(format) => {
                self.temp_format = format;
                StateResult::None
            }
            ExportDialogMessage::ToggleSaveSauce(enabled) => {
                self.save_sauce = enabled;
                StateResult::None
            }
            ExportDialogMessage::SetAnsiLevel(level) => {
                self.ansi_level = level;
                StateResult::None
            }
            ExportDialogMessage::SetScreenPrep(prep) => {
                self.screen_prep = prep;
                StateResult::None
            }
            ExportDialogMessage::ToggleMaxLineLength(enabled) => {
                self.max_line_length_enabled = enabled;
                StateResult::None
            }
            ExportDialogMessage::SetMaxLineLength(value) => {
                if let Ok(len) = value.parse::<u16>() {
                    self.max_line_length = len.max(1).min(999);
                }
                StateResult::None
            }
            ExportDialogMessage::ToggleUtf8Output(enabled) => {
                self.utf8_output = enabled;
                StateResult::None
            }
            ExportDialogMessage::ToggleCompress(enabled) => {
                self.compress = enabled;
                StateResult::None
            }
            ExportDialogMessage::SetSixelMaxColors(value) => {
                if let Ok(colors) = value.parse::<u16>() {
                    self.sixel_settings.max_colors = colors.max(2).min(256);
                }
                StateResult::None
            }
            ExportDialogMessage::SetSixelDiffusion(value) => {
                if let Ok(diff) = value.parse::<f32>() {
                    self.sixel_settings.diffusion = diff.max(0.0).min(1.0);
                }
                StateResult::None
            }
            ExportDialogMessage::ToggleSixelKmeans(enabled) => {
                self.sixel_settings.use_kmeans = enabled;
                StateResult::None
            }
            ExportDialogMessage::BrowseDirectory => {
                let initial_dir = if Path::new(&self.temp_directory).exists() {
                    Some(PathBuf::from(&self.temp_directory))
                } else {
                    std::env::current_dir().ok()
                };

                let mut dialog = rfd::FileDialog::new();
                if let Some(dir) = initial_dir {
                    dialog = dialog.set_directory(dir);
                }

                if let Some(path) = dialog.pick_folder() {
                    if let Some(path_str) = path.to_str() {
                        self.temp_directory = path_str.to_string();
                    }
                }
                StateResult::None
            }
            ExportDialogMessage::RestoreDefaults => {
                if let Some(ref default_fn) = self.default_directory_fn {
                    let default_dir = default_fn();
                    if let Some(path_str) = default_dir.to_str() {
                        self.temp_directory = path_str.to_string();
                    }
                }
                StateResult::None
            }
            ExportDialogMessage::Cancel => StateResult::Close,
        }
    }

    /// Create the modal content for the dialog
    pub fn view<'a, Message: Clone + 'static>(&'a self, on_message: impl Fn(ExportDialogMessage) -> Message + 'a + Clone) -> Element<'a, Message> {
        // Check directory validity
        let dir_path = Path::new(&self.temp_directory);
        let dir_valid = !self.temp_directory.is_empty() && dir_path.exists();
        let dir_error = if self.temp_directory.is_empty() {
            None
        } else if !dir_path.exists() {
            Some(fl!(LANGUAGE_LOADER, "export-dialog-dir-not-exist"))
        } else if !dir_path.is_dir() {
            Some(fl!(LANGUAGE_LOADER, "export-dialog-not-directory"))
        } else {
            None
        };

        // === File Section ===
        let file_section = self.view_file_section(&on_message, &dir_error);

        // === Format Options Section ===
        let format_options_section = self.view_format_options_section(&on_message);

        // === SIXEL Section (only if buffer has sixels and format supports it) ===
        let sixel_section = self.view_sixel_section(&on_message);

        // === SAUCE Section ===
        let sauce_section = self.view_sauce_section(&on_message);

        // === Buttons ===
        let is_at_defaults = if let Some(ref default_fn) = self.default_directory_fn {
            let default_dir = default_fn();
            default_dir.to_str().map(|s| s == self.temp_directory).unwrap_or(true)
        } else {
            true
        };

        let restore_btn = restore_defaults_button(!is_at_defaults, on_message.clone()(ExportDialogMessage::RestoreDefaults));

        let export_enabled = !self.temp_directory.is_empty() && !self.temp_filename.is_empty() && dir_valid;

        let export_btn = primary_button(
            fl!(LANGUAGE_LOADER, "export-dialog-export-button"),
            if export_enabled {
                Some(on_message.clone()(ExportDialogMessage::Export))
            } else {
                None
            },
        );

        let cancel_btn = secondary_button(fl!(LANGUAGE_LOADER, "dialog-cancel-button"), Some(on_message(ExportDialogMessage::Cancel)));

        let buttons_left = vec![restore_btn.into()];
        let buttons_right = vec![cancel_btn.into(), export_btn.into()];
        let buttons = button_row_with_left(buttons_left, buttons_right);

        // === Main Layout ===
        let mut content_col = column![file_section].spacing(DIALOG_SPACING);

        if let Some(fmt_section) = format_options_section {
            content_col = content_col.push(Space::new().height(DIALOG_SPACING));
            content_col = content_col.push(fmt_section);
        }

        if let Some(sixel_sec) = sixel_section {
            content_col = content_col.push(Space::new().height(DIALOG_SPACING));
            content_col = content_col.push(sixel_sec);
        }

        if let Some(sauce_sec) = sauce_section {
            content_col = content_col.push(Space::new().height(DIALOG_SPACING));
            content_col = content_col.push(sauce_sec);
        }

        let scrollable_content = scrollable(content_col.padding(4)).height(Length::Fill);

        let dialog_content = dialog_area(scrollable_content.into());
        let button_area = dialog_area(buttons.into());

        let modal = modal_container(
            column![container(dialog_content).height(Length::Fill), separator(), button_area,].into(),
            DIALOG_WIDTH_LARGE,
        );

        container(modal)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    }

    /// View the file section (directory, filename, format, preview)
    fn view_file_section<'a, Message: Clone + 'static>(
        &'a self,
        on_message: &(impl Fn(ExportDialogMessage) -> Message + 'a + Clone),
        dir_error: &Option<String>,
    ) -> Element<'a, Message> {
        // Directory input with browse button
        let on_msg = on_message.clone();
        let dir_input = text_input("", &self.temp_directory)
            .on_input(move |s| on_msg(ExportDialogMessage::ChangeDirectory(s)))
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fill);

        let browse_btn = browse_button(on_message.clone()(ExportDialogMessage::BrowseDirectory));

        let dir_input_row = row![dir_input, Space::new().width(4.0), browse_btn].align_y(Alignment::Center);

        let dir_row = row![left_label_small(fl!(LANGUAGE_LOADER, "export-dialog-folder")), dir_input_row,]
            .spacing(DIALOG_SPACING)
            .align_y(Alignment::Center);

        // Filename input with format picker
        let on_msg = on_message.clone();
        let file_input = text_input("", &self.temp_filename)
            .on_input(move |s| on_msg(ExportDialogMessage::ChangeFileName(s)))
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fill);

        let on_msg = on_message.clone();
        let format_picker = pick_list(self.available_formats.clone(), Some(self.temp_format), move |format| {
            on_msg(ExportDialogMessage::ChangeFormat(format))
        })
        .padding(6)
        .width(Length::Fixed(160.0));

        let file_row = row![
            left_label_small(fl!(LANGUAGE_LOADER, "export-dialog-file")),
            file_input,
            Space::new().width(4.0),
            format_picker,
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center);

        // Warning/Preview area
        let warning_content: Element<'a, Message> = if let Some(error) = dir_error {
            let error_msg = error.clone();
            row![
                error_tooltip(error.clone()),
                Space::new().width(4.0),
                text(error_msg).size(TEXT_SIZE_SMALL).style(|theme: &icy_ui::Theme| icy_ui::widget::text::Style {
                    color: Some(theme.destructive.base),
                })
            ]
            .into()
        } else {
            row![text(format!("→ {}", self.get_temp_full_path().display()))
                .size(TEXT_SIZE_SMALL)
                .style(|theme: &icy_ui::Theme| icy_ui::widget::text::Style { color: Some(theme.button.on) })]
            .into()
        };

        let preview_row = row![Space::new().width(LABEL_SMALL_WIDTH + DIALOG_SPACING), warning_content];

        effect_box(
            column![
                dir_row,
                Space::new().height(DIALOG_SPACING),
                file_row,
                Space::new().height(DIALOG_SPACING),
                preview_row,
            ]
            .spacing(0)
            .into(),
        )
    }

    /// View the format-specific options section
    fn view_format_options_section<'a, Message: Clone + 'static>(
        &'a self,
        on_message: &(impl Fn(ExportDialogMessage) -> Message + 'a + Clone),
    ) -> Option<Element<'a, Message>> {
        match self.format_category() {
            FormatCategory::Ansi => Some(self.view_ansi_options(on_message)),
            FormatCategory::Avatar | FormatCategory::CtrlA => Some(self.view_screen_prep_only(on_message)),
            FormatCategory::PCBoard => Some(self.view_pcboard_options(on_message)),
            // Binary, IcyDraw, Image, Other - no options
            _ => None,
        }
    }

    /// View ANSI format options
    fn view_ansi_options<'a, Message: Clone + 'static>(&'a self, on_message: &(impl Fn(ExportDialogMessage) -> Message + 'a + Clone)) -> Element<'a, Message> {
        // Compatibility level picker
        let on_msg = on_message.clone();
        let level_picker = pick_list(AnsiCompatibilityLevel::all().to_vec(), Some(self.ansi_level), move |level| {
            on_msg(ExportDialogMessage::SetAnsiLevel(level))
        })
        .padding(6)
        .width(Length::Fixed(120.0));

        let level_row = row![left_label_small("Compatibility".to_string()), level_picker]
            .spacing(DIALOG_SPACING)
            .align_y(Alignment::Center);

        // Screen preparation picker
        let on_msg = on_message.clone();
        let prep_picker = pick_list(ScreenPreperation::all().to_vec(), Some(self.screen_prep), move |prep| {
            on_msg(ExportDialogMessage::SetScreenPrep(prep))
        })
        .padding(6)
        .width(Length::Fixed(120.0));

        let prep_row = row![left_label_small("Screen Prep".to_string()), prep_picker]
            .spacing(DIALOG_SPACING)
            .align_y(Alignment::Center);

        // Max line length (checkbox + input)
        let on_msg = on_message.clone();
        let line_length_checkbox = checkbox(self.max_line_length_enabled)
            .on_toggle(move |checked| on_msg(ExportDialogMessage::ToggleMaxLineLength(checked)))
            .size(18);

        let on_msg = on_message.clone();
        let line_length_input = text_input("80", &self.max_line_length.to_string())
            .on_input_maybe(if self.max_line_length_enabled {
                Some(move |s| on_msg(ExportDialogMessage::SetMaxLineLength(s)))
            } else {
                None
            })
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fixed(60.0));

        let line_length_row = row![
            left_label_small("Max Line Length".to_string()),
            line_length_checkbox,
            Space::new().width(8.0),
            line_length_input,
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center);

        effect_box(
            column![
                level_row,
                Space::new().height(DIALOG_SPACING),
                prep_row,
                Space::new().height(DIALOG_SPACING),
                line_length_row,
            ]
            .spacing(0)
            .into(),
        )
    }

    /// View screen prep only options (Avatar, CtrlA, Renegade)
    fn view_screen_prep_only<'a, Message: Clone + 'static>(
        &'a self,
        on_message: &(impl Fn(ExportDialogMessage) -> Message + 'a + Clone),
    ) -> Element<'a, Message> {
        // Screen preparation picker
        let on_msg = on_message.clone();
        let prep_picker = pick_list(ScreenPreperation::all().to_vec(), Some(self.screen_prep), move |prep| {
            on_msg(ExportDialogMessage::SetScreenPrep(prep))
        })
        .padding(6)
        .width(Length::Fixed(120.0));

        let prep_row = row![left_label_small("Screen Prep".to_string()), prep_picker]
            .spacing(DIALOG_SPACING)
            .align_y(Alignment::Center);

        effect_box(column![prep_row].spacing(0).into())
    }

    /// View PCBoard format options (screen prep + UTF-8)
    fn view_pcboard_options<'a, Message: Clone + 'static>(
        &'a self,
        on_message: &(impl Fn(ExportDialogMessage) -> Message + 'a + Clone),
    ) -> Element<'a, Message> {
        // Screen preparation picker
        let on_msg = on_message.clone();
        let prep_picker = pick_list(ScreenPreperation::all().to_vec(), Some(self.screen_prep), move |prep| {
            on_msg(ExportDialogMessage::SetScreenPrep(prep))
        })
        .padding(6)
        .width(Length::Fixed(120.0));

        let prep_row = row![left_label_small("Screen Prep".to_string()), prep_picker]
            .spacing(DIALOG_SPACING)
            .align_y(Alignment::Center);

        // UTF-8 output checkbox
        let on_msg = on_message.clone();
        let utf8_checkbox = checkbox(self.utf8_output)
            .on_toggle(move |checked| on_msg(ExportDialogMessage::ToggleUtf8Output(checked)))
            .size(18);

        let utf8_row = row![left_label_small("UTF-8".to_string()), utf8_checkbox]
            .spacing(DIALOG_SPACING)
            .align_y(Alignment::Center);

        effect_box(column![prep_row, Space::new().height(DIALOG_SPACING), utf8_row,].spacing(0).into())
    }

    /// View SIXEL options section
    fn view_sixel_section<'a, Message: Clone + 'static>(
        &'a self,
        on_message: &(impl Fn(ExportDialogMessage) -> Message + 'a + Clone),
    ) -> Option<Element<'a, Message>> {
        // Only show SIXEL options if buffer has sixels and format is ANSI
        if !self.has_sixels || self.format_category() != FormatCategory::Ansi {
            return None;
        }

        // Max colors input
        let on_msg = on_message.clone();
        let colors_input = text_input("256", &self.sixel_settings.max_colors.to_string())
            .on_input(move |s| on_msg(ExportDialogMessage::SetSixelMaxColors(s)))
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fixed(60.0));

        let colors_row = row![left_label_small("Max Colors".to_string()), colors_input]
            .spacing(DIALOG_SPACING)
            .align_y(Alignment::Center);

        // Diffusion input
        let on_msg = on_message.clone();
        let diffusion_input = text_input("0.875", &format!("{:.3}", self.sixel_settings.diffusion))
            .on_input(move |s| on_msg(ExportDialogMessage::SetSixelDiffusion(s)))
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fixed(60.0));

        let diffusion_row = row![left_label_small("Diffusion".to_string()), diffusion_input]
            .spacing(DIALOG_SPACING)
            .align_y(Alignment::Center);

        // K-means checkbox
        let on_msg = on_message.clone();
        let kmeans_checkbox = checkbox(self.sixel_settings.use_kmeans)
            .on_toggle(move |checked| on_msg(ExportDialogMessage::ToggleSixelKmeans(checked)))
            .size(18);

        let kmeans_row = row![left_label_small("Use K-means".to_string()), kmeans_checkbox]
            .spacing(DIALOG_SPACING)
            .align_y(Alignment::Center);

        Some(effect_box(
            column![
                text("SIXEL Settings").size(TEXT_SIZE_NORMAL),
                Space::new().height(DIALOG_SPACING),
                colors_row,
                Space::new().height(DIALOG_SPACING),
                diffusion_row,
                Space::new().height(DIALOG_SPACING),
                kmeans_row,
            ]
            .spacing(0)
            .into(),
        ))
    }

    /// View SAUCE metadata section
    fn view_sauce_section<'a, Message: Clone + 'static>(
        &'a self,
        on_message: &(impl Fn(ExportDialogMessage) -> Message + 'a + Clone),
    ) -> Option<Element<'a, Message>> {
        // Only show SAUCE section if we have metadata
        if self.sauce_metadata.is_none() {
            return None;
        }

        let on_msg = on_message.clone();
        let save_sauce_checkbox = checkbox(self.save_sauce)
            .on_toggle(move |checked| on_msg(ExportDialogMessage::ToggleSaveSauce(checked)))
            .size(18);

        let sauce_row = row![left_label_small("Save SAUCE".to_string()), save_sauce_checkbox]
            .spacing(DIALOG_SPACING)
            .align_y(Alignment::Center);

        Some(effect_box(column![sauce_row].spacing(0).into()))
    }
}

// ============================================================================
// Builder function for export dialog
// ============================================================================

/// Create an export dialog for saving a screen buffer.
///
/// # Example
/// ```ignore
/// dialog_stack.push(
///     export_dialog(
///         "./output.ans",
///         BufferType::Unicode,
///         screen.clone(),
///         Message::ExportDialog,
///         |msg| match msg { Message::ExportDialog(m) => Some(m), _ => None },
///     )
///     .on_confirm(|path| Message::ExportComplete(path))
///     .on_cancel(|| Message::CloseExportDialog)
/// );
/// ```
pub fn export_dialog<M, F, E>(
    initial_path: impl Into<String>,
    buffer_type: BufferType,
    screen: Arc<Mutex<Box<dyn Screen>>>,
    on_message: F,
    extract_message: E,
) -> ExportDialogWrapper<M, F, E>
where
    M: Clone + Send + 'static,
    F: Fn(ExportDialogMessage) -> M + Clone + 'static,
    E: Fn(&M) -> Option<&ExportDialogMessage> + Clone + 'static,
{
    ExportDialogWrapper::new(ExportDialogState::new(initial_path.into(), buffer_type, screen), on_message, extract_message)
}

/// Create an export dialog with a default directory provider.
///
/// # Example
/// ```ignore
/// dialog_stack.push(
///     export_dialog_with_defaults(
///         "./output.ans",
///         BufferType::Unicode,
///         screen.clone(),
///         || default_export_dir(),
///         Message::ExportDialog,
///         |msg| match msg { Message::ExportDialog(m) => Some(m), _ => None },
///     )
///     .on_confirm(|path| Message::ExportComplete(path))
/// );
/// ```
pub fn export_dialog_with_defaults<M, F, D, E>(
    initial_path: impl Into<String>,
    buffer_type: BufferType,
    screen: Arc<Mutex<Box<dyn Screen>>>,
    default_dir_fn: D,
    on_message: F,
    extract_message: E,
) -> ExportDialogWrapper<M, F, E>
where
    M: Clone + Send + 'static,
    F: Fn(ExportDialogMessage) -> M + Clone + 'static,
    D: Fn() -> PathBuf + Send + Sync + 'static,
    E: Fn(&M) -> Option<&ExportDialogMessage> + Clone + 'static,
{
    ExportDialogWrapper::new(
        ExportDialogState::new(initial_path.into(), buffer_type, screen).with_default_directory_fn(default_dir_fn),
        on_message,
        extract_message,
    )
}

/// Create an export dialog with a default directory provider using the `dialog_msg!` macro.
///
/// # Example
/// ```ignore
/// use icy_engine_gui::dialog_msg;
/// dialog_stack.push(
///     export_dialog_with_defaults_from_msg(
///         "./output.ans",
///         BufferType::Unicode,
///         screen.clone(),
///         || default_export_dir(),
///         dialog_msg!(Message::ExportDialog),
///     )
///     .on_confirm(|path| Message::ExportComplete(path))
/// );
/// ```
pub fn export_dialog_with_defaults_from_msg<M, F, D, E>(
    initial_path: impl Into<String>,
    buffer_type: BufferType,
    screen: Arc<Mutex<Box<dyn Screen>>>,
    default_dir_fn: D,
    msg_tuple: (F, E),
) -> ExportDialogWrapper<M, F, E>
where
    M: Clone + Send + 'static,
    F: Fn(ExportDialogMessage) -> M + Clone + 'static,
    D: Fn() -> PathBuf + Send + Sync + 'static,
    E: Fn(&M) -> Option<&ExportDialogMessage> + Clone + 'static,
{
    ExportDialogWrapper::new(
        ExportDialogState::new(initial_path.into(), buffer_type, screen).with_default_directory_fn(default_dir_fn),
        msg_tuple.0,
        msg_tuple.1,
    )
}
