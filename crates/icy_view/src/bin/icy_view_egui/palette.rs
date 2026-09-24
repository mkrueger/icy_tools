//! Ctrl+P quick open and Ctrl+Shift+P command palette: a filter field over a ranked list.

use super::text;
use eframe::egui;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Files,
    Commands,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Target {
    Item(usize),
    Command(&'static str),
}

pub struct Entry {
    pub label: String,
    pub detail: String,
    pub target: Target,
}

pub struct Palette {
    pub kind: Kind,
    pub query: String,
    selected: usize,
    focus: bool,
}

pub enum Outcome {
    Open,
    Close,
    Activate(Target),
}

/// Case-insensitive subsequence match; contiguous runs and word starts rank higher.
pub fn score(query: &str, candidate: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }
    let candidate: Vec<char> = candidate.chars().flat_map(char::to_lowercase).collect();
    let mut position = 0;
    let mut score = 0;
    let mut previous: Option<usize> = None;
    let mut first = None;
    for wanted in query.chars().flat_map(char::to_lowercase).filter(|ch| !ch.is_whitespace()) {
        let found = candidate[position..].iter().position(|ch| *ch == wanted)? + position;
        score += 1;
        if previous.is_some_and(|previous| previous + 1 == found) {
            score += 6;
        }
        if found == 0 || !candidate[found - 1].is_alphanumeric() {
            score += 4;
        }
        first.get_or_insert(found);
        previous = Some(found);
        position = found + 1;
    }
    Some(score * 100 - first.unwrap_or(0) as i32 * 2 - candidate.len() as i32)
}

pub fn rank(query: &str, entries: &[Entry]) -> Vec<usize> {
    let mut ranked: Vec<(i32, usize)> = entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| score(query, &entry.label).map(|score| (score, index)))
        .collect();
    if !query.is_empty() {
        ranked.sort_by(|left, right| right.0.cmp(&left.0).then(left.1.cmp(&right.1)));
    }
    ranked.into_iter().map(|(_, index)| index).collect()
}

impl Palette {
    pub fn new(kind: Kind) -> Self {
        Self {
            kind,
            query: String::new(),
            selected: 0,
            focus: true,
        }
    }

    pub fn show(&mut self, context: &egui::Context, entries: &[Entry]) -> Outcome {
        let ranked = rank(&self.query, entries);
        self.selected = self.selected.min(ranked.len().saturating_sub(1));
        let (up, down, enter, escape) = context.input_mut(|input| {
            (
                input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                input.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
                input.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
            )
        });
        if escape {
            return Outcome::Close;
        }
        if up {
            self.selected = self.selected.saturating_sub(1);
        }
        if down {
            self.selected = (self.selected + 1).min(ranked.len().saturating_sub(1));
        }
        if enter {
            return ranked
                .get(self.selected)
                .map_or(Outcome::Open, |index| Outcome::Activate(entries[*index].target.clone()));
        }
        let width = (context.content_rect().width() - 40.0).clamp(280.0, 620.0);
        let area = egui::Modal::default_area(egui::Id::new("palette-area"))
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 64.0))
            .default_width(width);
        let mut outcome = Outcome::Open;
        let modal = egui::Modal::new(egui::Id::new("palette"))
            .area(area)
            .frame(egui::Frame::popup(&context.style()).inner_margin(8).corner_radius(10))
            .show(context, |ui| {
                ui.set_width(width);
                let hint = text(match self.kind {
                    Kind::Files => "egui-quick-open-hint",
                    Kind::Commands => "egui-command-palette-hint",
                });
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .id(egui::Id::new("palette-query"))
                        .hint_text(hint)
                        .margin(egui::vec2(8.0, 6.0))
                        .desired_width(f32::INFINITY),
                );
                if self.focus {
                    response.request_focus();
                    self.focus = false;
                }
                if response.changed() {
                    self.selected = 0;
                }
                ui.add_space(4.0);
                if ranked.is_empty() {
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new(text("filter-no-items-found")).weak());
                    ui.add_space(6.0);
                    return;
                }
                let row_height = 26.0;
                egui::ScrollArea::vertical()
                    .max_height(row_height * 12.0)
                    .auto_shrink([false, true])
                    .show_rows(ui, row_height, ranked.len(), |ui, rows| {
                        for row in rows {
                            let entry = &entries[ranked[row]];
                            let selected = row == self.selected;
                            let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), row_height), egui::Sense::click());
                            if selected || response.hovered() {
                                let fill = if selected {
                                    ui.visuals().selection.bg_fill
                                } else {
                                    ui.visuals().widgets.hovered.weak_bg_fill
                                };
                                ui.painter().rect_filled(rect, 5.0, fill);
                            }
                            if selected && (up || down) {
                                ui.scroll_to_rect(rect, None);
                            }
                            let inner = rect.shrink2(egui::vec2(8.0, 0.0));
                            let detail = ui
                                .painter()
                                .layout_no_wrap(entry.detail.clone(), egui::FontId::proportional(12.0), ui.visuals().weak_text_color());
                            let detail_width = detail.size().x.min(inner.width() * 0.45);
                            ui.painter().galley(
                                egui::pos2(inner.right() - detail_width, inner.center().y - detail.size().y / 2.0),
                                detail,
                                ui.visuals().weak_text_color(),
                            );
                            let label_rect = egui::Rect::from_min_max(inner.min, egui::pos2(inner.right() - detail_width - 12.0, inner.bottom()));
                            let mut child = ui.new_child(
                                egui::UiBuilder::new()
                                    .max_rect(label_rect)
                                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
                            );
                            child.add(egui::Label::new(&entry.label).truncate().selectable(false));
                            if response.clicked() {
                                outcome = Outcome::Activate(entry.target.clone());
                            }
                        }
                    });
            });
        if modal.should_close() && matches!(outcome, Outcome::Open) {
            outcome = Outcome::Close;
        }
        outcome
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_ranking_prefers_contiguous_and_word_start_matches() {
        assert!(score("xyz", "abc.ans").is_none());
        assert!(score("aca", "acid-art.ans").is_some());
        let entries: Vec<Entry> = ["us-mist.ans", "mistigris.ans", "zz_m_i_s_t.ans"]
            .iter()
            .enumerate()
            .map(|(index, label)| Entry {
                label: label.to_string(),
                detail: String::new(),
                target: Target::Item(index),
            })
            .collect();
        let ranked = rank("mist", &entries);
        assert_eq!(ranked.len(), 3);
        assert_eq!(entries[ranked[0]].label, "mistigris.ans");
        assert_eq!(entries[ranked[2]].label, "zz_m_i_s_t.ans");
        assert_eq!(rank("", &entries), vec![0, 1, 2]);
    }
}
