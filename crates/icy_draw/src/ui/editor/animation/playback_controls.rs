//! Playback controls widget for animation editor
//!
//! Professional video player-style controls with modern styling

use icy_engine_gui::theme::main_area_background;
use icy_ui::{
    widget::{button, column, container, row, slider, Space},
    Alignment, Background, Border, Element, Length, Shadow, Theme,
};

use super::{icons, AnimationEditor, AnimationEditorMessage};

// === Button Size Constants ===
const TRANSPORT_BUTTON_SIZE: f32 = 28.0;
const TRANSPORT_BUTTON_PADDING: f32 = 4.0;
const PLAY_BUTTON_SIZE: f32 = 40.0;
const SPEED_PICKER_WIDTH: f32 = 65.0;

/// Custom button style for transport controls
fn transport_button_style(theme: &Theme, is_active: bool) -> button::Style {
    if is_active {
        button::Style {
            background: Some(Background::Color(theme.accent.base)),
            text_color: theme.background.on,
            border: Border {
                color: theme.accent.hover,
                width: 1.0,
                radius: 6.0.into(),
            },
            shadow: Shadow {
                color: icy_ui::Color::from_rgba(0.0, 0.0, 0.0, 0.3),
                offset: icy_ui::Vector::new(0.0, 2.0),
                blur_radius: 4.0,
            },
            snap: false,
            ..Default::default()
        }
    } else {
        button::Style {
            background: Some(Background::Color(theme.secondary.base)),
            text_color: theme.background.on,
            border: Border {
                color: theme.primary.divider,
                width: 1.0,
                radius: 6.0.into(),
            },
            shadow: Shadow::default(),
            snap: false,
            ..Default::default()
        }
    }
}

/// Main play button style (larger, more prominent)
fn play_button_style(theme: &Theme, is_playing: bool) -> button::Style {
    let bg_color = if is_playing {
        theme.destructive.base // Red when playing (pause)
    } else {
        theme.success.base // Green when paused (play)
    };

    button::Style {
        background: Some(Background::Color(bg_color)),
        text_color: icy_ui::Color::WHITE,
        border: Border {
            color: icy_ui::Color::TRANSPARENT,
            width: 0.0,
            radius: 20.0.into(), // Circular
        },
        shadow: Shadow {
            color: icy_ui::Color::from_rgba(0.0, 0.0, 0.0, 0.4),
            offset: icy_ui::Vector::new(0.0, 3.0),
            blur_radius: 6.0,
        },
        snap: false,
        ..Default::default()
    }
}

/// Control bar background style
fn control_bar_style(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(main_area_background(theme))),
        border: Border {
            color: theme.primary.divider,
            width: 1.0,
            radius: 8.0.into(),
        },
        shadow: Shadow {
            color: icy_ui::Color::from_rgba(0.0, 0.0, 0.0, 0.2),
            offset: icy_ui::Vector::new(0.0, -2.0),
            blur_radius: 8.0,
        },
        text_color: None,
        snap: false,
    }
}

/// Build the complete playback controls UI (to be placed below terminal)
pub fn view_playback_controls(editor: &AnimationEditor) -> Element<'_, AnimationEditorMessage> {
    let frame_count = editor.frame_count();
    let current_frame = editor.current_frame();
    let is_playing = editor.playback.is_playing;
    let is_loop = editor.playback.is_loop;
    let is_ready = editor.is_ready();
    let has_error = editor.has_error();

    // Disable controls if animation has error or not ready
    let controls_enabled = is_ready && !has_error && frame_count > 0;

    // === Transport Controls ===

    // First frame button
    let first_btn: button::Button<'_, AnimationEditorMessage> = button(container(icons::first_page_icon()).center_x(Length::Fill).center_y(Length::Fill))
        .padding(TRANSPORT_BUTTON_PADDING)
        .width(TRANSPORT_BUTTON_SIZE)
        .height(TRANSPORT_BUTTON_SIZE)
        .style(move |theme, _status| transport_button_style(theme, false))
        .on_press_maybe((controls_enabled && current_frame > 0).then_some(AnimationEditorMessage::FirstFrame));

    // Previous frame button
    let prev_btn = button(container(icons::skip_previous_icon()).center_x(Length::Fill).center_y(Length::Fill))
        .padding(TRANSPORT_BUTTON_PADDING)
        .width(TRANSPORT_BUTTON_SIZE)
        .height(TRANSPORT_BUTTON_SIZE)
        .style(move |theme, _status| transport_button_style(theme, false))
        .on_press_maybe((controls_enabled && current_frame > 0).then_some(AnimationEditorMessage::PreviousFrame));

    // Main Play/Pause button (large, circular)
    let play_pause_btn = if is_playing {
        button(container(icons::pause_icon()).center_x(Length::Fill).center_y(Length::Fill))
            .padding(TRANSPORT_BUTTON_PADDING)
            .width(PLAY_BUTTON_SIZE)
            .height(PLAY_BUTTON_SIZE)
            .style(move |theme, _status| play_button_style(theme, true))
            .on_press_maybe(controls_enabled.then_some(AnimationEditorMessage::TogglePlayback))
    } else {
        button(container(icons::play_icon()).center_x(Length::Fill).center_y(Length::Fill))
            .padding(TRANSPORT_BUTTON_PADDING)
            .width(PLAY_BUTTON_SIZE)
            .height(PLAY_BUTTON_SIZE)
            .style(move |theme, _status| play_button_style(theme, false))
            .on_press_maybe(controls_enabled.then_some(AnimationEditorMessage::TogglePlayback))
    };

    // Next frame button
    let next_btn = button(container(icons::skip_next_icon()).center_x(Length::Fill).center_y(Length::Fill))
        .padding(TRANSPORT_BUTTON_PADDING)
        .width(TRANSPORT_BUTTON_SIZE)
        .height(TRANSPORT_BUTTON_SIZE)
        .style(move |theme, _status| transport_button_style(theme, false))
        .on_press_maybe((controls_enabled && current_frame + 1 < frame_count).then_some(AnimationEditorMessage::NextFrame));

    // Last frame button
    let last_btn = button(container(icons::last_page_icon()).center_x(Length::Fill).center_y(Length::Fill))
        .padding(TRANSPORT_BUTTON_PADDING)
        .width(TRANSPORT_BUTTON_SIZE)
        .height(TRANSPORT_BUTTON_SIZE)
        .style(move |theme, _status| transport_button_style(theme, false))
        .on_press_maybe((controls_enabled && current_frame + 1 < frame_count).then_some(AnimationEditorMessage::LastFrame));

    // Restart button (replay from beginning)
    let restart_btn = button(container(icons::replay_icon()).center_x(Length::Fill).center_y(Length::Fill))
        .padding(TRANSPORT_BUTTON_PADDING)
        .width(TRANSPORT_BUTTON_SIZE)
        .height(TRANSPORT_BUTTON_SIZE)
        .style(move |theme, _status| transport_button_style(theme, false))
        .on_press_maybe(controls_enabled.then_some(AnimationEditorMessage::Restart));

    // Loop toggle button
    let loop_btn = button(container(icons::repeat_icon()).center_x(Length::Fill).center_y(Length::Fill))
        .padding(TRANSPORT_BUTTON_PADDING)
        .width(TRANSPORT_BUTTON_SIZE)
        .height(TRANSPORT_BUTTON_SIZE)
        .style(move |theme, _status| transport_button_style(theme, is_loop))
        .on_press_maybe(controls_enabled.then_some(AnimationEditorMessage::ToggleLoop));

    // === Speed Control ===
    static SPEED_OPTIONS: &[&str] = &["0.25x", "0.5x", "1x", "2x", "4x"];
    let current_speed = match editor.playback_speed() {
        s if s <= 0.25 => "0.25x",
        s if s <= 0.5 => "0.5x",
        s if s <= 1.0 => "1x",
        s if s <= 2.0 => "2x",
        _ => "4x",
    };

    let speed_picker = icy_ui::widget::pick_list(SPEED_OPTIONS, Some(current_speed), |selected| {
        let speed = match selected {
            "0.25x" => 0.25,
            "0.5x" => 0.5,
            "1x" => 1.0,
            "2x" => 2.0,
            "4x" => 4.0,
            _ => 1.0,
        };
        AnimationEditorMessage::SetPlaybackSpeed(speed)
    })
    .width(SPEED_PICKER_WIDTH)
    .text_size(12);

    // === Assemble the control bar ===
    let transport_row = row![
        Space::new().width(Length::Fill),
        first_btn,
        prev_btn,
        Space::new().width(4),
        play_pause_btn,
        Space::new().width(4),
        next_btn,
        last_btn,
        Space::new().width(12),
        restart_btn,
        loop_btn,
        Space::new().width(Length::Fill),
        speed_picker,
    ]
    .spacing(4)
    .align_y(Alignment::Center);

    container(transport_row).width(Length::Fill).padding([8, 12]).style(control_bar_style).into()
}

/// Build the frame slider UI (progress bar style)
pub fn view_frame_slider(editor: &AnimationEditor) -> Element<'_, AnimationEditorMessage> {
    let frame_count = editor.frame_count();
    let current_frame = editor.current_frame();
    let is_ready = editor.is_ready();
    let has_error = editor.has_error();

    if frame_count > 1 && is_ready && !has_error {
        let max_frame = (frame_count.saturating_sub(1)) as f32;

        // Create a styled progress slider
        let progress_slider = slider(0.0..=max_frame, current_frame as f32, |v| AnimationEditorMessage::SeekFrame(v as usize))
            .width(Length::Fill)
            .step(1.0);

        container(progress_slider).width(Length::Fill).padding([0, 4]).into()
    } else {
        Space::new().height(4).into()
    }
}

/// Combined controls view - slider + transport (for placing below preview)
pub fn view_player_controls(editor: &AnimationEditor) -> Element<'_, AnimationEditorMessage> {
    let slider = view_frame_slider(editor);
    let controls = view_playback_controls(editor);

    column![slider, Space::new().height(4), controls,].spacing(0).width(Length::Fill).into()
}
