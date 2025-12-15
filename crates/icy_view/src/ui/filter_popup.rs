use iced::{
    Border, Color, Element, Length, Shadow, Theme,
    alignment::{Horizontal, Vertical},
    widget::{Id, button, container, row, text, text_input},
};

/// Messages for the filter popup
#[derive(Debug, Clone)]
pub enum FilterPopupMessage {
    /// Filter text changed
    FilterChanged(String),
    /// Clear the filter
    ClearFilter,
}

/// Filter popup widget - floating overlay for file filtering
pub struct FilterPopup {
    /// Current filter text
    pub filter: String,
    /// Whether the popup is visible
    pub visible: bool,
    /// Text input ID for focusing
    input_id: Id,
}

impl FilterPopup {
    pub fn new() -> Self {
        Self {
            filter: String::new(),
            visible: false,
            input_id: Id::unique(),
        }
    }

    /// Show the popup
    pub fn show(&mut self) {
        self.visible = true;
    }

    /// Hide the popup
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Check if visible
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Get the current filter
    pub fn get_filter(&self) -> &str {
        &self.filter
    }

    /// Clear the filter
    pub fn clear_filter(&mut self) {
        self.filter.clear();
    }

    /// Focus the input field
    pub fn focus_input<T: 'static>(&self) -> iced::Task<T> {
        iced::Task::batch([
            iced::widget::operation::focus(self.input_id.clone()),
            iced::widget::operation::select_all(self.input_id.clone()),
        ])
    }

    /// Update the popup state
    pub fn update(&mut self, msg: FilterPopupMessage) -> Option<String> {
        match msg {
            FilterPopupMessage::FilterChanged(filter) => {
                self.filter = filter.clone();
                Some(filter)
            }
            FilterPopupMessage::ClearFilter => {
                self.filter.clear();
                Some(String::new())
            }
        }
    }

    /// Render the popup content
    pub fn view(&self) -> Element<'_, FilterPopupMessage> {
        let filter_input = text_input("Filter… (Ctrl+F)", &self.filter)
            .id(self.input_id.clone())
            .on_input(FilterPopupMessage::FilterChanged)
            .padding([6, 10])
            .size(13)
            .width(Length::Fixed(200.0));

        // Clear button - only enabled when filter is not empty
        let clear_btn = if !self.filter.is_empty() {
            button(text("✕").size(12))
                .padding([4, 6])
                .style(popup_button_style)
                .on_press(FilterPopupMessage::ClearFilter)
        } else {
            button(text("✕").size(12)).padding([4, 6]).style(popup_button_disabled_style)
        };

        let content = row![filter_input, clear_btn].spacing(4).padding(8).align_y(Vertical::Center);

        container(content)
            .style(|theme: &Theme| container::Style {
                background: Some(iced::Background::Color(theme.palette().background)),
                border: Border {
                    color: theme.extended_palette().background.strong.color,
                    width: 1.0,
                    radius: 6.0.into(),
                },
                text_color: None,
                shadow: Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.4),
                    offset: iced::Vector::new(2.0, 2.0),
                    blur_radius: 8.0,
                },
                snap: false,
            })
            .width(Length::Shrink)
            .height(Length::Shrink)
            .into()
    }
}

impl Default for FilterPopup {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a filter popup overlay positioned at the top-right
pub fn filter_popup_overlay<'a, T: 'a>(
    popup: &'a FilterPopup,
    content: impl Into<Element<'a, T>>,
    map_msg: impl Fn(FilterPopupMessage) -> T + 'a,
) -> Element<'a, T>
where
    T: Clone,
{
    if !popup.visible {
        return content.into();
    }

    let popup_view = container(popup.view().map(map_msg))
        .align_x(Horizontal::Right)
        .align_y(Vertical::Top)
        .padding([48, 8]) // Below the navigation bar
        .width(Length::Fill)
        .height(Length::Fill);

    iced::widget::stack![content.into(), popup_view].into()
}

fn popup_button_style(theme: &iced::Theme, status: iced::widget::button::Status) -> iced::widget::button::Style {
    let palette = theme.extended_palette();
    let (bg, text_color) = match status {
        iced::widget::button::Status::Active => (palette.background.strong.color, palette.background.strong.text),
        iced::widget::button::Status::Hovered => (palette.primary.weak.color, palette.primary.weak.text),
        iced::widget::button::Status::Pressed => (palette.primary.base.color, palette.primary.base.text),
        iced::widget::button::Status::Disabled => (palette.background.weak.color, palette.background.weak.text.scale_alpha(0.5)),
    };
    iced::widget::button::Style {
        background: Some(iced::Background::Color(bg)),
        text_color,
        border: Border {
            color: palette.background.strong.color,
            width: 1.0,
            radius: 4.0.into(),
        },
        ..Default::default()
    }
}

fn popup_button_disabled_style(theme: &iced::Theme, _status: iced::widget::button::Status) -> iced::widget::button::Style {
    let palette = theme.extended_palette();
    iced::widget::button::Style {
        background: Some(iced::Background::Color(palette.background.weak.color)),
        text_color: palette.background.weak.text.scale_alpha(0.3),
        border: Border {
            color: palette.background.strong.color,
            width: 1.0,
            radius: 4.0.into(),
        },
        ..Default::default()
    }
}
