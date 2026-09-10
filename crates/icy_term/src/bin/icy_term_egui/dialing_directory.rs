use eframe::egui;
use icy_term::{Address, Options};

#[path = "profile_editor.rs"]
pub(super) mod profile_editor;

use super::{
    phonebook::{display_address, Phonebook, SortOrder},
    session,
};

pub enum DialRequest {
    Quick(Address, Options),
    Entry(Address, Options),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Confirmation {
    Close,
    Delete,
}

#[derive(Default)]
pub struct DialingDirectory {
    pub open: bool,
    pub phonebook: Option<Phonebook>,
    pub error: Option<String>,
    pub(super) message_blocked: bool,
    pub(super) options: Options,
    quick_profile: Address,
    quick_selected: bool,
    details: bool,
    show_password: bool,
    confirmation: Option<Confirmation>,
    focus_search: bool,
    icons: Option<Icons>,
    editor: profile_editor::ProfileEditor,
    load_sources: bool,
    source_result: Option<std::sync::mpsc::Receiver<Vec<Address>>>,
    #[cfg(test)]
    pub bounds: Option<egui::Rect>,
}

pub(super) struct Icons {
    add: egui::TextureHandle,
    delete: egui::TextureHandle,
    pub(super) call: egui::TextureHandle,
    pub(super) upload: egui::TextureHandle,
    pub(super) download: egui::TextureHandle,
    pub(super) menu: egui::TextureHandle,
    pub(super) logout: egui::TextureHandle,
    close: egui::TextureHandle,
    eye: egui::TextureHandle,
}

impl Icons {
    pub(super) fn load(context: &egui::Context) -> Self {
        fn load(context: &egui::Context, name: &str, bytes: &[u8]) -> egui::TextureHandle {
            let tree = resvg::usvg::Tree::from_data(bytes, &Default::default()).expect("bundled dialog icon");
            let mut image = resvg::tiny_skia::Pixmap::new(32, 32).unwrap();
            let size = tree.size();
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::from_scale(32.0 / size.width(), 32.0 / size.height()),
                &mut image.as_mut(),
            );
            let pixels: Vec<u8> = image.data().chunks_exact(4).flat_map(|pixel| [255, 255, 255, pixel[3]]).collect();
            context.load_texture(name, egui::ColorImage::from_rgba_unmultiplied([32, 32], &pixels), Default::default())
        }
        Self {
            add: load(context, "dial-add", include_bytes!("../../../data/icons/add.svg")),
            delete: load(context, "dial-delete", include_bytes!("../../../data/icons/delete.svg")),
            call: load(context, "dial-call", include_bytes!("../../../data/icons/call.svg")),
            upload: load(context, "terminal-upload", include_bytes!("../../../data/icons/upload.svg")),
            download: load(context, "terminal-download", include_bytes!("../../../data/icons/download.svg")),
            menu: load(context, "terminal-menu", include_bytes!("../../../data/icons/menu.svg")),
            logout: load(context, "terminal-logout", include_bytes!("../../../data/icons/logout.svg")),
            close: load(context, "dial-close", include_bytes!("../../../data/icons/close.svg")),
            eye: load(context, "dial-eye", include_bytes!("../../../data/icons/visibility.svg")),
        }
    }
}

pub(super) fn icon_button(ui: &mut egui::Ui, texture: &egui::TextureHandle, label: &str) -> egui::Response {
    ui.scope(|ui| {
        ui.spacing_mut().button_padding = egui::vec2(6.0, 6.0);
        ui.add(
            egui::Button::image(
                egui::Image::new(texture)
                    .fit_to_exact_size(egui::vec2(16.0, 16.0))
                    .tint(ui.visuals().text_color()),
            )
            .min_size(egui::vec2(28.0, 28.0)),
        )
        .on_hover_text(label)
    })
    .inner
}

impl DialingDirectory {
    pub fn open(&mut self) {
        self.open = true;
        self.focus_search = true;
        self.show_password = false;
        self.confirmation = None;
        self.quick_selected = self.phonebook.as_ref().is_none_or(|book| book.selection().is_none());
        if self.phonebook.as_ref().is_some_and(|book| book.draft.is_some()) {
            return;
        }
        self.reload();
    }

    pub(super) fn reload(&mut self) {
        let path = self
            .phonebook
            .as_ref()
            .map(|book| book.path().to_path_buf())
            .or_else(Address::get_dialing_directory_file);
        let result = path.ok_or_else(|| "Cannot locate the phonebook file".to_string()).and_then(Phonebook::load);
        match result {
            Ok(book) => {
                self.phonebook = Some(book);
                self.error = None;
                self.source_result = None;
                self.load_sources = true;
            }
            Err(error) => {
                self.error = Some(error);
            }
        }
        match Options::load_options() {
            Ok(options) => self.options = options,
            Err(_) => {
                self.options = Options::default();
                self.error = Some("Cannot read existing settings. Default connection settings will be used.".into());
            }
        }
    }

    fn poll_sources(&mut self, context: &egui::Context) {
        if self.load_sources {
            self.load_sources = false;
            let sources: Vec<_> = self.options.web_directories.iter().filter(|source| source.enabled).cloned().collect();
            if !sources.is_empty() {
                let (sender, receiver) = std::sync::mpsc::channel();
                self.source_result = Some(receiver);
                let context = context.clone();
                let mut remote = icy_term::AddressBook::default();
                remote.addresses.clear();
                std::thread::spawn(move || {
                    icy_term::data::merge_web_directories(&mut remote, &sources);
                    let _ = sender.send(remote.addresses);
                    context.request_repaint();
                });
            }
        }
        if let Some(receiver) = &self.source_result {
            match receiver.try_recv() {
                Ok(entries) => {
                    if let Some(book) = &mut self.phonebook {
                        book.book.addresses.extend(entries);
                    }
                    self.source_result = None;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.error = Some("Unable to load web directories".into());
                    self.source_result = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
    }

    fn close(&mut self) {
        if self.phonebook.as_ref().is_some_and(|book| book.draft.is_some()) {
            self.confirmation = Some(Confirmation::Close);
        } else {
            self.open = false;
            self.show_password = false;
        }
    }

    pub fn show(&mut self, context: &egui::Context, connected: bool) -> Option<DialRequest> {
        if !self.open {
            return None;
        }
        self.poll_sources(context);
        if self.icons.is_none() {
            self.icons = Some(Icons::load(context));
        }
        let screen = context.available_rect();
        let width = (screen.width() - 32.0).max(260.0);
        let height = (screen.height() - 32.0).max(100.0);
        let narrow = width < 640.0;
        let mut request = None;
        let _response = egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(context.style().visuals.panel_fill).inner_margin(16))
            .show(context, |ui| {
                if self.message_blocked || self.confirmation.is_some() || self.error.is_some() {
                    ui.disable();
                }
                ui.set_width(width);
                ui.set_min_height(height);
                ui.spacing_mut().item_spacing = egui::vec2(8.0, 6.0);
                ui.spacing_mut().button_padding = egui::vec2(10.0, 5.0);
                ui.spacing_mut().interact_size.y = 28.0;
                for (style, size) in [(egui::TextStyle::Body, 14.0), (egui::TextStyle::Button, 14.0), (egui::TextStyle::Small, 12.0)] {
                    ui.style_mut().text_styles.insert(style, egui::FontId::proportional(size));
                }
                egui::ScrollArea::vertical().id_salt("directory-overflow").max_height(height).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::Image::new(&self.icons.as_ref().unwrap().call)
                                .fit_to_exact_size(egui::vec2(22.0, 22.0))
                                .tint(ui.visuals().selection.stroke.color),
                        );
                        ui.heading(&*tr!("egui-directory"));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if icon_button(ui, &self.icons.as_ref().unwrap().close, &*tr!("egui-close")).clicked() {
                                self.close();
                            }
                        });
                    });
                    ui.separator();
                    let editing = self.phonebook.as_ref().is_some_and(|book| book.draft.is_some());
                    if editing {
                        self.details = true;
                    }
                    let remaining = (height - (ui.cursor().min.y - ui.min_rect().min.y) - 8.0).max(60.0);
                    ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), remaining), egui::Layout::top_down(egui::Align::Min), |ui| {
                        let editing = self.phonebook.as_ref().is_some_and(|book| book.draft.is_some());
                        if narrow {
                            ui.horizontal(|ui| {
                                ui.add_enabled_ui(!editing, |ui| {
                                    super::appearance::tab(ui, &mut self.details, false, &tr!("egui-directory-tab"));
                                });
                                super::appearance::tab(ui, &mut self.details, true, &tr!("egui-entry"));
                            });
                        }
                        if narrow {
                            let next = if self.details {
                                self.entry_ui(ui, connected)
                            } else {
                                self.list_ui(ui, connected, ui.available_height())
                            };
                            if next.is_some() {
                                request = next;
                            }
                        } else {
                            let body_height = ui.available_height();
                            let list_width = (width * 0.34).clamp(260.0, 360.0);
                            ui.horizontal_top(|ui| {
                                ui.allocate_ui_with_layout(egui::vec2(list_width, body_height), egui::Layout::top_down(egui::Align::Min), |ui| {
                                    ui.set_min_width(list_width);
                                    if let Some(entry) = self.list_ui(ui, connected, body_height) {
                                        request = Some(entry);
                                    }
                                });
                                ui.separator();
                                ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), body_height), egui::Layout::top_down(egui::Align::Min), |ui| {
                                    if let Some(entry) = self.entry_ui(ui, connected) {
                                        request = Some(entry);
                                    }
                                });
                            });
                        }
                    });
                });
            });
        #[cfg(test)]
        {
            self.bounds = Some(_response.response.rect);
        }
        if !self.message_blocked
            && self.confirmation.is_none()
            && self.error.is_none()
            && context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.close();
        }
        if !self.message_blocked {
            if let Some(error) = &self.error {
                if super::messages::MessageBox::error("directory-error", &tr!("egui-message-directory"), error)
                    .show(context)
                    .is_some()
                {
                    self.error = None;
                }
            } else if self.confirmation.is_some() {
                self.confirmation_ui(context);
            }
        }
        if request.is_some() {
            self.open = false;
            self.show_password = false;
        }
        request
    }

    fn list_ui(&mut self, ui: &mut egui::Ui, connected: bool, height: f32) -> Option<DialRequest> {
        let top = ui.cursor().top();
        let editing = self.phonebook.as_ref().is_some_and(|book| book.draft.is_some());
        let mut request = None;
        let mut reload = false;
        ui.add_enabled_ui(!editing, |ui| {
            let Some(book) = &mut self.phonebook else {
                return;
            };
            let search_id = ui.make_persistent_id("dial-search");
            let search_enter = ui.is_enabled() && ui.memory(|memory| memory.has_focus(search_id)) && ui.input(|input| input.key_pressed(egui::Key::Enter));
            let search = ui.add(
                egui::TextEdit::singleline(&mut book.query)
                    .id(search_id)
                    .margin(egui::vec2(8.0, 6.0))
                    .hint_text(&*tr!("dialing_directory-filter-placeholder"))
                    .desired_width(f32::INFINITY),
            );
            if self.focus_search {
                search.request_focus();
                self.focus_search = false;
            }
            ui.memory_mut(|memory| {
                memory.set_focus_lock_filter(
                    search.id,
                    egui::EventFilter {
                        vertical_arrows: true,
                        ..Default::default()
                    },
                )
            });
            ui.horizontal(|ui| {
                ui.add_enabled_ui(!book.book.write_lock, |ui| {
                    if icon_button(ui, &self.icons.as_ref().unwrap().add, &tr!("egui-new-entry")).clicked() {
                        book.begin_new(false);
                        self.quick_selected = false;
                        self.editor = Default::default();
                        self.details = true;
                        self.show_password = false;
                    }
                });
                if ui
                    .add(
                        egui::Button::new(egui::RichText::new("\u{2605}").size(18.0))
                            .selected(book.favorites_only)
                            .min_size(egui::vec2(28.0, 28.0)),
                    )
                    .on_hover_text(tr!("dialing_directory-starred-items"))
                    .clicked()
                {
                    book.favorites_only = !book.favorites_only;
                }
                let image = egui::Image::new(&self.icons.as_ref().unwrap().menu)
                    .fit_to_exact_size(egui::vec2(16.0, 16.0))
                    .tint(ui.visuals().text_color());
                ui.menu_image_button(image, |ui| {
                    ui.strong(tr!("egui-sort"));
                    ui.selectable_value(&mut book.sort, SortOrder::Name, &*tr!("settings-modem-name"));
                    ui.selectable_value(&mut book.sort, SortOrder::MostCalled, &*tr!("egui-most-called"));
                    ui.selectable_value(&mut book.sort, SortOrder::LastCalled, &*tr!("egui-last-called"));
                    let sources: std::collections::BTreeSet<_> = book.book.addresses.iter().filter_map(|entry| entry.web_source.clone()).collect();
                    if !sources.is_empty() {
                        ui.separator();
                        ui.strong(tr!("egui-source"));
                        ui.selectable_value(&mut book.source, None, &*tr!("egui-all-sources"));
                        ui.selectable_value(&mut book.source, Some(String::new()), &*tr!("egui-local-phonebook"));
                        for source in sources {
                            ui.selectable_value(&mut book.source, Some(source.clone()), source);
                        }
                    }
                    ui.separator();
                    if ui.add_enabled(self.source_result.is_none(), egui::Button::new(tr!("egui-refresh"))).clicked() {
                        reload = true;
                        ui.close();
                    }
                })
                .response
                .on_hover_text(tr!("egui-filters"));
                if self.source_result.is_some() {
                    ui.spinner();
                }
                if let Some(source) = &book.source {
                    ui.add(
                        egui::Label::new(egui::RichText::new(if source.is_empty() { tr!("egui-local-phonebook") } else { source.clone() }).weak()).truncate(),
                    );
                }
            });
            let indices = book.filtered();
            let show_quick = book.query.is_empty() && !book.favorites_only && book.source.is_none();
            if !show_quick {
                self.quick_selected = false;
            }
            let mut scroll_selection = false;
            if book.draft.is_none() && !indices.contains(&book.selected.unwrap_or(usize::MAX)) {
                book.selected = indices.first().copied();
            }
            if ui.is_enabled() && (search.has_focus() || search_enter) && !editing && !indices.is_empty() {
                let position = book
                    .selected
                    .and_then(|selected| indices.iter().position(|index| *index == selected))
                    .unwrap_or(0);
                let down = ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown));
                let up = ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp));
                if down {
                    book.selected = Some(indices[if self.quick_selected { 0 } else { (position + 1).min(indices.len() - 1) }]);
                    self.quick_selected = false;
                }
                if up {
                    self.quick_selected = show_quick && position == 0;
                    book.selected = Some(indices[position.saturating_sub(1)]);
                }
                scroll_selection = down || up;
                if self.quick_selected && search_enter {
                    self.details = true;
                } else if !connected && search_enter && !ui.ctx().will_discard() {
                    let entry = book.selection().unwrap();
                    match session::entry_connection_config(entry, &self.options) {
                        Ok(_) => request = Some(DialRequest::Entry(entry.clone(), self.options.clone())),
                        Err(error) => self.error = Some(error),
                    }
                }
            }
            egui::Frame::new()
                .fill(ui.visuals().extreme_bg_color)
                .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                .corner_radius(4)
                .inner_margin(4)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("dial-list")
                        .min_scrolled_height(0.0)
                        .max_height((height - (ui.cursor().top() - top) - 8.0).max(0.0))
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            if show_quick {
                                let response = directory_row(ui, &tr!("dialing_directory-connect-to-address"), "", self.quick_selected);
                                if response.clicked() {
                                    self.quick_selected = true;
                                    self.editor = Default::default();
                                    self.details = true;
                                    self.show_password = false;
                                }
                            }
                            if indices.is_empty() && !show_quick {
                                ui.label(&*tr!("dialing_directory-no-entries"));
                            }
                            for index in indices {
                                let entry = &book.book.addresses[index];
                                let name = entry.system_name.clone();
                                let address = display_address(entry);
                                let favorite = entry.is_favored;
                                let remote = entry.web_source.is_some();
                                let calls = entry.number_of_calls;
                                ui.push_id(index, |ui| {
                                    let response = address_row(
                                        ui,
                                        &name,
                                        &address,
                                        book.selected == Some(index) && !self.quick_selected,
                                        Some((favorite, calls)),
                                    );
                                    response.context_menu(|ui| {
                                        if ui
                                            .add_enabled(
                                                !book.book.write_lock && !remote,
                                                egui::Button::new(if favorite { tr!("egui-remove-favorite") } else { tr!("egui-add-favorite") }),
                                            )
                                            .clicked()
                                            && !ui.ctx().will_discard()
                                        {
                                            if let Err(error) = book.toggle_favorite(index) {
                                                self.error = Some(error);
                                            }
                                            ui.close();
                                        }
                                    });
                                    if scroll_selection && book.selected == Some(index) && !self.quick_selected {
                                        response.scroll_to_me(Some(egui::Align::Center));
                                    }
                                    if response.clicked() {
                                        book.selected = Some(index);
                                        self.quick_selected = false;
                                        self.details = true;
                                        self.show_password = false;
                                    }
                                    if response.double_clicked() && !connected && !ui.ctx().will_discard() {
                                        let entry = &book.book.addresses[index];
                                        match session::entry_connection_config(entry, &self.options) {
                                            Ok(_) => request = Some(DialRequest::Entry(entry.clone(), self.options.clone())),
                                            Err(error) => self.error = Some(error),
                                        }
                                    }
                                });
                            }
                        });
                });
        });
        if reload {
            self.reload();
        }
        request
    }

    fn entry_ui(&mut self, ui: &mut egui::Ui, connected: bool) -> Option<DialRequest> {
        if self.quick_selected && self.phonebook.as_ref().is_none_or(|book| book.draft.is_none()) {
            return self.quick_connect_ui(ui, connected);
        }
        let Some(book) = &mut self.phonebook else {
            return None;
        };
        if book.book.write_lock {
            ui.colored_label(ui.visuals().warn_fg_color, &*tr!("egui-phonebook-read-only"));
        }
        if let Some(draft) = &mut book.draft {
            ui.strong(if book.selected.is_some() {
                tr!("egui-edit-entry")
            } else {
                tr!("egui-new-entry")
            });
            self.editor
                .show(ui, draft, &self.options, &mut self.show_password, &self.icons.as_ref().unwrap().eye);
            ui.separator();
            ui.horizontal(|ui| {
                if ui.add(super::appearance::primary_button(tr!("egui-save"))).clicked() && !ui.ctx().will_discard() {
                    match book.save() {
                        Ok(()) => self.error = None,
                        Err(error) => self.error = Some(error),
                    }
                    self.show_password = false;
                }
                if ui.button(&*tr!("egui-discard")).clicked() {
                    book.draft = None;
                    self.show_password = false;
                    self.error = None;
                }
            });
            return None;
        }
        let Some(entry) = book.selection().cloned() else {
            ui.label(&*tr!("egui-no-entry-selected"));
            return None;
        };
        let remote = entry.web_source.is_some();
        if let Some(source) = &entry.web_source {
            ui.weak(format!("{source} / Read-only"));
        }
        ui.add(egui::Label::new(egui::RichText::new(&entry.system_name).strong().size(18.0)).wrap());
        ui.add(egui::Label::new(egui::RichText::new(display_address(&entry)).monospace()).wrap());
        ui.weak(format!(
            "{} / {}",
            profile_editor::protocol_name(entry.protocol),
            icy_term::fmt_terminal_emulation(&entry.terminal_type)
        ));
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt("profile-summary")
            .max_height((ui.available_height() - 110.0).max(50.0))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().interact_size.y = 20.0;
                    ui.spacing_mut().item_spacing.y = 4.0;
                    let date = |time: chrono::DateTime<chrono::Utc>| time.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M").to_string();
                    for (label, value) in [
                        (
                            &*tr!("dialing_directory-user"),
                            if entry.user_name.is_empty() { "-".into() } else { entry.user_name.clone() },
                        ),
                        (&*tr!("dialing_directory-screen_mode"), entry.get_screen_mode().to_string()),
                        (&*tr!("egui-font"), entry.font_name.clone().unwrap_or_else(|| tr!("egui-terminal-default"))),
                        (&*tr!("egui-baud"), entry.baud_emulation.to_string()),
                        (
                            &*tr!("dialing_directory-proxy"),
                            entry
                                .proxy
                                .as_ref()
                                .map(|proxy| format!("{}:{}", proxy.host, proxy.port))
                                .unwrap_or_else(|| tr!("egui-direct")),
                        ),
                        (&*tr!("egui-calls"), entry.number_of_calls.to_string()),
                        (&*tr!("egui-last-call"), entry.last_call.map(date).unwrap_or_else(|| tr!("egui-never"))),
                        (&*tr!("egui-total-time"), format!("{} min", entry.overall_duration.num_minutes())),
                        (&*tr!("egui-last-duration"), format!("{} sec", entry.last_call_duration.num_seconds())),
                        (&*tr!("egui-uploaded"), human_bytes::human_bytes(entry.uploaded_bytes as f64)),
                        (&*tr!("egui-downloaded"), human_bytes::human_bytes(entry.downloaded_bytes as f64)),
                    ] {
                        ui.horizontal_top(|ui| {
                            ui.allocate_ui_with_layout(egui::vec2(112.0, 20.0), egui::Layout::left_to_right(egui::Align::Min), |ui| {
                                ui.set_min_width(112.0);
                                ui.weak(label);
                            });
                            ui.add(egui::Label::new(value).wrap());
                        });
                    }
                });
                if entry.protocol == icy_net::ConnectionType::SSH {
                    ui.label(&*tr!("egui-strict-host-key"));
                }
                if !entry.comment.is_empty() {
                    ui.separator();
                    ui.strong(&*tr!("dialing_directory-notes"));
                    ui.add(egui::Label::new(&entry.comment).wrap());
                }
            });
        ui.separator();
        let eligible = session::entry_connection_config(&entry, &self.options);
        if let Err(reason) = &eligible {
            ui.colored_label(ui.visuals().warn_fg_color, reason);
        }
        let mut request = None;
        ui.horizontal_wrapped(|ui| {
            let image = egui::Image::new(&self.icons.as_ref().unwrap().call)
                .fit_to_exact_size(egui::vec2(16.0, 16.0))
                .tint(egui::Color32::WHITE);
            if ui
                .add_enabled(
                    !connected && eligible.is_ok(),
                    egui::Button::image_and_text(
                        image,
                        egui::RichText::new(&*tr!("dialing_directory-connect-button")).color(egui::Color32::WHITE),
                    )
                    .fill(super::appearance::PRIMARY),
                )
                .clicked()
                && !ui.ctx().will_discard()
            {
                request = Some(DialRequest::Entry(entry.clone(), self.options.clone()));
            }
            ui.add_enabled_ui(!book.book.write_lock, |ui| {
                if ui.add_enabled(!remote, egui::Button::new(&*tr!("egui-edit"))).clicked() {
                    book.begin_edit();
                    self.editor = Default::default();
                    self.show_password = false;
                }
                if ui.button(&*tr!("dialing_directory-duplicate")).clicked() {
                    book.begin_new(true);
                    self.editor = Default::default();
                    self.show_password = false;
                }
                ui.add_enabled_ui(!remote, |ui| {
                    if icon_button(ui, &self.icons.as_ref().unwrap().delete, &*tr!("egui-delete-entry")).clicked() {
                        self.confirmation = Some(Confirmation::Delete);
                    }
                });
            });
        });
        request
    }

    fn quick_connect_ui(&mut self, ui: &mut egui::Ui, connected: bool) -> Option<DialRequest> {
        let mut request = None;
        if ui.available_height() >= 240.0 {
            ui.heading(tr!("dialing_directory-connect-to"));
            ui.separator();
        }
        ui.add_enabled_ui(!connected, |ui| {
            let enter = self.editor.show_quick(
                ui,
                &mut self.quick_profile,
                &self.options,
                &mut self.show_password,
                &self.icons.as_ref().unwrap().eye,
            );
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                let mut entry = self.quick_profile.clone();
                entry.system_name = display_address(&entry);
                let dial = ui
                    .add_enabled(!entry.address.trim().is_empty(), super::appearance::primary_button(tr!("egui-quick-connect")))
                    .clicked()
                    || enter;
                if dial && !ui.ctx().will_discard() {
                    match session::entry_connection_config(&entry, &self.options) {
                        Ok(_) => request = Some(DialRequest::Quick(entry.clone(), self.options.clone())),
                        Err(error) => self.error = Some(error),
                    }
                }
                let writable = self.phonebook.as_ref().is_some_and(|book| !book.book.write_lock);
                if ui
                    .add_enabled(
                        writable && !entry.address.trim().is_empty(),
                        egui::Button::new(tr!("dialing_directory-add-bbs-button")),
                    )
                    .clicked()
                    && !ui.ctx().will_discard()
                {
                    match session::entry_connection_config(&entry, &self.options) {
                        Ok(_) => {
                            let book = self.phonebook.as_mut().unwrap();
                            book.begin_new(false);
                            book.draft = Some(entry);
                            self.editor = Default::default();
                            self.quick_selected = false;
                        }
                        Err(error) => self.error = Some(error),
                    }
                }
            });
        });
        request
    }

    fn confirmation_ui(&mut self, context: &egui::Context) {
        use super::messages::{MessageBox, Response};
        let delete = self.confirmation == Some(Confirmation::Delete);
        let title = if delete { tr!("egui-delete-question") } else { tr!("egui-discard-question") };
        let body = if delete { tr!("egui-message-delete") } else { tr!("egui-message-unsaved") };
        let label = if delete { tr!("egui-delete") } else { tr!("egui-discard-close") };
        let name = self
            .phonebook
            .as_ref()
            .and_then(|book| book.draft.as_ref().or_else(|| book.selection()))
            .map(|entry| entry.system_name.as_str())
            .unwrap_or_default();
        let dialog = MessageBox::question("directory-confirmation", &title, &body, &label)
            .details(name)
            .destructive();
        let response = if delete { dialog } else { dialog.save_option() }.show(context);
        match response {
            Some(Response::Accept) => {
                if delete {
                    if let Some(book) = &mut self.phonebook {
                        match book.delete() {
                            Ok(()) => {
                                self.error = None;
                                self.confirmation = None;
                            }
                            Err(error) => self.error = Some(error),
                        }
                    }
                } else {
                    if let Some(book) = &mut self.phonebook {
                        book.draft = None;
                    }
                    self.confirmation = None;
                    self.open = false;
                    self.show_password = false;
                }
            }
            Some(Response::Cancel) => self.confirmation = None,
            Some(Response::Save) => {
                if let Some(book) = &mut self.phonebook {
                    match book.save() {
                        Ok(()) => {
                            self.confirmation = None;
                            self.open = false;
                            self.show_password = false;
                        }
                        Err(error) => self.error = Some(error),
                    }
                }
            }
            None => {}
        }
    }
}

fn text_field(ui: &mut egui::Ui, label: &str, value: &mut String) {
    ui.push_id(label, |ui| {
        super::appearance::form_row(ui, label, |ui| {
            ui.add(egui::TextEdit::singleline(value).margin(egui::vec2(8.0, 6.0)).desired_width(f32::INFINITY));
        })
    });
}

fn directory_row(ui: &mut egui::Ui, name: &str, address: &str, selected: bool) -> egui::Response {
    address_row(ui, name, address, selected, None)
}

fn address_row(ui: &mut egui::Ui, name: &str, address: &str, selected: bool, metadata: Option<(bool, usize)>) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 52.0), egui::Sense::click());
    let visuals = ui.style().interact_selectable(&response, selected);
    let fill = if selected || response.hovered() {
        visuals.bg_fill
    } else {
        egui::Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 3.0, fill);
    if selected {
        ui.painter().rect_stroke(rect, 3.0, ui.visuals().selection.stroke, egui::StrokeKind::Inside);
    }
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(egui::Rect::from_min_max(
                rect.min + egui::vec2(10.0, 6.0),
                rect.max - egui::vec2(if metadata.is_some() { 60.0 } else { 10.0 }, 6.0),
            ))
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    child.spacing_mut().item_spacing.y = 2.0;
    child.add(
        egui::Label::new(egui::RichText::new(name).monospace().color(ui.visuals().text_color()))
            .selectable(false)
            .truncate(),
    );
    child.add(
        egui::Label::new(egui::RichText::new(address).monospace().small().color(ui.visuals().weak_text_color()))
            .selectable(false)
            .truncate(),
    );
    if let Some((favorite, calls)) = metadata {
        if favorite {
            ui.painter().text(
                rect.right_top() + egui::vec2(-10.0, 5.0),
                egui::Align2::RIGHT_TOP,
                "\u{2605}",
                egui::FontId::proportional(16.0),
                egui::Color32::from_rgb(211, 163, 57),
            );
        }
        ui.painter().text(
            rect.right_bottom() - egui::vec2(10.0, 7.0),
            egui::Align2::RIGHT_BOTTOM,
            format!("\u{2706} {calls}"),
            egui::FontId::proportional(12.0),
            ui.visuals().weak_text_color(),
        );
    }
    response.on_hover_text(format!("{name}\n{address}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phonebook::tests::{add, Fixture};

    fn frame(dialog: &mut DialingDirectory, context: &egui::Context, events: Vec<egui::Event>) -> (egui::FullOutput, Option<DialRequest>) {
        let mut request = None;
        let output = context.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 800.0))),
                events,
                ..Default::default()
            },
            |context| {
                request = dialog.show(context, false);
            },
        );
        (output, request)
    }

    fn click(dialog: &mut DialingDirectory, context: &egui::Context, label: &str) -> Option<DialRequest> {
        frame(dialog, context, vec![]);
        let (output, _) = frame(dialog, context, vec![]);
        let position = output
            .shapes
            .iter()
            .rev()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == label => Some(text.pos + text.galley.size() / 2.0),
                _ => None,
            })
            .unwrap_or_else(|| panic!("Missing control: {label}"));
        frame(
            dialog,
            context,
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        frame(
            dialog,
            context,
            vec![egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        )
        .1
    }

    #[test]
    fn dialog_edits_saves_duplicates_and_confirms_close() {
        let fixture = Fixture::new();
        let mut book = fixture.load();
        add(&mut book, "Test BBS");
        let mut dialog = DialingDirectory {
            phonebook: Some(book),
            open: true,
            ..Default::default()
        };
        let context = egui::Context::default();
        click(&mut dialog, &context, &*tr!("egui-edit"));
        assert!(dialog.phonebook.as_ref().unwrap().draft.is_some());
        dialog.phonebook.as_mut().unwrap().draft.as_mut().unwrap().system_name = "Renamed BBS".into();
        click(&mut dialog, &context, &*tr!("egui-save"));
        assert_eq!(dialog.phonebook.as_ref().unwrap().selection().unwrap().system_name, "Renamed BBS");
        click(&mut dialog, &context, &*tr!("dialing_directory-duplicate"));
        assert!(dialog.phonebook.as_ref().unwrap().selected.is_none());
        frame(&mut dialog, &context, vec![]);
        assert!(
            dialog.phonebook.as_ref().unwrap().selected.is_none(),
            "layout must not reselect an existing entry during creation"
        );
        click(&mut dialog, &context, &*tr!("egui-save"));
        assert_eq!(dialog.phonebook.as_ref().unwrap().book.addresses.len(), 2);
        click(&mut dialog, &context, &*tr!("egui-edit"));
        dialog.close();
        assert!(dialog.open);
        click(&mut dialog, &context, &*tr!("egui-cancel"));
        assert!(dialog.open && dialog.phonebook.as_ref().unwrap().draft.is_some());
        dialog.close();
        click(&mut dialog, &context, &*tr!("egui-discard-close"));
        assert!(!dialog.open);
    }

    #[test]
    fn dial_returns_selected_profile_and_delete_requires_confirmation() {
        let fixture = Fixture::new();
        let mut book = fixture.load();
        add(&mut book, "Test BBS");
        let mut dialog = DialingDirectory {
            phonebook: Some(book),
            open: true,
            ..Default::default()
        };
        let context = egui::Context::default();
        let request = click(&mut dialog, &context, &*tr!("dialing_directory-connect-button")).unwrap();
        let DialRequest::Entry(entry, _) = request else {
            panic!("Expected selected profile");
        };
        assert_eq!(entry.password, "test-secret");
        assert!(!dialog.open);
        dialog.open = true;
        dialog.confirmation = Some(Confirmation::Delete);
        click(&mut dialog, &context, &*tr!("egui-cancel"));
        assert_eq!(dialog.phonebook.as_ref().unwrap().book.addresses.len(), 1);
        dialog.confirmation = Some(Confirmation::Delete);
        click(&mut dialog, &context, &*tr!("egui-delete"));
        assert!(dialog.phonebook.as_ref().unwrap().book.addresses.is_empty());
    }

    #[test]
    fn terminal_options_can_be_edited_and_saved() {
        let fixture = Fixture::new();
        let mut book = fixture.load();
        add(&mut book, "Test BBS");
        let before = book.selection().unwrap().mouse_reporting_enabled;
        let mut dialog = DialingDirectory {
            phonebook: Some(book),
            open: true,
            ..Default::default()
        };
        let context = egui::Context::default();
        click(&mut dialog, &context, &*tr!("egui-edit"));
        click(&mut dialog, &context, &*tr!("settings-terminal-category"));
        click(&mut dialog, &context, &*tr!("egui-mouse-reporting"));
        click(&mut dialog, &context, &*tr!("egui-save"));
        let book = dialog.phonebook.as_ref().unwrap();
        let loaded = Phonebook::load(book.path().to_path_buf()).unwrap();
        assert_eq!(loaded.selection().unwrap().mouse_reporting_enabled, !before);
    }

    #[test]
    fn confirmation_can_save_before_closing() {
        let fixture = Fixture::new();
        let mut book = fixture.load();
        add(&mut book, "Test BBS");
        book.begin_edit();
        book.draft.as_mut().unwrap().system_name = "Saved on close".into();
        let mut dialog = DialingDirectory {
            phonebook: Some(book),
            open: true,
            ..Default::default()
        };
        let context = egui::Context::default();
        dialog.close();
        click(&mut dialog, &context, &tr!("egui-save"));
        assert!(!dialog.open);
        assert!(dialog.phonebook.as_ref().unwrap().draft.is_none());
        let saved = Phonebook::load(dialog.phonebook.as_ref().unwrap().path().to_path_buf()).unwrap();
        assert_eq!(saved.selection().unwrap().system_name, "Saved on close");
    }

    #[test]
    fn confirmation_enter_cancels_without_dialing_or_deleting() {
        let fixture = Fixture::new();
        let mut book = fixture.load();
        add(&mut book, "Test BBS");
        let mut dialog = DialingDirectory {
            phonebook: Some(book),
            open: true,
            focus_search: true,
            ..Default::default()
        };
        let context = egui::Context::default();
        frame(&mut dialog, &context, vec![]);
        dialog.confirmation = Some(Confirmation::Delete);
        frame(&mut dialog, &context, vec![]);
        let (_, request) = frame(
            &mut dialog,
            &context,
            vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(request.is_none());
        assert!(dialog.open && dialog.confirmation.is_none());
        let saved = Phonebook::load(dialog.phonebook.as_ref().unwrap().path().to_path_buf()).unwrap();
        assert_eq!(saved.book.addresses.len(), 1);
    }

    #[test]
    fn quick_connect_retains_unsaved_terminal_and_login_settings() {
        let fixture = Fixture::new();
        let mut dialog = DialingDirectory {
            phonebook: Some(fixture.load()),
            open: true,
            quick_profile: quick_profile("raw://localhost:2323"),
            ..Default::default()
        };
        dialog.quick_profile.terminal_type = icy_net::telnet::TerminalEmulation::Utf8Ansi;
        dialog.quick_profile.screen_mode = icy_engine::ScreenMode::Unicode(132, 43);
        dialog.quick_profile.user_name = "guest".into();
        let context = egui::Context::default();
        click(&mut dialog, &context, &tr!("dialing_directory-connect-to-address"));
        click(&mut dialog, &context, &tr!("settings-terminal-category"));
        let request = click(&mut dialog, &context, &tr!("egui-quick-connect")).unwrap();
        let DialRequest::Quick(entry, options) = request else {
            panic!("expected unsaved quick profile")
        };
        let config = session::entry_connection_config(&entry, &options).unwrap();
        assert_eq!(config.window_size, (132, 43));
        assert_eq!(config.user_name.as_deref(), Some("guest"));
        let saved = Phonebook::load(dialog.phonebook.as_ref().unwrap().path().to_path_buf()).unwrap();
        assert!(saved.book.addresses.is_empty());
    }

    #[test]
    fn palette_proxy_and_login_pages_preserve_drafts() {
        let fixture = Fixture::new();
        let mut book = fixture.load();
        add(&mut book, "Test BBS");
        let mut dialog = DialingDirectory {
            phonebook: Some(book),
            open: true,
            ..Default::default()
        };
        let context = egui::Context::default();
        click(&mut dialog, &context, &*tr!("egui-edit"));
        click(&mut dialog, &context, &*tr!("egui-direct-connection"));
        click(&mut dialog, &context, "Tor (SOCKS5)");
        click(&mut dialog, &context, &*tr!("egui-colors"));
        click(&mut dialog, &context, &*tr!("dialing_directory-custom-palette"));
        click(&mut dialog, &context, &*tr!("egui-login"));
        let (output, _) = frame(&mut dialog, &context, vec![]);
        assert!(!output
            .shapes
            .iter()
            .any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text().contains("test-secret"))));
        click(&mut dialog, &context, &*tr!("egui-save"));
        let book = dialog.phonebook.as_ref().unwrap();
        let loaded = Phonebook::load(book.path().to_path_buf()).unwrap();
        let entry = loaded.selection().unwrap();
        assert_eq!(entry.proxy.as_ref().unwrap().port, 9050);
        assert_eq!(entry.custom_palette.as_ref().unwrap().len(), 16);
        assert_eq!(entry.password, "test-secret");
    }

    #[test]
    fn background_sources_do_not_replace_an_open_draft() {
        let fixture = Fixture::new();
        let mut book = fixture.load();
        add(&mut book, "Local BBS");
        book.begin_edit();
        let mut remote = Address::new("Remote BBS");
        remote.address = "localhost".into();
        remote.web_source = Some("Community".into());
        let (sender, receiver) = std::sync::mpsc::channel();
        sender.send(vec![remote]).unwrap();
        let mut dialog = DialingDirectory {
            phonebook: Some(book),
            open: true,
            source_result: Some(receiver),
            ..Default::default()
        };
        frame(&mut dialog, &egui::Context::default(), vec![]);
        let book = dialog.phonebook.as_ref().unwrap();
        assert_eq!(book.book.addresses.len(), 2);
        assert_eq!(book.draft.as_ref().unwrap().system_name, "Local BBS");
        assert!(dialog.source_result.is_none());
    }

    #[test]
    fn viewing_an_entry_does_not_reveal_its_password() {
        let fixture = Fixture::new();
        let mut book = fixture.load();
        add(&mut book, "Test BBS");
        let mut dialog = DialingDirectory {
            phonebook: Some(book),
            open: true,
            ..Default::default()
        };
        let context = egui::Context::default();
        click(&mut dialog, &context, &*tr!("egui-edit"));
        let (output, _) = frame(&mut dialog, &context, vec![]);
        assert!(!output
            .shapes
            .iter()
            .any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text().contains("test-secret"))));
    }

    fn quick_profile(address: &str) -> Address {
        let mut profile = Address::from(icy_term::ConnectionInformation::parse(address).unwrap());
        profile.address = address.into();
        profile
    }

    #[test]
    fn quick_connect_is_not_lost_when_the_directory_is_rendered() {
        let fixture = Fixture::new();
        let mut dialog = DialingDirectory {
            phonebook: Some(fixture.load()),
            open: true,
            quick_profile: quick_profile("raw://localhost:2323"),
            ..Default::default()
        };
        let context = egui::Context::default();
        click(&mut dialog, &context, &tr!("dialing_directory-connect-to-address"));
        let request = click(&mut dialog, &context, &*tr!("egui-quick-connect")).unwrap();
        assert!(matches!(request, DialRequest::Quick(entry, _) if entry.address == "raw://localhost:2323"));
        assert!(!dialog.open);
    }

    #[test]
    fn list_click_selects_the_correct_entry() {
        let fixture = Fixture::new();
        let mut book = fixture.load();
        add(&mut book, "Alpha");
        add(&mut book, "Beta");
        let mut dialog = DialingDirectory {
            phonebook: Some(book),
            open: true,
            ..Default::default()
        };
        let context = egui::Context::default();
        click(&mut dialog, &context, "Alpha");
        assert_eq!(dialog.phonebook.as_ref().unwrap().selection().unwrap().system_name, "Alpha");
    }

    #[test]
    fn search_arrow_and_enter_dial_the_highlighted_entry() {
        let fixture = Fixture::new();
        let mut book = fixture.load();
        add(&mut book, "Alpha");
        add(&mut book, "Beta");
        book.selected = Some(0);
        let mut dialog = DialingDirectory {
            phonebook: Some(book),
            open: true,
            focus_search: true,
            ..Default::default()
        };
        let context = egui::Context::default();
        frame(&mut dialog, &context, vec![]);
        frame(&mut dialog, &context, vec![]);
        let key = |key| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        frame(&mut dialog, &context, vec![key(egui::Key::ArrowDown)]);
        assert_eq!(dialog.phonebook.as_ref().unwrap().selection().unwrap().system_name, "Beta");
        let (_, request) = frame(&mut dialog, &context, vec![key(egui::Key::Enter)]);
        assert!(matches!(request, Some(DialRequest::Entry(entry, _)) if entry.system_name == "Beta"));
    }

    #[test]
    fn quick_address_can_be_saved_as_a_profile() {
        let fixture = Fixture::new();
        let mut dialog = DialingDirectory {
            phonebook: Some(fixture.load()),
            open: true,
            quick_profile: quick_profile("ws://localhost:2323/terminal"),
            ..Default::default()
        };
        let context = egui::Context::default();
        click(&mut dialog, &context, &tr!("dialing_directory-connect-to-address"));
        click(&mut dialog, &context, &*tr!("dialing_directory-add-bbs-button"));
        let book = dialog.phonebook.as_ref().unwrap();
        assert_eq!(book.draft.as_ref().unwrap().address, "ws://localhost:2323/terminal");
        assert!(book.book.addresses.is_empty());
        click(&mut dialog, &context, &*tr!("egui-save"));
        assert_eq!(
            dialog.phonebook.as_ref().unwrap().selection().unwrap().protocol,
            icy_net::ConnectionType::Websocket
        );
    }

    #[test]
    fn quick_connect_accepts_enter_in_the_address_field() {
        let fixture = Fixture::new();
        let mut dialog = DialingDirectory {
            phonebook: Some(fixture.load()),
            open: true,
            quick_profile: quick_profile("raw://localhost:2323"),
            ..Default::default()
        };
        let context = egui::Context::default();
        click(&mut dialog, &context, &tr!("dialing_directory-connect-to-address"));
        click(&mut dialog, &context, "raw://localhost:2323");
        let (_, request) = frame(
            &mut dialog,
            &context,
            vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(matches!(request, Some(DialRequest::Quick(_, _))));
    }
}
