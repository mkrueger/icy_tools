use eframe::egui;
use icy_engine::{Screen, Selection, Shape};
use icy_engine_gui::Terminal;

pub enum ClickAction {
    Link(String),
    Command(bool, String),
}

pub fn click_action(screen: &dyn Screen, position: icy_engine::Position) -> Option<ClickAction> {
    for field in screen.mouse_fields() {
        if field.style.is_mouse_button() && field.contains(position.x, position.y) {
            if let Some(command) = &field.host_command {
                return Some(ClickAction::Command(field.style.reset_screen_after_click(), command.clone()));
            }
        }
    }
    for link in screen.hyperlinks() {
        if screen.is_position_in_range(position, link.position, link.length) {
            return Some(ClickAction::Link(link.url(screen)));
        }
    }
    None
}

#[derive(Default)]
pub struct Navigation {
    pub find_open: bool,
    pub find_initialized: bool,
    pub query: String,
    pub case_sensitive: bool,
    pub scroll_to: Option<f32>,
    pub scroll_x: Option<f32>,
    pub saved_scaling: Option<icy_engine_gui::ScalingMode>,
    pub search_failed: bool,
    pub search_result: Option<(usize, usize)>,
    dragging: bool,
}

impl Navigation {
    pub fn find(&mut self, terminal: &Terminal, backwards: bool) {
        let mut screen = terminal.screen.lock();
        let results = matches(&**screen, &self.query, self.case_sensitive);
        if let Some(selection) = next_match(&results, screen.selection().map(|selection| selection.anchor), backwards) {
            self.search_result = Some((results.iter().position(|result| result.anchor == selection.anchor).unwrap() + 1, results.len()));
            self.scroll_to = Some(selection.anchor.y as f32 * terminal.render_info.read().font_height);
            let _ = screen.set_selection(selection);
            self.search_failed = false;
        } else {
            self.search_result = None;
            self.search_failed = !self.query.is_empty();
        }
    }

    pub fn refresh_search(&mut self, terminal: &Terminal) {
        let mut screen = terminal.screen.lock();
        let results = matches(&**screen, &self.query, self.case_sensitive);
        let current = screen
            .selection()
            .and_then(|selection| results.iter().position(|result| result.anchor == selection.anchor))
            .unwrap_or(0);
        self.search_result = results.get(current).map(|selection| {
            self.scroll_to = Some(selection.anchor.y as f32 * terminal.render_info.read().font_height);
            let _ = screen.set_selection(*selection);
            (current + 1, results.len())
        });
        self.search_failed = !self.query.is_empty() && results.is_empty();
        if results.is_empty() {
            let _ = screen.clear_selection();
        }
    }

    pub fn interact(&mut self, terminal: &Terminal, response: &egui::Response, rectangular: bool) {
        let was_dragging = self.dragging;
        if response.ctx.input(|input| input.pointer.button_released(egui::PointerButton::Primary)) {
            self.dragging = false;
        }
        let info = terminal.render_info.read();
        let Some(pointer) = response.interact_pointer_pos() else { return };
        let Some((column, row)) = info.screen_to_cell(pointer.x, pointer.y) else {
            return;
        };
        let mut screen = terminal.screen.lock();
        let column = column + (terminal.scroll_x() / info.font_width.max(1.0)) as i32;
        let row = row + (terminal.scroll_y() / info.font_height.max(1.0)) as i32;
        let position = icy_engine::Position::new(column.clamp(0, (screen.width() - 1).max(0)), row.clamp(0, (screen.height() - 1).max(0)));
        if response.drag_started_by(egui::PointerButton::Primary) {
            self.dragging = true;
            let origin = response
                .ctx
                .input(|input| input.pointer.press_origin())
                .and_then(|origin| info.screen_to_cell(origin.x, origin.y));
            let anchor = origin
                .map(|(column, row)| {
                    icy_engine::Position::new(
                        column + (terminal.scroll_x() / info.font_width.max(1.0)) as i32,
                        row + (terminal.scroll_y() / info.font_height.max(1.0)) as i32,
                    )
                })
                .unwrap_or(position);
            let mut selection = Selection::new(anchor);
            selection.shape = if rectangular { Shape::Rectangle } else { Shape::Lines };
            let _ = screen.set_selection(selection);
        }
        if self.dragging || was_dragging {
            if let Some(mut selection) = screen.selection() {
                selection.lead = position;
                let _ = screen.set_selection(selection);
            }
        }
        if response.triple_clicked() {
            let mut selection = Selection::new((0, position.y));
            selection.lead = (screen.width() - 1, position.y).into();
            let _ = screen.set_selection(selection);
        } else if response.double_clicked() {
            let mut left = position.x;
            let mut right = position.x;
            let is_word = |column| {
                !screen
                    .buffer_type()
                    .convert_to_unicode(screen.char_at((column, position.y).into()).ch)
                    .is_whitespace()
            };
            while left > 0 && is_word(left - 1) {
                left -= 1;
            }
            while right + 1 < screen.width() && is_word(right + 1) {
                right += 1;
            }
            let mut selection = Selection::new((left, position.y));
            selection.lead = (right, position.y).into();
            let _ = screen.set_selection(selection);
        } else if response.clicked_by(egui::PointerButton::Primary) && !self.dragging && !was_dragging {
            let _ = screen.clear_selection();
        }
    }
}

#[cfg(test)]
pub fn find(screen: &dyn Screen, query: &str, case_sensitive: bool, backwards: bool) -> Option<Selection> {
    next_match(
        &matches(screen, query, case_sensitive),
        screen.selection().map(|selection| selection.anchor),
        backwards,
    )
}

fn next_match(results: &[Selection], previous: Option<icy_engine::Position>, backwards: bool) -> Option<Selection> {
    if backwards {
        results
            .iter()
            .rev()
            .find(|selection| previous.is_none_or(|position| selection.anchor < position))
            .or(results.last())
            .copied()
    } else {
        results
            .iter()
            .find(|selection| previous.is_none_or(|position| selection.anchor > position))
            .or(results.first())
            .copied()
    }
}

fn matches(screen: &dyn Screen, query: &str, case_sensitive: bool) -> Vec<Selection> {
    if query.is_empty() {
        return Vec::new();
    }
    let query: Vec<char> = query.chars().collect();
    let mut results = Vec::new();
    for row in 0..screen.height() {
        let line: Vec<char> = (0..screen.width())
            .map(|column| screen.buffer_type().convert_to_unicode(screen.char_at((column, row).into()).ch))
            .collect();
        for (column, candidate) in line.windows(query.len()).enumerate() {
            let matches = candidate.iter().zip(&query).all(|(actual, expected)| {
                if case_sensitive {
                    actual == expected
                } else {
                    actual.to_lowercase().eq(expected.to_lowercase())
                }
            });
            if !matches {
                continue;
            }
            let mut selection = Selection::new((column as i32, row));
            selection.lead = (column as i32 + query.len() as i32 - 1, row).into();
            results.push(selection);
        }
    }
    results
}

pub fn toggle_scrollback(terminal: &mut Terminal) -> bool {
    if terminal.is_in_scrollback_mode() {
        terminal.exit_scrollback_mode();
        return false;
    }
    let snapshot = terminal.screen.lock().as_editable().and_then(|screen| screen.snapshot_scrollback());
    if let Some(snapshot) = snapshot {
        terminal.enter_scrollback_mode(snapshot);
        return true;
    }
    false
}

pub fn selected_text(screen: &dyn Screen) -> Option<String> {
    let selection = screen.selection()?;
    icy_engine::clipboard::text(screen, screen.buffer_type(), &selection)
}

pub fn select_all(screen: &mut dyn Screen) {
    let mut selection = Selection::new((0, 0));
    selection.lead = (screen.width().saturating_sub(1), screen.height().saturating_sub(1)).into();
    selection.shape = Shape::Lines;
    let _ = screen.set_selection(selection);
}

#[cfg(test)]
mod tests {
    use super::*;
    use icy_engine::{AttributedChar, EditableScreen, TextScreen};
    use parking_lot::Mutex;
    use std::sync::Arc;

    #[test]
    fn copy_decodes_cp437_and_keeps_single_cell_selections() {
        let mut screen = TextScreen::default();
        screen.set_char((0, 0).into(), AttributedChar::new('\u{b3}', Default::default()));
        screen.set_selection(Selection::new((0, 0))).unwrap();
        assert_eq!(selected_text(&screen).as_deref(), Some("\u{2502}"));
    }

    #[test]
    fn scrollback_restores_the_live_screen() {
        let live: Arc<Mutex<Box<dyn Screen>>> = Arc::new(Mutex::new(Box::new(TextScreen::default())));
        let mut terminal = Terminal::new(live.clone());
        terminal.enter_scrollback_mode(Arc::new(Mutex::new(Box::new(TextScreen::default()))));
        assert!(!toggle_scrollback(&mut terminal));
        assert!(Arc::ptr_eq(&terminal.screen, &live));
    }

    #[test]
    fn history_contains_scrolled_lines_without_freezing_the_live_screen() {
        let mut screen = TextScreen::default();
        screen.terminal_state_mut().is_terminal_buffer = true;
        screen.set_scrollback_buffer_size(100);
        for (column, character) in "HISTORY".chars().enumerate() {
            screen.set_char((column as i32, 0).into(), AttributedChar::new(character, Default::default()));
        }
        screen.scroll_up();
        let live: Arc<Mutex<Box<dyn Screen>>> = Arc::new(Mutex::new(Box::new(screen)));
        let mut terminal = Terminal::new(live.clone());
        assert!(toggle_scrollback(&mut terminal));
        {
            let mut history = terminal.screen.lock();
            let selection = find(&**history, "HISTORY", true, false).unwrap();
            history.set_selection(selection).unwrap();
            assert_eq!(selected_text(&**history).as_deref(), Some("HISTORY"));
            let region = icy_engine::Rectangle::from(0, 0, 56, 16);
            let plain = history.render_region_to_rgba(region, &Default::default()).1;
            let selected = history
                .render_region_to_rgba(
                    region,
                    &icy_engine::RenderOptions {
                        selection: Some(selection),
                        ..Default::default()
                    },
                )
                .1;
            assert_ne!(plain, selected);
            let export = history
                .to_bytes("asc", &icy_engine::SaveOptions::ansi(icy_engine::AnsiCompatibilityLevel::Utf8Terminal))
                .unwrap();
            assert!(String::from_utf8(export).unwrap().contains("HISTORY"));
        }
        live.lock()
            .as_editable()
            .unwrap()
            .set_char((0, 0).into(), AttributedChar::new('L', Default::default()));
        assert_eq!(terminal.screen.lock().char_at((0, 0).into()).ch, 'H');
        assert!(!toggle_scrollback(&mut terminal));
        assert!(Arc::ptr_eq(&terminal.screen, &live));
        assert_eq!(terminal.screen.lock().char_at((0, 0).into()).ch, 'L');
    }

    #[test]
    fn search_wraps_and_uses_cell_positions() {
        let mut screen = TextScreen::default();
        for (column, character) in "One one".chars().enumerate() {
            screen.set_char((column as i32, 0).into(), AttributedChar::new(character, Default::default()));
        }
        let first = find(&screen, "one", false, false).unwrap();
        assert_eq!(first.anchor.x, 0);
        screen.set_selection(first).unwrap();
        let next = find(&screen, "one", false, false).unwrap();
        assert_eq!(next.anchor.x, 4);
        screen.set_selection(next).unwrap();
        assert_eq!(find(&screen, "one", false, false).unwrap().anchor.x, 0);
        assert_eq!(find(&screen, "One", true, true).unwrap().anchor.x, 0);
        assert!(find(&screen, "missing", false, false).is_none());
    }

    #[test]
    fn live_search_counts_matches_and_refreshes_case_changes() {
        let mut screen = TextScreen::default();
        for (column, character) in "One one".chars().enumerate() {
            screen.set_char((column as i32, 0).into(), AttributedChar::new(character, Default::default()));
        }
        let terminal = Terminal::new(Arc::new(Mutex::new(Box::new(screen))));
        let mut navigation = Navigation {
            query: "one".into(),
            ..Default::default()
        };
        navigation.refresh_search(&terminal);
        assert_eq!(navigation.search_result, Some((1, 2)));
        navigation.find(&terminal, false);
        assert_eq!(navigation.search_result, Some((2, 2)));
        navigation.case_sensitive = true;
        navigation.refresh_search(&terminal);
        assert_eq!(navigation.search_result, Some((1, 1)));
        assert_eq!(terminal.screen.lock().selection().unwrap().anchor.x, 4);
        navigation.query = "missing".into();
        navigation.refresh_search(&terminal);
        assert!(navigation.search_failed && navigation.search_result.is_none());
        navigation.query.clear();
        navigation.refresh_search(&terminal);
        assert!(!navigation.search_failed);
        assert!(terminal.screen.lock().selection().is_none());
    }
}
