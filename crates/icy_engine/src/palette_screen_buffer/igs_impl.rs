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

        IgsCommand::LineStyle { kind, style, value } => {
            if kind == 2 {
                // Lines
                state.executor.set_line_style_pub(style);
                // Extract thickness (lower bits) and end style (higher bits)
                let thickness = (value % 50).max(1);
                state.executor.set_line_thickness(thickness as usize);
            }
            // kind == 1 for polymarkers - not yet fully implemented
        }

        IgsCommand::WriteText { x, y, text } => {
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

        IgsCommand::AlterSoundEffect { .. } => {
            log::info!("IGS AlterSoundEffect not implemented");
        }

        IgsCommand::StopAllSound => {
            log::info!("IGS StopAllSound not implemented");
        }

        IgsCommand::RestoreSoundEffect { .. } => {
            log::info!("IGS RestoreSoundEffect not implemented");
        }

        IgsCommand::SetEffectLoops { .. } => {
            log::info!("IGS SetEffectLoops not implemented");
        }

        IgsCommand::GraphicScaling { mode } => {
            // Graphic scaling not implemented
            log::info!("IGS GraphicScaling mode {} not implemented", mode);
        }

        IgsCommand::Loop(_) => {
            // Loop command requires special handling at parser level
            log::warn!("IGS Loop command not implemented at this level");
        }

        // Additional drawing commands
        IgsCommand::PolymarkerPlot { x, y } => {
            state.executor.set_cur_position(x, y);
            // Actual plotting of polymarker not yet implemented
        }

        IgsCommand::PieSlice {
            x,
            y,
            radius,
            start_angle,
            end_angle,
        } => {
            state.executor.draw_arc_pub(buf, x, y, start_angle, end_angle, radius);
            // TODO: Fill the pie slice
        }

        IgsCommand::EllipticalArc {
            x: _,
            y: _,
            x_radius: _,
            y_radius: _,
            start_angle: _,
            end_angle: _,
        } => {
            // TODO: Implement elliptical arc
            log::info!("IGS EllipticalArc not fully implemented");
        }

        IgsCommand::EllipticalPieSlice {
            x: _,
            y: _,
            x_radius: _,
            y_radius: _,
            start_angle: _,
            end_angle: _,
        } => {
            // TODO: Implement elliptical pie slice
            log::info!("IGS EllipticalPieSlice not fully implemented");
        }

        IgsCommand::RoundedRectangles { x1, y1, x2, y2, fill: _ } => {
            state.executor.draw_rounded_rect(buf, x1, y1, x2, y2);
        }

        IgsCommand::FilledRectangle { x1, y1, x2, y2 } => {
            state.executor.draw_rect(buf, x1, y1, x2, y2);
        }

        // Style and appearance commands
        IgsCommand::SetPenColor {
            pen: _,
            red: _,
            green: _,
            blue: _,
        } => {
            // TODO: Set pen RGB values
            log::info!("IGS SetPenColor not implemented");
        }

        IgsCommand::DrawingMode { mode } => {
            // TODO: Implement drawing modes (Replace, Transparent, XOR, etc.)
            log::info!("IGS DrawingMode {} not implemented", mode);
        }

        IgsCommand::HollowSet { enabled } => {
            // TODO: Toggle hollow/filled mode
            log::info!("IGS HollowSet {} not implemented", enabled);
        }

        // Screen and system commands
        IgsCommand::GrabScreen { blit_type, mode, params: _ } => {
            log::info!("IGS GrabScreen (type {}, mode {}) not implemented", blit_type, mode);
        }

        IgsCommand::Initialize { mode } => {
            log::info!("IGS Initialize mode {} not implemented", mode);
        }

        IgsCommand::Cursor { mode } => match mode {
            0 => buf.caret_mut().visible = false,
            1 => buf.caret_mut().visible = true,
            _ => {}
        },

        IgsCommand::ChipMusic { .. } => {
            log::info!("IGS ChipMusic not implemented");
        }

        IgsCommand::Noise { .. } => {
            log::info!("IGS Noise not implemented");
        }

        IgsCommand::InputCommand { .. } => {
            log::info!("IGS InputCommand not implemented");
        }

        IgsCommand::AskIG { query } => {
            log::info!("IGS AskIG query {} not implemented", query);
        }

        IgsCommand::ScreenClear { mode } => match mode {
            0 | 3 | 4 | 5 => buf.clear_screen(),
            2 => buf.clear_buffer_down(),
            _ => {}
        },

        IgsCommand::SetResolution { resolution, palette } => {
            log::info!("IGS SetResolution {} palette {} not implemented", resolution, palette);
        }

        IgsCommand::PauseSeconds { seconds: _ } => {
            // Pauses are typically ignored in non-real-time rendering
        }

        IgsCommand::VsyncPause { vsyncs: _ } => {
            // Pauses are typically ignored in non-real-time rendering
        }

        // Extended X commands
        IgsCommand::SprayPaint {
            x: _,
            y: _,
            width: _,
            height: _,
            density: _,
        } => {
            log::info!("IGS SprayPaint not implemented");
        }

        IgsCommand::SetColorRegister { register, value } => {
            log::info!("IGS SetColorRegister {} = {} not implemented", register, value);
        }

        IgsCommand::SetRandomRange { .. } => {
            log::info!("IGS SetRandomRange not implemented");
        }

        IgsCommand::RightMouseMacro { .. } => {
            log::info!("IGS RightMouseMacro not implemented");
        }

        IgsCommand::DefineZone { .. } => {
            log::info!("IGS DefineZone not implemented");
        }

        IgsCommand::FlowControl { .. } => {
            log::info!("IGS FlowControl not implemented");
        }

        IgsCommand::LeftMouseButton { .. } => {
            log::info!("IGS LeftMouseButton not implemented");
        }

        IgsCommand::LoadFillPattern { .. } => {
            log::info!("IGS LoadFillPattern not implemented");
        }

        IgsCommand::RotateColorRegisters { .. } => {
            log::info!("IGS RotateColorRegisters not implemented");
        }

        IgsCommand::LoadMidiBuffer { .. } => {
            log::info!("IGS LoadMidiBuffer not implemented");
        }

        IgsCommand::SetDrawtoBegin { x, y } => {
            state.executor.set_cur_position(x, y);
        }

        IgsCommand::LoadBitblitMemory { .. } => {
            log::info!("IGS LoadBitblitMemory not implemented");
        }

        IgsCommand::LoadColorPalette { .. } => {
            log::info!("IGS LoadColorPalette not implemented");
        }

        // Additional VT52 commands
        IgsCommand::DeleteLine { .. } => {
            log::info!("IGS DeleteLine not implemented");
        }

        IgsCommand::InsertLine { .. } => {
            log::info!("IGS InsertLine not implemented");
        }

        IgsCommand::ClearLine { .. } => {
            buf.clear_line_end();
        }

        IgsCommand::CursorMotion { direction, count } => {
            let pos = buf.caret().position();
            let new_pos = match direction {
                0 => crate::Position::new(pos.x, pos.y.saturating_sub(count)), // up
                1 => crate::Position::new(pos.x, pos.y + count),               // down
                2 => crate::Position::new(pos.x.saturating_sub(count), pos.y), // left
                3 => crate::Position::new(pos.x + count, pos.y),               // right
                _ => pos,
            };
            buf.caret_mut().set_position(new_pos);
        }

        IgsCommand::PositionCursor { x, y } => {
            buf.caret_mut().set_position(crate::Position::new(x, y));
        }

        IgsCommand::RememberCursor { .. } => {
            *buf.saved_caret_pos() = buf.caret().position();
        }

        IgsCommand::InverseVideo { enabled } => {
            // TODO: Implement inverse video mode
            log::info!("IGS InverseVideo {} not implemented", enabled);
        }

        IgsCommand::LineWrap { enabled } => {
            // TODO: Implement line wrap control
            log::info!("IGS LineWrap {} not implemented", enabled);
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
