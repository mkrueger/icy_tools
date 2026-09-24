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
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum Footer {
            Restore,
            Cancel,
            Save,
            Close,
            Raw,
        }
        let dialog = match mode {
            Mode::Settings => appearance::Dialog::untitled("viewer-settings")
                .size(appearance::DialogSize::XLarge)
                .fixed_height(560.0),
            Mode::About => appearance::Dialog::new("viewer-about")
                .title("Icy View")
                .size(appearance::DialogSize::Width(740.0))
                .scroll(false),
            Mode::Help => appearance::Dialog::new("viewer-help")
                .title(text("help-title"))
                .size(appearance::DialogSize::Width(740.0))
                .fixed_height(560.0),
            Mode::Sauce => appearance::Dialog::new("viewer-sauce")
                .title(text("sauce-dialog-title"))
                .size(appearance::DialogSize::Large),
            Mode::Export => appearance::Dialog::new("viewer-export")
                .size(appearance::DialogSize::Medium)
                .confirm_on_enter(true),
        };
        let response = dialog.show(context, |dialog| {
            if mode == Mode::Settings {
                let pages: Vec<_> = ["settings-monitor-category", "settings-commands-category", "settings-paths-category"]
                    .iter()
                    .enumerate()
                    .map(|(index, key)| (index, text(key)))
                    .collect();
                dialog.tabs(&mut self.page, &pages);
            }
            dialog.content(|ui| {
                match mode {
                    Mode::About => {
                        let height = (context.content_rect().height() - 190.0).clamp(48.0, 440.0);
                        ui.set_height(height);
                        if let Some(about) = &mut self.about {
                            let response = about.show(ui, &icy_engine_gui::MonitorSettings::neutral());
                            if let Some(url) = super::link_at(&about.terminal, response.hover_pos()) {
                                response.clone().on_hover_cursor(egui::CursorIcon::PointingHand);
                                if response.clicked() {
                                    context.open_url(egui::OpenUrl::new_tab(url));
                                }
                            }
                        }
                    }
                    Mode::Settings => self.settings_fields(ui),
                    Mode::Help => help(ui),
                    Mode::Sauce => sauce(ui, preview, self.raw),
                    Mode::Export => self.export.fields(ui),
                }
                if let Some(error) = &self.error {
                    ui.add_space(6.0);
                    ui.add(egui::Label::new(egui::RichText::new(error).color(ui.visuals().error_fg_color)).wrap());
                }
            });
            let mut buttons = Vec::new();
            match mode {
                Mode::Settings => {
                    if self.page == 0 {
                        buttons.push(appearance::DialogButton::secondary(appearance::labels::restore_defaults(), Footer::Restore).leading());
                    }
                    buttons.push(appearance::DialogButton::cancel(appearance::labels::cancel(), Footer::Cancel));
                    buttons.push(appearance::DialogButton::primary(appearance::labels::ok(), Footer::Save));
                }
                Mode::Export => {
                    buttons.push(appearance::DialogButton::cancel(appearance::labels::cancel(), Footer::Cancel));
                    buttons.push(if self.export.confirmed.is_some() {
                        appearance::DialogButton::destructive(appearance::labels::overwrite(), Footer::Save)
                    } else {
                        appearance::DialogButton::primary(text("egui-save"), Footer::Save)
                    });
                }
                Mode::Sauce => {
                    if preview.sauce.is_some() {
                        let label = text(if self.raw { "sauce-btn-formatted" } else { "sauce-btn-raw" });
                        buttons.push(appearance::DialogButton::secondary(label, Footer::Raw).leading());
                    }
                    buttons.push(appearance::DialogButton::primary(appearance::labels::close(), Footer::Close).cancels());
                }
                Mode::About | Mode::Help => {
                    buttons.push(appearance::DialogButton::primary(appearance::labels::close(), Footer::Close).cancels());
                }
            }
            dialog.buttons(buttons);
        });
        let mut close = response.dismissed;
        match response.action {
            Some(Footer::Restore) => self.draft.monitor_settings = icy_engine_gui::MonitorSettings::default(),
            Some(Footer::Save) => {
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
            Some(Footer::Raw) => self.raw = !self.raw,
            Some(Footer::Cancel | Footer::Close) => close = true,
            None => {}
        }
        if close {
            self.mode = None;
        }
    }

    fn settings_fields(&mut self, ui: &mut egui::Ui) {
        match self.page {
            0 => monitor::fields(ui, &mut self.draft.monitor_settings),
            1 => {
                appearance::group(ui, &text("settings-commands-section"), |ui| {
                    for (index, command) in self.draft.external_commands.iter_mut().enumerate() {
                        appearance::form_row(ui, &format!("F{}", index + 5), |ui| {
                            ui.add_sized([ui.available_width(), 30.0], appearance::text_edit(&mut command.command));
                        });
                    }
                });
            }
            _ => {
                let mut error = None;
                appearance::group(ui, &text("settings-paths-header"), |ui| {
                    for (key, path) in [
                        ("settings-paths-config-dir", icy_view::get_config_dir().to_path_buf()),
                        ("settings-paths-config-file", icy_view::get_config_dir().join("options.toml")),
                        ("settings-paths-log-file", Options::get_log_file()),
                    ] {
                        appearance::form_row(ui, &text(key), |ui| {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button(text("settings-paths-open")).clicked() {
                                    if let Err(open_error) = open::that(&path) {
                                        error = Some(open_error.to_string());
                                    }
                                }
                                ui.add(egui::Label::new(path.to_string_lossy()).truncate())
                                    .on_hover_text(path.display().to_string());
                            });
                        });
                    }
                });
                if error.is_some() {
                    self.error = error;
                }
                appearance::group(ui, &text("settings-paths-user-header"), |ui| directory_row(ui, &mut self.draft.export_path));
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
        ui.horizontal(|ui| {
            let browse_width = 36.0;
            let input_width = (ui.available_width() - browse_width - ui.spacing().item_spacing.x).max(40.0);
            ui.add_sized([input_width, 30.0], appearance::text_edit(directory));
            if ui
                .add_sized([browse_width, 30.0], egui::Button::new("..."))
                .on_hover_text(text("button-open"))
                .clicked()
                && !ui.ctx().will_discard()
            {
                if let Some(path) = rfd::FileDialog::new().set_directory(&*directory).pick_folder() {
                    *directory = path.to_string_lossy().into_owned();
                }
            }
        });
    });
}

fn help(ui: &mut egui::Ui) {
    let commands = icy_view::commands::create_icy_view_commands();
    let mut groups: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for id in COMMANDS {
        let Some(command) = commands.get(id) else {
            continue;
        };
        let Some(shortcut) = command.primary_hotkey_display() else {
            continue;
        };
        let category = command.fluent_category_key().map(|key| text(&key)).unwrap_or_default();
        if groups.last().is_none_or(|(last, _)| *last != category) {
            groups.push((category, Vec::new()));
        }
        groups.last_mut().unwrap().1.push((shortcut, text(&command.fluent_action_key())));
    }
    let key_width = (ui.available_width() * 0.4).clamp(120.0, 190.0);
    for (category, rows) in groups {
        appearance::compact_group(ui, &category, |ui| {
            for (shortcut, action) in rows {
                ui.horizontal(|ui| {
                    ui.allocate_ui_with_layout(egui::vec2(key_width, 26.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.set_min_width(key_width);
                        shortcuts::keycaps(ui, &shortcut);
                    });
                    ui.add(egui::Label::new(action).wrap());
                });
            }
        });
    }
}

fn sauce(ui: &mut egui::Ui, preview: &Preview, raw: bool) {
    let Some(sauce) = &preview.sauce else {
        ui.add_space(24.0);
        ui.vertical_centered(|ui| {
            ui.label(egui::RichText::new("SAUCE").size(28.0).strong().color(ui.visuals().weak_text_color()));
            ui.add_space(4.0);
            ui.label(egui::RichText::new(text("egui-no-sauce")).weak());
        });
        ui.add_space(24.0);
        return;
    };
    if raw {
        sauce_raw(ui, preview, sauce);
    } else {
        sauce_formatted(ui, sauce);
    }
}

fn sauce_formatted(ui: &mut egui::Ui, sauce: &icy_sauce::SauceRecord) {
    let palette = super::colors::Sauce::new(ui.visuals().dark_mode);
    let title = sauce.title().to_string();
    let author = sauce.author().to_string();
    let group = sauce.group().to_string();
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        let (title, color) = match title.trim() {
            "" => (text("sauce-unknown"), palette.empty),
            title => (title.to_owned(), palette.title),
        };
        ui.add(egui::Label::new(appearance::bold(ui, title).size(20.0).color(color)).wrap().selectable(true));
    });
    let mut byline = egui::text::LayoutJob::default();
    for (value, color) in [(author.trim(), palette.author), (group.trim(), palette.group)] {
        if value.is_empty() {
            continue;
        }
        if !byline.text.is_empty() {
            byline.append("  •  ", 0.0, text_format(14.0, palette.separator));
        }
        byline.append(value, 0.0, text_format(14.0, color));
    }
    if !byline.text.is_empty() {
        ui.add(egui::Label::new(byline).wrap().selectable(true));
    }
    ui.add_space(8.0);
    appearance::compact_group(ui, "", |ui| {
        field(ui, "sauce-field-date", sauce_date(sauce).unwrap_or_else(|| text("sauce-unknown")), None);
        field(ui, "sauce-field-file-size", count("sauce-value-bytes", sauce.file_size().into()), None);
    });
    if let Some(capabilities) = sauce.capabilities() {
        let rows = capability_rows(&capabilities, sauce.data_type());
        if !rows.is_empty() {
            appearance::compact_group(ui, &text("sauce-section-capabilities"), |ui| {
                for (label, value) in rows {
                    field(ui, label, value, None);
                }
            });
        }
    }
    let comments: Vec<String> = sauce.comments().iter().map(|comment| comment.to_string().trim_end().to_owned()).collect();
    let comments = comments.join("\n");
    if !comments.trim().is_empty() {
        appearance::compact_group(ui, &text("sauce-section-comments"), |ui| comment_box(ui, comments.trim_matches('\n')));
    }
}

fn sauce_raw(ui: &mut egui::Ui, preview: &Preview, sauce: &icy_sauce::SauceRecord) {
    let header = sauce.header();
    let data_type = sauce.data_type();
    let data_type_number: u8 = header.data_type.into();
    appearance::compact_group(ui, &text("sauce-section-raw-header"), |ui| {
        field(ui, "sauce-field-title", sauce.title().to_string(), None);
        field(ui, "sauce-field-author", sauce.author().to_string(), None);
        field(ui, "sauce-field-group", sauce.group().to_string(), None);
        field(ui, "sauce-field-date", sauce.date().to_string(), None);
        field(ui, "sauce-field-data-type", data_type_number.to_string(), Some(format!("{data_type:?}")));
        field(
            ui,
            "sauce-field-file-type",
            header.file_type.to_string(),
            Some(file_type_name(data_type, header.file_type)),
        );
        field(ui, "sauce-field-file-size", sauce.file_size().to_string(), None);
    });
    appearance::compact_group(ui, &text("sauce-section-technical"), |ui| {
        for (label, value) in [
            ("sauce-field-tinfo1", header.t_info1),
            ("sauce-field-tinfo2", header.t_info2),
            ("sauce-field-tinfo3", header.t_info3),
            ("sauce-field-tinfo4", header.t_info4),
        ] {
            field(ui, label, value.to_string(), None);
        }
        field(
            ui,
            "sauce-field-tflags",
            format!("0x{:02X}", header.t_flags),
            Some(format!("0b{:08b}", header.t_flags)),
        );
        let info = header.t_info_s.to_string();
        field(
            ui,
            "sauce-field-tinfos",
            if info.trim().is_empty() {
                text("sauce-value-none")
            } else {
                info.trim().to_owned()
            },
            None,
        );
        field(ui, "sauce-section-comments", count("sauce-value-lines", sauce.comments().len() as u64), None);
    });
    let comments: Vec<String> = sauce.comments().iter().map(ToString::to_string).collect();
    if !comments.is_empty() {
        appearance::compact_group(ui, &text("sauce-section-comment-lines"), |ui| comment_box(ui, &comments.join("\n")));
    }
    let record = &preview.data[preview.data.len().saturating_sub(128)..];
    appearance::compact_group(ui, "SAUCE00", |ui| {
        let dump: Vec<String> = record
            .chunks(16)
            .enumerate()
            .map(|(row, chunk)| {
                let hex: String = chunk.iter().map(|byte| format!("{byte:02X} ")).collect();
                let ascii: String = chunk
                    .iter()
                    .map(|byte| if byte.is_ascii_graphic() || *byte == b' ' { *byte as char } else { '.' })
                    .collect();
                format!("{:02X}  {hex:<48} {ascii}", row * 16)
            })
            .collect();
        egui::ScrollArea::horizontal().id_salt("sauce-hex").show(ui, |ui| {
            ui.add(
                egui::Label::new(egui::RichText::new(dump.join("\n")).monospace().size(12.0))
                    .extend()
                    .selectable(true),
            );
        });
    });
}

fn capability_rows(capabilities: &icy_sauce::Capabilities, data_type: icy_sauce::SauceDataType) -> Vec<(&'static str, String)> {
    use icy_sauce::Capabilities;
    let flags = |ice: bool, nine_pixels: bool, legacy_aspect: bool| {
        let mut flags = Vec::new();
        if ice {
            flags.push("iCE".to_owned());
        }
        flags.push(text(if nine_pixels { "sauce-value-9px" } else { "sauce-value-8px" }));
        flags.push(text(if legacy_aspect { "sauce-value-legacy" } else { "sauce-value-modern" }));
        flags.join(", ")
    };
    let mut rows = Vec::new();
    match capabilities {
        Capabilities::Character(caps) => {
            rows.push(("sauce-field-format", format!("{data_type:?} / {:?}", caps.format)));
            if caps.columns > 0 || caps.lines > 0 {
                rows.push(("sauce-field-size", format!("{} × {}", caps.columns, caps.lines)));
            }
            rows.push((
                "sauce-field-flags",
                flags(caps.ice_colors, caps.letter_spacing.use_letter_spacing(), caps.aspect_ratio.use_aspect_ratio()),
            ));
            if let Some(font) = caps.font().map(|font| font.to_string()).filter(|font| !font.trim().is_empty()) {
                rows.push(("sauce-field-font", font.trim().to_owned()));
            }
        }
        Capabilities::Binary(caps) => {
            rows.push(("sauce-field-format", format!("{data_type:?} / {:?}", caps.format)));
            if caps.columns > 0 || caps.lines > 0 {
                rows.push(("sauce-field-size", format!("{} × {}", caps.columns, caps.lines)));
            }
            rows.push((
                "sauce-field-flags",
                flags(caps.ice_colors, caps.letter_spacing.use_letter_spacing(), caps.aspect_ratio.use_aspect_ratio()),
            ));
            if let Some(font) = caps.font().map(|font| font.to_string()).filter(|font| !font.trim().is_empty()) {
                rows.push(("sauce-field-font", font.trim().to_owned()));
            }
        }
        Capabilities::Vector(caps) => rows.push(("sauce-field-format", format!("{data_type:?} / {:?}", caps.format))),
        Capabilities::Bitmap(caps) => {
            rows.push(("sauce-field-format", format!("{data_type:?} / {:?}", caps.format)));
            if caps.width > 0 || caps.height > 0 {
                rows.push(("sauce-field-size", format!("{} × {}", caps.width, caps.height)));
            }
            if caps.pixel_depth > 0 {
                rows.push(("sauce-field-pixel-depth", count("sauce-value-bpp", caps.pixel_depth.into())));
            }
        }
        Capabilities::Audio(caps) => {
            rows.push(("sauce-field-format", format!("{data_type:?} / {:?}", caps.format)));
            if caps.sample_rate > 0 {
                rows.push(("sauce-field-sample-rate", count("sauce-value-hz", caps.sample_rate.into())));
            }
        }
        Capabilities::Archive(caps) => rows.push(("sauce-field-format", format!("{data_type:?} / {:?}", caps.format))),
        Capabilities::Executable(_) => rows.push(("sauce-field-format", format!("{data_type:?}"))),
    }
    rows
}

fn file_type_name(data_type: icy_sauce::SauceDataType, file_type: u8) -> String {
    use icy_sauce::{ArchiveFormat, AudioFormat, BitmapFormat, CharacterFormat, SauceDataType, VectorFormat};
    match data_type {
        SauceDataType::Character => format!("{:?}", CharacterFormat::from_sauce(file_type)),
        SauceDataType::Bitmap => format!("{:?}", BitmapFormat::from_sauce(data_type, file_type)),
        SauceDataType::Vector => format!("{:?}", VectorFormat::from_sauce(file_type)),
        SauceDataType::Audio => format!("{:?}", AudioFormat::from_sauce(file_type)),
        SauceDataType::Archive => format!("{:?}", ArchiveFormat::from_sauce(file_type)),
        other => format!("{other:?}"),
    }
}

/// Label column on the left, selectable value and an optional weak note on the right.
fn field(ui: &mut egui::Ui, label: &str, value: String, note: Option<String>) {
    let label_width = (ui.available_width() * 0.35).clamp(90.0, 150.0);
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(egui::vec2(label_width, 20.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.set_min_width(label_width);
            ui.add(egui::Label::new(egui::RichText::new(text(label)).weak()).truncate());
        });
        let value = if value.trim().is_empty() { "—".to_owned() } else { value };
        ui.add(egui::Label::new(value).wrap().selectable(true));
        if let Some(note) = note {
            ui.label(egui::RichText::new(note).weak().size(12.0));
        }
    });
}

fn comment_box(ui: &mut egui::Ui, comments: &str) {
    egui::Frame::new()
        .fill(ui.visuals().extreme_bg_color)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .corner_radius(4)
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::ScrollArea::vertical().id_salt("sauce-comments").max_height(120.0).show(ui, |ui| {
                ui.add(egui::Label::new(egui::RichText::new(comments).monospace()).wrap().selectable(true));
            });
        });
}

fn text_format(size: f32, color: egui::Color32) -> egui::TextFormat {
    egui::TextFormat {
        font_id: egui::FontId::proportional(size),
        color,
        ..Default::default()
    }
}

/// Localized count message without the bidi isolation marks that egui would render.
fn count(key: &str, count: u64) -> String {
    let mut args = std::collections::HashMap::new();
    args.insert("count", count);
    icy_view::LANGUAGE_LOADER.get_args(key, args).replace(['\u{2068}', '\u{2069}'], "")
}

/// `YYYY-MM-DD`, or nothing for the all-zero date of a record without one.
pub fn sauce_date(sauce: &icy_sauce::SauceRecord) -> Option<String> {
    let date = sauce.date();
    (date.year > 0 || date.month > 0 || date.day > 0).then(|| format!("{:04}-{:02}-{:02}", date.year, date.month, date.day))
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
        appearance::group(ui, "", |ui| {
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
            ui.add(egui::Label::new(egui::RichText::new(format!("→ {}", self.target().display())).weak().size(12.0)).truncate());
        });
        if before != self.target() {
            self.confirmed = None;
        }
        if !matches!(self.formats[self.format], FileFormat::Image(_)) {
            appearance::group(ui, "", |ui| {
                appearance::check_row(ui, "SAUCE", &mut self.save_sauce);
                appearance::check_row(ui, &text("egui-optimize-colors"), &mut self.options.preprocess.optimize_colors);
                appearance::check_row(ui, &text("egui-normalize-spaces"), &mut self.options.preprocess.normalize_whitespaces);
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
                        appearance::check_row(ui, "Truecolor", &mut ansi.always_use_rgb);
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
                        appearance::check_row(ui, "K-means", &mut ansi.sixel.use_kmeans);
                    }
                    self.options.format = icy_engine::formats::FormatOptions::Ansi(ansi);
                } else if self.formats[self.format] == FileFormat::XBin {
                    let mut compressed = self.options.compressed_options();
                    appearance::check_row(ui, &text("egui-compress"), &mut compressed.compress);
                    self.options.format = icy_engine::formats::FormatOptions::Compressed(compressed);
                } else if self.formats[self.format] == FileFormat::IcyDraw {
                    let mut native = self.options.icy_draw_options();
                    appearance::check_row(ui, &text("egui-compress"), &mut native.compress);
                    self.options.format = icy_engine::formats::FormatOptions::IcyDraw(native);
                } else {
                    let mut character = self.options.character_options();
                    screen_preparation(ui, &mut character.screen_prep);
                    appearance::check_row(ui, "UTF-8", &mut character.unicode);
                    self.options.format = icy_engine::formats::FormatOptions::Character(character);
                }
            });
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
