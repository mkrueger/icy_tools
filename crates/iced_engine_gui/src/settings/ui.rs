use super::*;
use crate::LANGUAGE_LOADER;
use crate::{MonitorSettings, MonitorType};
use i18n_embed_fl::fl;
use iced::widget::{Space, checkbox, column, container, pick_list, row, text};
use iced::{Alignment, Background, Border, Element, Length, Theme};

pub const CHECKBOX_SIZE: f32 = 18.0;
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

    let mut content: iced::widget::Column<'_, MonitorSettingsMessage> = column![section_header(fl!(LANGUAGE_LOADER, "settings-appearance-section")),];

    // Appearance section with rounded border
    let mut appearance_content = column![
        row![
            left_label(fl!(LANGUAGE_LOADER, "settings-theme")),
            pick_list(theme_options, Some(ThemeOption(s.get_theme())), |opt| {
                MonitorSettingsMessage::ThemeChanged(opt.into())
            })
            .width(Length::Fixed(INPUT_WIDTH))
            .text_size(TEXT_SIZE_NORMAL)
        ]
        .spacing(ROW_SPACING)
        .align_y(Alignment::Center),
        row![
            left_label(fl!(LANGUAGE_LOADER, "settings-monitor-type")),
            pick_list(monitor_type_options, Some(s.monitor_type), MonitorSettingsMessage::MonitorTypeChanged)
                .width(Length::Fixed(INPUT_WIDTH))
                .text_size(TEXT_SIZE_NORMAL)
        ]
        .spacing(ROW_SPACING)
        .align_y(Alignment::Center),
        row![
            left_label(fl!(LANGUAGE_LOADER, "settings-integer-scaling-label")),
            checkbox("", s.use_pixel_perfect_scaling)
                .on_toggle(MonitorSettingsMessage::PixelPerfectScalingChanged)
                .size(CHECKBOX_SIZE)
                .text_size(TEXT_SIZE_NORMAL)
        ]
        .spacing(ROW_SPACING)
        .align_y(Alignment::Center),
    ]
    .spacing(ROW_SPACING);

    // Custom monochrome color
    if s.monitor_type == MonitorType::CustomMonochrome {
        let c = icy_to_iced_color(s.custom_monitor_color.clone());
        appearance_content = appearance_content.push(
            row![
                left_label(fl!(LANGUAGE_LOADER, "settings-monitor-custom")),
                color_button(c, MonitorSettingsMessage::CustomColorChanged(c)),
                text(format!("RGB({}, {}, {})", (c.r * 255.0) as u8, (c.g * 255.0) as u8, (c.b * 255.0) as u8))
                    .size(TEXT_SIZE_SMALL)
                    .style(|theme: &Theme| text::Style {
                        color: Some(theme.extended_palette().background.strong.text)
                    })
            ]
            .spacing(ROW_SPACING)
            .align_y(Alignment::Center),
        );
    }

    // Border color
    let bc = icy_to_iced_color(s.border_color.clone());
    appearance_content = appearance_content.push(
        row![
            left_label(fl!(LANGUAGE_LOADER, "settings-background_color-label")),
            color_button(bc, MonitorSettingsMessage::BorderColorChanged(bc)),
            text(format!("RGB({}, {}, {})", (bc.r * 255.0) as u8, (bc.g * 255.0) as u8, (bc.b * 255.0) as u8))
                .size(TEXT_SIZE_SMALL)
                .style(|theme: &Theme| text::Style {
                    color: Some(theme.extended_palette().background.strong.text)
                }),
        ]
        .spacing(ROW_SPACING)
        .align_y(Alignment::Center),
    );

    // Add appearance content in a box
    content = content.push(effect_box(appearance_content.into()));

    // Tone (always applied)
    content = content.push(Space::new().height(SECTION_SPACING));
    content = content.push(section_header(fl!(LANGUAGE_LOADER, "settings-color-tone-section")));
    content = content.push(effect_box(
        column![
            slider_row_owned(
                fl!(LANGUAGE_LOADER, "settings-monitor-brightness"),
                s.brightness,
                0.0..=200.0,
                MonitorSettingsMessage::BrightnessChanged
            ),
            slider_row_owned(
                fl!(LANGUAGE_LOADER, "settings-monitor-contrast"),
                s.contrast,
                0.0..=200.0,
                MonitorSettingsMessage::ContrastChanged
            ),
            slider_row_owned(
                fl!(LANGUAGE_LOADER, "settings-monitor-gamma"),
                s.gamma,
                0.0..=4.0,
                MonitorSettingsMessage::GammaChanged
            ),
            slider_row_owned(
                fl!(LANGUAGE_LOADER, "settings-monitor-saturation"),
                s.saturation,
                0.0..=200.0,
                MonitorSettingsMessage::SaturationChanged
            ),
        ]
        .spacing(SLIDER_SPACING)
        .into(),
    ));

    // Bloom / glow group
    content = content.push(Space::new().height(SECTION_SPACING));
    content = content.push(section_header(fl!(LANGUAGE_LOADER, "settings-bloom-glow-section")));
    content = content.push(effect_box_toggleable(
        column![
            toggle_row(
                fl!(LANGUAGE_LOADER, "settings-enabled-checkbox").as_str(),
                s.use_bloom,
                MonitorSettingsMessage::BloomToggleChanged(true)
            ),
            disabled_slider(
                !s.use_bloom,
                fl!(LANGUAGE_LOADER, "settings-bloom-threshold-label"),
                s.bloom_threshold,
                0.0..=100.0,
                MonitorSettingsMessage::BloomThresholdChanged
            ),
            disabled_slider(
                !s.use_bloom,
                fl!(LANGUAGE_LOADER, "settings-bloom-radius-label"),
                s.bloom_radius,
                0.0..=50.0,
                MonitorSettingsMessage::BloomRadiusChanged
            ),
            disabled_slider(
                !s.use_bloom,
                fl!(LANGUAGE_LOADER, "settings-glow-strength-label"),
                s.glow_strength,
                0.0..=100.0,
                MonitorSettingsMessage::GlowStrengthChanged
            ),
            disabled_slider(
                !s.use_bloom,
                fl!(LANGUAGE_LOADER, "settings-phosphor-persistence-label"),
                s.phosphor_persistence,
                0.0..=100.0,
                MonitorSettingsMessage::PhosphorPersistenceChanged
            ),
        ]
        .spacing(SLIDER_SPACING)
        .into(),
        !s.use_bloom,
    ));

    // Scanlines
    content = content.push(Space::new().height(SECTION_SPACING));
    content = content.push(section_header(fl!(LANGUAGE_LOADER, "settings-scanlines-section")));
    content = content.push(effect_box_toggleable(
        column![
            toggle_row(
                fl!(LANGUAGE_LOADER, "settings-enabled-checkbox").as_str(),
                s.use_scanlines,
                MonitorSettingsMessage::ScanlinesToggleChanged(true)
            ),
            disabled_slider(
                !s.use_scanlines,
                fl!(LANGUAGE_LOADER, "settings-scanline-thickness-label"),
                s.scanline_thickness * 100.0,
                0.0..=100.0,
                |v| { MonitorSettingsMessage::ScanlineThicknessChanged(v / 100.0) }
            ),
            disabled_slider(
                !s.use_scanlines,
                fl!(LANGUAGE_LOADER, "settings-scanline-sharpness-label"),
                s.scanline_sharpness * 100.0,
                0.0..=100.0,
                |v| { MonitorSettingsMessage::ScanlineSharpnessChanged(v / 100.0) }
            ),
            disabled_slider(
                !s.use_scanlines,
                fl!(LANGUAGE_LOADER, "settings-scanline-phase-label"),
                s.scanline_phase * 100.0,
                0.0..=100.0,
                |v| { MonitorSettingsMessage::ScanlinePhaseChanged(v / 100.0) }
            ),
        ]
        .spacing(SLIDER_SPACING)
        .into(),
        !s.use_scanlines,
    ));

    // Geometry
    content = content.push(Space::new().height(SECTION_SPACING));
    content = content.push(section_header(fl!(LANGUAGE_LOADER, "settings-geometry-section")));
    content = content.push(effect_box_toggleable(
        column![
            toggle_row(
                fl!(LANGUAGE_LOADER, "settings-enabled-checkbox").as_str(),
                s.use_curvature,
                MonitorSettingsMessage::CurvatureToggleChanged(true)
            ),
            disabled_slider(
                !s.use_curvature,
                fl!(LANGUAGE_LOADER, "settings-curvature-x-label"),
                s.curvature_x,
                0.0..=100.0,
                MonitorSettingsMessage::CurvatureXChanged
            ),
            disabled_slider(
                !s.use_curvature,
                fl!(LANGUAGE_LOADER, "settings-curvature-y-label"),
                s.curvature_y,
                0.0..=100.0,
                MonitorSettingsMessage::CurvatureYChanged
            ),
        ]
        .spacing(SLIDER_SPACING)
        .into(),
        !s.use_curvature,
    ));

    // Noise / Artifacts
    content = content.push(Space::new().height(SECTION_SPACING));
    content = content.push(section_header(fl!(LANGUAGE_LOADER, "settings-noise-artifacts-section")));
    content = content.push(effect_box_toggleable(
        column![
            toggle_row(
                fl!(LANGUAGE_LOADER, "settings-enabled-checkbox").as_str(),
                s.use_noise,
                MonitorSettingsMessage::NoiseToggleChanged(true)
            ),
            disabled_slider(
                !s.use_noise,
                fl!(LANGUAGE_LOADER, "settings-noise-level-label"),
                s.noise_level,
                0.0..=100.0,
                MonitorSettingsMessage::NoiseLevelChanged
            ),
            disabled_slider(
                !s.use_noise,
                fl!(LANGUAGE_LOADER, "settings-sync-wobble-label"),
                s.sync_wobble,
                0.0..=100.0,
                MonitorSettingsMessage::SyncWobbleChanged
            ),
        ]
        .spacing(SLIDER_SPACING)
        .into(),
        !s.use_noise,
    ));

    container(content).padding(SECTION_PADDING).width(Length::Fill).into()
}

// Update toggle_row to accept &str instead of &'static str
fn toggle_row(label: &str, value: bool, msg: MonitorSettingsMessage) -> Element<'static, MonitorSettingsMessage> {
    let label_owned = label.to_string();
    row![
        left_label(label_owned),
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
            .size(CHECKBOX_SIZE),
    ]
    .spacing(ROW_SPACING)
    .align_y(Alignment::Center)
    .into()
}

fn disabled_slider<'a>(
    disabled: bool,
    label: String,
    value: f32,
    range: std::ops::RangeInclusive<f32>,
    on_change: impl Fn(f32) -> MonitorSettingsMessage + 'a,
) -> Element<'a, MonitorSettingsMessage> {
    if disabled {
        // Disabled appearance
        row![
            text(label).size(14).width(Length::Fixed(LABEL_WIDTH)).style(|theme: &Theme| text::Style {
                color: Some(Color::from_rgba(theme.palette().text.r, theme.palette().text.g, theme.palette().text.b, 0.5))
            }),
            slider(range, value, |_| MonitorSettingsMessage::Noop)
                .width(Length::Fill)
                .style(|theme: &Theme, _status| {
                    let palette = theme.extended_palette();
                    iced::widget::slider::Style {
                        rail: iced::widget::slider::Rail {
                            backgrounds: (
                                Background::Color(Color::from_rgba(
                                    palette.primary.base.color.r,
                                    palette.primary.base.color.g,
                                    palette.primary.base.color.b,
                                    0.3,
                                )),
                                Background::Color(Color::from_rgba(
                                    palette.background.weak.color.r,
                                    palette.background.weak.color.g,
                                    palette.background.weak.color.b,
                                    0.3,
                                )),
                            ),
                            width: 4.0,
                            border: Border::default(),
                        },
                        handle: iced::widget::slider::Handle {
                            shape: iced::widget::slider::HandleShape::Circle { radius: 8.0 },
                            background: Background::Color(Color::from_rgba(
                                palette.primary.base.color.r,
                                palette.primary.base.color.g,
                                palette.primary.base.color.b,
                                0.3,
                            )),
                            border_color: Color::from_rgba(1.0, 1.0, 1.0, 0.3),
                            border_width: 2.0,
                        },
                    }
                }),
            container(text(format!("{:.0}", value)).size(13).style(|theme: &Theme| text::Style {
                color: Some(Color::from_rgba(
                    theme.extended_palette().background.strong.text.r,
                    theme.extended_palette().background.strong.text.g,
                    theme.extended_palette().background.strong.text.b,
                    0.5
                ))
            }))
            .width(Length::Fixed(SLIDER_VALUE_WIDTH))
            .style(|theme: &Theme| container::Style {
                background: Some(Background::Color(Color::from_rgba(
                    theme.extended_palette().background.weak.color.r,
                    theme.extended_palette().background.weak.color.g,
                    theme.extended_palette().background.weak.color.b,
                    0.3
                ))),
                border: Border {
                    color: Color::from_rgba(
                        theme.extended_palette().background.strong.color.r,
                        theme.extended_palette().background.strong.color.g,
                        theme.extended_palette().background.strong.color.b,
                        0.3
                    ),
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            })
            .padding(4)
            .center_x(Length::Fixed(SLIDER_VALUE_WIDTH))
        ]
        .spacing(12)
        .align_y(Alignment::Center)
        .padding([4, 0])
        .into()
    } else {
        // Enabled appearance - use the normal slider_row_owned
        slider_row_owned(label, value, range, on_change)
    }
}
