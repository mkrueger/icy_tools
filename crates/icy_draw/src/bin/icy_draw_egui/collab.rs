//! Moebius-compatible collaboration: connect dialog, chat panel, remote cursors and document sync.

use super::{Dialog, DrawApp};
use eframe::egui::{self, Color32, Key};
use icy_draw::document::Document;
use icy_engine::{AttributedChar, BitFont, Color, IceMode, Layer, Palette, Position, Rectangle, TextBuffer, TextPane};
use icy_engine_edit::collaboration::{
    self, Block, Blocks, ChatMessage, ClientCommand, ClientConfig, CollaborationCoreState, CollaborationEvent, ConnectedDocument, CursorMode, User, UserId,
};
use icy_engine_gui::egui::appearance::{self, labels, DialogButton, DialogSize};
use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    sync::mpsc,
    time::Duration,
};
use tokio::sync::mpsc as async_mpsc;

const USER_LIST_WIDTH: f32 = 180.0;
const AVATAR_SIZE: f32 = 36.0;
const CHAT_AVATAR_SIZE: f32 = 40.0;
const STATUS_BADGE_SIZE: f32 = 12.0;
const VISIBLE_CHAT_ITEMS: usize = 50;
const ICON_PIXELS: u32 = 96;

const AVATARS: [(&str, &[u8]); 13] = [
    ("face", include_bytes!("../../ui/collaboration/icons/avatars/face.svg")),
    ("face_2", include_bytes!("../../ui/collaboration/icons/avatars/face_2.svg")),
    ("face_3", include_bytes!("../../ui/collaboration/icons/avatars/face_3.svg")),
    ("face_4", include_bytes!("../../ui/collaboration/icons/avatars/face_4.svg")),
    ("face_5", include_bytes!("../../ui/collaboration/icons/avatars/face_5.svg")),
    ("face_6", include_bytes!("../../ui/collaboration/icons/avatars/face_6.svg")),
    ("account_circle", include_bytes!("../../ui/collaboration/icons/avatars/account_circle.svg")),
    ("android", include_bytes!("../../ui/collaboration/icons/avatars/android.svg")),
    ("flutter_dash", include_bytes!("../../ui/collaboration/icons/avatars/flutter_dash.svg")),
    ("mood", include_bytes!("../../ui/collaboration/icons/avatars/mood.svg")),
    ("person", include_bytes!("../../ui/collaboration/icons/avatars/person.svg")),
    ("psychology", include_bytes!("../../ui/collaboration/icons/avatars/psychology.svg")),
    (
        "sentiment_satisfied",
        include_bytes!("../../ui/collaboration/icons/avatars/sentiment_satisfied.svg"),
    ),
];

/// Moebius user status bytes: Active, Idle, Away, Web.
const STATUSES: [(&str, &str, &[u8], Color32); 4] = [
    (
        "status_active",
        "Active",
        include_bytes!("../../ui/collaboration/icons/circle_filled.svg"),
        Color32::from_rgb(77, 204, 77),
    ),
    (
        "status_idle",
        "Idle",
        include_bytes!("../../ui/collaboration/icons/schedule.svg"),
        Color32::from_rgb(230, 179, 51),
    ),
    (
        "status_away",
        "Away",
        include_bytes!("../../ui/collaboration/icons/bedtime.svg"),
        Color32::from_rgb(204, 77, 77),
    ),
    (
        "status_web",
        "Web",
        include_bytes!("../../ui/collaboration/icons/public.svg"),
        Color32::from_rgb(77, 128, 230),
    ),
];

fn avatar_index(user_id: UserId) -> usize {
    user_id.wrapping_mul(2_654_435_761) as usize % AVATARS.len()
}

fn status_entry(status: u8) -> &'static (&'static str, &'static str, &'static [u8], Color32) {
    &STATUSES[(status as usize).min(STATUSES.len() - 1)]
}

#[derive(Default)]
pub(super) struct ConnectForm {
    pub url: String,
    pub nick: String,
    pub group: String,
    pub password: String,
    pub show_password: bool,
}

impl ConnectForm {
    fn valid(&self) -> bool {
        !self.url.trim().is_empty() && !self.nick.trim().is_empty()
    }
}

/// Client side of a collaboration session; the network runs on its own Tokio runtime.
#[derive(Default)]
pub(super) struct Collaboration {
    runtime: Option<tokio::runtime::Runtime>,
    events: Option<mpsc::Receiver<CollaborationEvent>>,
    commands: Option<async_mpsc::UnboundedSender<ClientCommand>>,
    pub core: CollaborationCoreState,
    pub server: String,
    pub nick: String,
    pub group: String,
    pub connecting: bool,
    pub active: bool,
    pub chat_visible: bool,
    pub chat_input: String,
    focus_chat: bool,
    remote_paste: HashMap<UserId, Blocks>,
    previews: HashMap<UserId, (u64, egui::TextureHandle)>,
    icons: HashMap<&'static str, egui::TextureHandle>,
    floating: Option<(i32, i32)>,
    pub form: ConnectForm,
}

impl Collaboration {
    /// True while connecting or connected.
    pub fn in_session(&self) -> bool {
        self.active || self.connecting
    }

    pub fn connect(&mut self, config: ClientConfig, context: &egui::Context) -> Result<(), String> {
        self.disconnect();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .thread_name("icy-draw-collaboration")
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?;
        let (event_sender, events) = mpsc::channel();
        let (commands, mut command_receiver) = async_mpsc::unbounded_channel::<ClientCommand>();
        self.server = config.url.clone();
        self.nick = config.nick.clone();
        self.group = config.group.clone();
        let context = context.clone();
        runtime.spawn(async move {
            let (handle, mut incoming) = match collaboration::connect(config).await {
                Ok(connection) => connection,
                Err(error) => {
                    let _ = event_sender.send(CollaborationEvent::Error(error));
                    context.request_repaint();
                    return;
                }
            };
            // A single forwarder keeps the outgoing commands in order.
            let forward = handle.clone();
            tokio::spawn(async move {
                while let Some(command) = command_receiver.recv().await {
                    let disconnect = command == ClientCommand::Disconnect;
                    if forward.send_command(command).await.is_err() || disconnect {
                        break;
                    }
                }
            });
            while let Some(event) = incoming.recv().await {
                let last = matches!(event, CollaborationEvent::Disconnected | CollaborationEvent::Error(_));
                if event_sender.send(event).is_err() {
                    return;
                }
                context.request_repaint();
                if last {
                    return;
                }
            }
            let _ = event_sender.send(CollaborationEvent::Disconnected);
            context.request_repaint();
            drop(handle);
        });
        self.runtime = Some(runtime);
        self.events = Some(events);
        self.commands = Some(commands);
        self.connecting = true;
        Ok(())
    }

    pub fn disconnect(&mut self) {
        if let Some(commands) = self.commands.take() {
            let _ = commands.send(ClientCommand::Disconnect);
        }
        self.events = None;
        if let Some(runtime) = self.runtime.take() {
            // Give the close frame a moment to leave without blocking the UI.
            std::thread::spawn(move || runtime.shutdown_timeout(Duration::from_secs(1)));
        }
        self.end_session();
    }

    fn end_session(&mut self) {
        self.active = false;
        self.connecting = false;
        self.core.end_session();
        self.remote_paste.clear();
        self.previews.clear();
        self.floating = None;
    }

    pub fn send(&self, command: ClientCommand) {
        if let Some(commands) = &self.commands {
            let _ = commands.send(command);
        }
    }

    /// The server does not echo chat back to its sender, so the message is added locally as well.
    pub fn send_chat(&mut self) {
        let text = self.chat_input.trim().to_owned();
        self.chat_input.clear();
        if text.is_empty() || !self.active {
            return;
        }
        self.send(ClientCommand::Chat { text: text.clone() });
        let time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or(0);
        self.core.add_chat_message(ChatMessage {
            id: self.core.our_user_id.unwrap_or(0),
            nick: self.nick.clone(),
            group: self.group.clone(),
            text,
            time,
        });
    }

    fn add_user(&mut self, user: User, announce: bool) {
        if announce && Some(user.id) != self.core.our_user_id {
            let nick = display_nick(&user.nick);
            let text = if user.group.is_empty() {
                format!("{nick} has joined")
            } else {
                format!("{nick} <{}> has joined", user.group)
            };
            self.core.add_system_message(&text);
        }
        self.core.add_user(user);
    }

    fn remove_user(&mut self, user_id: UserId, nick: &str) {
        let group = self.core.get_user(user_id).map(|user| user.user.group.clone()).unwrap_or_default();
        let nick = if nick.is_empty() {
            self.core.get_user(user_id).map(|user| user.user.nick.clone()).unwrap_or_default()
        } else {
            nick.to_owned()
        };
        let nick = display_nick(&nick);
        self.core.add_system_message(&if group.is_empty() {
            format!("{nick} has left")
        } else {
            format!("{nick} <{group}> has left")
        });
        self.core.remove_user(user_id);
        self.remote_paste.remove(&user_id);
        self.previews.remove(&user_id);
    }

    fn user_nick(&self, user_id: UserId) -> String {
        if Some(user_id) == self.core.our_user_id {
            return display_nick(&self.nick);
        }
        self.core
            .get_user(user_id)
            .map(|user| display_nick(&user.user.nick))
            .unwrap_or_else(|| "Someone".into())
    }

    fn user_color(&self, user_id: UserId) -> Color32 {
        let (red, green, blue) = self.core.user_color(user_id);
        Color32::from_rgb(red, green, blue)
    }

    fn icon(&mut self, context: &egui::Context, name: &'static str, svg: &[u8]) -> egui::TextureId {
        self.icons
            .entry(name)
            .or_insert_with(|| {
                let image = render_svg(svg);
                context.load_texture(format!("collab-{name}"), image, egui::TextureOptions::LINEAR)
            })
            .id()
    }

    fn remote_paste_changed(&mut self, user_id: UserId, change: impl FnOnce(&Blocks) -> Blocks) {
        if let Some(blocks) = self.remote_paste.get_mut(&user_id) {
            *blocks = change(blocks);
        }
    }
}

fn display_nick(nick: &str) -> String {
    if nick.trim().is_empty() {
        "Guest".into()
    } else {
        nick.to_owned()
    }
}

fn render_svg(svg: &[u8]) -> egui::ColorImage {
    let mut pixels = resvg::tiny_skia::Pixmap::new(ICON_PIXELS, ICON_PIXELS).unwrap();
    if let Ok(tree) = resvg::usvg::Tree::from_data(svg, &Default::default()) {
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::from_scale(ICON_PIXELS as f32 / tree.size().width(), ICON_PIXELS as f32 / tree.size().height()),
            &mut pixels.as_mut(),
        );
    }
    let mut data = pixels.take();
    // The Material icons are light grey; make them white so tinting keeps the user colour exact.
    for pixel in data.chunks_exact_mut(4) {
        let alpha = pixel[3];
        pixel[0] = alpha;
        pixel[1] = alpha;
        pixel[2] = alpha;
    }
    egui::ColorImage::from_rgba_premultiplied([ICON_PIXELS as usize; 2], &data)
}

/// Formats a chat timestamp (Moebius sends milliseconds) in local time.
pub(super) fn format_time(timestamp: u64) -> String {
    use chrono::{Local, TimeZone};
    if timestamp == 0 {
        return String::new();
    }
    let millis = if timestamp < 100_000_000_000 { timestamp * 1000 } else { timestamp };
    let Some(time) = Local.timestamp_millis_opt(millis as i64).single() else {
        return String::new();
    };
    match (Local::now().date_naive() - time.date_naive()).num_days() {
        ..=0 => time.format("%H:%M").to_string(),
        1 => format!("yesterday {}", time.format("%H:%M")),
        days => format!("{days}d ago"),
    }
}

/// Builds the single-layer buffer Moebius documents map to.
pub(super) fn remote_buffer(document: &ConnectedDocument) -> TextBuffer {
    let size = (document.columns as i32, document.rows as i32);
    let mut buffer = TextBuffer::new(size);
    buffer.terminal_state.is_terminal_buffer = false;
    buffer.layers.clear();
    let mut layer = Layer::new("Layer 1", size);
    layer.preallocate_lines(size.0, size.1);
    buffer.layers.push(layer);
    buffer.ice_mode = if document.ice_colors { IceMode::Ice } else { IceMode::Blink };
    buffer.set_use_letter_spacing(document.use_9px);
    let font_name = if document.font.is_empty() { "IBM VGA" } else { &document.font };
    if let Ok(font) = BitFont::from_sauce_name(font_name).or_else(|_| BitFont::from_sauce_name("IBM VGA")) {
        buffer.set_font(0, font);
    }
    let colors: Vec<Color> = document.palette.iter().map(|[red, green, blue]| Color::new(*red, *green, *blue)).collect();
    buffer.palette = Palette::from_slice(&colors);
    for column in 0..document.columns as usize {
        for row in 0..document.rows as usize {
            let block = document.document.get(column).and_then(|cells| cells.get(row)).cloned().unwrap_or_default();
            buffer.layers[0].set_char_unchecked(Position::new(column as i32, row as i32), block_char(&block));
        }
    }
    buffer.mark_dirty();
    buffer
}

fn block_char(block: &Block) -> AttributedChar {
    let mut ch = AttributedChar {
        ch: char::from_u32(block.code).unwrap_or(' '),
        ..Default::default()
    };
    ch.attribute.set_foreground(block.fg as u32);
    ch.attribute.set_background(block.bg as u32);
    ch
}

fn block_at(blocks: &Blocks, x: u32, y: u32) -> Block {
    blocks.data.get((y * blocks.columns + x) as usize).cloned().unwrap_or_default()
}

fn transform_blocks(blocks: &Blocks, columns: u32, rows: u32, source: impl Fn(u32, u32) -> (u32, u32)) -> Blocks {
    let mut data = Vec::with_capacity((columns * rows) as usize);
    for y in 0..rows {
        for x in 0..columns {
            let (sx, sy) = source(x, y);
            data.push(block_at(blocks, sx, sy));
        }
    }
    Blocks { columns, rows, data }
}

fn rotate_blocks(blocks: &Blocks) -> Blocks {
    let rows = blocks.rows;
    transform_blocks(blocks, blocks.rows, blocks.columns, |x, y| (y, rows - 1 - x))
}

fn flip_x_blocks(blocks: &Blocks) -> Blocks {
    let columns = blocks.columns;
    transform_blocks(blocks, blocks.columns, blocks.rows, |x, y| (columns - 1 - x, y))
}

fn flip_y_blocks(blocks: &Blocks) -> Blocks {
    let rows = blocks.rows;
    transform_blocks(blocks, blocks.columns, blocks.rows, |x, y| (x, rows - 1 - y))
}

fn blocks_hash(blocks: &Blocks) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (blocks.columns, blocks.rows).hash(&mut hasher);
    for block in &blocks.data {
        (block.code, block.fg, block.bg).hash(&mut hasher);
    }
    hasher.finish()
}

enum ChatItem {
    Group {
        id: UserId,
        nick: String,
        group: String,
        time: u64,
        texts: Vec<String>,
    },
    System(String),
}

fn chat_items(messages: &[ChatMessage]) -> Vec<ChatItem> {
    let mut items: Vec<ChatItem> = Vec::new();
    for message in messages {
        if message.id == 0 && message.nick.is_empty() {
            items.push(ChatItem::System(message.text.clone()));
            continue;
        }
        if let Some(ChatItem::Group { id, nick, texts, .. }) = items.last_mut() {
            if *id == message.id && *nick == message.nick {
                texts.push(message.text.clone());
                continue;
            }
        }
        items.push(ChatItem::Group {
            id: message.id,
            nick: message.nick.clone(),
            group: message.group.clone(),
            time: message.time,
            texts: vec![message.text.clone()],
        });
    }
    let skip = items.len().saturating_sub(VISIBLE_CHAT_ITEMS);
    items.split_off(skip)
}

impl DrawApp {
    /// Joining replaces the current document, so unsaved work is offered for saving first.
    pub(super) fn open_connect_dialog(&mut self) {
        self.document.finish();
        if self.modified() {
            self.pending = None;
            self.quitting = false;
            self.pending_connect = true;
            self.dialog = Some(Dialog::Close);
            return;
        }
        self.show_connect_dialog();
    }

    pub(super) fn show_connect_dialog(&mut self) {
        let form = &mut self.collab.form;
        if form.url.is_empty() {
            form.url = self.settings.last_collaboration_server().unwrap_or_default();
        }
        if form.nick.is_empty() {
            form.nick = self.settings.collaboration.nick.clone();
        }
        if form.group.is_empty() {
            form.group = self.settings.collaboration.group.clone();
        }
        form.show_password = false;
        self.dialog = Some(Dialog::Connect);
    }

    pub(super) fn start_collaboration(&mut self, context: &egui::Context) {
        let form = &self.collab.form;
        let config = ClientConfig {
            url: form.url.trim().to_owned(),
            nick: form.nick.trim().to_owned(),
            group: form.group.trim().to_owned(),
            password: form.password.clone(),
            ..Default::default()
        };
        self.settings.add_collaboration_server(&config.url);
        self.settings.collaboration.nick = config.nick.clone();
        self.settings.collaboration.group = config.group.clone();
        if self.persist_settings {
            self.settings.store_persistent();
        }
        self.document.finish();
        let result = self.collab.connect(config, context);
        self.result(result);
    }

    pub(super) fn disconnect_collaboration(&mut self) {
        self.collab.disconnect();
    }

    pub(super) fn connect_dialog(&mut self, context: &egui::Context) -> bool {
        #[derive(Clone, Copy)]
        enum Action {
            Cancel,
            Connect,
        }
        let servers = self.settings.collaboration_servers_list();
        let mut submit = false;
        let response = appearance::Dialog::new("connect-to-server")
            .title("Connect to Server")
            .subtitle("Join a Moebius-compatible collaboration session.")
            .size(DialogSize::Medium)
            .show(context, |dialog| {
                dialog.content(|ui| {
                    let form = &mut self.collab.form;
                    let enter = |ui: &egui::Ui, response: &egui::Response| response.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter));
                    appearance::group(ui, "Server", |ui| {
                        appearance::form_row(ui, "Server URL", |ui| {
                            ui.horizontal(|ui| {
                                let recent = if servers.is_empty() { 0.0 } else { 30.0 + ui.spacing().item_spacing.x };
                                let response = ui.add_sized(
                                    [(ui.available_width() - recent).max(80.0), ui.spacing().interact_size.y],
                                    appearance::text_edit(&mut form.url).hint_text("host:port or ws://host:port"),
                                );
                                if ui.memory(|memory| memory.focused().is_none()) && form.url.is_empty() {
                                    response.request_focus();
                                }
                                submit |= enter(ui, &response);
                                if !servers.is_empty() {
                                    let button = ui
                                        .add_sized([30.0, ui.spacing().interact_size.y], egui::Button::new("▾"))
                                        .on_hover_text("Recent servers");
                                    egui::Popup::menu(&button).show(|ui| {
                                        for server in servers.iter().rev() {
                                            if ui.selectable_label(form.url == *server, server).clicked() {
                                                form.url = server.clone();
                                            }
                                        }
                                    });
                                }
                            });
                        });
                        appearance::form_row(ui, "Password", |ui| {
                            ui.horizontal(|ui| {
                                let width = (ui.available_width() - 30.0 - ui.spacing().item_spacing.x).max(80.0);
                                let response = ui.add_sized(
                                    [width, ui.spacing().interact_size.y],
                                    appearance::text_edit(&mut form.password).hint_text("Optional").password(!form.show_password),
                                );
                                submit |= enter(ui, &response);
                                let (icon, tooltip) = if form.show_password {
                                    ("visibility_off", "Hide password")
                                } else {
                                    ("visibility", "Show password")
                                };
                                if self.icons.subtle_button(ui, icon, tooltip, false, 30.0).clicked() {
                                    form.show_password = !form.show_password;
                                }
                            });
                        });
                    });
                    appearance::group(ui, "Identity", |ui| {
                        appearance::form_row(ui, "Nickname", |ui| {
                            let response = ui.add(appearance::text_edit(&mut form.nick).hint_text("Your name").desired_width(f32::INFINITY));
                            submit |= enter(ui, &response);
                        });
                        appearance::form_row(ui, "Group", |ui| {
                            let response = ui.add(appearance::text_edit(&mut form.group).hint_text("Optional").desired_width(f32::INFINITY));
                            submit |= enter(ui, &response);
                        });
                    });
                });
                dialog.buttons([
                    DialogButton::cancel(labels::cancel(), Action::Cancel),
                    DialogButton::primary("Connect", Action::Connect).enabled(self.collab.form.valid()),
                ]);
            });
        let action = if submit && self.collab.form.valid() {
            Some(Action::Connect)
        } else {
            response.action
        };
        match action {
            Some(Action::Connect) => {
                self.dialog = None;
                self.start_collaboration(context);
                true
            }
            Some(Action::Cancel) => true,
            None => response.dismissed,
        }
    }

    pub(super) fn poll_collaboration(&mut self) {
        while let Some(event) = self.collab.events.as_ref().and_then(|events| events.try_recv().ok()) {
            self.collaboration_event(event);
        }
    }

    fn collaboration_error(&mut self, message: String) {
        self.collab.disconnect();
        self.dialog = Some(Dialog::Error(message));
    }

    pub(super) fn collaboration_event(&mut self, event: CollaborationEvent) {
        match event {
            CollaborationEvent::Connected(document) => {
                self.collab.core.start_session(&document);
                let mut state = icy_engine_edit::EditState::from_buffer(remote_buffer(&document));
                state.set_sauce_meta(icy_engine_edit::SauceMetaData {
                    title: document.title.clone().into(),
                    author: document.author.clone().into(),
                    group: document.group.clone().into(),
                    comments: document.comments.lines().map(|line| line.to_string().into()).collect(),
                });
                self.replace_document(Document::from_state(state));
                for user in document.users.iter().cloned() {
                    self.collab.add_user(user, false);
                }
                self.collab.connecting = false;
                self.collab.active = true;
                self.collab.chat_visible = true;
            }
            CollaborationEvent::Refused { reason } => {
                let reason = if reason.is_empty() { "Wrong password".into() } else { reason };
                self.collaboration_error(format!("The collaboration server refused the connection.\n\n{reason}"));
            }
            CollaborationEvent::UserJoined(user) => self.collab.add_user(user, true),
            CollaborationEvent::UserLeft { user_id, nick } => self.collab.remove_user(user_id, &nick),
            CollaborationEvent::CursorMoved { user_id, col, row } => {
                self.collab.core.update_cursor(user_id, col, row);
                self.collab.remote_paste.remove(&user_id);
            }
            CollaborationEvent::SelectionChanged { user_id, selecting, col, row } => {
                self.collab.core.update_selection(user_id, selecting, col, row);
                self.collab.remote_paste.remove(&user_id);
            }
            CollaborationEvent::OperationStarted { user_id, col, row } => self.collab.core.update_operation(user_id, col, row),
            CollaborationEvent::CursorHidden { user_id } => {
                self.collab.core.hide_user_cursor(user_id);
                self.collab.remote_paste.remove(&user_id);
            }
            CollaborationEvent::Draw { col, row, block } => {
                self.document.with_state(|state| {
                    let buffer = state.get_buffer_mut();
                    if let Some(layer) = buffer.layers.first_mut() {
                        let position = Position::new(col, row);
                        let mut ch = layer.char_at(position);
                        let remote = block_char(&block);
                        ch.ch = remote.ch;
                        ch.attribute.set_foreground(block.fg as u32);
                        ch.attribute.set_background(block.bg as u32);
                        layer.set_char(position, ch);
                    }
                    buffer.mark_dirty();
                });
            }
            CollaborationEvent::DrawPreview { .. } => {}
            CollaborationEvent::Chat(message) => self.collab.core.add_chat_message(message),
            CollaborationEvent::StatusChanged(status) => self.collab.core.update_user_status(status.id, status.status),
            CollaborationEvent::SauceChanged(sauce) => {
                self.document.with_state(|state| {
                    state.set_sauce_meta(icy_engine_edit::SauceMetaData {
                        title: sauce.title.clone().into(),
                        author: sauce.author.clone().into(),
                        group: sauce.group.clone().into(),
                        comments: sauce.comments.lines().map(|line| line.to_string().into()).collect(),
                    })
                });
                let nick = self.collab.user_nick(sauce.id);
                self.collab.core.add_system_message(&format!("{nick} changed the SAUCE record"));
            }
            CollaborationEvent::CanvasResized { user_id, columns, rows } => {
                self.resize_remote_canvas(columns, rows);
                self.collab.core.update_canvas_size(columns, rows);
                let nick = self.collab.user_nick(user_id);
                self.collab
                    .core
                    .add_system_message(&format!("{nick} changed the canvas size to {columns} × {rows}"));
            }
            CollaborationEvent::IceColorsChanged { user_id, value } => {
                self.document.with_state(|state| {
                    let buffer = state.get_buffer_mut();
                    buffer.ice_mode = if value { IceMode::Ice } else { IceMode::Blink };
                    buffer.mark_dirty();
                });
                self.collab.core.ice_colors = value;
                let nick = self.collab.user_nick(user_id);
                self.collab
                    .core
                    .add_system_message(&format!("{nick} turned iCE colors {}", if value { "on" } else { "off" }));
            }
            CollaborationEvent::Use9pxChanged { user_id, value } => {
                self.document.with_state(|state| {
                    let buffer = state.get_buffer_mut();
                    buffer.set_use_letter_spacing(value);
                    buffer.mark_dirty();
                });
                self.collab.core.use_9px = value;
                let nick = self.collab.user_nick(user_id);
                self.collab
                    .core
                    .add_system_message(&format!("{nick} turned letter spacing {}", if value { "on" } else { "off" }));
            }
            CollaborationEvent::FontChanged { user_id, font_name } => {
                if let Ok(font) = BitFont::from_sauce_name(&font_name) {
                    self.document.with_state(|state| {
                        let buffer = state.get_buffer_mut();
                        buffer.set_font(0, font);
                        buffer.mark_dirty();
                    });
                }
                self.collab.core.font = font_name.clone();
                let nick = self.collab.user_nick(user_id);
                self.collab.core.add_system_message(&format!("{nick} changed the font to {font_name}"));
            }
            CollaborationEvent::PasteAsSelection { user_id, blocks } => {
                let (col, row) = self
                    .collab
                    .core
                    .get_user(user_id)
                    .and_then(|user| user.operation.as_ref().map(|operation| (operation.col, operation.row)).or(user.cursor))
                    .unwrap_or((0, 0));
                self.collab.remote_paste.insert(user_id, blocks);
                self.collab.core.update_operation(user_id, col, row);
            }
            CollaborationEvent::Rotate { user_id } => self.collab.remote_paste_changed(user_id, rotate_blocks),
            CollaborationEvent::FlipX { user_id } => self.collab.remote_paste_changed(user_id, flip_x_blocks),
            CollaborationEvent::FlipY { user_id } => self.collab.remote_paste_changed(user_id, flip_y_blocks),
            CollaborationEvent::BackgroundChanged { user_id, .. } => {
                let nick = self.collab.user_nick(user_id);
                self.collab.core.add_system_message(&format!("{nick} changed the background"));
            }
            CollaborationEvent::Disconnected => {
                if self.collab.in_session() {
                    self.collaboration_error("Collaboration disconnected\n\nThe connection to the collaboration server was lost.".into());
                }
            }
            CollaborationEvent::Error(error) => {
                if self.collab.in_session() {
                    self.collaboration_error(format!(
                        "Collaboration connection error\n\nThe connection to the collaboration server failed.\n\nError: {error}"
                    ));
                }
            }
        }
    }

    /// Applies a Moebius canvas resize to the document layer, keeping the overlapping region.
    fn resize_remote_canvas(&mut self, columns: u32, rows: u32) {
        use icy_engine::{Line, Size};
        let width = columns as i32;
        let height = rows as i32;
        self.document.with_state(|state| {
            let buffer = state.get_buffer_mut();
            buffer.set_size(Size::new(width, height));
            // A local floating paste lives on its own layer and keeps its size.
            if let Some(layer) = buffer.layers.first_mut() {
                layer.set_size(Size::new(width, height));
                layer.lines.truncate(height.max(0) as usize);
                while layer.lines.len() < height.max(0) as usize {
                    layer.lines.push(Line::create(width));
                }
                for line in &mut layer.lines {
                    line.chars.resize(width.max(0) as usize, AttributedChar::invisible());
                }
            }
            buffer.mark_dirty();
        });
    }

    /// Sends local edits, caret moves and floating pastes to the server.
    pub(super) fn sync_collaboration(&mut self) {
        if !self.collab.active {
            return;
        }
        let paste = self.document.paste_active();
        let (stack, position, selecting, floating) = self.document.with_state(|state| {
            let (position, selecting) = match state.selection() {
                Some(selection) => (selection.lead, true),
                None => (state.layer_to_document_position(state.get_caret().position()), false),
            };
            let floating = if paste { state.get_floating_layer_position() } else { None };
            (state.get_undo_stack(), position, selecting, floating)
        });
        let mut commands = self
            .collab
            .core
            .sync_from_undo_stack(&stack.lock().unwrap(), (position.x, position.y), selecting);
        match (self.collab.floating, floating) {
            (None, Some((col, row))) => {
                if let Some(blocks) = self.document.with_state(|state| state.get_floating_layer_blocks()) {
                    commands.push(ClientCommand::PasteAsSelection { blocks });
                }
                commands.push(ClientCommand::Operation { col, row });
            }
            (Some(previous), Some((col, row))) if previous != (col, row) => commands.push(ClientCommand::Operation { col, row }),
            (Some(_), None) => commands.push(ClientCommand::Cursor {
                col: position.x,
                row: position.y,
            }),
            _ => {}
        }
        self.collab.floating = floating;
        for command in commands {
            self.collab.send(command);
        }
    }

    /// Scrolls the canvas so the given user's cursor is centred.
    pub(super) fn goto_user(&mut self, user_id: UserId) {
        let Some(user) = self.collab.core.get_user(user_id) else {
            return;
        };
        let position = match user.cursor_mode {
            CursorMode::Selection => user.selection.as_ref().map(|selection| (selection.col, selection.row)).or(user.cursor),
            CursorMode::Operation => user.operation.as_ref().map(|operation| (operation.col, operation.row)).or(user.cursor),
            _ => user.cursor,
        };
        let Some((col, row)) = position else {
            return;
        };
        let info = self.view.terminal.render_info.read();
        let cell = egui::vec2(info.font_width, info.font_height * if info.scan_lines { 2.0 } else { 1.0 }) * self.view.zoom;
        drop(info);
        let center = egui::vec2(col as f32 + 0.5, row as f32 + 0.5) * cell;
        let offset = (center - self.canvas_rect.size() / 2.0).max(egui::Vec2::ZERO).min(self.view.max_offset);
        self.view.scroll_to = Some(offset);
    }

    fn paste_preview(&mut self, context: &egui::Context, user_id: UserId) -> Option<egui::TextureId> {
        let blocks = self.collab.remote_paste.get(&user_id)?;
        if blocks.columns == 0 || blocks.rows == 0 {
            return None;
        }
        let hash = blocks_hash(blocks);
        if let Some((cached, texture)) = self.collab.previews.get(&user_id) {
            if *cached == hash {
                return Some(texture.id());
            }
        }
        let image = self.document.with_state(|state| {
            let buffer = state.get_buffer();
            let mut preview = TextBuffer::create((blocks.columns as i32, blocks.rows as i32));
            preview.palette = buffer.palette.clone();
            preview.set_font_table(buffer.font_table());
            preview.ice_mode = buffer.ice_mode;
            preview.set_use_letter_spacing(buffer.use_letter_spacing());
            for y in 0..blocks.rows {
                for x in 0..blocks.columns {
                    preview.layers[0].set_char((x as i32, y as i32), block_char(&block_at(blocks, x, y)));
                }
            }
            let (size, rgba) = preview.render_to_rgba(&Rectangle::from(0, 0, blocks.columns as i32, blocks.rows as i32).into(), false);
            (size.width > 0 && size.height > 0 && !rgba.is_empty())
                .then(|| egui::ColorImage::from_rgba_unmultiplied([size.width as usize, size.height as usize], &rgba))
        })?;
        let texture = context.load_texture(format!("collab-paste-{user_id}"), image, egui::TextureOptions::NEAREST);
        let id = texture.id();
        self.collab.previews.insert(user_id, (hash, texture));
        Some(id)
    }

    /// Paints remote carets, selections and floating pastes on top of the canvas.
    pub(super) fn remote_cursors(&mut self, context: &egui::Context, painter: &egui::Painter, origin: egui::Pos2, step: egui::Vec2) {
        if !self.collab.active {
            return;
        }
        let mut users: Vec<_> = self.collab.core.remote_users.values().cloned().collect();
        users.sort_by_key(|user| user.user.id);
        self.collab.previews.retain(|id, _| self.collab.remote_paste.contains_key(id));
        let cell = |col: i32, row: i32| egui::Rect::from_min_size(origin + egui::vec2(col as f32 * step.x, row as f32 * step.y), step);
        for user in users {
            let color = self.collab.user_color(user.user.id);
            let rect = match user.cursor_mode {
                CursorMode::Hidden => continue,
                CursorMode::Editing => {
                    let Some((col, row)) = user.cursor else {
                        continue;
                    };
                    let rect = cell(col, row);
                    painter.rect_filled(rect, 0, color.gamma_multiply(0.22));
                    painter.rect_stroke(rect, 0, egui::Stroke::new(2.0, color), egui::StrokeKind::Outside);
                    rect
                }
                CursorMode::Selection => {
                    let Some((col, row)) = user.selection.as_ref().map(|selection| (selection.col, selection.row)).or(user.cursor) else {
                        continue;
                    };
                    let (start_col, start_row) = user.cursor.unwrap_or((col, row));
                    let rect = cell(start_col, start_row).union(cell(col, row));
                    painter.rect_filled(rect, 0, color.gamma_multiply(0.08));
                    painter.rect_stroke(rect, 0, egui::Stroke::new(2.0, color), egui::StrokeKind::Outside);
                    painter.rect_stroke(rect.shrink(3.0), 0, egui::Stroke::new(1.0, color), egui::StrokeKind::Inside);
                    rect
                }
                CursorMode::Operation => {
                    let Some((col, row)) = user.operation.as_ref().map(|operation| (operation.col, operation.row)).or(user.cursor) else {
                        continue;
                    };
                    if let Some(texture) = self.paste_preview(context, user.user.id) {
                        let blocks = &self.collab.remote_paste[&user.user.id];
                        let rect = egui::Rect::from_min_size(cell(col, row).min, egui::vec2(blocks.columns as f32 * step.x, blocks.rows as f32 * step.y));
                        painter.image(
                            texture,
                            rect,
                            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                            Color32::WHITE.gamma_multiply(0.85),
                        );
                        painter.rect_stroke(rect, 0, egui::Stroke::new(2.0, color), egui::StrokeKind::Outside);
                        rect
                    } else {
                        let rect = cell(col, row);
                        painter.rect_filled(rect, 0, color.gamma_multiply(0.25));
                        painter.rect_stroke(rect, 0, egui::Stroke::new(2.0, color), egui::StrokeKind::Outside);
                        painter.rect_stroke(rect.shrink(2.0), 0, egui::Stroke::new(1.0, color), egui::StrokeKind::Inside);
                        rect
                    }
                }
            };
            let label = if user.cursor_mode == CursorMode::Operation && !user.user.group.is_empty() {
                format!("{} <{}>", display_nick(&user.user.nick), user.user.group)
            } else {
                display_nick(&user.user.nick)
            };
            let galley = painter.layout_no_wrap(label, egui::FontId::monospace(11.0), Color32::WHITE);
            let label_rect = egui::Rect::from_min_size(
                egui::pos2(rect.left() - 2.0, rect.top() - galley.size().y - 6.0),
                galley.size() + egui::vec2(8.0, 4.0),
            );
            painter.rect_filled(label_rect, 3, Color32::from_black_alpha(166));
            painter.rect_filled(
                egui::Rect::from_min_size(label_rect.min, egui::vec2(3.0, label_rect.height())),
                egui::CornerRadius {
                    nw: 3,
                    sw: 3,
                    ..Default::default()
                },
                color,
            );
            painter.galley(label_rect.min + egui::vec2(5.0, 2.0), galley, Color32::WHITE);
        }
    }

    fn avatar(&mut self, ui: &egui::Ui, rect: egui::Rect, user_id: UserId, status: Option<u8>) {
        let (name, svg) = AVATARS[avatar_index(user_id)];
        let texture = self.collab.icon(ui.ctx(), name, svg);
        let color = self.collab.user_color(user_id);
        let uv = egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0));
        ui.painter().image(texture, rect, uv, color);
        if let Some(status) = status {
            let (name, _, svg, fill) = *status_entry(status);
            let badge = egui::Rect::from_min_size(rect.max - egui::Vec2::splat(STATUS_BADGE_SIZE), egui::Vec2::splat(STATUS_BADGE_SIZE));
            ui.painter()
                .circle_filled(badge.center(), STATUS_BADGE_SIZE / 2.0 + 2.0, ui.visuals().panel_fill);
            ui.painter().circle_filled(badge.center(), STATUS_BADGE_SIZE / 2.0, fill);
            let icon = self.collab.icon(ui.ctx(), name, svg);
            ui.painter().image(icon, badge.shrink(2.5), uv, Color32::from_gray(235));
        }
    }

    fn user_entry(&mut self, ui: &mut egui::Ui, user_id: UserId, nick: &str, detail: &str, status: u8, clickable: bool) -> egui::Response {
        let sense = if clickable { egui::Sense::click() } else { egui::Sense::hover() };
        let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), AVATAR_SIZE + 8.0), sense);
        if clickable && response.hovered() {
            ui.painter().rect_filled(rect, 6, ui.visuals().widgets.hovered.weak_bg_fill);
        }
        let avatar = egui::Rect::from_min_size(rect.min + egui::vec2(4.0, 4.0), egui::Vec2::splat(AVATAR_SIZE));
        self.avatar(ui, avatar, user_id, Some(status));
        let text_left = avatar.right() + 8.0;
        let width = (rect.right() - text_left - 4.0).max(10.0);
        let layout = |text: &str, size: f32, color: Color32| {
            let mut job = egui::text::LayoutJob::single_section(text.to_owned(), egui::TextFormat::simple(egui::FontId::proportional(size), color));
            job.wrap = egui::text::TextWrapping::truncate_at_width(width);
            ui.fonts_mut(|fonts| fonts.layout_job(job))
        };
        let nick = layout(nick, 14.0, ui.visuals().strong_text_color());
        let detail = (!detail.is_empty()).then(|| layout(detail, 12.0, ui.visuals().weak_text_color()));
        let total = nick.size().y + detail.as_ref().map_or(0.0, |detail| detail.size().y + 1.0);
        let mut top = rect.center().y - total / 2.0;
        ui.painter().galley(egui::pos2(text_left, top), nick.clone(), Color32::PLACEHOLDER);
        top += nick.size().y + 1.0;
        if let Some(detail) = detail {
            ui.painter().galley(egui::pos2(text_left, top), detail, Color32::PLACEHOLDER);
        }
        if clickable {
            response.on_hover_cursor(egui::CursorIcon::PointingHand)
        } else {
            response
        }
    }

    fn user_list(&mut self, ui: &mut egui::Ui) {
        let own_height = AVATAR_SIZE + 8.0 + 9.0;
        let users: Vec<(UserId, String, String, u8)> = self
            .collab
            .core
            .sorted_users()
            .into_iter()
            .map(|user| (user.user.id, display_nick(&user.user.nick), user.user.group.clone(), user.status))
            .collect();
        let list_height = (ui.available_height() - own_height).max(0.0);
        ui.allocate_ui(egui::vec2(ui.available_width(), list_height), |ui| {
            egui::ScrollArea::vertical()
                .id_salt("collab-user-list")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    if users.is_empty() {
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new("No other users").size(12.0).weak());
                    }
                    for (id, nick, group, status) in users {
                        let status_label = status_entry(status).1;
                        let detail = if group.is_empty() { String::new() } else { format!("<{group}>") };
                        let response = self.user_entry(ui, id, &nick, &detail, status, true);
                        if response
                            .on_hover_text(format!("{nick} · {status_label}\nClick to jump to their cursor"))
                            .clicked()
                        {
                            self.goto_user(id);
                        }
                    }
                });
        });
        ui.separator();
        let own_id = self.collab.core.our_user_id.unwrap_or(0);
        let nick = display_nick(&self.collab.nick);
        let detail = if self.collab.group.is_empty() {
            "You".to_owned()
        } else {
            format!("<{}> · You", self.collab.group)
        };
        self.user_entry(ui, own_id, &nick, &detail, 0, false);
    }

    fn chat_messages(&mut self, ui: &mut egui::Ui) {
        let items = chat_items(&self.collab.core.chat_messages);
        egui::ScrollArea::vertical()
            .id_salt("collab-chat-messages")
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 12.0;
                if items.is_empty() {
                    ui.label(egui::RichText::new("No messages yet").size(12.0).weak());
                }
                for item in items {
                    match item {
                        ChatItem::System(text) => {
                            ui.label(egui::RichText::new(text).size(12.0).italics().weak());
                        }
                        ChatItem::Group { id, nick, group, time, texts } => {
                            ui.horizontal_top(|ui| {
                                ui.spacing_mut().item_spacing.x = 8.0;
                                let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(CHAT_AVATAR_SIZE), egui::Sense::hover());
                                self.avatar(ui, rect, id, None);
                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 2.0;
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 6.0;
                                        ui.label(egui::RichText::new(display_nick(&nick)).size(14.0).strong());
                                        if !group.is_empty() {
                                            ui.label(egui::RichText::new(format!("<{group}>")).size(12.0).weak());
                                        }
                                        let time = format_time(time);
                                        if !time.is_empty() {
                                            ui.label(egui::RichText::new(format!("· {time}")).size(11.0).weak());
                                        }
                                    });
                                    for text in texts {
                                        ui.add(egui::Label::new(egui::RichText::new(text).size(13.0)).wrap());
                                    }
                                });
                            });
                        }
                    }
                }
            });
    }

    /// Discord-style chat pane: user list on the left, grouped messages and input on the right.
    pub(super) fn chat_panel(&mut self, ui: &mut egui::Ui) {
        egui::SidePanel::left("collab-users")
            .exact_width(USER_LIST_WIDTH)
            .resizable(false)
            .frame(egui::Frame::new().inner_margin(egui::Margin::same(6)))
            .show_inside(ui, |ui| self.user_list(ui));
        egui::TopBottomPanel::bottom("collab-input")
            .frame(egui::Frame::new().inner_margin(egui::Margin {
                left: 12,
                right: 12,
                top: 4,
                bottom: 8,
            }))
            .show_separator_line(false)
            .show_inside(ui, |ui| {
                let response = ui.add(
                    appearance::text_edit(&mut self.collab.chat_input)
                        .id_salt("collab-chat-input")
                        .hint_text("Type a message...")
                        .desired_width(f32::INFINITY),
                );
                if response.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter)) {
                    self.collab.send_chat();
                    self.collab.focus_chat = true;
                }
                if std::mem::take(&mut self.collab.focus_chat) {
                    response.request_focus();
                }
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::new().inner_margin(egui::Margin {
                left: 12,
                right: 12,
                top: 8,
                bottom: 4,
            }))
            .show_inside(ui, |ui| self.chat_messages(ui));
    }

    pub(super) fn toggle_chat(&mut self) {
        self.collab.chat_visible = !self.collab.chat_visible;
        self.collab.focus_chat = self.collab.chat_visible;
    }

    /// Connection indicator for the status bar.
    pub(super) fn collaboration_status(&mut self, ui: &mut egui::Ui) {
        if self.collab.active {
            let users = self.collab.core.remote_users.len() + 1;
            let label = format!("● {} · {users} {}", self.collab.server, if users == 1 { "user" } else { "users" });
            let response = super::widgets::status_button(ui, &label, "Collaboration session — click to show or hide the chat");
            ui.painter()
                .circle_filled(egui::pos2(response.rect.left() + 10.0, response.rect.center().y), 3.5, STATUSES[0].3);
            if response.clicked() {
                self.toggle_chat();
            }
        } else if self.collab.connecting {
            ui.spinner();
            ui.label(egui::RichText::new(format!("Connecting to {}…", self.collab.server)).size(12.0).weak());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blocks() -> Blocks {
        Blocks {
            columns: 3,
            rows: 2,
            data: (0..6).map(|code| Block { code: 65 + code, fg: 7, bg: 0 }).collect(),
        }
    }

    fn codes(blocks: &Blocks) -> String {
        blocks.data.iter().map(|block| char::from_u32(block.code).unwrap()).collect()
    }

    #[test]
    fn paste_previews_follow_remote_rotate_and_flip() {
        assert_eq!(codes(&flip_x_blocks(&blocks())), "CBAFED");
        assert_eq!(codes(&flip_y_blocks(&blocks())), "DEFABC");
        let rotated = rotate_blocks(&blocks());
        assert_eq!((rotated.columns, rotated.rows), (2, 3));
        assert_eq!(codes(&rotated), "DAEBFC");
    }

    #[test]
    fn chat_groups_consecutive_messages_and_keeps_system_lines() {
        let message = |id, nick: &str, text: &str| ChatMessage {
            id,
            nick: nick.into(),
            group: String::new(),
            text: text.into(),
            time: 0,
        };
        let items = chat_items(&[
            message(1, "alice", "hi"),
            message(1, "alice", "there"),
            message(0, "", "bob has joined"),
            message(2, "bob", "yo"),
        ]);
        assert_eq!(items.len(), 3);
        assert!(matches!(&items[0], ChatItem::Group { texts, .. } if texts == &["hi", "there"]));
        assert!(matches!(&items[1], ChatItem::System(text) if text == "bob has joined"));
        assert!(matches!(&items[2], ChatItem::Group { nick, .. } if nick == "bob"));
    }
}
