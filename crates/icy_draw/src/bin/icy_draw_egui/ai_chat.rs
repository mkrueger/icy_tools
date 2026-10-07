use std::fmt::{self, Write};

use eframe::egui::{self, Color32};
use icy_draw::{fl, AiChatSettings, AiProvider};
use icy_engine::{Position, TextPane};

use icy_engine_gui::egui::appearance;

use super::{
    widgets::{self, Icons},
    DrawApp,
};

#[path = "ai_animation.rs"]
mod animation_tools;
#[path = "ai_canvas.rs"]
mod canvas;
#[path = "ai_font.rs"]
mod font_tools;
#[path = "ai_workspace.rs"]
mod workspace;
use workspace::Workspace;
#[path = "ai_connection.rs"]
mod connection;
#[path = "ai_image.rs"]
mod image_attachment;
#[path = "ai_knowledge.rs"]
mod knowledge;
use connection::{Job, Message, Request, Response};
use image_attachment::ReferenceImage;
#[path = "ai_copilot.rs"]
mod copilot;

const MAX_CONTEXT_BYTES: usize = 48 * 1024;
const MAX_CONTEXT_CELLS: usize = 16 * 1024;
const FONT_PREVIEW_GLYPHS: usize = 64;
const TRUNCATED: &str = "\n[Snapshot truncated at 48 KiB; remaining content is not attached.]";

struct Entry {
    user: bool,
    text: String,
    attachment: Option<String>,
    image: Option<ReferenceImage>,
}

impl Entry {
    fn message(&self) -> Message {
        if self.user {
            Message::user(&self.text, self.attachment.as_deref()).with_image(self.image.clone())
        } else {
            Message {
                role: "assistant".into(),
                content: self.text.clone(),
                image: None,
            }
        }
    }
}

/// Changes the assistant made on a draft of the open editor's document.
struct Proposal {
    workspace: Workspace,
    /// Identifies the document the draft was taken from.
    document: usize,
    /// Changed cells of a drawing; changed lines of a script.
    changes: usize,
    script_diff: Option<animation_tools::Diff>,
    preview: Option<egui::TextureHandle>,
    glyph_page: usize,
}

impl Proposal {
    fn new(workspace: Workspace, document: usize) -> Self {
        let (changes, script_diff) = match &workspace {
            Workspace::Canvas(draft) => (draft.changes().len(), None),
            Workspace::Animation(draft) => {
                let diff = draft.diff();
                (diff.removed.max(diff.added), Some(diff))
            }
            Workspace::Font(draft) => (draft.changed_codes().len(), None),
        };
        Self {
            workspace,
            document,
            changes,
            script_diff,
            preview: None,
            glyph_page: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProposalAction {
    Accept,
    Discard,
}

#[derive(Default)]
pub(super) struct Chat {
    pub visible: bool,
    was_visible: bool,
    /// Shows the connection settings instead of the conversation.
    settings_open: bool,
    connection: AiChatSettings,
    key: String,
    models: Vec<String>,
    connected: bool,
    entries: Vec<Entry>,
    input: String,
    attachment: Option<String>,
    preview_attachment: bool,
    image: Option<ReferenceImage>,
    image_texture: Option<egui::TextureHandle>,
    image_import: Option<image_attachment::Import>,
    drop_hovered: bool,
    pending: Option<Entry>,
    job: Option<Job>,
    error: Option<String>,
    /// The connection preferences changed and should be stored.
    persist: bool,
    knowledge_persist: bool,
    reference_preview: Option<(String, String)>,
    copilot: Option<copilot::Backend>,
    /// The signed-in Copilot account.
    account: Option<String>,
    /// Incremented when the conversation is cleared, so Copilot starts a new session.
    conversation: u64,
    /// Copilot connects once in the background when the panel first opens.
    auto_connected: bool,
    proposal: Option<Proposal>,
    proposal_action: Option<ProposalAction>,
    /// The document a pending request draws on.
    pending_document: usize,
    /// Send was requested; the app adds the drawing draft and sends.
    send_requested: bool,
}

impl Chat {
    pub fn new(connection: AiChatSettings) -> Self {
        Self {
            settings_open: connection.provider == AiProvider::OpenAiCompatible && connection.endpoint.is_empty(),
            connection,
            ..Default::default()
        }
    }

    fn poll(&mut self) {
        if let Some(result) = self.image_import.as_ref().and_then(image_attachment::Import::poll) {
            self.image_import = None;
            match result {
                Ok(image) => {
                    self.image = Some(image);
                    self.image_texture = None;
                }
                Err(error) => {
                    log::warn!("Cannot attach reference picture: {error}");
                    self.error = Some(error);
                }
            }
        }
        let Some(result) = self.job.as_ref().and_then(Job::poll) else {
            return;
        };
        self.job = None;
        self.handle(result);
    }

    fn handle(&mut self, result: Result<Response, String>) {
        match result {
            Ok(Response::Models { models, account }) => {
                if self.model().is_empty() && models.len() == 1 {
                    *self.model_mut() = models[0].clone();
                }
                self.models = models;
                self.account = account;
                self.connected = true;
                self.settings_open = false;
                self.persist = true;
            }
            Ok(Response::Reply(text)) => self.push_reply(text),
            Ok(Response::Proposal(text, workspace)) => {
                self.push_reply(text);
                self.proposal = Some(Proposal::new(*workspace, self.pending_document));
            }
            Err(error) => {
                self.restore_draft();
                self.error = Some(error);
            }
        }
    }

    fn push_reply(&mut self, text: String) {
        if let Some(entry) = self.pending.take() {
            self.entries.push(entry);
        }
        self.entries.push(Entry {
            user: false,
            text,
            attachment: None,
            image: None,
        });
    }

    /// Puts a message that was not answered back into the composer.
    fn restore_draft(&mut self) {
        if let Some(entry) = self.pending.take() {
            if self.input.trim().is_empty() {
                self.input = entry.text;
            }
            if self.attachment.is_none() {
                self.attachment = entry.attachment;
            }
            if self.image.is_none() {
                self.image = entry.image;
                self.image_texture = None;
            }
        }
    }

    fn cancel(&mut self) {
        self.job = None;
        self.restore_draft();
        self.error = Some(fl!("ai-chat-cancelled"));
    }

    fn clear(&mut self) {
        self.job = None;
        self.pending = None;
        self.entries.clear();
        self.input.clear();
        self.attachment = None;
        self.preview_attachment = false;
        self.image = None;
        self.image_texture = None;
        self.image_import = None;
        self.drop_hovered = false;
        self.error = None;
        self.conversation += 1;
        self.proposal = None;
    }

    fn model(&self) -> &String {
        match self.connection.provider {
            AiProvider::OpenAiCompatible => &self.connection.model,
            AiProvider::Copilot => &self.connection.copilot_model,
        }
    }

    fn model_mut(&mut self) -> &mut String {
        match self.connection.provider {
            AiProvider::OpenAiCompatible => &mut self.connection.model,
            AiProvider::Copilot => &mut self.connection.copilot_model,
        }
    }

    fn needs_setup(&self) -> bool {
        self.connection.provider == AiProvider::OpenAiCompatible && self.connection.endpoint.trim().is_empty()
    }

    fn copilot(&mut self) -> &copilot::Backend {
        let path = self.connection.copilot_path.clone();
        self.copilot.get_or_insert_with(|| copilot::Backend::start(path))
    }

    /// Forgets the connection state; used when the provider or its location changes.
    fn disconnect(&mut self) {
        self.clear();
        self.models.clear();
        self.connected = false;
        self.account = None;
        self.copilot = None;
    }

    fn connect(&mut self, context: &egui::Context) {
        self.error = None;
        self.job = Some(match self.connection.provider {
            AiProvider::OpenAiCompatible => Job::start(self.connection.clone(), self.key.clone(), Request::Models, context.clone()),
            AiProvider::Copilot => self.copilot().request(copilot::Command::Connect, context),
        });
    }

    fn can_send(&self) -> bool {
        self.job.is_none() && self.image_import.is_none() && !self.input.trim().is_empty() && !self.model().trim().is_empty()
    }

    /// `draft` is the drawing Copilot may edit and the document it belongs to.
    fn send(&mut self, context: &egui::Context, draft: Option<(Workspace, usize)>) {
        if self.image_import.is_some() {
            self.error = Some(fl!("ai-chat-image-loading"));
            return;
        }
        if self.input.trim().is_empty() {
            self.error = Some(fl!("ai-chat-empty"));
            return;
        }
        let knowledge = match knowledge::prepare(&self.connection.knowledge) {
            Ok(knowledge) => knowledge,
            Err(error) => {
                log::warn!("Assistant knowledge rejected: {error}");
                self.error = Some(error);
                return;
            }
        };
        let entry = Entry {
            user: true,
            text: std::mem::take(&mut self.input).trim().to_owned(),
            attachment: self.attachment.take(),
            image: self.image.take(),
        };
        self.preview_attachment = false;
        self.image_texture = None;
        let mut messages: Vec<_> = self.entries.iter().map(Entry::message).collect();
        messages.push(entry.message());
        self.pending = Some(entry);
        self.error = None;
        self.job = Some(match self.connection.provider {
            AiProvider::OpenAiCompatible => {
                if !knowledge.is_empty() {
                    messages.insert(
                        0,
                        Message {
                            role: "system".into(),
                            content: knowledge,
                            image: None,
                        },
                    );
                }
                Job::start(self.connection.clone(), self.key.clone(), Request::Chat(messages), context.clone())
            }
            AiProvider::Copilot => {
                let draft = draft.map(|(draft, document)| {
                    self.pending_document = document;
                    Box::new(draft)
                });
                let command = copilot::Command::Chat {
                    model: self.model().clone(),
                    conversation: self.conversation,
                    messages,
                    draft,
                    knowledge,
                };
                self.copilot().request(command, context)
            }
        });
    }

    /// Runs before the application's general file opener, using the visible panel's last layout.
    pub(super) fn route_file_drop(&mut self, context: &egui::Context, blocked: bool) {
        let panel = self
            .visible
            .then(|| egui::containers::panel::PanelState::load(context, egui::Id::new("ai-chat")))
            .flatten();
        let Some(panel) = panel else {
            self.drop_hovered = false;
            return;
        };
        let (position, hovering, dropping) = context.input(|input| {
            (
                input.pointer.latest_pos(),
                !input.raw.hovered_files.is_empty(),
                !input.raw.dropped_files.is_empty(),
            )
        });
        if hovering {
            if let Some(position) = position {
                self.drop_hovered = panel.rect.contains(position) && !blocked;
            }
        }
        let target = position.map_or(self.drop_hovered, |position| panel.rect.contains(position));
        if dropping && target {
            let files = context.input_mut(|input| std::mem::take(&mut input.raw.dropped_files));
            self.drop_hovered = false;
            let error = if blocked {
                Some(fl!("ai-chat-image-blocked"))
            } else if self.job.is_some() || self.image_import.is_some() {
                Some(fl!("ai-chat-image-busy"))
            } else if files.len() != 1 {
                Some(fl!("ai-chat-image-single"))
            } else if self.image.is_some() {
                Some(fl!("ai-chat-image-existing"))
            } else {
                None
            };
            if let Some(error) = error {
                log::warn!("Reference picture drop rejected: {error}");
                self.error = Some(error);
            } else if let Some(file) = files.into_iter().next() {
                self.error = None;
                self.image_import = Some(image_attachment::Import::start(file, context.clone()));
                if !self.needs_setup() {
                    self.settings_open = false;
                }
            }
        } else if !hovering && !dropping {
            self.drop_hovered = false;
        }
    }

    fn header(&mut self, ui: &mut egui::Ui, icons: &mut Icons) {
        ui.horizontal(|ui| {
            ui.set_height(28.0);
            if self.settings_open {
                if icons.button_sized(ui, "arrow_left", &fl!("ai-chat-back"), false, 24.0).clicked() {
                    self.settings_open = false;
                }

                ui.label(appearance::bold(ui, fl!("ai-chat-settings")));
            } else {
                ui.label(appearance::bold(ui, fl!("ai-chat-title")));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icons.button_sized(ui, "settings", &fl!("ai-chat-settings"), self.settings_open, 24.0).clicked() {
                    self.settings_open = !self.settings_open;
                }
                if !self.settings_open && icons.button_sized(ui, "add", &fl!("ai-chat-new"), false, 24.0).clicked() {
                    self.clear();
                }
            });
        });
    }

    fn settings_view(&mut self, ui: &mut egui::Ui, context: &egui::Context) {
        let busy = self.job.is_some();
        egui::ScrollArea::vertical().id_salt("ai-settings-scroll").show(ui, |ui| {
            ui.add_enabled_ui(!busy, |ui| {
                let mut provider = self.connection.provider;
                let providers = [
                    (AiProvider::OpenAiCompatible, fl!("ai-chat-provider-openai"), fl!("ai-chat-provider-openai-tip")),
                    (AiProvider::Copilot, fl!("ai-chat-provider-copilot"), fl!("ai-chat-provider-copilot-tip")),
                ];
                if widgets::segmented(ui, &mut provider, &providers) && provider != self.connection.provider {
                    // Conversations are never forwarded to a different service.
                    self.disconnect();
                    self.connection.provider = provider;
                }
                ui.add_space(8.0);
                match self.connection.provider {
                    AiProvider::OpenAiCompatible => self.openai_settings(ui),
                    AiProvider::Copilot => self.copilot_settings(ui),
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let enabled = !self.needs_setup();
                    if ui.add_enabled(enabled, appearance::primary_button(fl!("ai-chat-connect"))).clicked() {
                        self.connect(context);
                    }
                    if busy {
                        ui.spinner();
                    } else if self.connected {
                        let status = match &self.account {
                            Some(account) => fl!("ai-chat-connected-as", account = account.clone(), count = self.models.len()),
                            None => fl!("ai-chat-connected", count = self.models.len()),
                        };
                        ui.colored_label(Color32::from_rgb(77, 204, 77), status);
                    }
                });
                if let Some(error) = &self.error {
                    ui.add_space(4.0);
                    ui.add(
                        egui::Label::new(egui::RichText::new(error).color(ui.visuals().error_fg_color))
                            .wrap()
                            .selectable(true),
                    );
                }
                ui.add_space(8.0);
                ui.separator();
                ui.label(fl!("ai-chat-model"));
                self.model_picker(ui, f32::INFINITY);
                let manual = appearance::text_edit(self.model_mut())
                    .id(egui::Id::new("ai-model-id"))
                    .desired_width(f32::INFINITY)
                    .hint_text(fl!("ai-chat-model-id"));
                if ui.add(manual).lost_focus() {
                    self.persist = true;
                }
                ui.add_space(8.0);
                self.knowledge_settings(ui);
                ui.add_space(8.0);
                ui.label(egui::RichText::new(fl!("ai-chat-privacy")).small().weak());
                ui.add_space(6.0);
                ui.label(egui::RichText::new(fl!("ai-chat-image-hint")).small().weak());
            });
        });
    }

    fn knowledge_settings(&mut self, ui: &mut egui::Ui) {
        let mut changed = false;
        ui.collapsing(fl!("ai-knowledge-title"), |ui| {
            ui.label(fl!("ai-knowledge-privacy"));
            ui.label(fl!("ai-knowledge-instructions"));
            changed |= ui
                .add(
                    egui::TextEdit::multiline(&mut self.connection.knowledge.custom_instructions)
                        .margin(appearance::FIELD_MARGIN)
                        .id(egui::Id::new("ai-instructions"))
                        .desired_width(f32::INFINITY)
                        .desired_rows(5)
                        .hint_text(fl!("ai-knowledge-instructions-hint")),
                )
                .changed();
            ui.label(fl!("ai-knowledge-limits"));
            for (label, catalog, selected) in [
                (fl!("ai-knowledge-references"), knowledge::REFERENCES, &mut self.connection.knowledge.references),
                (fl!("ai-knowledge-presets"), knowledge::PRESETS, &mut self.connection.knowledge.presets),
            ] {
                ui.collapsing(label, |ui| {
                    for item in catalog {
                        let mut enabled = selected.iter().any(|id| id == item.id);
                        if ui.checkbox(&mut enabled, item.label()).changed() {
                            selected.retain(|id| id != item.id);
                            if enabled {
                                selected.push(item.id.into());
                            }
                            changed = true;
                        }
                        ui.push_id(item.id, |ui| {
                            ui.collapsing(fl!("ai-knowledge-preview"), |ui| {
                                ui.add(egui::Label::new(item.text).wrap().selectable(true));
                            });
                        });
                    }
                });
            }
            ui.collapsing(fl!("ai-knowledge-files"), |ui| {
                ui.label(fl!("ai-knowledge-files-hint"));
                let mut remove = None;
                let mut preview = None;
                for (index, path) in self.connection.knowledge.reference_files.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.add(egui::Label::new(path).wrap().selectable(true));
                        if ui.small_button(fl!("ai-knowledge-preview")).clicked() {
                            preview = Some(path.clone());
                        }
                        if ui.small_button(fl!("ai-chat-remove")).clicked() {
                            remove = Some(index);
                        }
                    });
                }
                if let Some(index) = remove {
                    self.connection.knowledge.reference_files.remove(index);
                    self.reference_preview = None;
                    changed = true;
                }
                if let Some(path) = preview {
                    match knowledge::read_reference(std::path::Path::new(&path)) {
                        Ok(text) => self.reference_preview = Some((path, text)),
                        Err(error) => {
                            log::warn!("Cannot preview assistant reference {path}: {error}");
                            self.reference_preview = None;
                            self.error = Some(format!("{path}: {error}"));
                        }
                    }
                }
                if let Some((path, text)) = &self.reference_preview {
                    ui.label(path);
                    egui::ScrollArea::both().id_salt("ai-reference-preview").max_height(180.0).show(ui, |ui| {
                        ui.add(egui::Label::new(egui::RichText::new(text).monospace()).selectable(true));
                    });
                }
                if ui.button(fl!("ai-knowledge-add-file")).clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter(fl!("ai-knowledge-files"), &["txt", "md", "rst", "toml", "json", "icy", "ans", "asc", "pcb"])
                        .pick_file()
                    {
                        match knowledge::read_reference(&path) {
                            Ok(_) => match path.to_str() {
                                Some(path) => {
                                    if !self.connection.knowledge.reference_files.iter().any(|existing| existing == path) {
                                        let mut candidate = self.connection.knowledge.clone();
                                        candidate.reference_files.push(path.into());
                                        match knowledge::prepare(&candidate) {
                                            Ok(_) => {
                                                self.connection.knowledge = candidate;
                                                changed = true;
                                            }
                                            Err(error) => {
                                                log::warn!("Assistant reference selection rejected: {error}");
                                                self.error = Some(error);
                                            }
                                        }
                                    }
                                }
                                None => {
                                    log::warn!("Assistant reference path is not UTF-8: {}", path.display());
                                    self.error = Some(fl!("ai-knowledge-path-error"));
                                }
                            },
                            Err(error) => {
                                log::warn!("Cannot import assistant reference {}: {error}", path.display());
                                self.error = Some(format!("{}: {error}", path.display()));
                            }
                        }
                    }
                }
            });
        });
        if changed {
            self.knowledge_persist = true;
        }
    }

    fn openai_settings(&mut self, ui: &mut egui::Ui) {
        ui.label(fl!("ai-chat-endpoint"));
        let endpoint = appearance::text_edit(&mut self.connection.endpoint)
            .id(egui::Id::new("ai-url"))
            .desired_width(f32::INFINITY)
            .hint_text("https://api.openai.com/v1");
        if ui.add(endpoint).changed() {
            // Never forward an existing conversation or key to a newly configured server.
            self.disconnect();
            self.key.clear();
            self.connection.model.clear();
        }
        ui.label(egui::RichText::new(fl!("ai-chat-endpoint-hint")).small().weak());
        ui.add_space(6.0);
        ui.label(fl!("ai-chat-key"));
        let key = appearance::text_edit(&mut self.key)
            .id(egui::Id::new("ai-key"))
            .desired_width(f32::INFINITY)
            .password(true);
        if ui.add(key).changed() {
            self.connected = false;
        }
        ui.label(egui::RichText::new(fl!("ai-chat-key-memory")).small().weak());
    }

    fn copilot_settings(&mut self, ui: &mut egui::Ui) {
        ui.add(egui::Label::new(fl!("ai-chat-copilot-info")).wrap());
        ui.add_space(6.0);
        ui.label(fl!("ai-chat-copilot-path"));
        let detected = copilot::find_cli("").map(|path| path.display().to_string());
        let path = appearance::text_edit(&mut self.connection.copilot_path)
            .id(egui::Id::new("ai-copilot-path"))
            .desired_width(f32::INFINITY)
            .hint_text(detected.clone().unwrap_or_else(|| fl!("ai-chat-copilot-not-found")));
        if ui.add(path).changed() {
            self.disconnect();
        }
        if detected.is_none() && self.connection.copilot_path.trim().is_empty() {
            ui.hyperlink_to(fl!("ai-chat-copilot-install"), "https://gh.io/copilot-cli");
        }
        ui.label(egui::RichText::new(fl!("ai-chat-copilot-login-hint")).small().weak());
    }

    fn model_picker(&mut self, ui: &mut egui::Ui, width: f32) {
        let selected = if self.model().is_empty() {
            fl!("ai-chat-select-model")
        } else {
            self.model().clone()
        };
        let mut manage = false;
        egui::ComboBox::from_id_salt("ai-model")
            .selected_text(egui::RichText::new(selected).small())
            .width(width.min(ui.available_width()))
            .truncate()
            .show_ui(ui, |ui| {
                let mut chosen = None;
                for model in &self.models {
                    if ui.selectable_label(model == self.model(), model).clicked() {
                        chosen = Some(model.clone());
                    }
                }
                if let Some(model) = chosen {
                    *self.model_mut() = model;
                    self.persist = true;
                }
                if !self.models.is_empty() {
                    ui.separator();
                }
                manage = ui.selectable_label(false, fl!("ai-chat-manage")).clicked();
            });
        if manage {
            self.settings_open = true;
        }
    }

    fn history(&mut self, ui: &mut egui::Ui, icons: &mut Icons) {
        egui::ScrollArea::vertical()
            .id_salt("ai-chat-history")
            .auto_shrink(false)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                ui.add_space(4.0);
                if self.entries.is_empty() && self.pending.is_none() {
                    self.empty_state(ui);
                    return;
                }
                for (index, entry) in self.entries.iter().enumerate() {
                    ui.push_id(index, |ui| {
                        if entry.user {
                            user_bubble(ui, entry);
                        } else {
                            message_body(ui, &entry.text);
                            if icons.subtle_button(ui, "file_copy", &fl!("ai-chat-copy"), true, 22.0).clicked() {
                                ui.ctx().copy_text(entry.text.clone());
                            }
                        }
                        ui.add_space(10.0);
                    });
                }
                self.proposal_card(ui);
                if let Some(entry) = &self.pending {
                    ui.push_id("pending", |ui| user_bubble(ui, entry));
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.spinner();
                        if let Some(progress) = self.job.as_ref().and_then(|job| job.progress.as_ref()) {
                            let progress = progress.lock();
                            ui.vertical(|ui| {
                                let phase = match progress.phase {
                                    connection::ProgressPhase::Preparing => fl!("ai-chat-progress-preparing"),
                                    connection::ProgressPhase::Thinking => fl!("ai-chat-thinking"),
                                    connection::ProgressPhase::Generating => fl!("ai-chat-progress-generating"),
                                    connection::ProgressPhase::RunningTool => fl!("ai-chat-progress-running"),
                                };
                                ui.label(egui::RichText::new(phase).weak());
                                let seconds = progress.started.elapsed().as_secs();
                                let elapsed = format!("{}:{:02}", seconds / 60, seconds % 60);
                                ui.label(
                                    egui::RichText::new(fl!("ai-chat-progress-time", elapsed = elapsed, count = progress.tool_calls))
                                        .small()
                                        .weak(),
                                );
                                if let Some(tool) = &progress.last_tool {
                                    ui.label(egui::RichText::new(fl!("ai-chat-progress-tool", tool = tool.as_str())).small().weak());
                                }
                                if let Some(count) = progress.changed_glyphs {
                                    ui.label(egui::RichText::new(fl!("ai-chat-progress-glyphs", count = count)).small().weak());
                                }
                            });
                            ui.ctx().request_repaint_after(std::time::Duration::from_secs(1));
                        } else {
                            ui.label(egui::RichText::new(fl!("ai-chat-thinking")).weak());
                        }
                    });
                }
            });
    }

    fn proposal_card(&mut self, ui: &mut egui::Ui) {
        let busy = self.job.is_some();
        let Some(proposal) = &mut self.proposal else {
            return;
        };
        let mut action = None;
        egui::Frame::new()
            .fill(ui.visuals().faint_bg_color)
            .stroke(egui::Stroke::new(1.0, appearance::PRIMARY))
            .corner_radius(8)
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                match (&proposal.workspace, &proposal.script_diff) {
                    (Workspace::Canvas(draft), _) => {
                        ui.label(appearance::bold(ui, fl!("ai-chat-proposal", count = proposal.changes)));
                        ui.add_space(4.0);
                        let texture = proposal.preview.get_or_insert_with(|| render_preview(ui.ctx(), draft));
                        let size = texture.size_vec2();
                        let scale = (ui.available_width() / size.x).min(2.0);
                        ui.add(egui::Image::new((texture.id(), size * scale)));
                    }
                    (Workspace::Animation(_), Some(diff)) => {
                        let title = fl!("ai-chat-proposal-script", removed = diff.removed, added = diff.added, line = diff.line);
                        ui.label(appearance::bold(ui, title));
                        ui.add_space(4.0);
                        script_diff(ui, diff);
                    }
                    (Workspace::Animation(_), None) => {}
                    (Workspace::Font(draft), _) => {
                        ui.label(appearance::bold(ui, fl!("ai-chat-proposal-font", count = proposal.changes)));
                        ui.add_space(4.0);
                        let pages = proposal.changes.div_ceil(FONT_PREVIEW_GLYPHS).max(1);
                        if pages > 1 {
                            let old_page = proposal.glyph_page;
                            ui.horizontal(|ui| {
                                if ui
                                    .add_enabled(proposal.glyph_page > 0, egui::Button::new("<"))
                                    .on_hover_text(fl!("ai-chat-font-prev"))
                                    .clicked()
                                {
                                    proposal.glyph_page -= 1;
                                }
                                let page = proposal.glyph_page + 1;
                                let first = proposal.glyph_page * FONT_PREVIEW_GLYPHS + 1;
                                let last = (first + FONT_PREVIEW_GLYPHS - 1).min(proposal.changes);
                                ui.label(fl!(
                                    "ai-chat-font-page",
                                    page = page,
                                    pages = pages,
                                    first = first,
                                    last = last,
                                    count = proposal.changes
                                ));
                                if ui
                                    .add_enabled(proposal.glyph_page + 1 < pages, egui::Button::new(">"))
                                    .on_hover_text(fl!("ai-chat-font-next"))
                                    .clicked()
                                {
                                    proposal.glyph_page += 1;
                                }
                            });
                            if old_page != proposal.glyph_page {
                                proposal.preview = None;
                            }
                        }
                        let texture = proposal.preview.get_or_insert_with(|| render_glyphs(ui.ctx(), draft, proposal.glyph_page));
                        let size = texture.size_vec2();
                        let scale = (ui.available_width() / size.x).min(4.0);
                        ui.add(egui::Image::new((texture.id(), size * scale)));
                        ui.label(egui::RichText::new(fl!("ai-chat-proposal-font-legend")).small().weak());
                    }
                }
                ui.add_space(4.0);
                ui.add_enabled_ui(!busy, |ui| {
                    ui.horizontal(|ui| {
                        if ui.add(appearance::primary_button(fl!("ai-chat-accept"))).clicked() {
                            action = Some(ProposalAction::Accept);
                        }
                        if ui.button(fl!("ai-chat-discard")).clicked() {
                            action = Some(ProposalAction::Discard);
                        }
                    });
                });
                ui.label(egui::RichText::new(fl!("ai-chat-proposal-hint")).small().weak());
            });
        ui.add_space(10.0);
        if action.is_some() {
            self.proposal_action = action;
        }
    }

    fn empty_state(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space((ui.available_height() * 0.25).max(16.0));
            ui.label(appearance::bold(ui, fl!("ai-chat-title")).size(18.0));
            ui.add_space(6.0);
            let scope = if self.connection.provider == AiProvider::Copilot {
                fl!("ai-chat-can-draw")
            } else {
                fl!("ai-chat-read-only")
            };
            ui.label(egui::RichText::new(scope).weak());
            ui.add_space(6.0);
            ui.label(egui::RichText::new(fl!("ai-chat-privacy")).small().weak());
            if self.needs_setup() {
                ui.add_space(12.0);
                if ui.add(appearance::primary_button(fl!("ai-chat-setup"))).clicked() {
                    self.settings_open = true;
                }
            }
        });
    }

    /// Returns true when the editor context should be attached.
    fn composer(&mut self, ui: &mut egui::Ui, icons: &mut Icons, can_attach: bool) -> bool {
        let mut attach = false;
        if let Some(error) = &self.error {
            ui.add(egui::Label::new(egui::RichText::new(error).color(ui.visuals().error_fg_color)).wrap());
            ui.add_space(4.0);
        }
        let prompt = egui::Id::new("ai-prompt");
        let focused = ui.memory(|memory| memory.has_focus(prompt));
        let visuals = ui.visuals();
        let stroke = if focused {
            egui::Stroke::new(1.0, appearance::PRIMARY)
        } else {
            visuals.widgets.inactive.bg_stroke
        };
        egui::Frame::new()
            .fill(visuals.extreme_bg_color)
            .stroke(stroke)
            .corner_radius(8)
            .inner_margin(egui::Margin::same(6))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                if let Some(attachment) = &self.attachment {
                    let mut remove = false;
                    ui.horizontal(|ui| {
                        let label = fl!("ai-chat-attached", size = format!("{:.1} KiB", attachment.len() as f32 / 1024.0));
                        let chip = egui::Button::new(egui::RichText::new(label).small())
                            .selected(self.preview_attachment)
                            .corner_radius(4);
                        if ui.add(chip).on_hover_text(fl!("ai-chat-preview")).clicked() {
                            self.preview_attachment = !self.preview_attachment;
                        }
                        remove = icons.subtle_button(ui, "delete", &fl!("ai-chat-remove"), true, 20.0).clicked();
                    });
                    if self.preview_attachment {
                        egui::ScrollArea::both().id_salt("ai-attachment").max_height(140.0).show(ui, |ui| {
                            ui.add(egui::Label::new(egui::RichText::new(attachment).monospace().small()).extend().selectable(true));
                        });
                    }
                    if remove {
                        self.attachment = None;
                        self.preview_attachment = false;
                    }
                }
                if let Some(image) = &self.image {
                    let mut remove = false;
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::Label::new(fl!(
                                "ai-chat-image-attached",
                                name = image.name.clone(),
                                width = image.width,
                                height = image.height
                            ))
                            .wrap(),
                        );
                        remove = icons.subtle_button(ui, "delete", &fl!("ai-chat-remove"), true, 20.0).clicked();
                    });
                    let texture = self
                        .image_texture
                        .get_or_insert_with(|| ui.ctx().load_texture("ai-reference-picture", image.color_image(), egui::TextureOptions::LINEAR));
                    let size = texture.size_vec2();
                    let scale = (ui.available_width() / size.x).min(96.0 / size.y).min(1.0);
                    ui.add(egui::Image::new((texture.id(), size * scale)));
                    if image.original_size != (image.width, image.height) {
                        ui.label(
                            egui::RichText::new(fl!("ai-chat-image-resized", width = image.original_size.0, height = image.original_size.1))
                                .small()
                                .weak(),
                        );
                    }
                    ui.label(egui::RichText::new(fl!("ai-chat-image-model")).small().weak());
                    if remove {
                        self.image = None;
                        self.image_texture = None;
                    }
                }
                if self.image_import.is_some() {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(fl!("ai-chat-image-loading"));
                    });
                }
                // Enter sends, Shift+Enter inserts a line break.
                let enter = focused
                    && ui.input_mut(|input| {
                        let before = input.events.len();
                        input
                            .events
                            .retain(|event| !matches!(event, egui::Event::Key { key: egui::Key::Enter, pressed: true, modifiers, .. } if modifiers.is_none()));
                        before != input.events.len()
                    });
                egui::ScrollArea::vertical().id_salt("ai-prompt-scroll").max_height(160.0).show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut self.input)
                            .id(prompt)
                            .frame(false)
                            .desired_rows(2)
                            .desired_width(f32::INFINITY)
                            .char_limit(32 * 1024)
                            .hint_text(fl!("ai-chat-prompt")),
                    );
                });
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(can_attach && self.job.is_none(), |ui| {
                        attach = icons.button_sized(ui, "add", &fl!("ai-chat-attach"), false, 24.0).clicked();
                    });
                    self.model_picker(ui, 180.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.job.is_some() {
                            if icons.button_sized(ui, "stop", &fl!("ai-chat-cancel"), false, 24.0).clicked() {
                                self.cancel();
                            }
                        } else {
                            let ready = self.can_send();
                            let clicked = icons.button_sized(ui, "send", &fl!("ai-chat-send"), ready, 24.0).clicked();
                            if ready && (clicked || enter) {
                                self.send_requested = true;
                                ui.memory_mut(|memory| memory.request_focus(prompt));
                            }
                        }
                    });
                });
            });
        attach
    }
}

/// Removed and added script lines, like a unified diff hunk.
fn script_diff(ui: &mut egui::Ui, diff: &animation_tools::Diff) {
    const MAX_LINES: usize = 200;
    egui::Frame::new()
        .fill(ui.visuals().extreme_bg_color)
        .corner_radius(4)
        .inner_margin(egui::Margin::same(6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::ScrollArea::both().id_salt("ai-script-diff").max_height(260.0).show(ui, |ui| {
                let removed = Color32::from_rgb(230, 110, 110);
                let added = Color32::from_rgb(110, 210, 120);
                let kept = ui.visuals().weak_text_color();
                for (kind, line) in diff.lines.iter().take(MAX_LINES) {
                    let color = match kind {
                        '-' => removed,
                        '+' => added,
                        _ => kept,
                    };
                    ui.add(egui::Label::new(egui::RichText::new(format!("{kind} {line}")).monospace().color(color)).extend());
                }
                if diff.lines.len() > MAX_LINES {
                    ui.label(egui::RichText::new(format!("… {} more", diff.lines.len() - MAX_LINES)).weak());
                }
            });
        });
}

/// Changed glyphs in bands of up to 16: the original above, the new glyph below.
fn render_glyphs(context: &egui::Context, draft: &font_tools::FontDraft, page: usize) -> egui::TextureHandle {
    const PER_ROW: usize = 16;
    const GAP: usize = 2;
    let codes: Vec<_> = draft
        .changed_codes()
        .into_iter()
        .skip(page * FONT_PREVIEW_GLYPHS)
        .take(FONT_PREVIEW_GLYPHS)
        .collect();
    let (width, height) = (draft.width.max(1), draft.height.max(1));
    let columns = codes.len().clamp(1, PER_ROW);
    let bands = codes.len().div_ceil(PER_ROW).max(1);
    let band_height = 2 * height + 3 * GAP;
    let size = [columns * (width + GAP) + GAP, bands * band_height];
    let mut image = egui::ColorImage::filled(size, Color32::from_gray(24));
    for (index, &code) in codes.iter().enumerate() {
        let left = GAP + (index % PER_ROW) * (width + GAP);
        let top = (index / PER_ROW) * band_height + GAP;
        for (glyph, top, color) in [
            (&draft.original[code as usize], top, Color32::from_gray(130)),
            (&draft.glyphs[code as usize], top + height + GAP, Color32::WHITE),
        ] {
            for (y, row) in glyph.iter().enumerate().take(height) {
                for (x, &set) in row.iter().enumerate().take(width) {
                    if set {
                        image[(left + x, top + y)] = color;
                    }
                }
            }
        }
    }
    context.load_texture("ai-proposal-glyphs", image, egui::TextureOptions::NEAREST)
}

/// Renders the area around the changed cells.
fn render_preview(context: &egui::Context, draft: &canvas::Draft) -> egui::TextureHandle {
    let buffer = &draft.buffer;
    let (mut left, mut top, mut right, mut bottom) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for (layer, position, _) in draft.changes() {
        let offset = buffer.layers[layer].offset();
        left = left.min(position.x + offset.x);
        top = top.min(position.y + offset.y);
        right = right.max(position.x + offset.x);
        bottom = bottom.max(position.y + offset.y);
    }
    if left > right {
        (left, top, right, bottom) = (0, 0, buffer.width() - 1, buffer.height() - 1);
    }
    let left = (left - 2).max(0);
    let top = (top - 1).max(0);
    let width = ((right + 3).min(buffer.width()) - left).clamp(1, 160);
    let height = ((bottom + 2).min(buffer.height()) - top).clamp(1, 100);
    let (size, pixels) = buffer.render_to_rgba(&icy_engine::Rectangle::from(left, top, width, height).into(), false);
    let image = egui::ColorImage::from_rgba_unmultiplied([size.width as usize, size.height as usize], &pixels);
    context.load_texture("ai-proposal", image, egui::TextureOptions::NEAREST)
}

/// Right-aligned bubble that is only as wide as its text.
fn user_bubble(ui: &mut egui::Ui, entry: &Entry) {
    const MARGIN: egui::Vec2 = egui::vec2(10.0, 6.0);
    let max = (ui.available_width() * 0.85 - 2.0 * MARGIN.x).max(40.0);
    let measure = |text: egui::WidgetText| text.into_galley(ui, Some(egui::TextWrapMode::Wrap), max, egui::TextStyle::Body).size().x;
    let note = entry
        .attachment
        .as_ref()
        .map(|_| egui::RichText::new(fl!("ai-chat-context-sent")).small().weak());
    let image_note = entry.image.as_ref().map(|image| {
        egui::RichText::new(fl!(
            "ai-chat-image-attached",
            name = image.name.clone(),
            width = image.width,
            height = image.height
        ))
        .small()
        .weak()
    });
    let width = measure(entry.text.clone().into())
        .max(note.clone().map_or(0.0, |note| measure(note.into())))
        .max(image_note.clone().map_or(0.0, |note| measure(note.into())))
        .ceil()
        + 1.0;
    ui.horizontal(|ui| {
        ui.add_space((ui.available_width() - width - 2.0 * MARGIN.x).max(0.0));
        egui::Frame::new()
            .fill(ui.visuals().widgets.inactive.weak_bg_fill)
            .corner_radius(8)
            .inner_margin(egui::Margin::symmetric(MARGIN.x as i8, MARGIN.y as i8))
            .show(ui, |ui| {
                ui.set_width(width);
                ui.vertical(|ui| {
                    if let Some(note) = note {
                        ui.label(note);
                    }
                    if let Some(note) = image_note {
                        ui.label(note);
                    }
                    ui.add(egui::Label::new(&entry.text).wrap().selectable(true));
                });
            });
    });
}

/// Renders the Markdown subset models commonly use: fenced code, headings, bullets, bold and inline code.
fn message_body(ui: &mut egui::Ui, text: &str) {
    for (index, part) in text.split("```").enumerate() {
        if index % 2 == 1 {
            let code = match part.split_once('\n') {
                Some((language, rest)) if !language.contains(' ') => rest,
                _ => part,
            };
            egui::Frame::new()
                .fill(ui.visuals().extreme_bg_color)
                .corner_radius(4)
                .inner_margin(egui::Margin::same(6))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    egui::ScrollArea::horizontal().id_salt(("code", index)).show(ui, |ui| {
                        ui.add(
                            egui::Label::new(egui::RichText::new(code.trim_end_matches('\n')).monospace())
                                .extend()
                                .selectable(true),
                        );
                    });
                });
            continue;
        }
        for line in part.trim_matches('\n').lines() {
            let trimmed = line.trim_start();
            if trimmed.is_empty() {
                ui.add_space(4.0);
            } else if let Some(heading) = trimmed.strip_prefix('#') {
                ui.add_space(2.0);
                ui.label(appearance::bold(ui, heading.trim_start_matches('#').trim()));
            } else if let Some(item) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")) {
                let indent = (line.len() - trimmed.len()) as f32 * 4.0;
                ui.horizontal_top(|ui| {
                    ui.add_space(indent + 4.0);
                    ui.label("•");
                    ui.add(egui::Label::new(inline(ui, item)).wrap().selectable(true));
                });
            } else {
                ui.add(egui::Label::new(inline(ui, line)).wrap().selectable(true));
            }
        }
    }
}

fn inline(ui: &egui::Ui, line: &str) -> egui::text::LayoutJob {
    let style = ui.style();
    let body = egui::TextStyle::Body.resolve(style);
    let bold = egui::FontId::new(body.size, appearance::bold_family(ui));
    let mono = egui::TextStyle::Monospace.resolve(style);
    let (mut strong, mut code) = (false, false);
    let mut job = egui::text::LayoutJob::default();
    let mut rest = line;
    while !rest.is_empty() {
        let next = if code {
            rest.find('`').map(|index| (index, 1))
        } else {
            [rest.find("**").map(|index| (index, 2)), rest.find('`').map(|index| (index, 1))]
                .into_iter()
                .flatten()
                .min_by_key(|(index, _)| *index)
        };
        let (end, marker) = next.unwrap_or((rest.len(), 0));
        if end > 0 {
            let format = egui::TextFormat {
                font_id: if code {
                    mono.clone()
                } else if strong {
                    bold.clone()
                } else {
                    body.clone()
                },
                color: if strong {
                    ui.visuals().strong_text_color()
                } else {
                    ui.visuals().text_color()
                },
                background: if code { ui.visuals().extreme_bg_color } else { Color32::TRANSPARENT },
                ..Default::default()
            };
            job.append(&rest[..end], 0.0, format);
        }
        if marker == 0 {
            break;
        }
        if marker == 1 {
            code = !code;
        } else {
            strong = !strong;
        }
        rest = &rest[end + marker..];
    }
    job
}

impl DrawApp {
    pub(super) fn ai_chat_panel(&mut self, context: &egui::Context, blocked: bool) {
        self.ai_chat.poll();
        let opened = self.ai_chat.visible && !self.ai_chat.was_visible;
        self.ai_chat.was_visible = self.ai_chat.visible;
        if !self.ai_chat.visible {
            return;
        }
        if opened && !self.ai_chat.settings_open {
            context.memory_mut(|memory| memory.request_focus(egui::Id::new("ai-prompt")));
        }
        let chat = &mut self.ai_chat;
        if chat.connection.provider == AiProvider::Copilot && !chat.connected && chat.job.is_none() && !std::mem::replace(&mut chat.auto_connected, true) {
            chat.connect(context);
        }
        let focused_before = chat_has_focus(context);
        let can_attach = !self.show_start;
        let mut attach = false;
        let Self { ai_chat: chat, icons, .. } = self;
        egui::SidePanel::right("ai-chat")
            .default_width(380.0)
            .width_range(280.0..=760.0)
            .resizable(true)
            .show(context, |ui| {
                if blocked {
                    ui.disable();
                }
                chat.header(ui, icons);
                ui.separator();
                if chat.settings_open {
                    chat.settings_view(ui, context);
                    return;
                }
                egui::TopBottomPanel::bottom("ai-chat-compose")
                    .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(0, 6)))
                    .show_separator_line(false)
                    .show_inside(ui, |ui| attach = chat.composer(ui, icons, can_attach));
                chat.history(ui, icons);
            });
        if self.ai_chat.drop_hovered && !blocked {
            if let Some(panel) = egui::containers::panel::PanelState::load(context, egui::Id::new("ai-chat")) {
                let painter = context.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("ai-image-drop")));
                painter.rect_filled(panel.rect, 0, Color32::from_rgba_unmultiplied(32, 100, 160, 80));
                painter.rect_stroke(panel.rect.shrink(2.0), 4, egui::Stroke::new(2.0, appearance::PRIMARY), egui::StrokeKind::Inside);
                painter.text(
                    panel.rect.center(),
                    egui::Align2::CENTER_CENTER,
                    fl!("ai-chat-image-drop"),
                    egui::FontId::proportional(16.0),
                    Color32::WHITE,
                );
            }
        }
        if attach {
            self.ai_chat.attachment = Some(self.ai_editor_context());
            self.ai_chat.preview_attachment = false;
        }
        if std::mem::take(&mut self.ai_chat.send_requested) {
            let draft = if self.ai_chat.connection.provider == AiProvider::Copilot {
                self.ai_draft()
            } else {
                None
            };
            self.ai_chat.send(context, draft);
        }
        match self.ai_chat.proposal_action.take() {
            Some(ProposalAction::Accept) => self.accept_ai_proposal(),
            Some(ProposalAction::Discard) => self.ai_chat.proposal = None,
            None => {}
        }
        // A proposal belongs to the document and editor it was made in.
        if self.ai_chat.proposal.as_ref().is_some_and(|proposal| !self.ai_proposal_fits(proposal)) {
            self.ai_chat.proposal = None;
        }
        let valid = self.ai_chat.connection.provider == AiProvider::Copilot || connection::endpoint(&self.ai_chat.connection.endpoint, "models").is_ok();
        if std::mem::take(&mut self.ai_chat.persist) && valid {
            self.settings.ai_chat = self.ai_chat.connection.clone();
            if self.persist_settings {
                self.settings.store_persistent();
            }
        }
        if std::mem::take(&mut self.ai_chat.knowledge_persist) {
            self.settings.ai_chat.knowledge = self.ai_chat.connection.knowledge.clone();
            if self.persist_settings {
                self.settings.store_persistent();
            }
        }
        if !blocked && (focused_before || chat_has_focus(context)) {
            // The editor modes also inspect raw events; chat keystrokes must not edit the canvas.
            context.input_mut(|input| {
                input.events.retain(|event| {
                    !matches!(
                        event,
                        egui::Event::Key { .. } | egui::Event::Text(_) | egui::Event::Paste(_) | egui::Event::Copy | egui::Event::Cut | egui::Event::Ime(_)
                    )
                });
            });
        }
    }

    fn ai_document_id(&self) -> usize {
        std::sync::Arc::as_ptr(&self.document.screen) as *const () as usize
    }

    /// The character-based editor the AI may draw in, if one is open.
    fn ai_drawable_kind(&self) -> Option<&'static str> {
        if self.show_start || self.animation.is_some() || self.font_editor.is_some() || self.rip.is_some() || self.igs.is_some() || self.skypix.is_some() {
            return None;
        }
        Some(if self.charfont.is_some() {
            "text-art font (current glyph)"
        } else if self.atascii.is_some() {
            "ATASCII"
        } else if self.vt52.is_some() {
            "VT52"
        } else if self.petscii.is_some() {
            "PETSCII"
        } else {
            "ANSI/ASCII"
        })
    }

    fn ai_proposal_fits(&self, proposal: &Proposal) -> bool {
        proposal.document == self.ai_document_id()
            && match proposal.workspace {
                Workspace::Canvas(_) => self.ai_drawable_kind().is_some(),
                Workspace::Animation(_) => self.animation.is_some(),
                Workspace::Font(ref draft) => self.font_editor.as_ref().is_some_and(|editor| {
                    (editor.state.font_width().max(0) as usize, editor.state.font_height().max(0) as usize) == (draft.width, draft.height)
                        && editor.state.get_all_glyph_data().len() == draft.glyphs.len()
                }),
            }
    }

    /// The draft the assistant's tools work on: the open proposal, so requests refine it, or a copy
    /// of the open editor's document. `None` for editors without assistant tools.
    fn ai_draft(&self) -> Option<(Workspace, usize)> {
        let document = self.ai_document_id();
        if let Some(proposal) = self.ai_chat.proposal.as_ref().filter(|proposal| self.ai_proposal_fits(proposal)) {
            return Some((proposal.workspace.clone(), document));
        }
        if let Some(editor) = &self.font_editor {
            let state = &editor.state;
            let draft = font_tools::FontDraft::new(
                state.font_name(),
                state.font_width(),
                state.font_height(),
                state.selected_char() as u32,
                state.get_all_glyph_data().clone(),
            );
            return Some((Workspace::Font(draft), document));
        }
        if let Some(editor) = &self.animation {
            let status = editor.mcp_status();
            let file_name = editor
                .path
                .as_ref()
                .and_then(|path| path.file_name())
                .map(|name| name.to_string_lossy().into_owned());
            let draft = animation_tools::AnimationDraft::new(&editor.source, file_name, status.frame_count, status.errors.first().cloned());
            return Some((Workspace::Animation(draft), document));
        }
        let kind = self.ai_drawable_kind()?;
        let draft = self.document.with_state(|state| {
            let selection = state.selection().map(|selection| {
                let bounds = selection.as_rectangle();
                (bounds.left(), bounds.top(), bounds.width(), bounds.height())
            });
            canvas::Draft::new(kind, state.get_buffer().clone(), state.get_current_layer().unwrap_or(0), selection)
        });
        Some((Workspace::Canvas(Box::new(draft)), document))
    }

    /// Applies the assistant's changes as one undo step.
    fn accept_ai_proposal(&mut self) {
        let Some(proposal) = self.ai_chat.proposal.take() else {
            return;
        };
        if !self.ai_proposal_fits(&proposal) {
            return;
        }
        let result = match &proposal.workspace {
            Workspace::Canvas(draft) => self.apply_ai_drawing(draft),
            Workspace::Animation(draft) => match &mut self.animation {
                // Replacing the whole script would silently drop edits made since the proposal.
                Some(editor) if editor.source == draft.original => {
                    let length = editor.source.len();
                    editor.replace_text(0, length, &draft.source)
                }
                _ => Err(fl!("ai-chat-apply-stale")),
            },
            Workspace::Font(draft) => self.apply_ai_glyphs(draft),
        };
        if let Err(error) = result {
            self.ai_chat.error = Some(error);
            self.ai_chat.proposal = Some(proposal);
        }
    }

    fn apply_ai_glyphs(&mut self, draft: &font_tools::FontDraft) -> Result<(), String> {
        let editor = self.font_editor.as_mut().ok_or_else(|| fl!("ai-chat-apply-stale-font"))?;
        let codes = draft.changed_codes();
        let current = editor.state.get_all_glyph_data();
        // Overwriting glyphs edited since the proposal would silently lose that work.
        if codes.iter().any(|&code| current.get(code as usize) != Some(&draft.original[code as usize])) {
            return Err(fl!("ai-chat-apply-stale-font"));
        }
        let glyphs = codes.into_iter().map(|code| (code, draft.glyphs[code as usize].clone())).collect();
        editor.replace_glyphs(glyphs, &fl!("ai-chat-undo"))
    }

    fn apply_ai_drawing(&mut self, draft: &canvas::Draft) -> Result<(), String> {
        let changes = draft.changes();
        self.document.with_state(|state| -> Result<(), String> {
            let buffer = state.get_buffer();
            for (layer, position, _) in &changes {
                let target = buffer.layers.get(*layer).ok_or_else(|| fl!("ai-chat-apply-failed"))?;
                if target.properties.is_locked || position.x >= target.width() || position.y >= target.height() {
                    return Err(fl!("ai-chat-apply-failed"));
                }
            }
            let _undo = state.begin_atomic_undo(fl!("ai-chat-undo"));
            for (layer, position, cell) in changes {
                state.set_char_at_layer_in_atomic(layer, position, cell).map_err(|error| error.to_string())?;
            }
            Ok(())
        })
    }

    fn ai_editor_context(&self) -> String {
        let mut snapshot = Snapshot::default();
        let result = self.write_ai_context(&mut snapshot);
        if result.is_err() {
            snapshot.text.push_str(TRUNCATED);
        }
        snapshot.text
    }

    fn write_ai_context(&self, out: &mut Snapshot) -> fmt::Result {
        if let Some(editor) = &self.animation {
            writeln!(out, "Editor: Lua animation\nSource (not executed):\n{}", editor.source)?;
        } else if let Some(editor) = &self.font_editor {
            let status = editor.mcp_status();
            writeln!(
                out,
                "Editor: bitmap font\nGlyph size: {}x{}\nGlyph count: {}\nSelected character code: {}",
                status.glyph_width, status.glyph_height, status.glyph_count, status.selected_char
            )?;
            if let Some(glyph) = editor.state.get_all_glyph_data().get(status.selected_char as usize) {
                writeln!(out, "Selected glyph bitmap (# = set pixel, . = unset):")?;
                for row in glyph {
                    for pixel in row {
                        out.write_char(if *pixel { '#' } else { '.' })?;
                    }
                    out.write_char('\n')?;
                }
            }
        } else if let Some(editor) = &self.rip {
            writeln!(out, "Editor: RIP\nCommands: {}", editor.document.commands().len())?;
            for (index, command) in editor.document.commands().iter().enumerate() {
                writeln!(out, "{index}: {command:?}")?;
            }
        } else if let Some(editor) = &self.igs {
            writeln!(out, "Editor: IGS\nItems: {}", editor.document.items().len())?;
            for (index, item) in editor.document.items().iter().enumerate() {
                writeln!(out, "{index}: {item:?}")?;
            }
        } else if let Some(editor) = &self.skypix {
            writeln!(out, "Editor: SkyPix\nItems: {}", editor.document.items().len())?;
            for (index, item) in editor.document.items().iter().enumerate() {
                writeln!(out, "{index}: {item:?}")?;
            }
        } else {
            let kind = self.ai_drawable_kind().unwrap_or("ANSI/ASCII");
            writeln!(out, "Editor: {kind}")?;
            if let Some(font) = &self.charfont {
                writeln!(out, "Selected character: {:?}", font.state.selected_char())?;
            }
            self.document.with_state(|state| -> fmt::Result {
                let buffer = state.get_buffer();
                writeln!(
                    out,
                    "Canvas: {}x{} cells\nEncoding: {:?}\nFormat: {:?}\nIce mode: {:?}\nPalette: {:?}",
                    buffer.width(),
                    buffer.height(),
                    buffer.buffer_type,
                    state.get_format_mode(),
                    buffer.ice_mode,
                    buffer.palette
                )?;
                for (slot, font) in buffer.font_iter() {
                    writeln!(out, "Font slot {slot}: {} ({:?})", font.name(), font.size())?;
                }
                writeln!(out, "Caret: {:?}\nSelection: {:?}", state.get_caret().position(), state.selection())?;
                for (index, layer) in buffer.layers.iter().enumerate() {
                    writeln!(
                        out,
                        "Layer {index}: {:?}, offset {:?}, size {}x{}, visible {}, locked {}",
                        layer.properties.title,
                        layer.offset(),
                        layer.width(),
                        layer.height(),
                        layer.is_visible(),
                        layer.properties.is_locked
                    )?;
                }
                let bounds = state.selection().map(|selection| selection.as_rectangle());
                let (left, top, right, bottom) = match bounds {
                    Some(rect) => (
                        rect.left().max(0),
                        rect.top().max(0),
                        rect.right().min(buffer.width()),
                        rect.bottom().min(buffer.height()),
                    ),
                    None => (0, 0, buffer.width(), buffer.height()),
                };
                writeln!(
                    out,
                    "Composite cells in document bounds ({left},{top})..({right},{bottom}), exclusive end.\n\
                     Each run: (x,y) length / native character code / Unicode approximation / attributes."
                )?;
                let mut remaining = MAX_CONTEXT_CELLS;
                for y in top..bottom {
                    let mut x = left;
                    while x < right {
                        if remaining == 0 {
                            writeln!(out, "[Cell snapshot limited to {MAX_CONTEXT_CELLS} cells; remaining cells are not attached.]")?;
                            return Ok(());
                        }
                        let cell = buffer.char_at(Position::new(x, y));
                        let mut end = x + 1;
                        while end < right && ((end - x) as usize) < remaining && buffer.char_at(Position::new(end, y)) == cell {
                            end += 1;
                        }
                        remaining -= (end - x) as usize;
                        writeln!(
                            out,
                            "({x},{y}) length={} code={} {:?} {:?}",
                            end - x,
                            cell.ch as u32,
                            buffer.buffer_type.convert_to_unicode(cell.ch),
                            cell.attribute
                        )?;
                        x = end;
                    }
                }
                Ok(())
            })?;
        }
        Ok(())
    }
}

fn chat_has_focus(context: &egui::Context) -> bool {
    context.memory(|memory| {
        memory.focused().is_some_and(|focused| {
            ["ai-url", "ai-key", "ai-model-id", "ai-prompt", "ai-instructions"]
                .iter()
                .any(|id| focused == egui::Id::new(id))
        })
    })
}

#[derive(Default)]
struct Snapshot {
    text: String,
}

impl Write for Snapshot {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let remaining = (MAX_CONTEXT_BYTES - TRUNCATED.len()).saturating_sub(self.text.len());
        if text.len() <= remaining {
            self.text.push_str(text);
            return Ok(());
        }
        let mut end = remaining;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        self.text.push_str(&text[..end]);
        Err(fmt::Error)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{tests::frame, NewKind};
    use super::*;
    use base64::Engine as _;
    use icy_engine::Size;

    #[tokio::test]
    async fn sending_the_composer_transmits_the_picture_and_retains_it_in_history() {
        let (settings, server) = connection::tests::mock(
            "200 OK",
            r#"{"choices":[{"message":{"content":"Use blue blocks."},"finish_reason":"stop"}]}"#.into(),
        )
        .await;
        let mut chat = Chat::new(settings);
        let image = image_attachment::test_image();
        chat.input = "Interpret as ANSI".into();
        chat.attachment = Some("CP437 80x25".into());
        chat.image = Some(image.clone());
        chat.send(&egui::Context::default(), None);
        assert!(chat.image.is_none());
        assert_eq!(chat.pending.as_ref().unwrap().image, Some(image.clone()));
        let request = tokio::time::timeout(std::time::Duration::from_secs(5), server).await.unwrap().unwrap();
        let body: serde_json::Value = serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(body["messages"][1]["content"][1]["image_url"]["url"], image.data_url());
        assert!(body["messages"][1]["content"][0]["text"].as_str().unwrap().contains("CP437 80x25"));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while chat.job.is_some() {
            assert!(std::time::Instant::now() < deadline);
            chat.poll();
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        }
        assert!(chat.error.is_none(), "{:?}", chat.error);
        assert_eq!(chat.entries[0].image, Some(image));
        assert_eq!(chat.entries[1].text, "Use blue blocks.");
    }

    fn drop_frame(context: &egui::Context, app: &mut DrawApp, position: Option<egui::Pos2>, files: Vec<egui::DroppedFile>, hovered: Vec<egui::HoveredFile>) {
        let _ = context.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 820.0))),
                events: vec![position.map_or(egui::Event::PointerGone, egui::Event::PointerMoved)],
                dropped_files: files,
                hovered_files: hovered,
                time: Some(context.input(|input| input.time) + 0.05),
                ..Default::default()
            },
            |context| app.show(context),
        );
    }

    fn wait_for_image(chat: &mut Chat) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while chat.image_import.is_some() {
            assert!(std::time::Instant::now() < deadline, "picture import did not finish");
            chat.poll();
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }

    fn image_chat_app() -> DrawApp {
        let mut app = DrawApp::new();
        app.ai_chat = Chat::new(AiChatSettings {
            endpoint: "http://127.0.0.1:1/v1".into(),
            model: "vision-model".into(),
            ..Default::default()
        });
        app.ai_chat.visible = true;
        app
    }

    #[test]
    fn picture_drop_attaches_and_previews_without_opening_or_sending() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("reference.png");
        let image = image_attachment::test_image();
        std::fs::write(&path, base64::engine::general_purpose::STANDARD.decode(&*image.data).unwrap()).unwrap();
        let context = egui::Context::default();
        let mut app = image_chat_app();
        app.ai_chat.input = "Use blue shading".into();
        frame(&context, &mut app, egui::vec2(1280.0, 820.0), vec![]);
        let panel = egui::containers::panel::PanelState::load(&context, egui::Id::new("ai-chat")).unwrap().rect;
        let original = app.document.screen.clone();
        let before = app.ai_editor_context();
        drop_frame(
            &context,
            &mut app,
            Some(panel.center()),
            vec![egui::DroppedFile {
                path: Some(path),
                ..Default::default()
            }],
            vec![],
        );
        assert!(std::sync::Arc::ptr_eq(&original, &app.document.screen));
        assert_eq!(app.ai_editor_context(), before);
        assert!(app.ai_chat.job.is_none());
        assert!(!app.ai_chat.send_requested);
        assert_eq!(app.ai_chat.input, "Use blue shading");
        wait_for_image(&mut app.ai_chat);
        assert!(app.ai_chat.error.is_none(), "{:?}", app.ai_chat.error);
        assert_eq!(app.ai_chat.image, Some(image));
        frame(&context, &mut app, egui::vec2(1280.0, 820.0), vec![]);
        assert!(app.ai_chat.image_texture.is_some());
        assert!(app.ai_chat.entries.is_empty());
    }

    #[test]
    fn drops_outside_the_chat_keep_normal_document_opening() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("drawing.ans");
        std::fs::write(&path, b"\x1b[34mNEW ART").unwrap();
        let context = egui::Context::default();
        let mut app = image_chat_app();
        frame(&context, &mut app, egui::vec2(1280.0, 820.0), vec![]);
        let panel = egui::containers::panel::PanelState::load(&context, egui::Id::new("ai-chat")).unwrap().rect;
        drop_frame(
            &context,
            &mut app,
            Some(egui::pos2(panel.left() - 20.0, panel.center().y)),
            vec![egui::DroppedFile {
                path: Some(path.clone()),
                ..Default::default()
            }],
            vec![],
        );
        assert_eq!(app.document.path.as_ref(), Some(&path));
        assert!(app.ai_chat.image.is_none());
        assert!(app.ai_chat.image_import.is_none());
    }

    #[test]
    fn pointer_gone_drop_uses_the_previous_image_hover_target() {
        let context = egui::Context::default();
        let mut app = image_chat_app();
        frame(&context, &mut app, egui::vec2(1280.0, 820.0), vec![]);
        let panel = egui::containers::panel::PanelState::load(&context, egui::Id::new("ai-chat")).unwrap().rect;
        drop_frame(
            &context,
            &mut app,
            Some(panel.center()),
            vec![],
            vec![egui::HoveredFile {
                mime: "image/png".into(),
                ..Default::default()
            }],
        );
        assert!(app.ai_chat.drop_hovered);
        let image = image_attachment::test_image();
        let bytes = base64::engine::general_purpose::STANDARD.decode(&*image.data).unwrap();
        drop_frame(
            &context,
            &mut app,
            None,
            vec![egui::DroppedFile {
                name: image.name.clone(),
                bytes: Some(bytes.into()),
                ..Default::default()
            }],
            vec![],
        );
        wait_for_image(&mut app.ai_chat);
        assert_eq!(app.ai_chat.image, Some(image));
        assert!(!app.ai_chat.drop_hovered);
    }

    #[test]
    fn invalid_multiple_and_replacement_drops_do_not_change_the_document() {
        let context = egui::Context::default();
        let mut app = image_chat_app();
        frame(&context, &mut app, egui::vec2(1280.0, 820.0), vec![]);
        let panel = egui::containers::panel::PanelState::load(&context, egui::Id::new("ai-chat")).unwrap().rect;
        let before = app.ai_editor_context();
        app.ai_chat.input = "keep this prompt".into();
        let invalid = egui::DroppedFile {
            name: "bad.png".into(),
            bytes: Some(b"invalid picture".to_vec().into()),
            ..Default::default()
        };
        drop_frame(&context, &mut app, Some(panel.center()), vec![invalid.clone()], vec![]);
        wait_for_image(&mut app.ai_chat);
        assert!(app.ai_chat.error.is_some());
        assert!(app.ai_chat.image.is_none());
        drop_frame(&context, &mut app, Some(panel.center()), vec![invalid.clone(), invalid.clone()], vec![]);
        assert!(app.ai_chat.error.is_some());
        assert!(app.ai_chat.image_import.is_none());
        let image = image_attachment::test_image();
        app.ai_chat.image = Some(image.clone());
        drop_frame(&context, &mut app, Some(panel.center()), vec![invalid], vec![]);
        assert_eq!(app.ai_chat.image, Some(image));
        assert!(app.ai_chat.image_import.is_none());
        assert_eq!(app.ai_chat.input, "keep this prompt");
        assert_eq!(app.ai_editor_context(), before);
        assert!(app.ai_chat.job.is_none());
    }

    #[test]
    fn cancelled_and_failed_requests_restore_images_and_new_chat_discards_them() {
        let mut chat = Chat::new(AiChatSettings::default());
        let image = image_attachment::test_image();
        for failed in [false, true] {
            chat.pending = Some(Entry {
                user: true,
                text: "Draw from this".into(),
                attachment: Some("editor snapshot".into()),
                image: Some(image.clone()),
            });
            if failed {
                chat.handle(Err("The selected model rejected image input".into()));
            } else {
                chat.cancel();
            }
            assert_eq!(chat.image, Some(image.clone()));
            assert_eq!(chat.input, "Draw from this");
            assert_eq!(chat.attachment.as_deref(), Some("editor snapshot"));
            chat.clear();
        }
        chat.pending = Some(Entry {
            user: true,
            text: "Picture".into(),
            attachment: None,
            image: Some(image.clone()),
        });
        chat.push_reply("Draft ready".into());
        assert_eq!(chat.entries[0].message().image, Some(image));
        chat.disconnect();
        assert!(chat.entries.is_empty());
        assert!(chat.image.is_none());
        assert!(chat.image_texture.is_none());
        assert!(chat.image_import.is_none());
    }

    const MODES: [(NewKind, &str); 10] = [
        (NewKind::Ansi, "ANSI/ASCII"),
        (NewKind::Atascii, "ATASCII"),
        (NewKind::Vt52, "VT52"),
        (NewKind::Petscii, "PETSCII"),
        (NewKind::TheDraw, "text-art font"),
        (NewKind::BitmapFont, "bitmap font"),
        (NewKind::Animation, "Lua animation"),
        (NewKind::Rip, "RIP"),
        (NewKind::Igs, "IGS"),
        (NewKind::Skypix, "SkyPix"),
    ];

    #[test]
    fn all_editor_modes_have_read_only_context_and_a_chat_panel() {
        let mut app = DrawApp::new();
        app.ai_chat.visible = true;
        for (kind, label) in MODES {
            app.create(kind, Size::new(80, 25));
            let context = egui::Context::default();
            let before = app.modified();
            let snapshot = app.ai_editor_context();
            assert!(snapshot.starts_with(&format!("Editor: {label}")), "{label}: {snapshot}");
            assert!(snapshot.len() <= MAX_CONTEXT_BYTES);
            assert_eq!(app.modified(), before, "{label}");
            assert!(app.ai_chat.attachment.is_none());
            frame(&context, &mut app, egui::vec2(1280.0, 820.0), vec![]);
            assert!(
                egui::containers::panel::PanelState::load(&context, egui::Id::new("ai-chat")).is_some(),
                "{label}"
            );
        }
    }

    #[test]
    fn chat_typing_does_not_change_any_editor() {
        let mut app = DrawApp::new();
        app.ai_chat.visible = true;
        app.ai_chat.connection.endpoint = "http://localhost:1234/v1".into();
        app.ai_chat.settings_open = false;
        for (kind, label) in MODES {
            app.create(kind, Size::new(80, 25));
            app.ai_chat.clear();
            let context = egui::Context::default();
            let size = egui::vec2(1280.0, 820.0);
            frame(&context, &mut app, size, vec![]);
            context.memory_mut(|memory| memory.request_focus(egui::Id::new("ai-prompt")));
            let before = app.ai_editor_context();
            frame(
                &context,
                &mut app,
                size,
                vec![
                    egui::Event::Text("hello".into()),
                    egui::Event::Key {
                        key: egui::Key::F5,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
            assert_eq!(app.ai_chat.input, "hello", "{label}");
            assert_eq!(app.ai_editor_context(), before, "{label}");
        }
    }

    #[test]
    fn snapshot_is_bounded_utf8_and_selection_scoped() {
        let mut app = DrawApp::new();
        app.document.path = Some("/private/path/not-for-the-model.icy".into());
        app.document.type_text("ABC").unwrap();
        app.document
            .with_state(|state| state.set_selection(icy_engine::Rectangle::from(1, 0, 1, 1)).unwrap());
        let snapshot = app.ai_editor_context();
        assert!(snapshot.contains("code=66"));
        assert!(!snapshot.contains("code=65"));
        assert!(!snapshot.contains("/private"));
        assert!(!snapshot.contains(TRUNCATED));

        app.create(NewKind::Animation, Size::new(80, 25));
        app.animation.as_mut().unwrap().source = "é".repeat(MAX_CONTEXT_BYTES);
        let snapshot = app.ai_editor_context();
        assert!(snapshot.len() <= MAX_CONTEXT_BYTES);
        assert!(snapshot.ends_with(TRUNCATED));

        app.create(NewKind::Ansi, Size::new(100_000, 1));
        let snapshot = app.ai_editor_context();
        assert!(snapshot.contains("[Cell snapshot limited to 16384 cells"));
        assert!(snapshot.len() <= MAX_CONTEXT_BYTES);
    }

    #[test]
    fn compact_panel_keeps_history_and_composer_inside_the_window() {
        let mut app = DrawApp::new();
        app.ai_chat.visible = true;
        app.ai_chat.input = "A question".into();
        let context = egui::Context::default();
        for _ in 0..3 {
            frame(&context, &mut app, egui::vec2(640.0, 480.0), vec![]);
        }
        let panel = egui::containers::panel::PanelState::load(&context, egui::Id::new("ai-chat")).unwrap();
        assert!(panel.rect.bottom() <= 480.0);
        assert!(panel.rect.width() <= 640.0);
    }

    #[test]
    fn new_chat_removes_context_history_and_draft_but_keeps_connection() {
        let mut chat = Chat::new(AiChatSettings {
            endpoint: "http://localhost:1234/v1".into(),
            model: "local".into(),
            ..Default::default()
        });
        chat.key = "test-only-key".into();
        chat.attachment = Some("snapshot".into());
        chat.input = "draft".into();
        chat.entries.push(Entry {
            user: true,
            text: "previous".into(),
            attachment: None,
            image: None,
        });
        chat.clear();
        assert!(chat.entries.is_empty());
        assert!(chat.input.is_empty());
        assert!(chat.attachment.is_none());
        assert_eq!(chat.connection.model, "local");
        assert_eq!(chat.key, "test-only-key");
    }

    #[test]
    fn connecting_collapses_settings_and_persists_the_connection() {
        let mut chat = Chat::new(AiChatSettings::default());
        assert!(chat.settings_open, "an unconfigured assistant starts in its settings");
        chat.connection.endpoint = "http://localhost:1234/v1".into();
        chat.handle(Ok(Response::Models {
            models: vec!["only-model".into()],
            account: None,
        }));
        assert!(!chat.settings_open);
        assert!(chat.connected);
        assert!(chat.persist);
        assert_eq!(chat.connection.model, "only-model");

        let mut chat = Chat::new(AiChatSettings::default());
        chat.handle(Err("unauthorized".into()));
        assert!(chat.settings_open, "a failed connection keeps the settings open");
        assert_eq!(chat.error.as_deref(), Some("unauthorized"));
    }

    #[test]
    fn copilot_progress_is_visible_and_cancellation_keeps_the_live_font_unchanged() {
        let mut app = DrawApp::new();
        app.create(NewKind::BitmapFont, Size::new(80, 25));
        let original = app.font_editor.as_ref().unwrap().state.get_all_glyph_data().clone();
        let (_sender, receiver) = std::sync::mpsc::channel();
        let (cancel, mut cancelled) = tokio::sync::oneshot::channel();
        let progress = std::sync::Arc::new(parking_lot::Mutex::new(connection::Progress {
            phase: connection::ProgressPhase::Generating,
            tool_calls: 4,
            last_tool: Some("icy_write_glyphs".into()),
            changed_glyphs: Some(192),
            ..Default::default()
        }));
        app.ai_chat.job = Some(Job::from_parts(receiver, cancel).with_progress(progress));
        app.ai_chat.pending = Some(Entry {
            user: true,
            text: "Design an RPG font".into(),
            attachment: None,
            image: None,
        });
        app.ai_chat.visible = true;
        app.ai_chat.settings_open = false;
        app.ai_chat.auto_connected = true;
        let context = egui::Context::default();
        let size = egui::vec2(1280.0, 820.0);
        frame(&context, &mut app, size, vec![]);
        let output = frame(&context, &mut app, size, vec![]);
        let text: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text.galley.text()),
                _ => None,
            })
            .collect();
        for expected in ["Generating response", "4 tool calls", "icy_write_glyphs", "192 glyphs changed in draft"] {
            assert!(text.iter().any(|text| text.contains(expected)), "missing {expected:?} in {text:?}");
        }
        app.ai_chat.cancel();
        assert_eq!(cancelled.try_recv(), Ok(()));
        assert_eq!(app.ai_chat.input, "Design an RPG font");
        assert!(app.ai_chat.job.is_none());
        assert!(app.ai_chat.pending.is_none());
        assert_eq!(app.font_editor.as_ref().unwrap().state.get_all_glyph_data(), &original);
    }

    #[test]
    fn knowledge_errors_preserve_the_composer_and_do_not_start_a_request() {
        let mut chat = Chat::new(AiChatSettings::default());
        chat.input = "Draw a menu".into();
        chat.attachment = Some("editor snapshot".into());
        chat.connection.knowledge.references = vec!["unknown".into()];
        chat.send(&egui::Context::default(), None);
        assert!(chat.error.as_ref().unwrap().contains("Unknown"));
        assert_eq!(chat.input, "Draw a menu");
        assert_eq!(chat.attachment.as_deref(), Some("editor snapshot"));
        assert!(chat.pending.is_none());
        assert!(chat.job.is_none());
    }

    #[test]
    fn custom_instruction_editor_accepts_multiline_and_marks_preferences_dirty() {
        let context = egui::Context::default();
        let mut chat = Chat::new(AiChatSettings::default());
        let header = egui::pos2(50.0, 18.0);
        for events in [
            vec![],
            vec![
                egui::Event::PointerMoved(header),
                egui::Event::PointerButton {
                    pos: header,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerButton {
                    pos: header,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            vec![],
            vec![egui::Event::Paste("Use blue.\nKeep hotkeys white.".into())],
        ] {
            let _ = context.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0))),
                    events,
                    ..Default::default()
                },
                |context| {
                    egui::CentralPanel::default().show(context, |ui| {
                        chat.knowledge_settings(ui);
                    });
                },
            );
            context.memory_mut(|memory| memory.request_focus(egui::Id::new("ai-instructions")));
        }
        assert_eq!(chat.connection.knowledge.custom_instructions, "Use blue.\nKeep hotkeys white.");
        assert!(chat.knowledge_persist);
        assert!(chat_has_focus(&context));
        assert!(!chat.send_requested);
    }

    #[test]
    fn knowledge_persists_without_a_configured_endpoint_and_survives_new_chat() {
        let mut app = DrawApp::new();
        app.ai_chat = Chat::new(AiChatSettings::default());
        app.ai_chat.visible = true;
        app.ai_chat.connection.knowledge.custom_instructions = "Use blue accents.\nKeep hotkeys white.".into();
        app.ai_chat.connection.knowledge.presets = vec!["bbs-menu".into()];
        app.ai_chat.knowledge_persist = true;
        let context = egui::Context::default();
        frame(&context, &mut app, egui::vec2(800.0, 600.0), vec![]);
        assert_eq!(app.settings.ai_chat.knowledge, app.ai_chat.connection.knowledge);
        app.ai_chat.clear();
        assert_eq!(app.ai_chat.connection.knowledge.presets, ["bbs-menu"]);
        assert!(app.ai_chat.connection.knowledge.custom_instructions.contains('\n'));
    }

    #[test]
    fn providers_keep_separate_models_and_switching_clears_the_conversation() {
        let mut chat = Chat::new(AiChatSettings {
            endpoint: "http://localhost:1234/v1".into(),
            model: "local".into(),
            copilot_model: "gpt-5-mini".into(),
            ..Default::default()
        });
        assert_eq!(chat.model(), "local");
        chat.entries.push(Entry {
            user: true,
            text: "private".into(),
            attachment: None,
            image: None,
        });
        chat.models = vec!["local".into()];
        chat.connected = true;
        let conversation = chat.conversation;
        chat.disconnect();
        chat.connection.provider = AiProvider::Copilot;
        assert!(chat.entries.is_empty() && chat.models.is_empty() && !chat.connected);
        assert!(chat.copilot.is_none());
        assert_ne!(chat.conversation, conversation, "Copilot must not continue the old session");
        assert_eq!(chat.model(), "gpt-5-mini");
        assert!(!chat.needs_setup(), "Copilot needs no URL");
        chat.handle(Ok(Response::Models {
            models: vec!["gpt-5-mini".into(), "claude".into()],
            account: Some("octocat".into()),
        }));
        assert_eq!(chat.account.as_deref(), Some("octocat"));
        assert_eq!(chat.connection.model, "local", "the OpenAI-compatible model is kept");
    }

    fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    #[test]
    fn enter_sends_shift_enter_breaks_lines_and_stop_restores_the_draft() {
        let mut app = DrawApp::new();
        app.ai_chat = Chat::new(AiChatSettings {
            endpoint: "http://127.0.0.1:9/v1".into(),
            model: "test-model".into(),
            ..Default::default()
        });
        app.ai_chat.visible = true;
        let context = egui::Context::default();
        let size = egui::vec2(1280.0, 820.0);
        frame(&context, &mut app, size, vec![]);
        context.memory_mut(|memory| memory.request_focus(egui::Id::new("ai-prompt")));
        frame(&context, &mut app, size, vec![egui::Event::Text("line".into())]);
        frame(&context, &mut app, size, vec![key(egui::Key::Enter, egui::Modifiers::SHIFT)]);
        assert!(app.ai_chat.pending.is_none());
        assert!(app.ai_chat.input.contains('\n'), "{:?}", app.ai_chat.input);
        frame(&context, &mut app, size, vec![egui::Event::Text("two".into())]);
        frame(&context, &mut app, size, vec![key(egui::Key::Enter, egui::Modifiers::NONE)]);
        assert_eq!(app.ai_chat.pending.as_ref().map(|entry| entry.text.as_str()), Some("line\ntwo"));
        assert!(app.ai_chat.input.is_empty());
        app.ai_chat.cancel();
        assert_eq!(app.ai_chat.input, "line\ntwo");
        assert!(app.ai_chat.job.is_none());
    }

    #[test]
    fn markdown_inline_formatting_drops_markers() {
        let context = egui::Context::default();
        let _ = context.run(Default::default(), |context| {
            egui::CentralPanel::default().show(context, |ui| {
                let job = inline(ui, "Use **bold** and `code` here");
                assert_eq!(job.text, "Use bold and code here");
                assert_eq!(job.sections.len(), 5);
            });
        });
    }

    #[test]
    #[ignore = "requires a working wgpu adapter"]
    fn gpu_chat_panel_renders() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let mut gpu = super::super::tests::Gpu::new().await;
            let mut app = DrawApp::new();
            app.ai_chat = Chat::new(AiChatSettings::default());
            app.ai_chat.visible = true;
            for name in ["chat-settings-warmup", "chat-settings"] {
                gpu.capture(&mut app, [1280, 820], 1.0, vec![], name);
            }
            app.ai_chat.connection = AiChatSettings {
                endpoint: "https://api.openai.com/v1".into(),
                model: "chat-latest".into(),
                ..Default::default()
            };
            app.ai_chat.models = vec!["chat-latest".into(), "gpt-mini".into()];
            app.ai_chat.settings_open = false;
            for name in ["chat-empty-warmup", "chat-empty"] {
                gpu.capture(&mut app, [1280, 820], 1.0, vec![], name);
            }
            app.ai_chat.entries = vec![
                Entry {
                    user: true,
                    text: "How can I make this logo look more metallic?".into(),
                    attachment: Some("snapshot".into()),
                    image: None,
                },
                Entry {
                    user: false,
                    text: "## Metallic shading\n\nUse a **vertical ramp** from bright to dark:\n\n- Top rows: `0xDB` in white\n- Middle: light gray with `0xB2`\n- Bottom: dark gray `0xB1`\n\n```\n\u{2588}\u{2593}\u{2592}\u{2591}\n```\nThat gives the classic chrome look.".into(),
                    attachment: None,
                    image: None,
                },
                Entry {
                    user: true,
                    text: "Thanks!".into(),
                    attachment: None,
                    image: None,
                },
            ];
            app.ai_chat.attachment = Some("x".repeat(3000));
            app.ai_chat.input = "And for the shadow?".into();
            for name in ["chat-conversation-warmup", "chat-conversation"] {
                gpu.capture(&mut app, [1280, 820], 1.0, vec![], name);
            }
            app.ai_chat.connection.provider = AiProvider::Copilot;
            app.ai_chat.auto_connected = true;
            app.ai_chat.connected = true;
            app.ai_chat.account = Some("octocat".into());
            app.ai_chat.settings_open = true;
            for name in ["chat-copilot-warmup", "chat-copilot"] {
                gpu.capture(&mut app, [1280, 820], 1.0, vec![], name);
            }
        });
    }

    /// Simulates Copilot drawing on the draft the app would send.
    fn propose(app: &mut DrawApp, text: &str) {
        let (mut workspace, document) = app.ai_draft().expect("drawable editor");
        workspace
            .call("icy_draw_text", &serde_json::json!({ "x": 0, "y": 0, "text": text, "fg": 14 }))
            .unwrap();
        app.ai_chat.pending_document = document;
        app.ai_chat.handle(Ok(Response::Proposal("Drawn".into(), Box::new(workspace))));
    }

    #[test]
    fn proposals_apply_as_one_undo_step_and_refine_the_open_draft() {
        let mut app = DrawApp::new();
        propose(&mut app, "HOUSE");
        assert_eq!(app.ai_chat.proposal.as_ref().unwrap().changes, 5);
        assert!(!app.document.modified(), "a proposal does not touch the document");
        // A follow-up request starts from the proposal, not from the unchanged document.
        let Some((Workspace::Canvas(draft), _)) = app.ai_draft() else {
            panic!("expected a canvas draft");
        };
        assert_eq!(draft.buffer.layers[0].char_at(Position::new(0, 0)).ch, 'H');
        propose(&mut app, "MOUSE");
        app.ai_chat.proposal_action = Some(ProposalAction::Accept);
        app.ai_chat.visible = true;
        frame(&egui::Context::default(), &mut app, egui::vec2(1280.0, 820.0), vec![]);
        assert!(app.ai_chat.proposal.is_none());
        let row: String = (0..5)
            .map(|x| app.document.with_state(|state| state.get_buffer().char_at(Position::new(x, 0)).ch))
            .collect();
        assert_eq!(row, "MOUSE");
        app.document.undo().unwrap();
        assert_eq!(app.document.with_state(|state| state.get_buffer().char_at(Position::new(0, 0)).ch), ' ');
        assert!(!app.document.modified(), "one undo removes the whole drawing");
    }

    #[test]
    fn proposals_are_discarded_explicitly_or_when_the_document_changes() {
        let mut app = DrawApp::new();
        app.ai_chat.visible = true;
        app.ai_chat.settings_open = false;
        let context = egui::Context::default();
        let size = egui::vec2(1280.0, 820.0);
        propose(&mut app, "X");
        frame(&context, &mut app, size, vec![]);
        assert!(app.ai_chat.proposal.as_ref().unwrap().preview.is_some(), "the card renders a preview");
        app.ai_chat.proposal_action = Some(ProposalAction::Discard);
        frame(&context, &mut app, size, vec![]);
        assert!(app.ai_chat.proposal.is_none());
        assert!(!app.document.modified());

        propose(&mut app, "X");
        app.create(NewKind::Ansi, Size::new(80, 25));
        frame(&context, &mut app, size, vec![]);
        assert!(app.ai_chat.proposal.is_none(), "a proposal never lands in another document");

        for kind in [NewKind::Rip, NewKind::Igs, NewKind::Skypix] {
            app.create(kind, Size::new(80, 25));
            assert!(app.ai_draft().is_none());
        }
        app.create(NewKind::Animation, Size::new(80, 25));
        assert!(matches!(app.ai_draft(), Some((Workspace::Animation(_), _))));
        for kind in [NewKind::Ansi, NewKind::Atascii, NewKind::Vt52, NewKind::Petscii, NewKind::TheDraw] {
            app.create(kind, Size::new(80, 25));
            assert!(app.ai_draft().is_some());
        }
    }

    #[test]
    fn locked_layers_reject_the_proposal_and_keep_it() {
        let mut app = DrawApp::new();
        propose(&mut app, "X");
        app.document.with_state(|state| state.get_buffer_mut().layers[0].properties.is_locked = true);
        app.accept_ai_proposal();
        assert!(app.ai_chat.proposal.is_some());
        assert!(app.ai_chat.error.is_some());
        assert!(!app.document.modified());
    }

    fn propose_script(app: &mut DrawApp, text: &str) {
        let (mut workspace, document) = app.ai_draft().expect("animation editor");
        workspace.call("icy_write_source", &serde_json::json!({ "text": text })).unwrap();
        app.ai_chat.pending_document = document;
        app.ai_chat.handle(Ok(Response::Proposal("Edited".into(), Box::new(workspace))));
    }

    #[test]
    fn script_proposals_show_a_diff_apply_as_one_undo_step_and_refuse_stale_scripts() {
        const ORIGINAL: &str = "local buf = new_buffer(80, 25)\nnext_frame(buf)";
        let mut app = DrawApp::new();
        app.create(NewKind::Animation, Size::new(80, 25));
        app.animation.as_mut().unwrap().source = ORIGINAL.into();
        propose_script(&mut app, "local buf = new_buffer(80, 25)\nbuf:print(\"Hi\")\nnext_frame(buf)");
        let diff = app.ai_chat.proposal.as_ref().unwrap().script_diff.as_ref().unwrap();
        assert_eq!((diff.line, diff.removed, diff.added), (2, 0, 1));
        assert_eq!(app.animation.as_ref().unwrap().source, ORIGINAL, "nothing is applied before accepting");

        app.ai_chat.visible = true;
        app.ai_chat.settings_open = false;
        frame(&egui::Context::default(), &mut app, egui::vec2(1280.0, 820.0), vec![]);
        assert!(app.ai_chat.proposal.is_some(), "the diff card renders and the proposal stays");
        app.accept_ai_proposal();
        assert!(app.ai_chat.proposal.is_none());
        assert!(app.animation.as_ref().unwrap().source.contains("buf:print"));
        app.animation.as_mut().unwrap().undo_source(false);
        assert_eq!(app.animation.as_ref().unwrap().source, ORIGINAL);

        propose_script(&mut app, "-- new");
        app.animation.as_mut().unwrap().source.push_str("\n-- typed by the user");
        app.accept_ai_proposal();
        assert!(app.ai_chat.proposal.is_some(), "a stale proposal is kept for discarding");
        assert!(app.ai_chat.error.is_some());
        assert!(app.animation.as_ref().unwrap().source.ends_with("-- typed by the user"));
    }

    #[test]
    fn font_preview_pages_render_later_glyphs_and_refresh_the_cached_texture() {
        let mut app = DrawApp::new();
        app.create(NewKind::BitmapFont, Size::new(80, 25));
        let (mut workspace, document) = app.ai_draft().unwrap();
        workspace.call("icy_transform_glyphs", &serde_json::json!({"operation": "invert"})).unwrap();
        app.ai_chat.pending_document = document;
        app.ai_chat.handle(Ok(Response::Proposal("Full font".into(), Box::new(workspace))));
        let pages = app.ai_chat.proposal.as_ref().unwrap().changes.div_ceil(FONT_PREVIEW_GLYPHS);
        assert!(pages > 1);
        app.ai_chat.visible = true;
        app.ai_chat.settings_open = false;
        let context = egui::Context::default();
        let size = egui::vec2(1280.0, 820.0);
        frame(&context, &mut app, size, vec![]);
        for page in 1..pages {
            let output = frame(&context, &mut app, size, vec![]);
            let old_texture = app.ai_chat.proposal.as_ref().unwrap().preview.as_ref().unwrap().id();
            let position = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == ">" => Some(text.pos + text.galley.size() / 2.0),
                    _ => None,
                })
                .expect("next-page button is visible");
            frame(
                &context,
                &mut app,
                size,
                vec![
                    egui::Event::PointerMoved(position),
                    egui::Event::PointerButton {
                        pos: position,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                    egui::Event::PointerButton {
                        pos: position,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
            let proposal = app.ai_chat.proposal.as_ref().unwrap();
            assert_eq!(proposal.glyph_page, page);
            assert_ne!(
                proposal.preview.as_ref().unwrap().id(),
                old_texture,
                "a changed page must invalidate its texture"
            );
        }
        let mut partial = font_tools::FontDraft::new("Partial", 3, 2, 0, vec![vec![vec![false; 3]; 2]; 130]);
        partial.call("icy_transform_glyphs", &serde_json::json!({"operation": "invert"})).unwrap();
        assert_eq!(
            render_glyphs(&context, &partial, 2).size(),
            [12, 10],
            "the final partial page shows glyphs 128 and 129"
        );
    }

    #[test]
    fn full_font_proposals_apply_and_undo_atomically_and_reject_one_stale_glyph() {
        use icy_engine_edit::bitfont::BitFontUndoState;
        let mut app = DrawApp::new();
        app.create(NewKind::BitmapFont, Size::new(80, 25));
        let original = app.font_editor.as_ref().unwrap().state.get_all_glyph_data().clone();
        assert!(original.len() > FONT_PREVIEW_GLYPHS);
        let (mut workspace, document) = app.ai_draft().unwrap();
        workspace.call("icy_transform_glyphs", &serde_json::json!({"operation": "invert"})).unwrap();
        let Workspace::Font(draft) = &workspace else {
            panic!("expected a font draft")
        };
        let expected = draft.glyphs.clone();
        app.ai_chat.pending_document = document;
        app.ai_chat.handle(Ok(Response::Proposal("Full font".into(), Box::new(workspace))));
        assert_eq!(app.ai_chat.proposal.as_ref().unwrap().changes, original.len());
        assert_eq!(app.font_editor.as_ref().unwrap().state.get_all_glyph_data(), &original);
        app.accept_ai_proposal();
        assert!(app.ai_chat.proposal.is_none());
        let editor = app.font_editor.as_mut().unwrap();
        assert_eq!(editor.state.get_all_glyph_data(), &expected);
        editor.state.undo().unwrap();
        assert_eq!(editor.state.get_all_glyph_data(), &original, "one undo restores every glyph");

        let (mut workspace, document) = app.ai_draft().unwrap();
        workspace.call("icy_transform_glyphs", &serde_json::json!({"operation": "invert"})).unwrap();
        app.ai_chat.pending_document = document;
        app.ai_chat.handle(Ok(Response::Proposal("Full font".into(), Box::new(workspace))));
        let last = char::from_u32((original.len() - 1) as u32).unwrap();
        app.font_editor
            .as_mut()
            .unwrap()
            .state
            .set_pixel(last, 0, 0, !original[original.len() - 1][0][0])
            .unwrap();
        let before_accept = app.font_editor.as_ref().unwrap().state.get_all_glyph_data().clone();
        app.accept_ai_proposal();
        assert!(app.ai_chat.proposal.is_some());
        assert!(app.ai_chat.error.is_some());
        assert_eq!(
            app.font_editor.as_ref().unwrap().state.get_all_glyph_data(),
            &before_accept,
            "a stale final glyph prevents even earlier glyphs from being applied"
        );
    }

    #[test]
    fn hex_batch_proposals_keep_the_live_font_untouched_until_apply_and_undo_together() {
        use icy_engine_edit::bitfont::BitFontUndoState;
        let mut app = DrawApp::new();
        app.create(NewKind::BitmapFont, Size::new(80, 25));
        let original = app.font_editor.as_ref().unwrap().state.get_all_glyph_data().clone();
        let (mut workspace, document) = app.ai_draft().unwrap();
        let mut reference = workspace.clone();
        reference
            .call("icy_transform_glyphs", &serde_json::json!({"operation": "invert", "codes": [65, 66]}))
            .unwrap();
        let read: serde_json::Value = serde_json::from_str(
            &reference
                .call("icy_read_glyphs", &serde_json::json!({"format": "hex", "codes": [65, 66]}))
                .unwrap(),
        )
        .unwrap();
        let Workspace::Font(expected) = reference else {
            panic!("expected a font draft")
        };
        workspace
            .call(
                "icy_write_glyphs",
                &serde_json::json!({
                    "format": "hex", "glyphs": read["glyphs"],
                }),
            )
            .unwrap();
        app.ai_chat.pending_document = document;
        app.ai_chat.handle(Ok(Response::Proposal("Batch".into(), Box::new(workspace))));
        assert_eq!(app.font_editor.as_ref().unwrap().state.get_all_glyph_data(), &original);
        app.accept_ai_proposal();
        assert!(app.ai_chat.proposal.is_none());
        let editor = app.font_editor.as_mut().unwrap();
        assert_eq!(editor.state.get_all_glyph_data(), &expected.glyphs);
        editor.state.undo().unwrap();
        assert_eq!(editor.state.get_all_glyph_data(), &original);
    }

    #[test]
    fn glyph_proposals_preview_apply_as_one_undo_step_and_refuse_stale_glyphs() {
        use icy_engine_edit::bitfont::BitFontUndoState;
        let mut app = DrawApp::new();
        app.create(NewKind::BitmapFont, Size::new(80, 25));
        let (mut workspace, document) = app.ai_draft().expect("bitmap font editor");
        let Workspace::Font(draft) = &workspace else {
            panic!("expected a font draft");
        };
        let (width, height) = (draft.width, draft.height);
        let original = draft.glyphs[65].clone();
        let rows: Vec<String> = (0..height).map(|_| "#".repeat(width)).collect();
        workspace.call("icy_write_glyph", &serde_json::json!({ "code": "A", "rows": rows })).unwrap();
        app.ai_chat.pending_document = document;
        app.ai_chat.handle(Ok(Response::Proposal("Glyph".into(), Box::new(workspace))));
        assert_eq!(app.ai_chat.proposal.as_ref().unwrap().changes, 1);

        app.ai_chat.visible = true;
        app.ai_chat.settings_open = false;
        frame(&egui::Context::default(), &mut app, egui::vec2(1280.0, 820.0), vec![]);
        assert!(app.ai_chat.proposal.as_ref().unwrap().preview.is_some(), "the card renders the glyphs");
        assert_eq!(app.font_editor.as_ref().unwrap().state.get_glyph_pixels('A'), &original, "nothing applied yet");
        app.accept_ai_proposal();
        let editor = app.font_editor.as_mut().unwrap();
        assert!(editor.state.get_glyph_pixels('A').iter().flatten().all(|&set| set));
        editor.state.undo().unwrap();
        assert_eq!(editor.state.get_glyph_pixels('A'), &original, "one undo restores the glyph");

        let (mut workspace, document) = app.ai_draft().unwrap();
        let rows: Vec<String> = (0..height).map(|_| ".".repeat(width)).collect();
        workspace.call("icy_write_glyph", &serde_json::json!({ "code": "A", "rows": rows })).unwrap();
        app.ai_chat.pending_document = document;
        app.ai_chat.handle(Ok(Response::Proposal("Glyph".into(), Box::new(workspace))));
        app.font_editor.as_mut().unwrap().state.set_pixel('A', 0, 0, !original[0][0]).unwrap();
        app.accept_ai_proposal();
        assert!(app.ai_chat.proposal.is_some(), "a stale proposal is kept for discarding");
        assert!(app.ai_chat.error.is_some());
    }
}
