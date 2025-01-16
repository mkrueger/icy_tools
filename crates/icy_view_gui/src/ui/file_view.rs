use directories::UserDirs;
use eframe::{
    egui::{self, Layout, RichText, Sense, TopBottomPanel, WidgetText},
    epaint::{FontFamily, FontId, Rounding},
};
use egui::{ScrollArea, TextEdit, Ui};
use i18n_embed_fl::fl;
use wildcard::Wildcard;

use core::f32;
use std::{env, path::PathBuf};

use crate::{Item, ItemFolder};

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
pub struct FileView {
    /// Selected file path
    pub selected_file: Option<usize>,
    pub scroll_pos: Option<usize>,
    /// Files in directory.
    pub parents: Vec<Box<dyn Item>>,
    pub files: Vec<Box<dyn Item>>,
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
        let mut folder = ItemFolder::new(path);
        folder.include_16colors = true;
        Self {
            selected_file: None,
            pre_select_file,
            scroll_pos: None,
            files: folder.get_subitems().unwrap_or_default(),
            parents: vec![Box::new(folder)],
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
            let response = ui.button("🗙").on_hover_text(fl!(crate::LANGUAGE_LOADER, "tooltip-reset-filter-button"));
            if response.clicked() {
                self.filter.clear();
            }

            ui.add_sized(
                [200.0, ui.style().spacing.interact_size.y],
                TextEdit::singleline(&mut self.filter).hint_text(fl!(crate::LANGUAGE_LOADER, "filter-entries-hint-text")),
            );

            if let Some(ver) = &self.upgrade_version {
                ui.hyperlink_to(
                    fl!(crate::LANGUAGE_LOADER, "menu-upgrade_version", version = ver.clone()),
                    "https://github.com/mkrueger/icy_tools/releases",
                );
            }

            ui.add_enabled_ui(
                self.parents.len() > 1 || self.parents.last().unwrap().get_file_path().parent().is_some(),
                |ui| {
                    let response = ui.button("⬆").on_hover_text("Parent Folder");
                    if response.clicked() {
                        command = Some(Message::ParentFolder);
                    }
                },
            );

            let response = ui.button("⟲").on_hover_text(fl!(crate::LANGUAGE_LOADER, "tooltip-refresh"));
            if response.clicked() {
                command = Some(Message::Refresh);
            }

            ui.menu_button("…", |ui| {
                let r = ui.hyperlink_to(
                    fl!(crate::LANGUAGE_LOADER, "menu-item-discuss"),
                    "https://github.com/mkrueger/icy_tools/discussions",
                );
                if r.clicked() {
                    ui.close_menu();
                }
                let r = ui.hyperlink_to(
                    fl!(crate::LANGUAGE_LOADER, "menu-item-report-bug"),
                    "https://github.com/mkrueger/icy_tools/issues/new",
                );
                if r.clicked() {
                    ui.close_menu();
                }
                let r = ui.hyperlink_to(
                    fl!(crate::LANGUAGE_LOADER, "menu-item-check-releases"),
                    "https://github.com/mkrueger/icy_tools/releases",
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

        ui.horizontal(|ui| match self.parents.last() {
            Some(path) => {
                let mut path_edit = path.get_file_path().to_string_lossy().to_string();
                let response = ui.add(TextEdit::singleline(&mut path_edit).desired_width(f32::INFINITY));
                if response.changed() {
                    let path = path_edit.parse::<PathBuf>().unwrap();
                    self.parents.push(Box::new(ItemFolder::new(path)));
                    command = self.refresh();
                }
            }
            None => {
                ui.colored_label(ui.style().visuals.error_fg_color, fl!(crate::LANGUAGE_LOADER, "error-invalid-path"));
            }
        });
        if self.selected_file.is_none() && !self.files.is_empty() {
            //  command = Some(Command::Select(0));
        }

        let area = ScrollArea::vertical();
        let row_height = ui.text_style_height(&egui::TextStyle::Body);
        let strong_color = ui.style().visuals.strong_text_color();
        let text_color = ui.style().visuals.text_color();

        let use_casing = self.filter.chars().any(|f| f.is_uppercase());
        let mut filter = self.filter.clone();

        if !(filter.ends_with('*') || filter.ends_with('*')) {
            filter.push('*');
        }

        let wildcard = Wildcard::new(filter.as_bytes()).unwrap();

        let filtered_entries = self.files.iter_mut().enumerate().filter(|(_, p)| {
            if self.filter.is_empty() {
                return true;
            }
            if use_casing {
                wildcard.is_match(p.get_label().as_bytes())
            } else {
                wildcard.is_match(p.get_label().to_lowercase().as_bytes())
            }
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
                    entry.get_label()
                } else {
                    if let Some(icon) = entry.get_icon() {
                        format!("{} {}", icon, entry.get_label())
                    } else {
                        entry.get_label()
                    }
                };

                let font_id = FontId::new(14.0, FontFamily::Proportional);
                let text: WidgetText = label.into();
                let galley = text.into_galley(ui, Some(egui::TextWrapMode::Truncate), f32::INFINITY, font_id);
                ui.painter()
                    .galley_with_override_text_color(egui::Align2::LEFT_TOP.align_size_within_rect(galley.size(), rect).min, galley, text_color);
                if response.hovered() {
                    if let Some(sauce) = &entry.get_sauce() {
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
    pub fn set_path(&mut self, path: impl Into<PathBuf>, include_16colors: bool) -> Option<Message> {
        let mut folder = ItemFolder::new(path.into());
        folder.include_16colors = include_16colors;
        self.parents.push(Box::new(folder));
        self.refresh()
    }

    pub fn refresh(&mut self) -> Option<Message> {
        self.files.clear();
        if let Some(items) = self.parents.last_mut().unwrap().get_subitems() {
            self.files = items;
        }
        self.selected_file = None;
        if let Some(file) = &self.pre_select_file {
            for (i, entry) in self.files.iter().enumerate() {
                if entry.get_label() == *file {
                    return Message::Select(i, false).into();
                }
            }
        }
        None
    }
}
