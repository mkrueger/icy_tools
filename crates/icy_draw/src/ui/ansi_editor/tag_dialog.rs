use iced::{
    Element, Length,
    widget::{Space, column, container, pick_list, row, text, text_input},
};
use icy_engine::{Position, TagPlacement};
use icy_engine_gui::settings::{effect_box, left_label};
use icy_engine_gui::ui::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TagPlacementChoice {
    InText,
    WithGotoXY,
}

impl TagPlacementChoice {
    pub const ALL: [TagPlacementChoice; 2] = [TagPlacementChoice::InText, TagPlacementChoice::WithGotoXY];

    pub fn to_engine(self) -> TagPlacement {
        match self {
            TagPlacementChoice::InText => TagPlacement::InText,
            TagPlacementChoice::WithGotoXY => TagPlacement::WithGotoXY,
        }
    }
}

impl std::fmt::Display for TagPlacementChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TagPlacementChoice::InText => write!(f, "In text"),
            TagPlacementChoice::WithGotoXY => write!(f, "With GotoXY"),
        }
    }
}

#[derive(Clone, Debug)]
pub enum TagDialogMessage {
    SetPreview(String),
    SetReplacement(String),
    SetPosX(String),
    SetPosY(String),
    SetPlacement(TagPlacementChoice),
    Ok,
    Cancel,
}

#[derive(Clone, Debug)]
pub struct TagDialog {
    pub position: Position,
    pub pos_x: String,
    pub pos_y: String,
    pub placement: TagPlacementChoice,
    pub preview: String,
    pub replacement_value: String,
}

impl TagDialog {
    pub fn new(position: Position) -> Self {
        Self {
            position,
            pos_x: position.x.to_string(),
            pos_y: position.y.to_string(),
            placement: TagPlacementChoice::InText,
            preview: "TAG".to_string(),
            replacement_value: String::new(),
        }
    }

    pub fn view(&self) -> Element<'_, TagDialogMessage> {
        let title = dialog_title("Tag".to_string());

        let preview_row = row![
            left_label("Preview".to_string()),
            text_input("", &self.preview)
                .size(TEXT_SIZE_NORMAL)
                .width(Length::Fill)
                .on_input(TagDialogMessage::SetPreview),
        ]
        .spacing(DIALOG_SPACING)
        .align_y(iced::Alignment::Center);

        let replacement_row = row![
            left_label("Replacement".to_string()),
            text_input("", &self.replacement_value)
                .size(TEXT_SIZE_NORMAL)
                .width(Length::Fill)
                .on_input(TagDialogMessage::SetReplacement),
        ]
        .spacing(DIALOG_SPACING)
        .align_y(iced::Alignment::Center);

        let pos_x_input = text_input("", &self.pos_x)
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fixed(70.0))
            .on_input(TagDialogMessage::SetPosX);
        let pos_y_input = text_input("", &self.pos_y)
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fixed(70.0))
            .on_input(TagDialogMessage::SetPosY);

        let pos_row = row![left_label("Position".to_string()), pos_x_input, text(",").size(TEXT_SIZE_NORMAL), pos_y_input,]
            .spacing(DIALOG_SPACING)
            .align_y(iced::Alignment::Center);

        let placement_row = row![
            left_label("Placement".to_string()),
            pick_list(TagPlacementChoice::ALL.as_slice(), Some(self.placement), TagDialogMessage::SetPlacement).width(Length::Fixed(140.0)),
        ]
        .spacing(DIALOG_SPACING)
        .align_y(iced::Alignment::Center);

        let content = column![
            title,
            Space::new().height(DIALOG_SPACING),
            effect_box(column![preview_row, replacement_row, pos_row, placement_row].spacing(DIALOG_SPACING).into()),
        ]
        .spacing(0);

        let dialog_content: Element<'_, TagDialogMessage> = dialog_area(container(content).width(Length::Fill).into());

        let buttons = button_row(vec![
            secondary_button("Cancel", Some(TagDialogMessage::Cancel)).into(),
            primary_button("OK", Some(TagDialogMessage::Ok)).into(),
        ]);
        let button_area: Element<'_, TagDialogMessage> = dialog_area(buttons);

        modal_container(column![dialog_content, separator(), button_area].into(), DIALOG_WIDTH_MEDIUM).into()
    }
}
