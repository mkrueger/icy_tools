//! Material Design icons for navigation and toolbar

use iced::{Length, Theme, widget::svg};

// Navigation icons
const ARROW_BACK_SVG: &[u8] = include_bytes!("../../data/icons/arrow_back.svg");
const ARROW_FORWARD_SVG: &[u8] = include_bytes!("../../data/icons/arrow_forward.svg");
const ARROW_UPWARD_SVG: &[u8] = include_bytes!("../../data/icons/arrow_upward.svg");
const ARROW_DOWNWARD_SVG: &[u8] = include_bytes!("../../data/icons/arrow_downward.svg");
const REFRESH_SVG: &[u8] = include_bytes!("../../data/icons/refresh.svg");
const SEARCH_SVG: &[u8] = include_bytes!("../../data/icons/search.svg");
const SETTINGS_SVG: &[u8] = include_bytes!("../../../icy_engine_gui/src/ui/icons/settings.svg");
const LANGUAGE_SVG: &[u8] = include_bytes!("../../data/icons/language.svg");

// View mode icons
const GRID_VIEW_SVG: &[u8] = include_bytes!("../../data/icons/grid_view.svg");
const VIEW_LIST_SVG: &[u8] = include_bytes!("../../data/icons/view_list.svg");

// Action icons
const SHUFFLE_SVG: &[u8] = include_bytes!("../../data/icons/shuffle.svg");

// Sort icons
const SORT_BY_ALPHA_SVG: &[u8] = include_bytes!("../../data/icons/sort_by_alpha.svg");
const STRAIGHTEN_SVG: &[u8] = include_bytes!("../../data/icons/straighten.svg");
const CALENDAR_TODAY_SVG: &[u8] = include_bytes!("../../data/icons/calendar_today.svg");

/// Create an SVG icon widget
fn create_icon<'a>(data: &'static [u8], size: f32) -> svg::Svg<'a, Theme> {
    svg(svg::Handle::from_memory(data)).width(Length::Fixed(size)).height(Length::Fixed(size))
}

// Navigation icons
pub fn arrow_back_icon<'a>(size: f32) -> svg::Svg<'a, Theme> {
    create_icon(ARROW_BACK_SVG, size)
}

pub fn arrow_forward_icon<'a>(size: f32) -> svg::Svg<'a, Theme> {
    create_icon(ARROW_FORWARD_SVG, size)
}

pub fn arrow_upward_icon<'a>(size: f32) -> svg::Svg<'a, Theme> {
    create_icon(ARROW_UPWARD_SVG, size)
}

pub fn arrow_downward_icon<'a>(size: f32) -> svg::Svg<'a, Theme> {
    create_icon(ARROW_DOWNWARD_SVG, size)
}

pub fn refresh_icon<'a>(size: f32) -> svg::Svg<'a, Theme> {
    create_icon(REFRESH_SVG, size)
}

pub fn search_icon<'a>(size: f32) -> svg::Svg<'a, Theme> {
    create_icon(SEARCH_SVG, size)
}

pub fn settings_icon<'a>(size: f32) -> svg::Svg<'a, Theme> {
    create_icon(SETTINGS_SVG, size)
}

pub fn language_icon<'a>(size: f32) -> svg::Svg<'a, Theme> {
    create_icon(LANGUAGE_SVG, size)
}

// View mode icons
pub fn grid_view_icon<'a>(size: f32) -> svg::Svg<'a, Theme> {
    create_icon(GRID_VIEW_SVG, size)
}

pub fn view_list_icon<'a>(size: f32) -> svg::Svg<'a, Theme> {
    create_icon(VIEW_LIST_SVG, size)
}

// Action icons
pub fn shuffle_icon<'a>(size: f32) -> svg::Svg<'a, Theme> {
    create_icon(SHUFFLE_SVG, size)
}

// Sort icons
pub fn sort_by_alpha_icon<'a>(size: f32) -> svg::Svg<'a, Theme> {
    create_icon(SORT_BY_ALPHA_SVG, size)
}

pub fn straighten_icon<'a>(size: f32) -> svg::Svg<'a, Theme> {
    create_icon(STRAIGHTEN_SVG, size)
}

pub fn calendar_today_icon<'a>(size: f32) -> svg::Svg<'a, Theme> {
    create_icon(CALENDAR_TODAY_SVG, size)
}
