use clipboard_rs::{Clipboard, ClipboardContent};
use i18n_embed_fl::fl;
use iced::{
    Alignment, Background, Element, Length, Theme,
    widget::{Space, column, container, row, text, tooltip},
};
use icy_engine::{Position, ScreenMode, Size, TerminalScrolling};
use icy_engine_gui::StateResult;
use icy_engine_gui::dialog_wrapper;
use icy_engine_gui::settings::effect_box;
use icy_engine_gui::ui::{
    DIALOG_SPACING, DIALOG_WIDTH_LARGE, SECTION_SPACING, SPACE_4, TEXT_SIZE_NORMAL, TEXT_SIZE_SMALL, button_row_with_left, dialog_area, modal_container,
    primary_button, secondary_button, section_header, separator,
};
use icy_net::telnet::TerminalEmulation;
use icy_parser_core::{BaudEmulation, CaretShape, MusicOption};

use super::terminal_settings_ui::{self, TerminalSettings, TerminalSettingsChange};

const LABEL_WIDTH: f32 = 130.0;
const VALUE_WIDTH: f32 = 120.0;

/// Result from the terminal info dialog when applying settings
#[derive(Debug, Clone)]
pub struct TerminalSettingsResult {
    pub terminal_type: TerminalEmulation,
    pub screen_mode: ScreenMode,
    pub ansi_music: MusicOption,
}

#[derive(Debug, Clone)]
pub enum TerminalInfoDialogMessage {
    Close,
    CopyToClipboard,
    SettingsChanged(TerminalSettingsChange),
    Apply,
}

/// Information about the current terminal state
#[derive(Debug, Clone)]
pub struct TerminalInfo {
    pub buffer_size: Size,
    pub screen_resolution: Size,
    pub font_size: Size,
    pub caret_position: Position,
    pub caret_visible: bool,
    pub caret_blinking: bool,
    pub caret_shape: CaretShape,
    pub insert_mode: bool,
    pub auto_wrap: bool,
    pub scroll_mode: TerminalScrolling,
    pub margins_top_bottom: Option<(i32, i32)>,
    pub margins_left_right: Option<(i32, i32)>,
    pub mouse_mode: String,
    pub inverse_mode: bool,
    pub ice_colors: bool,
    pub baud_emulation: BaudEmulation,
    // Terminal settings
    pub terminal_type: TerminalEmulation,
    pub screen_mode: ScreenMode,
    pub ansi_music: MusicOption,
}

impl Default for TerminalInfo {
    fn default() -> Self {
        Self {
            buffer_size: Size::new(80, 25),
            screen_resolution: Size::new(640, 400),
            font_size: Size::new(8, 16),
            caret_position: Position::default(),
            caret_visible: true,
            caret_blinking: true,
            caret_shape: CaretShape::Block,
            insert_mode: false,
            auto_wrap: true,
            scroll_mode: TerminalScrolling::Smooth,
            margins_top_bottom: None,
            margins_left_right: None,
            mouse_mode: "Off".to_string(),
            inverse_mode: false,
            ice_colors: false,
            baud_emulation: BaudEmulation::Off,
            terminal_type: TerminalEmulation::Ansi,
            screen_mode: ScreenMode::default(),
            ansi_music: MusicOption::Off,
        }
    }
}

#[dialog_wrapper(close_on_blur = true, result_type = TerminalSettingsResult)]
pub struct TerminalInfoDialog {
    pub info: TerminalInfo,
    // Editable settings
    pub selected_terminal_type: TerminalEmulation,
    pub selected_screen_mode: ScreenMode,
    pub selected_ansi_music: MusicOption,
}

impl TerminalInfoDialog {
    pub fn new(info: TerminalInfo) -> Self {
        Self {
            selected_terminal_type: info.terminal_type,
            selected_screen_mode: info.screen_mode,
            selected_ansi_music: info.ansi_music,
            info,
        }
    }

    pub fn handle_message(&mut self, message: TerminalInfoDialogMessage) -> StateResult<TerminalSettingsResult> {
        match message {
            TerminalInfoDialogMessage::Close => StateResult::Close,
            TerminalInfoDialogMessage::CopyToClipboard => {
                let text = self.format_info_text();
                if let Err(err) = crate::CLIPBOARD_CONTEXT.set(vec![ClipboardContent::Text(text)]) {
                    log::error!("Failed to copy to clipboard: {err}");
                }
                StateResult::None
            }
            TerminalInfoDialogMessage::SettingsChanged(change) => {
                match change {
                    TerminalSettingsChange::TerminalType(t) => {
                        self.selected_terminal_type = t;
                        // Always reset screen mode when terminal type changes
                        self.selected_screen_mode = terminal_settings_ui::get_default_screen_mode(self.selected_terminal_type);
                    }
                    TerminalSettingsChange::ScreenMode(mode) => {
                        self.selected_screen_mode = mode;
                    }
                    TerminalSettingsChange::AnsiMusic(music) => {
                        self.selected_ansi_music = music;
                    }
                }
                StateResult::None
            }
            TerminalInfoDialogMessage::Apply => StateResult::Success(TerminalSettingsResult {
                terminal_type: self.selected_terminal_type,
                screen_mode: self.selected_screen_mode,
                ansi_music: self.selected_ansi_music,
            }),
        }
    }

    pub fn has_changes(&self) -> bool {
        self.selected_terminal_type != self.info.terminal_type
            || self.selected_screen_mode != self.info.screen_mode
            || self.selected_ansi_music != self.info.ansi_music
    }

    fn format_info_text(&self) -> String {
        let margins_str = match (self.info.margins_top_bottom, self.info.margins_left_right) {
            (Some((t, b)), Some((l, r))) => format!("Lines {}→{} • Cols {}→{}", t, b, l, r),
            (Some((t, b)), None) => format!("Lines {}→{}", t, b),
            (None, Some((l, r))) => format!("Cols {}→{}", l, r),
            (None, None) => "Not Set".to_string(),
        };

        let scroll_mode_str = match self.info.scroll_mode {
            TerminalScrolling::Smooth => "Smooth",
            TerminalScrolling::Fast => "Fast",
            TerminalScrolling::Disabled => "Disabled",
        };

        let caret_shape_str = match self.info.caret_shape {
            CaretShape::Block => "Block",
            CaretShape::Underline => "Underline",
            CaretShape::Bar => "Bar",
        };

        format!(
            "Terminal Information\n\
             =====================\n\
             \n\
             Terminal:\n\
             - Size: {}x{} ({}x{} px)\n\
             - Font Size: {}x{}\n\
             - Auto Wrap: {}\n\
             - Scroll Mode: {}\n\
             - Margins: {}\n\
             - Mouse Tracking: {}\n\
             - Inverse Colors: {}\n\
             - ICE Colors: {}\n\
             \n\
             Caret:\n\
             - Position: X: {}, Y: {}\n\
             - Shape: {}\n\
             - Visible: {}\n\
             - Blinking: {}\n\
             - Input Mode: {}\n",
            self.info.buffer_size.width,
            self.info.buffer_size.height,
            self.info.screen_resolution.width,
            self.info.screen_resolution.height,
            self.info.font_size.width,
            self.info.font_size.height,
            if self.info.auto_wrap { "Yes" } else { "No" },
            scroll_mode_str,
            margins_str,
            self.info.mouse_mode,
            if self.info.inverse_mode { "Yes" } else { "No" },
            if self.info.ice_colors { "Yes" } else { "No" },
            self.info.caret_position.x,
            self.info.caret_position.y,
            caret_shape_str,
            if self.info.caret_visible { "Yes" } else { "No" },
            if self.info.caret_blinking { "Yes" } else { "No" },
            if self.info.insert_mode { "Insert" } else { "Overwrite" },
        )
    }

    pub fn view<'a, M: Clone + 'static>(&'a self, on_message: impl Fn(TerminalInfoDialogMessage) -> M + 'static + Clone) -> Element<'a, M> {
        self.create_modal_content(on_message)
    }

    fn create_row<M: Clone + 'static>(label: String, value: String) -> Element<'static, M> {
        row![
            text(label).size(TEXT_SIZE_NORMAL).width(Length::Fixed(LABEL_WIDTH)),
            text(value)
                .size(TEXT_SIZE_NORMAL)
                .width(Length::Fixed(VALUE_WIDTH))
                .style(|theme: &Theme| text::Style {
                    color: Some(theme.palette().text.scale_alpha(0.7)),
                }),
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center)
        .into()
    }

    fn create_row_with_mouse_mode_tooltip<M: Clone + 'static>(
        label: String,
        value: String,
        current_mode: String,
        mode_descriptions: [(String, String); 6],
    ) -> Element<'static, M> {
        let current_mode_lower = current_mode.to_lowercase();
        let tooltip_rows: Vec<Element<'static, M>> = mode_descriptions
            .into_iter()
            .map(|(mode, desc)| {
                let is_current = mode.to_lowercase() == current_mode_lower;
                let alpha = if is_current { 1.0 } else { 0.5 };
                row![
                    text(format!("{:<16}", mode))
                        .size(TEXT_SIZE_SMALL)
                        .font(iced::Font::MONOSPACE)
                        .style(move |theme: &Theme| text::Style {
                            color: Some(theme.palette().text.scale_alpha(alpha)),
                        }),
                    text(desc).size(TEXT_SIZE_SMALL).style(move |theme: &Theme| text::Style {
                        color: Some(theme.palette().text.scale_alpha(alpha)),
                    }),
                ]
                .spacing(8)
                .into()
            })
            .collect();

        let tooltip_content = iced::widget::Column::with_children(tooltip_rows).spacing(2);

        row![
            text(label).size(TEXT_SIZE_NORMAL).width(Length::Fixed(LABEL_WIDTH)),
            tooltip(
                text(value)
                    .size(TEXT_SIZE_NORMAL)
                    .font(iced::Font::MONOSPACE)
                    .width(Length::Fixed(VALUE_WIDTH))
                    .style(|theme: &Theme| text::Style {
                        color: Some(theme.palette().text.scale_alpha(0.7)),
                    }),
                container(tooltip_content).padding(8).style(container::rounded_box),
                tooltip::Position::Top,
            )
            .gap(5),
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center)
        .into()
    }

    fn create_size_row<M: Clone + 'static>(label: String, cols: i32, rows: i32, px_width: i32, px_height: i32) -> Element<'static, M> {
        row![
            text(label).size(TEXT_SIZE_NORMAL).width(Length::Fixed(LABEL_WIDTH)),
            row![
                text(format!("{}x{}", cols, rows)).size(TEXT_SIZE_NORMAL).style(|theme: &Theme| text::Style {
                    color: Some(theme.palette().text.scale_alpha(0.7)),
                }),
                text(format!("({}x{} px)", px_width, px_height))
                    .size(TEXT_SIZE_SMALL)
                    .style(|theme: &Theme| text::Style {
                        color: Some(theme.palette().text.scale_alpha(0.4)),
                    }),
            ]
            .width(Length::Fixed(VALUE_WIDTH))
            .align_y(Alignment::Center)
            .spacing(4),
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center)
        .into()
    }

    fn create_caret_shape_row_with_tooltip<M: Clone + 'static>(
        label: String,
        current_shape: CaretShape,
        shape_descriptions: [(String, String); 3],
    ) -> Element<'static, M> {
        let (symbol, name) = match current_shape {
            CaretShape::Block => ("█", "Block"),
            CaretShape::Underline => ("▁", "Underline"),
            CaretShape::Bar => ("▏", "Bar"),
        };

        let shapes: [(CaretShape, &str); 3] = [(CaretShape::Block, "█"), (CaretShape::Underline, "▁"), (CaretShape::Bar, "▏")];

        let tooltip_rows: Vec<Element<'static, M>> = shapes
            .iter()
            .zip(shape_descriptions.into_iter())
            .map(|((shape, sym), (shape_name, desc))| {
                let is_current = *shape == current_shape;
                let alpha = if is_current { 1.0 } else { 0.5 };
                row![
                    text(*sym).size(TEXT_SIZE_SMALL).style(move |theme: &Theme| text::Style {
                        color: Some(theme.palette().text.scale_alpha(alpha)),
                    }),
                    text(format!("{:<12}", shape_name))
                        .size(TEXT_SIZE_SMALL)
                        .font(iced::Font::MONOSPACE)
                        .style(move |theme: &Theme| text::Style {
                            color: Some(theme.palette().text.scale_alpha(alpha)),
                        }),
                    text(desc).size(TEXT_SIZE_SMALL).style(move |theme: &Theme| text::Style {
                        color: Some(theme.palette().text.scale_alpha(alpha)),
                    }),
                ]
                .spacing(8)
                .into()
            })
            .collect();

        let tooltip_content = iced::widget::Column::with_children(tooltip_rows).spacing(2);

        row![
            text(label).size(TEXT_SIZE_NORMAL).width(Length::Fixed(LABEL_WIDTH)),
            tooltip(
                row![
                    text(symbol).size(TEXT_SIZE_NORMAL).font(iced::Font::MONOSPACE),
                    Space::new().width(6.0),
                    text(name)
                        .size(TEXT_SIZE_NORMAL)
                        .font(iced::Font::MONOSPACE)
                        .style(|theme: &Theme| text::Style {
                            color: Some(theme.palette().text.scale_alpha(0.7)),
                        }),
                ]
                .align_y(Alignment::Center),
                container(tooltip_content).padding(8).style(container::rounded_box),
                tooltip::Position::Top,
            )
            .gap(5),
        ]
        .spacing(DIALOG_SPACING)
        .align_y(Alignment::Center)
        .into()
    }

    fn vertical_separator<'a, M: 'a>() -> Element<'a, M> {
        container(Space::new())
            .width(1.0)
            .height(Length::Fill)
            .style(|theme: &Theme| container::Style {
                background: Some(Background::Color(theme.palette().text.scale_alpha(0.15))),
                ..Default::default()
            })
            .into()
    }

    fn create_modal_content<'a, M: Clone + 'static>(&'a self, on_message: impl Fn(TerminalInfoDialogMessage) -> M + 'static + Clone) -> Element<'a, M> {
        // Get translations
        let terminal_title = fl!(crate::LANGUAGE_LOADER, "terminal-info-dialog-terminal-section");
        let caret_title = fl!(crate::LANGUAGE_LOADER, "terminal-info-dialog-caret-section");
        let settings_title = fl!(crate::LANGUAGE_LOADER, "settings-heading");
        let resolution_label = fl!(crate::LANGUAGE_LOADER, "terminal-info-dialog-resolution");
        let font_size_label = fl!(crate::LANGUAGE_LOADER, "terminal-info-dialog-font-size");
        let caret_shape_label = fl!(crate::LANGUAGE_LOADER, "terminal-info-dialog-caret-shape");
        let caret_position_label = fl!(crate::LANGUAGE_LOADER, "terminal-info-dialog-caret-position");
        let caret_visible_label = fl!(crate::LANGUAGE_LOADER, "terminal-info-dialog-caret-visible");
        let caret_blinking_label = fl!(crate::LANGUAGE_LOADER, "terminal-info-dialog-caret-blinking");
        let input_mode_label = fl!(crate::LANGUAGE_LOADER, "terminal-info-dialog-input-mode");
        let auto_wrap_label = fl!(crate::LANGUAGE_LOADER, "terminal-info-dialog-auto-wrap");
        let scroll_mode_label = fl!(crate::LANGUAGE_LOADER, "terminal-info-dialog-scroll-mode");
        let margins_label = fl!(crate::LANGUAGE_LOADER, "terminal-info-dialog-margins");
        let mouse_tracking_label = fl!(crate::LANGUAGE_LOADER, "terminal-info-dialog-mouse-tracking");
        let inverse_colors_label = fl!(crate::LANGUAGE_LOADER, "terminal-info-dialog-inverse-colors");
        let ice_colors_label = fl!(crate::LANGUAGE_LOADER, "terminal-info-dialog-use-ice-colors");
        let yes_str = fl!(crate::LANGUAGE_LOADER, "terminal-info-dialog-yes");
        let no_str = fl!(crate::LANGUAGE_LOADER, "terminal-info-dialog-no");
        let not_set_str = fl!(crate::LANGUAGE_LOADER, "terminal-info-dialog-not-set");
        let copy_button_label = fl!(crate::LANGUAGE_LOADER, "terminal-info-dialog-copy-button");
        let apply_button_label = fl!(crate::LANGUAGE_LOADER, "terminal-info-dialog-apply-button");

        // Mouse mode tooltip descriptions
        let mouse_mode_descriptions: [(String, String); 6] = [
            ("Off".to_string(), fl!(crate::LANGUAGE_LOADER, "terminal-info-dialog-mouse-mode-tooltip-off")),
            ("X10".to_string(), fl!(crate::LANGUAGE_LOADER, "terminal-info-dialog-mouse-mode-tooltip-x10")),
            (
                "VT200".to_string(),
                fl!(crate::LANGUAGE_LOADER, "terminal-info-dialog-mouse-mode-tooltip-vt200"),
            ),
            (
                "VT200Highlight".to_string(),
                fl!(crate::LANGUAGE_LOADER, "terminal-info-dialog-mouse-mode-tooltip-vt200highlight"),
            ),
            (
                "ButtonEvent".to_string(),
                fl!(crate::LANGUAGE_LOADER, "terminal-info-dialog-mouse-mode-tooltip-btnevent"),
            ),
            (
                "AnyEvent".to_string(),
                fl!(crate::LANGUAGE_LOADER, "terminal-info-dialog-mouse-mode-tooltip-anyevent"),
            ),
        ];

        // Caret shape tooltip descriptions
        let caret_shape_descriptions: [(String, String); 3] = [
            ("Block".to_string(), fl!(crate::LANGUAGE_LOADER, "terminal-info-dialog-shape-tooltip-block")),
            (
                "Underline".to_string(),
                fl!(crate::LANGUAGE_LOADER, "terminal-info-dialog-shape-tooltip-underline"),
            ),
            ("Bar".to_string(), fl!(crate::LANGUAGE_LOADER, "terminal-info-dialog-shape-tooltip-bar")),
        ];

        // Left column - Terminal state
        let margins_str = match (self.info.margins_top_bottom, self.info.margins_left_right) {
            (Some((t, b)), Some((l, r))) => format!("Lines {}→{} • Cols {}→{}", t, b, l, r),
            (Some((t, b)), None) => format!("Lines {}→{}", t, b),
            (None, Some((l, r))) => format!("Cols {}→{}", l, r),
            (None, None) => not_set_str,
        };

        let scroll_mode_str = match self.info.scroll_mode {
            TerminalScrolling::Smooth => "Smooth",
            TerminalScrolling::Fast => "Fast",
            TerminalScrolling::Disabled => "Disabled",
        };

        let left_col = column![
            section_header(terminal_title),
            Space::new().height(SPACE_4),
            Self::create_size_row::<M>(
                resolution_label,
                self.info.buffer_size.width,
                self.info.buffer_size.height,
                self.info.screen_resolution.width,
                self.info.screen_resolution.height
            ),
            Self::create_row::<M>(font_size_label, format!("{}x{}", self.info.font_size.width, self.info.font_size.height)),
            Self::create_row::<M>(auto_wrap_label, if self.info.auto_wrap { yes_str.clone() } else { no_str.clone() }),
            Self::create_row::<M>(scroll_mode_label, scroll_mode_str.to_string()),
            Self::create_row::<M>(margins_label, margins_str),
            Self::create_row_with_mouse_mode_tooltip::<M>(
                mouse_tracking_label,
                self.info.mouse_mode.clone(),
                self.info.mouse_mode.clone(),
                mouse_mode_descriptions
            ),
            Self::create_row::<M>(inverse_colors_label, if self.info.inverse_mode { yes_str.clone() } else { no_str.clone() }),
            Self::create_row::<M>(ice_colors_label, if self.info.ice_colors { yes_str.clone() } else { no_str.clone() }),
        ]
        .spacing(SPACE_4);

        // Right column - Caret state with visual shape display
        let right_col = column![
            section_header(caret_title),
            Space::new().height(SPACE_4),
            Self::create_row::<M>(
                caret_position_label,
                format!("X: {}, Y: {}", self.info.caret_position.x, self.info.caret_position.y)
            ),
            Self::create_caret_shape_row_with_tooltip::<M>(caret_shape_label, self.info.caret_shape, caret_shape_descriptions),
            Self::create_row::<M>(caret_visible_label, if self.info.caret_visible { yes_str.clone() } else { no_str.clone() }),
            Self::create_row::<M>(caret_blinking_label, if self.info.caret_blinking { yes_str.clone() } else { no_str.clone() }),
            Self::create_row::<M>(
                input_mode_label,
                if self.info.insert_mode {
                    "Insert".to_string()
                } else {
                    "Overwrite".to_string()
                }
            ),
        ]
        .spacing(SPACE_4);

        // Two columns layout with vertical separator
        let info_content = row![
            left_col,
            Space::new().width(DIALOG_SPACING),
            Self::vertical_separator::<M>(),
            Space::new().width(DIALOG_SPACING),
            right_col,
        ]
        .align_y(Alignment::Start);

        // Settings section using shared helper wrapped in effect_box
        // Fixed height to prevent dialog jumping (3 rows * row_height + spacing)
        let settings = TerminalSettings {
            terminal_type: self.selected_terminal_type,
            screen_mode: self.selected_screen_mode,
            ansi_music: self.selected_ansi_music,
        };
        let on_msg = on_message.clone();
        let settings_ui = terminal_settings_ui::build_terminal_settings_ui(&settings, move |change| on_msg(TerminalInfoDialogMessage::SettingsChanged(change)));
        let settings_box_content = container(settings_ui).height(Length::Fixed(90.0));
        let settings_content = column![
            section_header(settings_title),
            Space::new().height(SPACE_4),
            effect_box(settings_box_content.into()),
        ]
        .spacing(SPACE_4);

        // Main content with info and settings sections
        let content = column![info_content, Space::new().height(SECTION_SPACING), settings_content,];

        // Footer with buttons
        let on_msg = on_message.clone();
        let copy_btn = secondary_button(copy_button_label, Some(on_msg(TerminalInfoDialogMessage::CopyToClipboard)));

        // Apply button is only enabled if there are changes
        let on_msg = on_message.clone();
        let apply_btn = if self.has_changes() {
            primary_button(apply_button_label, Some(on_msg(TerminalInfoDialogMessage::Apply)))
        } else {
            secondary_button(apply_button_label, None)
        };

        let on_msg = on_message.clone();
        let close_btn = secondary_button(format!("{}", icy_engine_gui::ButtonType::Close), Some(on_msg(TerminalInfoDialogMessage::Close)));

        let buttons = button_row_with_left(vec![copy_btn.into()], vec![apply_btn.into(), close_btn.into()]);

        let dialog_content = dialog_area(content.into());
        let button_area = dialog_area(buttons.into());

        let modal = modal_container(
            column![container(dialog_content).height(Length::Shrink), separator(), button_area,].into(),
            DIALOG_WIDTH_LARGE,
        );

        container(modal)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    }
}

/// Create a terminal info dialog for the dialog stack
pub fn terminal_info_dialog_from_msg<M, F, E>(info: TerminalInfo, (on_message, extract_message): (F, E)) -> TerminalInfoDialogWrapper<M, F, E>
where
    M: Clone + Send + 'static,
    F: Fn(TerminalInfoDialogMessage) -> M + Clone + 'static,
    E: Fn(&M) -> Option<&TerminalInfoDialogMessage> + Clone + 'static,
{
    TerminalInfoDialogWrapper::new(TerminalInfoDialog::new(info), on_message, extract_message)
}
