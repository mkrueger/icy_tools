use crate::ui::dialogs::terminal_settings_ui;
use crate::ui::{MainWindowMode, Message};
use crate::{Address, AddressBook};
use i18n_embed_fl::fl;
use icy_ui::keyboard;
use icy_ui::{
    widget::{button, column, container, row, svg, text, Space},
    Alignment, Element, Event, Length, Task,
};
use icy_engine::ScreenMode;
use icy_engine_gui::ui::*;
use icy_net::{telnet::TerminalEmulation, ConnectionType};
use icy_parser_core::{BaudEmulation, MusicOption};
use parking_lot::Mutex;
use std::sync::Arc;

mod address_list;
mod address_options_panel;
mod delete_confirmation;

const DELETE_SVG: &[u8] = include_bytes!("../../../../data/icons/delete.svg");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialingDirectoryFilter {
    All,
    Favourites,
}

impl Default for DialingDirectoryFilter {
    fn default() -> Self {
        Self::All
    }
}

#[derive(Debug, Clone)]
pub struct DialingDirectoryState {
    pub addresses: Arc<Mutex<AddressBook>>,
    pub selected_bbs: Option<usize>,
    pub filter_mode: DialingDirectoryFilter,
    pub filter_text: String,
    pub show_passwords: bool,
    pub pending_delete: Option<usize>,
    pub quick_connect_address: Address,

    // Double-click detection
    last_click_time: Option<std::time::Instant>,
    last_clicked_index: Option<Option<usize>>,
}

impl DialingDirectoryState {
    pub fn new(addresses: Arc<Mutex<AddressBook>>) -> Self {
        Self {
            addresses,
            selected_bbs: None,
            filter_mode: DialingDirectoryFilter::All,
            filter_text: String::new(),
            show_passwords: false,
            pending_delete: None,
            quick_connect_address: Address::default(),
            last_click_time: None,
            last_clicked_index: None,
        }
    }

    pub fn view(&self, options: &crate::Options) -> Element<'_, Message> {
        // Main layout with left panel, right panel, and bottom bar
        let content_area = row![
            container(self.create_address_list()).padding(8),
            container(self.create_option_panel(options)).padding(8).width(Length::Fill)
        ]
        .height(Length::Fill);

        let main_content = column![container(content_area).height(Length::Fill), separator(), self.create_bottom_bar()];

        // If there's a pending delete, show the confirmation modal
        if let Some(idx) = self.pending_delete {
            self.delete_confirmation_modal(idx)
        } else {
            main_content.into()
        }
    }

    fn create_bottom_bar(&self) -> Element<'_, Message> {
        use icy_ui::widget::tooltip;
        let delete_label = fl!(crate::LANGUAGE_LOADER, "dialing_directory-delete");
        let delete_icon = svg(svg::Handle::from_memory(DELETE_SVG)).width(Length::Fixed(20.0)).height(Length::Fixed(20.0));
        let can_delete = self.selected_bbs.is_some();

        // Base delete button (icon only)
        let delete_button = if can_delete {
            button(delete_icon)
                .on_press(Message::from(DialingDirectoryMsg::DeleteAddress(self.selected_bbs.unwrap())))
                .padding(6)
        } else {
            // Disabled style (secondary) but still show icon + tooltip
            button(delete_icon).style(button::secondary).padding(6)
        };

        // Wrap in tooltip with localized text
        let del_btn: tooltip::Tooltip<'_, Message> = tooltip(
            delete_button,
            container(text(delete_label)).style(container::rounded_box),
            tooltip::Position::Right,
        )
        .gap(10)
        .style(container::rounded_box)
        .padding(8);

        let close_btn = secondary_button(
            format!("{}", icy_engine_gui::ButtonType::Close),
            Some(Message::from(DialingDirectoryMsg::Close)),
        );

        let connect_btn = primary_button(
            fl!(crate::LANGUAGE_LOADER, "dialing_directory-connect-button"),
            Some(Message::from(DialingDirectoryMsg::ConnectSelected)),
        );

        let buttons = button_row(vec![close_btn.into(), connect_btn.into()]);

        container(
            row![del_btn, Space::new().width(Length::Fill), buttons]
                .spacing(DIALOG_SPACING)
                .align_y(Alignment::Center),
        )
        .padding(DIALOG_PADDING)
        .width(Length::Fill)
        .into()
    }

    pub(crate) fn update(&mut self, msg: DialingDirectoryMsg) -> Task<Message> {
        match msg {
            DialingDirectoryMsg::SelectAddress(idx) => {
                // Double-click detection
                let now = std::time::Instant::now();
                let is_double_click = if let Some(last_time) = self.last_click_time {
                    if let Some(last_idx) = self.last_clicked_index {
                        last_idx == idx && now.duration_since(last_time).as_millis() < 250
                    } else {
                        false
                    }
                } else {
                    false
                };

                self.last_click_time = Some(now);
                self.last_clicked_index = Some(idx);
                self.selected_bbs = idx;

                if is_double_click {
                    // Trigger connect on double-click
                    return self.update(DialingDirectoryMsg::ConnectSelected);
                }

                Task::none()
            }

            DialingDirectoryMsg::ToggleFavorite(idx) => {
                if idx < self.addresses.lock().addresses.len() {
                    let tmp = self.addresses.lock().addresses[idx].is_favored;
                    self.addresses.lock().addresses[idx].is_favored = !tmp;
                }
                Task::none()
            }

            DialingDirectoryMsg::ChangeFilterMode(mode) => {
                self.filter_mode = mode;
                Task::none()
            }

            DialingDirectoryMsg::FilterTextChanged(text) => {
                self.filter_text = text;
                Task::none()
            }

            DialingDirectoryMsg::AddAddress => {
                let mut new_address = self.quick_connect_address.clone();
                self.quick_connect_address = Address::default();
                new_address.system_name = new_address.address.clone();
                self.addresses.lock().addresses.push(new_address);
                self.selected_bbs = Some(self.addresses.lock().addresses.len() - 1);
                Task::none()
            }

            DialingDirectoryMsg::DeleteAddress(idx) => {
                // Instead of deleting immediately, set pending_delete
                self.pending_delete = Some(idx);
                Task::none()
            }

            DialingDirectoryMsg::ConfirmDelete(idx) => {
                // Actually delete the address
                if idx < self.addresses.lock().addresses.len() {
                    self.addresses.lock().addresses.remove(idx);
                    // Adjust selected index if needed
                    if let Some(selected) = self.selected_bbs {
                        if selected == idx {
                            self.selected_bbs = None;
                        } else if selected > idx {
                            self.selected_bbs = Some(selected - 1);
                        }
                    }

                    // Save the address book
                    if let Err(e) = self.addresses.lock().store_phone_book() {
                        eprintln!("Failed to save address book: {}", e);
                    }
                }
                self.pending_delete = None;
                Task::none()
            }

            DialingDirectoryMsg::AddressFieldChanged { id, field } => {
                let mut lock = self.addresses.lock();
                let addr = if let Some(id) = id {
                    &mut lock.addresses[id]
                } else {
                    &mut self.quick_connect_address
                };

                match field {
                    AddressFieldChange::SystemName(name) => {
                        addr.system_name = name;
                    }
                    AddressFieldChange::Address(address) => {
                        addr.address = address;
                    }
                    AddressFieldChange::ModemId(modem_id) => {
                        addr.modem_id = modem_id;
                    }
                    AddressFieldChange::User(user) => {
                        addr.user_name = user;
                    }
                    AddressFieldChange::Password(password) => {
                        addr.password = password;
                    }
                    AddressFieldChange::AutoLogin(script) => {
                        addr.auto_login = script;
                    }
                    AddressFieldChange::Protocol(protocol) => {
                        addr.protocol = protocol;
                    }
                    AddressFieldChange::Terminal(terminal) => {
                        addr.terminal_type = terminal;
                        // Reset screen mode when terminal changes
                        addr.screen_mode = terminal_settings_ui::get_default_screen_mode(terminal);
                    }
                    AddressFieldChange::ScreenMode(mode) => {
                        addr.screen_mode = mode;
                    }
                    AddressFieldChange::Baud(baud) => {
                        addr.baud_emulation = baud;
                    }
                    AddressFieldChange::Music(music) => {
                        addr.ansi_music = music;
                    }
                    AddressFieldChange::Comment(comment) => {
                        addr.comment = comment;
                    }
                    AddressFieldChange::IsFavored(is_favored) => {
                        addr.is_favored = is_favored;
                    }
                    AddressFieldChange::MouseReporting(enabled) => {
                        addr.mouse_reporting_enabled = enabled;
                    }
                }

                Task::none()
            }

            DialingDirectoryMsg::ToggleShowPasswords => {
                self.show_passwords = !self.show_passwords;
                Task::none()
            }

            DialingDirectoryMsg::ConnectSelected => {
                // Get the selected address
                let addr = if let Some(idx) = self.selected_bbs {
                    if idx < self.addresses.lock().addresses.len() {
                        self.addresses.lock().addresses[idx].clone()
                    } else {
                        return Task::none();
                    }
                } else {
                    self.quick_connect_address.clone()
                };

                // Increment call counter for the selected address
                if let Some(idx) = self.selected_bbs {
                    self.addresses.lock().addresses[idx].number_of_calls += 1;
                    self.addresses.lock().addresses[idx].last_call = Some(chrono::Utc::now());
                }

                // Save the address book
                if let Err(e) = self.addresses.lock().store_phone_book() {
                    eprintln!("Failed to save address book: {}", e);
                }

                // Return a task that triggers the connection
                // You'll need to handle this in the parent component
                Task::done(Message::Connect(addr.into()))
            }

            DialingDirectoryMsg::Close => {
                // Cancel the delete operation
                if self.pending_delete.is_some() {
                    self.pending_delete = None;
                    return Task::none();
                }

                // Save any changes before closing
                if let Err(e) = self.addresses.lock().store_phone_book() {
                    eprintln!("Failed to save address book: {}", e);
                }
                // Return a task that closes the dialog
                Task::done(crate::ui::Message::CloseDialog(Box::new(MainWindowMode::ShowTerminal)))
            }

            DialingDirectoryMsg::GeneratePassword => {
                // Generate a random password
                let mut pw = String::new();
                for _ in 0..16 {
                    pw.push(unsafe { char::from_u32_unchecked(fastrand::u8(b'0'..=b'z') as u32) });
                }
                let mut lock = self.addresses.lock();
                let addr = if let Some(id) = self.selected_bbs {
                    &mut lock.addresses[id]
                } else {
                    &mut self.quick_connect_address
                };

                addr.password = pw;
                Task::none()
            }

            DialingDirectoryMsg::NavigateUp => {
                let addresses = self.filtered();

                if let Some(selected_idx) = self.selected_bbs {
                    // Find current selection in filtered list
                    if let Some((pos, _)) = addresses.iter().enumerate().find(|(_, (idx, _))| *idx == selected_idx) {
                        if pos > 0 {
                            // Select previous item
                            let (new_idx, _) = addresses[pos - 1];
                            self.selected_bbs = Some(new_idx);
                        } else {
                            // At top, move to quick connect if available
                            let show_quick_connect = self.filter_text.is_empty() && matches!(self.filter_mode, DialingDirectoryFilter::All);
                            if show_quick_connect {
                                self.selected_bbs = None;
                            }
                        }
                    }
                } else if !addresses.is_empty() {
                    // No selection (on quick connect), select last item
                    let (idx, _) = addresses[addresses.len() - 1];
                    self.selected_bbs = Some(idx);
                }
                Task::none()
            }

            DialingDirectoryMsg::NavigateDown => {
                let addresses = self.filtered();
                let show_quick_connect = self.filter_text.is_empty() && matches!(self.filter_mode, DialingDirectoryFilter::All);

                if let Some(selected_idx) = self.selected_bbs {
                    // Find current selection in filtered list
                    if let Some((pos, _)) = addresses.iter().enumerate().find(|(_, (idx, _))| *idx == selected_idx) {
                        if pos + 1 < addresses.len() {
                            // Select next item
                            let (new_idx, _) = addresses[pos + 1];
                            self.selected_bbs = Some(new_idx);
                        } else if show_quick_connect {
                            // At bottom, wrap to quick connect
                            self.selected_bbs = None;
                        }
                    }
                } else if !addresses.is_empty() {
                    // Currently on quick connect, select first item
                    let (idx, _) = addresses[0];
                    self.selected_bbs = Some(idx);
                }
                Task::none()
            }
        }
    }

    pub(crate) fn handle_event(&self, event: &icy_ui::Event) -> Option<Message> {
        match event {
            Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => match key {
                keyboard::Key::Named(keyboard::key::Named::Tab) => {
                    if modifiers.shift() {
                        Some(Message::FocusPrevious)
                    } else {
                        Some(Message::FocusNext)
                    }
                }
                _ => None,
            },
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum DialingDirectoryMsg {
    SelectAddress(Option<usize>),
    ToggleFavorite(usize),
    ChangeFilterMode(DialingDirectoryFilter),
    FilterTextChanged(String),
    AddAddress,
    DeleteAddress(usize),
    AddressFieldChanged { id: Option<usize>, field: AddressFieldChange },
    ToggleShowPasswords,
    GeneratePassword,
    ConnectSelected,
    Close,
    NavigateUp,
    NavigateDown,
    ConfirmDelete(usize),
}

#[derive(Debug, Clone)]
pub enum AddressFieldChange {
    SystemName(String),
    Address(String),
    ModemId(String),
    User(String),
    Password(String),
    AutoLogin(String),
    Protocol(ConnectionType),
    Terminal(TerminalEmulation),
    ScreenMode(ScreenMode),
    Baud(BaudEmulation),
    Music(MusicOption),
    Comment(String),
    IsFavored(bool),
    MouseReporting(bool),
}

impl From<DialingDirectoryMsg> for Message {
    fn from(m: DialingDirectoryMsg) -> Self {
        Message::DialingDirectory(m)
    }
}
