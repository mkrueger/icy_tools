use icy_engine::Position;
use icy_engine_gui::BufferView;

use super::{plot_point, BrushMode, ColorMode, PointRole};

pub fn draw_rectangle(buffer_view: &mut BufferView, from: impl Into<Position>, to: impl Into<Position>, mode: BrushMode, color_mode: ColorMode) {
    let mut from = from.into();
    let mut to = to.into();
    let mut y_mul = 1;
    if !matches!(mode, BrushMode::HalfBlock) {
        from.y /= 2;
        to.y /= 2;
        y_mul = 2;
    }

    for x in from.x + 1..to.x {
        plot_point(buffer_view, (x, from.y * y_mul), mode.clone(), color_mode, PointRole::TopSide);
        plot_point(buffer_view, (x, to.y * y_mul), mode.clone(), color_mode, PointRole::BottomSide);
    }

    for y in from.y + 1..to.y {
        plot_point(buffer_view, (from.x, y * y_mul), mode.clone(), color_mode, PointRole::LeftSide);
        plot_point(buffer_view, (to.x, y * y_mul), mode.clone(), color_mode, PointRole::RightSide);
    }

    if from.x != to.x && from.y != to.y {
        plot_point(buffer_view, (from.x, from.y * y_mul), mode.clone(), color_mode, PointRole::NWCorner);
        plot_point(buffer_view, (to.x, from.y * y_mul), mode.clone(), color_mode, PointRole::NECorner);

        plot_point(buffer_view, (from.x, to.y * y_mul), mode.clone(), color_mode, PointRole::SWCorner);
        plot_point(buffer_view, (to.x, to.y * y_mul), mode.clone(), color_mode, PointRole::SECorner);
    }
}

pub fn fill_rectangle(buffer_view: &mut BufferView, from: impl Into<Position>, to: impl Into<Position>, mode: BrushMode, color_mode: ColorMode) {
    let mut from = from.into();
    let mut to = to.into();
    let mut y_mul = 1;
    if !matches!(mode, BrushMode::HalfBlock) {
        from.y /= 2;
        to.y /= 2;
        y_mul = 2;
    }

    for y in from.y + 1..to.y {
        for x in from.x + 1..to.x {
            plot_point(buffer_view, (x, y * y_mul), mode.clone(), color_mode, PointRole::Fill);
        }
    }
    if matches!(mode, BrushMode::HalfBlock) {
        draw_rectangle(buffer_view, from, to, mode, color_mode);
    }
}
