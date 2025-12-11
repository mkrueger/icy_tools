//! File Settings Dialog
//!
//! Unified dialog for document settings including canvas size, SAUCE metadata,
//! and format compatibility options. Has two pages: settings and comments editor.

use iced::{
    Alignment, Element, Length,
    widget::{Space, checkbox, column, container, pick_list, row, text, text_editor, text_input},
};
use icy_engine::TextPane;
use icy_engine_gui::ButtonType;
use icy_engine_gui::settings::effect_box;
use icy_engine_gui::ui::{
    DIALOG_SPACING, DIALOG_WIDTH_MEDIUM, Dialog, DialogAction, SauceFieldColor, TEXT_SIZE_NORMAL, TEXT_SIZE_SMALL, dialog_area, dialog_title, modal_container,
    primary_button, sauce_input_style, secondary_button, section_header, separator, validated_input_style,
};

use crate::fl;
use crate::ui::Message;

// ============================================================================
// Format Mode
// ============================================================================

/// Document format mode - determines available features
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FormatMode {
    /// Legacy DOS: 16 fixed colors, single font, no palette editing
    #[default]
    LegacyDos,
    /// XBin: 16 colors from selectable palette, single font
    XBin,
    /// XBin Extended: 8 colors, custom palette (first 8), dual fonts
    XBinExtended,
    /// Unrestricted: Full RGB, unlimited fonts
    Unrestricted,
}

impl FormatMode {
    /// All available format modes
    pub const ALL: [FormatMode; 4] = [FormatMode::LegacyDos, FormatMode::XBin, FormatMode::XBinExtended, FormatMode::Unrestricted];

    /// Get the description for this format mode
    pub fn description(&self) -> &'static str {
        match self {
            FormatMode::LegacyDos => "16 fixed colors, single font, no palette editing",
            FormatMode::XBin => "16 colors from selectable palette, single font",
            FormatMode::XBinExtended => "8 colors, dual fonts, custom palette",
            FormatMode::Unrestricted => "Full RGB colors, unlimited fonts",
        }
    }

    /// Check if palette editing is allowed
    pub fn allows_palette_editing(&self) -> bool {
        matches!(self, FormatMode::XBin | FormatMode::XBinExtended | FormatMode::Unrestricted)
    }

    /// Check if font selection is allowed
    pub fn allows_font_selection(&self) -> bool {
        matches!(self, FormatMode::XBinExtended | FormatMode::Unrestricted)
    }

    /// Get maximum number of fonts
    pub fn max_fonts(&self) -> usize {
        match self {
            FormatMode::LegacyDos | FormatMode::XBin => 1,
            FormatMode::XBinExtended => 2,
            FormatMode::Unrestricted => usize::MAX,
        }
    }

    /// Get number of available colors
    pub fn color_count(&self) -> usize {
        match self {
            FormatMode::LegacyDos | FormatMode::XBin => 16,
            FormatMode::XBinExtended => 8,
            FormatMode::Unrestricted => 16777216, // 24-bit RGB
        }
    }
}

impl std::fmt::Display for FormatMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FormatMode::LegacyDos => write!(f, "Legacy DOS"),
            FormatMode::XBin => write!(f, "XBin"),
            FormatMode::XBinExtended => write!(f, "XBin Extended"),
            FormatMode::Unrestricted => write!(f, "Unrestricted"),
        }
    }
}

// ============================================================================
// Dialog Page
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum DialogPage {
    #[default]
    Settings,
    Comments,
}

// ============================================================================
// Dialog Messages
// ============================================================================

/// Messages for the File Settings dialog
#[derive(Debug, Clone)]
pub enum FileSettingsDialogMessage {
    // Canvas
    SetWidth(String),
    SetHeight(String),

    // SAUCE Metadata
    SetTitle(String),
    SetAuthor(String),
    SetGroup(String),

    // Comments editor
    CommentsAction(text_editor::Action),

    // Format
    SetFormatMode(FormatMode),
    SetIceColors(bool),
    Set9pxFont(bool),
    SetLegacyAspect(bool),

    // Page navigation
    ShowComments,
    ShowSettings,

    // Actions
    Apply,
    Cancel,
}

// ============================================================================
// Dialog Result
// ============================================================================

/// Result of the File Settings dialog
#[derive(Debug, Clone)]
pub struct FileSettingsResult {
    /// Canvas width
    pub width: i32,
    /// Canvas height
    pub height: i32,

    /// SAUCE title
    pub title: String,
    /// SAUCE author
    pub author: String,
    /// SAUCE group
    pub group: String,
    /// SAUCE comments
    pub comments: String,

    /// Format mode
    pub format_mode: FormatMode,
    /// Ice colors enabled
    pub ice_colors: bool,
    /// 9px font mode
    pub use_9px_font: bool,
    /// Legacy aspect ratio
    pub legacy_aspect: bool,
}

// ============================================================================
// Dialog State
// ============================================================================

/// State for the File Settings dialog
#[derive(Debug, Clone)]
pub struct FileSettingsDialog {
    // Current page
    page: DialogPage,

    // Canvas
    width: String,
    height: String,

    // SAUCE Metadata
    title: String,
    author: String,
    group: String,

    // Comments editor
    comments_content: text_editor::Content,

    // Format
    format_mode: FormatMode,
    ice_colors: bool,
    use_9px_font: bool,
    legacy_aspect: bool,

    // Original values for comparison
    original_width: i32,
    original_height: i32,
}

impl FileSettingsDialog {
    /// Create a new File Settings dialog with current document values
    pub fn new(
        width: i32,
        height: i32,
        title: String,
        author: String,
        group: String,
        comments: String,
        format_mode: FormatMode,
        ice_colors: bool,
        use_9px_font: bool,
        legacy_aspect: bool,
    ) -> Self {
        Self {
            page: DialogPage::Settings,
            width: width.to_string(),
            height: height.to_string(),
            title,
            author,
            group,
            comments_content: text_editor::Content::with_text(&comments),
            format_mode,
            ice_colors,
            use_9px_font,
            legacy_aspect,
            original_width: width,
            original_height: height,
        }
    }

    /// Create dialog from current edit state
    pub fn from_edit_state(state: &icy_engine_edit::EditState) -> Self {
        let buffer = state.get_buffer();
        let size = buffer.size();

        // Determine format mode from buffer settings
        let format_mode = Self::detect_format_mode(buffer);

        // Get SAUCE metadata from edit state
        let sauce = state.get_sauce_meta();
        let title = sauce.title.to_string();
        let author = sauce.author.to_string();
        let group = sauce.group.to_string();
        let comments = sauce.comments.iter().map(|s| s.to_string()).collect::<Vec<_>>().join("\n");

        Self::new(
            size.width,
            size.height,
            title,
            author,
            group,
            comments,
            format_mode,
            buffer.ice_mode == icy_engine::IceMode::Ice,
            buffer.use_letter_spacing(),
            buffer.use_aspect_ratio(),
        )
    }

    /// Detect format mode from buffer settings
    fn detect_format_mode(buffer: &icy_engine::TextBuffer) -> FormatMode {
        use icy_engine::{FontMode, PaletteMode};

        match (buffer.palette_mode, buffer.font_mode) {
            (PaletteMode::Fixed16, FontMode::Sauce | FontMode::Single) => FormatMode::LegacyDos,
            (PaletteMode::Free16, FontMode::Sauce | FontMode::Single) => FormatMode::XBin,
            (PaletteMode::Free8 | PaletteMode::Free16, FontMode::FixedSize) => FormatMode::XBinExtended,
            (PaletteMode::RGB, _) | (_, FontMode::Unlimited) => FormatMode::Unrestricted,
            _ => FormatMode::LegacyDos,
        }
    }

    /// Parse width value
    fn parsed_width(&self) -> Option<i32> {
        self.width.parse::<i32>().ok().filter(|&w| w >= 1 && w <= icy_engine::limits::MAX_BUFFER_WIDTH)
    }

    /// Parse height value
    fn parsed_height(&self) -> Option<i32> {
        self.height
            .parse::<i32>()
            .ok()
            .filter(|&h| h >= 1 && h <= icy_engine::limits::MAX_BUFFER_HEIGHT)
    }

    /// Check if all inputs are valid
    fn is_valid(&self) -> bool {
        self.parsed_width().is_some() && self.parsed_height().is_some()
    }

    /// Get comments text from editor
    fn get_comments(&self) -> String {
        self.comments_content.text()
    }

    /// Get the result if valid
    fn result(&self) -> Option<FileSettingsResult> {
        Some(FileSettingsResult {
            width: self.parsed_width()?,
            height: self.parsed_height()?,
            title: self.title.clone(),
            author: self.author.clone(),
            group: self.group.clone(),
            comments: self.get_comments(),
            format_mode: self.format_mode,
            ice_colors: self.ice_colors,
            use_9px_font: self.use_9px_font,
            legacy_aspect: self.legacy_aspect,
        })
    }

    /// View for the Settings page
    fn view_settings(&self) -> Element<'_, Message> {
        let title = dialog_title(fl!("file-settings-dialog-title"));

        let label_width = Length::Fixed(80.0);

        // ═══════════════════════════════════════════════════════════════════════
        // Canvas Size: [Width] x [Height]
        // ═══════════════════════════════════════════════════════════════════════
        let width_valid = self.parsed_width().is_some();
        let width_input = text_input("", &self.width)
            .on_input(|s| Message::FileSettingsDialog(FileSettingsDialogMessage::SetWidth(s)))
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fixed(70.0))
            .style(validated_input_style(width_valid));

        let height_valid = self.parsed_height().is_some();
        let height_input = text_input("", &self.height)
            .on_input(|s| Message::FileSettingsDialog(FileSettingsDialogMessage::SetHeight(s)))
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fixed(70.0))
            .style(validated_input_style(height_valid));

        let canvas_row = row![
            container(text(fl!("file-settings-canvas-size")).size(TEXT_SIZE_NORMAL)).width(label_width),
            width_input,
            text(" x ").size(TEXT_SIZE_NORMAL),
            height_input,
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center);

        // ═══════════════════════════════════════════════════════════════════════
        // Format: [PickList] <description>
        // ═══════════════════════════════════════════════════════════════════════
        let format_picker = pick_list(FormatMode::ALL.as_slice(), Some(self.format_mode), |m| {
            Message::FileSettingsDialog(FileSettingsDialogMessage::SetFormatMode(m))
        })
        .width(Length::Fixed(140.0));

        let format_description = text(self.format_mode.description())
            .size(TEXT_SIZE_SMALL)
            .style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.extended_palette().secondary.strong.color),
            });

        let format_row = row![
            container(text(fl!("file-settings-format")).size(TEXT_SIZE_NORMAL)).width(label_width),
            format_picker,
            format_description,
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center);

        // ═══════════════════════════════════════════════════════════════════════
        // SAUCE Section
        // ═══════════════════════════════════════════════════════════════════════
        let title_input = text_input("", &self.title)
            .on_input(|s| Message::FileSettingsDialog(FileSettingsDialogMessage::SetTitle(s)))
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fill)
            .style(sauce_input_style(SauceFieldColor::Title));

        let title_row = row![
            container(text(fl!("file-settings-title")).size(TEXT_SIZE_NORMAL)).width(label_width),
            title_input,
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center);

        let author_input = text_input("", &self.author)
            .on_input(|s| Message::FileSettingsDialog(FileSettingsDialogMessage::SetAuthor(s)))
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fixed(180.0))
            .style(sauce_input_style(SauceFieldColor::Author));

        let author_row = row![
            container(text(fl!("file-settings-author")).size(TEXT_SIZE_NORMAL)).width(label_width),
            author_input,
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center);

        let group_input = text_input("", &self.group)
            .on_input(|s| Message::FileSettingsDialog(FileSettingsDialogMessage::SetGroup(s)))
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fixed(180.0))
            .style(sauce_input_style(SauceFieldColor::Group));

        let group_row = row![
            container(text(fl!("file-settings-group")).size(TEXT_SIZE_NORMAL)).width(label_width),
            group_input,
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center);

        let sauce_section = column![section_header(fl!("file-settings-sauce")), title_row, author_row, group_row,].spacing(4);

        // ═══════════════════════════════════════════════════════════════════════
        // Checkboxes: [Ice] [Legacy AR] on first row, [9px font] below
        // ═══════════════════════════════════════════════════════════════════════
        let ice_checkbox = row![
            checkbox(self.ice_colors)
                .on_toggle(|v| Message::FileSettingsDialog(FileSettingsDialogMessage::SetIceColors(v)))
                .size(16),
            text(fl!("file-settings-ice")).size(TEXT_SIZE_NORMAL),
        ]
        .width(Length::Fixed(120.0))
        .spacing(6)
        .align_y(Alignment::Center);

        let aspect_checkbox = row![
            checkbox(self.legacy_aspect)
                .on_toggle(|v| Message::FileSettingsDialog(FileSettingsDialogMessage::SetLegacyAspect(v)))
                .size(16),
            text(fl!("file-settings-legacy-ar")).size(TEXT_SIZE_NORMAL),
        ]
        .spacing(6)
        .align_y(Alignment::Center);

        let font_9px_checkbox = row![
            checkbox(self.use_9px_font)
                .on_toggle(|v| Message::FileSettingsDialog(FileSettingsDialogMessage::Set9pxFont(v)))
                .size(16),
            text(fl!("file-settings-9px-font")).size(TEXT_SIZE_NORMAL),
        ]
        .spacing(6)
        .align_y(Alignment::Center);

        let checkbox_row1 = row![ice_checkbox, aspect_checkbox].spacing(8).align_y(Alignment::Center);

        let checkbox_row2 = row![font_9px_checkbox].spacing(8).align_y(Alignment::Center);

        let checkboxes = column![checkbox_row1, checkbox_row2].spacing(DIALOG_SPACING);

        // ═══════════════════════════════════════════════════════════════════════
        // Combine all sections
        // ═══════════════════════════════════════════════════════════════════════
        let content_column = column![
            canvas_row,
            format_row,
            Space::new().height(DIALOG_SPACING),
            sauce_section,
            Space::new().height(DIALOG_SPACING),
            checkboxes,
        ]
        .spacing(DIALOG_SPACING);

        let content_box = effect_box(content_column.into());

        let can_apply = self.is_valid();

        // Button row with Comments on the left, Cancel/OK on the right
        let comments_button = secondary_button(
            fl!("file-settings-comments-button"),
            Some(Message::FileSettingsDialog(FileSettingsDialogMessage::ShowComments)),
        );

        let button_row_content = row![
            comments_button,
            Space::new().width(Length::Fill),
            secondary_button(
                format!("{}", ButtonType::Cancel),
                Some(Message::FileSettingsDialog(FileSettingsDialogMessage::Cancel)),
            ),
            primary_button(
                format!("{}", ButtonType::Ok),
                can_apply.then(|| Message::FileSettingsDialog(FileSettingsDialogMessage::Apply)),
            ),
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center);

        let dialog_content = dialog_area(column![title, Space::new().height(DIALOG_SPACING), content_box].into());

        let button_area = dialog_area(button_row_content.into());

        modal_container(
            column![container(dialog_content).height(Length::Shrink), separator(), button_area].into(),
            DIALOG_WIDTH_MEDIUM,
        )
        .into()
    }

    /// View for the Comments page
    fn view_comments(&self) -> Element<'_, Message> {
        let title = dialog_title(fl!("file-settings-comments-title"));

        // Comments info
        let info_text = text(fl!("file-settings-comments-info"))
            .size(TEXT_SIZE_SMALL)
            .style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.extended_palette().secondary.strong.color),
            });

        // Text editor for comments
        let editor = text_editor(&self.comments_content)
            .on_action(|action| Message::FileSettingsDialog(FileSettingsDialogMessage::CommentsAction(action)))
            .height(Length::Fixed(200.0));

        let content_column = column![info_text, Space::new().height(DIALOG_SPACING), editor,].spacing(DIALOG_SPACING);

        let content_box = effect_box(content_column.into());

        let can_apply = self.is_valid();

        // Button row with Settings on the left, Cancel/OK on the right
        let settings_button = secondary_button(
            fl!("file-settings-settings-button"),
            Some(Message::FileSettingsDialog(FileSettingsDialogMessage::ShowSettings)),
        );

        let button_row_content = row![
            settings_button,
            Space::new().width(Length::Fill),
            secondary_button(
                format!("{}", ButtonType::Cancel),
                Some(Message::FileSettingsDialog(FileSettingsDialogMessage::Cancel)),
            ),
            primary_button(
                format!("{}", ButtonType::Ok),
                can_apply.then(|| Message::FileSettingsDialog(FileSettingsDialogMessage::Apply)),
            ),
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center);

        let dialog_content = dialog_area(column![title, Space::new().height(DIALOG_SPACING), content_box].into());

        let button_area = dialog_area(button_row_content.into());

        modal_container(
            column![container(dialog_content).height(Length::Shrink), separator(), button_area].into(),
            DIALOG_WIDTH_MEDIUM,
        )
        .into()
    }
}

impl Dialog<Message> for FileSettingsDialog {
    fn view(&self) -> Element<'_, Message> {
        match self.page {
            DialogPage::Settings => self.view_settings(),
            DialogPage::Comments => self.view_comments(),
        }
    }

    fn update(&mut self, message: &Message) -> Option<DialogAction<Message>> {
        let Message::FileSettingsDialog(msg) = message else {
            return None;
        };

        match msg {
            // Canvas
            FileSettingsDialogMessage::SetWidth(w) => {
                self.width = w.clone();
                Some(DialogAction::None)
            }
            FileSettingsDialogMessage::SetHeight(h) => {
                self.height = h.clone();
                Some(DialogAction::None)
            }

            // SAUCE (ASCII only, with length limits)
            FileSettingsDialogMessage::SetTitle(t) => {
                // SAUCE title: ASCII only, max length from icy_sauce::limits
                self.title = t.chars().filter(|c| c.is_ascii()).take(icy_sauce::limits::MAX_TITLE_LENGTH).collect();
                Some(DialogAction::None)
            }
            FileSettingsDialogMessage::SetAuthor(a) => {
                // SAUCE author: ASCII only, max length from icy_sauce::limits
                self.author = a.chars().filter(|c| c.is_ascii()).take(icy_sauce::limits::MAX_AUTHOR_LENGTH).collect();
                Some(DialogAction::None)
            }
            FileSettingsDialogMessage::SetGroup(g) => {
                // SAUCE group: ASCII only, max length from icy_sauce::limits
                self.group = g.chars().filter(|c| c.is_ascii()).take(icy_sauce::limits::MAX_GROUP_LENGTH).collect();
                Some(DialogAction::None)
            }

            // Comments editor action
            FileSettingsDialogMessage::CommentsAction(action) => {
                // Check if action would violate limits before applying
                let is_insert = matches!(action, text_editor::Action::Edit(_));

                if is_insert {
                    // Check current state before insert
                    let text = self.comments_content.text();
                    let lines: Vec<&str> = text.lines().collect();
                    let line_count = lines.len();
                    let max_line_len = lines.iter().map(|l| l.len()).max().unwrap_or(0);

                    // If already at limits, only allow non-insert actions
                    if line_count >= icy_sauce::limits::MAX_COMMENTS || max_line_len >= icy_sauce::limits::MAX_COMMENT_LENGTH {
                        // Still apply the action, but check afterwards
                        self.comments_content.perform(action.clone());

                        // Verify we're still within limits
                        let new_text = self.comments_content.text();
                        let new_lines: Vec<&str> = new_text.lines().collect();
                        let exceeds_limits =
                            new_lines.len() > icy_sauce::limits::MAX_COMMENTS || new_lines.iter().any(|l| l.len() > icy_sauce::limits::MAX_COMMENT_LENGTH);

                        if exceeds_limits {
                            // Rebuild with enforced limits (ASCII only)
                            let limited: String = new_lines
                                .iter()
                                .take(icy_sauce::limits::MAX_COMMENTS)
                                .map(|line| {
                                    line.chars()
                                        .filter(|c| c.is_ascii())
                                        .take(icy_sauce::limits::MAX_COMMENT_LENGTH)
                                        .collect::<String>()
                                })
                                .collect::<Vec<_>>()
                                .join("\n");
                            self.comments_content = text_editor::Content::with_text(&limited);
                        }
                    } else {
                        self.comments_content.perform(action.clone());
                    }
                } else {
                    // Non-insert actions (cursor movement, selection, delete) are always allowed
                    self.comments_content.perform(action.clone());
                }

                Some(DialogAction::None)
            }

            // Page navigation
            FileSettingsDialogMessage::ShowComments => {
                self.page = DialogPage::Comments;
                Some(DialogAction::None)
            }
            FileSettingsDialogMessage::ShowSettings => {
                self.page = DialogPage::Settings;
                Some(DialogAction::None)
            }

            // Format
            FileSettingsDialogMessage::SetFormatMode(mode) => {
                self.format_mode = *mode;
                Some(DialogAction::None)
            }
            FileSettingsDialogMessage::SetIceColors(v) => {
                self.ice_colors = *v;
                Some(DialogAction::None)
            }
            FileSettingsDialogMessage::Set9pxFont(v) => {
                self.use_9px_font = *v;
                Some(DialogAction::None)
            }
            FileSettingsDialogMessage::SetLegacyAspect(v) => {
                self.legacy_aspect = *v;
                Some(DialogAction::None)
            }

            // Actions
            FileSettingsDialogMessage::Apply => {
                if let Some(result) = self.result() {
                    Some(DialogAction::CloseWith(Message::ApplyFileSettings(result)))
                } else {
                    Some(DialogAction::None)
                }
            }
            FileSettingsDialogMessage::Cancel => Some(DialogAction::Close),
        }
    }

    fn request_cancel(&mut self) -> DialogAction<Message> {
        DialogAction::Close
    }

    fn request_confirm(&mut self) -> DialogAction<Message> {
        if self.is_valid() {
            if let Some(result) = self.result() {
                return DialogAction::CloseWith(Message::ApplyFileSettings(result));
            }
        }
        DialogAction::None
    }
}
