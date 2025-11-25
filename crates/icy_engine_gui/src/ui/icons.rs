use iced::{Length, Theme, widget::svg};

const WARNING_SVG: &[u8] = include_bytes!("icons/warning.svg");
const ERROR_SVG: &[u8] = include_bytes!("icons/error.svg");

pub fn warning_icon<'a>(size: f32) -> svg::Svg<'a, Theme> {
    svg(svg::Handle::from_memory(WARNING_SVG))
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
}

pub fn error_icon<'a>(size: f32) -> svg::Svg<'a, Theme> {
    svg(svg::Handle::from_memory(ERROR_SVG)).width(Length::Fixed(size)).height(Length::Fixed(size))
}
