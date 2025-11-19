use super::bgi::{ButtonStyle2, MouseField};
use super::igs::{TerminalResolutionExt, paint::DrawExecutor};
use crate::{ATARI_ST_HIGH_PALETTE, ATARI_ST_MEDIUM_PALETTE, AutoWrapMode, EditableScreen, GraphicsType, IGS_DESKTOP_PALETTE, IGS_PALETTE};
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

static IGS_LOW_COLOR_MAP: [u8; 16] = [0, 15, 1, 2, 4, 6, 3, 5, 7, 8, 9, 10, 12, 14, 11, 13];
// For Medium (4 colors) and High (2 colors), use direct mapping - palette changes via SetPenColor
static IGS_MEDIUM_COLOR_MAP: [u8; 16] = [0, 3, 1, 2, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3];
static IGS_HIGH_COLOR_MAP: [u8; 16] = [0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1];

fn get_color_map(buf: &dyn EditableScreen) -> &'static [u8; 16] {
    if let GraphicsType::IGS(term_res) = buf.graphics_type() {
        match term_res {
            icy_parser_core::TerminalResolution::Low => &IGS_LOW_COLOR_MAP,
            icy_parser_core::TerminalResolution::Medium => &IGS_MEDIUM_COLOR_MAP,
            icy_parser_core::TerminalResolution::High => &IGS_HIGH_COLOR_MAP,
        }
    } else {
        &IGS_LOW_COLOR_MAP
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
            let color_map = get_color_map(buf);
            let color = color_map[color as usize % color_map.len()];
            state.executor.set_color(pen, color);
        }

        IgsCommand::AttributeForFills { pattern_type, border } => {
            state.executor.set_fill_pattern(pattern_type);
            state.executor.set_draw_border(border);
        }

        IgsCommand::LineStyle { kind, value } => {
            match kind {
                LineStyleKind::Polymarker(pk) => {
                    state.executor.polymarker_type = pk;
                    state.executor.set_polymarker_size(value as usize);
                }
                LineStyleKind::Line(lk) => {
                    state.executor.line_kind = lk;
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
            state.executor.text_effects = effects;
            state.executor.text_size = size as i32;
            state.executor.text_rotation = rotation;
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
            let color_map = get_color_map(buf);
            let pen = color_map[pen as usize % color_map.len()];
            buf.palette_mut().set_color(pen as u32, crate::Color::new(r, g, b));
        }

        IgsCommand::DrawingMode { mode } => {
            state.executor.set_drawing_mode(mode);
        }

        IgsCommand::HollowSet { enabled } => {
            state.executor.hollow_set = enabled;
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
                    let from_start: crate::Position = crate::Position::new(src_x1, src_y1);
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
            let resolution = if let GraphicsType::IGS(res) = buf.graphics_type() {
                res
            } else {
                icy_parser_core::TerminalResolution::Low
            };

            if resolution == icy_parser_core::TerminalResolution::Low {
                match mode {
                    icy_parser_core::InitializationType::DesktopPaletteAndAttributes => {
                        *buf.palette_mut() = IGS_PALETTE.clone();
                    }

                    icy_parser_core::InitializationType::DesktopPaletteOnly => {
                        *buf.palette_mut() = IGS_DESKTOP_PALETTE.clone();
                    }

                    icy_parser_core::InitializationType::DesktopAttributesOnly => {
                        // TODO
                    }

                    icy_parser_core::InitializationType::IgDefaultPalette => {
                        *buf.palette_mut() = IGS_PALETTE.clone();
                    }

                    icy_parser_core::InitializationType::VdiDefaultPalette => {
                        *buf.palette_mut() = resolution.get_palette().clone();
                    }

                    icy_parser_core::InitializationType::DesktopResolutionAndClipping => {
                        // TODO
                    }
                }
            }
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

        IgsCommand::AskIG { .. } => {
            unreachable!("Handled in terminal");
        }

        IgsCommand::ScreenClear { mode } => {
            // Mode 5 is "Quick VT52 reset" which should reset colors to default
            if mode == 5 {
                if let GraphicsType::IGS(term) = buf.graphics_type() {
                    *buf.palette_mut() = term.get_palette().clone();
                }
            }

            match mode {
                0 | 3 | 4 | 5 => buf.clear_screen(),
                2 => buf.clear_buffer_down(),
                _ => {}
            }
        }

        IgsCommand::SetResolution { resolution, palette } => {
            use icy_parser_core::PaletteMode;

            buf.set_graphics_type(GraphicsType::IGS(resolution));

            // Update executor's terminal resolution
            state.executor.set_resolution(resolution);

            if resolution == icy_parser_core::TerminalResolution::Low {
                // Apply palette mode if requested
                match palette {
                    PaletteMode::NoChange => {
                        // Keep current palette
                    }
                    PaletteMode::Desktop => {
                        *buf.palette_mut() = IGS_DESKTOP_PALETTE.clone();
                    }
                    PaletteMode::IgDefault => {
                        *buf.palette_mut() = IGS_PALETTE.clone();
                    }
                    PaletteMode::VdiDefault => {
                        // All three modes use the same palette for now (from resolution)
                        *buf.palette_mut() = resolution.get_palette().clone();
                    }
                }
            } else if resolution == icy_parser_core::TerminalResolution::Medium {
                // Medium resolution uses IG default palette
                *buf.palette_mut() = ATARI_ST_MEDIUM_PALETTE.clone();
            } else if resolution == icy_parser_core::TerminalResolution::High {
                // High resolution uses IG default palette
                *buf.palette_mut() = ATARI_ST_HIGH_PALETTE.clone();
            }
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

        IgsCommand::DefineZone {
            zone_id,
            x1,
            y1,
            x2,
            y2,
            length: _,
            string,
        } => {
            // Special zone IDs for clearing
            match zone_id {
                9999 => {
                    // Clear all mouse zones
                    buf.clear_mouse_fields();
                }
                9998 => {
                    // Loopback toggle (not implemented)
                    log::info!("IGS DefineZone: Loopback toggle (9998) not implemented");
                }
                9997 => {
                    // Clear specific zone (not implemented - would need zone tracking)
                    log::info!("IGS DefineZone: Clear specific zone (9997) not implemented");
                }
                _ => {
                    // Define a new mouse zone with clickable region
                    let host_command = if !string.is_empty() { Some(string.clone()) } else { None };

                    // Create a default button style (similar to RIP)
                    let mut style = ButtonStyle2::default();
                    style.flags |= 1024; // Set IGS zone flag

                    buf.add_mouse_field(MouseField::new(
                        x1,
                        y1,
                        x2,
                        y2,
                        host_command.map(|s| String::from_utf8_lossy(&s).to_string()),
                        style,
                    ));
                }
            }
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
        IgsCommand::DeleteLine { count } => {
            buf.remove_terminal_line(count as i32);
        }

        IgsCommand::InsertLine { count, .. } => {
            buf.insert_terminal_line(count as i32);
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
            buf.terminal_state_mut().auto_wrap_mode = if enabled { AutoWrapMode::AutoWrap } else { AutoWrapMode::NoWrap };
        }

        // IGS-specific color commands (ESC b/c)
        IgsCommand::SetForeground { color } => {
            let color_map = get_color_map(buf);
            buf.caret_mut().set_foreground(color_map[color as usize % color_map.len()] as u32);
        }

        IgsCommand::SetBackground { color } => {
            let color_map = get_color_map(buf);
            buf.caret_mut().set_background(color_map[color as usize % color_map.len()] as u32);
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
