use eframe::egui;
use icy_engine::{ScreenMode, TerminalResolution};
use icy_net::{proxy::ProxyConfig, telnet::TerminalEmulation, ConnectionType};
use icy_parser_core::{BaudEmulation, MusicOption};
use icy_term::{Address, Options, SshAuthenticationMode};

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
}

impl ProfileEditor {
    pub fn show(&mut self, ui: &mut egui::Ui, entry: &mut Address, options: &Options, show_password: &mut bool, eye: &egui::TextureHandle) {
        ui.horizontal_wrapped(|ui| {
            for (page, label) in [
                (Page::Connection, &*tr!("egui-connection")),
                (Page::Terminal, &*tr!("settings-terminal-category")),
                (Page::Login, &*tr!("egui-login")),
                (Page::Colors, &*tr!("egui-colors")),
                (Page::Notes, &*tr!("dialing_directory-notes")),
            ] {
                super::super::appearance::tab(ui, &mut self.page, page, label);
            }
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt(("profile-page", self.page as u8))
            .max_height((ui.available_height() - 56.0).max(120.0))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                match self.page {
                    Page::Connection => self.connection(ui, entry, options, *show_password),
                    Page::Terminal => terminal(ui, entry),
                    Page::Login => {
                        login(ui, entry, show_password, eye);
                        if ui
                            .add_enabled(entry.password.is_empty(), egui::Button::new(&*tr!("egui-generate-password")))
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

    fn connection(&mut self, ui: &mut egui::Ui, entry: &mut Address, options: &Options, show_password: bool) {
        text_field(ui, &*tr!("egui-system-name"), &mut entry.system_name);
        text_field(
            ui,
            &if entry.protocol == ConnectionType::Modem {
                tr!("egui-phone-number")
            } else {
                tr!("dialing_directory-address")
            },
            &mut entry.address,
        );
        ui.label(&*tr!("dialing_directory-protocol"));
        egui::ComboBox::from_id_salt("protocol")
            .selected_text(protocol_name(entry.protocol))
            .show_ui(ui, |ui| {
                for protocol in icy_term::data::addresses::ALL {
                    ui.selectable_value(&mut entry.protocol, protocol, protocol_name(protocol));
                }
            });
        ui.checkbox(&mut entry.is_favored, &*tr!("egui-favorite"));
        if entry.protocol == ConnectionType::Modem {
            ui.label(&*tr!("dialing_directory-modem"));
            egui::ComboBox::from_id_salt("modem")
                .selected_text(if entry.modem_id.is_empty() {
                    tr!("egui-select-modem")
                } else {
                    entry.modem_id.clone()
                })
                .show_ui(ui, |ui| {
                    for modem in &options.modems {
                        ui.selectable_value(&mut entry.modem_id, modem.name.clone(), &modem.name);
                    }
                });
            if !options.modems.iter().any(|modem| modem.name == entry.modem_id) {
                ui.colored_label(ui.visuals().warn_fg_color, &*tr!("egui-no-modem-selected"));
            }
        }
        if matches!(
            entry.protocol,
            ConnectionType::Telnet | ConnectionType::Raw | ConnectionType::SSH | ConnectionType::Rlogin | ConnectionType::RloginSwapped
        ) {
            ui.separator();
            ui.strong(&*tr!("dialing_directory-proxy"));
            let mut preset = match &entry.proxy {
                None => 0,
                Some(proxy) if proxy.host == "127.0.0.1" && proxy.port == 9050 => 1,
                Some(proxy) if proxy.host == "127.0.0.1" && proxy.port == 4447 => 2,
                Some(_) => 3,
            };
            let previous = preset;
            let labels = [&*tr!("egui-direct-connection"), "Tor (SOCKS5)", "I2P (SOCKS5)", &*tr!("egui-custom-socks")];
            egui::ComboBox::from_id_salt("proxy").selected_text(labels[preset]).show_ui(ui, |ui| {
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
                ui.horizontal(|ui| {
                    ui.label(&*tr!("egui-port"));
                    ui.add(egui::DragValue::new(&mut proxy.port).range(1..=65535));
                });
                let mut user = proxy.username.clone().unwrap_or_default();
                text_field(ui, &*tr!("egui-proxy-user"), &mut user);
                proxy.username = (!user.is_empty()).then_some(user);
                ui.label(&*tr!("egui-proxy-password"));
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut self.proxy_password)
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
    ui.label(&*tr!("egui-terminal-emulation"));
    let previous = entry.terminal_type;
    egui::ComboBox::from_id_salt("emulation")
        .selected_text(icy_term::fmt_terminal_emulation(&entry.terminal_type))
        .show_ui(ui, |ui| {
            for terminal in icy_term::ALL_TERMINALS {
                ui.selectable_value(&mut entry.terminal_type, terminal, icy_term::fmt_terminal_emulation(&terminal));
            }
        });
    if previous != entry.terminal_type {
        entry.screen_mode = icy_term::normalize_screen_mode(entry.terminal_type, ScreenMode::default());
    }
    match &mut entry.screen_mode {
        ScreenMode::Vga(width, height) | ScreenMode::Unicode(width, height) => {
            ui.label(&*tr!("egui-screen-size"));
            egui::ComboBox::from_id_salt("size-preset")
                .selected_text(format!("{width} x {height}"))
                .show_ui(ui, |ui| {
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
            ui.horizontal_wrapped(|ui| {
                ui.label(&*tr!("egui-columns"));
                ui.add(egui::DragValue::new(width).range(1..=500));
                ui.label(&*tr!("egui-rows"));
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
            ui.label(&*tr!("egui-resolution"));
            egui::ComboBox::from_id_salt("st-resolution")
                .selected_text(format!("{resolution:?}"))
                .show_ui(ui, |ui| {
                    for value in [TerminalResolution::Low, TerminalResolution::Medium, TerminalResolution::High] {
                        ui.selectable_value(resolution, value, format!("{value:?}"));
                    }
                });
            ui.checkbox(igs, &*tr!("egui-igs-graphics"));
        }
        mode => {
            ui.label(format!("Screen: {mode}"));
        }
    }
    ui.label(&*tr!("dialing_directory-baud-emulation"));
    egui::ComboBox::from_id_salt("baud")
        .selected_text(entry.baud_emulation.to_string())
        .show_ui(ui, |ui| {
            for baud in BaudEmulation::OPTIONS {
                ui.selectable_value(&mut entry.baud_emulation, baud, baud.to_string());
            }
        });
    if matches!(entry.terminal_type, TerminalEmulation::Ansi | TerminalEmulation::Utf8Ansi) {
        ui.label(&*tr!("egui-ansi-music"));
        egui::ComboBox::from_id_salt("music")
            .selected_text(entry.ansi_music.to_string())
            .show_ui(ui, |ui| {
                for music in [MusicOption::Off, MusicOption::Banana, MusicOption::Conflicting, MusicOption::Both] {
                    ui.selectable_value(&mut entry.ansi_music, music, music.to_string());
                }
            });
    }
    ui.label(&*tr!("egui-font"));
    egui::ComboBox::from_id_salt("font")
        .width(ui.available_width().min(280.0))
        .selected_text(entry.font_name.as_deref().unwrap_or(&*tr!("egui-terminal-default")))
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut entry.font_name, None, &*tr!("egui-terminal-default"));
            for name in icy_engine::get_sauce_font_names() {
                ui.selectable_value(&mut entry.font_name, Some(name.into()), name);
            }
        });
    let mut lf_expand = entry.lf_expand();
    if ui.checkbox(&mut lf_expand, &*tr!("egui-lf-expand")).changed() {
        entry.set_lf_expand(lf_expand);
    }
    ui.checkbox(&mut entry.mouse_reporting_enabled, &*tr!("egui-mouse-reporting"));
}

fn login(ui: &mut egui::Ui, entry: &mut Address, show_password: &mut bool, eye: &egui::TextureHandle) {
    text_field(ui, &*tr!("egui-user-name"), &mut entry.user_name);
    ui.label(&*tr!("dialing_directory-password"));
    ui.horizontal(|ui| {
        ui.add_sized(
            [(ui.available_width() - 36.0).max(40.0), 28.0],
            egui::TextEdit::singleline(&mut entry.password).password(!*show_password),
        );
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
    if entry.protocol == ConnectionType::SSH {
        ui.label(&*tr!("dialing_directory-ssh-authentication"));
        egui::ComboBox::from_id_salt("ssh-auth")
            .selected_text(entry.ssh_authentication.to_string())
            .show_ui(ui, |ui| {
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
            ui.label(&*tr!("egui-key-passphrase"));
            ui.add(
                egui::TextEdit::singleline(&mut entry.ssh_key_passphrase)
                    .password(!*show_password)
                    .desired_width(f32::INFINITY),
            );
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
