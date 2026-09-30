//! Export dialog shared by the icy tools: the format, the destination and the options of the
//! chosen format. The dialog only collects an [`ExportRequest`]; the application writes it
//! (usually with [`ExportRequest::write_screen`]) and reports failures with
//! [`ExportDialog::set_error`], which keeps the dialog open.

use std::{
    io::Write,
    path::{Path, PathBuf},
};

use i18n_embed_fl::fl;
use icy_engine::{
    formats::FileFormat, AnsiCompatibilityLevel, ControlCharHandling, LineBreakBehavior, LineEnding, SauceMetaData, SaveOptions, Screen, ScreenPreperation,
};

use super::{
    appearance,
    dialog::{labels, Dialog, DialogButton, DialogSize},
};
use crate::{ExportOptionKind, ExportSettings, LANGUAGE_LOADER};

/// What the user confirmed in the [`ExportDialog`].
#[derive(Clone, Debug)]
pub struct ExportRequest {
    pub path: PathBuf,
    pub format: FileFormat,
    /// Options of the format, including the SAUCE record when the user keeps it.
    pub options: SaveOptions,
}

impl ExportRequest {
    /// Writes `screen` in the requested format.
    pub fn write_screen(&self, screen: &mut dyn Screen) -> Result<(), String> {
        self.write_with(|path| match self.format {
            FileFormat::Image(image) => image.save_screen(&*screen, path).map_err(|error| error.to_string()),
            format => {
                let data = screen.to_bytes(format.primary_extension(), &self.options).map_err(|error| error.to_string())?;
                std::fs::write(path, data).map_err(|error| error.to_string())
            }
        })
    }

    /// Lets `write` fill a temporary file beside the target, which then replaces the target, so a
    /// failed export never leaves a truncated file behind. Links to the target are followed.
    pub fn write_with(&self, write: impl FnOnce(&Path) -> Result<(), String>) -> Result<(), String> {
        let target = if self.path.exists() {
            self.path.canonicalize().map_err(|error| error.to_string())?
        } else {
            self.path.clone()
        };
        let parent = target.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or(Path::new("."));
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        let mut temporary = tempfile::Builder::new()
            .prefix(".icy-export-")
            .suffix(&format!(".{}", self.format.primary_extension()))
            .tempfile_in(parent)
            .map_err(|error| error.to_string())?;
        write(temporary.path())?;
        temporary.flush().map_err(|error| error.to_string())?;
        if let Ok(metadata) = std::fs::metadata(&target) {
            temporary.as_file().set_permissions(metadata.permissions()).map_err(|error| error.to_string())?;
        }
        temporary.as_file().sync_all().map_err(|error| error.to_string())?;
        temporary.persist(&target).map_err(|error| error.error.to_string())?;
        Ok(())
    }
}

pub enum ExportAction {
    Cancel,
    Export(ExportRequest),
}

pub struct ExportDialog {
    title: Option<String>,
    formats: Vec<FileFormat>,
    format: FileFormat,
    directory: String,
    file_name: String,
    sauce: Option<SauceMetaData>,
    save_sauce: bool,
    sixels: bool,
    settings: ExportSettings,
    /// The existing file the user was asked about; a second click overwrites it.
    confirmed: Option<PathBuf>,
    error: Option<String>,
    /// A format written with another of its extensions, such as XEP80 ATASCII as .xep.
    extension: Option<(FileFormat, &'static str)>,
}

impl ExportDialog {
    /// `formats` in the order the picker lists them, the first one is preselected. `name` is the
    /// suggested file name; a format extension is removed because the dialog appends it.
    pub fn new(formats: Vec<FileFormat>, directory: impl AsRef<Path>, name: &str) -> Self {
        let formats = if formats.is_empty() { vec![FileFormat::Ansi] } else { formats };
        let directory = directory.as_ref();
        let directory = if directory.as_os_str().is_empty() {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        } else {
            directory.to_path_buf()
        };
        let mut dialog = Self {
            title: None,
            format: formats[0],
            formats,
            directory: directory.to_string_lossy().into_owned(),
            file_name: String::new(),
            sauce: None,
            save_sauce: true,
            sixels: true,
            settings: ExportSettings::default(),
            confirmed: None,
            error: None,
            extension: None,
        };
        dialog.file_name = dialog.stem(name);
        if dialog.file_name.is_empty() {
            dialog.file_name = "export".into();
        }
        dialog
    }

    /// Without a title the dialog is headerless like the other shared dialogs; the export button
    /// already names the action.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Restores persisted options, including the last used format when it is offered.
    pub fn with_settings(mut self, settings: &ExportSettings) -> Self {
        self.settings = settings.clone();
        if let Some(format) = settings.format_in(&self.formats) {
            self.format = format;
        }
        self
    }

    pub fn with_format(mut self, format: FileFormat) -> Self {
        if self.formats.contains(&format) {
            self.format = format;
        }
        self
    }

    /// Writes `format` with `extension` instead of its primary one.
    pub fn with_extension(mut self, format: FileFormat, extension: &'static str) -> Self {
        self.extension = Some((format, extension));
        self
    }

    /// The extension of the chosen format's files.
    fn extension(&self) -> &str {
        match self.extension {
            Some((format, extension)) if format == self.format => extension,
            _ => self.format.primary_extension(),
        }
    }

    /// The SAUCE record to store; without one the SAUCE option is hidden.
    pub fn with_sauce(mut self, sauce: Option<SauceMetaData>) -> Self {
        self.sauce = sauce;
        self
    }

    /// Whether the picture contains sixel images, which shows their encoding options.
    pub fn with_sixels(mut self, sixels: bool) -> Self {
        self.sixels = sixels;
        self
    }

    /// The options to persist, with the chosen format.
    pub fn settings(&self) -> ExportSettings {
        ExportSettings {
            export_format_ext: Some(self.format.primary_extension().to_owned()),
            ..self.settings.clone()
        }
    }

    pub fn directory(&self) -> &str {
        &self.directory
    }

    pub fn format(&self) -> FileFormat {
        self.format
    }

    /// The file that gets written; a typed format extension is not doubled.
    pub fn target(&self) -> PathBuf {
        Path::new(&self.directory).join(format!("{}.{}", self.stem(&self.file_name), self.extension()))
    }

    /// Shows a failed export in the dialog.
    pub fn set_error(&mut self, error: impl Into<String>) {
        self.error = Some(error.into());
    }

    fn stem(&self, name: &str) -> String {
        let path = Path::new(name.trim());
        let known = path
            .extension()
            .and_then(|extension| extension.to_str())
            .and_then(|extension| FileFormat::from_extension(&extension.to_ascii_lowercase()))
            .is_some_and(|format| self.formats.contains(&format));
        let name = if known { path.file_stem() } else { path.file_name() };
        name.map(|name| name.to_string_lossy().into_owned()).unwrap_or_default()
    }

    /// The request for the current input, or why it cannot be exported.
    pub fn request(&self) -> Result<ExportRequest, String> {
        let typed = self.file_name.trim();
        let name = self.stem(typed);
        if name.is_empty() || Path::new(typed).file_name() != Some(std::ffi::OsStr::new(typed)) || typed.contains(['/', '\\']) {
            return Err(fl!(LANGUAGE_LOADER, "export-invalid-name"));
        }
        let directory = Path::new(&self.directory);
        if directory.exists() && !directory.is_dir() {
            return Err(fl!(LANGUAGE_LOADER, "export-not-a-folder"));
        }
        let mut options = self.settings.save_options(self.format);
        if self.save_sauce && ExportOptionKind::of(self.format).stores_sauce() {
            options.sauce = self.sauce.clone();
        }
        Ok(ExportRequest {
            path: self.target(),
            format: self.format,
            options,
        })
    }

    /// The export button: asks once before an existing file is replaced.
    fn confirm(&mut self) -> Option<ExportAction> {
        let request = match self.request() {
            Ok(request) => request,
            Err(error) => {
                self.error = Some(error);
                return None;
            }
        };
        if request.path.exists() && self.confirmed.as_ref() != Some(&request.path) {
            self.confirmed = Some(request.path);
            return None;
        }
        self.error = None;
        Some(ExportAction::Export(request))
    }

    /// Returns what the user decided; `None` while the dialog stays open.
    pub fn show(&mut self, context: &egui::Context) -> Option<ExportAction> {
        #[derive(Clone, Copy)]
        enum Button {
            Cancel,
            Export,
        }
        let before = (self.target(), self.format);
        let mut dialog = Dialog::new("icy-export-dialog");
        if let Some(title) = &self.title {
            dialog = dialog.title(title.clone());
        }
        let response = dialog.size(DialogSize::Medium).confirm_on_enter(true).show(context, |dialog| {
            dialog.content(|ui| {
                self.fields(ui);
                if self.confirmed.is_some() {
                    ui.add_space(4.0);
                    ui.colored_label(ui.visuals().warn_fg_color, fl!(LANGUAGE_LOADER, "export-overwrite-question"));
                }
                if let Some(error) = &self.error {
                    ui.add_space(4.0);
                    ui.add(egui::Label::new(egui::RichText::new(error).color(ui.visuals().error_fg_color)).wrap());
                }
            });
            let export = if self.confirmed.is_some() {
                DialogButton::destructive(labels::overwrite(), Button::Export)
            } else {
                DialogButton::primary(labels::export(), Button::Export)
            };
            dialog.buttons([
                DialogButton::cancel(labels::cancel(), Button::Cancel),
                export.enabled(!self.file_name.trim().is_empty()),
            ]);
        });
        if before != (self.target(), self.format) {
            self.confirmed = None;
            self.error = None;
        }
        match response.action {
            Some(Button::Cancel) => Some(ExportAction::Cancel),
            Some(Button::Export) => self.confirm(),
            None if response.dismissed => Some(ExportAction::Cancel),
            None => None,
        }
    }

    fn fields(&mut self, ui: &mut egui::Ui) {
        appearance::group(ui, &fl!(LANGUAGE_LOADER, "export-section-file"), |ui| {
            appearance::combo_row(ui, &fl!(LANGUAGE_LOADER, "export-format"), format_label(self.format), |ui| {
                for &format in &self.formats {
                    ui.selectable_value(&mut self.format, format, format_label(format));
                }
            });
            appearance::form_row(ui, &fl!(LANGUAGE_LOADER, "export-folder"), |ui| {
                trailing_row(ui, |ui| {
                    let browse = ui.button("…").on_hover_text(fl!(LANGUAGE_LOADER, "export-browse"));
                    if browse.clicked() && !ui.ctx().will_discard() {
                        if let Some(path) = rfd::FileDialog::new().set_directory(&self.directory).pick_folder() {
                            self.directory = path.to_string_lossy().into_owned();
                        }
                    }
                    ui.add(appearance::text_edit(&mut self.directory).desired_width(f32::INFINITY));
                });
            });
            appearance::form_row(ui, &fl!(LANGUAGE_LOADER, "export-file-name"), |ui| {
                trailing_row(ui, |ui| {
                    ui.label(egui::RichText::new(format!(".{}", self.extension())).weak());
                    ui.add(appearance::text_edit(&mut self.file_name).desired_width(f32::INFINITY));
                });
            });
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.label(egui::RichText::new("→").weak().size(12.0));
                ui.add(
                    egui::Label::new(egui::RichText::new(self.target().display().to_string()).weak().size(12.0))
                        .wrap()
                        .selectable(true),
                );
            });
        });
        let kind = ExportOptionKind::of(self.format);
        let settings = &mut self.settings;
        if kind != ExportOptionKind::Image {
            appearance::group(ui, &fl!(LANGUAGE_LOADER, "export-section-options"), |ui| {
                if self.sauce.is_some() {
                    appearance::check_row(ui, &fl!(LANGUAGE_LOADER, "export-save-sauce"), &mut self.save_sauce);
                }
                if kind.is_text() {
                    appearance::check_row(ui, &fl!(LANGUAGE_LOADER, "export-optimize-colors"), &mut settings.optimize_colors);
                    appearance::check_row(ui, &fl!(LANGUAGE_LOADER, "export-normalize-whitespace"), &mut settings.normalize_whitespace);
                }
                if kind == ExportOptionKind::Character {
                    screen_preparation(ui, &mut settings.screen_prep);
                }
                if kind == ExportOptionKind::Ascii || self.format == FileFormat::PCBoard {
                    appearance::check_row(ui, &fl!(LANGUAGE_LOADER, "export-utf8"), &mut settings.utf8_output);
                }
                if matches!(kind, ExportOptionKind::Compressed | ExportOptionKind::IcyDraw) {
                    appearance::check_row(ui, &fl!(LANGUAGE_LOADER, "export-compress"), &mut settings.compress);
                }
            });
        }
        if kind == ExportOptionKind::Ansi {
            appearance::group(ui, "ANSI", |ui| ansi_fields(ui, settings));
            if self.sixels && settings.ansi_level.supports_sixel() {
                appearance::group(ui, "Sixel", |ui| {
                    let sixel = &mut settings.sixel_settings;
                    appearance::form_row(ui, &fl!(LANGUAGE_LOADER, "export-sixel-colors"), |ui| {
                        ui.add(egui::DragValue::new(&mut sixel.max_colors).range(2..=256));
                    });
                    appearance::slider_row(ui, &fl!(LANGUAGE_LOADER, "export-sixel-diffusion"), &mut sixel.diffusion, 0.0..=1.0);
                    appearance::check_row(ui, &fl!(LANGUAGE_LOADER, "export-sixel-kmeans"), &mut sixel.use_kmeans);
                });
            }
        }
    }
}

/// A text field filling the row with a control after it. Only one line high, also when
/// [`appearance::form_row`] stacks the caption above it.
fn trailing_row(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    let size = egui::vec2(ui.available_width(), ui.spacing().interact_size.y.max(28.0));
    ui.allocate_ui_with_layout(size, egui::Layout::right_to_left(egui::Align::Center), add);
}

fn format_label(format: FileFormat) -> String {
    format!("{} (.{})", format.name(), format.primary_extension())
}

fn screen_preparation(ui: &mut egui::Ui, preparation: &mut ScreenPreperation) {
    let label = |value: ScreenPreperation| match value {
        ScreenPreperation::None => fl!(LANGUAGE_LOADER, "export-prep-none"),
        ScreenPreperation::ClearScreen => fl!(LANGUAGE_LOADER, "export-prep-clear"),
        ScreenPreperation::Home => fl!(LANGUAGE_LOADER, "export-prep-home"),
    };
    appearance::combo_row(ui, &fl!(LANGUAGE_LOADER, "export-screen-preparation"), label(*preparation), |ui| {
        for value in [ScreenPreperation::None, ScreenPreperation::ClearScreen, ScreenPreperation::Home] {
            ui.selectable_value(preparation, value, label(value));
        }
    });
}

fn ansi_fields(ui: &mut egui::Ui, settings: &mut ExportSettings) {
    appearance::combo_row(ui, &fl!(LANGUAGE_LOADER, "export-compatibility"), settings.ansi_level.to_string(), |ui| {
        for &level in AnsiCompatibilityLevel::all() {
            ui.selectable_value(&mut settings.ansi_level, level, level.to_string());
        }
    });
    ui.add_enabled_ui(settings.ansi_level.supports_truecolor(), |ui| {
        appearance::check_row(ui, &fl!(LANGUAGE_LOADER, "export-truecolor"), &mut settings.ansi_rgb_output);
    });
    screen_preparation(ui, &mut settings.screen_prep);

    let lengths = [
        fl!(LANGUAGE_LOADER, "export-line-length-default"),
        fl!(LANGUAGE_LOADER, "export-line-length-minimum"),
        fl!(LANGUAGE_LOADER, "export-line-length-maximum"),
    ];
    let mut length = match (settings.max_line_length_enabled, settings.line_length_minimum) {
        (false, _) => 0,
        (true, true) => 1,
        (true, false) => 2,
    };
    appearance::combo_row(ui, &fl!(LANGUAGE_LOADER, "export-line-length"), lengths[length].clone(), |ui| {
        for (index, label) in lengths.iter().enumerate() {
            ui.selectable_value(&mut length, index, label);
        }
    });
    settings.max_line_length_enabled = length != 0;
    settings.line_length_minimum = length == 1;
    if settings.max_line_length_enabled {
        appearance::form_row(ui, &fl!(LANGUAGE_LOADER, "export-columns"), |ui| {
            ui.add(egui::DragValue::new(&mut settings.max_line_length).range(1..=u16::MAX));
        });
    }

    let breaks = [
        (LineBreakBehavior::Wrap, fl!(LANGUAGE_LOADER, "export-line-break-wrap")),
        (LineBreakBehavior::Force, fl!(LANGUAGE_LOADER, "export-line-break-force")),
        (LineBreakBehavior::GotoXY, fl!(LANGUAGE_LOADER, "export-line-break-gotoxy")),
    ];
    let index = |value: &LineBreakBehavior| match value {
        LineBreakBehavior::Wrap => 0,
        LineBreakBehavior::Force => 1,
        LineBreakBehavior::GotoXY => 2,
    };
    let mut line_break = index(&settings.line_break);
    appearance::combo_row(ui, &fl!(LANGUAGE_LOADER, "export-line-break"), breaks[line_break].1.clone(), |ui| {
        for (value, label) in &breaks {
            ui.selectable_value(&mut line_break, index(value), label);
        }
    });
    settings.line_break = breaks[line_break].0.clone();

    appearance::combo_row(
        ui,
        &fl!(LANGUAGE_LOADER, "export-line-ending"),
        match settings.line_ending {
            LineEnding::Lf => "LF",
            LineEnding::CrLf => "CR LF",
        },
        |ui| {
            ui.selectable_value(&mut settings.line_ending, LineEnding::Lf, "LF");
            ui.selectable_value(&mut settings.line_ending, LineEnding::CrLf, "CR LF");
        },
    );

    let controls = [
        (ControlCharHandling::Ignore, fl!(LANGUAGE_LOADER, "export-controls-keep")),
        (ControlCharHandling::FilterOut, fl!(LANGUAGE_LOADER, "export-controls-filter")),
        (ControlCharHandling::IcyTerm, fl!(LANGUAGE_LOADER, "export-controls-escape")),
    ];
    let selected = controls
        .iter()
        .find(|(value, _)| *value == settings.control_char_handling)
        .map_or_else(String::new, |(_, label)| label.clone());
    appearance::combo_row(ui, &fl!(LANGUAGE_LOADER, "export-control-chars"), selected, |ui| {
        for (value, label) in &controls {
            ui.selectable_value(&mut settings.control_char_handling, *value, label);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use icy_engine::{formats::ImageFormat, TextScreen};

    fn screen() -> TextScreen {
        let mut screen = TextScreen::new((8, 2));
        for (x, ch) in "HELLO".chars().enumerate() {
            screen.buffer.layers[0].set_char((x as i32, 0), icy_engine::AttributedChar::new(ch, icy_engine::TextAttribute::default()));
        }
        screen
    }

    #[test]
    fn target_appends_the_format_extension_and_rejects_folders_in_the_name() {
        let directory = tempfile::tempdir().unwrap();
        let mut dialog = ExportDialog::new(vec![FileFormat::Ansi, FileFormat::XBin], directory.path(), "art.ans");
        assert_eq!(dialog.target(), directory.path().join("art.ans"));
        dialog.format = FileFormat::XBin;
        assert_eq!(dialog.target(), directory.path().join("art.xb"));
        assert_eq!(dialog.stem("picture.ans"), "picture");
        assert_eq!(dialog.stem("notes.txt"), "notes.txt", "unknown extensions belong to the name");
        dialog.file_name = "../outside".into();
        assert!(dialog.request().is_err());
        dialog.file_name = "  ".into();
        assert!(dialog.request().is_err());
    }

    #[test]
    fn a_format_can_be_written_with_another_of_its_extensions() {
        let directory = tempfile::tempdir().unwrap();
        let mut dialog =
            ExportDialog::new(vec![FileFormat::Atascii, FileFormat::IcyDraw], directory.path(), "art.xep").with_extension(FileFormat::Atascii, "xep");
        assert_eq!(dialog.target(), directory.path().join("art.xep"));
        dialog.format = FileFormat::IcyDraw;
        assert_eq!(dialog.target(), directory.path().join("art.icy"), "other formats keep their extension");
    }

    #[test]
    fn settings_restore_the_format_and_sauce_follows_the_checkbox() {
        let settings = ExportSettings {
            export_format_ext: Some("xb".into()),
            compress: true,
            ..Default::default()
        };
        let sauce = SauceMetaData {
            title: "Title".into(),
            ..Default::default()
        };
        let mut dialog = ExportDialog::new(vec![FileFormat::Ansi, FileFormat::XBin], "", "art")
            .with_settings(&settings)
            .with_sauce(Some(sauce));
        assert_eq!(dialog.format(), FileFormat::XBin);
        let request = dialog.request().unwrap();
        assert!(request.options.compressed_options().compress);
        assert!(request.options.sauce.is_some());
        dialog.save_sauce = false;
        assert!(dialog.request().unwrap().options.sauce.is_none());
        dialog.format = FileFormat::Image(ImageFormat::Png);
        dialog.save_sauce = true;
        assert!(dialog.request().unwrap().options.sauce.is_none(), "images have no SAUCE record");
        assert_eq!(dialog.settings().export_format_ext.as_deref(), Some("png"));
    }

    #[test]
    fn existing_files_need_a_second_confirmation() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("art.ans"), b"OLD").unwrap();
        let mut dialog = ExportDialog::new(vec![FileFormat::Ansi], directory.path(), "art");
        assert!(dialog.confirm().is_none());
        assert!(dialog.confirmed.is_some());
        assert!(matches!(dialog.confirm(), Some(ExportAction::Export(_))));
    }

    #[test]
    fn writing_replaces_the_file_only_after_success() {
        let directory = tempfile::tempdir().unwrap();
        let dialog = ExportDialog::new(vec![FileFormat::Ansi, FileFormat::Image(ImageFormat::Png)], directory.path(), "art");
        let request = dialog.request().unwrap();
        std::fs::write(&request.path, b"OLD").unwrap();
        assert!(request.write_with(|_| Err("failed".into())).is_err());
        assert_eq!(std::fs::read(&request.path).unwrap(), b"OLD");

        let mut screen = screen();
        request.write_screen(&mut screen).unwrap();
        let data = std::fs::read(&request.path).unwrap();
        assert!(data.windows(5).any(|window| window == b"HELLO"), "{data:?}");

        let image = dialog.with_format(FileFormat::Image(ImageFormat::Png)).request().unwrap();
        image.write_screen(&mut screen).unwrap();
        assert!(std::fs::read(&image.path).unwrap().starts_with(b"\x89PNG"));
        let leftovers = std::fs::read_dir(directory.path())
            .unwrap()
            .filter(|entry| entry.as_ref().unwrap().file_name().to_string_lossy().starts_with(".icy-export-"))
            .count();
        assert_eq!(leftovers, 0);
    }
}
