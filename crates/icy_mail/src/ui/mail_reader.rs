use crate::qwk::MessageInfo;
use crate::ui::main_window::{ConferenceRow, ROW_HEIGHT};
use crate::ui::threading::Row;
use crate::ui::{ConferenceColumn, MainWindow, Message, MessageColumn, Pane, SortDirection, ViewMode};
use icy_engine_gui::TerminalView;
use icy_ui::widget::{button, column, container, mouse_area, row, scroll_area, scrollable, table, text, text_input, Space};
use icy_ui::{Alignment, Border, Color, Element, Font, Length};

const TEXT_SIZE: f32 = 12.0;
const HEADER_TEXT_SIZE: f32 = 12.0;
const CONFERENCE_WIDTH: f32 = 320.0;
const AREA_COL: f32 = 52.0;
const COUNT_COL: f32 = 56.0;
const DATE_COL: f32 = 116.0;
const LINES_COL: f32 = 52.0;
const FROM_COL: f32 = 150.0;
/// Indent applied per reply level in threaded mode.
const THREAD_INDENT: f32 = 14.0;

impl MainWindow {
    pub fn mail_reader_view(&self) -> Element<'_, Message> {
        let Some(package) = &self.package else {
            return container(text("No package loaded")).into();
        };

        let conferences = focus_on_click(
            pane_frame(self.build_conference_list(), self.focus == Pane::Conferences)
                .width(Length::Fixed(CONFERENCE_WIDTH))
                .height(Length::Fill)
                .into(),
            Pane::Conferences,
        );

        let messages = focus_on_click(
            pane_frame(self.build_message_list(package), self.focus == Pane::Messages)
                .width(Length::Fill)
                .height(Length::FillPortion(2))
                .into(),
            Pane::Messages,
        );

        let reader = focus_on_click(
            pane_frame(self.build_message_view(), self.focus == Pane::Content)
                .width(Length::Fill)
                .height(Length::FillPortion(3))
                .into(),
            Pane::Content,
        );

        column![
            self.build_toolbar(),
            row![conferences, column![messages, reader].spacing(2).width(Length::Fill)]
                .spacing(2)
                .height(Length::Fill),
        ]
        .into()
    }

    fn build_toolbar(&self) -> Element<'_, Message> {
        let mode_button = |label: &'static str, mode: ViewMode| {
            let active = self.view_mode == mode;
            button(text(label).size(TEXT_SIZE))
                .on_press(Message::SetViewMode(mode))
                .padding([4, 10])
                .style(move |theme: &icy_ui::Theme, status| segmented_style(theme, status, active))
        };

        let filter = text_input("Filter author or subject\u{2026}", &self.filter)
            .id(self.filter_input.clone())
            .on_input(Message::FilterChanged)
            .size(TEXT_SIZE)
            .padding([4, 6])
            .width(Length::Fixed(240.0));

        let mut bar = row![
            button(text("Open").size(TEXT_SIZE)).on_press(Message::OpenPackage).padding([4, 10]),
            button(text("Refresh").size(TEXT_SIZE)).on_press(Message::Refresh).padding([4, 10]),
            button(text("New").size(TEXT_SIZE)).on_press(Message::NewMessage).padding([4, 10]),
            Space::new().width(Length::Fill),
            mode_button("List", ViewMode::List),
            mode_button("Threads", ViewMode::Threads),
            Space::new().width(12),
            filter,
        ]
        .spacing(4)
        .padding(6)
        .align_y(Alignment::Center);

        if !self.filter.is_empty() {
            bar = bar.push(button(text("\u{2715}").size(TEXT_SIZE)).on_press(Message::ClearFilter).padding([4, 8]));
        }

        container(bar)
            .width(Length::Fill)
            .style(|theme: &icy_ui::Theme| container::Style {
                background: Some(icy_ui::Background::Color(theme.primary.base)),
                ..Default::default()
            })
            .into()
    }

    // ----- conference list -----------------------------------------------------------------

    fn build_conference_list(&self) -> Element<'_, Message> {
        let _timer = crate::perf::Timer::with("build_conference_list", format!("{} rows", self.conference_rows().len()));
        let (sort_column, direction) = self.conference_sort;
        let header = header_row(vec![
            sort_header(
                "Area",
                AREA_COL,
                sort_column == ConferenceColumn::Area,
                direction,
                Message::SortConferencesBy(ConferenceColumn::Area),
            ),
            sort_header(
                "Description",
                0.0,
                sort_column == ConferenceColumn::Name,
                direction,
                Message::SortConferencesBy(ConferenceColumn::Name),
            ),
            sort_header(
                "Msgs",
                COUNT_COL,
                sort_column == ConferenceColumn::Count,
                direction,
                Message::SortConferencesBy(ConferenceColumn::Count),
            ),
        ]);

        let focused = self.focus == Pane::Conferences;
        let mut list = column![].spacing(0);
        for entry in self.conference_rows() {
            list = list.push(self.conference_row(entry, focused));
        }

        column![
            header,
            scrollable(list)
                .id(self.conference_scroll.clone())
                .width(Length::Fill)
                .height(Length::Fill)
                .on_scroll(|viewport| Message::ListScrolled {
                    pane: Pane::Conferences,
                    offset_y: viewport.absolute_offset().y,
                    height: viewport.bounds().height,
                })
                .direction(scrollable::Direction::Vertical(scrollable::Scrollbar::default())),
        ]
        .into()
    }

    fn conference_row<'a>(&self, entry: &'a ConferenceRow, focused: bool) -> Element<'a, Message> {
        let selected = self.selected_conference == entry.number;
        let label = if entry.number == 0 { "All".to_string() } else { entry.number.to_string() };

        let content = row![cell(label, AREA_COL), cell(entry.name.clone(), 0.0), cell(entry.count.to_string(), COUNT_COL),]
            .spacing(4)
            .padding([0, 6])
            .align_y(Alignment::Center);

        button(container(content).height(Length::Fixed(ROW_HEIGHT)).align_y(Alignment::Center))
            .on_press(Message::SelectConference(entry.number))
            .padding(0)
            .width(Length::Fill)
            .style(move |theme: &icy_ui::Theme, status| row_style(theme, status, selected, focused))
            .into()
    }

    // ----- message list --------------------------------------------------------------------

    fn build_message_list<'a>(&'a self, package: &'a crate::qwk::QwkPackage) -> Element<'a, Message> {
        let _timer = crate::perf::Timer::with("build_message_list", format!("{} rows", self.message_rows().len()));
        let threaded = self.view_mode == ViewMode::Threads;
        let (sort_column, direction) = self.message_sort;

        // Threading imposes its own order, so the headers are inert in that mode.
        let sortable = |column: MessageColumn| if threaded { None } else { Some(Message::SortMessagesBy(column)) };
        let header = header_row(vec![
            opt_sort_header(
                "Author",
                FROM_COL,
                !threaded && sort_column == MessageColumn::From,
                direction,
                sortable(MessageColumn::From),
            ),
            opt_sort_header(
                "Date",
                DATE_COL,
                !threaded && sort_column == MessageColumn::Date,
                direction,
                sortable(MessageColumn::Date),
            ),
            opt_sort_header(
                if threaded { "Subject (threaded)" } else { "Subject" },
                0.0,
                !threaded && sort_column == MessageColumn::Subject,
                direction,
                sortable(MessageColumn::Subject),
            ),
            opt_sort_header(
                "Lines",
                LINES_COL,
                !threaded && sort_column == MessageColumn::Lines,
                direction,
                sortable(MessageColumn::Lines),
            ),
        ]);

        let rows = self.message_rows();
        let body: Element<'a, Message> = if rows.is_empty() {
            container(
                text(if self.filter.is_empty() {
                    "No messages in this conference"
                } else {
                    "No messages match the filter"
                })
                .size(TEXT_SIZE),
            )
            .center_x(Length::Fill)
            .padding(16)
            .into()
        } else {
            let focused = self.focus == Pane::Messages;
            // Packets run to tens of thousands of messages; only the visible slice is built.
            table::virtual_table(ROW_HEIGHT, rows.len(), move |range| {
                let mut list = column![].spacing(0);
                for entry in &rows[range] {
                    list = list.push(self.message_row(entry, &package.infos[entry.index], focused, threaded));
                }
                list.into()
            })
            .id(self.message_scroll.clone())
            .width(Length::Fill)
            .height(Length::Fill)
            .cache_key(self.message_list_cache_key())
            .on_scroll(|viewport| Message::ListScrolled {
                pane: Pane::Messages,
                offset_y: viewport.absolute_offset().y,
                height: viewport.bounds().height,
            })
            .into()
        };

        column![header, body].into()
    }

    fn message_row<'a>(&self, entry: &Row, info: &'a MessageInfo, focused: bool, threaded: bool) -> Element<'a, Message> {
        let selected = self.selected_message == Some(info.index);

        let subject = if threaded && entry.depth > 0 {
            // Replies show the reply marker instead of repeating "Re:" over and over.
            let stripped = info.subject.trim_start();
            row![
                Space::new().width(Length::Fixed(f32::from(entry.depth) * THREAD_INDENT)),
                text("\u{21B3} ").size(TEXT_SIZE).font(Font::MONOSPACE),
                cell(stripped.to_string(), 0.0),
            ]
        } else if threaded {
            row![
                text(if entry.has_children { "\u{25BE} " } else { "  " }).size(TEXT_SIZE).font(Font::MONOSPACE),
                cell(info.subject.clone(), 0.0),
            ]
        } else {
            row![cell(info.subject.clone(), 0.0)]
        };

        let content = row![
            cell(info.from.clone(), FROM_COL),
            cell(info.date_str.clone(), DATE_COL),
            container(subject.align_y(Alignment::Center)).width(Length::Fill).clip(true),
            cell(info.lines.to_string(), LINES_COL),
        ]
        .spacing(4)
        .padding([0, 6])
        .align_y(Alignment::Center);

        button(container(content).height(Length::Fixed(ROW_HEIGHT)).align_y(Alignment::Center))
            .on_press(Message::SelectMessage(info.index))
            .padding(0)
            .width(Length::Fill)
            .style(move |theme: &icy_ui::Theme, status| row_style(theme, status, selected, focused))
            .into()
    }

    // ----- reading pane --------------------------------------------------------------------

    fn build_message_view(&self) -> Element<'_, Message> {
        let _timer = crate::perf::Timer::new("build_message_view");
        let (Some(index), Some(package)) = (self.selected_message, &self.package) else {
            return container(text("Select a message to read").size(14))
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into();
        };
        let Some(info) = package.infos.get(index) else {
            return container(text("Select a message to read").size(14))
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into();
        };

        let field = |label: &'static str, value: &str| {
            row![
                text(label).size(TEXT_SIZE).font(Font::MONOSPACE),
                text(value.to_string()).size(TEXT_SIZE).font(Font::MONOSPACE),
            ]
        };

        let header = column![
            field("Subject: ", &info.subject),
            row![
                field("From: ", &info.from),
                Space::new().width(16),
                field("To: ", &info.to),
                Space::new().width(16),
                field("Date: ", &info.date_str),
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(2)
        .padding([6, 8]);

        let zoom = self.terminal.get_zoom();
        let virtual_size = {
            let screen = self.terminal.screen.lock();
            screen.virtual_size()
        };

        // In FitWidth mode the zoom depends on the available viewport width, so the reported
        // content width must stay >= the widget width for `show_viewport` to see the real one.
        let is_fit_width = self.monitor_settings.scaling_mode.is_fit_width();
        let scrollable_width = if is_fit_width {
            (virtual_size.width as f32 * zoom).max(100_000.0)
        } else {
            virtual_size.width as f32 * zoom
        };
        let scrollable_size = icy_ui::Size::new(scrollable_width, virtual_size.height as f32 * zoom);
        let monitor_settings = self.monitor_settings.clone();

        let terminal_view = scroll_area()
            .id(self.terminal.scroll_area_id())
            .width(Length::Fill)
            .height(Length::Fill)
            .direction(if is_fit_width {
                scrollable::Direction::Vertical(scrollable::Scrollbar::default())
            } else {
                scrollable::Direction::Both {
                    vertical: scrollable::Scrollbar::default(),
                    horizontal: scrollable::Scrollbar::default(),
                }
            })
            .show_viewport(scrollable_size, move |viewport| {
                self.terminal.update_scroll_from_viewport(viewport, zoom);
                TerminalView::show_with_effects(&self.terminal, monitor_settings.clone(), None).map(Message::TerminalMessage)
            });

        column![
            container(header).width(Length::Fill).style(|theme: &icy_ui::Theme| container::Style {
                background: Some(icy_ui::Background::Color(theme.secondary.base)),
                ..Default::default()
            }),
            container(terminal_view).width(Length::Fill).height(Length::Fill),
        ]
        .into()
    }
}

// ----- shared building blocks ---------------------------------------------------------------

/// Monospace list cell; `width` of `0.0` means "take the remaining space".
fn cell<'a>(value: String, width: f32) -> Element<'a, Message> {
    let label = text(value).size(TEXT_SIZE).font(Font::MONOSPACE).wrapping(text::Wrapping::None);
    let width = if width > 0.0 { Length::Fixed(width) } else { Length::Fill };
    container(label).width(width).clip(true).into()
}

/// Header strip that stays put while the list scrolls underneath it.
fn header_row<'a>(cells: Vec<Element<'a, Message>>) -> Element<'a, Message> {
    let mut content = row![].spacing(4).padding([0, 6]).align_y(Alignment::Center);
    for cell in cells {
        content = content.push(cell);
    }

    container(container(content).height(Length::Fixed(ROW_HEIGHT + 4.0)).align_y(Alignment::Center))
        .width(Length::Fill)
        .style(|theme: &icy_ui::Theme| container::Style {
            background: Some(icy_ui::Background::Color(theme.primary.base)),
            border: Border {
                width: 0.0,
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

fn sort_header<'a>(label: &'a str, width: f32, active: bool, direction: SortDirection, on_press: Message) -> Element<'a, Message> {
    opt_sort_header(label, width, active, direction, Some(on_press))
}

fn opt_sort_header<'a>(label: &'a str, width: f32, active: bool, direction: SortDirection, on_press: Option<Message>) -> Element<'a, Message> {
    let arrow = if active { direction.arrow() } else { "" };
    let content = row![
        text(label).size(HEADER_TEXT_SIZE).font(Font::MONOSPACE),
        Space::new().width(Length::Fill),
        text(arrow).size(HEADER_TEXT_SIZE),
    ]
    .align_y(Alignment::Center);

    let width = if width > 0.0 { Length::Fixed(width) } else { Length::Fill };
    let mut header = button(content).padding([2, 2]).width(width).style(move |theme: &icy_ui::Theme, status| {
        use icy_ui::widget::button::{Status, Style};
        let text_color = if active { theme.accent.base } else { theme.primary.on };
        let background = match status {
            Status::Hovered | Status::Pressed => Some(icy_ui::Background::Color(theme.primary.component.hover)),
            _ => None,
        };
        Style {
            background,
            text_color,
            ..Style::default()
        }
    });

    if let Some(message) = on_press {
        header = header.on_press(message);
    }
    header.into()
}

/// Wraps a pane in a border that marks keyboard focus and forwards clicks as focus changes.
fn pane_frame<'a>(content: Element<'a, Message>, focused: bool) -> container::Container<'a, Message> {
    container(content).style(move |theme: &icy_ui::Theme| container::Style {
        border: Border {
            width: 1.0,
            color: if focused { theme.accent.base } else { theme.primary.divider },
            radius: 0.0.into(),
        },
        ..Default::default()
    })
}

fn row_style(theme: &icy_ui::Theme, status: icy_ui::widget::button::Status, selected: bool, focused: bool) -> icy_ui::widget::button::Style {
    use icy_ui::widget::button::{Status, Style};

    // An unfocused list keeps a dimmed selection so you can still see where you are.
    let (background, text_color) = if selected && focused {
        (Some(theme.accent.base), theme.accent.on)
    } else if selected {
        (Some(theme.primary.component.selected), theme.primary.on)
    } else {
        match status {
            Status::Hovered => (Some(theme.primary.component.hover), theme.primary.on),
            _ => (None, theme.primary.on),
        }
    };

    Style {
        background: background.map(icy_ui::Background::Color),
        text_color,
        ..Style::default()
    }
}

fn segmented_style(theme: &icy_ui::Theme, status: icy_ui::widget::button::Status, active: bool) -> icy_ui::widget::button::Style {
    use icy_ui::widget::button::{Status, Style};

    if active {
        return Style {
            background: Some(icy_ui::Background::Color(theme.accent.base)),
            text_color: theme.accent.on,
            ..Style::default()
        };
    }

    let background = match status {
        Status::Hovered | Status::Pressed => theme.primary.component.hover,
        _ => Color::TRANSPARENT,
    };
    Style {
        background: Some(icy_ui::Background::Color(background)),
        text_color: theme.primary.on,
        ..Style::default()
    }
}

/// Click anywhere in a pane to give it keyboard focus.
fn focus_on_click<'a>(content: Element<'a, Message>, pane: Pane) -> Element<'a, Message> {
    mouse_area(content).on_press(Message::FocusPane(pane)).into()
}
