use crate::ui::dialing_directory_dialog::{DialingDirectoryFilter, DialingDirectoryMsg};
use crate::ui::Message;
use crate::Address;
use i18n_embed_fl::fl;
use iced::widget::space;
use iced::Padding;
use iced::{
    widget::{button, column, container, row, scrollable, text, text_input, Column, Space},
    Alignment, Element, Length,
};
use icy_engine_gui::ui::{DIALOG_SPACING, TEXT_SIZE_NORMAL, TEXT_SIZE_SMALL};
use once_cell::sync::Lazy;
use std::mem::swap;

static CONNECT_TOADDRESS_PLACEHOLDER: Lazy<String> = Lazy::new(|| fl!(crate::LANGUAGE_LOADER, "dialing_directory-connect-to-address"));

impl super::DialingDirectoryState {
    pub fn filtered(&self) -> Vec<(usize, Address)> {
        let fav = matches!(self.filter_mode, DialingDirectoryFilter::Favourites);
        let needle = self.filter_text.trim().to_lowercase();
        let mut filtered: Vec<(usize, Address)> = self
            .addresses
            .lock()
            .addresses
            .iter()
            .enumerate()
            .filter(|(_, a)| {
                if fav && !a.is_favored {
                    return false;
                }
                if needle.is_empty() {
                    return true;
                }
                a.system_name.to_lowercase().contains(&needle) || a.address.to_lowercase().contains(&needle)
            })
            .map(|(idx, a)| (idx, a.clone()))
            .collect();

        // Sort by: 1) Favorites first, 2) Number of calls (descending)
        filtered.sort_by(|(_, a), (_, b)| {
            // First compare by favorite status (favorites come first)
            match (a.is_favored, b.is_favored) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                // If both are favorites or both are not, sort by number of calls (descending)
                _ => b.number_of_calls.cmp(&a.number_of_calls),
            }
        });

        filtered
    }

    pub fn create_address_list(&self) -> Element<'_, Message> {
        let addresses = self.filtered();

        let filter_input = text_input(&fl!(crate::LANGUAGE_LOADER, "dialing_directory-filter-placeholder"), &self.filter_text)
            .on_input(|s: String| Message::from(DialingDirectoryMsg::FilterTextChanged(s)))
            .padding(6)
            .size(16);
        let clear_btn: Element<Message> = if self.filter_text.is_empty() {
            Space::new().into()
        } else {
            button(text("×").wrapping(text::Wrapping::None))
                .on_press(Message::from(DialingDirectoryMsg::FilterTextChanged(String::new())))
                .width(Length::Shrink)
                .into()
        };
        let list_scroll: Element<Message> = {
            let mut col = Column::new();
            let show_quick_connect = self.filter_text.is_empty();

            if show_quick_connect && !self.addresses.lock().addresses.is_empty() {
                let selected = self.selected_bbs.is_none();
                let entry = address_row_entry(selected, None, CONNECT_TOADDRESS_PLACEHOLDER.to_string(), String::new(), false, u32::MAX, "");
                col = col.push(entry);
            }

            for (idx, a) in addresses {
                let selected = self.selected_bbs == Some(idx);
                // Pass the filter text for highlighting
                let entry = address_row_entry(
                    selected,
                    Some(idx),
                    a.system_name,
                    a.address,
                    a.is_favored,
                    a.number_of_calls as u32,
                    &self.filter_text, // Pass filter text for highlighting
                );
                col = col.push(entry);
            }

            scrollable(col.spacing(2).padding(Padding {
                top: 0.0,
                bottom: 0.0,
                left: 0.0,
                right: 13.0,
            }))
            .height(Length::Fill)
            .width(Length::Fill)
            .direction(scrollable::Direction::Vertical(scrollable::Scrollbar::new()))
            .into()
        };
        column![
            space().height(Length::Fixed(4.0)),
            row![filter_input, clear_btn].spacing(DIALOG_SPACING).align_y(Alignment::Center),
            container(list_scroll)
                .style(|_theme: &iced::Theme| container::Style {
                    background: Some(iced::Background::Color(iced::Color::from_rgba(0.0, 0.0, 0.0, 0.15))),
                    border: iced::Border {
                        color: iced::Color::from_rgba(0.3, 0.3, 0.3, 0.5),
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    text_color: None,
                    shadow: Default::default(),
                    snap: false,
                })
                .padding(4),
        ]
        .width(Length::Fixed(320.0))
        .spacing(DIALOG_SPACING)
        .into()
    }
}

// Update the address_row_entry function to support highlighting
fn address_row_entry<'a>(
    selected: bool,
    idx: Option<usize>,
    name: String,
    addr: String,
    favored: bool,
    calls: u32,
    search_text: &'a str, // Add search text parameter
) -> Element<'a, Message> {
    fn truncate_text(text: String, max_chars: usize) -> String {
        if text.chars().count() <= max_chars {
            text.to_string()
        } else {
            let mut result: String = text.chars().take(max_chars - 1).collect();
            result.push('…');
            result
        }
    }

    let star = if favored {
        text("★").size(16).style(|theme: &iced::Theme| iced::widget::text::Style {
            color: Some(theme.warning.base),
            ..Default::default()
        })
    } else {
        text("").size(16)
    };

    let truncated_name = truncate_text(name, 33);
    let truncated_addr = truncate_text(addr, 31);

    // Create highlighted text elements - pass owned Strings
    let name_element = if !search_text.is_empty() && truncated_name.to_lowercase().contains(&search_text.to_lowercase()) {
        highlight_name_text(truncated_name, search_text)
    } else {
        text(truncated_name).size(TEXT_SIZE_NORMAL).font(iced::Font::MONOSPACE).into()
    };

    let addr_element = if !search_text.is_empty() && truncated_addr.to_lowercase().contains(&search_text.to_lowercase()) {
        highlight_addr_text(truncated_addr, search_text)
    } else {
        text(truncated_addr)
            .size(TEXT_SIZE_SMALL)
            .style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.button.base),
                ..Default::default()
            })
            .font(iced::Font::MONOSPACE)
            .into()
    };

    let calls_text = text(if calls == u32::MAX { String::new() } else { format!("✆ {}", calls) }).size(TEXT_SIZE_SMALL);

    let content = column![
        row![name_element, Space::new().width(Length::Fill), star].align_y(Alignment::Center),
        row![addr_element, Space::new().width(Length::Fill), container(calls_text).center_y(Length::Shrink)]
    ]
    .spacing(2);

    let entry_container: container::Container<'_, Message> = container(content).width(Length::Fill).padding([6, 10]);

    let clickable = button(Space::new())
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(0)
        .style(|_theme: &iced::Theme, _status| button::Style {
            background: Some(iced::Background::Color(iced::Color::TRANSPARENT)),
            border: iced::Border {
                color: iced::Color::TRANSPARENT,
                width: 0.0,
                radius: 0.0.into(),
            },
            text_color: iced::Color::BLACK,
            shadow: Default::default(),
            snap: false,
        })
        .on_press(Message::from(DialingDirectoryMsg::SelectAddress(idx)));

    let stacked = iced::widget::stack![entry_container, clickable];

    if selected {
        container(stacked)
            .width(Length::Fill)
            .style(|theme: &iced::Theme| {
                let mut border_color = theme.accent.hover;
                border_color.a = 0.6;
                swap(&mut border_color.r, &mut border_color.g);

                container::Style {
                    background: Some(iced::Background::Color({
                        let mut c = theme.accent.selected;
                        swap(&mut c.r, &mut c.g);
                        c.a = 0.10;
                        c
                    })),
                    border: iced::Border {
                        color: border_color,
                        width: 1.0,
                        radius: 3.0.into(),
                    },
                    text_color: None,
                    shadow: Default::default(),
                    snap: false,
                }
            })
            .into()
    } else {
        container(stacked).width(Length::Fill).into()
    }
}

// Helper function to highlight text in name (larger font) - now takes owned String
fn highlight_name_text<'a>(text_str: String, search: &str) -> Element<'a, Message> {
    if search.is_empty() || !text_str.to_lowercase().contains(&search.to_lowercase()) {
        return text(text_str).size(TEXT_SIZE_NORMAL).font(iced::Font::MONOSPACE).into();
    }

    let lower_text = text_str.to_lowercase();
    let lower_search = search.to_lowercase();

    let mut row_elements: Vec<Element<'a, Message>> = Vec::new();
    let mut last = 0;

    for (idx, _) in lower_text.match_indices(&lower_search) {
        if idx > last {
            // Add non-highlighted part
            row_elements.push(text(text_str[last..idx].to_string()).size(TEXT_SIZE_NORMAL).font(iced::Font::MONOSPACE).into());
        }
        // Add highlighted part with different style
        row_elements.push(
            text(text_str[idx..idx + search.len()].to_string())
                .size(TEXT_SIZE_NORMAL)
                .font(iced::Font::MONOSPACE)
                .style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.accent.hover),
                    ..Default::default()
                })
                .into(),
        );
        last = idx + search.len();
    }
    if last < text_str.len() {
        row_elements.push(text(text_str[last..].to_string()).size(TEXT_SIZE_NORMAL).font(iced::Font::MONOSPACE).into());
    }

    row(row_elements).into()
}

// Helper function to highlight text in address (smaller font) - now takes owned String
fn highlight_addr_text<'a>(text_str: String, search: &str) -> Element<'a, Message> {
    if search.is_empty() || !text_str.to_lowercase().contains(&search.to_lowercase()) {
        return text(text_str)
            .size(TEXT_SIZE_SMALL)
            .style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.button.on),
                ..Default::default()
            })
            .font(iced::Font::MONOSPACE)
            .into();
    }

    let lower_text = text_str.to_lowercase();
    let lower_search = search.to_lowercase();

    let mut row_elements: Vec<Element<'a, Message>> = Vec::new();
    let mut last = 0;

    for (idx, _) in lower_text.match_indices(&lower_search) {
        if idx > last {
            // Add non-highlighted part
            row_elements.push(
                text(text_str[last..idx].to_string())
                    .size(TEXT_SIZE_SMALL)
                    .style(|theme: &iced::Theme| iced::widget::text::Style {
                        color: Some(theme.button.on),
                        ..Default::default()
                    })
                    .font(iced::Font::MONOSPACE)
                    .into(),
            );
        }
        // Add highlighted part with warning color
        row_elements.push(
            text(text_str[idx..idx + search.len()].to_string())
                .size(TEXT_SIZE_SMALL)
                .font(iced::Font::MONOSPACE)
                .style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.accent.hover),
                    ..Default::default()
                })
                .into(),
        );
        last = idx + search.len();
    }
    if last < text_str.len() {
        row_elements.push(
            text(text_str[last..].to_string())
                .size(TEXT_SIZE_SMALL)
                .style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.button.on),
                    ..Default::default()
                })
                .font(iced::Font::MONOSPACE)
                .into(),
        );
    }

    row(row_elements).into()
}
