//! Auto-hiding overlay toolbar of the original thumbnail view.

use eframe::egui;
use std::time::{Duration, Instant};

/// Generous delay the first time the tile view is entered.
const INITIAL_HIDE_DELAY: f32 = 5.0;
const NORMAL_HIDE_DELAY: f32 = 1.5;
/// Edge area that brings the hidden toolbar back.
pub const HOVER_ZONE: f32 = 40.0;

pub struct AutoHide {
    pub visible: bool,
    pub rect: egui::Rect,
    hovered: bool,
    shown_at: Instant,
    first_show: bool,
}

impl Default for AutoHide {
    fn default() -> Self {
        Self {
            visible: true,
            rect: egui::Rect::ZERO,
            hovered: false,
            shown_at: Instant::now(),
            first_show: true,
        }
    }
}

impl AutoHide {
    /// Show the toolbar again with the long delay, as when the tile view is entered.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn hover(&mut self, hovered: bool) {
        if hovered {
            self.hovered = true;
            if !self.visible {
                self.visible = true;
                self.shown_at = Instant::now();
            }
        } else if self.hovered {
            self.hovered = false;
            self.shown_at = Instant::now();
        }
    }

    /// Hides the toolbar once its delay elapsed and reports when to look again.
    pub fn update(&mut self) -> Option<Duration> {
        if !self.visible || self.hovered {
            return None;
        }
        let delay = if self.first_show { INITIAL_HIDE_DELAY } else { NORMAL_HIDE_DELAY };
        let elapsed = self.shown_at.elapsed().as_secs_f32();
        if elapsed >= delay {
            self.visible = false;
            self.first_show = false;
            return None;
        }
        Some(Duration::from_secs_f32(delay - elapsed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toolbar_hides_after_the_initial_delay_and_returns_on_hover() {
        let mut toolbar = AutoHide::default();
        assert!(toolbar.visible);
        let remaining = toolbar.update().expect("the hide timer runs while visible");
        assert!(remaining <= Duration::from_secs_f32(INITIAL_HIDE_DELAY) && toolbar.visible);

        toolbar.shown_at = Instant::now() - Duration::from_secs_f32(NORMAL_HIDE_DELAY + 0.1);
        assert!(toolbar.update().is_some(), "the first delay is longer than the later one");
        assert!(toolbar.visible);

        toolbar.shown_at = Instant::now() - Duration::from_secs_f32(INITIAL_HIDE_DELAY + 0.1);
        assert!(toolbar.update().is_none());
        assert!(!toolbar.visible);

        toolbar.hover(true);
        assert!(toolbar.visible);
        assert!(toolbar.update().is_none(), "a hovered toolbar stays visible");
        toolbar.shown_at = Instant::now() - Duration::from_secs_f32(INITIAL_HIDE_DELAY + 0.1);
        assert!(toolbar.update().is_none() && toolbar.visible);

        toolbar.hover(false);
        assert!(toolbar.update().is_some() && toolbar.visible, "leaving restarts the timer");
        toolbar.shown_at = Instant::now() - Duration::from_secs_f32(NORMAL_HIDE_DELAY + 0.1);
        assert!(toolbar.update().is_none());
        assert!(!toolbar.visible, "after the first time the short delay applies");

        toolbar.reset();
        assert!(toolbar.visible);
        toolbar.shown_at = Instant::now() - Duration::from_secs_f32(NORMAL_HIDE_DELAY + 0.1);
        assert!(toolbar.update().is_some(), "entering the tile view restores the long delay");
    }
}
