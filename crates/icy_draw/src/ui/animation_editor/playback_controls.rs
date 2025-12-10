//! Playback controls widget for animation editor

use iced::{
    Alignment, Element, Length,
    widget::{Space, button, container, row, slider, text},
};

use super::{AnimationEditor, AnimationEditorMessage, icons};

/// Build the playback controls UI
pub fn view_playback_controls(editor: &AnimationEditor) -> Element<'_, AnimationEditorMessage> {
    let frame_count = editor.frame_count();
    let current_frame = editor.current_frame();
    let is_playing = editor.playback.is_playing;
    let is_loop = editor.playback.is_loop;
    let is_ready = editor.is_ready();
    let has_error = editor.has_error();

    // Disable controls if animation has error or not ready
    let controls_enabled = is_ready && !has_error && frame_count > 0;

    // Play/Pause button
    let play_pause_btn = if is_playing {
        button(container(icons::pause_icon()).center_x(Length::Fill).center_y(Length::Fill))
            .width(32)
            .height(32)
            .on_press_maybe(controls_enabled.then_some(AnimationEditorMessage::TogglePlayback))
    } else {
        let icon = if current_frame + 1 >= frame_count && frame_count > 0 {
            icons::replay_icon()
        } else {
            icons::play_icon()
        };
        button(container(icon).center_x(Length::Fill).center_y(Length::Fill))
            .width(32)
            .height(32)
            .on_press_maybe(controls_enabled.then_some(AnimationEditorMessage::TogglePlayback))
    };

    // Previous frame button
    let prev_btn = button(container(icons::skip_previous_icon()).center_x(Length::Fill).center_y(Length::Fill))
        .width(32)
        .height(32)
        .on_press_maybe((controls_enabled && current_frame > 0).then_some(AnimationEditorMessage::PreviousFrame));

    // Next frame button
    let next_btn = button(container(icons::skip_next_icon()).center_x(Length::Fill).center_y(Length::Fill))
        .width(32)
        .height(32)
        .on_press_maybe((controls_enabled && current_frame + 1 < frame_count).then_some(AnimationEditorMessage::NextFrame));

    // First frame button
    let first_btn = button(container(icons::first_page_icon()).center_x(Length::Fill).center_y(Length::Fill))
        .width(32)
        .height(32)
        .on_press_maybe((controls_enabled && current_frame > 0).then_some(AnimationEditorMessage::FirstFrame));

    // Last frame button
    let last_btn = button(container(icons::last_page_icon()).center_x(Length::Fill).center_y(Length::Fill))
        .width(32)
        .height(32)
        .on_press_maybe((controls_enabled && current_frame + 1 < frame_count).then_some(AnimationEditorMessage::LastFrame));

    // Loop toggle button
    let loop_btn = button(container(icons::repeat_icon()).center_x(Length::Fill).center_y(Length::Fill))
        .width(32)
        .height(32)
        .style(if is_loop {
            iced::widget::button::primary
        } else {
            iced::widget::button::secondary
        })
        .on_press_maybe(controls_enabled.then_some(AnimationEditorMessage::ToggleLoop));

    // Frame counter
    let frame_display = if frame_count > 0 {
        text(format!("{} / {}", current_frame + 1, frame_count)).size(13).font(iced::Font::MONOSPACE)
    } else {
        text("0 / 0").size(13).font(iced::Font::MONOSPACE)
    };

    // Playback speed control
    static SPEED_OPTIONS: &[&str] = &["0.25x", "0.5x", "1x", "2x", "4x"];
    let current_speed = match editor.playback_speed() {
        s if s <= 0.25 => "0.25x",
        s if s <= 0.5 => "0.5x",
        s if s <= 1.0 => "1x",
        s if s <= 2.0 => "2x",
        _ => "4x",
    };
    let speed_picker = iced::widget::pick_list(SPEED_OPTIONS, Some(current_speed), |selected| {
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
    .width(70)
    .text_size(13);

    row![
        first_btn,
        prev_btn,
        play_pause_btn,
        next_btn,
        last_btn,
        Space::new().width(8),
        loop_btn,
        Space::new().width(16),
        frame_display,
        Space::new().width(Length::Fill),
        text("Speed:").size(13),
        Space::new().width(4),
        speed_picker,
    ]
    .spacing(2)
    .align_y(Alignment::Center)
    .into()
}

/// Build the frame slider UI (separate for layout flexibility)
pub fn view_frame_slider(editor: &AnimationEditor) -> Element<'_, AnimationEditorMessage> {
    let frame_count = editor.frame_count();
    let current_frame = editor.current_frame();
    let is_ready = editor.is_ready();
    let has_error = editor.has_error();

    if frame_count > 1 && is_ready && !has_error {
        let max_frame = (frame_count.saturating_sub(1)) as f32;
        slider(0.0..=max_frame, current_frame as f32, |v| AnimationEditorMessage::SeekFrame(v as usize))
            .width(Length::Fill)
            .into()
    } else {
        Space::new().height(0).into()
    }
}
