//! SVG icons for animation editor playback controls

use iced::{widget::svg, Element, Length};

// Playback control icons (Material Design)
const PLAY_ARROW_SVG: &[u8] = include_bytes!("icons/play_arrow.svg");
const PAUSE_SVG: &[u8] = include_bytes!("icons/pause.svg");
const SKIP_PREVIOUS_SVG: &[u8] = include_bytes!("icons/skip_previous.svg");
const SKIP_NEXT_SVG: &[u8] = include_bytes!("icons/skip_next.svg");
const FIRST_PAGE_SVG: &[u8] = include_bytes!("icons/first_page.svg");
const LAST_PAGE_SVG: &[u8] = include_bytes!("icons/last_page.svg");
const REPEAT_SVG: &[u8] = include_bytes!("icons/repeat.svg");
const REPLAY_SVG: &[u8] = include_bytes!("icons/replay.svg");

/// Icon size for playback controls
pub const ICON_SIZE: f32 = 36.0;

fn create_icon<'a, Message: 'a>(data: &'static [u8], size: f32) -> Element<'a, Message> {
    svg(svg::Handle::from_memory(data))
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .into()
}

pub fn play_icon<'a, Message: 'a>() -> Element<'a, Message> {
    create_icon(PLAY_ARROW_SVG, ICON_SIZE)
}

pub fn pause_icon<'a, Message: 'a>() -> Element<'a, Message> {
    create_icon(PAUSE_SVG, ICON_SIZE)
}

pub fn skip_previous_icon<'a, Message: 'a>() -> Element<'a, Message> {
    create_icon(SKIP_PREVIOUS_SVG, ICON_SIZE)
}

pub fn skip_next_icon<'a, Message: 'a>() -> Element<'a, Message> {
    create_icon(SKIP_NEXT_SVG, ICON_SIZE)
}

pub fn first_page_icon<'a, Message: 'a>() -> Element<'a, Message> {
    create_icon(FIRST_PAGE_SVG, ICON_SIZE)
}

pub fn last_page_icon<'a, Message: 'a>() -> Element<'a, Message> {
    create_icon(LAST_PAGE_SVG, ICON_SIZE)
}

pub fn repeat_icon<'a, Message: 'a>() -> Element<'a, Message> {
    create_icon(REPEAT_SVG, ICON_SIZE)
}

pub fn replay_icon<'a, Message: 'a>() -> Element<'a, Message> {
    create_icon(REPLAY_SVG, ICON_SIZE)
}
