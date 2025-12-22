use icy_parser_core::{DisplayMode, SkypixCommand};

use super::sky_paint::SkyPaint;
use crate::{BitFont, EditableScreen, Palette, SKYPIX_PALETTE, SKYPIX_PALETTE_8, Screen, Size, get_amiga_font_by_name};

/// Resets font page from a custom font (page 1) back to the default font (page 0).
/// Corrects the Y position so the caret stays at the baseline rather than the top
/// of the previous larger character cell.
///
/// When switching from a larger font to the default 8x8 font, the Y position needs
/// adjustment: y += old_font_height - base_font_height
fn reset_to_default_font(buf: &mut super::AmigaScreenBuffer) {
    let current_page = buf.caret().font_page();

    // Only adjust if we're on a custom font page
    if current_page != 0 {
        if let Some(current_font) = buf.font(current_page as usize) {
            let current_height = current_font.size().height;
            let default_height = buf.font_dimensions().height;

            let mut pos = buf.caret().position();
            pos.y += current_height - default_height;
            buf.caret_mut().set_position(pos);
        }
    }

    buf.caret_mut().set_font_page(0);
    buf.caret_mut().visible = true;
    buf.text_mode = super::TextMode::Jam2;
}

/// Switches to a custom font (page 1).
fn switch_to_custom_font(buf: &mut super::AmigaScreenBuffer, font: BitFont) {
    buf.set_font(1, font);
    buf.caret_mut().set_font_page(1);
    buf.caret_mut().visible = false; // Hide text caret in JAM1 mode.
}

/// Amiga 8x8 IBM compatible font for Skypix
pub const SKYPIX_DEFAULT_FONT_DATA: &str = include_str!("../../data/fonts/Amiga/Amiga8x8_IBM.yaff");

lazy_static::lazy_static! {
    pub static ref SKYPIX_DEFAULT_FONT: BitFont = BitFont::from_bytes("Amiga8x8_IBM", SKYPIX_DEFAULT_FONT_DATA.as_bytes()).unwrap();
}

pub const SKYPIX_SCREEN_SIZE: Size = Size { width: 640, height: 200 };

fn execute_skypix_command(buf: &mut super::AmigaScreenBuffer, paint: &mut SkyPaint, cmd: SkypixCommand) {
    // Get current pen colors from caret attribute
    let pen_a = buf.caret().attribute.foreground() as u8;
    buf.text_mode = super::TextMode::Jam1;
    match cmd {
        SkypixCommand::Comment { .. } => {
            // Command 0: Comments are ignored
        }

        SkypixCommand::SetPixel { x, y } => {
            paint.put_pixel(buf, x, y, pen_a);
        }

        SkypixCommand::DrawLine { x, y } => {
            paint.line_to(buf, x, y, pen_a);
        }

        SkypixCommand::AreaFill { mode, x, y } => {
            paint.flood_fill(buf, x, y, mode, pen_a);
        }

        SkypixCommand::RectangleFill { x1, y1, x2, y2 } => {
            paint.bar(buf, x1, y1, x2, y2, pen_a);
        }

        SkypixCommand::Ellipse { x, y, a, b } => {
            paint.ellipse(buf, x, y, a, b, pen_a);
        }

        SkypixCommand::GrabBrush { x1, y1, width, height } => {
            let x2 = x1 + width;
            let y2 = y1 + height;
            paint.rip_image = Some(paint.image(buf, x1, y1, x2, y2));
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
                switch_to_custom_font(buf, font);
            // JAM1 mode: CR moves to beginning of next line, LF is ignored
            } else {
                log::warn!("SKYPIX_SET_FONT: Font '{}' size {} not found", name, size);
                buf.caret_mut().set_font_page(0);
            }
        }
        SkypixCommand::ResetFont => {
            reset_to_default_font(buf);
        }

        SkypixCommand::NewPalette { colors } => {
            if colors.len() >= 16 {
                let mut palette: Palette = Palette::new();
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
            paint.fill_ellipse(buf, x, y, a, b, pen_a);
        }

        SkypixCommand::Delay { .. } => {
            // Handled at terminal level
        }

        SkypixCommand::SetPenA { color } => {
            buf.caret_mut().set_foreground(color as u32);
        }

        SkypixCommand::CrcTransfer { .. } => {
            // Handled at terminal level
        }

        SkypixCommand::SetDisplayMode { mode } => match mode {
            DisplayMode::EightColors => {
                *buf.palette_mut() = Palette::from_slice(&SKYPIX_PALETTE_8);
            }
            DisplayMode::SixteenColors => {
                *buf.palette_mut() = Palette::from_slice(&SKYPIX_PALETTE);
            }
        },

        SkypixCommand::SetPenB { color } => {
            buf.caret_mut().set_background(color as u32);
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

        SkypixCommand::EndSkypix => {
            *buf.palette_mut() = Palette::from_slice(&SKYPIX_PALETTE);

            buf.terminal_state.reset_terminal(buf.terminal_state.size());
            buf.caret.visible = true;
            buf.caret.shape = crate::CaretShape::Underline;
            buf.caret.set_foreground(buf.default_foreground_color());
            buf.caret.set_background(0);
            buf.caret.set_font_page(0);
            buf.text_mode = super::TextMode::Jam2;
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
    }
}
