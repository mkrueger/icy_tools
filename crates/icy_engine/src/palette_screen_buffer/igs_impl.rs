use super::igs::paint::DrawExecutor;
use crate::EditableScreen;
use icy_parser_core::{IgsCommand, LineStyleKind};

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
                state.executor.draw_polyline(buf, state.executor.line_color, &points);
                if points.len() >= 2 {
                    let last_idx = points.len() - 2;
                    state.executor.set_cur_position(points[last_idx], points[last_idx + 1]);
                }
            }
        }

        IgsCommand::PolyFill { points } => {
            if !points.is_empty() {
                state.executor.fill_poly(buf, &points);
            }
        }

        IgsCommand::FloodFill { x, y } => {
            state.executor.flood_fill(buf, x, y);
        }

        IgsCommand::ColorSet { pen, color } => {
            log::info!("IGS ColorSet pen={:?} color={}", pen, color);
            state.executor.set_color(pen, color);
        }

        IgsCommand::AttributeForFills { pattern_type, border } => {
            state.executor.set_fill_pattern(pattern_type);
            state.executor.set_draw_border(border);
        }

        IgsCommand::LineStyle { kind, value } => {
            match kind {
                LineStyleKind::Polymarker(pk) => {
                    state.executor.set_polymarker_type(pk as u8);
                    state.executor.set_polymarker_size(value as usize);
                }
                LineStyleKind::Line(lk) => {
                    state.executor.set_line_style_pub(lk as u8);
                    // Extract thickness (lower bits) and end style (higher bits)
                    let thickness: u16 = (value % 50).max(1);
                    state.executor.set_line_thickness(thickness as usize);
                }
            }
        }

        IgsCommand::WriteText { x, y, text } => {
            let pos = crate::Position::new(x, y);
            state.executor.write_text(buf, pos, &text);
        }

        IgsCommand::TextEffects { effects, size, rotation } => {
            state.executor.set_text_effects_pub(effects);
            state.executor.set_text_size(size as i32);
            state.executor.set_text_rotation_pub(rotation as u8);
        }

        IgsCommand::BellsAndWhistles { sound_effect } => {
            // Sound playback not implemented
            log::info!("IGS BellsAndWhistles sound {:?} not implemented", sound_effect);
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
            state.executor.draw_poly_maker(buf, x, y);
            state.executor.set_cur_position(x, y);
        }

        IgsCommand::PieSlice {
            x,
            y,
            radius,
            start_angle,
            end_angle,
        } => {
            state.executor.draw_pieslice_pub(buf, x, y, radius, start_angle, end_angle);
        }

        IgsCommand::EllipticalArc {
            x,
            y,
            x_radius,
            y_radius,
            start_angle,
            end_angle,
        } => {
            state.executor.draw_arc(buf, x, y, x_radius, y_radius, start_angle, end_angle);
        }

        IgsCommand::EllipticalPieSlice {
            x,
            y,
            x_radius,
            y_radius,
            start_angle,
            end_angle,
        } => {
            state
                .executor
                .draw_elliptical_pieslice_pub(buf, x, y, x_radius, y_radius, start_angle, end_angle);
        }

        IgsCommand::RoundedRectangles { x1, y1, x2, y2, fill: _ } => {
            state.executor.draw_rounded_rect(buf, x1, y1, x2, y2);
        }

        IgsCommand::FilledRectangle { x1, y1, x2, y2 } => {
            state.executor.fill_rect(buf, x1, y1, x2, y2);
        }

        // Style and appearance commands
        IgsCommand::SetPenColor { pen, red, green, blue } => {
            // Convert 3-bit RGB (0-7) to 8-bit (0-255)
            // Using value * 34 to match Atari ST convention: 0->0, 7->238
            let r = (red * 34) as u8;
            let g = (green * 34) as u8;
            let b = (blue * 34) as u8;
            buf.palette_mut().set_color(pen as u32, crate::Color::new(r, g, b));
        }

        IgsCommand::DrawingMode { mode } => {
            state.executor.set_drawing_mode(mode);
        }

        IgsCommand::HollowSet { enabled } => {
            // TODO: Toggle hollow/filled mode
            log::info!("IGS HollowSet {} not implemented", enabled);
        }

        // Screen and system commands
        IgsCommand::GrabScreen { operation, mode } => {
            use icy_parser_core::BlitOperation;
            match operation {
                BlitOperation::ScreenToScreen {
                    src_x1,
                    src_y1,
                    src_x2,
                    src_y2,
                    dest_x,
                    dest_y,
                } => {
                    let from_start = crate::Position::new(src_x1, src_y1);
                    let from_end = crate::Position::new(src_x2, src_y2);
                    let dest = crate::Position::new(dest_x, dest_y);
                    state.executor.blit_screen_to_screen(buf, mode as i32, from_start, from_end, dest);
                }
                BlitOperation::ScreenToMemory {
                    src_x1,
                    src_y1,
                    src_x2,
                    src_y2,
                } => {
                    let from_start = crate::Position::new(src_x1, src_y1);
                    let from_end = crate::Position::new(src_x2, src_y2);
                    state.executor.blit_screen_to_memory(buf, mode as i32, from_start, from_end);
                }
                BlitOperation::MemoryToScreen { dest_x, dest_y } => {
                    let dest = crate::Position::new(dest_x, dest_y);
                    let size = state.executor.get_screen_memory_size();
                    state.executor.blit_memory_to_screen(
                        buf,
                        mode as i32,
                        crate::Position::new(0, 0),
                        crate::Position::new(size.width - 1, size.height - 1),
                        dest,
                    );
                }
                BlitOperation::PieceOfMemoryToScreen {
                    src_x1,
                    src_y1,
                    src_x2,
                    src_y2,
                    dest_x,
                    dest_y,
                } => {
                    let from_start = crate::Position::new(src_x1, src_y1);
                    let from_end = crate::Position::new(src_x2, src_y2);
                    let dest = crate::Position::new(dest_x, dest_y);
                    state.executor.blit_memory_to_screen(buf, mode as i32, from_start, from_end, dest);
                }
                BlitOperation::MemoryToMemory {
                    src_x1,
                    src_y1,
                    src_x2,
                    src_y2,
                    dest_x,
                    dest_y,
                } => {
                    log::warn!(
                        "IGS GrabScreen MemoryToMemory not implemented: ({},{}) to ({},{}) -> ({},{})",
                        src_x1,
                        src_y1,
                        src_x2,
                        src_y2,
                        dest_x,
                        dest_y
                    );
                }
            }
        }

        IgsCommand::Initialize { mode } => {
            log::info!("IGS Initialize mode {:?} not implemented", mode);
        }

        IgsCommand::Cursor { mode } => {
            use icy_parser_core::CursorMode;
            match mode {
                CursorMode::Off => buf.caret_mut().visible = false,
                CursorMode::On => buf.caret_mut().visible = true,
                CursorMode::DestructiveBackspace | CursorMode::NonDestructiveBackspace => {
                    log::info!("IGS Cursor backspace mode {:?} not implemented", mode);
                }
            }
        }

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

        IgsCommand::ScreenClear { mode } => {
            // Mode 5 is "Quick VT52 reset" which should reset colors to default
            if mode == 5 {
                // Set default Atari ST palette (16 colors, 3-bit RGB)
                // Standard ST palette: White, Black, Red, Green, Blue, Cyan, Black, Yellow, ...
                let default_colors: [(i32, i32, i32); 16] = [
                    (7, 7, 7), // 0: White
                    (0, 0, 0), // 1: Black
                    (7, 0, 0), // 2: Red
                    (0, 7, 0), // 3: Green
                    (0, 0, 7), // 4: Blue
                    (0, 7, 7), // 5: Cyan
                    (7, 0, 7), // 6: Magenta
                    (7, 7, 0), // 7: Yellow
                    (5, 5, 5), // 8: Light Gray
                    (3, 3, 3), // 9: Dark Gray
                    (7, 3, 3), // 10: Light Red
                    (3, 7, 3), // 11: Light Green
                    (3, 3, 7), // 12: Light Blue
                    (3, 7, 7), // 13: Light Cyan
                    (7, 3, 7), // 14: Light Magenta
                    (7, 7, 3), // 15: Light Yellow
                ];

                for (i, (r, g, b)) in default_colors.iter().enumerate() {
                    let r8 = (r * 34) as u8;
                    let g8 = (g * 34) as u8;
                    let b8 = (b * 34) as u8;
                    buf.palette_mut().set_color(i as u32, crate::Color::new(r8, g8, b8));
                }
            }

            match mode {
                0 | 3 | 4 | 5 => buf.clear_screen(),
                2 => buf.clear_buffer_down(),
                _ => {}
            }
        }

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
            use icy_parser_core::Direction;
            let pos = buf.caret().position();
            let new_pos = match direction {
                Direction::Up => crate::Position::new(pos.x, pos.y.saturating_sub(count)),
                Direction::Down => crate::Position::new(pos.x, pos.y + count),
                Direction::Left => crate::Position::new(pos.x.saturating_sub(count), pos.y),
                Direction::Right => crate::Position::new(pos.x + count, pos.y),
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

        // IGS-specific color commands (ESC b/c)
        IgsCommand::SetForeground { color } => {
            buf.caret_mut().set_foreground(color as u32);
        }

        IgsCommand::SetBackground { color } => {
            buf.caret_mut().set_background(color as u32);
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
