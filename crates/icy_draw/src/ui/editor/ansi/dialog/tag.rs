use icy_ui::{
    widget::{button, column, container, pick_list, row, scrollable, text, text_input, Space},
    Element, Length,
};
use icy_engine::{Position, TagPlacement};
use icy_engine_gui::settings::{effect_box, left_label};
use icy_engine_gui::ui::*;

use crate::fl;
use crate::util::{get_available_taglists, load_taglist, TagReplacementList, TaglistInfo};

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
            TagPlacementChoice::InText => write!(f, "{}", fl!("tag-list-in-text")),
            TagPlacementChoice::WithGotoXY => write!(f, "{}", fl!("tag-list-with-gotoxy")),
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
    ToggleReplacements,
    SelectReplacement(String, String),
    SelectTaglist(TaglistInfo),
    ImportTaglist,
    SetFilter(String),
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
    /// Whether the replacement list browser is shown
    pub show_replacements: bool,
    /// Available tag lists (name, path)
    pub available_taglists: Vec<TaglistInfo>,
    /// Currently selected taglist
    pub selected_taglist: TaglistInfo,
    /// Loaded replacement entries
    pub replacement_list: TagReplacementList,
    /// Filter text for replacement list
    pub filter: String,
}

impl TagDialog {
    /// Create a dialog for editing an existing tag
    pub fn edit(tag: &icy_engine::Tag, index: usize, selected_taglist: &str, taglists_dir: Option<std::path::PathBuf>) -> Self {
        let placement = match tag.tag_placement {
            TagPlacement::InText => TagPlacementChoice::InText,
            TagPlacement::WithGotoXY => TagPlacementChoice::WithGotoXY,
        };

        let available_taglists: Vec<TaglistInfo> = get_available_taglists(taglists_dir.as_deref());

        let selected_id = if selected_taglist.is_empty() {
            available_taglists.first().map(|t| t.id.clone()).unwrap_or_default()
        } else {
            selected_taglist.to_string()
        };

        let selected_info = available_taglists
            .iter()
            .find(|t| t.id.eq_ignore_ascii_case(&selected_id))
            .cloned()
            .or_else(|| available_taglists.first().cloned())
            .unwrap_or_default();

        let replacement_list = load_taglist(&selected_info.id, taglists_dir.as_deref());

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
            show_replacements: false,
            available_taglists,
            selected_taglist: selected_info,
            replacement_list,
            filter: String::new(),
        }
    }

    pub fn view(&self) -> Element<'_, TagDialogMessage> {
        // If showing replacement browser, show that instead
        if self.show_replacements {
            return self.view_replacement_browser();
        }

        let preview_row = row![
            left_label(fl!("tag-edit-preview")),
            text_input("", &self.preview)
                .size(TEXT_SIZE_NORMAL)
                .width(Length::Fill)
                .on_input(TagDialogMessage::SetPreview),
        ]
        .spacing(DIALOG_SPACING)
        .align_y(icy_ui::Alignment::Center);

        let replacement_row = row![
            left_label(fl!("tag-edit-replacement")),
            text_input("", &self.replacement_value)
                .size(TEXT_SIZE_NORMAL)
                .width(Length::Fill)
                .on_input(TagDialogMessage::SetReplacement),
            button(text("…").size(TEXT_SIZE_NORMAL))
                .padding([2, 8])
                .on_press(TagDialogMessage::ToggleReplacements),
        ]
        .spacing(DIALOG_SPACING)
        .align_y(icy_ui::Alignment::Center);

        let pos_x_input = text_input("", &self.pos_x)
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fixed(70.0))
            .on_input(TagDialogMessage::SetPosX);
        let pos_y_input = text_input("", &self.pos_y)
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fixed(70.0))
            .on_input(TagDialogMessage::SetPosY);

        let pos_row = row![left_label(fl!("tag-edit-position")), pos_x_input, text(",").size(TEXT_SIZE_NORMAL), pos_y_input,]
            .spacing(DIALOG_SPACING)
            .align_y(icy_ui::Alignment::Center);

        let placement_row = row![
            left_label(fl!("tag-list-placement")),
            pick_list(TagPlacementChoice::ALL.as_slice(), Some(self.placement), TagDialogMessage::SetPlacement).width(Length::Fixed(140.0)),
        ]
        .spacing(DIALOG_SPACING)
        .align_y(icy_ui::Alignment::Center);

        let form_rows: Vec<Element<'_, TagDialogMessage>> = vec![preview_row.into(), replacement_row.into(), pos_row.into(), placement_row.into()];

        let content = column![
            Space::new().height(DIALOG_SPACING),
            effect_box(column(form_rows).spacing(DIALOG_SPACING).into()),
        ]
        .spacing(0);

        let dialog_content: Element<'_, TagDialogMessage> = dialog_area(container(content).width(Length::Fill).into());

        let buttons = button_row(vec![
            secondary_button(&fl!("button-cancel"), Some(TagDialogMessage::Cancel)).into(),
            primary_button("OK", Some(TagDialogMessage::Ok)).into(),
        ]);
        let button_area: Element<'_, TagDialogMessage> = dialog_area(buttons);

        modal_container(column![dialog_content, separator(), button_area].into(), DIALOG_WIDTH_MEDIUM).into()
    }

    fn view_replacement_browser(&self) -> Element<'_, TagDialogMessage> {
        // Header with taglist selector and filter
        let taglist_picker = pick_list(
            self.available_taglists.clone(),
            Some(self.selected_taglist.clone()),
            TagDialogMessage::SelectTaglist,
        )
        .width(Length::Fixed(150.0));

        let filter_input = text_input(&fl!("tag-edit-filter"), &self.filter)
            .size(TEXT_SIZE_NORMAL)
            .width(Length::Fill)
            .on_input(TagDialogMessage::SetFilter);

        let import_btn = secondary_button("Import…".to_string(), Some(TagDialogMessage::ImportTaglist));

        let header = row![taglist_picker, filter_input, import_btn]
            .spacing(DIALOG_SPACING)
            .align_y(icy_ui::Alignment::Center);

        let description = if !self.replacement_list.description.trim().is_empty() {
            Some(
                text(&self.replacement_list.description)
                    .size(TEXT_SIZE_NORMAL)
                    .wrapping(icy_ui::widget::text::Wrapping::Word)
                    .width(Length::Fill),
            )
        } else {
            None
        };

        // Build list of replacements
        let filter_lower = self.filter.to_lowercase();
        let mut list_items: Vec<Element<'_, TagDialogMessage>> = Vec::new();

        for entry in &self.replacement_list.entries {
            // Filter
            if !self.filter.is_empty() && !entry.tag.to_lowercase().contains(&filter_lower) && !entry.description.to_lowercase().contains(&filter_lower) {
                continue;
            }

            let tag_text = text(&entry.tag).size(TEXT_SIZE_NORMAL).width(Length::Fixed(150.0));
            let desc_text = text(&entry.description).size(TEXT_SIZE_SMALL).width(Length::Fill);

            let row_btn = button(row![tag_text, desc_text].spacing(DIALOG_SPACING).align_y(icy_ui::Alignment::Center))
                .width(Length::Fill)
                .padding([4, 8])
                .style(button::text_style)
                .on_press(TagDialogMessage::SelectReplacement(entry.example.clone(), entry.tag.clone()));

            list_items.push(row_btn.into());
        }

        let list = scrollable(column(list_items).spacing(2).padding(4))
            .width(Length::Fill)
            .height(Length::Fixed(300.0));

        let comments_box = if !self.replacement_list.comments.trim().is_empty() {
            let comments = text(&self.replacement_list.comments)
                .size(TEXT_SIZE_SMALL)
                .wrapping(icy_ui::widget::text::Wrapping::Word)
                .width(Length::Fill);

            Some(effect_box(container(comments).padding(8).width(Length::Fill).into()))
        } else {
            None
        };

        let mut info_column = column![header].spacing(DIALOG_SPACING);
        if let Some(desc) = description {
            info_column = info_column.push(desc);
        }

        let content = column![
            Space::new().height(DIALOG_SPACING),
            effect_box({
                let inner = column![info_column, container(list).height(Length::Fill).width(Length::Fill)].spacing(DIALOG_SPACING);

                inner.into()
            }),
        ]
        .spacing(0);

        let content = if let Some(c) = comments_box {
            content.push(Space::new().height(DIALOG_SPACING)).push(c)
        } else {
            content
        };

        let dialog_content: Element<'_, TagDialogMessage> = dialog_area(container(content).width(Length::Fill).height(Length::Fill).into());

        let buttons = button_row(vec![secondary_button(&fl!("menu-close"), Some(TagDialogMessage::ToggleReplacements)).into()]);
        let button_area: Element<'_, TagDialogMessage> = dialog_area(buttons);

        modal_container(column![dialog_content, separator(), button_area].into(), DIALOG_WIDTH_LARGE).into()
    }
}
