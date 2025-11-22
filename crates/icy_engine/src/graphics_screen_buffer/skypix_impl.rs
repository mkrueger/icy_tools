use icy_parser_core::{DisplayMode, SkypixCommand};

use crate::{
    BitFont, EditableScreen, Palette, SKYPIX_PALETTE, Size, get_amiga_font_by_name,
    palette_screen_buffer::bgi::{Bgi, WriteMode},
};

lazy_static::lazy_static! {
    pub static ref SKYPIX_IBM_FONT : BitFont = BitFont::from_sauce_name("IBM VGA50").unwrap();
}

pub const SKYPIX_SCREEN_SIZE: Size = Size { width: 640, height: 200 };

fn execute_skypix_command(buf: &mut super::GraphicsScreenBuffer, bgi: &mut Bgi, cmd: SkypixCommand) {
    match cmd {
        SkypixCommand::SetPixel { x, y } => {
            bgi.put_pixel(buf, x, y, bgi.get_color());
        }

        SkypixCommand::DrawLine { x, y } => {
            bgi.line_to(buf, x, y);
        }

        SkypixCommand::AreaFill { mode: _, x, y } => {
            // TODO: mode parameter seems to be unused in original implementation
            bgi.flood_fill(buf, x, y, bgi.get_color());
        }

        SkypixCommand::RectangleFill { x1, y1, x2, y2 } => {
            bgi.bar(buf, x1, y1, x2, y2);
        }

        SkypixCommand::Ellipse { x, y, a, b } => {
            bgi.ellipse(buf, x, y, 0, 360, a, b);
        }

        SkypixCommand::GrabBrush { x1, y1, width, height } => {
            let x2 = x1 + width;
            let y2 = y1 + height;
            bgi.rip_image = Some(bgi.get_image(buf, x1, y1, x2, y2));
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
            // TODO: minterm and mask parameters seem to be unused in original
            if bgi.rip_image.is_some() {
                let brush = bgi.rip_image.take().unwrap();
                bgi.put_image2(buf, src_x, src_y, width, height, dst_x, dst_y, &brush, WriteMode::Copy);
                bgi.rip_image = Some(brush);
            }
        }

        SkypixCommand::MovePen { x, y } => {
            bgi.move_to(x, y);
        }

        SkypixCommand::PlaySample {
            speed: _,
            start: _,
            end: _,
            loops: _,
        } => {
            // Sound playback not implemented - original implementation also just logged
            log::info!("SKYPIX_PLAY_SAMPLE not implemented");
        }

        SkypixCommand::SetFont { size, name } => {
            if let Some(font) = get_amiga_font_by_name(&name, size) {
                buf.set_font(0, font);
            } else {
                log::warn!("SKYPIX_SET_FONT: Font '{}' of size {} not found, setting default.", name, size);
                buf.set_font(0, SKYPIX_IBM_FONT.clone());
            }
        }

        SkypixCommand::NewPalette { colors } => {
            if colors.len() >= 16 {
                let mut palette = Palette::new();
                for i in 0..16 {
                    let color_val = colors[i];
                    let r = (color_val & 0xF) as u8;
                    let g = ((color_val >> 4) & 0xF) as u8;
                    let b = ((color_val >> 8) & 0xF) as u8;

                    // Convert 4-bit Amiga color to 8-bit RGB
                    // Amiga uses 0-15 range, we expand to 0-255
                    let r8 = (r * 17) as u8; // 15 * 17 = 255
                    let g8 = (g * 17) as u8;
                    let b8 = (b * 17) as u8;

                    palette.set_color(i as u32, crate::Color::new(r8, g8, b8));
                }
                *buf.palette_mut() = palette;
            }
        }

        SkypixCommand::ResetPalette => {
            *buf.palette_mut() = Palette::from_slice(&SKYPIX_PALETTE);
        }

        SkypixCommand::FilledEllipse { x, y, a, b } => {
            bgi.fill_ellipse(buf, x, y, 0, 360, a, b);
        }

        SkypixCommand::Delay { .. } => {
            // Handled by the terminal
        }

        SkypixCommand::SetPenA { color } => {
            let col = color as u8;
            bgi.set_color(col);
            buf.caret_mut().set_foreground(col as u32);
        }

        SkypixCommand::CrcTransfer {
            mode: _,
            width: _,
            height: _,
            filename: _,
        } => {
            // XMODEM transfer needs to be handled at parser/protocol level
            log::warn!("SKYPIX_CRC_TRANSFER not implemented at BGI level");
        }

        SkypixCommand::SetDisplayMode { mode } => {
            // Display mode switching (3 vs 4 bitplanes)
            match mode {
                DisplayMode::EightColors => log::info!("Display mode: 8 colors (3 bitplanes)"),
                DisplayMode::SixteenColors => log::info!("Display mode: 16 colors (4 bitplanes)"),
            }
        }

        SkypixCommand::SetPenB { color } => {
            let col = color as u8;
            bgi.set_bk_color(col);
            buf.caret_mut().set_background(col as u32);
        }

        SkypixCommand::PositionCursor { x, y } => {
            buf.caret_mut().set_position((x, y).into());

            // Also update BGI pen position for graphics
            bgi.move_to(x, y);
        }

        SkypixCommand::ControllerReturn { c: _, x: _, y: _ } => {
            // Controller return (mouse/menu events) needs to be handled at higher level
            log::warn!("SKYPIX_CONTROLLER_RETURN not implemented");
        }

        SkypixCommand::DefineGadget {
            num: _,
            cmd: _,
            x1: _,
            y1: _,
            x2: _,
            y2: _,
        } => {
            // Gadget definition needs to be handled at UI level
            log::warn!("SKYPIX_DEFINE_GADGET not implemented");
        }
    }
}

impl super::GraphicsScreenBuffer {
    pub(crate) fn handle_skypix_command_impl(&mut self, cmd: SkypixCommand) {
        // Temporarily extract bgi to avoid borrow checker issues
        // SAFETY: We ensure that bgi and self don't overlap in memory access during the call
        let bgi_ptr = &mut self.bgi as *mut Bgi;
        unsafe {
            execute_skypix_command(self, &mut *bgi_ptr, cmd);
        }
        self.mark_dirty();
    }
}
