use super::{TerminalResolutionExt, vdi_paint::VdiPaint};
use crate::bgi::{ButtonStyle2, MouseField};
use crate::{ATARI_ST_HIGH_PALETTE, ATARI_ST_MEDIUM_PALETTE, AutoWrapMode, EditableScreen, GraphicsType, IGS_DESKTOP_PALETTE, IGS_PALETTE};
use icy_parser_core::{DrawingMode, IgsCommand, LineMarkerStyle, PatternType, PenType};

static IGS_LOW_COLOR_MAP: [u8; 16] = [0, 15, 1, 2, 4, 6, 3, 5, 7, 8, 9, 10, 12, 14, 11, 13];
// For Medium (4 colors) and High (2 colors), use direct mapping - palette changes via SetPenColor
static IGS_MEDIUM_COLOR_MAP: [u8; 16] = [0, 3, 1, 2, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3];
static IGS_HIGH_COLOR_MAP: [u8; 16] = [0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1];

fn get_color_map(buf: &dyn EditableScreen) -> (usize, &'static [u8; 16]) {
    if let GraphicsType::IGS(term_res) = buf.graphics_type() {
        match term_res {
            icy_parser_core::TerminalResolution::Low => (16, &IGS_LOW_COLOR_MAP),
            icy_parser_core::TerminalResolution::Medium => (4, &IGS_MEDIUM_COLOR_MAP),
            icy_parser_core::TerminalResolution::High => (2, &IGS_HIGH_COLOR_MAP),
        }
    } else {
        (16, &IGS_LOW_COLOR_MAP)
    }
}

fn run_igs_command(buf: &mut dyn EditableScreen, paint: &mut VdiPaint, cmd: IgsCommand) {
    match cmd {
        IgsCommand::Box { x1, y1, x2, y2, rounded } => {
            let (x1, y1, x2, y2) = (
                x1.evaluate(&paint.random_bounds, 0, 0),
                y1.evaluate(&paint.random_bounds, 0, 0),
                x2.evaluate(&paint.random_bounds, 0, 0),
                y2.evaluate(&paint.random_bounds, 0, 0),
            );
            if rounded {
                // Rounded box - use polyline to approximate
                paint.draw_rounded_rect(buf, x1, y1, x2, y2);
            } else {
                paint.draw_rect_pub(buf, x1, y1, x2, y2);
            }
        }

        IgsCommand::Line { x1, y1, x2, y2 } => {
            let (x1, y1, x2, y2) = (
                x1.evaluate(&paint.random_bounds, 0, 0),
                y1.evaluate(&paint.random_bounds, 0, 0),
                x2.evaluate(&paint.random_bounds, 0, 0),
                y2.evaluate(&paint.random_bounds, 0, 0),
            );
            paint.draw_line_pub(buf, x1, y1, x2, y2);
            paint.draw_to_position = (x2, y2).into();
        }

        IgsCommand::LineDrawTo { x, y } => {
            let (x, y) = (x.evaluate(&paint.random_bounds, 0, 0), y.evaluate(&paint.random_bounds, 0, 0));
            let pos = paint.draw_to_position;
            paint.draw_line_pub(buf, pos.x, pos.y, x, y);
            paint.draw_to_position = (x, y).into();
        }

        IgsCommand::Circle { x, y, radius } => {
            let (x, y, radius) = (
                x.evaluate(&paint.random_bounds, 0, 0),
                y.evaluate(&paint.random_bounds, 0, 0),
                radius.evaluate(&paint.random_bounds, 0, 0),
            );
            paint.draw_circle_pub(buf, x, y, radius);
        }

        IgsCommand::Ellipse { x, y, x_radius, y_radius } => {
            let (x, y, x_radius, y_radius) = (
                x.evaluate(&paint.random_bounds, 0, 0),
                y.evaluate(&paint.random_bounds, 0, 0),
                x_radius.evaluate(&paint.random_bounds, 0, 0),
                y_radius.evaluate(&paint.random_bounds, 0, 0),
            );
            paint.draw_ellipse_pub(buf, x, y, x_radius, y_radius);
        }

        IgsCommand::Arc {
            x,
            y,
            start_angle,
            end_angle,
            radius,
        } => {
            let (x, y, radius, start_angle, end_angle) = (
                x.evaluate(&paint.random_bounds, 0, 0),
                y.evaluate(&paint.random_bounds, 0, 0),
                radius.evaluate(&paint.random_bounds, 0, 0),
                start_angle.evaluate(&paint.random_bounds, 0, 0),
                end_angle.evaluate(&paint.random_bounds, 0, 0),
            );
            paint.draw_arc_pub(buf, x, y, radius, radius, start_angle, end_angle);
        }

        IgsCommand::PolyLine { points } => {
            if !points.is_empty() {
                let int_points: Vec<i32> = points.iter().map(|p| p.evaluate(&paint.random_bounds, 0, 0)).collect();
                paint.draw_polyline(buf, paint.line_color, &int_points);
                if int_points.len() >= 2 {
                    let last_idx = int_points.len() - 2;
                    paint.draw_to_position = (int_points[last_idx], int_points[last_idx + 1]).into();
                }
            }
        }

        IgsCommand::PolyFill { points } => {
            if !points.is_empty() {
                let int_points: Vec<i32> = points.iter().map(|p| p.evaluate(&paint.random_bounds, 0, 0)).collect();
                paint.fill_poly(buf, &int_points);
            }
        }

        IgsCommand::FloodFill { x, y } => {
            let (x, y) = (x.evaluate(&paint.random_bounds, 0, 0), y.evaluate(&paint.random_bounds, 0, 0));
            paint.flood_fill(buf, x, y);
        }

        IgsCommand::ColorSet { pen, color } => {
            let (_, color_map) = get_color_map(buf);

            let mapped_color = color_map[color as usize % color_map.len()];
            match pen {
                PenType::Polymarker => paint.polymarker_color = mapped_color,
                PenType::Line => paint.line_color = mapped_color,
                PenType::Fill => paint.fill_color = mapped_color,
                PenType::Text => paint.text_color = mapped_color,
            }
        }

        IgsCommand::AttributeForFills { pattern_type, border } => {
            paint.set_fill_pattern(pattern_type);
            paint.fill_draw_border = border;
        }

        IgsCommand::SetLineOrMarkerStyle { style } => {
            match style {
                LineMarkerStyle::PolyMarkerSize(pk, size) => {
                    paint.polymarker_type = pk;
                    paint.polymarker_size = size as i32;
                }
                LineMarkerStyle::LineThickness(lk, thickness) => {
                    paint.line_kind = lk;
                    paint.line_thickness = thickness as i32;
                    // Thickness mode: no special endpoint handling needed
                }
                LineMarkerStyle::LineEndpoints(lk, _left, _right) => {
                    paint.line_kind = lk;
                    // TODO: Implement vsl_ends() for arrow/rounded endpoints
                    // For now, just set the line kind
                }
            }
        }

        IgsCommand::WriteText { x, y, text } => {
            let (x, y) = (x.evaluate(&paint.random_bounds, 0, 0), y.evaluate(&paint.random_bounds, 0, 0));
            let pos = crate::Position::new(x, y);
            paint.write_text(buf, pos, &text);
        }

        IgsCommand::TextEffects { effects, size, rotation } => {
            paint.text_effects = effects;
            paint.text_size = size as i32;
            paint.text_rotation = rotation;
        }

        IgsCommand::BellsAndWhistles { .. } => {
            // Handled by terminal thread.
        }

        IgsCommand::AlterSoundEffect { .. } => {
            // Handled by terminal thread.
        }

        IgsCommand::StopAllSound => {
            // Handled by terminal thread.
        }

        IgsCommand::RestoreSoundEffect { .. } => {
            // Handled by terminal thread.
        }

        IgsCommand::SetEffectLoops { .. } => {
            // Handled by terminal thread.
        }

        IgsCommand::GraphicScaling { mode } => {
            paint.scaling_mode = mode;
        }

        IgsCommand::Loop(_) => {
            unreachable!("Handled in parser backend.");
        }

        // Additional drawing commands
        IgsCommand::PolymarkerPlot { x, y } => {
            let (x, y) = (x.evaluate(&paint.random_bounds, 0, 0), y.evaluate(&paint.random_bounds, 0, 0));
            paint.draw_poly_marker(buf, x, y);
            paint.draw_to_position = (x, y).into();
        }

        IgsCommand::PieSlice {
            x,
            y,
            radius,
            start_angle,
            end_angle,
        } => {
            let (x, y, radius, start_angle, end_angle) = (
                x.evaluate(&paint.random_bounds, 0, 0),
                y.evaluate(&paint.random_bounds, 0, 0),
                radius.evaluate(&paint.random_bounds, 0, 0),
                start_angle.evaluate(&paint.random_bounds, 0, 0),
                end_angle.evaluate(&paint.random_bounds, 0, 0),
            );
            paint.draw_pieslice_pub(buf, x, y, radius, start_angle, end_angle);
        }

        IgsCommand::EllipticalArc {
            x,
            y,
            x_radius,
            y_radius,
            start_angle,
            end_angle,
        } => {
            let (x, y, x_radius, y_radius, start_angle, end_angle) = (
                x.evaluate(&paint.random_bounds, 0, 0),
                y.evaluate(&paint.random_bounds, 0, 0),
                x_radius.evaluate(&paint.random_bounds, 0, 0),
                y_radius.evaluate(&paint.random_bounds, 0, 0),
                start_angle.evaluate(&paint.random_bounds, 0, 0),
                end_angle.evaluate(&paint.random_bounds, 0, 0),
            );
            paint.draw_arc_pub(buf, x, y, x_radius, y_radius, start_angle, end_angle);
        }

        IgsCommand::EllipticalPieSlice {
            x,
            y,
            x_radius,
            y_radius,
            start_angle,
            end_angle,
        } => {
            let (x, y, x_radius, y_radius, start_angle, end_angle) = (
                x.evaluate(&paint.random_bounds, 0, 0),
                y.evaluate(&paint.random_bounds, 0, 0),
                x_radius.evaluate(&paint.random_bounds, 0, 0),
                y_radius.evaluate(&paint.random_bounds, 0, 0),
                start_angle.evaluate(&paint.random_bounds, 0, 0),
                end_angle.evaluate(&paint.random_bounds, 0, 0),
            );
            paint.draw_elliptical_pieslice_pub(buf, x, y, x_radius, y_radius, start_angle, end_angle);
        }

        IgsCommand::RoundedRectangles { x1, y1, x2, y2, fill: _ } => {
            let (x1, y1, x2, y2) = (
                x1.evaluate(&paint.random_bounds, 0, 0),
                y1.evaluate(&paint.random_bounds, 0, 0),
                x2.evaluate(&paint.random_bounds, 0, 0),
                y2.evaluate(&paint.random_bounds, 0, 0),
            );
            paint.draw_rounded_rect(buf, x1, y1, x2, y2);
        }

        IgsCommand::FilledRectangle { x1, y1, x2, y2 } => {
            let (x1, y1, x2, y2) = (
                x1.evaluate(&paint.random_bounds, 0, 0),
                y1.evaluate(&paint.random_bounds, 0, 0),
                x2.evaluate(&paint.random_bounds, 0, 0),
                y2.evaluate(&paint.random_bounds, 0, 0),
            );
            paint.fill_rect(buf, x1, y1, x2, y2);
        }

        // Style and appearance commands
        IgsCommand::SetPenColor { pen, red, green, blue } => {
            let r = (red * 34) as u8;
            let g = (green * 34) as u8;
            let b = (blue * 34) as u8;
            let (palette_size, color_map) = get_color_map(buf);

            // For Medium (4 colors) and High (2 colors) resolution, only accept pens 0-3 or 0-1
            // Ignore attempts to set pens outside the valid range for the current resolution
            if (pen as usize) < palette_size {
                let mapped_pen = color_map[pen as usize];
                buf.palette_mut().set_color(mapped_pen as u32, crate::Color::new(r, g, b));
            }
        }

        IgsCommand::DrawingMode { mode } => {
            paint.drawing_mode = mode;
        }

        IgsCommand::HollowSet { enabled } => {
            if enabled {
                // H command sets: vsf_interior(0), vswr_mode(2), vsf_perimeter(1)
                paint.set_fill_pattern(PatternType::Hollow);
                paint.drawing_mode = DrawingMode::Transparent;
                paint.fill_draw_border = true;
            } else {
                // Restore to solid fill with replace mode
                paint.set_fill_pattern(PatternType::Solid);
                paint.drawing_mode = DrawingMode::Replace;
                paint.fill_draw_border = false;
            }
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
                    paint.blit_screen_to_screen(buf, mode, from_start, from_end, dest);
                }
                BlitOperation::ScreenToMemory {
                    src_x1,
                    src_y1,
                    src_x2,
                    src_y2,
                } => {
                    let from_start = crate::Position::new(src_x1, src_y1);
                    let from_end = crate::Position::new(src_x2, src_y2);
                    paint.blit_screen_to_memory(buf, mode, from_start, from_end);
                }
                BlitOperation::MemoryToScreen { dest_x, dest_y } => {
                    let dest = crate::Position::new(dest_x, dest_y);
                    paint.blit_memory_to_screen(buf, mode, dest);
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
                    paint.blit_piece_of_memory_to_screen(buf, mode, from_start, from_end, dest);
                }
                BlitOperation::MemoryToMemory {
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
                    paint.blit_memory_to_memory(mode, from_start, from_end, dest);
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
                        *buf.palette_mut() = IGS_DESKTOP_PALETTE.clone();
                        buf.reset_resolution();
                        paint.reset_attributes();
                    }

                    icy_parser_core::InitializationType::DesktopPaletteOnly => {
                        *buf.palette_mut() = IGS_DESKTOP_PALETTE.clone();
                    }

                    icy_parser_core::InitializationType::DesktopAttributesOnly => {
                        paint.reset_attributes();
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
            // Handled in terminal
        }

        IgsCommand::Noise { .. } => {
            // Handled in terminal
        }

        IgsCommand::InputCommand { .. } => {
            log::info!("IGS InputCommand not implemented");
        }

        IgsCommand::AskIG { .. } => {
            // Handled in terminal (may happen in unit tests)
        }

        IgsCommand::ScreenClear { mode } => {
            use icy_parser_core::ScreenClearMode;

            // Mode QuickVt52Reset resets colors to default
            if mode == ScreenClearMode::QuickVt52Reset {
                if let GraphicsType::IGS(term) = buf.graphics_type() {
                    *buf.palette_mut() = term.get_palette().clone();
                }
            }

            match mode {
                ScreenClearMode::ClearAndHome
                | ScreenClearMode::ClearWholeScreen
                | ScreenClearMode::ClearWholeScreenAndHome
                | ScreenClearMode::QuickVt52Reset => buf.clear_screen(),
                ScreenClearMode::ClearCursorToBottom => buf.clear_buffer_down(),
                ScreenClearMode::ClearHomeToToCursor => {
                    // TODO: Implement clear from home to cursor
                }
            }
        }

        IgsCommand::SetResolution { resolution, palette } => {
            use icy_parser_core::PaletteMode;

            buf.set_graphics_type(GraphicsType::IGS(resolution));

            // Update executor's terminal resolution
            paint.set_resolution(resolution);

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

        IgsCommand::Pause { .. } => {
            // Pauses are typically ignored in non-real-time rendering, handled on viewer/terminal level
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

        IgsCommand::SetRandomRange { range_type } => {
            paint.random_bounds.update(&range_type);
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
                    let (x1, y1, x2, y2) = (
                        x1.evaluate(&paint.random_bounds, 0, 0),
                        y1.evaluate(&paint.random_bounds, 0, 0),
                        x2.evaluate(&paint.random_bounds, 0, 0),
                        y2.evaluate(&paint.random_bounds, 0, 0),
                    );

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

        IgsCommand::LoadFillPattern { pattern, data } => {
            paint.user_patterns[pattern as usize] = data;
        }

        IgsCommand::RotateColorRegisters { .. } => {
            log::info!("IGS RotateColorRegisters not implemented");
        }

        IgsCommand::LoadMidiBuffer { .. } => {
            log::info!("IGS LoadMidiBuffer not implemented");
        }

        IgsCommand::SetDrawtoBegin { x, y } => {
            let (x, y) = (x.evaluate(&paint.random_bounds, 0, 0), y.evaluate(&paint.random_bounds, 0, 0));
            paint.draw_to_position = (x, y).into();
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
            let pos = buf.caret_position();
            let new_pos = match direction {
                Direction::Up => crate::Position::new(pos.x, pos.y.saturating_sub(count)),
                Direction::Down => crate::Position::new(pos.x, pos.y + count),
                Direction::Left => crate::Position::new(pos.x.saturating_sub(count), pos.y),
                Direction::Right => crate::Position::new(pos.x + count, pos.y),
            };
            buf.set_caret_position(new_pos);
        }

        IgsCommand::PositionCursor { x, y } => {
            buf.caret_mut().set_position(crate::Position::new(
                x.evaluate(&paint.random_bounds, 0, 0),
                y.evaluate(&paint.random_bounds, 0, 0),
            ));
        }

        IgsCommand::RememberCursor { .. } => {
            *buf.saved_caret_pos() = buf.caret_position();
        }

        IgsCommand::InverseVideo { enabled } => {
            buf.terminal_state_mut().inverse_video = enabled;
        }

        IgsCommand::LineWrap { enabled } => {
            buf.terminal_state_mut().auto_wrap_mode = if enabled { AutoWrapMode::AutoWrap } else { AutoWrapMode::NoWrap };
        }

        // IGS-specific color commands (ESC b/c)
        IgsCommand::SetTextColor { layer, color } => {
            // SetTextColor uses direct palette indices, NOT color_map
            // This is for ANSI-style text rendering, not IGS graphics
            match layer {
                icy_parser_core::TextColorLayer::Foreground => {
                    buf.caret_mut().set_foreground(color as u32);
                }
                icy_parser_core::TextColorLayer::Background => {
                    buf.caret_mut().set_background(color as u32);
                }
            }
        }
    }
}

impl crate::PaletteScreenBuffer {
    pub(crate) fn handle_igs_command_impl(&mut self, cmd: IgsCommand) {
        // Initialize IGS state if not present
        if self.igs_state.is_none() {
            self.igs_state = Some(VdiPaint::default());
        }

        // Take ownership temporarily to avoid borrow conflicts
        let mut paint = self.igs_state.take().unwrap();
        run_igs_command(self, &mut paint, cmd);
        self.igs_state = Some(paint);

        self.mark_dirty();
    }
}
