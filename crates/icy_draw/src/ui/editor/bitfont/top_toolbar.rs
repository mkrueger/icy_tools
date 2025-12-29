//! Top toolbar component for BitFont Editor
//!
//! Shows keyboard shortcut hints for actions not in the menu.

use iced::{
    widget::{container, row, text},
    Element, Length, Task,
};

use crate::ui::editor::ansi::{ColorSwitcher, ColorSwitcherMessage};

/// Messages from the top toolbar
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum BitFontTopToolbarMessage {
    /// Toggle filled shapes
    ToggleFilled(bool),
    /// Go to next character
    NextChar,
    /// Go to previous character
    PrevChar,
    /// Color switcher message
    ColorSwitcher(ColorSwitcherMessage),
}

/// Top toolbar state
pub struct BitFontTopToolbar {
    /// Shape filled toggle
    pub filled: bool,
    /// Color switcher
    pub color_switcher: ColorSwitcher,
    /// Current foreground color
    pub foreground: u32,
    /// Current background color
    pub background: u32,
}

impl Default for BitFontTopToolbar {
    fn default() -> Self {
        Self::new()
    }
}

impl BitFontTopToolbar {
    pub fn new() -> Self {
        Self {
            filled: false,
            color_switcher: ColorSwitcher::new(),
            foreground: 7, // Light gray (DOS default)
            background: 0, // Black
        }
    }

    /// Update the top toolbar state
    pub fn update(&mut self, message: BitFontTopToolbarMessage) -> Task<BitFontTopToolbarMessage> {
        match message {
            BitFontTopToolbarMessage::ToggleFilled(v) => self.filled = v,
            BitFontTopToolbarMessage::NextChar | BitFontTopToolbarMessage::PrevChar => {
                // Handled by parent
            }
            BitFontTopToolbarMessage::ColorSwitcher(msg) => match msg {
                ColorSwitcherMessage::SwapColors => {
                    self.color_switcher.start_swap_animation();
                }
                ColorSwitcherMessage::ResetToDefault => {
                    self.foreground = 7;
                    self.background = 0;
                    self.color_switcher.confirm_swap();
                }
                ColorSwitcherMessage::Tick(delta) => {
                    if self.color_switcher.tick(delta) {
                        return Task::done(BitFontTopToolbarMessage::ColorSwitcher(ColorSwitcherMessage::AnimationComplete));
                    }
                }
                ColorSwitcherMessage::AnimationComplete => {
                    std::mem::swap(&mut self.foreground, &mut self.background);
                    self.color_switcher.confirm_swap();
                }
            },
        }
        Task::none()
    }

    pub fn view_color_switcher(&self) -> Element<'_, BitFontTopToolbarMessage> {
        self.color_switcher
            .view(self.foreground, self.background)
            .map(BitFontTopToolbarMessage::ColorSwitcher)
    }

    /// Render the top toolbar with keyboard shortcut hints
    /// Shows only non-obvious shortcuts that are not in the menu
    pub fn view(&self) -> Element<'_, BitFontTopToolbarMessage> {
        let hints = row![
            Self::hint("Ctrl+Arrow", "Slide"),
            Self::sep(),
            Self::hint("Alt+Arrow", "Ins/Del Line/Col"),
            Self::sep(),
            Self::hint("+/-", "Next/Prev Char"),
            Self::sep(),
            Self::hint("MMB", "Toggle Select"),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);

        container(hints)
            .width(Length::Fill)
            .height(Length::Fixed(40.0))
            .padding([4, 8])
            .center_y(Length::Fill)
            .style(container::rounded_box)
            .into()
    }

    /// Create a hint label: "Key: Action"
    fn hint<'a>(key: &'a str, action: &'a str) -> Element<'a, BitFontTopToolbarMessage> {
        row![
            text(key).size(14).style(|theme: &iced::Theme| {
                text::Style {
                    color: Some(theme.background.on),
                }
            }),
            text(":")
                .style(|theme: &iced::Theme| {
                    text::Style {
                        color: Some(theme.secondary.on),
                    }
                })
                .size(14),
            text(action)
                .style(|theme: &iced::Theme| {
                    text::Style {
                        color: Some(theme.secondary.on),
                    }
                })
                .size(14),
        ]
        .spacing(4)
        .into()
    }

    /// Visual separator (vertical bar)
    fn sep() -> Element<'static, BitFontTopToolbarMessage> {
        text("|")
            .size(14)
            .style(|theme: &iced::Theme| text::Style {
                color: Some(theme.button.base),
            })
            .into()
    }
}
