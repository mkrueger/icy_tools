// This file contains the implementation of handle_rip_command
// It maps RipCommand enums to BGI function calls

use crate::{
    BitFont, EditableScreen, Position, Size,
    rip::bgi::{Bgi, ButtonStyle2, Direction, FillStyle as BgiFillStyle, FontType, LabelOrientation, LineStyle as BgiLineStyle, WriteMode as BgiWriteMode},
};
use icy_parser_core::RipCommand;
pub const RIP_SCREEN_SIZE: Size = Size { width: 640, height: 350 };

lazy_static::lazy_static! {
    pub static ref RIP_FONT : BitFont = BitFont::from_sauce_name("IBM VGA50").unwrap();
}

// Helper function that takes mutable references separately to avoid borrow checker issues
fn execute_rip_command(buf: &mut dyn EditableScreen, bgi: &mut Bgi, cmd: RipCommand) {
    match cmd {
        // Level 0 commands
        RipCommand::TextWindow { x0, y0, x1, y1, wrap, size } => {
            if x0 == 0 && y0 == 0 && x1 == 0 && y1 == 0 && size == 0 && !wrap {
                bgi.suspend_text = !bgi.suspend_text;
            }
            buf.terminal_state_mut().set_text_window(x0, y0, x1, y1);
            buf.caret_mut().set_font_page(size.clamp(0, 4) as usize);
            buf.caret_mut().set_position_xy(x0, y0);
        }

        RipCommand::ViewPort { x0, y0, x1, y1 } => {
            bgi.set_viewport(x0, y0, x1, y1);
        }

        RipCommand::ResetWindows => {
            buf.terminal_state_mut().clear_text_window();
            buf.clear_screen();
            buf.reset_terminal();
            bgi.graph_defaults(buf);
        }

        RipCommand::EraseWindow => {
            buf.terminal_state_mut().clear_text_window();
        }

        RipCommand::EraseView => {
            bgi.clear_viewport(buf);
        }

        RipCommand::GotoXY { x, y } => {
            bgi.move_to(x, y);
        }

        RipCommand::Home => {
            buf.home();
        }

        RipCommand::EraseEOL => {
            buf.clear_line_end();
        }

        RipCommand::Color { c } => {
            bgi.set_color(c as u8);
        }

        RipCommand::SetPalette { colors } => {
            bgi.set_palette(buf, &colors);
        }

        RipCommand::OnePalette { color, value } => {
            bgi.set_palette_color(buf, color, value as u8);
        }

        RipCommand::WriteMode { mode } => {
            bgi.set_write_mode(BgiWriteMode::from(mode as u8));
        }

        RipCommand::Move { x, y } => {
            bgi.move_to(x, y);
        }

        RipCommand::Text { text } => {
            bgi.out_text(buf, &text);
        }

        RipCommand::TextXY { x, y, text } => {
            bgi.out_text_xy(buf, x, y, &text);
        }

        RipCommand::FontStyle { font, direction, size, res: _ } => {
            bgi.set_text_style(FontType::from(font as u8), Direction::from(direction as u8), size);
        }

        RipCommand::Pixel { x, y } => {
            bgi.put_pixel(buf, x, y, bgi.get_color());
        }

        RipCommand::Line { x0, y0, x1, y1 } => {
            bgi.line(buf, x0, y0, x1, y1);
        }

        RipCommand::Rectangle { x0, y0, x1, y1 } => {
            bgi.rectangle(buf, x0, y0, x1, y1);
        }

        RipCommand::Bar { x0, y0, x1, y1 } => {
            let (left, right) = if x0 < x1 { (x0, x1) } else { (x1, x0) };
            let (top, bottom) = if y0 < y1 { (y0, y1) } else { (y1, y0) };
            bgi.bar(buf, left, top, right, bottom);
        }

        RipCommand::Circle { x_center, y_center, radius } => {
            bgi.circle(buf, x_center, y_center, radius);
        }

        RipCommand::Oval {
            x,
            y,
            st_ang,
            end_ang,
            x_rad,
            y_rad,
        } => {
            bgi.ellipse(buf, x, y, st_ang, end_ang, x_rad, y_rad);
        }

        RipCommand::FilledOval { x, y, x_rad, y_rad } => {
            bgi.fill_ellipse(buf, x, y, 0, 360, x_rad, y_rad);
        }

        RipCommand::Arc { x, y, st_ang, end_ang, radius } => {
            bgi.arc(buf, x, y, st_ang, end_ang, radius);
        }

        RipCommand::OvalArc {
            x,
            y,
            st_ang,
            end_ang,
            x_rad,
            y_rad,
        } => {
            bgi.ellipse(buf, x, y, st_ang, end_ang, x_rad, y_rad);
        }

        RipCommand::PieSlice { x, y, st_ang, end_ang, radius } => {
            bgi.pie_slice(buf, x, y, st_ang, end_ang, radius);
        }

        RipCommand::OvalPieSlice {
            x,
            y,
            st_ang,
            end_ang,
            x_rad,
            y_rad,
        } => {
            bgi.sector(buf, x, y, st_ang, end_ang, x_rad, y_rad);
        }

        RipCommand::Bezier {
            x1,
            y1,
            x2,
            y2,
            x3,
            y3,
            x4,
            y4,
            cnt,
        } => {
            bgi.rip_bezier(buf, x1, y1, x2, y2, x3, y3, x4, y4, cnt);
        }

        RipCommand::Polygon { points } => {
            if points.is_empty() {
                return;
            }
            let npoints = points[0] as usize;
            let mut poly_points = Vec::new();
            for i in 0..npoints {
                if i * 2 + 2 < points.len() {
                    poly_points.push(Position::new(points[i * 2 + 1], points[i * 2 + 2]));
                }
            }
            bgi.draw_poly(buf, &poly_points);
        }

        RipCommand::FilledPolygon { points } => {
            if points.is_empty() {
                return;
            }
            let npoints = points[0] as usize;
            let mut poly_points = Vec::new();
            for i in 0..npoints {
                if i * 2 + 2 < points.len() {
                    poly_points.push(Position::new(points[i * 2 + 1], points[i * 2 + 2]));
                }
            }
            bgi.fill_poly(buf, &poly_points);
        }

        RipCommand::PolyLine { points } => {
            if points.is_empty() {
                return;
            }
            let npoints = points[0] as usize;
            let mut poly_points = Vec::new();
            for i in 0..npoints {
                if i * 2 + 2 < points.len() {
                    poly_points.push(Position::new(points[i * 2 + 1], points[i * 2 + 2]));
                }
            }
            bgi.draw_poly_line(buf, &poly_points);
        }

        RipCommand::Fill { x, y, border } => {
            bgi.flood_fill(buf, x, y, border as u8);
        }

        RipCommand::LineStyle { style, user_pat, thick } => {
            bgi.set_line_style(BgiLineStyle::from(style as u8));
            if style == 4 {
                bgi.set_line_pattern(user_pat);
            }
            bgi.set_line_thickness(thick);
        }

        RipCommand::FillStyle { pattern, color } => {
            bgi.set_fill_style(BgiFillStyle::from(pattern as u8));
            bgi.set_fill_color(color as u8);
        }

        RipCommand::FillPattern {
            c1,
            c2,
            c3,
            c4,
            c5,
            c6,
            c7,
            c8,
            col,
        } => {
            let pattern = vec![c1 as u8, c2 as u8, c3 as u8, c4 as u8, c5 as u8, c6 as u8, c7 as u8, c8 as u8];
            bgi.set_user_fill_pattern(&pattern);
            bgi.set_fill_color(col as u8);
        }

        // Level 1 commands
        RipCommand::Mouse {
            num,
            x0,
            y0,
            x1,
            y1,
            clk,
            clr: _,
            res: _,
            text,
        } => {
            let host_command = if !text.is_empty() { Some(text) } else { None };
            bgi.add_button(
                buf,
                x0,
                y0,
                x1,
                y1,
                0,    // hotkey
                clk,  // flags
                None, // icon_file
                &format!("{}", num),
                host_command,
                false, // pressed
            );
        }

        RipCommand::MouseFields => {
            buf.clear_mouse_fields();
        }

        RipCommand::BeginText {
            x0: _,
            y0: _,
            x1: _,
            y1: _,
            res: _,
        } => {
            // BeginText not implemented in current BGI
        }

        RipCommand::RegionText {
            x: _,
            y: _,
            w: _,
            h: _,
            res: _,
        } => {
            // RegionText not implemented in current BGI
        }

        RipCommand::EndText => {
            // EndText not implemented in current BGI
        }

        RipCommand::GetImage { x0, y0, x1, y1, res: _ } => {
            bgi.rip_image = Some(bgi.get_image(buf, x0, y0, x1, y1));
        }

        RipCommand::PutImage { x, y, mode, res: _ } => {
            bgi.put_rip_image(buf, x, y, BgiWriteMode::from(mode as u8));
        }

        RipCommand::WriteIcon { res: _, data: _ } => {
            // WriteIcon not implemented in current BGI
        }

        RipCommand::LoadIcon {
            x: _,
            y: _,
            mode: _,
            clipboard: _,
            res: _,
            file_name: _,
        } => {
            // LoadIcon not implemented in current BGI
        }

        RipCommand::ButtonStyle {
            wid,
            hgt,
            orient,
            flags,
            bevsize,
            dfore,
            dback,
            bright,
            dark,
            surface,
            grp_no,
            flags2,
            uline_col,
            corner_col,
            res: _,
        } => {
            let style = ButtonStyle2 {
                size: Size::new(wid, hgt),
                orientation: LabelOrientation::from(orient as u8),
                bevel_size: bevsize,
                label_color: dfore,
                drop_shadow_color: dback,
                bright,
                dark,
                flags,
                flags2,
                surface_color: surface,
                group: grp_no,
                underline_color: uline_col,
                corner_color: corner_col,
            };
            bgi.set_button_style(style);
        }

        RipCommand::Button {
            x0,
            y0,
            x1,
            y1,
            hotkey,
            flags,
            res: _,
            text,
        } => {
            let split: Vec<&str> = text.split("<>").collect();

            if split.len() >= 2 {
                let icon_file = if split.len() >= 4 { Some(split[0]) } else { None };
                let label = split[if split.len() >= 4 { 1 } else { 0 }];
                let host_cmd = if split.len() >= 3 { Some(split[split.len() - 2].to_string()) } else { None };

                bgi.add_button(buf, x0, y0, x1, y1, hotkey as u8, flags, icon_file, label, host_cmd, false);
            }
        }

        RipCommand::Define { res: _, text: _ } => {
            // Macro definition - not implemented
        }

        RipCommand::Query { query: _ } => {
            // Query command - not implemented
        }

        RipCommand::CopyRegion {
            x0: _,
            y0: _,
            x1: _,
            y1: _,
            dest_x: _,
            dest_y: _,
            mode: _,
            res: _,
        } => {
            // CopyRegion not implemented in current BGI
        }

        RipCommand::ReadScene { file_name: _ } => {
            // ReadScene not implemented in current BGI
        }

        RipCommand::FileQuery { file_name: _ } => {
            // File query - not implemented
        }

        RipCommand::EnterBlockMode => {
            // Block mode - not implemented
        }

        RipCommand::TextVariable { text: _ } => {
            // Text variable - not implemented
        }

        RipCommand::NoMore => {
            // End of RIP commands
        }
    }
}

impl crate::PaletteScreenBuffer {
    pub(crate) fn handle_rip_command_impl(&mut self, cmd: RipCommand) {
        // Temporarily extract bgi to avoid borrow checker issues
        // SAFETY: We ensure that bgi and self don't overlap in memory access during the call
        let bgi_ptr = &mut self.bgi as *mut Bgi;
        unsafe {
            execute_rip_command(self, &mut *bgi_ptr, cmd);
        }
        self.mark_dirty();
    }
}
