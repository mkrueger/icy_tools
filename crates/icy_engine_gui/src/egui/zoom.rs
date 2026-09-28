use std::ops::RangeInclusive;

use ::egui;

use crate::ScalingMode;

const INTEGER_TRACKPAD_THRESHOLD: f32 = 8.0;

/// Applies Ctrl/Cmd+mouse-wheel zoom while the pointer is over `ui`.
///
/// The wheel input is consumed so the same gesture does not also scroll the content.
pub fn mouse_wheel(ui: &mut egui::Ui, enabled: bool, current_zoom: f32, use_integer_scaling: bool, range: RangeInclusive<f32>) -> Option<ScalingMode> {
    let bounds = ui.max_rect();
    if !enabled || !ui.rect_contains_pointer(bounds) {
        return None;
    }
    let zoom = if use_integer_scaling {
        let (steps, smooth) = ui.input(|input| {
            input
                .events
                .iter()
                .filter_map(|event| match event {
                    egui::Event::MouseWheel { unit, delta, modifiers } if modifiers.command => Some((*unit, delta.x + delta.y)),
                    _ => None,
                })
                .fold((0, 0.0), |(steps, smooth), (unit, delta)| {
                    if unit == egui::MouseWheelUnit::Point && delta.abs() < INTEGER_TRACKPAD_THRESHOLD {
                        (steps, smooth + delta)
                    } else {
                        (steps + delta.signum() as i32, smooth)
                    }
                })
        });
        let id = ui.id().with("icy-wheel-zoom");
        let mut accumulated = ui.data(|data| data.get_temp::<f32>(id)).unwrap_or_default() + smooth;
        let smooth_steps = (accumulated / INTEGER_TRACKPAD_THRESHOLD).trunc() as i32;
        accumulated -= smooth_steps as f32 * INTEGER_TRACKPAD_THRESHOLD;
        ui.data_mut(|data| data.insert_temp(id, accumulated));
        zoom_from_steps(current_zoom, steps + smooth_steps, range)
    } else {
        zoom_from_factor(current_zoom, ui.input(|input| input.zoom_delta()), range)
    };
    if zoom.is_some() {
        ui.input_mut(|input| {
            input.smooth_scroll_delta = egui::Vec2::ZERO;
            input.raw_scroll_delta = egui::Vec2::ZERO;
        });
    }
    zoom.map(ScalingMode::Manual)
}

fn zoom_from_factor(current_zoom: f32, factor: f32, range: RangeInclusive<f32>) -> Option<f32> {
    if factor == 1.0 {
        return None;
    }
    let minimum = (*range.start()).min(current_zoom);
    changed(current_zoom, (current_zoom * factor).clamp(minimum, *range.end()))
}

fn zoom_from_steps(current_zoom: f32, steps: i32, range: RangeInclusive<f32>) -> Option<f32> {
    if steps == 0 || steps < 0 && current_zoom <= 1.0 {
        return None;
    }
    let zoom = if steps > 0 {
        current_zoom.floor() + steps as f32
    } else {
        current_zoom.ceil() + steps as f32
    };
    changed(current_zoom, zoom.clamp((*range.start()).max(1.0), *range.end()))
}

fn changed(current_zoom: f32, zoom: f32) -> Option<f32> {
    ((zoom - current_zoom).abs() > f32::EPSILON).then_some(zoom)
}

#[cfg(test)]
mod tests {
    use super::{mouse_wheel, zoom_from_factor, zoom_from_steps};

    #[test]
    fn smooth_wheel_zoom_is_proportional_and_clamped() {
        let zoomed_in = zoom_from_factor(1.0, 1.25, 0.5..=4.0).unwrap();
        let zoomed_out = zoom_from_factor(1.0, 0.8, 0.5..=4.0).unwrap();
        assert!(zoomed_in > 1.0);
        assert!(zoomed_out < 1.0);
        assert_eq!(zoom_from_factor(4.0, 1.25, 0.5..=4.0), None);
        assert_eq!(zoom_from_factor(0.5, 0.8, 0.5..=4.0), None);
        assert_eq!(zoom_from_factor(1.0, 1.0, 0.5..=4.0), None);
        assert_eq!(zoom_from_factor(0.3, 0.8, 0.5..=4.0), None);
    }

    #[test]
    fn integer_wheel_zoom_uses_whole_steps() {
        assert_eq!(zoom_from_steps(1.0, 1, 0.5..=8.0), Some(2.0));
        assert_eq!(zoom_from_steps(3.0, -1, 0.5..=8.0), Some(2.0));
        assert_eq!(zoom_from_steps(1.0, -1, 0.5..=8.0), None);
        assert_eq!(zoom_from_steps(0.3, -1, 0.5..=8.0), None);
        assert_eq!(zoom_from_steps(0.3, 1, 0.5..=8.0), Some(1.0));
    }

    #[test]
    fn integer_trackpad_deltas_accumulate_before_stepping() {
        let context = egui::Context::default();
        let frame = |delta| {
            let mut result = None;
            let _ = context.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 300.0))),
                    events: vec![
                        egui::Event::PointerMoved(egui::pos2(100.0, 100.0)),
                        egui::Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            delta: egui::vec2(0.0, delta),
                            modifiers: egui::Modifiers {
                                command: true,
                                ctrl: true,
                                ..Default::default()
                            },
                        },
                    ],
                    ..Default::default()
                },
                |context| {
                    egui::CentralPanel::default().show(context, |ui| {
                        result = mouse_wheel(ui, true, 1.0, true, 0.5..=4.0);
                    });
                },
            );
            result
        };

        assert_eq!(frame(4.0), None);
        assert_eq!(frame(4.0), Some(crate::ScalingMode::Manual(2.0)));
    }

    #[test]
    fn command_wheel_over_ui_changes_zoom_and_consumes_scroll() {
        let context = egui::Context::default();
        let _ = context.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 300.0))),
                modifiers: egui::Modifiers {
                    command: true,
                    ctrl: true,
                    ..Default::default()
                },
                events: vec![
                    egui::Event::PointerMoved(egui::pos2(100.0, 100.0)),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0.0, 100.0),
                        modifiers: egui::Modifiers {
                            command: true,
                            ctrl: true,
                            ..Default::default()
                        },
                    },
                ],
                ..Default::default()
            },
            |context| {
                egui::CentralPanel::default().show(context, |ui| {
                    let mode = mouse_wheel(ui, true, 1.0, false, 0.5..=4.0);
                    assert!(matches!(mode, Some(crate::ScalingMode::Manual(zoom)) if zoom > 1.0));
                    assert_eq!(ui.input(|input| input.smooth_scroll_delta), egui::Vec2::ZERO);
                    assert_eq!(ui.input(|input| input.raw_scroll_delta), egui::Vec2::ZERO);
                });
            },
        );
    }
}
