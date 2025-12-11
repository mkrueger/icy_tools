use i18n_embed_fl::fl;
use iced::{
    Alignment, Border, Color, Element, Event, Length, Theme,
    widget::{Space, button, column, container, row, text},
};
use std::fmt;

use crate::{
    DIALOG_SPACING, DIALOG_WIDTH_MEDIUM, LANGUAGE_LOADER, TEXT_SIZE_NORMAL, TEXT_SIZE_SMALL, button_row, danger_button, dialog_area, modal_container,
    modal_overlay, primary_button, secondary_button,
    ui::dialog::{Dialog, DialogAction},
    ui::icons::{error_icon, warning_icon},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonSet {
    Ok,
    Close,
    OkCancel,
    YesNo,
    YesNoCancel,
    DeleteCancel,
    OverwriteCancel,
}

impl ButtonSet {
    /// Returns buttons in correct order with appropriate styles
    pub fn to_buttons(&self) -> Vec<(ButtonType, ButtonStyle)> {
        match self {
            Self::Ok => vec![(ButtonType::Ok, ButtonStyle::Primary)],
            Self::Close => vec![(ButtonType::Close, ButtonStyle::Secondary)],
            Self::OkCancel => vec![(ButtonType::Cancel, ButtonStyle::Secondary), (ButtonType::Ok, ButtonStyle::Primary)],
            Self::YesNo => vec![(ButtonType::No, ButtonStyle::Secondary), (ButtonType::Yes, ButtonStyle::Primary)],
            Self::YesNoCancel => vec![
                (ButtonType::Cancel, ButtonStyle::Secondary),
                (ButtonType::No, ButtonStyle::Secondary),
                (ButtonType::Yes, ButtonStyle::Primary),
            ],
            Self::DeleteCancel => vec![(ButtonType::Cancel, ButtonStyle::Secondary), (ButtonType::Delete, ButtonStyle::Danger)],
            Self::OverwriteCancel => vec![(ButtonType::Cancel, ButtonStyle::Secondary), (ButtonType::Overwrite, ButtonStyle::Danger)],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonType {
    Ok,
    Cancel,
    Yes,
    No,
    Close,
    Delete,
    Overwrite,
}

impl fmt::Display for ButtonType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::Ok => fl!(LANGUAGE_LOADER, "dialog-ok-button"),
            Self::Cancel => fl!(LANGUAGE_LOADER, "dialog-cancel-button"),
            Self::Yes => fl!(LANGUAGE_LOADER, "dialog-yes-button"),
            Self::No => fl!(LANGUAGE_LOADER, "dialog-no-button"),
            Self::Close => fl!(LANGUAGE_LOADER, "dialog-close-button"),
            Self::Delete => fl!(LANGUAGE_LOADER, "dialog-delete-button"),
            Self::Overwrite => fl!(LANGUAGE_LOADER, "dialog-overwrite-button"),
        };
        write!(f, "{}", text)
    }
}

impl ButtonType {
    pub fn to_result(&self) -> DialogResult {
        match self {
            Self::Ok => DialogResult::Ok,
            Self::Cancel => DialogResult::Cancel,
            Self::Yes => DialogResult::Yes,
            Self::No => DialogResult::No,
            Self::Close => DialogResult::Close,
            Self::Delete => DialogResult::Delete,
            Self::Overwrite => DialogResult::Overwrite,
        }
    }

    pub fn primary<'a, Message: Clone + 'a>(&self, is_sensitive: bool, on_press: Message) -> button::Button<'a, Message> {
        let label = format!("{}", self);
        primary_button(label, is_sensitive.then_some(on_press))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogType {
    Plain,
    Error,
    Warning,
    Info,
    Question,
}

impl DialogType {
    pub fn icon<'a>(&self) -> Option<Element<'a, Theme>> {
        let c = *self;
        match self {
            DialogType::Plain => None,
            DialogType::Error => Some(
                error_icon(48.0)
                    .style(move |theme: &Theme, _status| {
                        let color = c.icon_color(theme);
                        iced::widget::svg::Style { color: Some(color) }
                    })
                    .into(),
            ),
            DialogType::Warning => Some(
                warning_icon(48.0)
                    .style(move |theme: &Theme, _status| {
                        let color = c.icon_color(theme);
                        iced::widget::svg::Style { color: Some(color) }
                    })
                    .into(),
            ),
            DialogType::Info => None,
            DialogType::Question => None,
        }
    }

    pub fn icon_color(&self, theme: &Theme) -> Color {
        let palette = theme.extended_palette();
        match self {
            DialogType::Plain => Color::TRANSPARENT,
            DialogType::Error => palette.danger.base.color,
            DialogType::Warning => palette.warning.base.color,
            DialogType::Info => palette.primary.base.color,
            DialogType::Question => palette.primary.base.color,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogResult {
    Ok,
    Cancel,
    Yes,
    No,
    Close,
    Delete,
    Overwrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonStyle {
    Primary,
    Secondary,
    Danger,
}

#[derive(Debug, Clone)]
pub struct ConfirmationDialog {
    pub dialog_type: DialogType,
    pub title: String,
    pub secondary_message: Option<String>,
    pub primary_message: String,
    pub buttons: ButtonSet,
}

impl Default for ConfirmationDialog {
    fn default() -> Self {
        Self {
            dialog_type: DialogType::Plain,
            title: String::new(),
            primary_message: String::new(),
            secondary_message: None,
            buttons: ButtonSet::Ok,
        }
    }
}

impl ConfirmationDialog {
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            dialog_type: DialogType::Plain,
            title: title.into(),
            primary_message: message.into(),
            secondary_message: None,
            buttons: ButtonSet::Ok,
        }
    }

    pub fn dialog_type(mut self, dialog_type: DialogType) -> Self {
        self.dialog_type = dialog_type;
        self
    }

    pub fn secondary_message(mut self, msg: impl Into<String>) -> Self {
        self.secondary_message = Some(msg.into());
        self
    }

    pub fn buttons(mut self, buttons: ButtonSet) -> Self {
        self.buttons = buttons;
        self
    }
    pub fn view<'a, Message: 'a + Clone>(self, background: Element<'a, Message>, on_result: impl Fn(DialogResult) -> Message + 'a) -> Element<'a, Message> {
        let mut text_column = column![text(self.title.clone()).size(22.0).font(iced::Font {
            weight: iced::font::Weight::Bold,
            ..iced::Font::default()
        })];

        if let Some(secondary) = self.secondary_message.clone() {
            text_column = text_column.push(text(secondary).size(TEXT_SIZE_SMALL).style(|theme: &Theme| iced::widget::text::Style {
                color: Some(theme.extended_palette().secondary.base.color),
            }));
        }

        // Build header with icon and title/secondary message side by side
        let header = if let Some(icon_elem) = self.dialog_type.icon() {
            let icon_container = container(icon_elem).style(|_theme: &Theme| container::Style {
                background: None,
                border: Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            });
            row![icon_container, Space::new().width(12.0), text_column.width(Length::Fill)].align_y(Alignment::Center)
        } else {
            row![text_column.width(Length::Fill)]
        };

        // Main content area with primary message
        let content = column![
            header,
            Space::new().height(DIALOG_SPACING),
            text(self.primary_message.clone()).size(TEXT_SIZE_NORMAL)
        ]
        .spacing(DIALOG_SPACING);

        // Assembly - convert Theme elements to Message elements
        let dialog_content: Element<'a, Message> = dialog_area(content.into()).map(|_| unreachable!());
        let buttons_row: Element<'a, Message> = button_row(
            self.buttons
                .to_buttons()
                .into_iter()
                .map(|(button_type, style)| {
                    let msg = on_result(button_type.to_result());
                    let label = button_type.to_string();
                    match style {
                        ButtonStyle::Primary => primary_button(&label, Some(msg)),
                        ButtonStyle::Secondary => secondary_button(&label, Some(msg)),
                        ButtonStyle::Danger => danger_button(&label, Some(msg)),
                    }
                    .into()
                })
                .collect(),
        );

        let button_area: Element<'a, Message> = dialog_area(buttons_row);
        let modal = modal_container(column![dialog_content, button_area].into(), DIALOG_WIDTH_MEDIUM);

        // Overlay + centering
        modal_overlay::<Message>(background, modal.into())
    }

    /// Build just the dialog content without modal overlay (for use with Dialog trait)
    pub fn view_content<'a, Message: 'a + Clone>(&'a self, on_result: impl Fn(DialogResult) -> Message + 'a) -> Element<'a, Message> {
        let mut text_column = column![text(self.title.clone()).size(22.0).font(iced::Font {
            weight: iced::font::Weight::Bold,
            ..iced::Font::default()
        })];

        if let Some(secondary) = self.secondary_message.clone() {
            text_column = text_column.push(text(secondary).size(TEXT_SIZE_SMALL).style(|theme: &Theme| iced::widget::text::Style {
                color: Some(theme.extended_palette().secondary.base.color),
            }));
        }

        // Build header with icon and title/secondary message side by side
        let header = if let Some(icon_elem) = self.dialog_type.icon() {
            let icon_container = container(icon_elem).style(|_theme: &Theme| container::Style {
                background: None,
                border: Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            });
            row![icon_container, Space::new().width(12.0), text_column.width(Length::Fill)].align_y(Alignment::Center)
        } else {
            row![text_column.width(Length::Fill)]
        };

        // Main content area with primary message
        let content = column![
            header,
            Space::new().height(DIALOG_SPACING),
            text(self.primary_message.clone()).size(TEXT_SIZE_NORMAL)
        ]
        .spacing(DIALOG_SPACING);

        // Assembly - convert Theme elements to Message elements
        let dialog_content: Element<'a, Message> = dialog_area(content.into()).map(|_| unreachable!());
        let buttons_row: Element<'a, Message> = button_row(
            self.buttons
                .to_buttons()
                .into_iter()
                .map(|(button_type, style)| {
                    let msg = on_result(button_type.to_result());
                    let label = button_type.to_string();
                    match style {
                        ButtonStyle::Primary => primary_button(&label, Some(msg)),
                        ButtonStyle::Secondary => secondary_button(&label, Some(msg)),
                        ButtonStyle::Danger => danger_button(&label, Some(msg)),
                    }
                    .into()
                })
                .collect(),
        );

        let button_area: Element<'a, Message> = dialog_area(buttons_row);
        modal_container(column![dialog_content, button_area].into(), DIALOG_WIDTH_MEDIUM).into()
    }

    /// Get the default cancel result for this button set
    pub fn cancel_result(&self) -> DialogResult {
        match self.buttons {
            ButtonSet::Ok => DialogResult::Ok,
            ButtonSet::Close => DialogResult::Close,
            ButtonSet::OkCancel | ButtonSet::DeleteCancel | ButtonSet::OverwriteCancel => DialogResult::Cancel,
            ButtonSet::YesNo => DialogResult::No,
            ButtonSet::YesNoCancel => DialogResult::Cancel,
        }
    }

    /// Get the default confirm result for this button set
    pub fn confirm_result(&self) -> DialogResult {
        match self.buttons {
            ButtonSet::Ok => DialogResult::Ok,
            ButtonSet::Close => DialogResult::Close,
            ButtonSet::OkCancel => DialogResult::Ok,
            ButtonSet::YesNo | ButtonSet::YesNoCancel => DialogResult::Yes,
            ButtonSet::DeleteCancel => DialogResult::Delete,
            ButtonSet::OverwriteCancel => DialogResult::Overwrite,
        }
    }
}

/// A ConfirmationDialog wrapped for use with the Dialog trait system.
/// This stores the dialog along with how to map results to app messages.
pub struct ConfirmationDialogWrapper<M, F>
where
    F: Fn(DialogResult) -> M,
{
    pub dialog: ConfirmationDialog,
    pub on_result: F,
    /// Flag to track if we've already handled a button click
    _handled: bool,
}

impl<M, F> ConfirmationDialogWrapper<M, F>
where
    F: Fn(DialogResult) -> M,
{
    pub fn new(dialog: ConfirmationDialog, on_result: F) -> Self {
        Self {
            dialog,
            on_result,
            _handled: false,
        }
    }
}

// ============================================================================
// Builder functions for common dialog patterns
// ============================================================================

/// Create a simple error dialog with an OK button.
///
/// # Example
/// ```ignore
/// dialog_stack.push(error_dialog(
///     "File Not Found",
///     "The file could not be located.",
///     |_result| Message::DismissError,
/// ));
/// ```
pub fn error_dialog<M, F>(title: impl Into<String>, message: impl Into<String>, on_result: F) -> ConfirmationDialogWrapper<M, F>
where
    M: Clone + Send + 'static,
    F: Fn(DialogResult) -> M + Send + 'static,
{
    ConfirmationDialogWrapper::new(
        ConfirmationDialog::new(title, message).dialog_type(DialogType::Error).buttons(ButtonSet::Ok),
        on_result,
    )
}

/// Create a warning dialog with an OK button.
///
/// # Example
/// ```ignore
/// dialog_stack.push(warning_dialog(
///     "Unsaved Changes",
///     "You have unsaved changes.",
///     |_result| Message::DismissWarning,
/// ));
/// ```
pub fn warning_dialog<M, F>(title: impl Into<String>, message: impl Into<String>, on_result: F) -> ConfirmationDialogWrapper<M, F>
where
    M: Clone + Send + 'static,
    F: Fn(DialogResult) -> M + Send + 'static,
{
    ConfirmationDialogWrapper::new(
        ConfirmationDialog::new(title, message).dialog_type(DialogType::Warning).buttons(ButtonSet::Ok),
        on_result,
    )
}

/// Create an info dialog with an OK button.
///
/// # Example
/// ```ignore
/// dialog_stack.push(info_dialog(
///     "Export Complete",
///     "Your file has been exported successfully.",
///     |_result| Message::DismissInfo,
/// ));
/// ```
pub fn info_dialog<M, F>(title: impl Into<String>, message: impl Into<String>, on_result: F) -> ConfirmationDialogWrapper<M, F>
where
    M: Clone + Send + 'static,
    F: Fn(DialogResult) -> M + Send + 'static,
{
    ConfirmationDialogWrapper::new(
        ConfirmationDialog::new(title, message).dialog_type(DialogType::Info).buttons(ButtonSet::Ok),
        on_result,
    )
}

/// Create a Yes/No confirmation dialog.
///
/// # Example
/// ```ignore
/// dialog_stack.push(confirm_yes_no(
///     "Save Changes?",
///     "Do you want to save your changes before closing?",
///     |result| match result {
///         DialogResult::Yes => Message::SaveAndClose,
///         _ => Message::CloseWithoutSaving,
///     },
/// ));
/// ```
pub fn confirm_yes_no<M, F>(title: impl Into<String>, message: impl Into<String>, on_result: F) -> ConfirmationDialogWrapper<M, F>
where
    M: Clone + Send + 'static,
    F: Fn(DialogResult) -> M + Send + 'static,
{
    ConfirmationDialogWrapper::new(
        ConfirmationDialog::new(title, message)
            .dialog_type(DialogType::Question)
            .buttons(ButtonSet::YesNo),
        on_result,
    )
}

/// Create a Yes/No/Cancel confirmation dialog.
///
/// # Example
/// ```ignore
/// dialog_stack.push(confirm_yes_no_cancel(
///     "Save Changes?",
///     "Do you want to save your changes before closing?",
///     |result| match result {
///         DialogResult::Yes => Message::SaveAndClose,
///         DialogResult::No => Message::CloseWithoutSaving,
///         _ => Message::CancelClose,
///     },
/// ));
/// ```
pub fn confirm_yes_no_cancel<M, F>(title: impl Into<String>, message: impl Into<String>, on_result: F) -> ConfirmationDialogWrapper<M, F>
where
    M: Clone + Send + 'static,
    F: Fn(DialogResult) -> M + Send + 'static,
{
    ConfirmationDialogWrapper::new(
        ConfirmationDialog::new(title, message)
            .dialog_type(DialogType::Question)
            .buttons(ButtonSet::YesNoCancel),
        on_result,
    )
}

/// Create a delete confirmation dialog (Delete/Cancel buttons).
///
/// # Example
/// ```ignore
/// dialog_stack.push(confirm_delete(
///     "Delete File?",
///     "This action cannot be undone.",
///     |result| match result {
///         DialogResult::Delete => Message::PerformDelete,
///         _ => Message::CancelDelete,
///     },
/// ));
/// ```
pub fn confirm_delete<M, F>(title: impl Into<String>, message: impl Into<String>, on_result: F) -> ConfirmationDialogWrapper<M, F>
where
    M: Clone + Send + 'static,
    F: Fn(DialogResult) -> M + Send + 'static,
{
    ConfirmationDialogWrapper::new(
        ConfirmationDialog::new(title, message)
            .dialog_type(DialogType::Warning)
            .buttons(ButtonSet::DeleteCancel),
        on_result,
    )
}

/// Create an overwrite confirmation dialog (Overwrite/Cancel buttons).
///
/// # Example
/// ```ignore
/// dialog_stack.push(confirm_overwrite(
///     "File Exists",
///     "Do you want to overwrite the existing file?",
///     |result| match result {
///         DialogResult::Overwrite => Message::PerformOverwrite,
///         _ => Message::CancelOverwrite,
///     },
/// ));
/// ```
pub fn confirm_overwrite<M, F>(title: impl Into<String>, message: impl Into<String>, on_result: F) -> ConfirmationDialogWrapper<M, F>
where
    M: Clone + Send + 'static,
    F: Fn(DialogResult) -> M + Send + 'static,
{
    ConfirmationDialogWrapper::new(
        ConfirmationDialog::new(title, message)
            .dialog_type(DialogType::Warning)
            .buttons(ButtonSet::OverwriteCancel),
        on_result,
    )
}

/// Create an Ok/Cancel confirmation dialog.
///
/// # Example
/// ```ignore
/// dialog_stack.push(confirm_ok_cancel(
///     "Proceed?",
///     "Are you sure you want to continue?",
///     |result| match result {
///         DialogResult::Ok => Message::Proceed,
///         _ => Message::Cancel,
///     },
/// ));
/// ```
pub fn confirm_ok_cancel<M, F>(title: impl Into<String>, message: impl Into<String>, on_result: F) -> ConfirmationDialogWrapper<M, F>
where
    M: Clone + Send + 'static,
    F: Fn(DialogResult) -> M + Send + 'static,
{
    ConfirmationDialogWrapper::new(
        ConfirmationDialog::new(title, message)
            .dialog_type(DialogType::Plain)
            .buttons(ButtonSet::OkCancel),
        on_result,
    )
}

/// Internal message type for confirmation dialog button clicks
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmationMessage {
    ButtonClicked(DialogResult),
}

impl<M, F> Dialog<M> for ConfirmationDialogWrapper<M, F>
where
    M: Clone + Send + 'static,
    F: Fn(DialogResult) -> M + Send + 'static,
{
    fn view(&self) -> Element<'_, M> {
        self.dialog.view_content(|r| (self.on_result)(r))
    }

    fn update(&mut self, _message: &M) -> Option<DialogAction<M>> {
        // Don't intercept messages - let them pass through to the app.
        // The dialog buttons send messages directly to the app.
        // The app should close the dialog explicitly via dialogs.pop() or
        // by handling the message appropriately.
        //
        // We used to return CloseWith here, but that caused issues because
        // dialogs.update() is called BEFORE the app's message handling,
        // so returning Some would prevent the app from ever seeing the message.
        None
    }

    fn request_cancel(&mut self) -> DialogAction<M> {
        let result = self.dialog.cancel_result();
        DialogAction::CloseWith((self.on_result)(result))
    }

    fn request_confirm(&mut self) -> DialogAction<M> {
        let result = self.dialog.confirm_result();
        DialogAction::CloseWith((self.on_result)(result))
    }

    fn handle_event(&mut self, _event: &Event) -> Option<DialogAction<M>> {
        None
    }

    fn close_on_blur(&self) -> bool {
        true
    }
}
