use super::*;
use crate::{section_header, LANGUAGE_LOADER, SECTION_PADDING, SECTION_SPACING, SLIDER_SPACING};
use crate::{MonitorSettings, MonitorType};
use i18n_embed_fl::fl;
use iced::widget::{checkbox, column, container, pick_list, row, text, Space};
use iced::{Alignment, Background, Border, Element, Length, Theme};

pub const CHECKBOX_SIZE: f32 = 18.0;
pub fn show_monitor_settings(s: MonitorSettings) -> Element<'static, MonitorSettingsMessage> {
    show_monitor_settings_with_options(s, true)
}

/// Show monitor settings panel with optional scaling controls.
/// Set `show_scaling_options` to `false` to hide the "Auto" scaling mode and "Integer Scaling" checkboxes.
pub fn show_monitor_settings_with_options(s: MonitorSettings, show_scaling_options: bool) -> Element<'static, MonitorSettingsMessage> {
    let monitor_type_options = vec![
        MonitorType::Color,
        MonitorType::Grayscale,
        MonitorType::Amber,
        MonitorType::Green,
        MonitorType::Apple2,
        MonitorType::Futuristic,
        MonitorType::CustomMonochrome,
    ];

    // New Theme API: Theme::all() returns Vec<Theme> with light/dark and custom themes
    let theme_options: Vec<ThemeOption> = Theme::all().into_iter().map(ThemeOption).collect();

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
    ]
    .spacing(ROW_SPACING);

    // Scaling options (hidden in some applications like icy_draw)
    if show_scaling_options {
        appearance_content = appearance_content.push(
            row![
                left_label(fl!(LANGUAGE_LOADER, "settings-scaling-mode-label")),
                checkbox(s.scaling_mode.is_auto())
                    .on_toggle(|is_auto| {
                        if is_auto {
                            MonitorSettingsMessage::ScalingModeChanged(crate::ScalingMode::Auto)
                        } else {
                            MonitorSettingsMessage::ScalingModeChanged(crate::ScalingMode::Manual(1.0))
                        }
                    })
                    .size(CHECKBOX_SIZE)
                    .text_size(TEXT_SIZE_NORMAL)
            ]
            .spacing(ROW_SPACING)
            .align_y(Alignment::Center),
        );
        appearance_content = appearance_content.push(
            row![
                left_label(fl!(LANGUAGE_LOADER, "settings-integer-scaling-label")),
                checkbox(s.use_integer_scaling)
                    .on_toggle(MonitorSettingsMessage::IntegerScalingChanged)
                    .size(CHECKBOX_SIZE)
                    .text_size(TEXT_SIZE_NORMAL)
            ]
            .spacing(ROW_SPACING)
            .align_y(Alignment::Center),
        );
    }

    appearance_content = appearance_content.push(
        row![
            left_label(fl!(LANGUAGE_LOADER, "settings-bilinear-filtering-label")),
            checkbox(s.use_bilinear_filtering)
                .on_toggle(MonitorSettingsMessage::BilinearFilteringChanged)
                .size(CHECKBOX_SIZE)
                .text_size(TEXT_SIZE_NORMAL)
        ]
        .spacing(ROW_SPACING)
        .align_y(Alignment::Center),
    );

    // Custom monochrome color
    if s.monitor_type == MonitorType::CustomMonochrome {
        let c = icy_to_iced_color(s.custom_monitor_color.clone());
        appearance_content = appearance_content.push(
            row![
                left_label(fl!(LANGUAGE_LOADER, "settings-monitor-custom")),
                color_button(c, MonitorSettingsMessage::CustomColorChanged(c)),
                text(format!("RGB({}, {}, {})", (c.r * 255.0) as u8, (c.g * 255.0) as u8, (c.b * 255.0) as u8))
                    .size(TEXT_SIZE_NORMAL)
                    .style(|theme: &Theme| text::Style {
                        color: Some(theme.secondary.on)
                    })
            ]
            .spacing(ROW_SPACING)
            .align_y(Alignment::Center),
        );
    }

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
        checkbox(value)
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
                color: Some(theme.background.on.scale_alpha(0.5))
            }),
            slider(range, value, |_| MonitorSettingsMessage::Noop)
                .width(Length::Fill)
                .style(|theme: &Theme, _status| {
                    iced::widget::slider::Style {
                        rail: iced::widget::slider::Rail {
                            backgrounds: (
                                Background::Color(theme.accent.base.scale_alpha(0.3)),
                                Background::Color(theme.primary.base.scale_alpha(0.3)),
                            ),
                            width: 4.0,
                            border: Border::default(),
                        },
                        handle: iced::widget::slider::Handle {
                            shape: iced::widget::slider::HandleShape::Circle { radius: 8.0 },
                            background: Background::Color(theme.accent.base.scale_alpha(0.3)),
                            border_color: Color::from_rgba(1.0, 1.0, 1.0, 0.3),
                            border_width: 2.0,
                        },
                    }
                }),
            container(text(format!("{:.0}", value)).size(13).style(|theme: &Theme| text::Style {
                color: Some(theme.secondary.on.scale_alpha(0.5))
            }))
            .width(Length::Fixed(SLIDER_VALUE_WIDTH))
            .style(|theme: &Theme| container::Style {
                background: Some(Background::Color(theme.primary.base.scale_alpha(0.3))),
                border: Border {
                    color: theme.secondary.base.scale_alpha(0.3),
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
