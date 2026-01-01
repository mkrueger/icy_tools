use i18n_embed_fl::fl;
use icy_ui::{
    widget::{button, container, row, text, Space},
    Alignment, Border, Color, Element, Event, Length, Theme,
};
use icy_engine_gui::command_handler;
use icy_parser_core::BaudEmulation;
use icy_sauce::{Capabilities, SauceRecord};

use crate::commands::{cmd, create_icy_view_commands};
use crate::LANGUAGE_LOADER;

/// SAUCE field colors - different for light and dark themes
struct SauceColors {
    title: Color,
    author: Color,
    group: Color,
    date: Color,
    size: Color,
    separator: Color,
}

impl SauceColors {
    fn for_theme(theme: &Theme) -> Self {
        let is_dark = theme.is_dark;
        if is_dark {
            // Dark theme - bright colors
            Self {
                title: Color::from_rgb(0.9, 0.9, 0.6),  // Yellow
                author: Color::from_rgb(0.6, 0.9, 0.6), // Green
                group: Color::from_rgb(0.6, 0.8, 0.9),  // Blue
                date: Color::from_rgb(0.7, 0.7, 0.7),   // Gray
                size: Color::from_rgb(0.8, 0.6, 0.8),   // Purple
                separator: Color::from_rgb(0.4, 0.4, 0.4),
            }
        } else {
            // Light theme - darker, more saturated colors
            Self {
                title: Color::from_rgb(0.6, 0.5, 0.0),  // Dark yellow/gold
                author: Color::from_rgb(0.0, 0.5, 0.0), // Dark green
                group: Color::from_rgb(0.0, 0.4, 0.6),  // Dark blue
                date: Color::from_rgb(0.4, 0.4, 0.4),   // Gray
                size: Color::from_rgb(0.5, 0.2, 0.5),   // Dark purple
                separator: Color::from_rgb(0.6, 0.6, 0.6),
            }
        }
    }
}

/// Status bar messages
#[derive(Debug, Clone)]
pub enum StatusBarMessage {
    /// Toggle baud emulation popup/cycle through rates
    CycleBaudEmulation,
    /// Cycle baud rate forward (same as CycleBaudEmulation, for keyboard shortcut naming)
    CycleBaudRate,
    /// Cycle baud rate backward
    CycleBaudRateBackward,
    /// Set baud rate to Off (max speed)
    SetBaudRateOff,
    /// Toggle auto-scroll mode
    ToggleAutoScroll,
    /// Cycle scroll speed (slow/medium/fast)
    CycleScrollSpeed,
    /// Cycle scroll speed backward (fast/medium/slow)
    CycleScrollSpeedBackward,
    /// Show SAUCE dialog
    ShowSauceInfo,
}

// Command handler for StatusBar
command_handler!(StatusBarCommands, create_icy_view_commands(), => StatusBarMessage {
    cmd::DIALOG_SAUCE => StatusBarMessage::ShowSauceInfo,
    cmd::PLAYBACK_SCROLL_SPEED => StatusBarMessage::CycleScrollSpeed,
    cmd::PLAYBACK_SCROLL_SPEED_BACK => StatusBarMessage::CycleScrollSpeedBackward,
    cmd::PLAYBACK_BAUD_RATE => StatusBarMessage::CycleBaudRate,
    cmd::PLAYBACK_BAUD_RATE_BACK => StatusBarMessage::CycleBaudRateBackward,
    cmd::PLAYBACK_BAUD_RATE_OFF => StatusBarMessage::SetBaudRateOff,
});

/// Status bar
#[derive(Clone, Default)]
pub struct StatusInfo {
    /// Selected file name
    pub file_name: Option<String>,
    /// File size in bytes
    pub file_size: Option<u64>,
    /// Content size (file size without SAUCE record)
    pub content_size: Option<usize>,
    /// Number of items in current directory
    pub item_count: usize,
    /// Number of selected items
    pub selected_count: usize,
    /// Additional status message
    pub message: Option<String>,
    /// Current baud emulation setting
    pub baud_emulation: BaudEmulation,
    /// Whether a file is currently being viewed
    pub is_viewing_file: bool,
    /// Sauce information (if available)
    pub sauce_info: Option<SauceRecord>,
    /// Buffer size (width x height) from the actual screen
    pub buffer_size: Option<(i32, i32)>,
    /// Whether auto-scroll is enabled (setting)
    pub auto_scroll_enabled: bool,
    /// Archive info: (archive type name, file size in bytes)
    pub archive_info: Option<(String, u64)>,
    /// Current zoom level (1.0 = 100%)
    pub zoom_level: Option<f32>,
}

impl StatusInfo {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_item_count(mut self, count: usize) -> Self {
        self.item_count = count;
        self
    }

    pub fn with_baud_emulation(mut self, baud: BaudEmulation) -> Self {
        self.baud_emulation = baud;
        self
    }

    pub fn with_viewing_file(mut self, viewing: bool) -> Self {
        self.is_viewing_file = viewing;
        self
    }

    pub fn with_buffer_size(mut self, size: Option<(i32, i32)>) -> Self {
        self.buffer_size = size;
        self
    }

    pub fn with_content_size(mut self, size: Option<usize>) -> Self {
        self.content_size = size;
        self
    }

    pub fn with_auto_scroll_enabled(mut self, enabled: bool) -> Self {
        self.auto_scroll_enabled = enabled;
        self
    }

    pub fn with_zoom_level(mut self, zoom: Option<f32>) -> Self {
        self.zoom_level = zoom;
        self
    }
}

/// Format file size in human-readable format
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Status bar widget
pub struct StatusBar {
    /// Command handler for status bar shortcuts
    commands: StatusBarCommands,
}

impl StatusBar {
    pub fn new() -> Self {
        Self {
            commands: StatusBarCommands::new(),
        }
    }

    /// Handle an event and return the corresponding message if it matches a command
    pub fn handle_event(&self, event: &Event) -> Option<StatusBarMessage> {
        self.commands.handle(event)
    }
}

/// Format sauce capabilities info (excluding size which is shown from real buffer)
fn format_sauce_info(sauce: &SauceRecord) -> String {
    if let Some(caps) = sauce.capabilities() {
        match caps {
            Capabilities::Character(caps) => {
                let mut info_parts = Vec::new();
                if caps.ice_colors {
                    info_parts.push("iCE".to_string());
                }
                if let Some(font) = caps.font() {
                    let font = font.to_string();
                    let font = font.trim();
                    if !font.is_empty() {
                        info_parts.push(font.to_string());
                    }
                }
                info_parts.join(" ")
            }
            Capabilities::Bitmap(caps) => {
                format!("{}bpp", caps.pixel_depth)
            }
            Capabilities::Audio(caps) => {
                if caps.sample_rate > 0 {
                    format!("{} Hz", caps.sample_rate)
                } else {
                    String::new()
                }
            }
            _ => String::new(),
        }
    } else {
        String::new()
    }
}

impl StatusBar {
    pub fn view(info: &StatusInfo, theme: &Theme) -> Element<'static, StatusBarMessage> {
        let colors = SauceColors::for_theme(theme);

        // Build left content based on whether we have sauce info
        let left_content: Element<'static, StatusBarMessage> = if let Some(ref sauce) = info.sauce_info {
            // Build sauce info display: Title • Author • Group • Date • Info
            let mut parts: Vec<Element<'static, StatusBarMessage>> = Vec::new();

            let title = sauce.title().to_string();
            let title = title.trim();
            if !title.is_empty() {
                parts.push(text(title.to_string()).size(12).color(colors.title).into());
            }

            let author = sauce.author().to_string();
            let author = author.trim();
            if !author.is_empty() {
                if !parts.is_empty() {
                    parts.push(text(" • ").size(12).color(colors.separator).into());
                }
                parts.push(text(author.to_string()).size(12).color(colors.author).into());
            }

            let group = sauce.group().to_string();
            let group = group.trim();
            if !group.is_empty() {
                if !parts.is_empty() {
                    parts.push(text(" • ").size(12).color(colors.separator).into());
                }
                parts.push(text(group.to_string()).size(12).color(colors.group).into());
            }

            // Date
            let date = sauce.date().to_string();
            let date = date.trim();
            if !date.is_empty() && date != "00000000" {
                if !parts.is_empty() {
                    parts.push(text(" • ").size(12).color(colors.separator).into());
                }
                // Format date nicely if it's in YYYYMMDD format
                let formatted_date = if date.len() == 8 {
                    format!("{}-{}-{}", &date[0..4], &date[4..6], &date[6..8])
                } else {
                    date.to_string()
                };
                parts.push(text(formatted_date).size(12).color(colors.date).into());
            }

            // Content size (file size without SAUCE)
            if let Some(content_size) = info.content_size {
                if !parts.is_empty() {
                    parts.push(text(" • ").size(12).color(colors.separator).into());
                }
                parts.push(text(format_size(content_size as u64)).size(12).color(colors.date).into());
            }

            // Buffer size (from real screen, not SAUCE)
            if let Some((width, height)) = info.buffer_size {
                if !parts.is_empty() {
                    parts.push(text(" • ").size(12).color(colors.separator).into());
                }
                parts.push(text(format!("{}×{}", width, height)).size(12).color(colors.size).into());
            }

            // Capabilities info (iCE, font, etc. - size is already shown above)
            let caps_info = format_sauce_info(sauce);
            if !caps_info.is_empty() {
                if !parts.is_empty() {
                    parts.push(text(" • ").size(12).color(colors.separator).into());
                }
                parts.push(text(caps_info).size(12).color(colors.size).into());
            }

            if parts.is_empty() {
                // Fallback to filename if no sauce fields are filled
                if let Some(ref name) = info.file_name {
                    let size_str = info.file_size.map(|s| format!(" — {}", format_size(s))).unwrap_or_default();
                    text(format!("{}{}", name, size_str)).size(12).color(colors.date).into()
                } else {
                    text(fl!(LANGUAGE_LOADER, "statusbar-ready")).size(12).color(colors.separator).into()
                }
            } else {
                // Wrap SAUCE info in a clickable button
                let is_dark = theme.is_dark;
                button(row(parts).spacing(0))
                    .on_press(StatusBarMessage::ShowSauceInfo)
                    .padding([0, 0])
                    .style(move |_theme: &icy_ui::Theme, status| {
                        use icy_ui::widget::button::{Status, Style};
                        let hover_alpha = if is_dark { 0.1 } else { 0.15 };
                        let pressed_alpha = if is_dark { 0.15 } else { 0.2 };
                        let base = Style {
                            background: Some(icy_ui::Background::Color(Color::TRANSPARENT)),
                            text_color: Color::WHITE,
                            border: Border::default(),
                            shadow: Default::default(),
                            snap: false,
                            ..Default::default()
                        };
                        match status {
                            Status::Active | Status::Disabled | Status::Selected => base,
                            Status::Hovered => Style {
                                background: Some(icy_ui::Background::Color(if is_dark {
                                    Color::from_rgba(1.0, 1.0, 1.0, hover_alpha)
                                } else {
                                    Color::from_rgba(0.0, 0.0, 0.0, hover_alpha)
                                })),
                                ..base
                            },
                            Status::Pressed => Style {
                                background: Some(icy_ui::Background::Color(if is_dark {
                                    Color::from_rgba(1.0, 1.0, 1.0, pressed_alpha)
                                } else {
                                    Color::from_rgba(0.0, 0.0, 0.0, pressed_alpha)
                                })),
                                ..base
                            },
                        }
                    })
                    .into()
            }
        } else if let Some((ref archive_type, size)) = info.archive_info {
            // Display archive info: "Archive: TYPE xxx KB"
            let mut parts: Vec<Element<'static, StatusBarMessage>> = Vec::new();
            parts.push(text("Archive: ").size(12).color(colors.separator).into());
            parts.push(text(archive_type.clone()).size(12).color(colors.title).into());
            parts.push(text(" ").size(12).into());
            parts.push(text(format_size(size)).size(12).color(colors.size).into());
            row(parts).spacing(0).into()
        } else if let Some(ref msg) = info.message {
            text(msg.clone()).size(12).color(colors.date).into()
        } else if let Some(ref name) = info.file_name {
            let size_str = info.file_size.map(|s| format!(" — {}", format_size(s))).unwrap_or_default();
            let buffer_str = info.buffer_size.map(|(w, h)| format!(" • {}×{}", w, h)).unwrap_or_default();
            text(format!("{}{}{}", name, size_str, buffer_str)).size(12).color(colors.date).into()
        } else {
            text(fl!(LANGUAGE_LOADER, "statusbar-ready")).size(12).color(colors.separator).into()
        };

        // Baud emulation button
        let baud_text = match info.baud_emulation {
            BaudEmulation::Off => "MAX".to_string(),
            BaudEmulation::Rate(rate) => format!("{} BPS", rate),
        };

        let baud_button: Element<'static, StatusBarMessage> = if info.is_viewing_file {
            button(text(baud_text).size(12))
                .on_press(StatusBarMessage::CycleBaudEmulation)
                .padding([2, 8])
                .style(|theme: &icy_ui::Theme, status| {
                    use icy_ui::widget::button::{Status, Style};

                    let base = Style {
                        background: Some(icy_ui::Background::Color(Color::TRANSPARENT)),
                        text_color: theme.accent.hover,
                        border: Border {
                            color: theme.accent.selected,
                            width: 1.0,
                            radius: 4.0.into(),
                        },
                        shadow: Default::default(),
                        snap: false,
                        ..Default::default()
                    };

                    match status {
                        Status::Active | Status::Disabled | Status::Selected => base,
                        Status::Hovered => Style {
                            background: Some(icy_ui::Background::Color(theme.accent.selected)),
                            text_color: theme.accent.on,
                            ..base
                        },
                        Status::Pressed => Style {
                            background: Some(icy_ui::Background::Color(theme.accent.hover)),
                            text_color: theme.primary.on,
                            ..base
                        },
                    }
                })
                .into()
        } else {
            // Disabled state when not viewing a file
            button(text(baud_text).size(12))
                .padding([2, 8])
                .style(|theme: &icy_ui::Theme, _status| icy_ui::widget::button::Style {
                    background: Some(icy_ui::Background::Color(Color::TRANSPARENT)),
                    text_color: theme.button.on.scale_alpha(0.5),
                    border: Border {
                        color: theme.button.on.scale_alpha(0.3),
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    shadow: Default::default(),
                    snap: false,
                    ..Default::default()
                })
                .into()
        };

        // Auto-scroll button (always visible)
        let is_auto_scroll_enabled = info.auto_scroll_enabled;
        let auto_scroll_button: Element<'static, StatusBarMessage> = {
            let scroll_text = "SCROLL";
            button(text(scroll_text).size(12))
                .on_press(StatusBarMessage::ToggleAutoScroll)
                .padding([2, 8])
                .style(move |theme: &icy_ui::Theme, status| {
                    use icy_ui::widget::button::{Status, Style};

                    let base = Style {
                        background: if is_auto_scroll_enabled {
                            Some(icy_ui::Background::Color(theme.accent.selected))
                        } else {
                            Some(icy_ui::Background::Color(Color::TRANSPARENT))
                        },
                        text_color: if is_auto_scroll_enabled { theme.accent.on } else { theme.accent.hover },
                        border: Border {
                            color: theme.accent.selected,
                            width: 1.0,
                            radius: 4.0.into(),
                        },
                        shadow: Default::default(),
                        snap: false,
                        ..Default::default()
                    };

                    match status {
                        Status::Active | Status::Disabled | Status::Selected => base,
                        Status::Hovered => Style {
                            background: Some(icy_ui::Background::Color(theme.accent.selected)),
                            text_color: theme.accent.on,
                            ..base
                        },
                        Status::Pressed => Style {
                            background: Some(icy_ui::Background::Color(theme.accent.hover)),
                            text_color: theme.primary.on,
                            ..base
                        },
                    }
                })
                .into()
        };

        // Right side: item count
        let right_content: Element<'static, StatusBarMessage> = if info.item_count > 0 {
            let count_text = fl!(LANGUAGE_LOADER, "statusbar-items", count = info.item_count);
            text(count_text)
                .size(12)
                .style(|theme: &icy_ui::Theme| text::Style {
                    color: Some(theme.background.on.scale_alpha(0.6)),
                })
                .into()
        } else {
            text("").size(12).into()
        };

        let content = row![
            container(left_content).padding([0, 10]),
            Space::new().width(Length::Fill),
            auto_scroll_button,
            baud_button,
            container(right_content).padding([0, 10]),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        container(content)
            .width(Length::Fill)
            .padding([4, 0])
            .style(|theme| container::Style {
                background: Some(icy_ui::Background::Color(theme.secondary.base)),
                border: icy_ui::Border {
                    color: theme.primary.divider,
                    width: 1.0,
                    radius: 0.0.into(),
                },
                ..Default::default()
            })
            .into()
    }
}
