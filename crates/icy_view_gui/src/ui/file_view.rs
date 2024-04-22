use directories::UserDirs;
use eframe::{
    egui::{self, Image, Layout, RichText, Sense, TopBottomPanel, WidgetText},
    epaint::{FontFamily, FontId, Rounding},
};
use egui::{ScrollArea, TextEdit, Ui};
use i18n_embed_fl::fl;
use icy_sauce::SauceInformation;

use std::{
    env,
    fs::{self, File},
    io::{Error, Read},
    path::{Path, PathBuf},
};

use super::options::{Options, ScrollSpeed};

pub enum Message {
    Select(usize, bool),
    Open(usize),
    Cancel,
    Refresh,
    ParentFolder,
    ToggleAutoScroll,
    ShowSauce(usize),
    ShowHelpDialog,
    ChangeScrollSpeed,
}

#[derive(Clone)]
pub struct FileEntry {
    pub file_info: FileInfo,
    pub file_data: Option<Vec<u8>>,
    pub read_sauce: bool,
    pub sauce: Option<SauceInformation>,
}

impl FileEntry {
    pub fn get_data<T>(&self, func: fn(&PathBuf, &[u8]) -> T) -> anyhow::Result<T> {
        if let Some(data) = &self.file_data {
            return Ok(func(&self.file_info.path, data));
        }

        let file = File::open(&self.file_info.path)?;
        let mmap = unsafe { memmap::MmapOptions::new().map(&file)? };
        Ok(func(&self.file_info.path, &mmap))
    }

    pub fn read_image<'a>(&self, func: fn(&PathBuf, Vec<u8>) -> Image<'a>) -> anyhow::Result<Image<'a>> {
        let path = self.file_info.clone();
        if let Some(data) = &self.file_data {
            let data = data.clone();
            Ok(func(&path.path, data))
        } else {
            let data = fs::read(&path.path)?;
            Ok(func(&path.path, data))
        }
    }

    pub fn is_file(&self) -> bool {
        self.file_data.is_some() || !self.file_info.dir
    }

    fn load_sauce(&mut self) {
        if self.read_sauce {
            return;
        }
        self.read_sauce = true;

        if let Ok(Ok(Some(data))) = self.get_data(|_, data| SauceInformation::read(data)) {
            self.sauce = Some(data);
        }
    }

    pub(crate) fn is_dir(&self) -> bool {
        self.file_info.dir
    }

    fn is_dir_or_archive(&self) -> bool {
        if let Some(ext) = self.file_info.path.extension() {
            if ext.to_string_lossy().to_ascii_lowercase() == "zip" {
                return true;
            }
        }

        self.is_dir()
    }

    pub(crate) fn get_sauce(&self) -> Option<SauceInformation> {
        if !self.read_sauce {
            return None;
        }
        self.sauce.clone()
    }
}

pub struct FileView {
    /// Current opened path.
    path: PathBuf,
    /// Selected file path
    pub selected_file: Option<usize>,
    pub scroll_pos: Option<usize>,
    /// Files in directory.
    pub files: Vec<FileEntry>,
    pub upgrade_version: Option<String>,

    pub options: super::options::Options,
    pub filter: String,
    pre_select_file: Option<String>,
}

impl FileView {
    pub fn new(initial_path: Option<PathBuf>, options: Options) -> Self {
        let mut path = if let Some(path) = initial_path {
            path
        } else if let Some(user_dirs) = UserDirs::new() {
            user_dirs.home_dir().to_path_buf()
        } else {
            env::current_dir().unwrap_or_default()
        };

        let mut pre_select_file = None;

        if !path.exists() {
            pre_select_file = Some(path.file_name().unwrap().to_string_lossy().to_string());
            path.pop();
        }

        if path.is_file() && path.extension().unwrap_or_default().to_string_lossy().to_ascii_lowercase() != "zip" {
            pre_select_file = Some(path.file_name().unwrap().to_string_lossy().to_string());
            path.pop();
        }

        Self {
            path,
            selected_file: None,
            pre_select_file,
            scroll_pos: None,
            files: Vec::new(),
            filter: String::new(),
            options,
            upgrade_version: None,
        }
    }

    pub(crate) fn show_ui(&mut self, ui: &mut Ui, file_chooser: bool) -> Option<Message> {
        let mut command: Option<Message> = None;

        if file_chooser {
            TopBottomPanel::bottom("bottom_buttons").show_inside(ui, |ui| {
                ui.set_width(350.0);
                ui.add_space(4.0);
                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(fl!(crate::LANGUAGE_LOADER, "button-open")).clicked() {
                        if let Some(sel) = self.selected_file {
                            command = Some(Message::Open(sel));
                        }
                    }
                    if ui.button(fl!(crate::LANGUAGE_LOADER, "button-cancel")).clicked() {
                        command = Some(Message::Cancel);
                    }
                });
            });
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add(
                TextEdit::singleline(&mut self.filter)
                    .hint_text(fl!(crate::LANGUAGE_LOADER, "filter-entries-hint-text"))
                    .desired_width(300.),
            );
            let response = ui.button("🗙").on_hover_text(fl!(crate::LANGUAGE_LOADER, "tooltip-reset-filter-button"));
            if response.clicked() {
                self.filter.clear();
            }
            if let Some(ver) = &self.upgrade_version {
                ui.hyperlink_to(
                    fl!(crate::LANGUAGE_LOADER, "menu-upgrade_version", version = ver.clone()),
                    "https://github.com/mkrueger/icy_view/releases/latest",
                );
            }
        });

        ui.horizontal(|ui| {
            match self.path.to_str() {
                Some(path) => {
                    let mut path_edit = path.to_string();
                    ui.add(TextEdit::singleline(&mut path_edit).desired_width(f32::INFINITY));
                }
                None => {
                    ui.colored_label(ui.style().visuals.error_fg_color, fl!(crate::LANGUAGE_LOADER, "error-invalid-path"));
                }
            }

            ui.add_enabled_ui(self.path.parent().is_some(), |ui| {
                let response = ui.button("⬆").on_hover_text("Parent Folder");
                if response.clicked() {
                    command = Some(Message::ParentFolder);
                }
            });

            let response = ui.button("⟲").on_hover_text(fl!(crate::LANGUAGE_LOADER, "tooltip-refresh"));
            if response.clicked() {
                command = Some(Message::Refresh);
            }

            ui.menu_button("…", |ui| {
                let r = ui.hyperlink_to(
                    fl!(crate::LANGUAGE_LOADER, "menu-item-discuss"),
                    "https://github.com/mkrueger/icy_view/discussions",
                );
                if r.clicked() {
                    ui.close_menu();
                }
                let r = ui.hyperlink_to(
                    fl!(crate::LANGUAGE_LOADER, "menu-item-report-bug"),
                    "https://github.com/mkrueger/icy_view/issues/new",
                );
                if r.clicked() {
                    ui.close_menu();
                }
                let r = ui.hyperlink_to(
                    fl!(crate::LANGUAGE_LOADER, "menu-item-check-releases"),
                    "https://github.com/mkrueger/icy_view/releases/latest",
                );
                if r.clicked() {
                    ui.close_menu();
                }
                ui.separator();
                let mut b = self.options.auto_scroll_enabled;
                if ui.checkbox(&mut b, fl!(crate::LANGUAGE_LOADER, "menu-item-auto-scroll")).clicked() {
                    command = Some(Message::ToggleAutoScroll);
                    ui.close_menu();
                }
                let title = match self.options.scroll_speed {
                    ScrollSpeed::Slow => fl!(crate::LANGUAGE_LOADER, "menu-item-scroll-speed-slow"),
                    ScrollSpeed::Medium => {
                        fl!(crate::LANGUAGE_LOADER, "menu-item-scroll-speed-medium")
                    }
                    ScrollSpeed::Fast => fl!(crate::LANGUAGE_LOADER, "menu-item-scroll-speed-fast"),
                };

                let r = ui.selectable_label(false, title);
                if r.clicked() {
                    command = Some(Message::ChangeScrollSpeed);
                    ui.close_menu();
                }
            });
        });
        if self.selected_file.is_none() && !self.files.is_empty() {
            //  command = Some(Command::Select(0));
        }

        let area = ScrollArea::vertical();
        let row_height = ui.text_style_height(&egui::TextStyle::Body);
        let strong_color = ui.style().visuals.strong_text_color();
        let text_color = ui.style().visuals.text_color();

        let filter = self.filter.to_lowercase();
        let filtered_entries = self.files.iter_mut().enumerate().filter(|(_, p)| {
            if filter.is_empty() {
                return true;
            }
            if let Some(sauce) = &p.sauce {
                if sauce.title().to_string().to_lowercase().contains(&filter)
                /*    || sauce
                    .group
                    .to_string()
                    .to_lowercase()
                    .contains(&filter)
                || sauce
                    .author
                    .to_string()
                    .to_lowercase()
                    .contains(&filter)*/
                {
                    return true;
                }
            }
            p.file_info.path.to_string_lossy().to_lowercase().contains(&filter)
        });

        let mut indices = Vec::new();
        let area_res = area.show(ui, |ui| {
            for (real_idx, entry) in filtered_entries {
                let (id, rect) = ui.allocate_space([ui.available_width(), row_height].into());

                indices.push(real_idx);
                let is_selected = Some(real_idx) == self.selected_file;
                let text_color = if is_selected { strong_color } else { text_color };
                let mut response = ui.interact(rect, id, Sense::click());
                if response.hovered() {
                    ui.painter()
                        .rect_filled(rect.expand(1.0), Rounding::same(4.0), ui.style().visuals.widgets.active.bg_fill);
                } else if is_selected {
                    ui.painter()
                        .rect_filled(rect.expand(1.0), Rounding::same(4.0), ui.style().visuals.extreme_bg_color);
                }

                let label = if !ui.is_rect_visible(rect) {
                    get_file_name(&entry.file_info.path).to_string()
                } else {
                    match entry.is_dir_or_archive() {
                        true => "🗀 ",
                        false => "🗋 ",
                    }
                    .to_string()
                        + get_file_name(&entry.file_info.path)
                };

                let font_id = FontId::new(14.0, FontFamily::Proportional);
                let text: WidgetText = label.into();
                let galley = text.into_galley(ui, Some(false), f32::INFINITY, font_id);
                ui.painter()
                    .galley_with_override_text_color(egui::Align2::LEFT_TOP.align_size_within_rect(galley.size(), rect).min, galley, text_color);
                if response.hovered() {
                    entry.load_sauce();
                    if let Some(sauce) = &entry.sauce {
                        response = response.on_hover_ui(|ui| {
                            egui::Grid::new("some_unique_id").num_columns(2).spacing([4.0, 2.0]).show(ui, |ui| {
                                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(fl!(crate::LANGUAGE_LOADER, "heading-title"));
                                });
                                ui.strong(sauce.title().to_string());
                                ui.end_row();
                                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(fl!(crate::LANGUAGE_LOADER, "heading-author"));
                                });
                                ui.strong(sauce.author().to_string());
                                ui.end_row();
                                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(fl!(crate::LANGUAGE_LOADER, "heading-group"));
                                });
                                ui.strong(sauce.group().to_string());
                                ui.end_row();
                                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(fl!(crate::LANGUAGE_LOADER, "heading-screen-mode"));
                                });
                                let mut flags: String = String::new();
                                if let Ok(caps) = sauce.get_character_capabilities() {
                                    if caps.use_ice {
                                        flags.push_str("ICE");
                                    }

                                    if caps.use_letter_spacing {
                                        if !flags.is_empty() {
                                            flags.push(',');
                                        }
                                        flags.push_str("9px");
                                    }

                                    if caps.use_aspect_ratio {
                                        if !flags.is_empty() {
                                            flags.push(',');
                                        }
                                        flags.push_str("AR");
                                    }

                                    if flags.is_empty() {
                                        ui.strong(RichText::new(format!("{}x{}", caps.width, caps.height)));
                                    } else {
                                        ui.strong(RichText::new(format!("{}x{} ({})", caps.width, caps.height, flags)));
                                    }
                                }
                                ui.end_row();
                            });
                        });
                    }
                }

                if response.clicked() {
                    command = Some(Message::Select(real_idx, false));
                }

                if response.double_clicked() {
                    command = Some(Message::Open(real_idx));
                }
            }
        });

        if ui.is_enabled() {
            if ui.input(|i| i.key_pressed(egui::Key::PageUp) && i.modifiers.alt) {
                command = Some(Message::ParentFolder);
            }

            if ui.input(|i| i.key_pressed(egui::Key::F1)) {
                command = Some(Message::ShowHelpDialog);
            }

            if ui.input(|i| i.key_pressed(egui::Key::F2)) {
                command = Some(Message::ToggleAutoScroll);
            }

            if ui.input(|i| i.key_pressed(egui::Key::F3)) {
                command = Some(Message::ChangeScrollSpeed);
            }

            if let Some(s) = self.selected_file {
                if ui.input(|i| i.key_pressed(egui::Key::F4)) {
                    command = Some(Message::ShowSauce(s));
                }
                let found = indices.iter().position(|i| *i == s);
                if let Some(idx) = found {
                    if ui.input(|i| i.key_pressed(egui::Key::ArrowUp) && i.modifiers.is_none()) && idx > 0 {
                        command = Some(Message::Select(indices[idx - 1], false));
                    }

                    if ui.input(|i| i.key_pressed(egui::Key::ArrowDown) && i.modifiers.is_none()) && idx + 1 < indices.len() {
                        command = Some(Message::Select(indices[idx + 1], false));
                    }

                    if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        command = Some(Message::Open(s));
                    }

                    if !self.files.is_empty() {
                        if ui.input(|i: &egui::InputState| i.key_pressed(egui::Key::Home) && i.modifiers.is_none() && !indices.is_empty()) {
                            command = Some(Message::Select(indices[0], false));
                        }

                        if ui.input(|i| i.key_pressed(egui::Key::End) && i.modifiers.is_none()) && !indices.is_empty() {
                            command = Some(Message::Select(indices[indices.len() - 1], false));
                        }

                        if ui.input(|i| i.key_pressed(egui::Key::PageUp) && i.modifiers.is_none()) && !indices.is_empty() {
                            let page_size = (area_res.inner_rect.height() / row_height) as usize;
                            command = Some(Message::Select(indices[idx.saturating_sub(page_size)], false));
                        }

                        if ui.input(|i| i.key_pressed(egui::Key::PageDown) && i.modifiers.is_none()) && !indices.is_empty() {
                            let page_size = (area_res.inner_rect.height() / row_height) as usize;
                            command = Some(Message::Select(indices[(idx.saturating_add(page_size)).min(indices.len() - 1)], false));
                        }
                    }
                }
            } else if !self.files.is_empty() {
                if ui.input(|i| {
                    i.key_pressed(egui::Key::ArrowUp)
                        || i.key_pressed(egui::Key::ArrowDown)
                        || i.key_pressed(egui::Key::PageUp)
                        || i.key_pressed(egui::Key::PageDown)
                }) {
                    command = Some(Message::Select(0, false));
                }

                if ui.input(|i| i.key_pressed(egui::Key::Home)) {
                    command = Some(Message::Select(0, false));
                }

                if ui.input(|i| i.key_pressed(egui::Key::End)) {
                    command = Some(Message::Select(self.files.len().saturating_sub(1), false));
                }
            }
        }

        command
    }

    pub fn get_path(&self) -> PathBuf {
        self.path.clone()
    }

    pub fn set_path(&mut self, path: impl Into<PathBuf>) -> Option<Message> {
        self.path = path.into();
        self.refresh()
    }

    pub fn refresh(&mut self) -> Option<Message> {
        self.files.clear();

        if self.path.is_file() {
            match fs::File::open(&self.path) {
                Ok(file) => match zip::ZipArchive::new(file) {
                    Ok(mut archive) => {
                        for i in 0..archive.len() {
                            match archive.by_index(i) {
                                Ok(mut file) => {
                                    let mut data = Vec::new();
                                    file.read_to_end(&mut data).unwrap_or_default();

                                    let entry = FileEntry {
                                        file_info: FileInfo {
                                            path: file.enclosed_name().unwrap_or(PathBuf::from("unknown")).to_path_buf(),
                                            dir: file.is_dir(),
                                        },
                                        file_data: Some(data),
                                        read_sauce: false,
                                        sauce: None,
                                    };
                                    self.files.push(entry);
                                }
                                Err(err) => {
                                    log::error!("Error reading zip file: {}", err);
                                }
                            }
                        }
                    }
                    Err(err) => {
                        log::error!("Error reading zip archive: {}", err);
                    }
                },
                Err(err) => {
                    log::error!("Failed to open zip file: {}", err);
                }
            }
        } else {
            let folders = read_folder(&self.path);
            match folders {
                Ok(folders) => {
                    self.files = folders
                        .iter()
                        .map(|f| FileEntry {
                            file_info: f.clone(),
                            read_sauce: false,
                            sauce: None,
                            file_data: None,
                        })
                        .collect();
                }
                Err(err) => {
                    log::error!("Failed to read folder: {}", err);
                }
            }
        }
        self.selected_file = None;

        if let Some(file) = &self.pre_select_file {
            for (i, entry) in self.files.iter().enumerate() {
                if let Some(file_name) = entry.file_info.path.file_name() {
                    if file_name.to_string_lossy() == *file {
                        return Message::Select(i, false).into();
                    }
                }
            }
        }
        None
    }
}

#[cfg(windows)]
fn is_drive_root(path: &Path) -> bool {
    path.to_str()
        .filter(|path| &path[1..] == ":\\")
        .and_then(|path| path.chars().next())
        .map_or(false, |ch| ch.is_ascii_uppercase())
}

fn get_file_name(path: &Path) -> &str {
    #[cfg(windows)]
    if path.is_dir() && is_drive_root(path) {
        return path.to_str().unwrap_or_default();
    }
    path.file_name().and_then(|name| name.to_str()).unwrap_or_default()
}

#[cfg(windows)]
extern "C" {
    pub fn GetLogicalDrives() -> u32;
}

#[cfg(windows)]
fn get_drives() -> Vec<PathBuf> {
    let mut drive_names = Vec::new();
    let mut drives = unsafe { GetLogicalDrives() };
    let mut letter = b'A';
    while drives > 0 {
        if drives & 1 != 0 {
            drive_names.push(format!("{}:\\", letter as char).into());
        }
        drives >>= 1;
        letter += 1;
    }
    drive_names
}

fn read_folder(path: &Path) -> Result<Vec<FileInfo>, Error> {
    fs::read_dir(path).map(|entries| {
        let mut file_infos: Vec<FileInfo> = entries
            .filter_map(|result| result.ok())
            .filter_map(|entry| {
                let info = FileInfo::new(entry.path());
                if !info.dir {
                    // Do not show system files.
                    if !info.path.is_file() {
                        return None;
                    }
                }

                #[cfg(unix)]
                if info.get_file_name().starts_with('.') {
                    return None;
                }

                Some(info)
            })
            .collect();

        // Sort keeping folders before files.
        file_infos.sort_by(|a, b| match a.dir == b.dir {
            true => a.path.file_name().cmp(&b.path.file_name()),
            false => b.dir.cmp(&a.dir),
        });

        #[cfg(windows)]
        let file_infos = {
            let drives = get_drives();
            let mut infos = Vec::with_capacity(drives.len() + file_infos.len());
            for drive in drives {
                infos.push(FileInfo { path: drive, dir: true });
            }
            infos.append(&mut file_infos);
            infos
        };

        file_infos
    })
}

#[derive(Clone, Debug, Default)]
pub struct FileInfo {
    pub path: PathBuf,
    pub dir: bool,
}

impl FileInfo {
    pub fn new(path: PathBuf) -> Self {
        let dir = path.is_dir();
        Self { path, dir }
    }

    pub fn get_file_name(&self) -> &str {
        #[cfg(windows)]
        if self.dir && is_drive_root(&self.path) {
            return self.path.to_str().unwrap_or_default();
        }
        self.path.file_name().and_then(|name| name.to_str()).unwrap_or_default()
    }
}
