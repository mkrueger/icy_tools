//! Shuffle mode - slideshow-like display of random files from current container
//!
//! Features:
//! - Displays random files with auto-scroll
//! - Shows SAUCE comments with fade-in effect
//! - Displays title/author/group info overlay
//! - Auto-advances after scroll completes
//! - Exits on Escape/Enter/Mouse click

use std::time::Instant;

use iced::{
    Color, Element, Length,
    widget::{Space, column, container, row, stack, text},
};
use rand::seq::SliceRandom;

// ============================================================================
// TIMING CONSTANTS - Adjust these to change shuffle mode behavior
// ============================================================================

/// Minimum time to show each file before advancing (seconds)
/// If scrolling/comments finish earlier, this ensures a minimum display time
const MIN_SHOW_TIME_SECS: f32 = 7.0;

/// How long to wait after scroll completes before advancing to next file (seconds)
const POST_SCROLL_DELAY_SECS: f32 = 3.0;

/// Delay before comments start appearing after file loads (seconds)
const COMMENT_START_DELAY_SECS: f32 = 0.0;

/// Duration for comment fade in effect (seconds)
const COMMENT_FADE_DURATION_SECS: f32 = 0.5;

/// Speed at which comments scroll up (pixels per second)
const COMMENT_SCROLL_SPEED: f32 = 48.0;

// ============================================================================
// DISPLAY CONSTANTS - Font sizes, colors, and layout
// ============================================================================

/// Font size for comment text
const COMMENT_FONT_SIZE: f32 = 24.0;

/// Line height for comments (pixels)
const COMMENT_LINE_HEIGHT: f32 = 32.0;

/// Spacing between comment lines (pixels)
const COMMENT_LINE_SPACING: f32 = 6.0;

/// Shadow offset for comment text (pixels)
const COMMENT_SHADOW_OFFSET: f32 = 2.0;

/// Padding around comments (pixels)
const COMMENT_PADDING: f32 = 20.0;

/// Font size for title text
const TITLE_FONT_SIZE: f32 = 32.0;

/// Font size for author/group text
const AUTHOR_GROUP_FONT_SIZE: f32 = 24.0;

/// Background opacity for info overlay
const INFO_OVERLAY_BG_OPACITY: f32 = 0.6;

// ============================================================================
// SCREEN ZONE RATIOS - Control where comments fade in/out
// ============================================================================

/// Zone where text becomes fully visible (fraction from top, 0.75 = 1/4 from bottom)
const FULLY_VISIBLE_ZONE_RATIO: f32 = 0.75;

/// Zone where text starts fading out (fraction from top, 0.55 = just above middle)
const FADE_OUT_START_RATIO: f32 = 0.55;

/// Zone where text is fully invisible (fraction from top, 0.45 = just below middle)
const FADE_OUT_END_RATIO: f32 = 0.35;

/// Where comments start scrolling from (1.0 = screen bottom, 1.1 = 10% below)
const COMMENT_START_OFFSET_RATIO: f32 = 1.0;

/// Messages for shuffle mode
#[derive(Debug, Clone)]
pub enum ShuffleModeMessage {
    /// Exit shuffle mode (Escape/Enter/Click)
    Exit,
    /// Advance to next file
    NextFile,
    /// Animation tick
    Tick(f32),
}

/// State for a single comment line
#[derive(Debug, Clone)]
struct CommentLineState {
    text: String,
}

/// Shuffle mode state
pub struct ShuffleMode {
    /// List of item indices to shuffle through (indices into file_browser.files)
    item_indices: Vec<usize>,
    /// Current position in shuffled list
    current_position: usize,
    /// Whether shuffle mode is active
    pub is_active: bool,
    /// When the current file started displaying (for minimum show time)
    file_started_at: Option<Instant>,
    /// When the current file finished scrolling (for post-scroll delay)
    scroll_finished_at: Option<Instant>,
    /// Whether we're waiting for scroll to complete
    waiting_for_scroll: bool,
    /// Current SAUCE info for overlay display
    current_title: Option<String>,
    current_author: Option<String>,
    current_group: Option<String>,
    /// Comment lines to display
    comment_lines: Vec<String>,
    /// Current comment animation state
    comment_states: Vec<CommentLineState>,
    /// When comments started displaying
    comments_started_at: Option<Instant>,
    /// Current scroll offset for soft scrolling (in pixels)
    scroll_offset: f32,
    /// Time accumulator for comment display
    comment_timer: f32,
    /// Overall fade-in opacity for the entire comment block
    comment_block_opacity: f32,
    /// Whether all comments have scrolled off the screen
    comments_finished: bool,
    /// Last known screen height (for calculating when comments are done)
    last_screen_height: f32,
}

impl ShuffleMode {
    pub fn new() -> Self {
        Self {
            item_indices: Vec::new(),
            current_position: 0,
            is_active: false,
            file_started_at: None,
            scroll_finished_at: None,
            waiting_for_scroll: true,
            current_title: None,
            current_author: None,
            current_group: None,
            comment_lines: Vec::new(),
            comment_states: Vec::new(),
            comments_started_at: None,
            scroll_offset: 0.0,
            comment_timer: 0.0,
            comment_block_opacity: 0.0,
            comments_finished: false,
            last_screen_height: 800.0,
        }
    }

    /// Start shuffle mode with the given item indices
    pub fn start(&mut self, indices: Vec<usize>) {
        if indices.is_empty() {
            return;
        }

        let mut shuffled = indices;
        let mut rng = rand::rng();
        shuffled.shuffle(&mut rng);

        self.item_indices = shuffled;
        self.current_position = 0;
        self.is_active = true;
        self.file_started_at = Some(Instant::now());
        self.scroll_finished_at = None;
        self.waiting_for_scroll = true;
        self.clear_sauce_info();
    }

    /// Stop shuffle mode
    pub fn stop(&mut self) {
        self.is_active = false;
        self.item_indices.clear();
        self.current_position = 0;
        self.clear_sauce_info();
    }

    /// Get the current item index to display
    pub fn current_item_index(&self) -> Option<usize> {
        if self.is_active && self.current_position < self.item_indices.len() {
            Some(self.item_indices[self.current_position])
        } else {
            None
        }
    }

    /// Advance to the next item, returns the new item index if available
    pub fn next_item(&mut self) -> Option<usize> {
        if !self.is_active {
            return None;
        }

        self.current_position += 1;
        if self.current_position >= self.item_indices.len() {
            // Reshuffle and start over
            let mut rng = rand::rng();
            self.item_indices.shuffle(&mut rng);
            self.current_position = 0;
        }

        self.file_started_at = Some(Instant::now());
        self.scroll_finished_at = None;
        self.waiting_for_scroll = true;
        self.clear_sauce_info();

        self.current_item_index()
    }

    /// Set SAUCE info for current file
    pub fn set_sauce_info(&mut self, title: Option<String>, author: Option<String>, group: Option<String>, comments: Vec<String>) {
        self.current_title = title.filter(|s| !s.trim().is_empty());
        self.current_author = author.filter(|s| !s.trim().is_empty());
        self.current_group = group.filter(|s| !s.trim().is_empty());

        // Filter empty comment lines
        self.comment_lines = comments.into_iter().filter(|s| !s.trim().is_empty()).collect();

        // Initialize comment animation states
        self.comment_states = self.comment_lines.iter().map(|text| CommentLineState { text: text.clone() }).collect();

        self.comments_started_at = Some(Instant::now());
        self.scroll_offset = 0.0;
        self.comment_timer = 0.0;
        self.comment_block_opacity = 0.0;
        self.comments_finished = false;
    }

    fn clear_sauce_info(&mut self) {
        self.current_title = None;
        self.current_author = None;
        self.current_group = None;
        self.comment_lines.clear();
        self.comment_states.clear();
        self.comments_started_at = None;
        self.scroll_offset = 0.0;
        self.comment_timer = 0.0;
        self.comment_block_opacity = 0.0;
        self.comments_finished = false;
    }

    /// Notify that scrolling has completed
    pub fn notify_scroll_complete(&mut self) {
        if self.waiting_for_scroll {
            self.waiting_for_scroll = false;
            // Don't set scroll_finished_at yet - wait for comments to finish too
            if self.comment_lines.is_empty() {
                // No comments, we're done
                self.scroll_finished_at = Some(Instant::now());
                self.comments_finished = true;
            }
        }
    }

    /// Check if we should advance to the next file
    /// Priority:
    /// 1. If scroll + comments finished → wait post-scroll delay (3s) → advance
    /// 2. If minimum show time (7s) elapsed and scroll NOT finished → advance (fallback)
    pub fn should_advance(&mut self) -> bool {
        if !self.is_active {
            return false;
        }

        // Check if scrolling and comments are both finished
        let scroll_complete = !self.waiting_for_scroll && self.comments_finished;

        if scroll_complete {
            // Normal case: scroll finished, check post-scroll delay
            if let Some(finished_at) = self.scroll_finished_at {
                return finished_at.elapsed().as_secs_f32() >= POST_SCROLL_DELAY_SECS;
            }
        } else {
            // little hack: if file has started scrolling remove the file start timer, so it scrolled.
            // end timer gives some delay after scrolling is done.
            // start timer is only for small files.
            let should_remove_file_start_timer = if let Some(started_at) = self.file_started_at {
                started_at.elapsed().as_secs_f32() >= 0.1
            } else {
                false
            };

            if should_remove_file_start_timer {
                println!("Removing file start timer because scrolling is done.");
                self.file_started_at = None;
            }
        }

        // Fallback: if scroll is NOT complete but minimum time has passed, advance anyway
        if let Some(started_at) = self.file_started_at {
            if started_at.elapsed().as_secs_f32() >= MIN_SHOW_TIME_SECS {
                return true;
            }
        }

        false
    }

    /// Check if all comments have scrolled off the top of the screen
    fn check_comments_finished(&self) -> bool {
        if self.comment_lines.is_empty() {
            return true;
        }

        let total_line_height = COMMENT_LINE_HEIGHT + COMMENT_LINE_SPACING;
        let num_lines = self.comment_states.len() as f32;

        // Use stored screen height or default if not set
        let screen_height = if self.last_screen_height > 0.0 { self.last_screen_height } else { 800.0 };

        // Start offset matches comments_overlay (starts below screen)
        let start_offset = screen_height * COMMENT_START_OFFSET_RATIO;

        // All comments have scrolled off when the last line has passed the fade_out_end zone
        // Last line starts at: start_offset + (num_lines - 1) * total_line_height
        // Current position: start_offset + (num_lines - 1) * total_line_height - scroll_offset
        let last_line_y = start_offset + ((num_lines - 1.0) * total_line_height) - self.scroll_offset;

        // Consider finished when last line has scrolled past the fade out end zone
        let fade_out_end = screen_height * FADE_OUT_END_RATIO;
        last_line_y < fade_out_end
    }

    /// Update comment animations - smooth pixel-by-pixel scroll effect
    pub fn tick(&mut self, delta_seconds: f32) {
        if !self.is_active {
            return;
        }

        // If no comments, nothing to animate
        if self.comment_lines.is_empty() {
            return;
        }

        // Check if we should start showing comments
        if let Some(started_at) = self.comments_started_at {
            let elapsed = started_at.elapsed().as_secs_f32();
            if elapsed < COMMENT_START_DELAY_SECS {
                return;
            }
        } else {
            return;
        }

        self.comment_timer += delta_seconds;

        // Fade in the comment block initially
        if self.comment_block_opacity < 1.0 {
            self.comment_block_opacity = (self.comment_block_opacity + delta_seconds / COMMENT_FADE_DURATION_SECS).min(1.0);
        }

        // Smooth pixel-by-pixel scroll
        self.scroll_offset += delta_seconds * COMMENT_SCROLL_SPEED;

        // Check if all comments have finished scrolling
        if !self.comments_finished && self.check_comments_finished() {
            self.comments_finished = true;
            self.scroll_finished_at = Some(Instant::now());
        }
    }

    /// Create the SAUCE info overlay (title/author/group at top)
    pub fn info_overlay(&self) -> Element<'_, ShuffleModeMessage> {
        if !self.is_active {
            return Space::new().into();
        }

        let mut info_parts: Vec<Element<'_, ShuffleModeMessage>> = Vec::new();

        // Title
        if let Some(ref title) = self.current_title {
            info_parts.push(text(title).size(TITLE_FONT_SIZE).color(Color::WHITE).into());
        }

        // Author & Group on same line
        let mut author_group: Vec<Element<'_, ShuffleModeMessage>> = Vec::new();
        if let Some(ref author) = self.current_author {
            author_group.push(
                text(format!("by {}", author))
                    .size(AUTHOR_GROUP_FONT_SIZE)
                    .color(Color::from_rgb(0.9, 0.9, 0.6)) // Yellow like status bar
                    .into(),
            );
        }
        if let Some(ref group) = self.current_group {
            if !author_group.is_empty() {
                author_group.push(text(" / ").size(AUTHOR_GROUP_FONT_SIZE).color(Color::from_rgb(0.7, 0.7, 0.7)).into());
            }
            author_group.push(
                text(group)
                    .size(AUTHOR_GROUP_FONT_SIZE)
                    .color(Color::from_rgb(0.6, 0.9, 0.6)) // Green like status bar
                    .into(),
            );
        }

        if !author_group.is_empty() {
            info_parts.push(row(author_group).spacing(4).into());
        }

        if info_parts.is_empty() {
            return Space::new().into();
        }

        let info_column = column(info_parts).spacing(4).padding(16);

        // Wrap in container with semi-transparent background
        container(info_column)
            .style(|_theme| container::Style {
                background: Some(iced::Background::Color(Color::from_rgba(0.0, 0.0, 0.0, INFO_OVERLAY_BG_OPACITY))),
                border: iced::Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 0.0.into(),
                },
                ..Default::default()
            })
            .into()
    }

    /// Create the comments overlay (scrolls from bottom, fades out at 1/2 screen)
    /// Uses pixel-perfect scrolling by calculating exact position offsets
    pub fn comments_overlay(&self, screen_height: f32) -> Element<'_, ShuffleModeMessage> {
        if !self.is_active || self.comment_states.is_empty() || self.comment_block_opacity < 0.01 {
            return Space::new().into();
        }

        let total_line_height = COMMENT_LINE_HEIGHT + COMMENT_LINE_SPACING;

        // Screen zones for opacity calculation:
        // - Bottom of screen = where text enters (starts invisible)
        // - 1/3 from bottom = fully visible zone
        // - 1/2 screen = where text starts fading out
        // - Top = invisible

        let screen_bottom = screen_height;
        let fully_visible_zone = screen_height * FULLY_VISIBLE_ZONE_RATIO;
        let fade_out_start = screen_height * FADE_OUT_START_RATIO;
        let fade_out_end = screen_height * FADE_OUT_END_RATIO;

        // Build comments with opacity based on their scroll position
        let mut elements: Vec<Element<'_, ShuffleModeMessage>> = Vec::new();

        // Add initial spacer to push content below the screen (starts at bottom)
        // When scroll_offset = 0, spacer = screen_height * ratio (content at/below screen bottom)
        // As scroll_offset increases, spacer shrinks (content scrolls up into view)
        let start_offset = screen_height * COMMENT_START_OFFSET_RATIO;
        let initial_spacer = (start_offset - self.scroll_offset).max(0.0);
        if initial_spacer > 0.0 {
            elements.push(Space::new().height(initial_spacer).into());
        }

        for (i, state) in self.comment_states.iter().enumerate() {
            // Calculate this line's screen position for opacity
            let line_offset = i as f32 * total_line_height;
            let screen_y = initial_spacer + line_offset;

            // Calculate opacity based on screen position
            let opacity = if screen_y > screen_bottom {
                // Below screen - invisible
                0.0
            } else if screen_y > fully_visible_zone {
                // Fading in from bottom
                let fade_distance = screen_bottom - fully_visible_zone;
                let pos_in_fade = screen_bottom - screen_y;
                (pos_in_fade / fade_distance).clamp(0.0, 1.0)
            } else if screen_y > fade_out_start {
                // Fully visible zone (between 1/2 and 1/3)
                1.0
            } else if screen_y > fade_out_end {
                // Fading out towards top
                let fade_distance = fade_out_start - fade_out_end;
                let pos_in_fade = screen_y - fade_out_end;
                (pos_in_fade / fade_distance).clamp(0.0, 1.0)
            } else {
                // Above fade out zone - invisible
                0.0
            };

            let final_opacity = self.comment_block_opacity * opacity;

            // Create text element with calculated opacity
            let shadow_color = Color::from_rgba(0.0, 0.0, 0.0, final_opacity * 0.8);
            let text_color = Color::from_rgba(1.0, 1.0, 1.0, final_opacity);

            // Text with shadow effect
            let comment_with_shadow = stack![
                // Shadow (offset)
                row![
                    Space::new().width(COMMENT_SHADOW_OFFSET),
                    column![
                        Space::new().height(COMMENT_SHADOW_OFFSET),
                        text(&state.text).size(COMMENT_FONT_SIZE).color(shadow_color),
                    ],
                ],
                // Main text
                text(&state.text).size(COMMENT_FONT_SIZE).color(text_color),
            ];

            elements.push(container(comment_with_shadow).height(total_line_height).into());
        }

        let comments_column = column(elements).padding(COMMENT_PADDING).align_x(iced::alignment::Horizontal::Center);

        container(comments_column)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .into()
    }

    /// Create the full shuffle mode overlay view
    pub fn overlay_view(&self, screen_height: f32) -> Element<'_, ShuffleModeMessage> {
        if !self.is_active {
            return Space::new().into();
        }

        let info = self.info_overlay();
        let comments = self.comments_overlay(screen_height);

        // Stack info at top, comments at bottom
        let overlay = stack![
            // Info overlay at top-left
            container(info)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(iced::alignment::Horizontal::Left)
                .align_y(iced::alignment::Vertical::Top),
            // Comments overlay
            comments,
        ];

        overlay.into()
    }

    /// Update screen height for finished calculation
    pub fn set_screen_height(&mut self, height: f32) {
        self.last_screen_height = height;
    }

    /// Check if shuffle mode needs animation ticks
    pub fn needs_animation(&self) -> bool {
        // Need animation while active:
        // - While minimum show time hasn't elapsed
        // - While comments are still scrolling
        // - During the post-scroll delay (waiting to advance to next file)
        if !self.is_active {
            return false;
        }

        // During minimum show time, always need ticks
        if let Some(started_at) = self.file_started_at {
            if started_at.elapsed().as_secs_f32() < MIN_SHOW_TIME_SECS + 0.2 {
                return true;
            }
        }

        // Still scrolling comments
        if !self.comments_finished {
            return true;
        }

        // During post-scroll delay, need ticks to check should_advance()
        if let Some(finished_at) = self.scroll_finished_at {
            if finished_at.elapsed().as_secs_f32() < POST_SCROLL_DELAY_SECS + 0.2 {
                return true;
            }
        }
        println!("Shuffle mode does not need animation ticks.");
        false
    }
}
