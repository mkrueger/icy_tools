use icy_parser_core::{DisplayMode, SkypixCommand};

use super::sky_paint::SkyPaint;
use crate::{BitFont, EditableScreen, Palette, SKYPIX_PALETTE, SKYPIX_PALETTE_8, Size, get_amiga_font_by_name};

/// Amiga 8x8 IBM compatible font for Skypix
pub const SKYPIX_DEFAULT_FONT_DATA: &str = include_str!("../../data/fonts/Amiga/Amiga8x8_IBM.yaff");

lazy_static::lazy_static! {
    pub static ref SKYPIX_DEFAULT_FONT: BitFont = BitFont::from_bytes("Amiga8x8_IBM", SKYPIX_DEFAULT_FONT_DATA.as_bytes()).unwrap();
}

pub const SKYPIX_SCREEN_SIZE: Size = Size { width: 640, height: 200 };

fn execute_skypix_command(buf: &mut super::AmigaScreenBuffer, paint: &mut SkyPaint, cmd: SkypixCommand) {
    match cmd {
        SkypixCommand::Comment { .. } => {
            // Command 0: Comments are ignored
        }

        SkypixCommand::SetPixel { x, y } => {
            paint.put_pixel(buf, x, y, paint.pen_a);
        }

        SkypixCommand::DrawLine { x, y } => {
            paint.line_to(buf, x, y);
        }

        SkypixCommand::AreaFill { mode, x, y } => {
            paint.flood_fill(buf, x, y, mode);
        }

        SkypixCommand::RectangleFill { x1, y1, x2, y2 } => {
            paint.bar(buf, x1, y1, x2, y2);
        }

        SkypixCommand::Ellipse { x, y, a, b } => {
            paint.ellipse(buf, x, y, a, b);
        }

        SkypixCommand::GrabBrush { x1, y1, width, height } => {
            let x2 = x1 + width;
            let y2 = y1 + height;
            paint.rip_image = Some(paint.get_image(buf, x1, y1, x2, y2));
        }

        SkypixCommand::UseBrush {
            src_x,
            src_y,
            dst_x,
            dst_y,
            width,
            height,
            minterm: _,
            mask: _,
        } => {
            if let Some(brush) = paint.rip_image.take() {
                paint.put_image2(buf, src_x, src_y, width, height, dst_x, dst_y, &brush);
                paint.rip_image = Some(brush);
            }
        }

        SkypixCommand::MovePen { x, y } => {
            paint.move_pen(x, y);
        }

        SkypixCommand::PlaySample { .. } => {
            log::info!("SKYPIX_PLAY_SAMPLE not implemented");
        }

        SkypixCommand::SetFont { size, name } => {
            if let Some(font) = get_amiga_font_by_name(&name, size) {
                buf.set_font(1, font);
                buf.caret_mut().set_font_page(1);
                buf.caret_mut().visible = false; // Hide text caret in JAM1 mode.
                // JAM1 mode: CR moves to beginning of next line, LF is ignored
                buf.terminal_state_mut().cr_is_if = true;
            } else {
                log::warn!("SKYPIX_SET_FONT: Font '{}' size {} not found", name, size);
                buf.caret_mut().set_font_page(0);
            }
        }
        SkypixCommand::ResetFont => {
            buf.caret_mut().set_font_page(0);
            buf.caret_mut().visible = true; // Show text in JAM2 mode.
            // JAM2 mode: Standard ANSI CR/LF behavior
            buf.terminal_state_mut().cr_is_if = false;
        }

        SkypixCommand::NewPalette { colors } => {
            if colors.len() >= 16 {
                let mut palette = Palette::new();
                for i in 0..16 {
                    let color_val = colors[i];
                    let r = ((color_val & 0xF) * 17) as u8;
                    let g = (((color_val >> 4) & 0xF) * 17) as u8;
                    let b = (((color_val >> 8) & 0xF) * 17) as u8;
                    palette.set_color(i as u32, crate::Color::new(r, g, b));
                }
                *buf.palette_mut() = palette;
            }
        }

        SkypixCommand::ResetPalette => {
            *buf.palette_mut() = Palette::from_slice(&SKYPIX_PALETTE);
        }

        SkypixCommand::FilledEllipse { x, y, a, b } => {
            paint.fill_ellipse(buf, x, y, a, b);
        }

        SkypixCommand::Delay { .. } => {
            // Handled at terminal level
        }

        SkypixCommand::SetPenA { color } => {
            let col = color as u8;
            paint.pen_a = col;
            paint.pen_a = col;
            buf.caret_mut().set_foreground(col as u32);
        }

        SkypixCommand::CrcTransfer { .. } => {
            // Handled at terminal level
        }

        SkypixCommand::SetDisplayMode { mode } => {
            match mode {
                DisplayMode::EightColors => {
                    *buf.palette_mut() = Palette::from_slice(&SKYPIX_PALETTE_8);
                }
                DisplayMode::SixteenColors => {
                    *buf.palette_mut() = Palette::from_slice(&SKYPIX_PALETTE);
                }
            }
        }

        SkypixCommand::SetPenB { color } => {
            let col = color as u8;
            paint.pen_b = col;
            buf.caret_mut().set_background(col as u32);
        }

        SkypixCommand::PositionCursor { x, y } => {
            buf.caret_mut().set_position((x, y).into());
            paint.move_pen(x, y);
        }

        SkypixCommand::ControllerReturn { .. } => {
            log::warn!("SKYPIX_CONTROLLER_RETURN not implemented");
        }

        SkypixCommand::DefineGadget { .. } => {
            log::warn!("SKYPIX_DEFINE_GADGET not implemented");
        }
    }
}

impl super::AmigaScreenBuffer {
    /*pub(crate) fn init_skypix(&mut self) {
        self.sky_paint.init_viewport(self.pixel_size.width, self.pixel_size.height);
    }*/

    pub(crate) fn handle_skypix_command_impl(&mut self, cmd: SkypixCommand) {
        // Use a raw pointer to avoid borrowing issues
        // Safety: We're splitting the borrow - sky_paint doesn't alias with the rest of self
        let paint_ptr = &mut self.sky_paint as *mut SkyPaint;
        let paint = unsafe { &mut *paint_ptr };

        execute_skypix_command(self, paint, cmd);

        self.mark_dirty();
    }
}
