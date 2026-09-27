//! Slideshow overlay with the timing and fading SAUCE comments of the original viewer.

use eframe::egui;
use std::time::Instant;

/// Lower bound so short files do not flash by.
const MIN_SHOW_TIME: f32 = 7.0;
/// Pause after a long file has scrolled to the bottom.
const POST_SCROLL_DELAY: f32 = MIN_SHOW_TIME * 0.5;
const COMMENT_FADE_DURATION: f32 = 0.5;
const COMMENT_SCROLL_SPEED: f32 = 48.0;
const COMMENT_FONT_SIZE: f32 = 24.0;
const COMMENT_LINE_HEIGHT: f32 = 32.0;
const COMMENT_LINE_SPACING: f32 = 6.0;
const COMMENT_SHADOW_OFFSET: f32 = 2.0;
const TITLE_FONT_SIZE: f32 = 32.0;
const AUTHOR_GROUP_FONT_SIZE: f32 = 24.0;
const INFO_PADDING: f32 = 16.0;
const INFO_BG_OPACITY: f32 = 0.6;
const FULLY_VISIBLE_ZONE: f32 = 0.75;
const FADE_OUT_START: f32 = 0.55;
const FADE_OUT_END: f32 = 0.35;

pub struct Shuffle {
    queue: Vec<usize>,
    position: usize,
    started_at: Instant,
    last_tick: Instant,
    scrolled: bool,
    finished_at: Option<Instant>,
    title: Option<String>,
    author: Option<String>,
    group: Option<String>,
    comments: Vec<String>,
    comment_offset: f32,
    comment_opacity: f32,
    comments_done: bool,
    height: f32,
}

impl Shuffle {
    pub fn new(mut queue: Vec<usize>) -> Option<Self> {
        if queue.is_empty() {
            return None;
        }
        fastrand::shuffle(&mut queue);
        Some(Self {
            queue,
            position: 0,
            started_at: Instant::now(),
            last_tick: Instant::now(),
            scrolled: false,
            finished_at: None,
            title: None,
            author: None,
            group: None,
            comments: Vec::new(),
            comment_offset: 0.0,
            comment_opacity: 0.0,
            comments_done: true,
            height: 800.0,
        })
    }

    pub fn current(&self) -> Option<usize> {
        self.queue.get(self.position).copied()
    }

    pub fn peek_next(&self) -> Option<usize> {
        self.queue.get((self.position + 1) % self.queue.len()).copied()
    }

    pub fn advance(&mut self) -> Option<usize> {
        self.position += 1;
        if self.position >= self.queue.len() {
            fastrand::shuffle(&mut self.queue);
            self.position = 0;
        }
        self.restart();
        self.current()
    }

    fn restart(&mut self) {
        self.started_at = Instant::now();
        self.scrolled = false;
        self.finished_at = None;
        self.title = None;
        self.author = None;
        self.group = None;
        self.comments.clear();
        self.comment_offset = 0.0;
        self.comment_opacity = 0.0;
        self.comments_done = true;
    }

    pub fn set_sauce(&mut self, sauce: Option<&icy_sauce::SauceRecord>) {
        let field = |value: String| Some(value.trim().to_owned()).filter(|value| !value.is_empty());
        self.title = sauce.and_then(|sauce| field(sauce.title().to_string()));
        self.author = sauce.and_then(|sauce| field(sauce.author().to_string()));
        self.group = sauce.and_then(|sauce| field(sauce.group().to_string()));
        self.comments = sauce
            .map(|sauce| {
                sauce
                    .comments()
                    .iter()
                    .map(|comment| comment.to_string())
                    .filter(|comment| !comment.trim().is_empty())
                    .collect()
            })
            .unwrap_or_default();
        self.comment_offset = 0.0;
        self.comment_opacity = 0.0;
        self.comments_done = self.comments.is_empty();
        self.started_at = Instant::now();
    }

    /// The preview reached the bottom; comments may still be running.
    pub fn notify_scrolled(&mut self) {
        self.scrolled = true;
        if self.comments_done && self.finished_at.is_none() {
            self.finished_at = Some(Instant::now());
        }
    }

    pub fn should_advance(&self) -> bool {
        let Some(finished_at) = self.finished_at else {
            return false;
        };
        self.scrolled
            && self.comments_done
            && self.started_at.elapsed().as_secs_f32() >= MIN_SHOW_TIME
            && finished_at.elapsed().as_secs_f32() >= POST_SCROLL_DELAY
    }

    fn tick(&mut self) {
        let delta = self.last_tick.elapsed().as_secs_f32().min(0.1);
        self.last_tick = Instant::now();
        if self.comments.is_empty() {
            return;
        }
        self.comment_opacity = (self.comment_opacity + delta / COMMENT_FADE_DURATION).min(1.0);
        self.comment_offset += delta * COMMENT_SCROLL_SPEED;
        if !self.comments_done && self.last_line_position() < self.height * FADE_OUT_END {
            self.comments_done = true;
            if self.scrolled {
                self.finished_at = Some(Instant::now());
            }
        }
    }

    fn last_line_position(&self) -> f32 {
        let line = COMMENT_LINE_HEIGHT + COMMENT_LINE_SPACING;
        self.height + (self.comments.len() as f32 - 1.0) * line - self.comment_offset
    }

    pub fn show(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        self.height = rect.height().max(1.0);
        self.tick();
        let painter = ui.painter_at(rect);
        self.info(&painter, rect);
        let line = COMMENT_LINE_HEIGHT + COMMENT_LINE_SPACING;
        for (index, comment) in self.comments.iter().enumerate() {
            let y = self.height + index as f32 * line - self.comment_offset;
            if y > self.height {
                break;
            }
            let opacity = self.comment_opacity * fade(y, self.height);
            if opacity <= 0.01 {
                continue;
            }
            let position = egui::pos2(rect.center().x, rect.top() + y);
            let font = egui::FontId::proportional(COMMENT_FONT_SIZE);
            painter.text(
                position + egui::Vec2::splat(COMMENT_SHADOW_OFFSET),
                egui::Align2::CENTER_TOP,
                comment,
                font.clone(),
                egui::Color32::BLACK.gamma_multiply(opacity * 0.8),
            );
            painter.text(position, egui::Align2::CENTER_TOP, comment, font, egui::Color32::WHITE.gamma_multiply(opacity));
        }
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(16));
    }

    fn info(&self, painter: &egui::Painter, rect: egui::Rect) {
        if self.title.is_none() && self.author.is_none() && self.group.is_none() {
            return;
        }
        let mut lines: Vec<Vec<(String, egui::Color32, f32)>> = Vec::new();
        if let Some(title) = &self.title {
            lines.push(vec![(title.clone(), egui::Color32::WHITE, TITLE_FONT_SIZE)]);
        }
        let mut credits = Vec::new();
        if let Some(author) = &self.author {
            credits.push((format!("by {author}"), egui::Color32::from_rgb(230, 230, 153), AUTHOR_GROUP_FONT_SIZE));
        }
        if let Some(group) = &self.group {
            if !credits.is_empty() {
                credits.push((" / ".to_owned(), egui::Color32::from_rgb(179, 179, 179), AUTHOR_GROUP_FONT_SIZE));
            }
            credits.push((group.clone(), egui::Color32::from_rgb(153, 230, 153), AUTHOR_GROUP_FONT_SIZE));
        }
        if !credits.is_empty() {
            lines.push(credits);
        }
        let galleys: Vec<Vec<_>> = lines
            .iter()
            .map(|line| {
                line.iter()
                    .map(|(value, color, size)| painter.layout_no_wrap(value.clone(), egui::FontId::proportional(*size), *color))
                    .collect()
            })
            .collect();
        let width = galleys
            .iter()
            .map(|line| line.iter().map(|galley| galley.size().x).sum::<f32>())
            .fold(0.0_f32, f32::max);
        let height: f32 = galleys
            .iter()
            .map(|line| line.iter().map(|galley| galley.size().y).fold(0.0_f32, f32::max) + 4.0)
            .sum();
        let background = egui::Rect::from_min_size(rect.min, egui::vec2(width + INFO_PADDING * 2.0, height + INFO_PADDING * 2.0));
        painter.rect_filled(background, 0.0, egui::Color32::BLACK.gamma_multiply(INFO_BG_OPACITY));
        let mut top = background.top() + INFO_PADDING;
        for line in galleys {
            let mut left = background.left() + INFO_PADDING;
            let mut line_height = 0.0_f32;
            for galley in line {
                line_height = line_height.max(galley.size().y);
                let size = galley.size();
                painter.galley(egui::pos2(left, top), galley, egui::Color32::WHITE);
                left += size.x;
            }
            top += line_height + 4.0;
        }
    }
}

fn fade(y: f32, height: f32) -> f32 {
    let fully_visible = height * FULLY_VISIBLE_ZONE;
    let fade_out_start = height * FADE_OUT_START;
    let fade_out_end = height * FADE_OUT_END;
    if y > fully_visible {
        ((height - y) / (height - fully_visible).max(1.0)).clamp(0.0, 1.0)
    } else if y > fade_out_start {
        1.0
    } else if y > fade_out_end {
        ((y - fade_out_end) / (fade_out_start - fade_out_end).max(1.0)).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advancing_waits_for_scroll_comments_and_minimum_show_time() {
        let mut shuffle = Shuffle::new(vec![3, 7]).unwrap();
        assert!(shuffle.queue.contains(&3) && shuffle.queue.contains(&7));
        assert!(!shuffle.should_advance(), "must not advance before the file scrolled");

        shuffle.notify_scrolled();
        assert!(!shuffle.should_advance(), "minimum show time must still apply");

        shuffle.started_at = Instant::now() - std::time::Duration::from_secs_f32(MIN_SHOW_TIME + 1.0);
        assert!(!shuffle.should_advance(), "post scroll delay must still apply");
        shuffle.finished_at = Some(Instant::now() - std::time::Duration::from_secs_f32(POST_SCROLL_DELAY + 0.1));
        assert!(shuffle.should_advance());

        let first = shuffle.current().unwrap();
        let next = shuffle.advance().unwrap();
        assert_ne!(first, next);
        assert!(!shuffle.should_advance(), "a new file restarts the timers");
    }

    #[test]
    fn comments_scroll_upwards_and_finish_after_leaving_the_screen() {
        let mut shuffle = Shuffle::new(vec![0]).unwrap();
        shuffle.height = 600.0;
        shuffle.comments = vec!["first".into(), "second".into()];
        shuffle.comments_done = false;
        shuffle.notify_scrolled();
        assert!(shuffle.finished_at.is_none(), "comments still running");

        // Entering from the bottom stays invisible, the band above the middle is fully visible,
        // and lines fade out again towards the top.
        assert_eq!(fade(600.0, 600.0), 0.0);
        assert_eq!(fade(400.0, 600.0), 1.0);
        assert!(fade(300.0, 600.0) > 0.0 && fade(300.0, 600.0) < 1.0);
        assert_eq!(fade(0.0, 600.0), 0.0);

        shuffle.last_tick = Instant::now() - std::time::Duration::from_millis(100);
        shuffle.tick();
        assert!(shuffle.comment_offset > 0.0 && shuffle.comment_opacity > 0.0);

        shuffle.comment_offset = 10_000.0;
        shuffle.last_tick = Instant::now();
        shuffle.tick();
        assert!(shuffle.comments_done && shuffle.finished_at.is_some());
    }
}
