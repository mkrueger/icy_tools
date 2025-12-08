// This file contains the implementation of handle_rip_command
// It maps RipCommand enums to BGI function calls

use std::{
    fs,
    io::{Cursor, Read},
    path,
};

use crate::{
    BitFont, EditableScreen, Position, Result, Size,
    bgi::{Bgi, ButtonStyle2, Direction, FontType, LabelOrientation, LineStyle as BgiLineStyle, MouseField, WriteMode as BgiWriteMode},
};
use byteorder::{LittleEndian, ReadBytesExt};
use icy_parser_core::{FillStyle, ImagePasteMode, LineStyle, RipCommand, WriteMode as RipWriteMode};
pub const RIP_SCREEN_SIZE: Size = Size { width: 640, height: 350 };
pub static RIP_TERMINAL_ID: &str = "RIPSCRIP015410\0";

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
                buf.terminal_state_mut().reset_text_window();
                buf.set_caret_position((0, 0).into());
                return;
            }
            buf.terminal_state_mut().set_text_window(x0.into(), y0.into(), x1.into(), y1.into());
            buf.caret_mut().set_font_page(size.clamp(0, 4) as usize);
            buf.set_caret_position((x0 as i32, y0 as i32).into());
        }

        RipCommand::ViewPort { x0, y0, x1, y1 } => {
            bgi.set_viewport(x0.into(), y0.into(), x1.into(), y1.into());
        }

        RipCommand::ResetWindows => {
            buf.terminal_state_mut().reset_text_window();
            buf.clear_screen();
            buf.reset_terminal();
            bgi.graph_defaults(buf);
        }

        RipCommand::EraseWindow => {
            buf.terminal_state_mut().reset_text_window();
        }

        RipCommand::EraseView => {
            bgi.clear_viewport(buf);
        }

        RipCommand::GotoXY { x, y } => {
            bgi.move_to(x.into(), y.into());
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
            let colors_i32: Vec<i32> = colors.iter().map(|&c| c.into()).collect();
            bgi.set_palette(buf, &colors_i32);
        }

        RipCommand::OnePalette { color, value } => {
            bgi.set_palette_color(buf, color.into(), value as u8);
        }

        RipCommand::WriteMode { mode } => {
            let bgi_mode = match mode {
                RipWriteMode::Normal => BgiWriteMode::Copy,
                RipWriteMode::Xor => BgiWriteMode::Xor,
            };
            bgi.set_write_mode(bgi_mode);
        }

        RipCommand::Move { x, y } => {
            bgi.move_to(x.into(), y.into());
        }

        RipCommand::Text { text } => {
            bgi.out_text(buf, &text);
        }

        RipCommand::TextXY { x, y, text } => {
            bgi.out_text_xy(buf, x.into(), y.into(), &text);
        }

        RipCommand::FontStyle { font, direction, size, res: _ } => {
            bgi.set_text_style(FontType::from(font as u8), Direction::from(direction as u8), size.into());
        }

        RipCommand::Pixel { x, y } => {
            bgi.put_pixel(buf, x.into(), y.into(), bgi.get_color());
        }

        RipCommand::Line { x0, y0, x1, y1 } => {
            bgi.line(buf, x0.into(), y0.into(), x1.into(), y1.into());
        }

        RipCommand::Rectangle { x0, y0, x1, y1 } => {
            bgi.rectangle(buf, x0.into(), y0.into(), x1.into(), y1.into());
        }

        RipCommand::Bar { x0, y0, x1, y1 } => {
            let (left, right) = if x0 < x1 { (x0, x1) } else { (x1, x0) };
            let (top, bottom) = if y0 < y1 { (y0, y1) } else { (y1, y0) };
            bgi.bar(buf, left.into(), top.into(), right.into(), bottom.into());
        }

        RipCommand::Circle { x_center, y_center, radius } => {
            bgi.circle(buf, x_center.into(), y_center.into(), radius.into());
        }

        RipCommand::Oval {
            x,
            y,
            st_ang,
            end_ang,
            x_rad,
            y_rad,
        } => {
            bgi.ellipse(buf, x.into(), y.into(), st_ang.into(), end_ang.into(), x_rad.into(), y_rad.into());
        }

        RipCommand::FilledOval { x, y, x_rad, y_rad } => {
            bgi.fill_ellipse(buf, x.into(), y.into(), 0, 360, x_rad.into(), y_rad.into());
        }

        RipCommand::Arc { x, y, st_ang, end_ang, radius } => {
            bgi.arc(buf, x.into(), y.into(), st_ang.into(), end_ang.into(), radius.into());
        }

        RipCommand::OvalArc {
            x,
            y,
            st_ang,
            end_ang,
            x_rad,
            y_rad,
        } => {
            bgi.ellipse(buf, x.into(), y.into(), st_ang.into(), end_ang.into(), x_rad.into(), y_rad.into());
        }

        RipCommand::PieSlice { x, y, st_ang, end_ang, radius } => {
            bgi.pie_slice(buf, x.into(), y.into(), st_ang.into(), end_ang.into(), radius.into());
        }

        RipCommand::OvalPieSlice {
            x,
            y,
            st_ang,
            end_ang,
            x_rad,
            y_rad,
        } => {
            bgi.sector(buf, x.into(), y.into(), st_ang.into(), end_ang.into(), x_rad.into(), y_rad.into());
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
            bgi.rip_bezier(
                buf,
                x1.into(),
                y1.into(),
                x2.into(),
                y2.into(),
                x3.into(),
                y3.into(),
                x4.into(),
                y4.into(),
                cnt.into(),
            );
        }

        RipCommand::Polygon { points } => {
            if points.is_empty() {
                return;
            }
            let mut poly_points = Vec::new();
            for i in 0..points.len() / 2 {
                poly_points.push(Position::new(points[i * 2].into(), points[i * 2 + 1].into()));
            }
            bgi.draw_poly(buf, &poly_points);
        }

        RipCommand::FilledPolygon { points } => {
            if points.is_empty() {
                return;
            }
            let mut poly_points = Vec::new();
            for i in 0..points.len() / 2 {
                poly_points.push(Position::new(points[i * 2].into(), points[i * 2 + 1].into()));
            }
            bgi.fill_poly(buf, &poly_points);
        }

        RipCommand::PolyLine { points } => {
            if points.is_empty() {
                return;
            }
            let mut poly_points = Vec::new();
            for i in 0..points.len() / 2 {
                poly_points.push(Position::new(points[i * 2].into(), points[i * 2 + 1].into()));
            }
            bgi.draw_poly_line(buf, &poly_points);
        }

        RipCommand::Fill { x, y, border } => {
            bgi.flood_fill(buf, x.into(), y.into(), border as u8);
        }

        RipCommand::LineStyle { style, user_pat, thick } => {
            bgi.set_line_style(BgiLineStyle::from(style as u8));
            if style == LineStyle::User {
                bgi.set_line_pattern(user_pat.into());
            }
            bgi.set_line_thickness(thick.into());
        }

        RipCommand::FillStyle { pattern, color } => {
            bgi.set_fill_style(pattern);
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
            bgi.set_fill_style(FillStyle::User);
            bgi.set_fill_color(col as u8);
        }

        // Level 1 commands
        RipCommand::Mouse {
            num: _,
            x0,
            y0,
            x1,
            y1,
            clk: _,
            clr: _,
            res: _,
            text,
        } => {
            let host_command = parse_host_command(&text);
            let mut style = ButtonStyle2::default();
            style.flags |= 1024;
            buf.add_mouse_field(MouseField::new(x0.into(), y0.into(), x1.into(), y1.into(), host_command, style));
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

        RipCommand::RegionText { .. } => {
            // RegionText not implemented in current BGI
        }

        RipCommand::EndText => {
            // EndText not implemented in current BGI
        }

        RipCommand::GetImage { x0, y0, x1, y1, res: _ } => {
            bgi.rip_image = Some(bgi.get_image(buf, x0.into(), y0.into(), x1.into(), y1.into()));
        }

        RipCommand::PutImage { x, y, mode, res: _ } => {
            let bgi_mode = match mode {
                ImagePasteMode::Copy => BgiWriteMode::Copy,
                ImagePasteMode::Xor => BgiWriteMode::Xor,
                ImagePasteMode::Or => BgiWriteMode::Or,
                ImagePasteMode::And => BgiWriteMode::And,
                ImagePasteMode::Not => BgiWriteMode::Not,
            };
            bgi.put_rip_image(buf, x.into(), y.into(), bgi_mode);
        }

        RipCommand::WriteIcon { res: _, data: _ } => {
            // WriteIcon not implemented in current BGI
        }

        RipCommand::LoadIcon {
            x,
            y,
            mode,
            clipboard: _,
            res: _,
            file_name,
        } => {
            let Ok(file_name) = lookup_cache_file(bgi, &file_name) else {
                return;
            };
            if !file_name.exists() {
                log::error!("File not found: {}", file_name.display());
                return;
            }
            let Ok(mut file) = std::fs::File::open(file_name) else {
                return;
            };
            let mut file_buf = Vec::new();
            let _ = file.read_to_end(&mut file_buf);

            let _len = file_buf.len();
            let mut br = Cursor::new(file_buf);

            let width = br.read_u16::<LittleEndian>().unwrap() as i32 + 1;
            let height = br.read_u16::<LittleEndian>().unwrap() as i32 + 1;

            // let _tmp = br.read_u16::<LittleEndian>()? + 1;

            /*
            00    Paste the image on-screen normally                   (COPY)
            01    Exclusive-OR  image with the one already on screen   (XOR)
            02    Logically OR  image with the one already on screen   (OR)
            03    Logically AND image with the one already on screen   (AND)
            04    Paste the inverse of the image on the screen         (NOT)
            */
            let bgi_mode = match mode {
                ImagePasteMode::Copy => BgiWriteMode::Copy,
                ImagePasteMode::Xor => BgiWriteMode::Xor,
                ImagePasteMode::Or => BgiWriteMode::Or,
                ImagePasteMode::And => BgiWriteMode::And,
                ImagePasteMode::Not => BgiWriteMode::Not,
            };
            bgi.set_write_mode(bgi_mode);
            let res = buf.get_resolution();

            for y2 in 0..height {
                if i32::from(y) + y2 >= res.height {
                    break;
                }
                let row = (width / 8 + i32::from((width & 7) != 0)) as usize;
                let mut planes = vec![0u8; row * 4];
                let _ = br.read_exact(&mut planes);

                for x2 in 0..width as usize {
                    if i32::from(x) + x2 as i32 >= res.width {
                        break;
                    }
                    let bit = 7 - (x2 & 7);
                    let mut color = (planes[x2 / 8] >> bit) & 1;
                    color |= ((planes[row + (x2 / 8)] >> bit) & 1) << 1;
                    color |= ((planes[(row * 2) + (x2 / 8)] >> bit) & 1) << 2;
                    color |= ((planes[(row * 3) + (x2 / 8)] >> bit) & 1) << 3;
                    bgi.put_pixel(buf, i32::from(x) + x2 as i32, i32::from(y) + y2, color);
                }
            }
            // Restore original write mode
            bgi.set_write_mode(bgi_mode);
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
                size: Size::new(wid.into(), hgt.into()),
                orientation: LabelOrientation::from(orient as u8),
                bevel_size: bevsize.into(),
                label_color: dfore.into(),
                drop_shadow_color: dback.into(),
                bright: bright.into(),
                dark: dark.into(),
                flags: flags.into(),
                flags2: flags2.into(),
                surface_color: surface.into(),
                group: grp_no.into(),
                underline_color: uline_col.into(),
                corner_color: corner_col.into(),
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
            if split.len() == 4 {
                bgi.add_button(
                    buf,
                    x0.into(),
                    y0.into(),
                    x1.into(),
                    y1.into(),
                    hotkey as u8,
                    flags.into(),
                    Some(split[0]),
                    split[1],
                    parse_host_command(split[2]),
                    false,
                );
            } else if split.len() == 3 {
                bgi.add_button(
                    buf,
                    x0.into(),
                    y0.into(),
                    x1.into(),
                    y1.into(),
                    hotkey as u8,
                    flags.into(),
                    None,
                    split[1],
                    parse_host_command(split[2]),
                    false,
                );
            } else if split.len() == 2 {
                bgi.add_button(
                    buf,
                    x0.into(),
                    y0.into(),
                    x1.into(),
                    y1.into(),
                    hotkey as u8,
                    flags.into(),
                    None,
                    split[1],
                    None,
                    false,
                );
            } else {
                bgi.add_button(
                    buf,
                    x0.into(),
                    y0.into(),
                    x1.into(),
                    y1.into(),
                    hotkey as u8,
                    flags.into(),
                    None,
                    &format!("error in text {}", split.len()),
                    None,
                    false,
                );
            }
        }

        RipCommand::Define { .. } => {
            // Macro definition - not implemented
        }

        RipCommand::Query { .. } => {
            // Query command - not implemented
        }

        RipCommand::CopyRegion {
            x0,
            y0,
            x1,
            y1,
            res: _,
            dest_line,
        } => {
            let image = bgi.get_image(buf, x0.into(), y0.into(), (x1 + 1).into(), (y1 + 1).into());
            bgi.put_image(buf, x0.into(), dest_line.into(), &image, bgi.get_write_mode());
        }

        RipCommand::ReadScene { file_name: _ } => {
            // ReadScene not implemented in current BGI
        }

        RipCommand::FileQuery { mode: _, res: _, file_name: _ } => {
            // File query - should be hadled by the terminal sink
        }

        RipCommand::EnterBlockMode { .. } => {
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

fn parse_host_command(split: &str) -> Option<String> {
    if split.is_empty() {
        return None;
    }
    let mut res = String::new();
    let mut got_caret = false;
    for c in split.chars() {
        if got_caret {
            match c {
                // Null (ASCII 0)
                '@' => res.push('\x00'),
                // Beep
                'G' => res.push('\x07'),
                // Clear Screen (Top of Form)
                'L' => res.push('\x0C'),
                // Carriage Return
                'M' => res.push('\x0D'),
                // Break (sometimes)
                'C' => res.push('\x18'),
                // Backspace
                'H' => res.push('\x08'),
                // Escape character
                '[' => res.push('\x1B'),
                // Pause data transmission
                'S' => res.push('\x13'), // XOFF
                // Resume data transmission
                'Q' => res.push('\x11'), // XON
                _ => {
                    log::error!("Invalid character after ^ in button command: {}", c);
                }
            }
            got_caret = false;
            continue;
        }
        if c == '^' {
            got_caret = true;
            continue;
        }
        res.push(c);
    }
    Some(res)
}

fn lookup_cache_file(bgi: &mut Bgi, search_file: &str) -> Result<path::PathBuf> {
    let mut search_file = search_file.to_uppercase();
    let has_extension = search_file.contains('.');
    if !has_extension {
        search_file.push('.');
    }

    for path in fs::read_dir(&bgi.file_path)?.flatten() {
        if let Some(file_name) = path.file_name().to_str() {
            if has_extension && file_name.to_uppercase() == search_file {
                return Ok(path.path());
            }
            if !has_extension && file_name.to_uppercase().starts_with(&search_file) {
                return Ok(path.path());
            }
        }
    }
    Ok(bgi.file_path.join(&search_file))
}
