use parking_lot::Mutex;
use std::sync::Arc;

use i18n_embed_fl::fl;
use iced::{
    Border, Color, Element, Length, Shadow, Theme,
    alignment::{Horizontal, Vertical},
    widget::{Id, button, column, container, row, text, text_input},
};
use icy_engine::{AttributedChar, BufferType, EditableScreen, Position, Screen, Selection};
use icy_engine_gui::ui::{BUTTON_FONT_SIZE, danger_button_style, primary_button_style, secondary_button_style};

use crate::ui::{MainWindowMode, Message};

#[derive(Debug, Clone)]
pub enum FindDialogMsg {
    ChangePattern(String),
    FindNext,
    FindPrev,
    CloseDialog,
    SetCasing(bool),
}

#[derive(Debug)]
pub struct DialogState {
    pattern: String,
    conv_pattern: Vec<char>,
    pub case_sensitive: bool,
    cur_sel: usize,
    cur_pos: Position,
    results: Vec<Position>,
    search_input_id: Id,
    last_selected_pos: Option<Position>,
}

impl DialogState {
    pub fn new() -> Self {
        Self {
            pattern: String::new(),
            conv_pattern: Vec::new(),
            case_sensitive: false,
            cur_sel: 0,
            cur_pos: Position::default(),
            results: Vec::new(),
            search_input_id: Id::unique(),
            last_selected_pos: None,
        }
    }

    pub fn update(&mut self, msg: FindDialogMsg, edit_screen: Arc<Mutex<Box<dyn Screen>>>) -> Option<Message> {
        match msg {
            FindDialogMsg::ChangePattern(pattern) => {
                self.pattern = pattern.clone();

                // Clear selection if pattern is empty
                if self.pattern.is_empty() {
                    let mut edit_screen = edit_screen.lock();
                    let _ = edit_screen.clear_selection();
                    self.results.clear();
                    self.cur_sel = 0;
                    self.cur_pos = Position::default();
                    self.last_selected_pos = None;
                    return None;
                }

                // Search for the new pattern
                let mut edit_screen_locked = edit_screen.lock();
                if let Some(editable) = edit_screen_locked.as_editable() {
                    self.search_pattern(editable);
                }
                drop(edit_screen_locked);

                // Check if the current/last position still matches the new pattern
                let should_keep_position = if let Some(last_pos) = self.last_selected_pos {
                    // Check if this position is in our results (meaning it still matches)
                    self.results.contains(&last_pos)
                } else {
                    false
                };

                if should_keep_position {
                    // Keep the same position, just update the selection length
                    let pos = self.last_selected_pos.unwrap();
                    // Find the index of this position in the results
                    if let Some(index) = self.results.iter().position(|&p| p == pos) {
                        self.cur_sel = index;
                        self.cur_pos = pos;
                        self.select_current(edit_screen);
                    }
                } else if !self.results.is_empty() {
                    // Pattern changed and doesn't match at current position
                    // Find the closest match or select the first one
                    if let Some(last_pos) = self.last_selected_pos {
                        // Try to find the closest match to the last position
                        let closest_idx = self.find_closest_match(last_pos);
                        self.cur_sel = closest_idx;
                        self.cur_pos = self.results[closest_idx];
                    } else {
                        // No previous position, select first match
                        self.cur_sel = 0;
                        self.cur_pos = self.results[0];
                    }
                    self.select_current(edit_screen);
                } else {
                    // No results found
                    let mut edit_screen = edit_screen.lock();
                    let _ = edit_screen.clear_selection();
                    self.last_selected_pos = None;
                }
                None
            }
            FindDialogMsg::FindNext => {
                if !self.results.is_empty() {
                    self.find_next(edit_screen);
                }
                None
            }
            FindDialogMsg::FindPrev => {
                if !self.results.is_empty() {
                    self.find_prev(edit_screen);
                }
                None
            }
            FindDialogMsg::CloseDialog => {
                // Clear selection when closing
                let mut edit_screen = edit_screen.lock();
                let _ = edit_screen.clear_selection();
                self.last_selected_pos = None;
                Some(crate::ui::Message::CloseDialog(Box::new(MainWindowMode::ShowTerminal)))
            }
            FindDialogMsg::SetCasing(case_sensitive) => {
                self.case_sensitive = case_sensitive;

                // Re-search with new case sensitivity setting
                if !self.pattern.is_empty() {
                    let mut screen_locked = edit_screen.lock();
                    if let Some(editable) = screen_locked.as_editable() {
                        self.search_pattern(editable);
                    }
                    drop(screen_locked);

                    // Try to keep the same position if it still matches
                    if let Some(last_pos) = self.last_selected_pos {
                        if self.results.contains(&last_pos) {
                            // Same position still matches
                            if let Some(index) = self.results.iter().position(|&p| p == last_pos) {
                                self.cur_sel = index;
                                self.cur_pos = last_pos;
                                self.select_current(edit_screen);
                            }
                        } else if !self.results.is_empty() {
                            // Find closest match
                            let closest_idx = self.find_closest_match(last_pos);
                            self.cur_sel = closest_idx;
                            self.cur_pos = self.results[closest_idx];
                            self.select_current(edit_screen);
                        }
                    } else if !self.results.is_empty() {
                        self.cur_sel = 0;
                        self.cur_pos = self.results[0];
                        self.select_current(edit_screen);
                    }
                }
                None
            }
        }
    }

    fn find_closest_match(&self, target: Position) -> usize {
        // Find the result closest to the target position
        let mut closest_idx = 0;
        let mut min_distance = i32::MAX;

        for (idx, &pos) in self.results.iter().enumerate() {
            // Simple Manhattan distance for now
            let distance = (pos.y - target.y).abs() + (pos.x - target.x).abs();
            if distance < min_distance {
                min_distance = distance;
                closest_idx = idx;
            }
            // If we find an exact match or one that's very close, use it
            if distance == 0 {
                return idx;
            }
        }

        closest_idx
    }

    pub fn search_pattern(&mut self, buf: &dyn EditableScreen) {
        let mut cur_len = 0;
        let mut start_pos = Position::default();
        self.results.clear();

        if self.pattern.is_empty() {
            return;
        }

        // Convert string to Vec<char>
        let pattern_chars: Vec<char> = self.pattern.chars().collect();

        if self.case_sensitive {
            self.conv_pattern = pattern_chars;
        } else {
            self.conv_pattern = self.pattern.chars().map(|c| c.to_ascii_lowercase()).collect();
        }

        for y in 0..buf.get_line_count() {
            for x in 0..buf.get_width() {
                let ch = buf.get_char((x, y).into());
                if self.compare(buf.buffer_type(), cur_len, ch) {
                    if cur_len == 0 {
                        start_pos = (x, y).into();
                    }
                    cur_len += 1;
                    if cur_len >= self.conv_pattern.len() {
                        self.results.push(start_pos);
                        cur_len = 0;
                    }
                } else if self.compare(buf.buffer_type(), 0, ch) {
                    start_pos = (x, y).into();
                    cur_len = 1;
                } else {
                    cur_len = 0;
                }
            }
        }
    }

    fn compare(&self, buffer_type: BufferType, cur_len: usize, attributed_char: AttributedChar) -> bool {
        if cur_len >= self.conv_pattern.len() {
            return false;
        }

        let ch = buffer_type.convert_to_unicode(attributed_char.ch);
        if self.case_sensitive {
            self.conv_pattern[cur_len] == ch
        } else {
            self.conv_pattern[cur_len] == ch.to_ascii_lowercase()
        }
    }

    fn select_current(&mut self, edit_screen: Arc<Mutex<Box<dyn Screen>>>) {
        if self.cur_sel >= self.results.len() {
            return;
        }

        let pos = self.results[self.cur_sel];
        let mut sel = Selection::new(pos);
        sel.lead = Position::new(pos.x + self.pattern.len() as i32 - 1, pos.y);

        let mut edit_screen = edit_screen.lock();
        let _ = edit_screen.clear_selection();
        let _ = edit_screen.set_selection(sel);

        // Remember this position for next pattern change
        self.last_selected_pos = Some(pos);
    }

    pub fn find_next(&mut self, edit_screen: Arc<Mutex<Box<dyn Screen>>>) {
        if self.results.is_empty() || self.pattern.is_empty() {
            return;
        }

        // Move to next result
        self.cur_sel = (self.cur_sel + 1) % self.results.len();
        self.cur_pos = self.results[self.cur_sel];
        self.select_current(edit_screen);
    }

    pub fn find_prev(&mut self, edit_screen: Arc<Mutex<Box<dyn Screen>>>) {
        if self.results.is_empty() || self.pattern.is_empty() {
            return;
        }

        // Move to previous result
        if self.cur_sel == 0 {
            self.cur_sel = self.results.len() - 1;
        } else {
            self.cur_sel -= 1;
        }
        self.cur_pos = self.results[self.cur_sel];
        self.select_current(edit_screen);
    }

    pub fn view(&self) -> Element<'_, Message> {
        let search_input = text_input(&fl!(crate::LANGUAGE_LOADER, "terminal-find-hint"), &self.pattern)
            .id(self.search_input_id.clone())
            .on_input(|s| Message::FindDialog(FindDialogMsg::ChangePattern(s)))
            .padding(6)
            .width(Length::Fixed(200.0));

        let prev_button = button(text("↑").size(BUTTON_FONT_SIZE).wrapping(text::Wrapping::None))
            .on_press(Message::FindDialog(FindDialogMsg::FindPrev))
            .padding([4, 8])
            .style(if self.results.is_empty() {
                secondary_button_style
            } else {
                primary_button_style
            });

        let next_button = button(text("↓").size(BUTTON_FONT_SIZE).wrapping(text::Wrapping::None))
            .on_press(Message::FindDialog(FindDialogMsg::FindNext))
            .padding([4, 8])
            .style(if self.results.is_empty() {
                secondary_button_style
            } else {
                primary_button_style
            });

        let close_button = button(text("✕").size(BUTTON_FONT_SIZE).wrapping(text::Wrapping::None))
            .on_press(Message::FindDialog(FindDialogMsg::CloseDialog))
            .padding([4, 8])
            .style(danger_button_style);

        let case_button = if self.case_sensitive {
            button(text("🗛").size(12).wrapping(text::Wrapping::None))
                .on_press(Message::FindDialog(FindDialogMsg::SetCasing(false)))
                .padding([2, 4])
                .style(primary_button_style)
        } else {
            button(text("🗛").size(12).wrapping(text::Wrapping::None))
                .on_press(Message::FindDialog(FindDialogMsg::SetCasing(true)))
                .padding([2, 4])
                .style(secondary_button_style)
        };

        let results_label = if self.results.is_empty() {
            if !self.pattern.is_empty() {
                row![
                    text("⚠").size(14).color(Color::from_rgb(0.8, 0.2, 0.2)),
                    text(fl!(crate::LANGUAGE_LOADER, "terminal-find-no-results"))
                        .size(12)
                        .color(Color::from_rgb(0.8, 0.2, 0.2))
                ]
                .spacing(4)
            } else {
                row![text("").size(12)]
            }
        } else {
            row![
                text("✓").size(14).color(Color::from_rgb(0.2, 0.8, 0.2)),
                text(fl!(
                    crate::LANGUAGE_LOADER,
                    "terminal-find-results",
                    cur = (self.cur_sel + 1).to_string(),
                    total = self.results.len().to_string()
                ))
                .size(12)
                .color(Color::from_rgb(0.2, 0.8, 0.2))
            ]
            .spacing(4)
        };

        let content = column![
            row![search_input, prev_button, next_button, close_button,].spacing(4).align_y(Vertical::Center),
            row![case_button, iced::widget::Space::new().width(Length::Fill), results_label,]
                .spacing(8)
                .align_y(Vertical::Center)
                .padding([0, 4]),
        ]
        .spacing(8)
        .padding(12);

        container(content)
            .style(|theme: &Theme| container::Style {
                background: Some(iced::Background::Color(theme.palette().background)),
                border: Border {
                    color: theme.extended_palette().background.strong.color,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                text_color: None,
                shadow: Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.3),
                    offset: iced::Vector::new(2.0, 2.0),
                    blur_radius: 4.0,
                },
                snap: false,
            })
            .width(Length::Shrink)
            .height(Length::Shrink)
            .into()
    }

    pub fn focus_search_input(&self) -> iced::Task<Message> {
        iced::Task::batch([
            iced::widget::operation::focus(self.search_input_id.clone()),
            iced::widget::operation::select_all(self.search_input_id.clone()),
        ])
    }
}

pub fn find_dialog_overlay<'a>(state: &'a DialogState, content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    // Create an overlay that positions the find dialog in the upper right
    let find_dialog = container(state.view())
        .align_x(Horizontal::Right)
        .align_y(Vertical::Top)
        .padding([8, 8])
        .width(Length::Fill)
        .height(Length::Fill);

    // Stack the find dialog over the content without shadowing
    iced::widget::stack![content.into(), find_dialog,].into()
}
