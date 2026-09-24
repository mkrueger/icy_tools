//! Shared dialog shell, so every dialog of the egui front ends looks and behaves the same.
//!
//! - One frame, optional header for messages and help, body and button row.
//! - Buttons are listed in visual order. [`DialogButton::leading`] buttons sit at the left edge,
//!   the others at the right edge, so the primary button ends up last, like on macOS.
//! - Escape and the optional header's close button trigger the cancel button ([`DialogButton::cancel`] or
//!   [`DialogButton::cancels`]); without one the dialog reports [`DialogResponse::dismissed`].
//! - With [`Dialog::confirm_on_enter`], Enter triggers the enabled primary button unless a widget
//!   keeps the keyboard focus (a multi-line editor, a focused button, ...).
//! - Nested dialogs only react to the keyboard while they are the topmost modal.

use ::egui::{self, Color32, FontId};

use super::appearance::{self, PRIMARY};

pub const DIALOG_PADDING: i8 = 16;
/// Buttons in a dialog's button row are at least this wide, so short labels like "OK" line up.
pub const BUTTON_MIN_WIDTH: f32 = 80.0;
pub const DANGER: Color32 = Color32::from_rgb(192, 57, 43);
pub const WARNING: Color32 = Color32::from_rgb(214, 150, 20);

/// Standard labels, so the same action is named the same everywhere.
pub mod labels {
    use crate::LANGUAGE_LOADER;
    use i18n_embed_fl::fl;

    pub fn ok() -> String {
        fl!(LANGUAGE_LOADER, "dialog-ok-button")
    }
    pub fn cancel() -> String {
        fl!(LANGUAGE_LOADER, "dialog-cancel-button")
    }
    pub fn close() -> String {
        fl!(LANGUAGE_LOADER, "dialog-close-button")
    }
    pub fn yes() -> String {
        fl!(LANGUAGE_LOADER, "dialog-yes-button")
    }
    pub fn no() -> String {
        fl!(LANGUAGE_LOADER, "dialog-no-button")
    }
    pub fn delete() -> String {
        fl!(LANGUAGE_LOADER, "dialog-delete-button")
    }
    pub fn overwrite() -> String {
        fl!(LANGUAGE_LOADER, "dialog-overwrite-button")
    }
    pub fn copy() -> String {
        fl!(LANGUAGE_LOADER, "cmd-edit-copy-action")
    }
    pub fn restore_defaults() -> String {
        fl!(LANGUAGE_LOADER, "settings-restore-defaults-button")
    }
}

pub fn dialog_frame(context: &egui::Context) -> egui::Frame {
    egui::Frame::window(&context.style()).inner_margin(DIALOG_PADDING).corner_radius(10)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DialogSize {
    /// Message boxes and single choice prompts.
    Small,
    /// Short forms.
    Medium,
    /// Forms with long values or editors.
    Large,
    /// Tabbed dialogs and wide reports.
    XLarge,
    Width(f32),
}

impl DialogSize {
    pub fn width(self) -> f32 {
        match self {
            DialogSize::Small => 420.0,
            DialogSize::Medium => 520.0,
            DialogSize::Large => 640.0,
            DialogSize::XLarge => 760.0,
            DialogSize::Width(width) => width,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonKind {
    Primary,
    Secondary,
    Destructive,
}

pub struct DialogButton<A> {
    label: String,
    action: A,
    kind: ButtonKind,
    cancel: bool,
    leading: bool,
    enabled: bool,
    tooltip: Option<String>,
}

impl<A> DialogButton<A> {
    fn new(label: impl Into<String>, action: A, kind: ButtonKind) -> Self {
        Self {
            label: label.into(),
            action,
            kind,
            cancel: false,
            leading: false,
            enabled: true,
            tooltip: None,
        }
    }

    /// The dialog's main action: filled with the accent colour, triggered by Enter when enabled.
    pub fn primary(label: impl Into<String>, action: A) -> Self {
        Self::new(label, action, ButtonKind::Primary)
    }

    pub fn secondary(label: impl Into<String>, action: A) -> Self {
        Self::new(label, action, ButtonKind::Secondary)
    }

    /// An action that destroys data. Enter never triggers it; without a primary button it cancels instead.
    pub fn destructive(label: impl Into<String>, action: A) -> Self {
        Self::new(label, action, ButtonKind::Destructive)
    }

    /// Secondary button that Escape and the header's close button trigger as well.
    pub fn cancel(label: impl Into<String>, action: A) -> Self {
        Self::secondary(label, action).cancels()
    }

    /// Lets Escape and the header's close button trigger this button, e.g. a lone primary "Close".
    pub fn cancels(mut self) -> Self {
        self.cancel = true;
        self
    }

    /// Places the button at the left edge, for actions beside the dialog's result like "Restore Defaults".
    pub fn leading(mut self) -> Self {
        self.leading = true;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.tooltip = Some(text.into());
        self
    }
}

pub struct DialogResponse<A> {
    /// The button that was clicked, or triggered with Escape or Enter.
    pub action: Option<A>,
    /// Escape or the close button was used on a dialog without a cancel button.
    pub dismissed: bool,
    pub rect: egui::Rect,
}

pub struct Dialog {
    id: egui::Id,
    title: Option<String>,
    subtitle: Option<String>,
    icon: Option<String>,
    size: DialogSize,
    height: Option<f32>,
    max_height: f32,
    scroll: bool,
    confirm_on_enter: bool,
    floating: Option<(egui::Align2, egui::Vec2)>,
}

impl Dialog {
    /// A titleless dialog, with its body and buttons styled by the shared shell.
    pub fn new(id: impl std::hash::Hash) -> Self {
        Self {
            id: egui::Id::new(id),
            title: None,
            subtitle: None,
            icon: None,
            size: DialogSize::Medium,
            height: None,
            max_height: 560.0,
            scroll: true,
            confirm_on_enter: false,
            floating: None,
        }
    }

    /// Alias for [`Self::new`], kept for callers that distinguish tabbed dialogs.
    pub fn untitled(id: impl std::hash::Hash) -> Self {
        Self::new(id)
    }

    /// Show an optional header for messages, help and other content that needs a heading.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn size(mut self, size: DialogSize) -> Self {
        self.size = size;
        self
    }

    pub fn max_width(self, width: f32) -> Self {
        self.size(DialogSize::Width(width))
    }

    /// Keeps the dialog at this height whatever the body shows, so switching pages does not resize it.
    pub fn fixed_height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self.max_height = self.max_height.max(height);
        self
    }

    pub fn max_height(mut self, height: f32) -> Self {
        self.max_height = height;
        self
    }

    /// Whether the body scrolls when it does not fit. Turn off for bodies that size themselves.
    pub fn scroll(mut self, scroll: bool) -> Self {
        self.scroll = scroll;
        self
    }

    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// A glyph shown in front of the title.
    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn confirm_on_enter(mut self, confirm: bool) -> Self {
        self.confirm_on_enter = confirm;
        self
    }

    /// Shows the dialog as a non-modal panel anchored to the window, e.g. for live previews.
    /// Floating dialogs leave the keyboard to the application.
    pub fn floating(mut self, align: egui::Align2, offset: egui::Vec2) -> Self {
        self.floating = Some((align, offset));
        self
    }

    pub fn show<A: Clone>(self, context: &egui::Context, body: impl FnOnce(&mut DialogUi<'_, A>)) -> DialogResponse<A> {
        let screen = context.content_rect();
        let width = (screen.width() - 48.0).clamp(240.0, self.size.width().max(240.0));
        let offset = self.floating.map_or(0.0, |(_, offset)| offset.y.abs());
        let available = (screen.height() - 64.0 - offset).clamp(110.0, self.max_height.max(110.0));
        let height = self.height.map_or(available, |height| height.min(available));
        let footer_id = self.id.with("footer-height");
        let footer_height = context.data(|data| data.get_temp::<f32>(footer_id)).unwrap_or(56.0);
        let mut state = State::<A>::default();
        let content = |ui: &mut egui::Ui| {
            ui.set_width(width);
            if self.height.is_some() {
                ui.set_min_height(height);
            }
            let top = ui.cursor().top();
            if let Some(title) = &self.title {
                state.header_close = header(ui, title, self.subtitle.as_deref(), self.icon.as_deref());
            }
            let mut dialog = DialogUi {
                ui,
                id: self.id,
                top,
                height,
                fixed: self.height.is_some(),
                scroll: self.scroll,
                footer_height,
                page: 0,
                state: &mut state,
            };
            body(&mut dialog);
        };
        let (rect, keyboard) = if let Some((align, offset)) = self.floating {
            let response = egui::Window::new(self.title.clone().unwrap_or_default())
                .id(self.id)
                .title_bar(false)
                .resizable(false)
                .anchor(align, offset)
                .fixed_size(egui::vec2(width, height))
                .frame(dialog_frame(context))
                .show(context, |ui| content(ui));
            (response.map_or(egui::Rect::NOTHING, |response| response.response.rect), false)
        } else {
            let response = egui::Modal::new(self.id).frame(dialog_frame(context)).show(context, |ui| content(ui));
            (response.response.rect, response.is_top_modal && !response.any_popup_open)
        };
        if let Some(measured) = state.footer_height {
            if (measured - footer_height).abs() > 0.5 {
                context.data_mut(|data| data.insert_temp(footer_id, measured));
                context.request_discard("dialog footer height changed");
            }
        }
        let mut response = DialogResponse {
            action: state.clicked.take(),
            dismissed: false,
            rect,
        };
        let escape = keyboard && context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        if response.action.is_none() && (state.header_close || escape) {
            match state.cancel.take() {
                Some(cancel) => response.action = Some(cancel),
                None => response.dismissed = true,
            }
        }
        if response.action.is_none()
            && self.confirm_on_enter
            && keyboard
            && state.default.is_some()
            && context.memory(|memory| memory.focused().is_none())
            && context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Enter))
        {
            response.action = state.default.take();
        }
        if context.will_discard() {
            // The pass is repeated with the same input, so the click is reported then.
            response.action = None;
            response.dismissed = false;
        }
        response
    }
}

struct State<A> {
    header_close: bool,
    clicked: Option<A>,
    cancel: Option<A>,
    default: Option<A>,
    footer_height: Option<f32>,
}

impl<A> Default for State<A> {
    fn default() -> Self {
        Self {
            header_close: false,
            clicked: None,
            cancel: None,
            default: None,
            footer_height: None,
        }
    }
}

/// Body and button row of a [`Dialog`]. Add tabs, then the content, then the buttons.
pub struct DialogUi<'a, A> {
    ui: &'a mut egui::Ui,
    id: egui::Id,
    top: f32,
    height: f32,
    fixed: bool,
    scroll: bool,
    footer_height: f32,
    /// Selected tab, so every page keeps its own scroll position.
    page: usize,
    state: &'a mut State<A>,
}

impl<A: Clone> DialogUi<'_, A> {
    /// The dialog's `Ui` above the body, for anything that should not scroll with it.
    pub fn ui(&mut self) -> &mut egui::Ui {
        self.ui
    }

    /// Page selector for tabbed dialogs; falls back to a drop-down when the tabs do not fit.
    pub fn tabs<P: PartialEq + Copy>(&mut self, current: &mut P, pages: &[(P, String)]) {
        let ui = &mut *self.ui;
        let spacing = 2.0;
        let natural: f32 = pages
            .iter()
            .map(|(_, label)| {
                ui.fonts_mut(|fonts| fonts.layout_no_wrap(label.clone(), FontId::proportional(14.0), Color32::PLACEHOLDER).size().x) + 20.0 + spacing
            })
            .sum();
        if natural - spacing > ui.available_width() {
            let selected = pages
                .iter()
                .find(|(page, _)| page == current)
                .map(|(_, label)| label.clone())
                .unwrap_or_default();
            egui::ComboBox::from_id_salt(self.id.with("tabs"))
                .width(ui.available_width())
                .selected_text(selected)
                .show_ui(ui, |ui| {
                    for (page, label) in pages {
                        ui.selectable_value(current, *page, label);
                    }
                });
        } else {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = spacing;
                for (page, label) in pages {
                    appearance::tab(ui, current, *page, label);
                }
            });
        }
        ui.add_space(12.0);
        self.page = pages.iter().position(|(page, _)| page == current).unwrap_or_default();
    }

    /// The dialog body; it scrolls when it does not fit unless [`Dialog::scroll`] is off.
    pub fn content<R>(&mut self, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
        if !self.scroll {
            return add(self.ui);
        }
        let max_height = (self.height - (self.ui.cursor().top() - self.top) - self.footer_height).max(0.0);
        egui::ScrollArea::vertical()
            .id_salt(self.id.with(("body", self.page)))
            .auto_shrink([false, !self.fixed])
            .min_scrolled_height(0.0)
            .max_height(max_height)
            .show(self.ui, |ui| {
                // Keeps the content clear of the floating scroll bar.
                egui::Frame::new()
                    .inner_margin(egui::Margin {
                        right: 12,
                        ..Default::default()
                    })
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        add(ui)
                    })
                    .inner
            })
            .inner
    }

    /// Reports `action` as if one of the buttons was clicked, e.g. when picking an item closes the dialog.
    pub fn finish(&mut self, action: A) {
        self.state.clicked = Some(action);
    }

    /// The button row at the bottom of the dialog, in visual order.
    pub fn buttons(&mut self, buttons: impl IntoIterator<Item = DialogButton<A>>) {
        let buttons: Vec<_> = buttons.into_iter().collect();
        let ui = &mut *self.ui;
        let top = ui.cursor().top();
        ui.add_space(4.0);
        let separator_y = ui.cursor().top();
        ui.painter().hline(
            (ui.min_rect().left() - f32::from(DIALOG_PADDING))..=(ui.max_rect().right() + f32::from(DIALOG_PADDING)),
            separator_y,
            ui.visuals().widgets.noninteractive.bg_stroke,
        );
        ui.add_space(4.0);
        for button in &buttons {
            if button.cancel && self.state.cancel.is_none() {
                self.state.cancel = Some(button.action.clone());
            }
            if button.kind == ButtonKind::Primary && button.enabled && self.state.default.is_none() {
                self.state.default = Some(button.action.clone());
            }
        }
        // Like a macOS alert, Enter picks the safe choice when the dialog's action destroys data.
        if self.state.default.is_none() && buttons.iter().any(|button| button.kind == ButtonKind::Destructive) {
            self.state.default = self.state.cancel.clone();
        }
        if let Some(action) = button_row(ui, buttons) {
            self.state.clicked = Some(action);
        }
        self.state.footer_height = Some(ui.cursor().top() - top);
    }
}

const CLOSE_SIZE: f32 = 24.0;

fn header(ui: &mut egui::Ui, title: &str, subtitle: Option<&str>, icon: Option<&str>) -> bool {
    let right = ui.max_rect().right();
    let width = (ui.available_width() - CLOSE_SIZE - 8.0).max(0.0);
    let row = ui
        .horizontal(|ui| {
            ui.set_max_width(width);
            if let Some(icon) = icon {
                ui.label(egui::RichText::new(icon).size(24.0));
                ui.add_space(2.0);
            }
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.add(egui::Label::new(appearance::bold(ui, title).size(16.0)).wrap());
                if let Some(subtitle) = subtitle {
                    ui.add(egui::Label::new(egui::RichText::new(subtitle).size(12.0).weak()).wrap());
                }
            });
        })
        .response
        .rect;
    let rect = egui::Rect::from_center_size(egui::pos2(right - CLOSE_SIZE / 2.0, row.center().y), egui::Vec2::splat(CLOSE_SIZE));
    let response = ui
        .interact(rect, ui.id().with("dialog-close"), egui::Sense::click())
        .on_hover_text(labels::close());
    let visuals = ui.style().interact(&response);
    if response.hovered() {
        ui.painter().circle_filled(rect.center(), CLOSE_SIZE / 2.0, visuals.bg_fill);
    }
    // Drawn as lines, since the glyph sits below the title's centre in most fonts.
    let arm = 4.5;
    let stroke = egui::Stroke::new(1.6, visuals.fg_stroke.color);
    let center = rect.center();
    ui.painter()
        .line_segment([center - egui::Vec2::splat(arm), center + egui::Vec2::splat(arm)], stroke);
    ui.painter()
        .line_segment([center + egui::vec2(-arm, arm), center + egui::vec2(arm, -arm)], stroke);
    ui.add_space(10.0);
    response.clicked()
}

fn button_widget(button: &DialogButton<impl Sized>, height: f32) -> egui::Button<'static> {
    let widget = match button.kind {
        ButtonKind::Primary => appearance::primary_button(button.label.clone()),
        ButtonKind::Destructive => egui::Button::new(egui::RichText::new(button.label.clone()).color(Color32::WHITE)).fill(DANGER),
        ButtonKind::Secondary => egui::Button::new(button.label.clone()),
    };
    widget.min_size(egui::vec2(BUTTON_MIN_WIDTH, height))
}

fn button_width(ui: &egui::Ui, button: &DialogButton<impl Sized>) -> f32 {
    let text = ui.fonts_mut(|fonts| {
        fonts
            .layout_no_wrap(button.label.clone(), egui::TextStyle::Button.resolve(ui.style()), Color32::PLACEHOLDER)
            .size()
            .x
    });
    (text + 2.0 * ui.spacing().button_padding.x).max(BUTTON_MIN_WIDTH)
}

/// Leading buttons on the left, the rest on the right in the given order. When everything does not
/// fit on one line, the leading buttons move to a line of their own above the others.
///
/// [`DialogUi::buttons`] uses it for the dialog footer; forms embedded in a view use it directly.
pub fn button_row<A: Clone>(ui: &mut egui::Ui, buttons: Vec<DialogButton<A>>) -> Option<A> {
    let spacing = ui.spacing().item_spacing.x;
    let height = ui.spacing().interact_size.y;
    let total: f32 = buttons.iter().map(|button| button_width(ui, button) + spacing).sum::<f32>() - spacing;
    let stacked = total > ui.available_width();
    let (leading, trailing): (Vec<_>, Vec<_>) = buttons.into_iter().partition(|button| button.leading);
    let mut clicked = None;
    let mut add = |ui: &mut egui::Ui, button: &DialogButton<A>| {
        let mut response = ui.add_enabled(button.enabled, button_widget(button, height));
        if let Some(tooltip) = &button.tooltip {
            response = response.on_hover_text(tooltip);
        }
        if response.clicked() {
            clicked = Some(button.action.clone());
        }
    };
    if stacked && !leading.is_empty() {
        ui.horizontal_wrapped(|ui| {
            for button in &leading {
                add(ui, button);
            }
        });
    }
    ui.horizontal(|ui| {
        if !stacked {
            for button in &leading {
                add(ui, button);
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center).with_main_wrap(true), |ui| {
            for button in trailing.iter().rev() {
                add(ui, button);
            }
        });
    });
    clicked
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageKind {
    Info,
    Question,
    Warning,
    Error,
}

#[derive(Clone)]
enum Choice<A> {
    User(A),
    Copy,
}

/// Message box on top of [`Dialog`] with an icon for its kind.
pub struct MessageBox<A> {
    id: egui::Id,
    kind: MessageKind,
    title: String,
    message: String,
    details: String,
    buttons: Vec<DialogButton<A>>,
    copy: bool,
}

impl<A: Clone> MessageBox<A> {
    pub fn new(id: impl std::hash::Hash, kind: MessageKind, title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            id: egui::Id::new(id),
            kind,
            title: title.into(),
            message: message.into(),
            details: String::new(),
            buttons: Vec::new(),
            copy: false,
        }
    }

    /// Secondary text below the message.
    pub fn details(mut self, details: impl Into<String>) -> Self {
        self.details = details.into();
        self
    }

    pub fn buttons(mut self, buttons: impl IntoIterator<Item = DialogButton<A>>) -> Self {
        self.buttons.extend(buttons);
        self
    }

    /// Adds a leading button that copies the title, message and details to the clipboard.
    pub fn copyable(mut self) -> Self {
        self.copy = true;
        self
    }

    pub fn show(self, context: &egui::Context) -> DialogResponse<A> {
        let mut buttons = Vec::new();
        if self.copy {
            buttons.push(DialogButton::secondary(labels::copy(), Choice::Copy).leading());
        }
        buttons.extend(self.buttons.into_iter().map(|button| DialogButton {
            label: button.label,
            action: Choice::User(button.action),
            kind: button.kind,
            cancel: button.cancel,
            leading: button.leading,
            enabled: button.enabled,
            tooltip: button.tooltip,
        }));
        let (kind, message, details) = (self.kind, &self.message, &self.details);
        let response = Dialog::new(self.id)
            .title(&self.title)
            .size(DialogSize::Small)
            .max_height(360.0)
            .confirm_on_enter(true)
            .show(context, |dialog| {
                dialog.content(|ui| {
                    ui.horizontal_top(|ui| {
                        message_icon(ui, kind);
                        ui.add_space(4.0);
                        ui.vertical(|ui| {
                            ui.add(egui::Label::new(message.as_str()).wrap().selectable(true));
                            if !details.is_empty() {
                                ui.add_space(6.0);
                                ui.add(egui::Label::new(egui::RichText::new(details.as_str()).weak()).wrap().selectable(true));
                            }
                        });
                    });
                    ui.add_space(4.0);
                });
                dialog.buttons(buttons);
            });
        let action = match response.action {
            Some(Choice::Copy) => {
                context.copy_text(format!("{}\n{}\n{}", self.title, self.message, self.details).trim_end().to_owned());
                None
            }
            Some(Choice::User(action)) => Some(action),
            None => None,
        };
        DialogResponse {
            action,
            dismissed: response.dismissed,
            rect: response.rect,
        }
    }
}

fn message_icon(ui: &mut egui::Ui, kind: MessageKind) {
    let (color, glyph) = match kind {
        MessageKind::Info => (PRIMARY, "i"),
        MessageKind::Question => (PRIMARY, "?"),
        MessageKind::Warning => (WARNING, "!"),
        MessageKind::Error => (DANGER, "!"),
    };
    let (rect, _) = ui.allocate_exact_size(egui::vec2(32.0, 32.0), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 15.0, color);
    let font = FontId::new(19.0, appearance::bold_family(ui));
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, glyph, font, Color32::WHITE);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Action {
        Reset,
        Cancel,
        Ok,
    }

    fn key(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }

    struct Frame {
        response: DialogResponse<Action>,
        texts: std::collections::HashMap<String, egui::Rect>,
    }

    fn frame(context: &egui::Context, size: egui::Vec2, events: Vec<egui::Event>, ok_enabled: bool) -> Frame {
        let mut response = None;
        let output = context.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                events,
                ..Default::default()
            },
            |context| {
                response = Some(Dialog::new("test").confirm_on_enter(true).show(context, |dialog| {
                    dialog.content(|ui| ui.label("Body"));
                    dialog.buttons([
                        DialogButton::secondary("Restore Defaults", Action::Reset).leading(),
                        DialogButton::cancel("Cancel", Action::Cancel),
                        DialogButton::primary("OK", Action::Ok).enabled(ok_enabled),
                    ]);
                }));
            },
        );
        let texts = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some((text.galley.text().to_owned(), text.galley.rect.translate(text.pos.to_vec2()))),
                _ => None,
            })
            .collect();
        Frame {
            response: response.unwrap(),
            texts,
        }
    }

    #[test]
    fn escape_cancels_and_enter_confirms() {
        let context = egui::Context::default();
        let size = egui::vec2(800.0, 600.0);
        frame(&context, size, vec![], true);
        assert_eq!(frame(&context, size, vec![], true).response.action, None);
        assert_eq!(frame(&context, size, vec![key(egui::Key::Escape)], true).response.action, Some(Action::Cancel));
        assert_eq!(frame(&context, size, vec![key(egui::Key::Enter)], true).response.action, Some(Action::Ok));
        assert_eq!(
            frame(&context, size, vec![key(egui::Key::Enter)], false).response.action,
            None,
            "Enter must not trigger a disabled primary button"
        );
    }

    #[test]
    fn enter_cancels_a_destructive_message() {
        let context = egui::Context::default();
        let show = |events: Vec<egui::Event>| {
            let mut response = None;
            let _ = context.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0))),
                    events,
                    ..Default::default()
                },
                |context| {
                    response = Some(
                        MessageBox::new("delete", MessageKind::Warning, "Delete?", "Gone for good.")
                            .buttons([
                                DialogButton::cancel("Cancel", Action::Cancel),
                                DialogButton::destructive("Delete", Action::Reset),
                            ])
                            .show(context),
                    );
                },
            );
            response.unwrap().action
        };
        show(vec![]);
        assert_eq!(show(vec![key(egui::Key::Enter)]), Some(Action::Cancel));
    }

    #[test]
    fn buttons_follow_the_platform_order() {
        let context = egui::Context::default();
        for size in [egui::vec2(800.0, 600.0), egui::vec2(300.0, 600.0)] {
            frame(&context, size, vec![], true);
            let frame = frame(&context, size, vec![], true);
            let (reset, cancel, ok) = (frame.texts["Restore Defaults"], frame.texts["Cancel"], frame.texts["OK"]);
            assert!(
                frame.texts["Body"].top() < frame.response.rect.top() + 40.0,
                "titleless dialogs must not reserve space for a header"
            );
            assert!(cancel.right() < ok.left(), "the primary button must come last at {size:?}");
            assert!(
                frame.response.rect.contains_rect(ok),
                "the primary button must be inside the dialog at {size:?}"
            );
            if size.x > 500.0 {
                assert!(reset.right() < cancel.left() && (reset.center().y - ok.center().y).abs() < 1.0);
                assert!(
                    ok.right() > frame.response.rect.right() - f32::from(DIALOG_PADDING) - BUTTON_MIN_WIDTH,
                    "the primary button must sit at the right edge"
                );
            } else {
                assert!(reset.bottom() < ok.top(), "narrow dialogs move leading buttons above the others");
            }
        }
    }

    #[test]
    fn close_button_dismisses_a_dialog_without_cancel_button() {
        let context = egui::Context::default();
        let size = egui::vec2(800.0, 600.0);
        let show = |events: Vec<egui::Event>| {
            let mut response = None;
            let output = context.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                    events,
                    ..Default::default()
                },
                |context| {
                    response = Some(Dialog::new("plain").title("Plain").show::<()>(context, |dialog| {
                        dialog.content(|ui| ui.label("Body"));
                    }));
                },
            );
            let response = response.unwrap();
            // The close button sits at the right edge, centred on the title.
            let close = output.shapes.iter().find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == "Plain" => Some(egui::pos2(
                    response.rect.right() - f32::from(DIALOG_PADDING) - CLOSE_SIZE / 2.0,
                    text.pos.y + text.galley.size().y / 2.0,
                )),
                _ => None,
            });
            (response, close)
        };
        show(vec![]);
        let (_, close) = show(vec![]);
        let close = close.expect("the header must have a close button");
        let button = |pressed| egui::Event::PointerButton {
            pos: close,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        show(vec![egui::Event::PointerMoved(close)]);
        show(vec![button(true)]);
        let (response, _) = show(vec![button(false)]);
        assert!(response.dismissed && response.action.is_none());
    }
}
