use super::igs::paint::DrawExecutor;
use crate::EditableScreen;
use icy_parser_core::IgsCommand;

pub struct IgsState {
    pub executor: DrawExecutor,
}

impl IgsState {
    pub fn new() -> Self {
        Self {
            executor: DrawExecutor::default(),
        }
    }
}

fn execute_igs_command(buf: &mut dyn EditableScreen, state: &mut IgsState, cmd: IgsCommand) {
    match cmd {
        IgsCommand::Box { x1, y1, x2, y2, rounded } => {
            if rounded {
                // Rounded box - use polyline to approximate
                state.executor.draw_rounded_rect(buf, x1, y1, x2, y2);
            } else {
                state.executor.draw_rect(buf, x1, y1, x2, y2);
            }
        }

        IgsCommand::Line { x1, y1, x2, y2 } => {
            state.executor.draw_line_pub(buf, x1, y1, x2, y2);
            state.executor.set_cur_position(x2, y2);
        }

        IgsCommand::LineDrawTo { x, y } => {
            let pos = state.executor.get_cur_position();
            state.executor.draw_line_pub(buf, pos.x, pos.y, x, y);
            state.executor.set_cur_position(x, y);
        }

        IgsCommand::Circle { x, y, radius } => {
            state.executor.draw_circle_pub(buf, x, y, radius);
        }

        IgsCommand::Ellipse { x, y, x_radius, y_radius } => {
            state.executor.draw_ellipse_pub(buf, x, y, x_radius, y_radius);
        }

        IgsCommand::Arc {
            x,
            y,
            start_angle,
            end_angle,
            radius,
        } => {
            state.executor.draw_arc_pub(buf, x, y, start_angle, end_angle, radius);
        }

        IgsCommand::PolyLine { points } => {
            if !points.is_empty() {
                state.executor.draw_polyline_pub(buf, &points);
                if points.len() >= 2 {
                    let last_idx = points.len() - 2;
                    state.executor.set_cur_position(points[last_idx], points[last_idx + 1]);
                }
            }
        }

        IgsCommand::PolyFill { points } => {
            if !points.is_empty() {
                state.executor.fill_poly_pub(buf, &points);
            }
        }

        IgsCommand::FloodFill { x, y } => {
            state.executor.flood_fill_pub(buf, x, y);
        }

        IgsCommand::ColorSet { pen, color } => {
            state.executor.set_color(pen, color);
        }

        IgsCommand::AttributeForFills {
            pattern_type,
            pattern_index,
            border,
        } => {
            state.executor.set_fill_pattern(pattern_type, pattern_index);
            state.executor.set_draw_border(border);
        }

        IgsCommand::LineStyle { style, thickness } => {
            state.executor.set_line_style_pub(style);
            state.executor.set_line_thickness(thickness as usize);
        }

        IgsCommand::WriteText { x, y, justification: _, text } => {
            let pos = crate::Position::new(x, y);
            state.executor.write_text(buf, pos, &text);
        }

        IgsCommand::TextEffects { effects, size, rotation } => {
            state.executor.set_text_effects_pub(effects);
            state.executor.set_text_size(size as i32);
            state.executor.set_text_rotation_pub(rotation);
        }

        IgsCommand::BellsAndWhistles { sound_number } => {
            // Sound playback not implemented
            log::info!("IGS BellsAndWhistles sound {} not implemented", sound_number);
        }

        IgsCommand::GraphicScaling { enabled } => {
            // Graphic scaling not implemented
            log::info!("IGS GraphicScaling {} not implemented", enabled);
        }

        IgsCommand::LoopCommand {
            x1: _,
            y1: _,
            x2: _,
            y2: _,
            command: _,
            parameters: _,
        } => {
            // Loop command requires special handling at parser level
            log::warn!("IGS LoopCommand not implemented at this level");
        }

        // VT52 compatibility commands - these use EditableScreen trait methods
        IgsCommand::CursorUp => {
            let pos = buf.caret().position();
            buf.caret_mut().set_position(crate::Position::new(pos.x, pos.y.saturating_sub(1)));
        }

        IgsCommand::CursorDown => {
            let pos = buf.caret().position();
            buf.caret_mut().set_position(crate::Position::new(pos.x, pos.y + 1));
        }

        IgsCommand::CursorRight => {
            let pos = buf.caret().position();
            buf.caret_mut().set_position(crate::Position::new(pos.x + 1, pos.y));
        }

        IgsCommand::CursorLeft => {
            let pos = buf.caret().position();
            buf.caret_mut().set_position(crate::Position::new(pos.x.saturating_sub(1), pos.y));
        }

        IgsCommand::CursorHome => {
            buf.caret_mut().set_position(crate::Position::new(0, 0));
        }

        IgsCommand::ClearScreen => {
            buf.clear_screen();
        }

        IgsCommand::ClearToEOL => {
            buf.clear_line_end();
        }

        IgsCommand::ClearToEOS => {
            buf.clear_buffer_down();
        }

        IgsCommand::SetCursorPos { x, y } => {
            buf.caret_mut().set_position(crate::Position::new(x, y));
        }

        IgsCommand::SetForeground { color } => {
            buf.caret_mut().set_foreground(color as u32);
        }

        IgsCommand::SetBackground { color } => {
            buf.caret_mut().set_background(color as u32);
        }

        IgsCommand::ShowCursor => {
            buf.caret_mut().visible = true;
        }

        IgsCommand::HideCursor => {
            buf.caret_mut().visible = false;
        }

        IgsCommand::SaveCursorPos => {
            *buf.saved_caret_pos() = buf.caret().position();
        }

        IgsCommand::RestoreCursorPos => {
            let pos = *buf.saved_caret_pos();
            buf.caret_mut().set_position(pos);
        }
    }
}

impl crate::PaletteScreenBuffer {
    pub(crate) fn handle_igs_command_impl(&mut self, cmd: IgsCommand) {
        // Initialize IGS state if not present
        if self.igs_state.is_none() {
            self.igs_state = Some(IgsState::new());
        }

        // Take ownership temporarily to avoid borrow conflicts
        let mut state = self.igs_state.take().unwrap();
        execute_igs_command(self, &mut state, cmd);
        self.igs_state = Some(state);

        self.mark_dirty();
    }
}
