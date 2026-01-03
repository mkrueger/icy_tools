use icy_engine::{Position, TagPlacement};
use icy_engine_gui::settings::effect_box;
use icy_engine_gui::ui::*;
use icy_ui::{
    widget::{button, column, container, row, scrollable, text, Space},
    Element, Length, Theme,
};

use crate::fl;

#[derive(Clone, Debug)]
pub enum TagListDialogMessage {
    Close,
    Delete(usize),
}

#[derive(Clone, Debug)]
pub struct TagListItem {
    pub index: usize,
    pub is_enabled: bool,
    pub preview: String,
    pub replacement_value: String,
    pub position: Position,
    pub placement: TagPlacement,
}

#[derive(Clone, Debug)]
pub struct TagListDialog {
    pub items: Vec<TagListItem>,
}

impl TagListDialog {
    pub fn new(items: Vec<TagListItem>) -> Self {
        Self { items }
    }

    pub fn view(&self) -> Element<'_, TagListDialogMessage> {
        let title = dialog_title(fl!("tag-list-title"));

        let header = row![
            text(fl!("tag-list-preview")).size(TEXT_SIZE_SMALL).width(Length::Fixed(140.0)),
            text(fl!("tag-list-pos")).size(TEXT_SIZE_SMALL).width(Length::Fixed(80.0)),
            text(fl!("tag-list-placement")).size(TEXT_SIZE_SMALL).width(Length::Fixed(120.0)),
            text(fl!("tag-list-replacement")).size(TEXT_SIZE_SMALL),
            Space::new().width(Length::Fixed(44.0)),
        ]
        .spacing(DIALOG_SPACING)
        .align_y(icy_ui::Alignment::Center);

        let mut rows = column![].spacing(2).padding(4);

        if self.items.is_empty() {
            rows = rows.push(
                container(text(fl!("tag-list-no-tags")).size(TEXT_SIZE_NORMAL))
                    .width(Length::Fill)
                    .padding(8)
                    .style(|theme: &Theme| container::Style {
                        background: Some(icy_ui::Background::Color(theme.background.base)),
                        ..Default::default()
                    }),
            );
        } else {
            for item in &self.items {
                let preview_base = text(&item.preview)
                    .size(TEXT_SIZE_NORMAL)
                    .font(icy_ui::Font::MONOSPACE)
                    .width(Length::Fixed(140.0));

                let preview = if item.is_enabled {
                    preview_base
                } else {
                    preview_base.style(|theme: &Theme| text::Style { color: Some(theme.button.on) })
                };

                let pos = text(format!("{},{}", item.position.x, item.position.y))
                    .size(TEXT_SIZE_SMALL)
                    .width(Length::Fixed(80.0));

                let placement = text(match item.placement {
                    TagPlacement::InText => fl!("tag-list-in-text"),
                    TagPlacement::WithGotoXY => fl!("tag-list-with-gotoxy"),
                })
                .size(TEXT_SIZE_SMALL)
                .width(Length::Fixed(120.0));

                let repl = if item.replacement_value.trim().is_empty() {
                    text("-").size(TEXT_SIZE_SMALL)
                } else {
                    text(&item.replacement_value).size(TEXT_SIZE_SMALL)
                };

                let delete_btn = button(text(fl!("tag-toolbar-delete")).size(TEXT_SIZE_SMALL))
                    .padding([2, 6])
                    .style(button::text_style)
                    .on_press(TagListDialogMessage::Delete(item.index));

                let row_el = row![preview, pos, placement, repl, delete_btn]
                    .spacing(DIALOG_SPACING)
                    .align_y(icy_ui::Alignment::Center);

                rows = rows.push(row_el);
            }
        }

        let list = scrollable(rows).width(Length::Fill).height(Length::Fill);

        let content = column![
            title,
            Space::new().height(DIALOG_SPACING),
            effect_box(column![header, container(list).height(Length::Fill).width(Length::Fill)].spacing(6).into()),
        ]
        .spacing(0);

        let dialog_content: Element<'_, TagListDialogMessage> = dialog_area(container(content).width(Length::Fill).height(Length::Fill).into());

        let buttons = button_row(vec![secondary_button(&fl!("menu-close"), Some(TagListDialogMessage::Close)).into()]);
        let button_area: Element<'_, TagListDialogMessage> = dialog_area(buttons);

        modal_container(column![dialog_content, separator(), button_area].into(), DIALOG_WIDTH_LARGE).into()
    }
}
