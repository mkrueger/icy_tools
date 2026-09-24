use super::{
    browser::{Browser, Location},
    colors,
    dialogs::{Dialogs, Mode, COMMANDS},
    icons::{Icon, Icons},
    preview::Preview,
    text,
    thumbnails::Thumbnails,
};
use eframe::egui;
use icy_engine_gui::{
    egui::{appearance, shortcuts},
    ScalingMode,
};
use icy_view::{
    items::{NavPoint, ProviderType},
    Options, ScrollSpeed, ViewMode,
};
use std::{path::PathBuf, time::Duration};

pub struct Viewer {
    pub browser: Browser,
    pub preview: Preview,
    pub options: Options,
    pub dialogs: Dialogs,
    icons: Icons,
    thumbnails: Thumbnails,
    pub(super) tiles: super::tile_grid::TileGrid,
    folder_tiles: super::tile_grid::TileGrid,
    pub(super) tile_toolbar: super::tile_toolbar::AutoHide,
    file_list: super::file_list::FileList,
    folder: Option<Browser>,
    path_input: String,
    revision: u64,
    show_preview: bool,
    focus_filter: bool,
    fullscreen: bool,
    shuffle: Option<super::shuffle::Shuffle>,
    commands: icy_engine_gui::CommandSet,
    selected_scroll: bool,
    browser_focus: bool,
}

impl Viewer {
    pub fn new(path: PathBuf, options: Options, context: &egui::Context) -> anyhow::Result<Self> {
        let mut options = options;
        options.monitor_settings.scaling_mode = super::preview::viewer_scaling(&options.monitor_settings);
        let mut browser = Browser::new(path, options.sort_order)?;
        browser.refresh(context);
        let path_input = browser.location.point.path.clone();
        Ok(Self {
            browser,
            preview: Preview::new(context)?,
            options,
            dialogs: Dialogs::default(),
            icons: Icons::default(),
            thumbnails: Thumbnails::new(context),
            tiles: Default::default(),
            folder_tiles: Default::default(),
            tile_toolbar: Default::default(),
            file_list: super::file_list::FileList::new(context),
            folder: None,
            path_input,
            revision: 0,
            show_preview: false,
            focus_filter: false,
            fullscreen: false,
            shuffle: None,
            commands: icy_view::commands::create_icy_view_commands(),
            selected_scroll: false,
            browser_focus: true,
        })
    }

    pub fn show(&mut self, context: &egui::Context) {
        let blocked = self.dialogs.mode.is_some() || self.dialogs.error.is_some() || self.browser.error.is_some();
        if self.revision != self.browser.revision() {
            self.revision = self.browser.revision();
            self.path_input = self.browser.location.point.path.clone();
            self.thumbnails.clear(context);
            self.folder = None;
            self.preview.stop();
            self.shuffle = None;
        }
        if let Some((path, data)) = self.browser.poll(context) {
            self.preview.load(path.clone(), data, self.options.auto_scroll_enabled, context);
            if let Some(shuffle) = &mut self.shuffle {
                shuffle.set_sauce(self.preview.sauce.as_ref());
            }
            context.send_viewport_cmd(egui::ViewportCommand::Title(format!(
                "{} - Icy View",
                PathBuf::from(path).file_name().unwrap_or_default().to_string_lossy()
            )));
        }
        if let Some(folder) = &mut self.folder {
            let _ = folder.poll(context);
        }
        self.preview.poll(context);
        self.thumbnails.poll(context);
        if let Some(error) = self.preview.error.take() {
            self.dialogs.error = Some(error);
            self.shuffle = None;
        }
        if !blocked && !context.will_discard() {
            self.keyboard(context);
        }
        if !blocked {
            let paths: Vec<_> = context.input(|input| input.raw.dropped_files.iter().filter_map(|file| file.path.clone()).collect());
            if let Some(path) = paths.first() {
                self.open_path(path.clone(), context);
            }
        }
        let narrow = context.content_rect().width() < 680.0;
        let style = context.style();
        egui::TopBottomPanel::top("navigation")
            .frame(egui::Frame::side_top_panel(&style).inner_margin(egui::Margin::symmetric(8, 6)))
            .show(context, |ui| {
                ui.add_enabled_ui(!blocked, |ui| self.toolbar(ui, narrow));
            });
        let status_fill = style.visuals.panel_fill.lerp_to_gamma(style.visuals.faint_bg_color, 0.45);
        egui::TopBottomPanel::bottom("status")
            .frame(
                egui::Frame::side_top_panel(&style)
                    .fill(status_fill)
                    .inner_margin(egui::Margin::symmetric(10, 3)),
            )
            .show(context, |ui| {
                ui.add_enabled_ui(!blocked, |ui| self.status(ui, narrow));
            });
        let preview_only = self.shuffle.is_some() || (self.show_preview && (narrow || self.options.view_mode == ViewMode::Tiles));
        if !narrow && !preview_only && self.options.view_mode == ViewMode::List {
            egui::SidePanel::left(egui::Id::new(("files", self.options.sauce_mode)))
                .resizable(true)
                .default_width(if self.options.sauce_mode { 906.0 } else { 300.0 })
                .width_range(200.0..=(context.content_rect().width() - 200.0))
                .show(context, |ui| {
                    ui.add_enabled_ui(!blocked, |ui| self.list(ui));
                });
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(context.style().visuals.panel_fill))
            .show(context, |ui| {
                ui.add_enabled_ui(!blocked, |ui| {
                    let viewing_preview = preview_only || (!narrow && self.options.view_mode == ViewMode::List && self.folder.is_none());
                    if viewing_preview && ui.rect_contains_pointer(ui.max_rect()) && ui.input(|input| input.pointer.any_pressed()) {
                        self.browser_focus = false;
                        self.shuffle = None;
                    }
                    if preview_only {
                        self.preview.show(ui, &self.options);
                        if let Some(shuffle) = &mut self.shuffle {
                            let rect = ui.max_rect();
                            shuffle.show(ui, rect);
                        }
                    } else if self.options.view_mode == ViewMode::Tiles {
                        egui::Frame::new().inner_margin(10).show(ui, |ui| self.grid(ui, false));
                    } else if narrow {
                        egui::Frame::new().inner_margin(8).show(ui, |ui| self.list(ui));
                    } else if self.folder.is_some() {
                        egui::Frame::new().inner_margin(10).show(ui, |ui| self.grid(ui, true));
                    } else {
                        self.preview.show(ui, &self.options);
                    }
                });
            });
        self.dialogs.show(context, &mut self.options, &mut self.preview);
        if self.dialogs.mode.is_none() {
            if let Some(error) = self.dialogs.error.clone().or_else(|| self.browser.error.clone()) {
                let response = appearance::MessageBox::new("viewer-error", appearance::MessageKind::Error, text("preview-error-title"), error)
                    .copyable()
                    .buttons([appearance::DialogButton::primary(appearance::labels::close(), ()).cancels()])
                    .show(context);
                if response.action.is_some() || response.dismissed {
                    self.dialogs.error = None;
                    self.browser.error = None;
                }
            }
        }
        if self.shuffle.is_some() && !blocked && !self.preview.loading && !self.browser.preview_loading {
            let at_end = self.preview.screen.offset.y >= self.preview.screen.max_offset.y - 1.0;
            let advance = self
                .shuffle
                .as_mut()
                .map(|shuffle| {
                    if at_end {
                        shuffle.notify_scrolled();
                    }
                    shuffle.should_advance()
                })
                .unwrap_or_default();
            if advance {
                self.shuffle_next(context);
            }
        }
        if self.shuffle.is_some() {
            context.request_repaint_after(Duration::from_millis(100));
        }
        if self.options.view_mode == ViewMode::Tiles && !preview_only {
            if let Some(remaining) = self.tile_toolbar.update() {
                context.request_repaint_after(remaining);
            }
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui, narrow: bool) {
        let context = ui.ctx().clone();
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.horizontal(|ui| {
            self.navigation_buttons(ui, &context);
            ui.add_space(4.0);
            if narrow {
                self.location_field(ui, ui.available_width(), &context);
                return;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                self.view_buttons(ui, &context);
                ui.add_space(4.0);
                let filter = (ui.available_width() * 0.3).clamp(140.0, 240.0);
                self.filter_field(ui, filter, &context);
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    self.location_field(ui, ui.available_width(), &context);
                });
            });
        });
        if narrow {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    self.view_buttons(ui, &context);
                    ui.add_space(4.0);
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        self.filter_field(ui, ui.available_width(), &context);
                    });
                });
            });
        }
    }

    fn navigation_buttons(&mut self, ui: &mut egui::Ui, context: &egui::Context) {
        let return_to_list =
            self.show_preview && (ui.ctx().content_rect().width() < 680.0 || self.options.view_mode == ViewMode::Tiles) || self.shuffle.is_some();
        if self
            .icons
            .button(ui, Icon::Back, &text("tooltip-back"), return_to_list || !self.browser.back.is_empty(), false)
            .clicked()
        {
            if return_to_list {
                self.show_preview = false;
                self.shuffle = None;
            } else {
                self.browser.history(false, context);
            }
        }
        if self
            .icons
            .button(ui, Icon::Forward, &text("tooltip-forward"), !self.browser.forward.is_empty(), false)
            .clicked()
        {
            self.browser.history(true, context);
        }
        if self
            .icons
            .button(ui, Icon::Up, &text("tooltip-up"), self.browser.location.point.can_navigate_up(), false)
            .clicked()
        {
            self.browser.up(context);
        }
        if self.icons.button(ui, Icon::Refresh, &text("tooltip-refresh"), true, false).clicked() {
            self.browser.refresh(context);
        }
    }

    /// Address field with the 16colors toggle in front of the path, like the location bar of a browser.
    fn location_field(&mut self, ui: &mut egui::Ui, width: f32, context: &egui::Context) {
        let id = egui::Id::new("location-input");
        let web = self.browser.location.point.is_web();
        let mut toggle_web = false;
        let response = field(ui, width, id, |ui| {
            let image = self.icons.image(context, Icon::Web, 16.0);
            toggle_web = ui
                .scope(|ui| {
                    super::icons::compact(ui);
                    ui.spacing_mut().button_padding = egui::vec2(4.0, 3.0);
                    ui.add(super::icons::tool_button(image, web).min_size(egui::vec2(24.0, 22.0)))
                })
                .inner
                .on_hover_text(text("tooltip-browse-16colors"))
                .clicked();
            ui.add(
                egui::TextEdit::singleline(&mut self.path_input)
                    .id(id)
                    .frame(false)
                    .margin(egui::vec2(2.0, 4.0))
                    .desired_width(f32::INFINITY),
            )
        });
        if toggle_web {
            let point = if web {
                NavPoint::file(std::env::current_dir().unwrap_or_default().to_string_lossy())
            } else {
                NavPoint::web("")
            };
            self.browser.navigate(Location { point, container: None }, context);
        }
        if response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
            if web {
                self.browser.navigate(
                    Location {
                        point: NavPoint::web(self.path_input.trim_matches('/')),
                        container: None,
                    },
                    context,
                );
            } else {
                self.open_path(PathBuf::from(&self.path_input), context);
            }
        }
    }

    fn filter_field(&mut self, ui: &mut egui::Ui, width: f32, context: &egui::Context) {
        let id = egui::Id::new("filter-input");
        let response = field(ui, width, id, |ui| {
            ui.add_space(4.0);
            let tint = ui.visuals().weak_text_color();
            ui.add(self.icons.image(context, Icon::Search, 16.0).tint(tint));
            ui.add(
                egui::TextEdit::singleline(&mut self.browser.filter)
                    .id(id)
                    .frame(false)
                    .margin(egui::vec2(2.0, 4.0))
                    .hint_text(text("filter-entries-hint-text"))
                    .desired_width(f32::INFINITY),
            )
        });
        if self.focus_filter {
            response.request_focus();
            self.focus_filter = false;
        }
        if response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
            if let Some(index) = self.browser.visible().first().copied() {
                self.activate(index, true, context);
            }
        }
    }

    /// Right-to-left: the menu, the slideshow toggle and the list/tiles switch.
    fn view_buttons(&mut self, ui: &mut egui::Ui, context: &egui::Context) {
        let menu = super::icons::tool_button(self.icons.image(context, Icon::Menu, 18.0), false);
        ui.scope(|ui| {
            super::icons::compact(ui);
            egui::containers::menu::MenuButton::from_button(menu).ui(ui, |ui| self.menu(ui)).0
        })
        .inner
        .on_hover_text(text("egui-menu"));
        if self
            .icons
            .button(
                ui,
                Icon::Shuffle,
                &text("tooltip-shuffle-mode"),
                !self.browser.items.is_empty(),
                self.shuffle.is_some(),
            )
            .clicked()
        {
            if self.shuffle.is_some() {
                self.shuffle = None;
            } else {
                self.shuffle_start(context);
            }
        }
        ui.add_space(2.0);
        let tiles = self.options.view_mode == ViewMode::Tiles;
        egui::Frame::new()
            .fill(ui.visuals().faint_bg_color)
            .corner_radius(7)
            .inner_margin(1)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                if self.icons.button(ui, Icon::Tiles, &text("tooltip-view-mode-tiles"), true, tiles).clicked() && !tiles {
                    self.options.view_mode = ViewMode::Tiles;
                    self.show_preview = false;
                    self.tile_toolbar.reset();
                }
                if self.icons.button(ui, Icon::List, &text("tooltip-view-mode-list"), true, !tiles).clicked() {
                    self.options.view_mode = ViewMode::List;
                    self.show_preview = false;
                }
            });
    }

    fn menu(&mut self, ui: &mut egui::Ui) {
        let context = ui.ctx().clone();
        for id in [
            "file.open",
            "dialog.export",
            "dialog.sauce",
            "edit.copy",
            "settings.open",
            "view.fullscreen",
            "help.show",
            "help.about",
            "window.new",
            "window.close",
        ] {
            let command = self.commands.get(id).unwrap();
            let enabled = !matches!(id, "dialog.export" | "dialog.sauce" | "edit.copy") || !self.preview.file.is_empty();
            if ui
                .add_enabled(
                    enabled,
                    egui::Button::new(text(&command.fluent_action_key())).shortcut_text(command.primary_hotkey_display().unwrap_or_default()),
                )
                .clicked()
            {
                ui.close();
                self.action(id, &context);
            }
        }
        ui.separator();
        ui.checkbox(&mut self.options.sauce_mode, text("tooltip-sauce-mode-on"));
        ui.menu_button(text("egui-sort"), |ui| {
            for (order, key) in [
                (icy_view::sort_order::SortOrder::NameAsc, "tooltip-sort-name-asc"),
                (icy_view::sort_order::SortOrder::NameDesc, "tooltip-sort-name-desc"),
                (icy_view::sort_order::SortOrder::SizeAsc, "tooltip-sort-size-asc"),
                (icy_view::sort_order::SortOrder::SizeDesc, "tooltip-sort-size-desc"),
                (icy_view::sort_order::SortOrder::DateAsc, "tooltip-sort-date-asc"),
                (icy_view::sort_order::SortOrder::DateDesc, "tooltip-sort-date-desc"),
            ] {
                if ui.selectable_label(self.options.sort_order == order, text(key)).clicked() {
                    self.sort(order);
                    ui.close();
                }
            }
        });
        if ui.button(text("button-open")).on_hover_text(text("egui-open-folder")).clicked() {
            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                self.open_path(path, &context);
            }
            ui.close();
        }
        egui::widgets::global_theme_preference_buttons(ui);
    }

    fn list(&mut self, ui: &mut egui::Ui) {
        let context = ui.ctx().clone();
        self.browse_controls(ui);
        ui.add_space(2.0);
        let response = self
            .file_list
            .show(ui, &self.browser, &mut self.icons, self.options.sauce_mode, self.selected_scroll);
        self.selected_scroll = false;
        if let Some(order) = response.sort {
            self.sort(order);
        }
        if let Some((index, enter)) = response.activate {
            self.browser_focus = true;
            self.activate(index, enter, &context);
        }
    }

    /// Sort controls on the left, the loading spinner and the SAUCE column toggle on the right.
    fn browse_controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            self.sort_controls(ui);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let sauce = self.options.sauce_mode;
                let tooltip = text(if sauce { "tooltip-sauce-mode-off" } else { "tooltip-sauce-mode-on" });
                if pill(ui, sauce, "SAUCE", 22.0).on_hover_text(tooltip).clicked() {
                    self.options.sauce_mode = !sauce;
                }
                if self.browser.loading {
                    ui.add(egui::Spinner::new().size(14.0));
                }
            });
        });
    }

    fn sort_controls(&mut self, ui: &mut egui::Ui) {
        use icy_view::sort_order::SortOrder;
        let icon = match self.browser.sort {
            SortOrder::NameAsc | SortOrder::NameDesc => Icon::SortName,
            SortOrder::SizeAsc | SortOrder::SizeDesc => Icon::SortSize,
            SortOrder::DateAsc | SortOrder::DateDesc => Icon::SortDate,
        };
        let button = super::icons::tool_button(self.icons.image(ui.ctx(), icon, 18.0), false);
        ui.scope(|ui| {
            super::icons::compact(ui);
            egui::containers::menu::MenuButton::from_button(button)
                .ui(ui, |ui| {
                    for (order, key) in [
                        (SortOrder::NameAsc, "tooltip-sort-name-asc"),
                        (SortOrder::NameDesc, "tooltip-sort-name-desc"),
                        (SortOrder::SizeAsc, "tooltip-sort-size-asc"),
                        (SortOrder::SizeDesc, "tooltip-sort-size-desc"),
                        (SortOrder::DateAsc, "tooltip-sort-date-asc"),
                        (SortOrder::DateDesc, "tooltip-sort-date-desc"),
                    ] {
                        if ui.selectable_label(self.browser.sort == order, text(key)).clicked() {
                            self.sort(order);
                            ui.close();
                        }
                    }
                })
                .0
        })
        .inner
        .on_hover_text(text("egui-sort"));
        let ascending = matches!(self.browser.sort, SortOrder::NameAsc | SortOrder::SizeAsc | SortOrder::DateAsc);
        if self
            .icons
            .button(ui, if ascending { Icon::Up } else { Icon::Down }, &text("egui-sort"), true, false)
            .clicked()
        {
            let order = match self.browser.sort {
                SortOrder::NameAsc => SortOrder::NameDesc,
                SortOrder::NameDesc => SortOrder::NameAsc,
                SortOrder::SizeAsc => SortOrder::SizeDesc,
                SortOrder::SizeDesc => SortOrder::SizeAsc,
                SortOrder::DateAsc => SortOrder::DateDesc,
                SortOrder::DateDesc => SortOrder::DateAsc,
            };
            self.sort(order);
        }
    }

    /// Overlay toolbar of the tile view; it fades out and returns when the top left corner is touched.
    fn tile_toolbar(&mut self, ui: &mut egui::Ui, area: egui::Rect) {
        if !self.tile_toolbar.visible {
            let zone = egui::Rect::from_min_size(area.min, egui::Vec2::splat(super::tile_toolbar::HOVER_ZONE));
            if ui.rect_contains_pointer(zone) {
                self.tile_toolbar.hover(true);
            }
            return;
        }
        let context = ui.ctx().clone();
        let response = egui::Area::new("tile-toolbar".into())
            .order(egui::Order::Middle)
            .fixed_pos(area.min + egui::vec2(4.0, 4.0))
            .show(&context, |ui| {
                let visuals = ui.visuals();
                egui::Frame::new()
                    .fill(visuals.window_fill.gamma_multiply(0.96))
                    .stroke(visuals.window_stroke)
                    .corner_radius(8)
                    .shadow(egui::Shadow {
                        offset: [0, 2],
                        blur: 10,
                        spread: 0,
                        color: egui::Color32::from_black_alpha(if visuals.dark_mode { 110 } else { 40 }),
                    })
                    .inner_margin(4)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 2.0;
                            self.tile_controls(ui)
                        });
                    });
            })
            .response;
        self.tile_toolbar.rect = response.rect;
        let hovered = context.rect_contains_pointer(response.layer_id, response.rect) || egui::Popup::is_any_open(&context);
        self.tile_toolbar.hover(hovered);
    }

    fn tile_controls(&mut self, ui: &mut egui::Ui) {
        let context = ui.ctx().clone();
        if self
            .icons
            .button(ui, Icon::Up, &text("tooltip-up"), self.browser.location.point.can_navigate_up(), false)
            .clicked()
        {
            self.browser.up(&context);
        }
        ui.separator();
        self.sort_controls(ui);
    }

    fn sort(&mut self, order: icy_view::sort_order::SortOrder) {
        self.options.sort_order = order;
        self.browser.sort = order;
        let selected = self.browser.location.point.selected_item.clone();
        icy_view::items::sort_items(&mut self.browser.items, order);
        self.browser.selected = self.browser.items.iter().position(|item| Some(item.get_label()) == selected);
        self.selected_scroll = true;
        self.browser_focus = true;
        if let Some(folder) = &mut self.folder {
            folder.sort = order;
            icy_view::items::sort_items(&mut folder.items, order);
        }
    }

    fn grid(&mut self, ui: &mut egui::Ui, folder: bool) {
        let context = ui.ctx().clone();
        let area = ui.max_rect();
        let source = if folder { self.folder.as_ref().unwrap() } else { &self.browser };
        if source.loading {
            ui.spinner();
        }
        let grid = if folder { &mut self.folder_tiles } else { &mut self.tiles };
        let activate = grid.show(ui, source, &mut self.thumbnails, &mut self.icons, !folder && self.selected_scroll);
        if !folder {
            self.selected_scroll = false;
            self.tile_toolbar(ui, area);
        }
        if let Some((index, enter)) = activate {
            self.browser_focus = true;
            if folder {
                let folder = self.folder.as_ref().unwrap();
                let mut location = folder.location.clone();
                location.point.selected_item = Some(folder.items[index].get_label());
                self.browser.navigate(location, &context);
                self.folder = None;
            } else {
                self.activate(index, enter, &context);
                if !enter {
                    self.show_preview = false;
                }
            }
        }
    }

    fn activate(&mut self, index: usize, enter: bool, context: &egui::Context) {
        let Some(item) = self.browser.items.get(index) else {
            return;
        };
        let container = item.is_container();
        if container && enter {
            self.browser.enter(index, context);
            self.show_preview = false;
            return;
        }
        if container {
            if let Ok(mut folder) = Browser::new(std::env::current_dir().unwrap_or_default(), self.options.sort_order) {
                let path = item
                    .get_full_path()
                    .unwrap_or_else(|| format!("{}/{}", self.browser.location.point.path.trim_end_matches('/'), item.get_file_path()));
                folder.location = Location {
                    point: NavPoint {
                        provider_type: self.browser.location.point.provider_type,
                        path,
                        selected_item: None,
                    },
                    container: Some(item.clone()),
                };
                folder.refresh(context);
                self.folder = Some(folder);
            }
        } else {
            self.folder = None;
        }
        self.browser.select(index, context);
        self.preview.stop();
        if !container && (enter || context.content_rect().width() < 680.0) {
            self.show_preview = true;
        }
    }

    /// File information on the left, playback and zoom controls on the right.
    fn status(&mut self, ui: &mut egui::Ui, narrow: bool) {
        ui.spacing_mut().item_spacing.x = 6.0;
        if narrow {
            ui.horizontal(|ui| self.status_info(ui));
        }
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                self.status_controls(ui);
                if !narrow {
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| self.status_info(ui));
                }
            });
        });
    }

    fn status_info(&mut self, ui: &mut egui::Ui) {
        if self.preview.loading || self.browser.loading {
            ui.add(egui::Spinner::new().size(14.0));
        }
        self.file_info(ui);
    }

    /// Right-to-left: zoom, baud rate, auto scroll and the position in the folder.
    fn status_controls(&mut self, ui: &mut egui::Ui) {
        let small = |value: String| egui::RichText::new(value).size(12.0);
        let zoom = match self.options.monitor_settings.scaling_mode {
            ScalingMode::Manual(zoom) => format!("{:.0}%", zoom * 100.0),
            _ => text("egui-zoom-fit"),
        };
        status_menu(ui, small(zoom), &text("egui-zoom"), |ui| {
            if ui
                .selectable_label(
                    matches!(self.options.monitor_settings.scaling_mode, ScalingMode::FitWidth),
                    text("cmd-view-zoom_fit-action"),
                )
                .clicked()
            {
                self.options.monitor_settings.scaling_mode = ScalingMode::FitWidth;
            }
            ui.separator();
            for zoom in [0.5, 1.0, 1.5, 2.0, 3.0, 4.0] {
                let selected = matches!(self.options.monitor_settings.scaling_mode, ScalingMode::Manual(current) if current == zoom);
                if ui.selectable_label(selected, format!("{:.0}%", zoom * 100.0)).clicked() {
                    self.options.monitor_settings.scaling_mode = ScalingMode::Manual(zoom);
                }
            }
        });
        let baud = if self.preview.baud == 0 {
            small("BPS: OFF".into()).color(ui.visuals().weak_text_color())
        } else {
            small(format!("{} BPS", self.preview.baud)).color(ui.visuals().hyperlink_color)
        };
        status_menu(ui, baud, &text("egui-baud-emulation"), |ui| {
            for rate in BAUD_RATES {
                if ui
                    .selectable_label(self.preview.baud == *rate, if *rate == 0 { "OFF".into() } else { format!("{rate} BPS") })
                    .clicked()
                {
                    self.preview.set_baud(*rate);
                }
            }
        });
        let scroll = self.options.auto_scroll_enabled;
        if pill(ui, scroll, &text("egui-auto-scroll"), 20.0)
            .on_hover_text(text(if scroll { "toast-auto-scroll-on" } else { "toast-auto-scroll-off" }))
            .clicked()
        {
            self.options.auto_scroll_enabled = !scroll;
        }
        ui.separator();
        let position = format!("{} / {}", self.browser.selected.map_or(0, |index| index + 1), self.browser.items.len());
        ui.label(small(position).color(ui.visuals().weak_text_color()));
    }

    /// Colour-coded SAUCE summary of the shown file; clicking it opens the SAUCE dialog.
    fn file_info(&mut self, ui: &mut egui::Ui) {
        let dark = ui.visuals().dark_mode;
        let palette = colors::Sauce::new(dark);
        let sauce = self.preview.sauce.clone();
        let mut job = if self.preview.file.is_empty() {
            egui::text::LayoutJob::default()
        } else {
            let screen = self.preview.screen.terminal.screen.lock();
            let buffer = (screen.width(), screen.height());
            drop(screen);
            sauce_summary(dark, sauce.as_ref(), Some(self.preview.content_size), Some(buffer))
        };
        if job.text.is_empty() {
            if let Some(item) = self.browser.selected.and_then(|index| self.browser.items.get(index)) {
                let label = item.get_label();
                if let Some(icy_engine::formats::FileFormat::Archive(format)) = icy_engine::formats::FileFormat::from_path(std::path::Path::new(&label)) {
                    append(
                        &mut job,
                        &format!("Archive: {}", icy_engine::formats::FileFormat::Archive(format).name()),
                        palette.title,
                        palette.separator,
                    );
                } else {
                    append(&mut job, &label, palette.date, palette.separator);
                }
                if let Some(size) = item.size() {
                    append(&mut job, &format_size(size), palette.size, palette.separator);
                }
            }
        }
        if job.text.is_empty() {
            append(&mut job, &text("statusbar-ready"), palette.separator, palette.separator);
        }
        let background = ui.painter().add(egui::Shape::Noop);
        let response =
            ui.add(
                egui::Label::new(job)
                    .truncate()
                    .selectable(false)
                    .sense(if sauce.is_some() { egui::Sense::click() } else { egui::Sense::hover() }),
            );
        if sauce.is_some() {
            if response.hovered() {
                ui.painter().set(
                    background,
                    egui::Shape::rect_filled(response.rect.expand2(egui::vec2(6.0, 3.0)), 4.0, ui.visuals().widgets.hovered.weak_bg_fill),
                );
            }
            if response
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(text("cmd-dialog-sauce-action"))
                .clicked()
            {
                self.dialogs.open(Mode::Sauce, &self.options, &self.preview);
            }
        }
    }

    fn keyboard(&mut self, context: &egui::Context) {
        let editing = context.wants_keyboard_input();
        let actions: Vec<_> = COMMANDS
            .iter()
            .copied()
            .filter(|id| !editing || matches!(*id, "help.show" | "view.fullscreen"))
            .filter(|id| self.commands.get(id).is_some_and(|command| shortcuts::consume(context, command)))
            .collect();
        for action in actions {
            self.action(action, context);
        }
        if editing {
            return;
        }
        if context.input_mut(|input| {
            let copy = input.events.iter().any(|event| matches!(event, egui::Event::Copy));
            input.events.retain(|event| !matches!(event, egui::Event::Copy));
            copy
        }) {
            self.preview.copy(context);
        }
        if context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            self.show_preview = false;
            self.shuffle = None;
        }
        if self.shuffle.is_some() && context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Space)) {
            self.shuffle_next(context);
        }
        if context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)) {
            if self.shuffle.is_some() {
                self.shuffle_next(context);
            } else if let Some(index) = self.browser.selected {
                self.activate(index, true, context);
            }
        }
        if context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Backspace)) {
            self.browser.up(context);
        }
        for (key, step) in [
            (egui::Key::ArrowDown, 1isize),
            (egui::Key::ArrowUp, -1),
            (egui::Key::ArrowRight, 1),
            (egui::Key::ArrowLeft, -1),
        ] {
            if context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, key)) {
                let visible = self.browser.visible();
                let current = visible.iter().position(|index| Some(*index) == self.browser.selected);
                let next = current.map_or(0, |index| (index as isize + step).clamp(0, visible.len().saturating_sub(1) as isize) as usize);
                let tile_navigation = self.options.view_mode == ViewMode::Tiles && !self.show_preview;
                let index = if tile_navigation {
                    self.tiles.navigate(self.browser.selected, key)
                } else {
                    visible.get(next).copied()
                };
                if let Some(index) = index {
                    self.activate(index, false, context);
                    if tile_navigation {
                        self.show_preview = false;
                    }
                    self.selected_scroll = true;
                    self.browser_focus = true;
                }
            }
        }
        for (key, offset) in [
            (egui::Key::PageDown, 400.0),
            (egui::Key::PageUp, -400.0),
            (egui::Key::Home, -f32::MAX),
            (egui::Key::End, f32::MAX),
        ] {
            if context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, key)) {
                if self.browser_focus && !self.show_preview && self.shuffle.is_none() {
                    let visible = self.browser.visible();
                    let index = if self.options.view_mode == ViewMode::Tiles {
                        self.tiles.navigate(self.browser.selected, key)
                    } else {
                        let page = (self.file_list.viewport_height / super::file_list::ROW_HEIGHT).floor().max(1.0) as usize;
                        let current = visible.iter().position(|index| Some(*index) == self.browser.selected).unwrap_or(0);
                        let next = match key {
                            egui::Key::Home => 0,
                            egui::Key::End => visible.len().saturating_sub(1),
                            egui::Key::PageUp => current.saturating_sub(page),
                            _ => (current + page).min(visible.len().saturating_sub(1)),
                        };
                        visible.get(next).copied()
                    };
                    if let Some(index) = index {
                        self.activate(index, false, context);
                        self.show_preview = false;
                        self.selected_scroll = true;
                    }
                } else {
                    self.preview.screen.scroll_to = Some(self.preview.screen.offset + egui::vec2(0.0, offset));
                }
            }
        }
    }

    pub fn action(&mut self, id: &str, context: &egui::Context) {
        match id {
            "file.open" => {
                if let Some(path) = rfd::FileDialog::new().pick_file() {
                    self.open_path(path, context);
                }
            }
            "nav.back" => self.browser.history(false, context),
            "nav.forward" => self.browser.history(true, context),
            "nav.up" => self.browser.up(context),
            "dialog.filter" => self.focus_filter = true,
            "settings.open" => self.dialogs.open(Mode::Settings, &self.options, &self.preview),
            "help.show" => self.dialogs.open(Mode::Help, &self.options, &self.preview),
            "help.about" => self.dialogs.open(Mode::About, &self.options, &self.preview),
            "dialog.sauce" => self.dialogs.open(Mode::Sauce, &self.options, &self.preview),
            "dialog.export" if !self.preview.file.is_empty() => self.dialogs.open(Mode::Export, &self.options, &self.preview),
            "edit.copy" => self.preview.copy(context),
            "edit.select_all" => self.preview.select_all(),
            "view.fullscreen" => {
                self.fullscreen = !self.fullscreen;
                context.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
            }
            "view.zoom_in" => {
                self.options.monitor_settings.scaling_mode = ScalingMode::Manual(ScalingMode::zoom_in(
                    self.preview.screen.zoom,
                    self.options.monitor_settings.use_integer_scaling,
                ))
            }
            "view.zoom_out" => {
                self.options.monitor_settings.scaling_mode = ScalingMode::Manual(ScalingMode::zoom_out(
                    self.preview.screen.zoom,
                    self.options.monitor_settings.use_integer_scaling,
                ))
            }
            "view.zoom_reset" => self.options.monitor_settings.scaling_mode = ScalingMode::Manual(1.0),
            "view.zoom_fit" => self.options.monitor_settings.scaling_mode = ScalingMode::FitWidth,
            "playback.toggle_scroll" => self.options.auto_scroll_enabled = !self.options.auto_scroll_enabled,
            "playback.scroll_speed" | "playback.scroll_speed_back" => {
                let index = match self.options.scroll_speed {
                    ScrollSpeed::Slow => 0,
                    ScrollSpeed::Medium => 1,
                    ScrollSpeed::Fast => 2,
                };
                self.options.scroll_speed = [ScrollSpeed::Slow, ScrollSpeed::Medium, ScrollSpeed::Fast][(index + if id.ends_with("back") { 2 } else { 1 }) % 3];
            }
            "playback.baud_rate_off" => self.preview.set_baud(0),
            "playback.baud_rate" | "playback.baud_rate_back" => {
                let index = BAUD_RATES.iter().position(|rate| *rate == self.preview.baud).unwrap_or(0);
                self.preview
                    .set_baud(BAUD_RATES[(index + if id.ends_with("back") { BAUD_RATES.len() - 1 } else { 1 }) % BAUD_RATES.len()]);
            }
            "window.close" | "app.quit" => context.send_viewport_cmd(egui::ViewportCommand::Close),
            "window.new" => {
                let result = std::env::current_exe().and_then(|program| {
                    std::process::Command::new(program)
                        .arg(&self.browser.location.point.path)
                        .arg("--config-dir")
                        .arg(icy_view::get_config_dir())
                        .spawn()
                });
                if let Err(error) = result {
                    self.dialogs.error = Some(error.to_string());
                }
            }
            id if id.starts_with("external.command_") => {
                if let Some(index) = id.rsplit('_').next().and_then(|part| part.parse().ok()) {
                    self.external(index);
                }
            }
            _ => {}
        }
    }

    fn external(&mut self, index: usize) {
        let Some(command) = self.options.external_commands.get(index) else {
            return;
        };
        if self.preview.file.is_empty() {
            return;
        }
        let result = (|| -> anyhow::Result<()> {
            let mut path = PathBuf::from(&self.preview.file);
            if !path.is_file() || self.browser.location.point.provider_type == ProviderType::Web {
                let directory = std::env::temp_dir().join(format!("icy_view_{}", std::process::id()));
                std::fs::create_dir_all(&directory)?;
                path = directory.join(format!("{}-{}", fastrand::u64(..), path.file_name().unwrap_or_default().to_string_lossy()));
                std::fs::write(&path, self.preview.data.as_ref())?;
            }
            let (program, args) = command
                .build_command(&path)
                .ok_or_else(|| anyhow::anyhow!("{}", text("egui-command-invalid")))?;
            std::process::Command::new(program).args(args).spawn()?;
            Ok(())
        })();
        if let Err(error) = result {
            self.dialogs.error = Some(error.to_string());
        }
    }

    fn open_path(&mut self, path: PathBuf, context: &egui::Context) {
        match Browser::new(path, self.options.sort_order) {
            Ok(browser) => self.browser.navigate(browser.location.clone(), context),
            Err(error) => self.dialogs.error = Some(error.to_string()),
        }
    }

    pub(super) fn shuffle_start(&mut self, context: &egui::Context) {
        let files = self
            .browser
            .visible()
            .into_iter()
            .filter(|index| !self.browser.items[*index].is_container())
            .collect();
        self.shuffle = super::shuffle::Shuffle::new(files);
        self.options.auto_scroll_enabled = true;
        let Some(index) = self.shuffle.as_ref().and_then(|shuffle| shuffle.current()) else {
            return;
        };
        self.play(index, context);
    }

    fn shuffle_next(&mut self, context: &egui::Context) {
        let Some(index) = self.shuffle.as_mut().and_then(|shuffle| shuffle.advance()) else {
            self.shuffle = None;
            return;
        };
        self.play(index, context);
    }

    fn play(&mut self, index: usize, context: &egui::Context) {
        self.preview.stop();
        self.browser.select(index, context);
        self.show_preview = true;
        if let Some(next) = self.shuffle.as_ref().and_then(|shuffle| shuffle.peek_next()) {
            self.browser.preload(next);
        }
    }
}

const BAUD_RATES: &[u32] = &[0, 300, 1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200];

fn append(job: &mut egui::text::LayoutJob, value: &str, color: egui::Color32, separator: egui::Color32) {
    let value = value.trim();
    if value.is_empty() {
        return;
    }
    let font = egui::FontId::proportional(12.0);
    if !job.text.is_empty() {
        job.append(
            " • ",
            0.0,
            egui::TextFormat {
                font_id: font.clone(),
                color: separator,
                ..Default::default()
            },
        );
    }
    job.append(
        value,
        0.0,
        egui::TextFormat {
            font_id: font,
            color,
            ..Default::default()
        },
    );
}

/// Rounded toggle chip, filled with the selection colour when on.
fn pill(ui: &mut egui::Ui, on: bool, label: &str, height: f32) -> egui::Response {
    ui.scope(|ui| {
        ui.spacing_mut().button_padding = egui::vec2(10.0, 2.0);
        let color = if on {
            ui.visuals().selection.stroke.color
        } else {
            ui.visuals().weak_text_color()
        };
        let button = egui::Button::new(egui::RichText::new(label).size(12.0).color(color))
            .selected(on)
            .corner_radius(height / 2.0)
            .min_size(egui::vec2(0.0, height));
        let response = ui.add(button);
        response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), on, label));
        response
    })
    .inner
}

/// Frameless drop-down for the status bar.
fn status_menu(ui: &mut egui::Ui, label: egui::RichText, tooltip: &str, content: impl FnOnce(&mut egui::Ui)) {
    ui.scope(|ui| {
        ui.spacing_mut().button_padding = egui::vec2(8.0, 2.0);
        let button = egui::Button::new(label)
            .right_text(egui::RichText::new("⏷").size(10.0))
            .frame_when_inactive(false)
            .stroke(egui::Stroke::NONE)
            .min_size(egui::vec2(0.0, 20.0));
        egui::containers::menu::MenuButton::from_button(button).ui(ui, content).0
    })
    .inner
    .on_hover_text(tooltip);
}

/// Rounded input surface holding an icon and a frameless text edit; the border follows the focus.
fn field(ui: &mut egui::Ui, width: f32, id: egui::Id, add: impl FnOnce(&mut egui::Ui) -> egui::Response) -> egui::Response {
    let focused = ui.memory(|memory| memory.has_focus(id));
    let visuals = ui.visuals().clone();
    let stroke = if focused {
        egui::Stroke::new(1.0, visuals.selection.stroke.color)
    } else {
        visuals.widgets.inactive.bg_stroke
    };
    ui.allocate_ui_with_layout(egui::vec2(width, 30.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
        egui::Frame::new()
            .fill(visuals.extreme_bg_color)
            .stroke(stroke)
            .corner_radius(6)
            .inner_margin(egui::Margin {
                left: 3,
                right: 6,
                top: 0,
                bottom: 0,
            })
            .show(ui, |ui| {
                ui.set_min_size(egui::vec2(ui.available_width(), 28.0));
                ui.spacing_mut().item_spacing.x = 2.0;
                add(ui)
            })
            .inner
    })
    .inner
}

/// Title, author, group, date, size and capabilities in the colours of the original status bar.
pub(super) fn sauce_summary(
    dark: bool,
    sauce: Option<&icy_sauce::SauceRecord>,
    content_size: Option<usize>,
    buffer: Option<(i32, i32)>,
) -> egui::text::LayoutJob {
    let palette = colors::Sauce::new(dark);
    let mut job = egui::text::LayoutJob::default();
    if let Some(sauce) = sauce {
        append(&mut job, &sauce.title().to_string(), palette.title, palette.separator);
        append(&mut job, &sauce.author().to_string(), palette.author, palette.separator);
        append(&mut job, &sauce.group().to_string(), palette.group, palette.separator);
        if let Some(date) = super::dialogs::sauce_date(sauce) {
            append(&mut job, &date, palette.date, palette.separator);
        }
    }
    if let Some(size) = content_size {
        append(&mut job, &format_size(size as u64), palette.date, palette.separator);
    }
    if let Some((width, height)) = buffer {
        append(&mut job, &format!("{width}×{height}"), palette.size, palette.separator);
    }
    if let Some(sauce) = sauce {
        append(&mut job, &capabilities(sauce), palette.size, palette.separator);
    }
    job
}

/// Short capability summary shown next to the SAUCE fields, as in the original status bar.
fn capabilities(sauce: &icy_sauce::SauceRecord) -> String {
    match sauce.capabilities() {
        Some(icy_sauce::Capabilities::Character(caps)) => {
            let mut parts = Vec::new();
            if caps.ice_colors {
                parts.push("iCE".to_owned());
            }
            if let Some(font) = caps.font() {
                let font = font.to_string();
                if !font.trim().is_empty() {
                    parts.push(font.trim().to_owned());
                }
            }
            parts.join(" ")
        }
        Some(icy_sauce::Capabilities::Bitmap(caps)) => format!("{}bpp", caps.pixel_depth),
        Some(icy_sauce::Capabilities::Audio(caps)) if caps.sample_rate > 0 => format!("{} Hz", caps.sample_rate),
        _ => String::new(),
    }
}

pub(super) fn format_size(size: u64) -> String {
    if size >= 1024 * 1024 {
        format!("{:.1} MiB", size as f64 / 1048576.0)
    } else if size >= 1024 {
        format!("{:.1} KiB", size as f64 / 1024.0)
    } else {
        format!("{size} B")
    }
}

impl eframe::App for Viewer {
    fn update(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        self.show(context);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sauce_summary_colours_every_field_and_hides_an_empty_date() {
        let sauce = icy_sauce::MetaData {
            title: "TITLE".into(),
            author: "AUTHOR".into(),
            group: "GROUP".into(),
            ..Default::default()
        }
        .to_builder()
        .unwrap()
        .build();
        let job = sauce_summary(true, Some(&sauce), Some(2048), Some((80, 25)));
        let palette = colors::Sauce::new(true);
        let colour = |value: &str| {
            job.sections
                .iter()
                .find(|section| &job.text[section.byte_range.clone()] == value)
                .map(|section| section.format.color)
        };
        assert_eq!(colour("TITLE"), Some(palette.title));
        assert_eq!(colour("AUTHOR"), Some(palette.author));
        assert_eq!(colour("GROUP"), Some(palette.group));
        assert_eq!(colour("80×25"), Some(palette.size));
        assert_eq!(colour("2.0 KiB"), Some(palette.date));
        assert_eq!(colour(" • "), Some(palette.separator));
        assert!(!job.text.contains("0000"), "an empty date must stay hidden: {}", job.text);
        assert_ne!(colours_for_light_theme(), palette.title);
    }

    fn colours_for_light_theme() -> egui::Color32 {
        colors::Sauce::new(false).title
    }
}
