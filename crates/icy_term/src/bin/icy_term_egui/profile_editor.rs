use eframe::egui;
use icy_engine::{ScreenMode, TerminalResolution};
use icy_net::{proxy::ProxyConfig, telnet::TerminalEmulation, ConnectionType};
use icy_parser_core::{BaudEmulation, MusicOption};
use icy_term::{Address, Options, SshAuthenticationMode};

use super::super::appearance;
use super::{icon_button, text_field};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Connection,
    Terminal,
    Login,
    Colors,
    Notes,
}

#[derive(Default)]
pub struct ProfileEditor {
    pub page: Page,
    proxy_password: String,
    error: Option<String>,
    connect_requested: bool,
}

impl ProfileEditor {
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        entry: &mut Address,
        options: &Options,
        show_password: &mut bool,
        eye: &egui::TextureHandle,
        icons: &mut super::IconCache,
    ) {
        self.show_inner(ui, entry, options, show_password, eye, icons, false);
    }

    pub fn show_quick(
        &mut self,
        ui: &mut egui::Ui,
        entry: &mut Address,
        options: &Options,
        show_password: &mut bool,
        eye: &egui::TextureHandle,
        icons: &mut super::IconCache,
    ) -> bool {
        self.connect_requested = false;
        self.show_inner(ui, entry, options, show_password, eye, icons, true);
        self.connect_requested
    }

    #[allow(clippy::too_many_arguments)]
    fn show_inner(
        &mut self,
        ui: &mut egui::Ui,
        entry: &mut Address,
        options: &Options,
        show_password: &mut bool,
        eye: &egui::TextureHandle,
        icons: &mut super::IconCache,
        quick: bool,
    ) {
        let pages = [
            (Page::Connection, &*tr!("egui-connection")),
            (Page::Terminal, &*tr!("settings-terminal-category")),
            (Page::Login, &*tr!("egui-login")),
            (Page::Colors, &*tr!("egui-colors")),
            (Page::Notes, &*tr!("dialing_directory-notes")),
        ];
        if ui.available_height() < 300.0 {
            egui::ComboBox::from_id_salt("profile-category")
                .width(ui.available_width())
                .selected_text(pages.iter().find(|(page, _)| *page == self.page).unwrap().1)
                .show_ui(ui, |ui| {
                    for (page, label) in pages {
                        ui.selectable_value(&mut self.page, page, label);
                    }
                });
        } else {
            ui.horizontal_wrapped(|ui| {
                for (page, label) in pages {
                    super::super::appearance::tab(ui, &mut self.page, page, label);
                }
            });
        }
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt(("profile-page", self.page as u8))
            .min_scrolled_height(0.0)
            .max_height(ui.available_height())
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                match self.page {
                    Page::Connection => self.connection(ui, entry, options, *show_password, icons, quick),
                    Page::Terminal => terminal(ui, entry),
                    Page::Login => {
                        login(ui, entry, show_password, eye);
                        if ui
                            .add_enabled(entry.password.is_empty(), egui::Button::new(&*tr!("egui-generate-password")))
                            .on_hover_text(tr!("dialing_directory-generate-tooltip"))
                            .on_disabled_hover_text(tr!("dialing_directory-generate-disabled-tooltip"))
                            .clicked()
                            && !ui.ctx().will_discard()
                        {
                            match generate_password() {
                                Ok(password) => {
                                    entry.password = password;
                                    self.error = None;
                                }
                                Err(error) => self.error = Some(error),
                            }
                        }
                        if let Some(error) = &self.error {
                            ui.colored_label(ui.visuals().error_fg_color, error);
                        }
                    }
                    Page::Colors => colors(ui, entry),
                    Page::Notes => {
                        ui.label(&*tr!("dialing_directory-notes"));
                        ui.add(egui::TextEdit::multiline(&mut entry.comment).desired_width(f32::INFINITY).desired_rows(10));
                    }
                }
            });
    }

    fn connection(&mut self, ui: &mut egui::Ui, entry: &mut Address, options: &Options, show_password: bool, icons: &mut super::IconCache, quick: bool) {
        if !quick {
            text_field(ui, &*tr!("egui-system-name"), &mut entry.system_name);
            icon_row(ui, entry, icons);
        }
        let address_id = ui.make_persistent_id("profile-address");
        self.connect_requested =
            quick && ui.is_enabled() && ui.memory(|memory| memory.has_focus(address_id)) && ui.input(|input| input.key_pressed(egui::Key::Enter));
        appearance::form_row(
            ui,
            &if entry.protocol == ConnectionType::Modem {
                tr!("egui-phone-number")
            } else {
                tr!("dialing_directory-address")
            },
            |ui| {
                if ui
                    .add(appearance::text_edit(&mut entry.address).id(address_id).desired_width(f32::INFINITY))
                    .changed()
                    && quick
                {
                    if let Ok(info) = icy_term::ConnectionInformation::parse(&entry.address) {
                        if let Some(protocol) = info.protocol {
                            entry.protocol = protocol;
                        }
                    }
                }
            },
        );
        appearance::combo_row(ui, &tr!("dialing_directory-protocol"), protocol_name(entry.protocol), |ui| {
            for protocol in icy_term::data::addresses::ALL {
                ui.selectable_value(&mut entry.protocol, protocol, protocol_name(protocol));
            }
        });
        if !quick {
            ui.checkbox(&mut entry.is_favored, &*tr!("egui-favorite"));
        }
        if entry.protocol == ConnectionType::Modem {
            appearance::combo_row(
                ui,
                &tr!("dialing_directory-modem"),
                if entry.modem_id.is_empty() {
                    tr!("egui-select-modem")
                } else {
                    entry.modem_id.clone()
                },
                |ui| {
                    for modem in &options.modems {
                        ui.selectable_value(&mut entry.modem_id, modem.name.clone(), &modem.name);
                    }
                },
            );
            if !options.modems.iter().any(|modem| modem.name == entry.modem_id) {
                ui.colored_label(ui.visuals().warn_fg_color, &*tr!("egui-no-modem-selected"));
            }
        }
        if matches!(
            entry.protocol,
            ConnectionType::Telnet | ConnectionType::Raw | ConnectionType::SSH | ConnectionType::Rlogin | ConnectionType::RloginSwapped
        ) {
            ui.separator();
            appearance::section(ui, &tr!("dialing_directory-proxy"));
            let mut preset = match &entry.proxy {
                None => 0,
                Some(proxy) if proxy.host == "127.0.0.1" && proxy.port == 9050 => 1,
                Some(proxy) if proxy.host == "127.0.0.1" && proxy.port == 4447 => 2,
                Some(_) => 3,
            };
            let previous = preset;
            let labels = [&*tr!("egui-direct-connection"), "Tor (SOCKS5)", "I2P (SOCKS5)", &*tr!("egui-custom-socks")];
            appearance::combo_row(ui, &tr!("egui-connection"), labels[preset], |ui| {
                for (index, label) in labels.iter().enumerate() {
                    ui.selectable_value(&mut preset, index, *label);
                }
            });
            if preset != previous {
                entry.proxy = match preset {
                    0 => None,
                    1 => Some(ProxyConfig::socks5("127.0.0.1", 9050)),
                    2 => Some(ProxyConfig::socks5("127.0.0.1", 4447)),
                    _ => Some(entry.proxy.clone().unwrap_or_else(|| ProxyConfig::socks5("127.0.0.1", 1080))),
                };
                self.proxy_password.clear();
            }
            if let Some(proxy) = &mut entry.proxy {
                text_field(ui, &*tr!("dialing_directory-proxy-host"), &mut proxy.host);
                appearance::form_row(ui, &tr!("egui-port"), |ui| {
                    ui.add(egui::DragValue::new(&mut proxy.port).range(1..=65535));
                });
                let mut user = proxy.username.clone().unwrap_or_default();
                text_field(ui, &*tr!("egui-proxy-user"), &mut user);
                proxy.username = (!user.is_empty()).then_some(user);
                ui.label(&*tr!("egui-proxy-password"));
                if ui
                    .add(
                        appearance::text_edit(&mut self.proxy_password)
                            .password(!show_password)
                            .desired_width(f32::INFINITY)
                            .hint_text(if proxy.password.is_some() {
                                tr!("egui-stored-password")
                            } else {
                                String::new()
                            }),
                    )
                    .changed()
                {
                    proxy.password = (!self.proxy_password.is_empty()).then(|| icy_net::ssh::SecretString::new(self.proxy_password.clone()));
                }
                if proxy.password.is_some() && ui.button(&*tr!("egui-clear-proxy-password")).clicked() {
                    proxy.password = None;
                    self.proxy_password.clear();
                }
            }
        }
        if entry.protocol == ConnectionType::SSH {
            text_field(ui, &*tr!("egui-proxy-command"), &mut entry.proxy_command);
        }
        if matches!(entry.protocol, ConnectionType::Websocket | ConnectionType::SecureWebsocket) && entry.proxy.is_some() {
            ui.colored_label(ui.visuals().warn_fg_color, &*tr!("egui-websocket-proxy-error"));
            if ui.button(&*tr!("egui-remove-proxy")).clicked() {
                entry.proxy = None;
                self.proxy_password.clear();
            }
        }
    }
}

fn icon_row(ui: &mut egui::Ui, entry: &mut Address, icons: &mut super::IconCache) {
    appearance::form_row(ui, &tr!("egui-icon"), |ui| {
        ui.horizontal_wrapped(|ui| {
            let texture = entry.icon.clone().and_then(|path| icons.get(ui.ctx(), &path).cloned());
            let (preview, _) = ui.allocate_exact_size(egui::vec2(32.0, 32.0), egui::Sense::hover());
            match &texture {
                Some(texture) => egui::Image::new(texture).corner_radius(6).paint_at(ui, preview),
                None => super::paint_monogram(ui, preview, &entry.system_name),
            }
            if ui.button(&*tr!("egui-icon-choose")).on_hover_text(tr!("egui-icon-tooltip")).clicked() && !ui.ctx().will_discard() {
                if let Some(path) = rfd::FileDialog::new().add_filter("Image", &super::ICON_EXTENSIONS).pick_file() {
                    entry.icon = Some(path.to_string_lossy().into_owned());
                }
            }
            if ui.add_enabled(entry.icon.is_some(), egui::Button::new(&*tr!("egui-icon-remove"))).clicked() {
                entry.icon = None;
            }
            if entry.icon.is_some() && texture.is_none() {
                ui.colored_label(ui.visuals().error_fg_color, &*tr!("egui-icon-missing"));
            }
        });
    });
}

pub fn protocol_name(protocol: ConnectionType) -> &'static str {
    match protocol {
        ConnectionType::Telnet => "Telnet",
        ConnectionType::Raw => "Raw TCP",
        ConnectionType::SSH => "SSH",
        ConnectionType::Modem => "Modem",
        ConnectionType::Websocket => "WebSocket",
        ConnectionType::SecureWebsocket => "Secure WebSocket",
        ConnectionType::Rlogin => "Rlogin",
        ConnectionType::RloginSwapped => "Rlogin (swapped)",
        ConnectionType::Serial => "Serial",
        ConnectionType::Channel => "Channel",
    }
}

pub(crate) fn terminal(ui: &mut egui::Ui, entry: &mut Address) {
    let previous = entry.terminal_type;
    appearance::combo_row(ui, &tr!("egui-terminal-emulation"), icy_term::fmt_terminal_emulation(&previous), |ui| {
        for terminal in icy_term::ALL_TERMINALS {
            ui.selectable_value(&mut entry.terminal_type, terminal, icy_term::fmt_terminal_emulation(&terminal));
        }
    });
    if previous != entry.terminal_type {
        entry.screen_mode = icy_term::normalize_screen_mode(entry.terminal_type, ScreenMode::default());
    }
    match &mut entry.screen_mode {
        ScreenMode::Vga(width, height) | ScreenMode::Unicode(width, height) => {
            appearance::combo_row(ui, &tr!("egui-screen-size"), format!("{width} x {height}"), |ui| {
                for (columns, rows) in [(80, 25), (80, 50), (132, 37), (132, 52)] {
                    if ui
                        .selectable_label(*width == columns && *height == rows, format!("{columns} x {rows}"))
                        .clicked()
                    {
                        *width = columns;
                        *height = rows;
                    }
                }
            });
            appearance::form_row(ui, &tr!("egui-columns"), |ui| {
                ui.add(egui::DragValue::new(width).range(1..=500));
            });
            appearance::form_row(ui, &tr!("egui-rows"), |ui| {
                ui.add(egui::DragValue::new(height).range(1..=200));
            });
        }
        ScreenMode::Atascii(width) => {
            let mut xep80 = *width == 80;
            if ui.checkbox(&mut xep80, &*tr!("egui-xep80-module")).changed() {
                *width = if xep80 { 80 } else { 40 };
            }
        }
        ScreenMode::AtariST(resolution, igs) => {
            appearance::combo_row(ui, &tr!("egui-resolution"), format!("{resolution:?}"), |ui| {
                for value in [TerminalResolution::Low, TerminalResolution::Medium, TerminalResolution::High] {
                    ui.selectable_value(resolution, value, format!("{value:?}"));
                }
            });
            ui.checkbox(igs, &*tr!("egui-igs-graphics"));
        }
        mode => {
            appearance::value_row(ui, &tr!("dialing_directory-screen_mode"), &mode.to_string());
        }
    }
    ui.separator();
    appearance::combo_row(ui, &tr!("dialing_directory-baud-emulation"), entry.baud_emulation.to_string(), |ui| {
        for baud in BaudEmulation::OPTIONS {
            ui.selectable_value(&mut entry.baud_emulation, baud, baud.to_string());
        }
    });
    if matches!(entry.terminal_type, TerminalEmulation::Ansi | TerminalEmulation::Utf8Ansi) {
        appearance::combo_row(ui, &tr!("egui-ansi-music"), entry.ansi_music.to_string(), |ui| {
            for music in [MusicOption::Off, MusicOption::Banana, MusicOption::Conflicting, MusicOption::Both] {
                ui.selectable_value(&mut entry.ansi_music, music, music.to_string());
            }
        });
    }
    appearance::combo_row(
        ui,
        &tr!("egui-font"),
        entry.font_name.clone().unwrap_or_else(|| tr!("egui-terminal-default")),
        |ui| {
            ui.selectable_value(&mut entry.font_name, None, &*tr!("egui-terminal-default"));
            for name in icy_engine::get_sauce_font_names() {
                ui.selectable_value(&mut entry.font_name, Some(name.into()), name);
            }
        },
    );
    let mut lf_expand = entry.lf_expand();
    if ui.checkbox(&mut lf_expand, &*tr!("egui-lf-expand")).changed() {
        entry.set_lf_expand(lf_expand);
    }
    ui.checkbox(&mut entry.mouse_reporting_enabled, &*tr!("egui-mouse-reporting"));
}

fn login(ui: &mut egui::Ui, entry: &mut Address, show_password: &mut bool, eye: &egui::TextureHandle) {
    text_field(ui, &*tr!("egui-user-name"), &mut entry.user_name);
    appearance::form_row(ui, &tr!("dialing_directory-password"), |ui| {
        ui.horizontal(|ui| {
            // desired_width covers the text area only, so leave room for the field's own padding.
            let width = (ui.available_width() - 36.0 - 2.0 * appearance::FIELD_MARGIN.x).max(40.0);
            ui.add(appearance::text_edit(&mut entry.password).password(!*show_password).desired_width(width));
            if icon_button(
                ui,
                eye,
                &if *show_password {
                    tr!("egui-hide-password")
                } else {
                    tr!("egui-show-password")
                },
            )
            .clicked()
            {
                *show_password = !*show_password;
            }
        });
    });
    if entry.protocol == ConnectionType::SSH {
        appearance::combo_row(ui, &tr!("dialing_directory-ssh-authentication"), entry.ssh_authentication.to_string(), |ui| {
            for mode in [
                SshAuthenticationMode::Password,
                SshAuthenticationMode::PrivateKey,
                SshAuthenticationMode::Agent,
                SshAuthenticationMode::Auto,
            ] {
                ui.selectable_value(&mut entry.ssh_authentication, mode, mode.to_string());
            }
        });
        if matches!(entry.ssh_authentication, SshAuthenticationMode::PrivateKey | SshAuthenticationMode::Auto) {
            text_field(ui, &*tr!("dialing_directory-ssh-private-key"), &mut entry.ssh_private_key);
            if ui.button(&*tr!("egui-browse")).clicked() && !ui.ctx().will_discard() {
                if let Some(path) = rfd::FileDialog::new().set_title(&*tr!("egui-select-ssh-key")).pick_file() {
                    entry.ssh_private_key = path.to_string_lossy().into();
                }
            }
            appearance::form_row(ui, &tr!("egui-key-passphrase"), |ui| {
                ui.add(
                    appearance::text_edit(&mut entry.ssh_key_passphrase)
                        .password(!*show_password)
                        .desired_width(f32::INFINITY),
                );
            });
        }
    }
    ui.separator();
    text_field(ui, &*tr!("egui-auto-login-expression"), &mut entry.auto_login);
    egui::ComboBox::from_id_salt("auto-login-presets")
        .selected_text(&*tr!("egui-login-presets"))
        .show_ui(ui, |ui| {
            for (label, expression) in [
                (&*tr!("egui-interactive"), ""),
                (&*tr!("egui-name-password"), "@W@N@P"),
                (&*tr!("egui-escapes-name-password"), "@E@W@N@P"),
                (&*tr!("egui-disable-iemsi"), "@I"),
            ] {
                if ui.selectable_label(entry.auto_login == expression, label).clicked() {
                    entry.auto_login = expression.into();
                }
            }
        });
}

pub(crate) fn colors(ui: &mut egui::Ui, entry: &mut Address) {
    ui.checkbox(&mut entry.ice_mode, &*tr!("egui-ice-colors"));
    let mut custom = entry.custom_palette.is_some();
    if ui.checkbox(&mut custom, &*tr!("dialing_directory-custom-palette")).changed() {
        entry.custom_palette = custom.then(default_palette);
    }
    if let Some(palette) = &mut entry.custom_palette {
        egui::Grid::new("palette").spacing(egui::vec2(12.0, 10.0)).show(ui, |ui| {
            for (index, color) in palette.iter_mut().enumerate() {
                ui.vertical(|ui| {
                    ui.color_edit_button_srgb(color).on_hover_text(format!("Color {index}"));
                    ui.small(format!("{index:02}"));
                });
                if index % 4 == 3 {
                    ui.end_row();
                }
            }
        });
        if ui.button(&*tr!("egui-reset-palette")).clicked() {
            *palette = default_palette();
        }
    }
}

fn default_palette() -> Vec<[u8; 3]> {
    icy_engine::DOS_DEFAULT_PALETTE
        .iter()
        .map(|color| {
            let (red, green, blue) = color.rgb();
            [red, green, blue]
        })
        .collect()
}

fn generate_password() -> Result<String, String> {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut random = [0; 20];
    getrandom::fill(&mut random).map_err(|_| "System random generator unavailable".to_string())?;
    Ok(random.iter().map(|byte| ALPHABET[(byte & 63) as usize] as char).collect())
}

#[cfg(test)]
mod tests {
    #[test]
    fn generated_password_is_ascii_and_random() {
        let first = super::generate_password().unwrap();
        assert_eq!(first.len(), 20);
        assert!(first.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte)));
        assert_ne!(first, super::generate_password().unwrap());
    }
}
