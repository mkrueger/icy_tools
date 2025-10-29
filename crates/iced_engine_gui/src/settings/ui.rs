use super::*;
use crate::LANGUAGE_LOADER;
use crate::{MonitorSettings, MonitorType};
use i18n_embed_fl::fl;
use iced::widget::{Space, checkbox, column, container, pick_list, row, text};
use iced::{Alignment, Background, Border, Element, Length, Theme};

pub fn show_monitor_settings<'a>(s: &'a MonitorSettings) -> Element<'a, MonitorSettingsMessage> {
    let monitor_type_options = vec![
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
        section_header("Appearance"),
        Space::new().height(8),
        row![
            right_label("Theme"),
            pick_list(theme_options, Some(ThemeOption(s.get_theme())), |opt| {
                MonitorSettingsMessage::ThemeChanged(opt.into())
            })
            .width(Length::Fixed(INPUT_WIDTH))
            .text_size(14)
        ]
        .spacing(12)
        .align_y(Alignment::Center),
        row![
            right_label_owned(fl!(LANGUAGE_LOADER, "settings-monitor-type")),
            pick_list(monitor_type_options, Some(s.monitor_type), MonitorSettingsMessage::MonitorTypeChanged)
                .width(Length::Fixed(INPUT_WIDTH))
                .text_size(14)
        ]
        .spacing(12)
        .align_y(Alignment::Center),
        row![
            right_label("Pixel Perfect Scaling"),
            checkbox("Use integer scaling", s.use_pixel_perfect_scaling)
                .on_toggle(MonitorSettingsMessage::PixelPerfectScalingChanged)
                .size(16)
                .text_size(14)
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    ]
    .spacing(ROW_SPACING);

    // Custom monochrome color
    if s.monitor_type == MonitorType::CustomMonochrome {
        let c = icy_to_iced_color(s.custom_monitor_color.clone());
        content = content.push(
            row![
                right_label_owned(fl!(LANGUAGE_LOADER, "settings-monitor-custom")),
                color_button(c, MonitorSettingsMessage::CustomColorChanged(c)),
                text(format!("RGB({}, {}, {})", (c.r * 255.0) as u8, (c.g * 255.0) as u8, (c.b * 255.0) as u8))
                    .size(12)
                    .style(|theme: &Theme| text::Style {
                        color: Some(theme.extended_palette().background.strong.text)
                    })
            ]
            .spacing(12)
            .align_y(Alignment::Center),
        );
    }

    // Border color
    let bc = icy_to_iced_color(s.border_color.clone());
    content = content.push(
        row![
            right_label_owned(fl!(LANGUAGE_LOADER, "settings-background_color-label")),
            color_button(bc, MonitorSettingsMessage::BorderColorChanged(bc)),
            text(format!("RGB({}, {}, {})", (bc.r * 255.0) as u8, (bc.g * 255.0) as u8, (bc.b * 255.0) as u8))
                .size(12)
                .style(|theme: &Theme| text::Style {
                    color: Some(theme.extended_palette().background.strong.text)
                }),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    );

    // Tone (always applied)
    content = content.push(Space::new().height(24));
    content = content.push(section_header("Color & Tone"));
    content = content.push(effect_box(
        column![
            slider_row_owned("Brightness".to_string(), s.brightness, 0.0..=200.0, MonitorSettingsMessage::BrightnessChanged),
            slider_row_owned("Contrast".to_string(), s.contrast, 0.0..=200.0, MonitorSettingsMessage::ContrastChanged),
            slider_row_owned("Gamma".to_string(), s.gamma, 0.0..=4.0, MonitorSettingsMessage::GammaChanged),
            slider_row_owned("Saturation".to_string(), s.saturation, 0.0..=200.0, MonitorSettingsMessage::SaturationChanged),
        ]
        .spacing(8)
        .into(),
    ));

    // Bloom / glow group
    content = content.push(Space::new().height(24));
    content = content.push(section_header("Bloom & Glow"));
    content = content.push(effect_box(
        column![
            toggle_row("Bloom", s.use_bloom, MonitorSettingsMessage::BloomToggleChanged(true)),
            if s.use_bloom {
                column![
                    slider_row_owned(
                        "Threshold".to_string(),
                        s.bloom_threshold,
                        0.0..=100.0,
                        MonitorSettingsMessage::BloomThresholdChanged
                    ),
                    slider_row_owned("Radius".to_string(), s.bloom_radius, 0.0..=50.0, MonitorSettingsMessage::BloomRadiusChanged),
                ]
                .spacing(6)
                .into()
            } else {
                Into::<Element<'_, MonitorSettingsMessage>>::into(Space::new())
            },
            slider_row_owned(
                "Glow Strength".to_string(),
                s.glow_strength,
                0.0..=100.0,
                MonitorSettingsMessage::GlowStrengthChanged
            ),
            slider_row_owned(
                "Phosphor Persistence".to_string(),
                s.phosphor_persistence,
                0.0..=100.0,
                MonitorSettingsMessage::PhosphorPersistenceChanged
            ),
        ]
        .spacing(10)
        .into(),
    ));

    // Scanlines
    content = content.push(Space::new().height(24));
    content = content.push(section_header("Scanlines"));
    content = content.push(effect_box(
        column![
            toggle_row("Scanlines", s.use_scanlines, MonitorSettingsMessage::ScanlinesToggleChanged(true)),
            if s.use_scanlines {
                column![
                    slider_row_owned("Thickness".to_string(), s.scanline_thickness * 100.0, 0.0..=100.0, |v| {
                        MonitorSettingsMessage::ScanlineThicknessChanged(v / 100.0)
                    }),
                    slider_row_owned("Sharpness".to_string(), s.scanline_sharpness * 100.0, 0.0..=100.0, |v| {
                        MonitorSettingsMessage::ScanlineSharpnessChanged(v / 100.0)
                    }),
                    slider_row_owned("Phase".to_string(), s.scanline_phase * 100.0, 0.0..=100.0, |v| {
                        MonitorSettingsMessage::ScanlinePhaseChanged(v / 100.0)
                    }),
                ]
                .spacing(6)
                .into()
            } else {
                Into::<Element<'_, MonitorSettingsMessage>>::into(Space::new())
            },
        ]
        .spacing(10)
        .into(),
    ));

    // Geometry
    content = content.push(Space::new().height(24));
    content = content.push(section_header("Geometry"));
    content = content.push(effect_box(
        column![
            toggle_row("Curvature / Distortion", s.use_curvature, MonitorSettingsMessage::CurvatureToggleChanged(true)),
            if s.use_curvature {
                column![
                    slider_row_owned("Curvature X".to_string(), s.curvature_x, 0.0..=100.0, MonitorSettingsMessage::CurvatureXChanged),
                    slider_row_owned("Curvature Y".to_string(), s.curvature_y, 0.0..=100.0, MonitorSettingsMessage::CurvatureYChanged),
                ]
                .spacing(6)
                .into()
            } else {
                Into::<Element<'_, MonitorSettingsMessage>>::into(Space::new())
            },
        ]
        .spacing(10)
        .into(),
    ));

    // Noise / Artifacts
    content = content.push(Space::new().height(24));
    content = content.push(section_header("Noise & Artifacts"));
    content = content.push(effect_box(
        column![
            toggle_row("Noise", s.use_noise, MonitorSettingsMessage::NoiseToggleChanged(true)),
            if s.use_noise {
                slider_row_owned("Noise Level".to_string(), s.noise_level, 0.0..=100.0, MonitorSettingsMessage::NoiseLevelChanged)
            } else {
                Into::<Element<'_, MonitorSettingsMessage>>::into(Space::new())
            },
            slider_row_owned("Sync Wobble".to_string(), s.sync_wobble, 0.0..=100.0, MonitorSettingsMessage::SyncWobbleChanged),
        ]
        .spacing(10)
        .into(),
    ));

    container(content).padding(SECTION_PADDING).width(Length::Fill).into()
}

// Helpers

fn right_label(txt: &str) -> Element<'_, MonitorSettingsMessage> {
    container(text(txt).size(14))
        .width(Length::Fixed(LABEL_WIDTH))
        .align_x(iced::alignment::Horizontal::Right)
        .into()
}

fn right_label_owned(txt: String) -> Element<'static, MonitorSettingsMessage> {
    container(text(txt).size(14))
        .width(Length::Fixed(LABEL_WIDTH))
        .align_x(iced::alignment::Horizontal::Right)
        .into()
}

fn toggle_row(label: &'static str, value: bool, msg: MonitorSettingsMessage) -> Element<'static, MonitorSettingsMessage> {
    row![
        right_label(label),
        checkbox("", value)
            .on_toggle(move |new_val| if new_val {
                msg.clone()
            } else {
                match msg {
                    MonitorSettingsMessage::BloomToggleChanged(_) => MonitorSettingsMessage::BloomToggleChanged(false),
                    MonitorSettingsMessage::ScanlinesToggleChanged(_) => MonitorSettingsMessage::ScanlinesToggleChanged(false),
                    MonitorSettingsMessage::CurvatureToggleChanged(_) => MonitorSettingsMessage::CurvatureToggleChanged(false),
                    MonitorSettingsMessage::NoiseToggleChanged(_) => MonitorSettingsMessage::NoiseToggleChanged(false),
                    _ => msg.clone(),
                }
            })
            .size(18),
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .into()
}

fn effect_box<'a>(inner: Element<'a, MonitorSettingsMessage>) -> Element<'a, MonitorSettingsMessage> {
    container(inner)
        .style(|theme: &Theme| container::Style {
            background: Some(Background::Color(theme.extended_palette().background.weak.color)),
            border: Border {
                color: theme.extended_palette().background.strong.color,
                width: 1.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        })
        .padding(16)
        .width(Length::Fill)
        .into()
}
