use std::{path::PathBuf, sync::Arc, time::Duration};

use clap::Parser;
use eframe::{egui, egui_wgpu};
use icy_engine::{formats::FileFormat, TextScreen};
use icy_engine_gui::{
    terminal::egui::TerminalCallback, Blink, CRTShaderProgram, CRTShaderState, MonitorSettings, ScalingMode, Terminal, TerminalShaderRenderer,
};
use parking_lot::Mutex;

macro_rules! tr {
    ($key:literal) => {
        i18n_embed_fl::fl!(icy_term::LANGUAGE_LOADER, $key)
    };
    ($key:literal, $($args:tt)*) => {
        i18n_embed_fl::fl!(icy_term::LANGUAGE_LOADER, $key, $($args)*)
    };
}

#[path = "icy_term_egui/appearance.rs"]
mod appearance;
use icy_engine::TextPane;
#[path = "icy_term_egui/audio.rs"]
mod audio;
#[path = "icy_term_egui/dialing_directory.rs"]
mod dialing_directory;
#[path = "icy_term_egui/hotkeys.rs"]
mod hotkeys;
#[path = "icy_term_egui/input.rs"]
mod input;
#[path = "icy_term_egui/mcp.rs"]
mod mcp;
#[path = "icy_term_egui/messages.rs"]
mod messages;
#[path = "icy_term_egui/navigation.rs"]
mod navigation;
#[path = "icy_term_egui/overlays.rs"]
mod overlays;
#[path = "icy_term_egui/phonebook.rs"]
mod phonebook;
#[path = "icy_term_egui/session.rs"]
mod session;
#[path = "icy_term_egui/settings.rs"]
mod settings;
#[path = "icy_term_egui/terminal_info.rs"]
mod terminal_info;
#[path = "icy_term_egui/tools.rs"]
mod tools;
#[path = "icy_term_egui/transfers.rs"]
mod transfers;

#[derive(Parser)]
#[command(version, about = "Icy Term - BBS terminal")]
struct Args {
    #[arg(conflicts_with = "connect", value_name = "FILE_OR_URL")]
    file: Option<PathBuf>,
    #[arg(long, value_name = "URL")]
    connect: Option<String>,
    #[arg(long)]
    utf8: bool,
    #[arg(long)]
    mcp_port: Option<u16>,
    #[arg(short, long)]
    run: Option<PathBuf>,
    #[arg(long)]
    play: Option<PathBuf>,
    #[arg(long)]
    config: Option<PathBuf>,
    #[arg(long)]
    phonebook: Option<PathBuf>,
}

struct TerminalApp {
    terminal: Terminal,
    shader_state: CRTShaderState,
    settings: MonitorSettings,
    document_name: String,
    error: Option<String>,
    messages: messages::Messages,
    show_monitor: bool,
    address: String,
    utf8: bool,
    session: Option<session::Session>,
    connected: bool,
    focus_terminal: bool,
    terminal_input_id: Option<egui::Id>,
    dialing_directory: dialing_directory::DialingDirectory,
    connected_size: (u16, u16),
    active_profile: Option<icy_term::Address>,
    terminal_emulation: icy_net::telnet::TerminalEmulation,
    connected_at: Option<std::time::Instant>,
    navigation: navigation::Navigation,
    transfers: transfers::Transfers,
    sound: Option<icy_engine_gui::music::music::SoundThread>,
    preferences: Option<settings::Settings>,
    tools: tools::Tools,
    connecting: bool,
    mcp: Option<mcp::Bridge>,
    pending_script: Option<icy_term::mcp::SenderType<icy_term::mcp::ScriptResult>>,
    children: Vec<(egui::ViewportId, Arc<Mutex<TerminalApp>>)>,
    new_window: bool,
    confirm_close: bool,
    closed: bool,
    pending_link: Option<String>,
    cache_root: Option<PathBuf>,
    remote_focus: bool,
    styled: bool,
    icons: Option<dialing_directory::Icons>,
    about_open: bool,
    help_open: bool,
    bps_open: bool,
    baud: icy_parser_core::BaudEmulation,
    screen_mode: icy_engine::ScreenMode,
    ansi_music: icy_parser_core::MusicOption,
    latest_version: Option<semver::Version>,
    version_check: Option<std::sync::mpsc::Receiver<semver::Version>>,
}

impl TerminalApp {
    fn new(screen: TextScreen, document_name: String) -> Self {
        let shader_state = CRTShaderState::from_screen(&screen);
        let screen_mode = icy_engine::ScreenMode::Vga(screen.width(), screen.height());
        Self {
            terminal: Terminal::new(Arc::new(Mutex::new(Box::new(screen)))),
            shader_state,
            settings: MonitorSettings::default(),
            document_name,
            error: None,
            messages: Default::default(),
            show_monitor: false,
            address: String::new(),
            utf8: false,
            session: None,
            connected: false,
            focus_terminal: true,
            terminal_input_id: None,
            dialing_directory: Default::default(),
            connected_size: (80, 25),
            active_profile: None,
            terminal_emulation: Default::default(),
            connected_at: None,
            navigation: Default::default(),
            transfers: Default::default(),
            sound: None,
            preferences: None,
            tools: Default::default(),
            connecting: false,
            mcp: None,
            pending_script: None,
            children: Vec::new(),
            new_window: false,
            confirm_close: false,
            closed: false,
            pending_link: None,
            cache_root: None,
            remote_focus: false,
            styled: false,
            icons: None,
            about_open: false,
            help_open: false,
            bps_open: false,
            baud: icy_parser_core::BaudEmulation::Off,
            screen_mode,
            ansi_music: icy_parser_core::MusicOption::Off,
            latest_version: None,
            version_check: None,
        }
    }

    fn load(&mut self, path: PathBuf) {
        if self.dialing_directory.open {
            return;
        }
        if self.connected || self.connecting {
            self.error = Some("Disconnect before opening a file".into());
            return;
        }
        match load_screen(&path) {
            Ok(screen) => {
                self.session = None;
                self.shader_state = CRTShaderState::from_screen(&screen);
                self.terminal = Terminal::new(Arc::new(Mutex::new(Box::new(screen))));
                self.document_name = path.display().to_string();
                self.terminal_input_id = None;
                self.error = None;
            }
            Err(error) => self.error = Some(format!("{}: {error}", path.display())),
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        if self.icons.is_none() {
            self.icons = Some(dialing_directory::Icons::load(ui.ctx()));
        }
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let icons = self.icons.as_ref().unwrap();
            let phone = egui::Image::new(&icons.call)
                .fit_to_exact_size(egui::vec2(16.0, 16.0))
                .tint(egui::Color32::WHITE);
            if ui
                .add(egui::Button::image_and_text(phone, egui::RichText::new(tr!("egui-directory")).color(egui::Color32::WHITE)).fill(appearance::PRIMARY))
                .on_hover_text(tr!("terminal-dialing_directory"))
                .clicked()
            {
                self.dialing_directory.open();
            }
            ui.separator();
            ui.add_enabled_ui(self.connected && !self.transfers.active, |ui| {
                if dialing_directory::icon_button(ui, &icons.upload, &tr!("terminal-upload")).clicked() {
                    self.transfers.choose(false);
                }
                if dialing_directory::icon_button(ui, &icons.download, &tr!("terminal-download")).clicked() {
                    self.transfers.choose(true);
                }
            });
            if self.sound.as_mut().is_some_and(|sound| sound.is_playing()) {
                let orange = egui::Color32::from_rgb(230, 145, 40);
                let label = match self.sound.as_ref().map_or(0, |sound| sound.stop_button) {
                    0 => tr!("toolbar-stop-playing1"),
                    1 => tr!("toolbar-stop-playing2"),
                    2 => tr!("toolbar-stop-playing3"),
                    3 => tr!("toolbar-stop-playing4"),
                    4 => tr!("toolbar-stop-playing5"),
                    _ => tr!("toolbar-stop-playing6"),
                };
                if ui
                    .add(egui::Button::new(egui::RichText::new(format!("\u{1F507} {label}")).color(orange)).stroke(egui::Stroke::new(1.0, orange)))
                    .clicked()
                {
                    if let Some(sound) = &mut self.sound {
                        sound.clear();
                    }
                }
            }
            if let Some(latest) = &self.latest_version {
                if ui
                    .add(egui::Button::new(
                        egui::RichText::new(tr!("menu-upgrade_version", version = latest.to_string())).color(ui.visuals().hyperlink_color),
                    ))
                    .clicked()
                {
                    self.pending_link = Some(format!("https://github.com/mkrueger/icy_tools/releases/tag/IcyTerm{latest}"));
                }
            }
            let menu = icons.menu.clone();
            let logout = icons.logout.clone();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let image = egui::Image::new(&menu)
                    .fit_to_exact_size(egui::vec2(18.0, 18.0))
                    .tint(ui.visuals().text_color());
                ui.menu_image_button(image, |ui| self.menus(ui)).response.on_hover_text(tr!("terminal-menu"));
                if self.connected || self.connecting {
                    let label = if self.connected { tr!("egui-disconnect") } else { tr!("egui-cancel") };
                    if dialing_directory::icon_button(ui, &logout, &label).clicked() {
                        self.disconnect();
                    }
                }
            });
        });
    }

    fn menus(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.menu_button(&*tr!("egui-file"), |ui| {
                if ui.button(&*tr!("egui-new-window")).clicked() {
                    self.new_window = true;
                    ui.close();
                }
                if ui
                    .add_enabled(
                        !self.connected && !self.connecting && !self.tools.script_running,
                        egui::Button::new(tr!("egui-open-file")),
                    )
                    .clicked()
                {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Terminal art", &["ans", "asc", "txt", "icy", "xb", "bin", "pcb", "avt"])
                        .pick_file()
                    {
                        self.load(path);
                    }
                    ui.close();
                }
                if ui
                    .add(egui::Button::new(&*tr!("egui-save-screen")).shortcut_text(hotkeys::shortcut(hotkeys::Action::ExportScreen)))
                    .clicked()
                {
                    self.save_screen();
                    ui.close();
                }
                if ui
                    .add(egui::Button::new(tr!("settings-heading")).shortcut_text(hotkeys::shortcut(hotkeys::Action::Settings)))
                    .clicked()
                {
                    self.open_settings();
                    ui.close();
                }
                if ui
                    .add(egui::Button::new(&*tr!("egui-close-window")).shortcut_text(hotkeys::shortcut(hotkeys::Action::CloseWindow)))
                    .clicked()
                {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    ui.close();
                }
            });
            ui.menu_button(tr!("egui-view"), |ui| {
                ui.selectable_value(&mut self.settings.scaling_mode, ScalingMode::Auto, &*tr!("egui-fit"));
                if ui.selectable_label(!self.settings.scaling_mode.is_auto(), &*tr!("egui-zoom")).clicked() {
                    self.settings.scaling_mode = ScalingMode::Manual(1.0);
                }
                let mut zoom = match self.settings.scaling_mode {
                    ScalingMode::Manual(zoom) => zoom,
                    _ => self.terminal.get_zoom(),
                };
                if ui.button("-").on_hover_text(&*tr!("egui-zoom-out")).clicked() {
                    zoom = ScalingMode::zoom_out(zoom, self.settings.use_integer_scaling);
                    self.settings.scaling_mode = ScalingMode::Manual(zoom);
                }
                if ui
                    .add_sized(
                        [64.0, 18.0],
                        egui::DragValue::new(&mut zoom).range(0.5..=4.0).speed(0.01).fixed_decimals(2).suffix("x"),
                    )
                    .on_hover_text(&*tr!("egui-zoom"))
                    .changed()
                {
                    self.settings.scaling_mode = ScalingMode::Manual(zoom);
                }
                if ui.button("+").on_hover_text(&*tr!("egui-zoom-in")).clicked() {
                    self.settings.scaling_mode = ScalingMode::Manual(ScalingMode::zoom_in(zoom, self.settings.use_integer_scaling));
                }
                ui.separator();
                ui.toggle_value(&mut self.show_monitor, &*tr!("settings-monitor-category"));
                if ui.button(&*tr!("egui-fullscreen")).clicked() {
                    let fullscreen = ui.input(|input| input.viewport().fullscreen.unwrap_or(false));
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fullscreen));
                }
                if ui.selectable_label(self.terminal.is_in_scrollback_mode(), &*tr!("egui-history")).clicked() {
                    self.toggle_scrollback();
                    ui.close();
                }
            });
            ui.menu_button(tr!("egui-quick-connect"), |ui| self.connection_bar(ui));
            ui.menu_button(&*tr!("egui-edit"), |ui| {
                if ui
                    .add_enabled(
                        self.terminal.screen.lock().selection().is_some(),
                        egui::Button::new(&*tr!("terminal-menu-copy")),
                    )
                    .clicked()
                {
                    self.copy_selection(ui.ctx());
                    ui.close();
                }
                if ui.button(&*tr!("terminal-menu-paste")).clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                    self.focus_terminal = true;
                    ui.close();
                }
                if ui.button(&*tr!("egui-select-all")).clicked() {
                    navigation::select_all(&mut **self.terminal.screen.lock());
                    ui.close();
                }
                if ui.button(&*tr!("egui-find-command")).clicked() {
                    self.navigation.find_open = true;
                    ui.close();
                }
            });
            ui.menu_button(&*tr!("egui-session"), |ui| {
                self.transfers.menu(ui, self.connected, &self.dialing_directory.options);
                ui.separator();
                self.tools.menu(ui, self.connected || self.connecting, &self.dialing_directory.options);
                if ui.button(tr!("terminal-menu-info")).clicked() {
                    self.terminal_info();
                    ui.close();
                }
                if ui.button(&*tr!("egui-terminal-settings-command")).clicked() {
                    self.tools.terminal = Some(self.terminal_profile());
                    self.tools.scrollback = self.dialing_directory.options.max_scrollback_lines;
                    ui.close();
                }
                if let Some(sound) = &mut self.sound {
                    let options = &mut self.dialing_directory.options;
                    if ui.toggle_value(&mut options.audio_enabled, &*tr!("egui-audio")).changed() {
                        if let Err(error) = sound.configure(options.audio_enabled, options.master_volume, options.audio_device.clone()) {
                            self.error = Some(error.to_string());
                        }
                    }
                    if sound.is_playing() && ui.button(&*tr!("egui-stop-sound")).clicked() {
                        sound.clear();
                    }
                }
            });
        });
    }

    fn copy_selection(&self, context: &egui::Context) {
        if let Some(text) = navigation::selected_text(&**self.terminal.screen.lock()) {
            context.copy_text(text);
        }
    }

    fn toggle_scrollback(&mut self) {
        if navigation::toggle_scrollback(&mut self.terminal) {
            self.navigation.saved_scaling = Some(self.settings.scaling_mode);
            self.settings.scaling_mode = ScalingMode::Manual(self.terminal.get_zoom());
            self.navigation.scroll_to = Some(f32::MAX);
        } else {
            if let Some(scaling) = self.navigation.saved_scaling.take() {
                self.settings.scaling_mode = scaling;
            }
            self.navigation.scroll_to = Some(0.0);
        }
        self.focus_terminal = true;
    }

    fn connect(&mut self, context: &egui::Context) {
        let entry = match icy_term::ConnectionInformation::parse(self.address.trim()) {
            Ok(info) => {
                let mut entry = icy_term::Address::from(info);
                if self.utf8 {
                    entry.terminal_type = icy_net::telnet::TerminalEmulation::Utf8Ansi;
                    entry.screen_mode = icy_engine::ScreenMode::Unicode(80, 25);
                }
                entry
            }
            Err(error) => {
                self.error = Some(error.to_string());
                return;
            }
        };
        let config = match session::entry_connection_config(&entry, &self.dialing_directory.options) {
            Ok(config) => config,
            Err(error) => {
                self.error = Some(error);
                return;
            }
        };
        let name = config.connection_info.to_string();
        self.active_profile = Some(entry);
        self.start_connection(config, name, context);
    }

    fn disconnect(&mut self) {
        self.finish_call();
        self.finish_script_response(Err("Disconnected".into()));
        self.session = None;
        self.connected = false;
        self.connecting = false;
        self.remote_focus = false;
        self.transfers.event(&icy_term::TerminalEvent::Disconnected(None));
        self.tools.event(&icy_term::TerminalEvent::Disconnected(None));
        if let Some(sound) = &self.sound {
            sound.clear();
        }
    }

    fn start_connection(&mut self, mut config: icy_term::ConnectionConfig, name: String, context: &egui::Context) {
        if self.connected || self.connecting {
            self.error = Some("Disconnect before dialing another entry".into());
            return;
        }
        if let Some(root) = &self.cache_root {
            let key: String = config
                .connection_info
                .endpoint()
                .chars()
                .map(|character| if character.is_ascii_alphanumeric() { character } else { '_' })
                .collect();
            let directory = root.join(key);
            match std::fs::create_dir_all(&directory) {
                Ok(()) => config.cache_directory = Some(directory),
                Err(error) => {
                    self.error = Some(format!("Cache: {error}"));
                    return;
                }
            }
        }
        if self.tools.script_running && self.session.is_some() {
            self.document_name = name;
            self.connected_size = config.window_size;
            self.terminal_emulation = config.terminal_type;
            self.connecting = true;
            self.command(icy_term::TerminalCommand::Connect(config), context);
            return;
        }
        self.session = None;
        let screen = TextScreen::default();
        self.shader_state = CRTShaderState::from_screen(&screen);
        self.terminal = Terminal::new(Arc::new(Mutex::new(Box::new(screen))));
        if let Some(scaling) = self.navigation.saved_scaling.take() {
            self.settings.scaling_mode = scaling;
        }
        self.navigation = Default::default();
        self.document_name = name;
        self.connected_size = config.window_size;
        self.terminal_emulation = config.terminal_type;
        self.session = Some(session::Session::start(self.terminal.screen.clone(), config, context.clone()));
        if let (Some(session), Some(book)) = (&self.session, &self.dialing_directory.phonebook) {
            *session.address_book.lock() = book.book.clone();
        }
        self.connected = false;
        self.connecting = true;
        self.focus_terminal = true;
        self.error = None;
    }

    fn connection_bar(&mut self, ui: &mut egui::Ui) {
        let active = self.connected || self.connecting;
        let mut connect = false;
        ui.add_enabled_ui(!active, |ui| {
            ui.horizontal(|ui| {
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.address)
                        .id_salt("address")
                        .desired_width((ui.available_width() - 75.0).max(100.0))
                        .hint_text("telnet://host:port"),
                );
                connect = response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
                ui.checkbox(&mut self.utf8, "UTF-8");
            });
        });
        ui.horizontal(|ui| {
            if active {
                if ui.button(if self.connected { tr!("egui-disconnect") } else { tr!("egui-cancel") }).clicked() {
                    self.disconnect();
                }
            } else {
                connect |= ui
                    .add_enabled(!self.address.trim().is_empty(), egui::Button::new(&*tr!("dialing_directory-connect-button")))
                    .clicked();
            }
            ui.label(if self.connected {
                tr!("egui-connected")
            } else if active {
                tr!("egui-connecting")
            } else {
                tr!("egui-offline")
            });
        });
        if connect && !active {
            self.connect(ui.ctx());
        }
    }

    fn receive_events(&mut self, context: &egui::Context) {
        let mut stopped = false;
        let mut target = None;
        if let Some(session) = &self.session {
            loop {
                let event = session.events.try_recv();
                if let Ok(event) = &event {
                    self.transfers.event(event);
                    self.tools.event(event);
                    if let Some(sound) = &mut self.sound {
                        if let Err(error) = audio::dispatch(sound, event, &self.dialing_directory.options) {
                            self.error = Some(error);
                        }
                    }
                }
                match event {
                    Ok(icy_term::TerminalEvent::Connected) => {
                        self.connected = true;
                        self.connecting = false;
                        self.connected_at = Some(std::time::Instant::now());
                        self.focus_terminal = true;
                        if let (Some(book), Some(entry)) = (&mut self.dialing_directory.phonebook, &self.active_profile) {
                            if let Err(error) = book.record_call(entry) {
                                self.error = Some(format!("Call history: {error}"));
                            }
                        }
                    }
                    Ok(icy_term::TerminalEvent::Disconnected(error)) => {
                        self.error = error;
                        stopped = true;
                        break;
                    }
                    Ok(icy_term::TerminalEvent::Error(title, detail)) => self.error = Some(format!("{title}: {detail}")),
                    Ok(icy_term::TerminalEvent::TerminalSettingsChanged {
                        terminal_type,
                        screen_mode,
                        ansi_music,
                    }) => {
                        self.terminal_emulation = terminal_type;
                        self.screen_mode = screen_mode;
                        self.ansi_music = ansi_music;
                        let size = screen_mode.window_size();
                        self.connected_size = (size.width as u16, size.height as u16);
                    }
                    Ok(icy_term::TerminalEvent::ScriptFinished(result)) => {
                        if let Err(error) = &result {
                            self.error = Some(error.clone());
                        }
                        if let Some(response) = self.pending_script.take() {
                            mcp::respond(response, result.map(|()| String::new()));
                        }
                    }
                    Ok(icy_term::TerminalEvent::Connect(address)) => target = Some(address),
                    Ok(icy_term::TerminalEvent::Reconnect) => target = self.active_profile.as_ref().map(|entry| entry.system_name.clone()),
                    Ok(icy_term::TerminalEvent::SendCredentials(mode)) => {
                        if let Some(entry) = &self.active_profile {
                            let mut credentials = String::new();
                            if mode == 0 || mode == 1 {
                                credentials.push_str(&entry.user_name);
                                credentials.push('\r');
                            }
                            if mode == 0 || mode == 2 {
                                credentials.push_str(&entry.password);
                                credentials.push('\r');
                            }
                            let buffer = self.terminal.original_screen.as_ref().unwrap_or(&self.terminal.screen).lock().buffer_type();
                            let _ = session.send(input::encode_terminal_events(
                                &[egui::Event::Text(credentials)],
                                buffer,
                                false,
                                self.terminal_emulation,
                            ));
                        }
                    }
                    Ok(icy_term::TerminalEvent::Quit) => context.send_viewport_cmd(egui::ViewportCommand::Close),
                    Ok(icy_term::TerminalEvent::SerialAutoDetectComplete) => self.connecting = false,
                    Ok(_) => {}
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        self.error = Some("Terminal worker stopped".into());
                        stopped = true;
                        break;
                    }
                }
            }
        }
        if stopped {
            self.finish_script_response(Err("Terminal worker stopped".into()));
            self.finish_call();
            self.session = None;
            self.connected = false;
            self.connecting = false;
        }
        if let Some(target) = target {
            self.connect_target(&target, context);
        }
    }

    fn finish_call(&mut self) {
        if let Some(started) = self.connected_at.take() {
            if let (Some(book), Some(entry)) = (&mut self.dialing_directory.phonebook, &self.active_profile) {
                if let Err(error) = book.record_duration(entry, started.elapsed()) {
                    self.error = Some(format!("Call history: {error}"));
                }
            }
        }
    }

    fn monitor(&mut self, context: &egui::Context) {
        if !self.show_monitor {
            return;
        }
        let mut close = false;
        let width = (context.content_rect().width() - 48.0).clamp(220.0, 480.0);
        let height = (context.content_rect().height() - 120.0).clamp(110.0, 620.0);
        egui::Window::new(&*tr!("settings-monitor-category"))
            .id(egui::Id::new("monitor-settings"))
            .title_bar(false)
            .resizable(false)
            .fixed_size(egui::vec2(width, height))
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-8.0, 48.0))
            .frame(appearance::dialog_frame(context))
            .show(context, |ui| {
                ui.set_width(width);
                ui.set_min_height(height);
                close |= appearance::dialog_header(ui, &tr!("settings-monitor-category"));
                egui::ScrollArea::vertical()
                    .min_scrolled_height(0.0)
                    .max_height((height - 104.0).max(0.0))
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        use icy_engine_gui::MonitorType;
                        appearance::combo_row(ui, &tr!("egui-monitor-type"), format!("{:?}", self.settings.monitor_type), |ui| {
                            for mode in [
                                MonitorType::Color,
                                MonitorType::Grayscale,
                                MonitorType::Amber,
                                MonitorType::Green,
                                MonitorType::Apple2,
                                MonitorType::Futuristic,
                                MonitorType::CustomMonochrome,
                            ] {
                                ui.selectable_value(&mut self.settings.monitor_type, mode, format!("{mode:?}"));
                            }
                        });
                        if self.settings.monitor_type == MonitorType::CustomMonochrome {
                            let (red, green, blue) = self.settings.custom_monitor_color.rgb();
                            let mut color = [red, green, blue];
                            appearance::form_row(ui, &tr!("egui-colors"), |ui| {
                                if ui.color_edit_button_srgb(&mut color).changed() {
                                    self.settings.custom_monitor_color = icy_engine::Color::new(color[0], color[1], color[2]);
                                }
                            });
                        }
                        ui.checkbox(&mut self.settings.use_integer_scaling, &*tr!("egui-integer-scaling"));
                        ui.checkbox(&mut self.settings.use_bilinear_filtering, &*tr!("egui-bilinear-filtering"));
                        ui.separator();
                        appearance::slider_row(ui, &tr!("settings-monitor-brightness"), &mut self.settings.brightness, 0.0..=200.0);
                        appearance::slider_row(ui, &tr!("settings-monitor-contrast"), &mut self.settings.contrast, 0.0..=200.0);
                        appearance::slider_row(ui, &tr!("settings-monitor-gamma"), &mut self.settings.gamma, 0.1..=4.0);
                        appearance::slider_row(ui, &tr!("settings-monitor-saturation"), &mut self.settings.saturation, 0.0..=200.0);
                        ui.separator();
                        ui.checkbox(&mut self.settings.use_scanlines, &*tr!("settings-monitor-scanlines"));
                        if self.settings.use_scanlines {
                            appearance::slider_row(ui, &tr!("egui-thickness"), &mut self.settings.scanline_thickness, 0.0..=1.0);
                            appearance::slider_row(ui, &tr!("egui-sharpness"), &mut self.settings.scanline_sharpness, 0.0..=1.0);
                            appearance::slider_row(ui, &tr!("egui-phase"), &mut self.settings.scanline_phase, 0.0..=1.0);
                        }
                        ui.checkbox(&mut self.settings.use_bloom, &*tr!("egui-bloom"));
                        if self.settings.use_bloom {
                            appearance::slider_row(ui, &tr!("egui-threshold"), &mut self.settings.bloom_threshold, 0.0..=100.0);
                            appearance::slider_row(ui, &tr!("egui-radius"), &mut self.settings.bloom_radius, 0.0..=50.0);
                            appearance::slider_row(ui, &tr!("egui-glow"), &mut self.settings.glow_strength, 0.0..=100.0);
                            appearance::slider_row(ui, &tr!("egui-persistence"), &mut self.settings.phosphor_persistence, 0.0..=100.0);
                        }
                        ui.checkbox(&mut self.settings.use_curvature, &*tr!("egui-curvature"));
                        if self.settings.use_curvature {
                            appearance::slider_row(ui, &tr!("egui-horizontal"), &mut self.settings.curvature_x, 0.0..=100.0);
                            appearance::slider_row(ui, &tr!("egui-vertical"), &mut self.settings.curvature_y, 0.0..=100.0);
                        }
                        ui.checkbox(&mut self.settings.use_noise, &*tr!("egui-noise"));
                        if self.settings.use_noise {
                            appearance::slider_row(ui, &tr!("egui-noise-level"), &mut self.settings.noise_level, 0.0..=100.0);
                            appearance::slider_row(ui, &tr!("egui-sync-wobble"), &mut self.settings.sync_wobble, 0.0..=100.0);
                        }
                    });
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    if ui.button(tr!("egui-reset-monitor")).clicked() {
                        self.settings = MonitorSettings::default();
                    }
                    close |= ui.button(tr!("egui-close")).clicked();
                });
            });
        if close {
            self.show_monitor = false;
        }
    }

    fn schedule_frame(&mut self, context: &egui::Context) {
        let now = Blink::now_ms();
        let screen = self.terminal.screen.lock();
        let buffer_type = screen.buffer_type();
        let caret = screen.caret();
        let mut delay = screen.terminal_state().synchronized_output_remaining();
        for (blink, enabled, rate) in [
            (
                &mut self.shader_state.caret_blink,
                caret.visible && caret.blinking && self.terminal.has_focus,
                buffer_type.caret_blink_rate(),
            ),
            (&mut self.shader_state.character_blink, screen.ice_mode().has_blink(), buffer_type.blink_rate()),
        ] {
            if enabled {
                blink.set_rate(rate as u128);
                blink.update(now);
                let remaining = Duration::from_millis(blink.time_until_next(now) as u64 + 1);
                delay = Some(delay.map_or(remaining, |current| current.min(remaining)));
            }
        }
        if self.settings.use_noise {
            delay = Some(delay.map_or(Duration::from_millis(33), |current| current.min(Duration::from_millis(33))));
        }
        if let Some(delay) = delay {
            context.request_repaint_after(delay);
        }
    }

    fn terminal_view(&mut self, ui: &mut egui::Ui) {
        let wheel_direction = if self.dialing_directory.options.invert_mouse_wheel { -1.0 } else { 1.0 };
        let wheel = ui.input(|input| input.smooth_scroll_delta.y) * wheel_direction;
        let local_wheel = !self.blocks_terminal() && ui.rect_contains_pointer(ui.max_rect()) && {
            let screen = self.terminal.screen.lock();
            let mouse = &screen.terminal_state().mouse_state;
            self.terminal.is_in_scrollback_mode()
                || !mouse.mouse_tracking_enabled
                || (mouse.mouse_mode == icy_engine::MouseMode::OFF && !mouse.alternate_scroll_enabled)
                || ui.input(|input| input.modifiers.shift)
        };
        if self.connected && local_wheel {
            if wheel > 0.0 && !self.terminal.is_in_scrollback_mode() {
                self.toggle_scrollback();
            } else if wheel < 0.0 && self.terminal.is_in_scrollback_mode() && self.terminal.scroll_y() >= self.terminal.max_scroll_y() - 1.0 {
                self.toggle_scrollback();
            }
        }
        let resolution = self.terminal.screen.lock().resolution();
        let available = ui.available_size();
        let zoom = self.settings.scaling_mode.compute_zoom(
            resolution.width as f32,
            resolution.height as f32,
            available.x,
            available.y,
            self.settings.use_integer_scaling,
        );
        let auto = self.settings.scaling_mode.is_auto();
        let remote_mouse = {
            let screen = self.terminal.screen.lock();
            let mouse = &screen.terminal_state().mouse_state;
            self.connected
                && !self.terminal.is_in_scrollback_mode()
                && mouse.mouse_tracking_enabled
                && (mouse.mouse_mode != icy_engine::MouseMode::OFF || mouse.alternate_scroll_enabled)
                && !ui.input(|input| input.modifiers.shift)
                && !self.blocks_terminal()
                && self.terminal.scroll_x() == 0.0
                && self.terminal.scroll_y() == 0.0
        };
        let content_size = if auto {
            available
        } else {
            egui::vec2(self.terminal.content_width() * zoom, self.terminal.content_height() * zoom).max(available)
        };

        let mut scroll_area = egui::ScrollArea::both();
        if let Some(offset) = self.navigation.scroll_to.take() {
            scroll_area = scroll_area.vertical_scroll_offset(offset.min((content_size.y - available.y).max(0.0) / zoom) * zoom);
        }
        if let Some(offset) = self.navigation.scroll_x.take() {
            scroll_area = scroll_area.horizontal_scroll_offset(offset * zoom);
        }
        scroll_area
            .id_salt(self.shader_state.instance_id)
            .wheel_scroll_multiplier(egui::vec2(wheel_direction, wheel_direction))
            .scroll_source(egui::scroll_area::ScrollSource {
                mouse_wheel: !remote_mouse,
                drag: false,
                ..Default::default()
            })
            .auto_shrink([false, false])
            .scroll_bar_visibility(if auto {
                egui::scroll_area::ScrollBarVisibility::AlwaysHidden
            } else {
                egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded
            })
            .show_viewport(ui, |ui, viewport| {
                let origin = ui.cursor().min;
                ui.allocate_space(content_size);
                let bounds = egui::Rect::from_min_size(origin + viewport.min.to_vec2(), viewport.size());
                let response = ui.interact(bounds, ui.id().with("terminal"), egui::Sense::click_and_drag());
                self.terminal_input_id = Some(response.id);
                if (response.clicked() || self.focus_terminal) && !self.dialing_directory.open {
                    response.request_focus();
                    self.focus_terminal = false;
                }
                ui.memory_mut(|memory| {
                    memory.set_focus_lock_filter(
                        response.id,
                        egui::EventFilter {
                            tab: true,
                            horizontal_arrows: true,
                            vertical_arrows: true,
                            escape: true,
                        },
                    )
                });
                self.terminal.has_focus = response.has_focus() && ui.input(|input| input.focused) && !self.show_monitor && !self.dialing_directory.open;
                if !self.blocks_terminal() && !ui.ctx().will_discard() {
                    let action = response.hover_pos().and_then(|position| {
                        let info = self.terminal.render_info.read();
                        let (column, row) = info.screen_to_cell(position.x, position.y)?;
                        let position = icy_engine::Position::new(
                            column + (self.terminal.scroll_x() / info.font_width.max(1.0)) as i32,
                            row + (self.terminal.scroll_y() / info.font_height.max(1.0)) as i32,
                        );
                        navigation::click_action(&**self.terminal.screen.lock(), position)
                    });
                    if action.is_some() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if response.clicked_by(egui::PointerButton::Primary) && !ui.input(|input| input.modifiers.shift) {
                        match action {
                            Some(navigation::ClickAction::Link(url)) => self.pending_link = Some(url),
                            Some(navigation::ClickAction::Command(clear, command)) if self.connected && !self.terminal.is_in_scrollback_mode() => {
                                if clear {
                                    if let Some(screen) = self.terminal.screen.lock().as_editable() {
                                        screen.clear_screen();
                                        screen.reset_terminal();
                                    }
                                }
                                let buffer = self.terminal.screen.lock().buffer_type();
                                let bytes = input::encode_terminal_events(&[egui::Event::Text(command)], buffer, false, self.terminal_emulation);
                                if let Some(session) = &self.session {
                                    if let Err(error) = session.send(bytes) {
                                        self.error = Some(error);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    if !remote_mouse {
                        self.navigation.interact(&self.terminal, &response, ui.input(|input| input.modifiers.alt));
                    }
                }
                self.terminal
                    .update_scroll_viewport([viewport.min.x, viewport.min.y, viewport.width(), viewport.height()], zoom);
                let mut settings = self.settings.clone();
                if auto && (resolution.width as f32 > bounds.width() || resolution.height as f32 > bounds.height()) {
                    settings.use_integer_scaling = false;
                }
                let frame = CRTShaderProgram::new(&self.terminal, Arc::new(settings), None).frame(
                    &self.shader_state,
                    [bounds.width(), bounds.height()],
                    ui.ctx().pixels_per_point(),
                );
                ui.painter().add(egui_wgpu::Callback::new_paint_callback(
                    bounds,
                    TerminalCallback {
                        frame,
                        bounds,
                        context: ui.ctx().clone(),
                    },
                ));
                if self.terminal.is_in_scrollback_mode() {
                    let font_height = self.terminal.screen.lock().font_dimensions().height.max(1) as f32;
                    let lines = ((self.terminal.max_scroll_y() - self.terminal.scroll_y()) / font_height) as i32;
                    let text = format!("\u{2191} {lines:04}");
                    let galley = ui
                        .painter()
                        .layout_no_wrap(text, egui::FontId::monospace(12.0), ui.visuals().selection.stroke.color);
                    let padding = egui::vec2(8.0, 3.0);
                    let rect = egui::Rect::from_min_size(
                        egui::pos2(bounds.max.x - galley.size().x - 2.0 * padding.x - 12.0, bounds.min.y + 8.0),
                        galley.size() + 2.0 * padding,
                    );
                    ui.painter().rect_filled(rect, 4.0, egui::Color32::from_black_alpha(180));
                    ui.painter()
                        .rect_stroke(rect, 4.0, egui::Stroke::new(1.0, ui.visuals().window_stroke.color), egui::StrokeKind::Inside);
                    ui.painter().galley(rect.min + padding, galley, egui::Color32::WHITE);
                }
                response.context_menu(|ui| self.terminal_menu(ui));
            });
    }

    fn show(&mut self, context: &egui::Context) {
        let blocked_at_start = self.blocks_terminal();
        if !self.styled {
            self.styled = true;
            appearance::apply(context);
        }
        self.receive_mcp(context);
        self.receive_events(context);
        if let Some(error) = self.error.take() {
            self.messages.error(tr!("egui-message-error"), error);
        }
        if let Some(receiver) = &self.version_check {
            if let Ok(latest) = receiver.try_recv() {
                self.latest_version = Some(latest);
                self.version_check = None;
            }
        }
        self.shortcuts(context);
        if let Some(sound) = &mut self.sound {
            if let Err(error) = sound.update_state() {
                self.error = Some(error.to_string());
            }
            if sound.is_playing() {
                context.request_repaint_after(Duration::from_millis(100));
            }
        }
        if self.terminal.has_focus && !self.blocks_terminal() {
            if context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::F11)) {
                let fullscreen = context.input(|input| input.viewport().fullscreen.unwrap_or(false));
                context.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fullscreen));
            }
            if context.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::F)) {
                self.navigation.find_open = true;
            }
            if context.input_mut(|input| input.consume_key(egui::Modifiers::SHIFT, egui::Key::PageUp)) {
                if !self.terminal.is_in_scrollback_mode() {
                    self.toggle_scrollback();
                } else {
                    self.navigation.scroll_to = Some((self.terminal.scroll_y() - self.terminal.visible_content_height()).max(0.0));
                }
            }
            if self.terminal.is_in_scrollback_mode() && context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
                self.toggle_scrollback();
            }
            if self.terminal.is_in_scrollback_mode() && context.input_mut(|input| input.consume_key(egui::Modifiers::SHIFT, egui::Key::PageDown)) {
                if self.terminal.scroll_y() >= self.terminal.max_scroll_y() - 1.0 {
                    self.toggle_scrollback();
                } else {
                    self.navigation.scroll_to = Some(self.terminal.scroll_y() + self.terminal.visible_content_height());
                }
            }
            if self.terminal.is_in_scrollback_mode() {
                self.scrollback_keys(context);
            }
        }
        let directory_was_open = self.dialing_directory.open;
        if let Some(path) = context.input(|input| input.raw.dropped_files.iter().find_map(|file| file.path.clone())) {
            self.load(path);
        }
        if !directory_was_open {
            egui::TopBottomPanel::top("toolbar")
                .frame(
                    egui::Frame::NONE
                        .fill(context.style().visuals.panel_fill)
                        .inner_margin(egui::Margin::symmetric(10, 5)),
                )
                .show(context, |ui| self.toolbar(ui));
            egui::TopBottomPanel::bottom("status")
                .frame(
                    egui::Frame::NONE
                        .fill(context.style().visuals.panel_fill)
                        .inner_margin(egui::Margin::symmetric(12, 5)),
                )
                .show(context, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().interact_size.y = 20.0;
                        let wide = ui.available_width() > 640.0;
                        let status = if self.connected {
                            tr!("egui-connected")
                        } else if self.connecting {
                            tr!("egui-connecting")
                        } else {
                            tr!("egui-offline")
                        };
                        let color = if self.connected {
                            egui::Color32::from_rgb(78, 173, 118)
                        } else if self.connecting {
                            ui.visuals().warn_fg_color
                        } else {
                            ui.visuals().weak_text_color()
                        };
                        let (indicator, _) = ui.allocate_exact_size(egui::vec2(8.0, 20.0), egui::Sense::hover());
                        ui.painter().circle_filled(indicator.center(), 3.0, color);
                        ui.label(egui::RichText::new(status).small().color(color));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if let Some(started) = self.connected_at {
                                let seconds = started.elapsed().as_secs();
                                ui.label(
                                    egui::RichText::new(format!("{:02}:{:02}:{:02}", seconds / 3600, seconds / 60 % 60, seconds % 60))
                                        .monospace()
                                        .small(),
                                );
                                context.request_repaint_after(Duration::from_secs(1));
                            }
                            let info = {
                                let screen = self.terminal.screen.lock();
                                format!(
                                    "{} • {}x{}",
                                    icy_term::fmt_terminal_emulation(&self.terminal_emulation),
                                    screen.width(),
                                    screen.height()
                                )
                            };
                            if wide
                                && ui
                                    .add(egui::Button::new(egui::RichText::new(info).small()).frame(false))
                                    .on_hover_text(tr!("terminal-menu-info"))
                                    .clicked()
                            {
                                self.terminal_info();
                            }
                            if self.tools.host_info.is_some() && ui.add(egui::Button::new(egui::RichText::new("IEMSI").small())).clicked() {
                                self.tools.info_open = true;
                            }
                            let baud = if self.connected {
                                match self.baud {
                                    icy_parser_core::BaudEmulation::Off => tr!("select-bps-dialog-bps-max"),
                                    icy_parser_core::BaudEmulation::Rate(rate) => tr!("select-bps-dialog-bps", bps = rate),
                                }
                            } else {
                                "LOCAL".into()
                            };
                            if ui
                                .add_enabled(self.connected, egui::Button::new(egui::RichText::new(baud).small()))
                                .on_hover_text(tr!("select-bps-dialog-heading"))
                                .clicked()
                            {
                                self.bps_open = true;
                            }
                            if self.transfers.capture.is_some()
                                && ui
                                    .add(egui::Button::new(
                                        egui::RichText::new(tr!("toolbar-stop-capture")).small().color(ui.visuals().error_fg_color),
                                    ))
                                    .clicked()
                            {
                                self.command(icy_term::TerminalCommand::StopCapture, context);
                            }
                            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                ui.add(egui::Label::new(egui::RichText::new(&self.document_name).small()).truncate())
                                    .on_hover_text(&self.document_name);
                            });
                        });
                    });
                    if self.transfers.capture.is_some() || self.tools.script_running || self.tools.pause.is_some() {
                        ui.horizontal_wrapped(|ui| {
                            if self.transfers.capture.is_some() {
                                ui.colored_label(ui.visuals().warn_fg_color, &*tr!("egui-recording"));
                            }
                            if self.tools.script_running {
                                ui.separator();
                                ui.label(&*tr!("egui-script-running"));
                            }
                            if self.tools.pause.is_some() {
                                ui.separator();
                                ui.label(&*tr!("egui-host-pause"));
                            }
                        });
                    }
                });
        }
        self.schedule_frame(context);
        if self.navigation.find_open && !directory_was_open {
            let search_blocked = self.messages.is_open()
                || self.error.is_some()
                || self.confirm_close
                || self.pending_link.is_some()
                || self.preferences.is_some()
                || self.tools.blocks_input()
                || self.transfers.open;
            let terminal_bounds = context.available_rect();
            egui::Area::new(egui::Id::new("find-overlay"))
                .order(egui::Order::Foreground)
                .pivot(egui::Align2::RIGHT_TOP)
                .fixed_pos(terminal_bounds.right_top() + egui::vec2(-8.0, 8.0))
                .constrain_to(terminal_bounds)
                .movable(false)
                .show(context, |ui| {
                    egui::Frame::popup(ui.style()).corner_radius(4).inner_margin(12).show(ui, |ui| {
                        if search_blocked {
                            ui.disable();
                        }
                        ui.set_width((terminal_bounds.width() - 40.0).clamp(220.0, 360.0));
                        ui.spacing_mut().item_spacing = egui::vec2(4.0, 6.0);
                        ui.spacing_mut().button_padding = egui::vec2(6.0, 4.0);
                        ui.spacing_mut().interact_size = egui::vec2(28.0, 28.0);
                        ui.horizontal_wrapped(|ui| {
                            let response = ui.add(
                                egui::TextEdit::singleline(&mut self.navigation.query)
                                    .desired_width((ui.available_width() - 108.0).max(100.0))
                                    .hint_text(&*tr!("egui-find")),
                            );
                            if !search_blocked && !self.navigation.find_initialized {
                                response.request_focus();
                                self.navigation.find_initialized = true;
                                self.focus_terminal = false;
                            }
                            if response.changed() {
                                self.navigation.refresh_search(&self.terminal);
                            }
                            let enter = !search_blocked
                                && (response.has_focus() || response.lost_focus())
                                && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
                            if enter {
                                response.request_focus();
                            }
                            let backwards = ui.input(|input| input.modifiers.shift);
                            if ui
                                .add_enabled(!self.navigation.query.is_empty(), egui::Button::new("\u{2191}"))
                                .on_hover_text(tr!("egui-previous"))
                                .clicked()
                                || (enter && backwards)
                            {
                                self.navigation.find(&self.terminal, true);
                            }
                            if ui
                                .add_enabled(!self.navigation.query.is_empty(), egui::Button::new("\u{2193}"))
                                .on_hover_text(tr!("egui-next"))
                                .clicked()
                                || (enter && !backwards)
                            {
                                self.navigation.find(&self.terminal, false);
                            }
                            if ui.button("\u{00d7}").on_hover_text(tr!("egui-close")).clicked()
                                || (!search_blocked && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)))
                            {
                                self.navigation.find_open = false;
                                self.navigation.find_initialized = false;
                                self.focus_terminal = true;
                            }
                        });
                        ui.horizontal(|ui| {
                            if ui
                                .toggle_value(&mut self.navigation.case_sensitive, "Aa")
                                .on_hover_text(tr!("egui-match-case"))
                                .changed()
                            {
                                self.navigation.refresh_search(&self.terminal);
                            }
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if self.navigation.search_failed {
                                    ui.colored_label(ui.visuals().warn_fg_color, tr!("egui-not-found"));
                                } else if let Some((current, total)) = self.navigation.search_result {
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(tr!("terminal-find-results", cur = current.to_string(), total = total.to_string())).weak(),
                                        )
                                        .truncate(),
                                    );
                                }
                            });
                        });
                    });
                });
        }
        if !self.dialing_directory.open {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(context, |ui| self.terminal_view(ui));
            self.monitor(context);
        }
        self.dialing_directory.message_blocked = self.messages.is_open() || self.error.is_some() || self.confirm_close || self.pending_link.is_some();
        if let Some(request) = self.dialing_directory.show(context, self.connected || self.connecting) {
            match request {
                dialing_directory::DialRequest::Quick(entry, options) | dialing_directory::DialRequest::Entry(entry, options) => {
                    match session::entry_connection_config(&entry, &options) {
                        Ok(config) => {
                            self.address = phonebook::display_address(&entry);
                            self.utf8 = entry.terminal_type == icy_net::telnet::TerminalEmulation::Utf8Ansi;
                            self.active_profile = Some(entry.clone());
                            self.baud = entry.baud_emulation;
                            self.start_connection(config, entry.system_name, context);
                        }
                        Err(error) => self.error = Some(error),
                    }
                }
            }
        }
        if directory_was_open && !self.dialing_directory.open {
            self.focus_terminal = true;
        }
        if !directory_was_open {
            for command in self.transfers.show(context, &self.dialing_directory.options, self.connected) {
                if let Some(session) = &self.session {
                    if let Err(error) = session.command(command) {
                        self.error = Some(error);
                    }
                }
            }
            for command in self.tools.show(context) {
                self.command(command, context);
            }
        }
        if let Some(preferences) = &mut self.preferences {
            if let Some(options) = preferences.show(context) {
                self.settings = options.monitor_settings.clone();
                context.set_theme(match options.is_dark_mode {
                    None => egui::ThemePreference::System,
                    Some(true) => egui::ThemePreference::Dark,
                    Some(false) => egui::ThemePreference::Light,
                });
                if let Some(sound) = &mut self.sound {
                    if let Err(error) = sound.configure(options.audio_enabled, options.master_volume, options.audio_device.clone()) {
                        self.error = Some(error.to_string());
                    }
                }
                self.dialing_directory.options = options;
                self.terminal
                    .original_screen
                    .as_ref()
                    .unwrap_or(&self.terminal.screen)
                    .lock()
                    .set_scrollback_buffer_size(self.dialing_directory.options.max_scrollback_lines);
            }
            if preferences.closed {
                self.preferences = None;
                self.focus_terminal = true;
            }
        }
        self.windows(context);
        if self.help_open {
            overlays::help(context, &mut self.help_open);
        }
        if self.about_open {
            if let Some(url) = overlays::about(context, &mut self.about_open) {
                self.pending_link = Some(url);
            }
        }
        if self.bps_open {
            if let Some(baud) = overlays::baud(context, &mut self.bps_open, self.baud) {
                self.baud = baud;
                self.command(icy_term::TerminalCommand::SetBaudEmulation(baud), context);
            }
        }
        if !self.confirm_close && !self.messages.is_open() {
            if let Some(url) = self.pending_link.clone() {
                let allowed = url::Url::parse(&url).is_ok_and(|url| matches!(url.scheme(), "https" | "http" | "mailto"));
                if let Some(response) = messages::MessageBox::question(
                    "external-link",
                    &tr!("egui-open-link-question"),
                    &tr!("egui-message-link"),
                    &tr!("settings-paths-open"),
                )
                .details(&url)
                .enabled(allowed)
                .show(context)
                {
                    if response == messages::Response::Accept && allowed {
                        if let Err(error) = webbrowser::open(&url) {
                            self.error = Some(error.to_string());
                        }
                    }
                    self.pending_link = None;
                }
            }
        }
        if let Some(error) = self.error.take() {
            self.messages.error(tr!("egui-message-error"), error);
        }
        let message_was_open = self.messages.is_open();
        self.messages.show(context);
        self.send_input(context, blocked_at_start || message_was_open);
    }

    fn send_input(&mut self, context: &egui::Context, blocked_at_start: bool) {
        let focused = self.terminal_input_id.is_some_and(|id| context.memory(|memory| memory.has_focus(id)));
        let remote_focus =
            self.connected && focused && context.input(|input| input.focused) && !self.blocks_terminal() && !self.terminal.is_in_scrollback_mode();
        if self.remote_focus != remote_focus && !context.will_discard() {
            self.remote_focus = remote_focus;
            let screen = self.terminal.original_screen.as_ref().unwrap_or(&self.terminal.screen).lock();
            if self.connected && screen.terminal_state().mouse_state.focus_out_event_enabled {
                if let Some(session) = &self.session {
                    let _ = session.send(if remote_focus { b"\x1b[I".to_vec() } else { b"\x1b[O".to_vec() });
                }
            }
        }
        if blocked_at_start || !focused || self.blocks_terminal() || !context.input(|input| input.focused) || context.will_discard() {
            return;
        }
        if self.terminal.screen.lock().selection().is_some() {
            let copy = context.input(|input| input.events.iter().any(|event| matches!(event, egui::Event::Copy | egui::Event::Cut)));
            if copy {
                self.copy_selection(context);
                context.input_mut(|input| {
                    input.events.retain(|event| {
                        !matches!(
                            event,
                            egui::Event::Copy
                                | egui::Event::Cut
                                | egui::Event::Key {
                                    key: egui::Key::C | egui::Key::X,
                                    modifiers: egui::Modifiers { command: true, .. },
                                    ..
                                }
                        )
                    })
                });
            }
        }
        if self.connecting || self.terminal.is_in_scrollback_mode() {
            return;
        }
        let bytes = {
            let screen = self.terminal.screen.lock();
            context.input(|input| {
                let mut bytes = input::encode_protocol_events(
                    &input.events,
                    screen.buffer_type(),
                    self.connected && screen.terminal_state().bracketed_paste_mode,
                    self.terminal_emulation,
                    if self.connected { screen.terminal_state().kitty_keyboard.flags() } else { 0 },
                );
                if self.connected && self.terminal.scroll_x() == 0.0 && self.terminal.scroll_y() == 0.0 {
                    bytes.extend(input::encode_mouse_events(
                        input,
                        &screen.terminal_state().mouse_state,
                        &self.terminal.render_info.read(),
                    ));
                }
                bytes
            })
        };
        if !bytes.is_empty() {
            if self.session.is_none() {
                self.session = Some(session::Session::idle(self.terminal.screen.clone(), context.clone()));
            }
            if let Some(session) = &self.session {
                if let Err(error) = session.send(bytes) {
                    self.error = Some(error);
                }
            }
        }
    }

    /// Right-click actions, matching the legacy terminal context menu.
    /// Checked in the background so a slow network cannot stall the first frame.
    fn check_for_updates(&mut self, context: &egui::Context) {
        let (sender, receiver) = std::sync::mpsc::channel();
        self.version_check = Some(receiver);
        let context = context.clone();
        std::thread::spawn(move || {
            let current = semver::Version::parse(env!("CARGO_PKG_VERSION")).expect("package version");
            if let Some(latest) = icy_engine_gui::release_check::latest_release("mkrueger/icy_tools", "IcyTerm") {
                if latest > current {
                    let _ = sender.send(latest);
                    context.request_repaint();
                }
            }
        });
    }

    fn terminal_menu(&mut self, ui: &mut egui::Ui) {
        let context = ui.ctx().clone();
        if self.connected || self.connecting {
            if ui
                .add(egui::Button::new(&*tr!("terminal-hangup")).shortcut_text(hotkeys::shortcut(hotkeys::Action::Hangup)))
                .clicked()
            {
                self.disconnect();
                ui.close();
            }
        } else if ui
            .add(egui::Button::new(&*tr!("terminal-dialing_directory")).shortcut_text(hotkeys::shortcut(hotkeys::Action::DialingDirectory)))
            .clicked()
        {
            self.dialing_directory.open();
            ui.close();
        }
        ui.separator();
        ui.add_enabled_ui(self.connected && !self.transfers.active, |ui| {
            if ui
                .add(egui::Button::new(&*tr!("terminal-upload")).shortcut_text(hotkeys::shortcut(hotkeys::Action::Upload)))
                .clicked()
            {
                self.transfers.choose(false);
                ui.close();
            }
            if ui
                .add(egui::Button::new(&*tr!("terminal-download")).shortcut_text(hotkeys::shortcut(hotkeys::Action::Download)))
                .clicked()
            {
                self.transfers.choose(true);
                ui.close();
            }
        });
        ui.separator();
        let has_selection = self.terminal.screen.lock().selection().is_some();
        if ui
            .add_enabled(
                has_selection,
                egui::Button::new(&*tr!("terminal-menu-copy")).shortcut_text(hotkeys::shortcut(hotkeys::Action::Copy)),
            )
            .clicked()
        {
            self.copy_selection(&context);
            ui.close();
        }
        if ui
            .add(egui::Button::new(&*tr!("terminal-menu-paste")).shortcut_text(hotkeys::shortcut(hotkeys::Action::Paste)))
            .clicked()
        {
            context.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
            self.focus_terminal = true;
            ui.close();
        }
        ui.separator();
        if ui.button(&*tr!("terminal-menu-info")).clicked() {
            self.terminal_info();
            ui.close();
        }
        if ui
            .add(egui::Button::new(tr!("settings-heading")).shortcut_text(hotkeys::shortcut(hotkeys::Action::Settings)))
            .clicked()
        {
            self.open_settings();
            ui.close();
        }
    }

    fn terminal_info(&mut self) {
        let profile = self.terminal_profile();
        let screen = self.terminal.original_screen.as_ref().unwrap_or(&self.terminal.screen).lock();
        self.tools.terminal_info = Some(terminal_info::Dialog::new(&**screen, profile, self.terminal_emulation, self.baud));
        self.tools.scrollback = self.dialing_directory.options.max_scrollback_lines;
    }

    fn terminal_profile(&self) -> icy_term::Address {
        let screen = self.terminal.original_screen.as_ref().unwrap_or(&self.terminal.screen).lock();
        let mut profile = self.active_profile.clone().unwrap_or_default();
        profile.terminal_type = self.terminal_emulation;
        profile.screen_mode = match self.screen_mode {
            icy_engine::ScreenMode::Vga(_, _) => icy_engine::ScreenMode::Vga(screen.width(), screen.height()),
            icy_engine::ScreenMode::Unicode(_, _) => icy_engine::ScreenMode::Unicode(screen.width(), screen.height()),
            mode => mode,
        };
        profile.screen_mode = icy_term::normalize_screen_mode(profile.terminal_type, profile.screen_mode);
        profile.ansi_music = self.ansi_music;
        profile.baud_emulation = self.baud;
        profile.ice_mode = screen.ice_mode() == icy_engine::IceMode::Ice;
        profile.set_lf_expand(screen.terminal_state().lf_expand);
        profile.mouse_reporting_enabled = screen.terminal_state().mouse_state.mouse_tracking_enabled;
        profile
    }

    fn blocks_terminal(&self) -> bool {
        self.navigation.find_open
            || self.messages.is_open()
            || self.error.is_some()
            || self.show_monitor
            || self.dialing_directory.open
            || self.transfers.open
            || self.preferences.is_some()
            || self.tools.blocks_input()
            || self.confirm_close
            || self.pending_link.is_some()
            || self.about_open
            || self.help_open
            || self.bps_open
    }

    fn shortcuts(&mut self, context: &egui::Context) {
        if self.blocks_terminal() {
            return;
        }
        let terminal_has_keyboard = self.connected && self.terminal.has_focus && !self.terminal.is_in_scrollback_mode();
        for action in hotkeys::poll(context, terminal_has_keyboard) {
            self.shortcut(action, context);
        }
    }

    fn shortcut(&mut self, action: hotkeys::Action, context: &egui::Context) {
        use hotkeys::Action;
        match action {
            Action::DialingDirectory => self.dialing_directory.open(),
            Action::Hangup => {
                if self.connected || self.connecting {
                    self.disconnect();
                }
            }
            Action::Serial => {
                if !self.connected && !self.connecting {
                    self.tools.serial = self.dialing_directory.options.serial.clone();
                    self.tools.serial_open = true;
                }
            }
            Action::Upload | Action::Download => {
                if self.connected && !self.transfers.active {
                    self.transfers.choose(action == Action::Download);
                }
            }
            Action::SendLogin | Action::SendUser | Action::SendPassword => {
                let user = matches!(action, Action::SendLogin | Action::SendUser);
                let password = matches!(action, Action::SendLogin | Action::SendPassword);
                self.send_login(user, password);
            }
            Action::ClearScreen => {
                if self.terminal.is_in_scrollback_mode() {
                    self.toggle_scrollback();
                }
                let mut screen = self.terminal.screen.lock();
                if let Some(editable) = screen.as_editable() {
                    editable.clear_scrollback();
                    editable.clear_screen();
                }
            }
            Action::Scrollback => self.toggle_scrollback(),
            Action::Find => self.navigation.find_open = true,
            Action::ToggleMouse => {
                let mut screen = self.terminal.screen.lock();
                let enabled = !screen.terminal_state().mouse_state.mouse_tracking_enabled;
                if let Some(editable) = screen.as_editable() {
                    editable.terminal_state_mut().mouse_state.mouse_tracking_enabled = enabled;
                }
            }
            Action::Capture => {
                if self.transfers.capture.is_some() {
                    self.command(icy_term::TerminalCommand::StopCapture, context);
                } else if let Some(path) = rfd::FileDialog::new()
                    .set_directory(&self.dialing_directory.options.capture_path)
                    .set_file_name("capture.ans")
                    .save_file()
                {
                    self.command(icy_term::TerminalCommand::StartCapture(path.to_string_lossy().into_owned()), context);
                }
            }
            Action::ExportScreen => self.save_screen(),
            Action::RunScript => {
                if self.tools.script_running {
                    self.command(icy_term::TerminalCommand::StopScript, context);
                } else if let Some(path) = rfd::FileDialog::new().add_filter("Lua", &["lua"]).pick_file() {
                    self.command(icy_term::TerminalCommand::RunScript(path), context);
                }
            }
            Action::Settings => self.open_settings(),
            Action::Quit => context.send_viewport_cmd(egui::ViewportCommand::Close),
            Action::About => self.about_open = true,
            Action::Help => self.help_open = true,
            Action::Fullscreen => {
                let fullscreen = context.input(|input| input.viewport().fullscreen.unwrap_or(false));
                context.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fullscreen));
            }
            Action::ZoomIn | Action::ZoomOut => {
                let zoom = match self.settings.scaling_mode {
                    ScalingMode::Manual(zoom) => zoom,
                    _ => self.terminal.get_zoom(),
                };
                let integer = self.settings.use_integer_scaling;
                self.settings.scaling_mode = ScalingMode::Manual(if action == Action::ZoomIn {
                    ScalingMode::zoom_in(zoom, integer)
                } else {
                    ScalingMode::zoom_out(zoom, integer)
                });
            }
            Action::ZoomReset => self.settings.scaling_mode = ScalingMode::Manual(1.0),
            Action::ZoomFit => self.settings.scaling_mode = ScalingMode::Auto,
            Action::NewWindow => self.new_window = true,
            Action::CloseWindow => context.send_viewport_cmd(egui::ViewportCommand::Close),
            Action::Copy => self.copy_selection(context),
            Action::Paste => {
                context.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                self.focus_terminal = true;
            }
        }
    }

    /// Legacy scrollback navigation: any other key leaves the history.
    fn scrollback_keys(&mut self, context: &egui::Context) {
        let line = self.terminal.screen.lock().font_dimensions().height as f32;
        let page = self.terminal.visible_content_height();
        let scroll = context.input_mut(|input| {
            for (key, delta) in [
                (egui::Key::ArrowUp, -line),
                (egui::Key::ArrowDown, line),
                (egui::Key::PageUp, -page),
                (egui::Key::PageDown, page),
            ] {
                if input.consume_key(egui::Modifiers::NONE, key) {
                    return Some(self.terminal.scroll_y() + delta);
                }
            }
            if input.consume_key(egui::Modifiers::NONE, egui::Key::Home) {
                return Some(0.0);
            }
            if input.consume_key(egui::Modifiers::NONE, egui::Key::End) {
                return Some(self.terminal.max_scroll_y());
            }
            None
        });
        if let Some(target) = scroll {
            self.navigation.scroll_to = Some(target.clamp(0.0, self.terminal.max_scroll_y()));
        }
    }

    fn send_login(&mut self, user: bool, password: bool) {
        let Some(address) = self.active_profile.clone() else {
            return;
        };
        if !self.connected {
            return;
        }
        let enter = icy_term::scripting::parse_key_string(self.terminal_emulation, "enter").unwrap_or_else(|| vec![b'\r']);
        let mut data = Vec::new();
        if user && !address.user_name.is_empty() {
            data.extend_from_slice(address.user_name.as_bytes());
            data.extend_from_slice(&enter);
        }
        if password && !address.password.is_empty() {
            data.extend_from_slice(address.password.as_bytes());
            data.extend_from_slice(&enter);
        }
        if data.is_empty() {
            return;
        }
        if let Some(session) = &self.session {
            if let Err(error) = session.send(data) {
                self.error = Some(error);
            }
        }
    }

    fn open_settings(&mut self) {
        if let Some(path) = icy_term::Options::options_file() {
            let mut options = self.dialing_directory.options.clone();
            options.monitor_settings = self.settings.clone();
            match settings::Settings::open(&options, path) {
                Ok(settings) => self.preferences = Some(settings),
                Err(error) => self.error = Some(error),
            }
        }
    }

    fn save_screen(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("ANSI", &["ans"])
            .add_filter("Text", &["asc"])
            .set_file_name("screen.ans")
            .save_file()
        else {
            return;
        };
        let extension = path.extension().and_then(|extension| extension.to_str()).unwrap_or("ans");
        let options = icy_engine::SaveOptions::ansi(icy_engine::AnsiCompatibilityLevel::Utf8Terminal);
        let data = self.terminal.screen.lock().to_bytes(extension, &options);
        match data {
            Ok(data) => {
                if let Err(error) = std::fs::write(path, data) {
                    self.error = Some(error.to_string());
                }
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn command(&mut self, command: icy_term::TerminalCommand, context: &egui::Context) {
        if matches!(command, icy_term::TerminalCommand::SetTerminalProfile { .. }) && self.terminal.is_in_scrollback_mode() {
            self.toggle_scrollback();
        }
        if matches!(
            command,
            icy_term::TerminalCommand::OpenSerial(_) | icy_term::TerminalCommand::AutoDetectSerial(_)
        ) {
            if self.connected || self.connecting {
                self.error = Some("Disconnect before opening a serial port".into());
                return;
            }
            self.connecting = true;
            self.active_profile = None;
            self.document_name = self.tools.serial.device.clone();
        }
        if self.session.is_none() {
            let screen = self.terminal.original_screen.as_ref().unwrap_or(&self.terminal.screen).clone();
            self.session = Some(session::Session::idle(screen, context.clone()));
        }
        if let Some(session) = &self.session {
            if let Some(book) = &self.dialing_directory.phonebook {
                *session.address_book.lock() = book.book.clone();
            }
            if matches!(command, icy_term::TerminalCommand::RunScript(_) | icy_term::TerminalCommand::RunScriptCode(_)) {
                self.tools.script_running = true;
            }
            if let Err(error) = session.command(command) {
                self.finish_script_response(Err(error.clone()));
                self.tools.script_running = false;
                self.error = Some(error);
            }
        }
        self.focus_terminal = true;
    }

    fn windows(&mut self, context: &egui::Context) {
        if self.new_window && !context.will_discard() {
            self.new_window = false;
            let screen = icy_term::welcome_screen::create_welcome_screen(None);
            let mut child = Self::new(screen, "Icy Term".into());
            child.settings = self.settings.clone();
            child.dialing_directory.options = self.dialing_directory.options.clone();
            child.cache_root = self.cache_root.clone();
            if self.sound.is_some() {
                let mut sound = icy_engine_gui::music::music::SoundThread::new();
                let options = &child.dialing_directory.options;
                let _ = sound.configure(options.audio_enabled, options.master_volume, options.audio_device.clone());
                child.sound = Some(sound);
            }
            let id = egui::ViewportId::from_hash_of(("terminal-window", child.shader_state.instance_id));
            self.children.push((id, Arc::new(Mutex::new(child))));
        }
        self.children.retain(|(_, child)| !child.lock().closed);
        for (id, child) in &self.children {
            let child = child.clone();
            let parent = context.viewport_id();
            context.show_viewport_deferred(
                *id,
                egui::ViewportBuilder::default()
                    .with_title("Icy Term")
                    .with_inner_size([1000.0, 720.0])
                    .with_min_inner_size([360.0, 240.0]),
                move |context, _class| {
                    let mut child = child.lock();
                    child.show(context);
                    if child.closed {
                        context.request_repaint_of(parent);
                    }
                },
            );
        }
        if context.input(|input| input.viewport().close_requested()) && !self.closed {
            if self.connected
                || self.connecting
                || self.tools.script_running
                || self.preferences.is_some()
                || !self.children.is_empty()
                || self.dialing_directory.phonebook.as_ref().is_some_and(|book| book.draft.is_some())
            {
                context.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.confirm_close = true;
            } else {
                self.shutdown_window();
            }
        }
        if self.confirm_close && !self.messages.is_open() {
            if let Some(response) = messages::MessageBox::question(
                "close-window",
                &tr!("egui-close-question"),
                &tr!("egui-close-warning"),
                &tr!("egui-close-window"),
            )
            .destructive()
            .show(context)
            {
                if response == messages::Response::Accept {
                    self.shutdown_window();
                    context.send_viewport_cmd(egui::ViewportCommand::Close);
                } else {
                    self.confirm_close = false;
                }
            }
        }
    }

    fn shutdown_window(&mut self) {
        self.closed = true;
        self.confirm_close = false;
        self.disconnect();
        self.mcp = None;
        for (_, child) in &self.children {
            child.lock().shutdown_window();
        }
    }
}

impl eframe::App for TerminalApp {
    fn update(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        self.show(context);
    }
}

impl Drop for TerminalApp {
    fn drop(&mut self) {
        self.finish_script_response(Err("Window closed".into()));
        self.finish_call();
    }
}

fn load_screen(path: &std::path::Path) -> anyhow::Result<TextScreen> {
    let format = FileFormat::from_path(path).ok_or_else(|| anyhow::anyhow!("Unsupported file extension"))?;
    Ok(format.load(path, None)?.screen)
}

fn main() -> anyhow::Result<()> {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    let args = Args::parse();
    let mut target = args.connect;
    let file = args.file.filter(|path| {
        if !path.exists() && FileFormat::from_path(path).is_none() {
            target = Some(path.to_string_lossy().into_owned());
            false
        } else {
            true
        }
    });
    if let Some(path) = args.config {
        icy_term::Options::set_options_file(path);
    }
    if let Some(path) = args.phonebook {
        icy_term::Address::set_dialing_directory_file(path);
    }
    let (screen, name) = if let Some(path) = file {
        (load_screen(&path)?, path.display().to_string())
    } else {
        (icy_term::welcome_screen::create_welcome_screen(None), "Icy Term".to_string())
    };
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../../build/linux/128x128.png"))?;
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: egui_wgpu::WgpuConfiguration {
            // Waiting for vsync halves the frame rate while the compositor resizes the window.
            present_mode: egui_wgpu::wgpu::PresentMode::AutoNoVsync,
            ..Default::default()
        },
        multisampling: 0,
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 720.0])
            .with_min_inner_size([360.0, 240.0])
            .with_icon(icon),
        ..Default::default()
    };
    eframe::run_native(
        "Icy Term",
        options,
        Box::new(move |creation| {
            let render_state = creation.wgpu_render_state.as_ref().ok_or("wgpu renderer unavailable")?;
            render_state
                .renderer
                .write()
                .callback_resources
                .insert(TerminalShaderRenderer::new(&render_state.device, render_state.target_format));
            let mut app = TerminalApp::new(screen, name);
            app.cache_root = directories::ProjectDirs::from("com", "GitHub", "icy_term").map(|directories| directories.config_dir().join("cache"));
            match icy_term::Options::load_options() {
                Ok(options) => {
                    app.settings = options.monitor_settings.clone();
                    app.dialing_directory.options = options;
                }
                Err(error) => app.error = Some(format!("Settings: {error}")),
            }
            let mut sound = icy_engine_gui::music::music::SoundThread::new();
            let options = &app.dialing_directory.options;
            if let Err(error) = sound.configure(options.audio_enabled, options.master_volume, options.audio_device.clone()) {
                app.error = Some(error.to_string());
            }
            app.sound = Some(sound);
            app.check_for_updates(&creation.egui_ctx);
            creation.egui_ctx.set_theme(match app.dialing_directory.options.is_dark_mode {
                None => egui::ThemePreference::System,
                Some(true) => egui::ThemePreference::Dark,
                Some(false) => egui::ThemePreference::Light,
            });
            if let Some(port) = args.mcp_port {
                app.mcp = Some(mcp::Bridge::start(port, creation.egui_ctx.clone()));
            }
            app.utf8 = args.utf8;
            if let Some(address) = target {
                app.connect_target(&address, &creation.egui_ctx);
            }
            if let Some(path) = args.play {
                app.command(icy_term::TerminalCommand::PlayFile(path), &creation.egui_ctx);
            }
            if let Some(path) = args.run {
                app.command(icy_term::TerminalCommand::RunScript(path), &creation.egui_ctx);
            }
            Ok(Box::new(app))
        }),
    )
    .map_err(|error| anyhow::anyhow!("{error}"))
}

#[cfg(test)]
#[path = "icy_term_egui/gpu_tests.rs"]
mod gpu_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_input_reaches_the_modem_without_bypassing_focus() {
        let context = egui::Context::default();
        let (wake_tx, wake_rx) = std::sync::mpsc::channel();
        context.set_request_repaint_callback(move |_| {
            let _ = wake_tx.send(());
        });
        let screen = icy_term::welcome_screen::create_welcome_screen(None);
        let input_row = screen.caret.position().y;
        let mut app = TerminalApp::new(screen, "offline".into());
        let run = |app: &mut TerminalApp, events| {
            let _ = context.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 720.0))),
                    events,
                    focused: true,
                    ..Default::default()
                },
                |context| app.show(context),
            );
        };
        run(&mut app, vec![]);
        run(&mut app, vec![]);
        assert!(app.session.is_none(), "idle frames must not create workers");
        app.show_monitor = true;
        run(&mut app, vec![egui::Event::Text("dialog".into())]);
        app.show_monitor = false;
        context.memory_mut(|memory| memory.request_focus(egui::Id::new("other-widget")));
        run(&mut app, vec![egui::Event::Text("unfocused".into())]);
        assert!(app.session.is_none());
        app.focus_terminal = true;
        run(&mut app, vec![]);
        run(&mut app, vec![egui::Event::Text("AT".into())]);
        run(
            &mut app,
            vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        let wait_for_ok = |app: &mut TerminalApp, row: i32| {
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            loop {
                let screen = app.terminal.screen.lock();
                let first: String = (0..2).map(|column| screen.char_at((column, row).into()).ch).collect();
                let second: String = (0..2).map(|column| screen.char_at((column, row + 1).into()).ch).collect();
                if first == "AT" && second == "OK" {
                    break;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "offline modem returned {first:?} / {second:?}, expected AT / OK"
                );
                drop(screen);
                wake_rx
                    .recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
                    .expect("offline AT must echo and return OK");
                run(app, vec![]);
            }
        };
        wait_for_ok(&mut app, input_row);
        assert!(!app.connected && !app.connecting);
        let previous_events = std::mem::replace(&mut app.session.as_mut().unwrap().events, std::sync::mpsc::channel().1);
        app.disconnect();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            assert!(std::time::Instant::now() < deadline, "previous modem worker must finish disconnecting");
            if matches!(
                previous_events
                    .recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
                    .unwrap(),
                icy_term::TerminalEvent::Disconnected(_)
            ) {
                break;
            }
        }
        {
            let mut screen = app.terminal.screen.lock();
            let screen = screen.as_editable().unwrap();
            screen.clear_screen();
            screen.terminal_state_mut().bracketed_paste_mode = true;
        }
        run(&mut app, vec![egui::Event::Paste("AT".into())]);
        run(
            &mut app,
            vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(app.session.is_some(), "offline typing must work again after disconnect");
        wait_for_ok(&mut app, 0);
    }

    #[test]
    fn child_windows_have_independent_screens_and_close_their_workers() {
        let context = egui::Context::default();
        context.set_embed_viewports(false);
        let mut app = TerminalApp::new(TextScreen::default(), "parent".into());
        app.new_window = true;
        let output = context.run(egui::RawInput::default(), |context| app.windows(context));
        assert_eq!(app.children.len(), 1);
        let (id, child) = &app.children[0];
        assert!(output.viewport_output.contains_key(id));
        let mut child = child.lock();
        assert!(child.terminal.screen.lock().caret().position().y > 1);
        assert!(!Arc::ptr_eq(&app.terminal.screen, &child.terminal.screen));
        assert_ne!(app.shader_state.instance_id, child.shader_state.instance_id);
        child.command(icy_term::TerminalCommand::RunScriptCode("assert(true)".into()), &context);
        assert!(child.session.is_some());
        child.shutdown_window();
        assert!(child.closed && child.session.is_none());
        drop(child);
        let _ = context.run(egui::RawInput::default(), |context| app.windows(context));
        assert!(app.children.is_empty());
    }

    #[test]
    fn connected_app_sends_only_focused_terminal_input() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
            sync::mpsc,
            time::Instant,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = format!("raw://{}", listener.local_addr().unwrap());
        let expected = b"yes\r\t\x1b\x1b[D";
        let (received_tx, received_rx) = mpsc::channel();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            stream.write_all(b"\x1b[HREADY").unwrap();
            let mut received = vec![0; expected.len()];
            stream.read_exact(&mut received).unwrap();
            received_tx.send(()).unwrap();
            stream.read_to_end(&mut received).unwrap();
            received
        });
        let context = egui::Context::default();
        let (wake_tx, wake_rx) = mpsc::channel();
        context.set_request_repaint_callback(move |_| {
            let _ = wake_tx.send(());
        });
        let mut app = TerminalApp::new(TextScreen::default(), "test".into());
        app.address = address;
        app.connect(&context);
        let run = |app: &mut TerminalApp, events| {
            let _ = context.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 720.0))),
                    events,
                    focused: true,
                    ..Default::default()
                },
                |context| app.show(context),
            );
        };
        let deadline = Instant::now() + Duration::from_secs(5);
        while !app.connected {
            wake_rx.recv_timeout(deadline.saturating_duration_since(Instant::now())).unwrap();
            run(&mut app, Vec::new());
        }
        run(&mut app, Vec::new());
        app.show_monitor = true;
        run(&mut app, vec![egui::Event::Text("dialog-only".into())]);
        app.show_monitor = false;
        app.dialing_directory.open = true;
        run(&mut app, vec![egui::Event::Text("phonebook-only".into())]);
        app.dialing_directory.open = false;
        context.memory_mut(|memory| memory.request_focus(egui::Id::new("other-widget")));
        run(&mut app, vec![egui::Event::Text("unfocused".into())]);
        app.focus_terminal = true;
        run(&mut app, Vec::new());
        run(
            &mut app,
            vec![
                egui::Event::Text("yes".into()),
                egui::Event::Key {
                    key: egui::Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        for key in [egui::Key::Tab, egui::Key::Escape, egui::Key::ArrowLeft] {
            run(
                &mut app,
                vec![egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
            assert!(app.terminal.has_focus);
        }
        app.load(PathBuf::from("missing-file.ans"));
        assert!(app.session.is_some());
        assert_eq!(app.error.as_deref(), Some("Disconnect before opening a file"));
        received_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        app.session = None;
        let received = server.join().unwrap();
        assert_eq!(received, expected);
    }

    #[test]
    fn welcome_frame_survives_resize_and_hidpi() {
        let screen = icy_term::welcome_screen::create_welcome_screen(None);
        let caret_position = screen.caret.position();
        assert_eq!(caret_position.x, 0);
        assert!(caret_position.y > 1);
        let mut app = TerminalApp::new(screen, "Icy Term".into());
        let context = egui::Context::default();
        for (width, height, pixels_per_point) in [(1000.0, 720.0, 1.0), (360.0, 640.0, 1.0), (800.0, 600.0, 2.0)] {
            context.set_pixels_per_point(pixels_per_point);
            let output = context.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height))),
                    ..Default::default()
                },
                |context| app.show(context),
            );
            assert!(output.shapes.iter().any(|shape| matches!(shape.shape, egui::Shape::Callback(_))));
            let frame = CRTShaderProgram::new(&app.terminal, Arc::new(app.settings.clone()), None).frame(
                &app.shader_state,
                [app.terminal.visible_width_px(), app.terminal.visible_height_px()],
                pixels_per_point,
            );
            assert!(!frame.slices_blink_off.is_empty());
            assert!(frame.text_slice_count > 0);
            assert!(app.terminal.visible_width_px() <= width);
            assert!(app.terminal.visible_height_px() < height);
            assert_eq!(app.terminal.screen.lock().caret().position(), caret_position);
        }
    }

    #[test]
    fn failed_load_preserves_document() {
        let screen = FileFormat::IcyDraw
            .from_bytes(include_bytes!("../../data/welcome_screen.1.icy"), None)
            .unwrap()
            .screen;
        let mut app = TerminalApp::new(screen, "original".into());
        let instance_id = app.shader_state.instance_id;
        app.load(PathBuf::from("missing-file.ans"));
        assert!(app.error.is_some());
        assert_eq!(app.document_name, "original");
        assert_eq!(app.shader_state.instance_id, instance_id);
    }
}
