use super::{preview::Preview, text};
use eframe::egui;
use icy_engine::formats::{FileFormat, ImageFormat};
use icy_engine_gui::egui::{appearance, monitor, screen::ScreenView, shortcuts};
use icy_view::Options;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Mode {
    Settings,
    About,
    Help,
    Sauce,
    Export,
}

pub const COMMANDS: &[&str] = &[
    "file.open",
    "dialog.export",
    "dialog.filter",
    "edit.copy",
    "edit.select_all",
    "nav.back",
    "nav.forward",
    "nav.up",
    "view.zoom_in",
    "view.zoom_out",
    "view.zoom_reset",
    "view.zoom_fit",
    "view.fullscreen",
    "playback.toggle_scroll",
    "playback.scroll_speed",
    "playback.scroll_speed_back",
    "playback.baud_rate",
    "playback.baud_rate_back",
    "playback.baud_rate_off",
    "dialog.sauce",
    "external.command_0",
    "external.command_1",
    "external.command_2",
    "external.command_3",
    "settings.open",
    "help.show",
    "help.about",
    "window.new",
    "window.close",
    "app.quit",
];

pub struct Dialogs {
    pub mode: Option<Mode>,
    pub error: Option<String>,
    draft: Options,
    baseline: Option<Vec<u8>>,
    page: usize,
    about: Option<ScreenView>,
    raw: bool,
    export: Export,
}

impl Default for Dialogs {
    fn default() -> Self {
        Self {
            mode: None,
            error: None,
            draft: Options::default(),
            baseline: None,
            page: 0,
            about: None,
            raw: false,
            export: Export::default(),
        }
    }
}

impl Dialogs {
    pub fn open(&mut self, mode: Mode, options: &Options, preview: &Preview) {
        self.mode = Some(mode);
        self.error = None;
        if mode == Mode::Settings {
            self.draft = options.clone();
            self.baseline = std::fs::read(icy_view::get_config_dir().join("options.toml")).ok();
        }
        if mode == Mode::Export {
            self.export = Export::new(options, preview);
        }
        if mode == Mode::About && self.about.is_none() {
            match FileFormat::IcyDraw.from_bytes(include_bytes!("../../../data/about.icy"), None) {
                Ok(mut document) => {
                    icy_engine_gui::version_helper::replace_version_marker(
                        &mut document.screen.buffer,
                        &icy_view::VERSION,
                        option_env!("ICY_BUILD_DATE").map(String::from),
                    );
                    document.screen.caret.visible = false;
                    self.about = Some(ScreenView::new(document.screen));
                }
                Err(error) => self.error = Some(error.to_string()),
            }
        }
    }

    pub fn show(&mut self, context: &egui::Context, options: &mut Options, preview: &mut Preview) {
        let Some(mode) = self.mode else {
            return;
        };
        let title = match mode {
            Mode::Settings => text("settings-heading"),
            Mode::About => "Icy View".into(),
            Mode::Help => text("help-title"),
            Mode::Sauce => text("sauce-dialog-title"),
            Mode::Export => text("cmd-file-export-action"),
        };
        let mut close = context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        let response = appearance::Dialog::new("viewer-dialog", title)
            .max_width(740.0)
            .scroll(false)
            .show(context, |dialog| {
                dialog.content(|ui| {
                    let height = (context.content_rect().height() - 190.0).clamp(48.0, 440.0);
                    ui.scope(|ui| {
                        ui.set_height(height + if mode == Mode::Settings { 36.0 } else { 0.0 });
                        if mode == Mode::About {
                            if let Some(about) = &mut self.about {
                                let response = about.show(ui, &icy_engine_gui::MonitorSettings::neutral());
                                if let Some(url) = super::link_at(&about.terminal, response.hover_pos()) {
                                    response.clone().on_hover_cursor(egui::CursorIcon::PointingHand);
                                    if response.clicked() {
                                        context.open_url(egui::OpenUrl::new_tab(url));
                                    }
                                }
                            }
                        } else {
                            if mode == Mode::Settings {
                                ui.horizontal(|ui| {
                                    for (index, key) in ["settings-monitor-category", "settings-commands-category", "settings-paths-category"]
                                        .iter()
                                        .enumerate()
                                    {
                                        appearance::tab(ui, &mut self.page, index, &text(key));
                                    }
                                });
                            }
                            egui::ScrollArea::vertical()
                                .id_salt(("dialog-body", self.page, mode as u8))
                                .auto_shrink([false, false])
                                .min_scrolled_height(0.0)
                                .max_height(height)
                                .show(ui, |ui| {
                                    ui.set_height(height);
                                    match mode {
                                        Mode::Settings => self.settings_fields(ui),
                                        Mode::Help => help(ui),
                                        Mode::Sauce => sauce(ui, preview, &mut self.raw),
                                        Mode::Export => self.export.fields(ui),
                                        Mode::About => {}
                                    }
                                });
                        }
                    });
                    if let Some(error) = &self.error {
                        ui.add(egui::Label::new(egui::RichText::new(error).color(ui.visuals().error_fg_color)).wrap());
                    }
                });
                dialog.actions(|ui| {
                    if matches!(mode, Mode::Settings | Mode::Export) {
                        if ui
                            .add(appearance::primary_button(text(if self.export.confirmed.is_some() && mode == Mode::Export {
                                "egui-overwrite"
                            } else {
                                "egui-save"
                            })))
                            .clicked()
                            && !context.will_discard()
                        {
                            let result = if mode == Mode::Settings {
                                self.save_settings().map(|()| {
                                    *options = self.draft.clone();
                                    true
                                })
                            } else {
                                self.export.save(preview)
                            };
                            match result {
                                Ok(saved) => close |= saved,
                                Err(error) => self.error = Some(error.to_string()),
                            }
                        }
                        close |= ui.button(text("button-cancel")).clicked();
                        if mode == Mode::Settings && self.page == 0 && ui.button(text("settings-reset_button")).clicked() {
                            self.draft.monitor_settings = icy_engine_gui::MonitorSettings::default();
                        }
                    } else {
                        close |= ui.add(appearance::primary_button(text("dialog-close-button"))).clicked();
                    }
                });
            });
        if close || response.closed {
            self.mode = None;
        }
    }

    fn settings_fields(&mut self, ui: &mut egui::Ui) {
        match self.page {
            0 => monitor::fields(ui, &mut self.draft.monitor_settings, text),
            1 => {
                appearance::section(ui, &text("settings-commands-section"));
                for (index, command) in self.draft.external_commands.iter_mut().enumerate() {
                    appearance::form_row(ui, &format!("F{}", index + 5), |ui| {
                        ui.add_sized([ui.available_width(), 30.0], appearance::text_edit(&mut command.command));
                    });
                }
            }
            _ => {
                appearance::section(ui, &text("settings-paths-header"));
                for (key, path) in [
                    ("settings-paths-config-dir", icy_view::get_config_dir().to_path_buf()),
                    ("settings-paths-config-file", icy_view::get_config_dir().join("options.toml")),
                    ("settings-paths-log-file", Options::get_log_file()),
                ] {
                    appearance::form_row(ui, &text(key), |ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(text("settings-paths-open")).clicked() {
                                if let Err(error) = open::that(&path) {
                                    self.error = Some(error.to_string());
                                }
                            }
                            ui.add(egui::Label::new(path.to_string_lossy()).truncate())
                                .on_hover_text(path.display().to_string());
                        });
                    });
                }
                appearance::section(ui, &text("settings-paths-user-header"));
                directory_row(ui, &mut self.draft.export_path);
            }
        }
    }

    fn save_settings(&self) -> anyhow::Result<()> {
        let path = icy_view::get_config_dir().join("options.toml");
        save_settings(&path, self.baseline.as_deref(), &self.draft)
    }
}

fn save_settings(path: &Path, baseline: Option<&[u8]>, draft: &Options) -> anyhow::Result<()> {
    anyhow::ensure!(std::fs::read(path).ok().as_deref() == baseline, "{}", text("egui-settings-conflict"));
    let mut value = if let Some(bytes) = baseline {
        toml::from_str::<toml::Value>(std::str::from_utf8(bytes)?)?
    } else {
        toml::Value::Table(Default::default())
    };
    merge(&mut value, toml::Value::try_from(draft)?);
    let temporary = path.with_extension(format!("toml.{}.new", fastrand::u64(..)));
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&temporary)?;
    let result = (|| -> anyhow::Result<()> {
        file.write_all(toml::to_string_pretty(&value)?.as_bytes())?;
        file.sync_all()?;
        anyhow::ensure!(std::fs::read(path).ok().as_deref() == baseline, "{}", text("egui-settings-conflict"));
        std::fs::rename(&temporary, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temporary);
    }
    result
}

fn merge(target: &mut toml::Value, source: toml::Value) {
    if let (Some(target), Some(source)) = (target.as_table_mut(), source.as_table()) {
        for (key, value) in source {
            if let Some(existing) = target.get_mut(key) {
                merge(existing, value.clone());
            } else {
                target.insert(key.clone(), value.clone());
            }
        }
    } else {
        *target = source;
    }
}

fn directory_row(ui: &mut egui::Ui, directory: &mut String) {
    appearance::form_row(ui, &text("settings-paths-export-path"), |ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("...").on_hover_text(text("button-open")).clicked() && !ui.ctx().will_discard() {
                if let Some(path) = rfd::FileDialog::new().set_directory(&*directory).pick_folder() {
                    *directory = path.to_string_lossy().into_owned();
                }
            }
            ui.add_sized([ui.available_width(), 30.0], appearance::text_edit(directory));
        });
    });
}

fn help(ui: &mut egui::Ui) {
    let commands = icy_view::commands::create_icy_view_commands();
    let mut category = String::new();
    for id in COMMANDS {
        let Some(command) = commands.get(id) else {
            continue;
        };
        let Some(shortcut) = command.primary_hotkey_display() else {
            continue;
        };
        let next = command.fluent_category_key().map(|key| text(&key)).unwrap_or_default();
        if next != category {
            appearance::section(ui, &next);
            category = next;
        }
        ui.horizontal_wrapped(|ui| {
            ui.allocate_ui_with_layout(egui::vec2(170.0, 26.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.set_min_width(170.0);
                shortcuts::keycaps(ui, &shortcut);
            });
            ui.label(text(&command.fluent_action_key()));
        });
        ui.separator();
    }
}

fn sauce(ui: &mut egui::Ui, preview: &Preview, raw: &mut bool) {
    let Some(sauce) = &preview.sauce else {
        ui.label(text("egui-no-sauce"));
        return;
    };
    ui.horizontal(|ui| {
        appearance::tab(ui, raw, false, &text("sauce-btn-formatted"));
        appearance::tab(ui, raw, true, &text("sauce-btn-raw"));
    });
    if *raw {
        let bytes = &preview.data[preview.data.len().saturating_sub(128)..];
        for chunk in bytes.chunks(16) {
            ui.add(
                egui::Label::new(egui::RichText::new(chunk.iter().map(|byte| format!("{byte:02X} ")).collect::<String>()).monospace())
                    .wrap()
                    .selectable(true),
            );
        }
        return;
    }
    for (label, value) in [
        ("sauce-field-title", sauce.title().to_string()),
        ("sauce-field-author", sauce.author().to_string()),
        ("sauce-field-group", sauce.group().to_string()),
        ("sauce-field-date", sauce.date().to_string()),
        ("sauce-field-file-size", sauce.file_size().to_string()),
        ("sauce-field-type", format!("{:?}", sauce.data_type())),
    ] {
        appearance::value_row(ui, &text(label), &value);
    }
    appearance::section(ui, &text("sauce-section-capabilities"));
    if let Some(capabilities) = sauce.capabilities() {
        ui.add(egui::Label::new(format!("{capabilities:#?}")).wrap().selectable(true));
    }
    appearance::section(ui, &text("sauce-section-comments"));
    for comment in sauce.comments() {
        ui.add(egui::Label::new(comment.to_string()).wrap().selectable(true));
    }
}

struct Export {
    formats: Vec<FileFormat>,
    format: usize,
    directory: String,
    filename: String,
    confirmed: Option<PathBuf>,
    save_sauce: bool,
    options: icy_engine::SaveOptions,
}

impl Default for Export {
    fn default() -> Self {
        Self {
            formats: vec![FileFormat::Ansi],
            format: 0,
            directory: String::new(),
            filename: String::new(),
            confirmed: None,
            save_sauce: true,
            options: Default::default(),
        }
    }
}

impl Export {
    fn new(options: &Options, preview: &Preview) -> Self {
        let formats = if preview.image.is_some() {
            vec![FileFormat::Image(ImageFormat::Png), FileFormat::Image(ImageFormat::Gif)]
        } else {
            FileFormat::save_formats_with_images_for_buffer_type(preview.screen.terminal.screen.lock().buffer_type())
        };
        let filename = Path::new(&preview.file).file_stem().unwrap_or_default().to_string_lossy().into_owned();
        Self {
            formats,
            filename,
            directory: options.export_path().to_string_lossy().into_owned(),
            ..Default::default()
        }
    }

    fn target(&self) -> PathBuf {
        Path::new(&self.directory).join(format!("{}.{}", self.filename, self.formats[self.format].primary_extension()))
    }

    fn fields(&mut self, ui: &mut egui::Ui) {
        let before = self.target();
        appearance::combo_row(
            ui,
            &text("egui-format"),
            format!("{} (.{})", self.formats[self.format], self.formats[self.format].primary_extension()),
            |ui| {
                for (index, format) in self.formats.iter().enumerate() {
                    ui.selectable_value(&mut self.format, index, format.to_string());
                }
            },
        );
        directory_row(ui, &mut self.directory);
        appearance::form_row(ui, &text("header-name"), |ui| {
            ui.add_sized([ui.available_width(), 30.0], appearance::text_edit(&mut self.filename));
        });
        ui.add(egui::Label::new(self.target().display().to_string()).truncate());
        if before != self.target() {
            self.confirmed = None;
        }
        if !matches!(self.formats[self.format], FileFormat::Image(_)) {
            ui.checkbox(&mut self.save_sauce, "SAUCE");
            ui.checkbox(&mut self.options.preprocess.optimize_colors, text("egui-optimize-colors"));
            ui.checkbox(&mut self.options.preprocess.normalize_whitespaces, text("egui-normalize-spaces"));
            if self.formats[self.format] == FileFormat::Ansi {
                let mut ansi = self.options.ansi_options();
                appearance::combo_row(ui, &text("egui-compatibility"), ansi.level.to_string(), |ui| {
                    for level in [
                        icy_engine::AnsiCompatibilityLevel::AnsiSys,
                        icy_engine::AnsiCompatibilityLevel::Vt100,
                        icy_engine::AnsiCompatibilityLevel::IcyTerm,
                        icy_engine::AnsiCompatibilityLevel::Utf8Terminal,
                    ] {
                        ui.selectable_value(&mut ansi.level, level, level.to_string());
                    }
                });
                screen_preparation(ui, &mut ansi.screen_prep);
                ui.add_enabled_ui(ansi.level.supports_truecolor(), |ui| {
                    ui.checkbox(&mut ansi.always_use_rgb, "Truecolor");
                });
                let mut length = match ansi.line_length {
                    icy_engine::LineLength::Default => 0,
                    icy_engine::LineLength::Minimum(_) => 1,
                    icy_engine::LineLength::Maximum(_) => 2,
                };
                let mut columns = match ansi.line_length {
                    icy_engine::LineLength::Minimum(value) | icy_engine::LineLength::Maximum(value) => value,
                    _ => 80,
                };
                let lengths = [text("egui-line-default"), text("egui-line-min"), text("egui-line-max")];
                appearance::combo_row(ui, &text("egui-line-length"), &lengths[length], |ui| {
                    for (index, label) in lengths.iter().enumerate() {
                        ui.selectable_value(&mut length, index, label);
                    }
                });
                if length != 0 {
                    appearance::form_row(ui, &text("egui-columns"), |ui| {
                        ui.add(egui::DragValue::new(&mut columns).range(1..=u16::MAX));
                    });
                }
                ansi.line_length = match length {
                    1 => icy_engine::LineLength::Minimum(columns),
                    2 => icy_engine::LineLength::Maximum(columns),
                    _ => icy_engine::LineLength::Default,
                };
                let mut line_break = match ansi.line_break {
                    icy_engine::LineBreakBehavior::Wrap => 0,
                    icy_engine::LineBreakBehavior::Force => 1,
                    icy_engine::LineBreakBehavior::GotoXY => 2,
                };
                let breaks = [text("egui-line-wrap"), text("egui-line-force"), "GotoXY".into()];
                appearance::combo_row(ui, &text("egui-line-break"), &breaks[line_break], |ui| {
                    for (index, label) in breaks.iter().enumerate() {
                        ui.selectable_value(&mut line_break, index, label);
                    }
                });
                ansi.line_break = match line_break {
                    1 => icy_engine::LineBreakBehavior::Force,
                    2 => icy_engine::LineBreakBehavior::GotoXY,
                    _ => icy_engine::LineBreakBehavior::Wrap,
                };
                appearance::combo_row(ui, &text("egui-line-ending"), format!("{:?}", ansi.line_ending), |ui| {
                    ui.selectable_value(&mut ansi.line_ending, icy_engine::LineEnding::Lf, "LF");
                    ui.selectable_value(&mut ansi.line_ending, icy_engine::LineEnding::CrLf, "CRLF");
                });
                let controls = [
                    (icy_engine::ControlCharHandling::Ignore, "egui-controls-keep"),
                    (icy_engine::ControlCharHandling::FilterOut, "egui-controls-filter"),
                    (icy_engine::ControlCharHandling::IcyTerm, "egui-controls-escape"),
                ];
                let selected = controls.iter().find(|(value, _)| *value == ansi.control_char_handling).unwrap().1;
                appearance::combo_row(ui, &text("egui-control-chars"), text(selected), |ui| {
                    for (value, label) in controls {
                        ui.selectable_value(&mut ansi.control_char_handling, value, text(label));
                    }
                });
                if ansi.level.supports_sixel() {
                    appearance::section(ui, "Sixel");
                    appearance::form_row(ui, &text("egui-colors"), |ui| {
                        ui.add(egui::DragValue::new(&mut ansi.sixel.max_colors).range(2..=256));
                    });
                    appearance::slider_row(ui, &text("egui-diffusion"), &mut ansi.sixel.diffusion, 0.0..=1.0);
                    ui.checkbox(&mut ansi.sixel.use_kmeans, "K-means");
                }
                self.options.format = icy_engine::formats::FormatOptions::Ansi(ansi);
            } else if self.formats[self.format] == FileFormat::XBin {
                let mut compressed = self.options.compressed_options();
                ui.checkbox(&mut compressed.compress, text("egui-compress"));
                self.options.format = icy_engine::formats::FormatOptions::Compressed(compressed);
            } else if self.formats[self.format] == FileFormat::IcyDraw {
                let mut native = self.options.icy_draw_options();
                ui.checkbox(&mut native.compress, text("egui-compress"));
                self.options.format = icy_engine::formats::FormatOptions::IcyDraw(native);
            } else {
                let mut character = self.options.character_options();
                screen_preparation(ui, &mut character.screen_prep);
                ui.checkbox(&mut character.unicode, "UTF-8");
                self.options.format = icy_engine::formats::FormatOptions::Character(character);
            }
        }
        if self.confirmed.is_some() {
            ui.colored_label(ui.visuals().warn_fg_color, text("egui-overwrite-question"));
        }
    }

    fn save(&mut self, preview: &mut Preview) -> anyhow::Result<bool> {
        anyhow::ensure!(
            !self.filename.trim().is_empty() && Path::new(&self.filename).file_name() == Some(std::ffi::OsStr::new(&self.filename)),
            "{}",
            text("egui-invalid-name")
        );
        let target = self.target();
        if target.exists() && self.confirmed.as_ref() != Some(&target) {
            self.confirmed = Some(target);
            return Ok(false);
        }
        std::fs::create_dir_all(&self.directory)?;
        let format = self.formats[self.format];
        if let Some(image) = &preview.image_pixels {
            image.save(&target)?;
        } else if let FileFormat::Image(format) = format {
            format.save_screen(&**preview.screen.terminal.screen.lock(), &target)?;
        } else {
            let mut options = self.options.clone();
            if self.save_sauce {
                options.sauce = preview.sauce.as_ref().map(|sauce| sauce.metadata().clone());
            }
            let data = preview.screen.terminal.screen.lock().to_bytes(format.primary_extension(), &options)?;
            std::fs::write(&target, data)?;
        }
        Ok(true)
    }
}

fn screen_preparation(ui: &mut egui::Ui, preparation: &mut icy_engine::ScreenPreperation) {
    let values = [
        (icy_engine::ScreenPreperation::None, "egui-prep-none"),
        (icy_engine::ScreenPreperation::ClearScreen, "egui-prep-clear"),
        (icy_engine::ScreenPreperation::Home, "egui-prep-home"),
    ];
    let selected = values.iter().find(|(value, _)| value == preparation).unwrap().1;
    appearance::combo_row(ui, &text("egui-screen-preparation"), text(selected), |ui| {
        for (value, label) in values {
            ui.selectable_value(preparation, value, text(label));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_preserve_unknown_values_and_reject_external_changes() {
        let fixture = crate::tests::Fixture::new();
        let path = fixture.0.join("options.toml");
        let mut value = toml::Value::try_from(Options::default()).unwrap();
        value.as_table_mut().unwrap().insert("future".into(), toml::Value::String("keep".into()));
        let baseline = toml::to_string(&value).unwrap();
        std::fs::write(&path, &baseline).unwrap();
        let draft = Options {
            export_path: "new-path".into(),
            ..Default::default()
        };
        save_settings(&path, Some(baseline.as_bytes()), &draft).unwrap();
        let saved = std::fs::read_to_string(&path).unwrap();
        let value: toml::Value = toml::from_str(&saved).unwrap();
        assert_eq!(value["future"].as_str(), Some("keep"));
        assert_eq!(value["export_path"].as_str(), Some("new-path"));
        assert!(save_settings(&path, Some(baseline.as_bytes()), &Options::default()).is_err());
        assert_eq!(std::fs::read_to_string(path).unwrap(), saved);
    }

    #[test]
    fn export_writes_real_formats_and_confirmation_tracks_target() {
        let fixture = crate::tests::Fixture::new();
        let context = egui::Context::default();
        let mut preview = Preview::new(&context).unwrap();
        preview.load("art.ans".into(), b"HELLO".to_vec(), false, &context);
        crate::tests::wait_preview(&mut preview, &context);
        let mut export = Export {
            directory: fixture.0.to_string_lossy().into_owned(),
            filename: "art".into(),
            ..Default::default()
        };
        std::fs::write(export.target(), b"OLD").unwrap();
        assert!(!export.save(&mut preview).unwrap());
        assert_eq!(std::fs::read(export.target()).unwrap(), b"OLD");
        export.filename = "other".into();
        std::fs::write(export.target(), b"OTHER").unwrap();
        assert!(!export.save(&mut preview).unwrap());
        assert_eq!(std::fs::read(export.target()).unwrap(), b"OTHER");
        assert!(export.save(&mut preview).unwrap());
        let data = std::fs::read(export.target()).unwrap();
        assert!(data.windows(5).any(|bytes| bytes == b"HELLO"));
        export.filename = "../outside".into();
        assert!(export.save(&mut preview).is_err());
        export.filename = "image".into();
        export.formats = vec![FileFormat::Image(ImageFormat::Png)];
        assert!(export.save(&mut preview).unwrap());
        assert!(image::open(export.target()).unwrap().width() > 0);
    }
}
