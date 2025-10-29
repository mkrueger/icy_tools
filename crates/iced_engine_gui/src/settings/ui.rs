use i18n_embed_fl::fl;
use iced::widget::{Space, checkbox, column, container, pick_list, row, text};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme};

// Import LANGUAGE_LOADER from the ui module
use crate::LANGUAGE_LOADER;
use crate::{MonitorSettings, MonitorType};

use super::*;

pub fn show_monitor_settings<'a>(monitor_settings: &'a MonitorSettings) -> Element<'a, MonitorSettingsMessage> {
    // Create monitor type options
    let monitor_type_options: Vec<MonitorType> = vec![
        MonitorType::Color,
        MonitorType::Grayscale,
        MonitorType::Amber,
        MonitorType::Green,
        MonitorType::Apple2,
        MonitorType::Futuristic,
        MonitorType::CustomMonochrome,
    ];

    let theme_options: Vec<ThemeOption> = vec![
        ThemeOption(Theme::Light),
        ThemeOption(Theme::Dark),
        ThemeOption(Theme::Dracula),
        ThemeOption(Theme::Nord),
        ThemeOption(Theme::SolarizedLight),
        ThemeOption(Theme::SolarizedDark),
        ThemeOption(Theme::GruvboxLight),
        ThemeOption(Theme::GruvboxDark),
        ThemeOption(Theme::CatppuccinLatte),
        ThemeOption(Theme::CatppuccinFrappe),
        ThemeOption(Theme::CatppuccinMacchiato),
        ThemeOption(Theme::CatppuccinMocha),
        ThemeOption(Theme::TokyoNight),
        ThemeOption(Theme::TokyoNightStorm),
        ThemeOption(Theme::TokyoNightLight),
        ThemeOption(Theme::KanagawaWave),
        ThemeOption(Theme::KanagawaDragon),
        ThemeOption(Theme::KanagawaLotus),
        ThemeOption(Theme::Moonfly),
        ThemeOption(Theme::Nightfly),
        ThemeOption(Theme::Oxocarbon),
        ThemeOption(Theme::Ferra),
    ];

    let mut content = column![
        // Appearance Section
        section_header("Appearance"),
        Space::new().height(8),
        // Theme selection
        row![
            container(text("Theme").size(14))
                .width(Length::Fixed(LABEL_WIDTH))
                .align_x(iced::alignment::Horizontal::Right),
            pick_list(theme_options, Some(ThemeOption(monitor_settings.get_theme())), |opt| {
                MonitorSettingsMessage::ThemeChanged(opt.into())
            })
            .width(Length::Fixed(INPUT_WIDTH))
            .text_size(14)
        ]
        .spacing(12)
        .align_y(Alignment::Center),
        Space::new().height(24),
        // Monitor Settings Section
        section_header("Monitor Settings"),
        Space::new().height(8),
        // Monitor type selection
        row![
            container(text(fl!(LANGUAGE_LOADER, "settings-monitor-type")).size(14))
                .width(Length::Fixed(LABEL_WIDTH))
                .align_x(iced::alignment::Horizontal::Right),
            pick_list(
                monitor_type_options,
                Some(monitor_settings.monitor_type),
                MonitorSettingsMessage::MonitorTypeChanged
            )
            .width(Length::Fixed(INPUT_WIDTH))
            .text_size(14)
        ]
        .spacing(12)
        .align_y(Alignment::Center),
        // NEW: Pixel Perfect Scaling toggle
        row![
            container(text("Pixel Perfect Scaling").size(14))
                .width(Length::Fixed(LABEL_WIDTH))
                .align_x(iced::alignment::Horizontal::Right),
            checkbox("Use integer nearest-neighbor scaling", monitor_settings.use_pixel_perfect_scaling)
                .on_toggle(MonitorSettingsMessage::PixelPerfectScalingChanged)
                .size(16)
                .text_size(14)
                .width(Length::Fixed(INPUT_WIDTH))
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    ]
    .spacing(ROW_SPACING);

    // Custom color picker (if custom monochrome is selected)
    if monitor_settings.monitor_type == MonitorType::CustomMonochrome {
        let custom_color = icy_to_iced_color(monitor_settings.custom_monitor_color.clone());
        content = content.push(
            row![
                container(text(fl!(LANGUAGE_LOADER, "settings-monitor-custom")).size(14))
                    .width(Length::Fixed(LABEL_WIDTH))
                    .align_x(iced::alignment::Horizontal::Right),
                color_button(custom_color, MonitorSettingsMessage::CustomColorChanged(custom_color)),
                text(format!(
                    "RGB({}, {}, {})",
                    (custom_color.r * 255.0) as u8,
                    (custom_color.g * 255.0) as u8,
                    (custom_color.b * 255.0) as u8
                ))
                .size(12)
                .style(|theme: &Theme| {
                    text::Style {
                        color: Some(theme.extended_palette().background.strong.text),
                    }
                }),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
        );
    }

    // Border color picker
    let border_color = icy_to_iced_color(monitor_settings.border_color.clone());
    content = content.push(
        row![
            container(text(fl!(LANGUAGE_LOADER, "settings-background_color-label")).size(14))
                .width(Length::Fixed(LABEL_WIDTH))
                .align_x(iced::alignment::Horizontal::Right),
            color_button(border_color, MonitorSettingsMessage::BorderColorChanged(border_color)),
            text(format!(
                "RGB({}, {}, {})",
                (border_color.r * 255.0) as u8,
                (border_color.g * 255.0) as u8,
                (border_color.b * 255.0) as u8
            ))
            .size(12)
            .style(|theme: &Theme| {
                text::Style {
                    color: Some(theme.extended_palette().background.strong.text),
                }
            }),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    );

    // CRT Filter Section
    content = content.push(Space::new().height(24));
    content = content.push(section_header("CRT Filter Effects"));
    content = content.push(Space::new().height(8));

    // CRT filter checkbox with styled appearance
    let use_filter = monitor_settings.use_filter;
    content = content.push(
        container(
            checkbox(fl!(LANGUAGE_LOADER, "settings-monitor-use-crt-filter-checkbox"), monitor_settings.use_filter)
                .on_toggle(MonitorSettingsMessage::UseFilterChanged)
                .size(16)
                .text_size(14),
        )
        .style(move |theme: &Theme| container::Style {
            background: Some(Background::Color(if use_filter {
                theme.extended_palette().success.weak.color
            } else {
                theme.extended_palette().background.weak.color
            })),
            border: Border {
                color: if use_filter {
                    theme.extended_palette().success.base.color
                } else {
                    Color::TRANSPARENT
                },
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        })
        .padding(8)
        .width(Length::Fill),
    );

    // CRT filter sliders (only if enabled)
    if monitor_settings.use_filter {
        content = content.push(Space::new().height(16));

        // Create owned strings for the labels to avoid lifetime issues
        let brightness_label = fl!(LANGUAGE_LOADER, "settings-monitor-brightness");
        let contrast_label = fl!(LANGUAGE_LOADER, "settings-monitor-contrast");
        let saturation_label = fl!(LANGUAGE_LOADER, "settings-monitor-saturation");
        let gamma_label = fl!(LANGUAGE_LOADER, "settings-monitor-gamma");
        let blur_label = fl!(LANGUAGE_LOADER, "settings-monitor-blur");
        let curve_label = fl!(LANGUAGE_LOADER, "settings-monitor-curve");
        let scanlines_label = fl!(LANGUAGE_LOADER, "settings-monitor-scanlines");

        let sliders_container = container(
            column![
                slider_row_owned(
                    brightness_label,
                    monitor_settings.brightness,
                    0.0..=100.0,
                    MonitorSettingsMessage::BrightnessChanged
                ),
                slider_row_owned(contrast_label, monitor_settings.contrast, 0.0..=100.0, MonitorSettingsMessage::ContrastChanged),
                slider_row_owned(
                    saturation_label,
                    monitor_settings.saturation,
                    0.0..=100.0,
                    MonitorSettingsMessage::SaturationChanged
                ),
                slider_row_owned(gamma_label, monitor_settings.gamma, 0.0..=100.0, MonitorSettingsMessage::GammaChanged),
                slider_row_owned(blur_label, monitor_settings.blur, 0.0..=100.0, MonitorSettingsMessage::BlurChanged),
                slider_row_owned(curve_label, monitor_settings.curvature, 0.0..=100.0, MonitorSettingsMessage::CurvatureChanged),
                slider_row_owned(
                    scanlines_label,
                    monitor_settings.scanlines,
                    0.0..=100.0,
                    MonitorSettingsMessage::ScanlinesChanged
                ),
            ]
            .spacing(8),
        )
        .style(|theme: &Theme| container::Style {
            background: Some(Background::Color(theme.extended_palette().background.weak.color)),
            border: Border {
                color: theme.extended_palette().background.strong.color,
                width: 1.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        })
        .padding(16);

        content = content.push(sliders_container);
    }

    container(content).padding(SECTION_PADDING).width(Length::Fill).into()
}
