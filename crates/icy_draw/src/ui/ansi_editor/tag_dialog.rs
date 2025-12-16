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
    /// If Some(index), we are editing an existing tag at that index
    pub edit_index: Option<usize>,
    /// If Some(length), the tag was created from a selection with this width
    pub length: Option<usize>,
    /// If true, this tag was created from a selection
    pub from_selection: bool,
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
            edit_index: None,
            length: None,
            from_selection: false,
        }
    }

    /// Create a dialog from a selection with extracted text
    pub fn new_from_selection(position: Position, preview_text: String, selection_width: usize) -> Self {
        let preview = if preview_text.trim().is_empty() {
            "TAG".to_string()
        } else {
            preview_text
        };
        Self {
            position,
            pos_x: position.x.to_string(),
            pos_y: position.y.to_string(),
            placement: TagPlacementChoice::InText,
            preview,
            replacement_value: String::new(),
            edit_index: None,
            length: Some(selection_width),
            from_selection: true,
        }
    }

    /// Create a dialog for editing an existing tag
    pub fn edit(tag: &icy_engine::Tag, index: usize) -> Self {
        let placement = match tag.tag_placement {
            TagPlacement::InText => TagPlacementChoice::InText,
            TagPlacement::WithGotoXY => TagPlacementChoice::WithGotoXY,
        };
        Self {
            position: tag.position,
            pos_x: tag.position.x.to_string(),
            pos_y: tag.position.y.to_string(),
            placement,
            preview: tag.preview.clone(),
            replacement_value: tag.replacement_value.clone(),
            edit_index: Some(index),
            length: if tag.length > 0 { Some(tag.length) } else { None },
            from_selection: false,
        }
    }

    /// Returns true if this dialog is editing an existing tag
    pub fn is_editing(&self) -> bool {
        self.edit_index.is_some()
    }

    pub fn view(&self) -> Element<'_, TagDialogMessage> {
        let title_text = if self.is_editing() {
            "Edit Tag"
        } else if self.from_selection {
            "New Tag from Selection"
        } else {
            "New Tag"
        };
        let title = dialog_title(title_text.to_string());

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

        // Build form rows - include length info if from selection
        let mut form_rows: Vec<Element<'_, TagDialogMessage>> = vec![
            preview_row.into(),
            replacement_row.into(),
            pos_row.into(),
            placement_row.into(),
        ];

        if let Some(len) = self.length {
            let length_row = row![
                left_label("Length".to_string()),
                text(format!("{} characters", len)).size(TEXT_SIZE_NORMAL),
            ]
            .spacing(DIALOG_SPACING)
            .align_y(iced::Alignment::Center);
            form_rows.push(length_row.into());
        }

        let content = column![
            title,
            Space::new().height(DIALOG_SPACING),
            effect_box(column(form_rows).spacing(DIALOG_SPACING).into()),
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
