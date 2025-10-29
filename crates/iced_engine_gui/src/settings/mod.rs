use i18n_embed_fl::fl;
use iced::widget::{button, column, container, row, rule, slider, text};
use iced::{Alignment, Background, Border, Color, Element, Length, Shadow, Theme};
use icy_engine::Color as IcyColor;
use lazy_static::lazy_static;
use std::fmt;

// Import LANGUAGE_LOADER from the ui module
use crate::LANGUAGE_LOADER;
use crate::MonitorType;

pub mod msg;
pub use msg::*;

pub mod ui;
pub use ui::*;

// Design constants
const LABEL_WIDTH: f32 = 140.0;
const INPUT_WIDTH: f32 = 220.0;
const SECTION_PADDING: u16 = 20;
const ROW_SPACING: f32 = 12.0;
const SLIDER_LABEL_WIDTH: f32 = 140.0;
const SLIDER_VALUE_WIDTH: f32 = 50.0;

lazy_static! {
    static ref MONITOR_NAMES: [String; 7] = [
        fl!(LANGUAGE_LOADER, "settings-monitor-color"),
        fl!(LANGUAGE_LOADER, "settings-monitor-grayscale"),
        fl!(LANGUAGE_LOADER, "settings-monitor-amber"),
        fl!(LANGUAGE_LOADER, "settings-monitor-green"),
        fl!(LANGUAGE_LOADER, "settings-monitor-apple2"),
        fl!(LANGUAGE_LOADER, "settings-monitor-futuristic"),
        fl!(LANGUAGE_LOADER, "settings-monitor-custom"),
    ];
}

// Add Display trait for MonitorType to work with pick_list
impl fmt::Display for MonitorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let index = self.to_index();
        write!(f, "{}", MONITOR_NAMES[index])
    }
}

// Wrapper type for Theme to implement Display
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeOption(pub Theme);

impl From<Theme> for ThemeOption {
    fn from(theme: Theme) -> Self {
        ThemeOption(theme)
    }
}

impl From<ThemeOption> for Theme {
    fn from(option: ThemeOption) -> Self {
        option.0
    }
}

impl fmt::Display for ThemeOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Theme::Light => write!(f, "Light"),
            Theme::Dark => write!(f, "Dark"),
            Theme::Dracula => write!(f, "Dracula"),
            Theme::Nord => write!(f, "Nord"),
            Theme::SolarizedLight => write!(f, "Solarized Light"),
            Theme::SolarizedDark => write!(f, "Solarized Dark"),
            Theme::GruvboxLight => write!(f, "Gruvbox Light"),
            Theme::GruvboxDark => write!(f, "Gruvbox Dark"),
            Theme::CatppuccinLatte => write!(f, "Catppuccin Latte"),
            Theme::CatppuccinFrappe => write!(f, "Catppuccin Frappe"),
            Theme::CatppuccinMacchiato => write!(f, "Catppuccin Macchiato"),
            Theme::CatppuccinMocha => write!(f, "Catppuccin Mocha"),
            Theme::TokyoNight => write!(f, "Tokyo Night"),
            Theme::TokyoNightStorm => write!(f, "Tokyo Night Storm"),
            Theme::TokyoNightLight => write!(f, "Tokyo Night Light"),
            Theme::KanagawaWave => write!(f, "Kanagawa Wave"),
            Theme::KanagawaDragon => write!(f, "Kanagawa Dragon"),
            Theme::KanagawaLotus => write!(f, "Kanagawa Lotus"),
            Theme::Moonfly => write!(f, "Moonfly"),
            Theme::Nightfly => write!(f, "Nightfly"),
            Theme::Oxocarbon => write!(f, "Oxocarbon"),
            Theme::Ferra => write!(f, "Ferra"),
            Theme::Custom(_) => write!(f, "Custom"),
        }
    }
}

impl MonitorType {
    pub fn from_index(index: usize) -> Self {
        match index {
            0 => MonitorType::Color,
            1 => MonitorType::Grayscale,
            2 => MonitorType::Amber,
            3 => MonitorType::Green,
            4 => MonitorType::Apple2,
            5 => MonitorType::Futuristic,
            6 => MonitorType::CustomMonochrome,
            _ => MonitorType::Color,
        }
    }

    pub fn to_index(&self) -> usize {
        match self {
            MonitorType::Color => 0,
            MonitorType::Grayscale => 1,
            MonitorType::Amber => 2,
            MonitorType::Green => 3,
            MonitorType::Apple2 => 4,
            MonitorType::Futuristic => 5,
            MonitorType::CustomMonochrome => 6,
        }
    }
}

// Color picker button with preview
fn color_button<'a, Message: Clone + 'a>(color: Color, on_press: Message) -> Element<'a, Message> {
    button("")
        .on_press(on_press)
        .style(move |theme: &Theme, status| iced::widget::button::Style {
            background: Some(Background::Color(match status {
                iced::widget::button::Status::Active => color,
                iced::widget::button::Status::Hovered => Color {
                    r: (color.r * 1.1).min(1.0),
                    g: (color.g * 1.1).min(1.0),
                    b: (color.b * 1.1).min(1.0),
                    a: color.a,
                },
                iced::widget::button::Status::Pressed => Color {
                    r: (color.r * 0.9).max(0.0),
                    g: (color.g * 0.9).max(0.0),
                    b: (color.b * 0.9).max(0.0),
                    a: color.a,
                },
                iced::widget::button::Status::Disabled => Color {
                    r: color.r * 0.5,
                    g: color.g * 0.5,
                    b: color.b * 0.5,
                    a: 0.5,
                },
            })),
            text_color: Color::WHITE,
            border: Border {
                color: if matches!(status, iced::widget::button::Status::Hovered) {
                    theme.extended_palette().primary.strong.color
                } else {
                    Color::from_rgb(0.5, 0.5, 0.5)
                },
                width: if matches!(status, iced::widget::button::Status::Hovered) { 2.0 } else { 1.0 },
                radius: 4.0.into(),
            },
            shadow: if matches!(status, iced::widget::button::Status::Pressed) {
                Shadow::default()
            } else {
                Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.2),
                    offset: iced::Vector::new(0.0, 1.0),
                    blur_radius: 2.0,
                }
            },
            snap: false,
        })
        .width(Length::Fixed(60.0))
        .height(Length::Fixed(28.0))
        .into()
}

// Section header with styling
fn section_header(title: &str) -> Element<'_, MonitorSettingsMessage> {
    column![
        text(title).size(16).style(|theme: &Theme| {
            text::Style {
                color: Some(theme.extended_palette().primary.base.color),
            }
        }),
        rule::horizontal(1).style(|theme: &Theme| {
            iced::widget::rule::Style {
                color: theme.extended_palette().background.weak.color,
                radius: 0.0.into(),
                fill_mode: iced::widget::rule::FillMode::Full,
                snap: false,
            }
        }),
    ]
    .spacing(4)
    .into()
}

// Styled slider row
pub fn slider_row_owned<'a>(
    label: String,
    value: f32,
    range: std::ops::RangeInclusive<f32>,
    on_change: impl Fn(f32) -> MonitorSettingsMessage + 'a,
) -> Element<'a, MonitorSettingsMessage> {
    container(
        row![
            text(label).size(14).width(Length::Fixed(SLIDER_LABEL_WIDTH)),
            slider(range, value, on_change).width(Length::Fill).style(|theme: &Theme, status| {
                let palette = theme.extended_palette();
                iced::widget::slider::Style {
                    rail: iced::widget::slider::Rail {
                        backgrounds: (Background::Color(palette.primary.base.color), Background::Color(palette.background.weak.color)),
                        width: 4.0,
                        border: Border::default(),
                    },
                    handle: iced::widget::slider::Handle {
                        shape: iced::widget::slider::HandleShape::Circle { radius: 8.0 },
                        background: Background::Color(if status == iced::widget::slider::Status::Dragged {
                            palette.primary.strong.color
                        } else {
                            palette.primary.base.color
                        }),
                        border_color: Color::WHITE,
                        border_width: 2.0,
                    },
                }
            }),
            container(text(format!("{:.0}", value)).size(13).style(|theme: &Theme| {
                text::Style {
                    color: Some(theme.extended_palette().background.strong.text),
                }
            }))
            .width(Length::Fixed(SLIDER_VALUE_WIDTH))
            .style(|theme: &Theme| {
                container::Style {
                    background: Some(Background::Color(theme.extended_palette().background.weak.color)),
                    border: Border {
                        color: theme.extended_palette().background.strong.color,
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..Default::default()
                }
            })
            .padding(4)
            .center_x(Length::Fixed(SLIDER_VALUE_WIDTH))
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .padding([4, 0])
    .into()
}

// Helper functions for color conversion
pub fn iced_to_icy_color(color: Color) -> IcyColor {
    IcyColor::new((color.r * 255.0) as u8, (color.g * 255.0) as u8, (color.b * 255.0) as u8)
}

pub fn icy_to_iced_color(color: IcyColor) -> Color {
    let (r, g, b) = color.get_rgb();
    Color::from_rgb8(r, g, b)
}
