#![allow(clippy::float_cmp)]
use std::cmp::max;

use egui::epaint::ahash::HashMap;
use egui::Vec2;
use glow::HasContext as _;
use icy_engine::editor::EditState;
use icy_engine::Buffer;
use icy_engine::Size;
use icy_engine::TextAttribute;
use icy_engine::TextPane;
use image::EncodableLayout;
use image::RgbaImage;
use web_time::Instant;

use crate::TerminalCalc;
use crate::TerminalOptions;

use super::Blink;
use super::BufferView;

const FONT_TEXTURE_SLOT: u32 = 8;
const BUFFER_TEXTURE_SLOT: u32 = 10;
const REFERENCE_IMAGE_TEXTURE_SLOT: u32 = 12;

pub struct TerminalRenderer {
    terminal_shader: glow::Program,

    font_lookup_table: HashMap<usize, usize>,

    terminal_render_texture: glow::Texture,
    font_texture: glow::Texture,
    vertex_array: glow::VertexArray,

    old_palette_checksum: u32,

    redraw_view: bool,
    redraw_font: bool,

    last_scroll_position: Vec2,
    last_char_size: Vec2,
    last_buffer_rect_size: Vec2,

    caret_blink: Blink,
    character_blink: Blink,

    start_time: Instant,

    reference_image_texture: glow::Texture,
    pub reference_image: Option<RgbaImage>,
    pub load_reference_image: bool,
    pub show_reference_image: bool,
    pub igs_executor: Option<(icy_engine::Size, Vec<u8>)>,
    pub color_image: Option<(Size, Vec<u8>)>,
    pub color_image_upated: bool,
}

impl TerminalRenderer {
    pub fn new(gl: &glow::Context) -> Self {
        unsafe {
            let reference_image_texture = create_reference_image_texture(gl);
            let font_texture = create_font_texture(gl);
            let terminal_render_texture = create_buffer_texture(gl);
            let terminal_shader = compile_shader(gl);

            let vertex_array = gl.create_vertex_array().expect("Cannot create vertex array");

            Self {
                terminal_shader,
                font_lookup_table: HashMap::default(),
                old_palette_checksum: 0,

                terminal_render_texture,
                font_texture,
                reference_image: None,
                load_reference_image: false,
                show_reference_image: false,
                redraw_view: true,
                redraw_font: true,
                vertex_array,
                caret_blink: Blink::new((1000.0 / 1.875) as u128 / 2),
                character_blink: Blink::new((1000.0 / 1.8) as u128),
                reference_image_texture,
                start_time: Instant::now(),
                last_scroll_position: Vec2::ZERO,
                last_char_size: Vec2::ZERO,
                last_buffer_rect_size: Vec2::ZERO,
                igs_executor: None,
                color_image: None,
                color_image_upated: false,
            }
        }
    }

    pub(crate) fn destroy(&self, gl: &glow::Context) {
        unsafe {
            gl.delete_vertex_array(self.vertex_array);

            gl.delete_program(self.terminal_shader);

            gl.delete_texture(self.terminal_render_texture);
            gl.delete_texture(self.font_texture);
            gl.delete_texture(self.reference_image_texture);
        }
    }

    pub fn redraw_terminal(&mut self) {
        self.redraw_view = true;
    }

    pub fn redraw_font(&mut self) {
        self.redraw_font = true;
    }

    pub fn update_textures(&mut self, gl: &glow::Context, edit_state: &mut EditState, calc: &TerminalCalc, use_fg: bool, use_bg: bool) {
        self.check_blink_timers();

        if self.redraw_font || edit_state.get_buffer().is_font_table_updated() {
            self.redraw_font = false;
            edit_state.get_buffer_mut().set_font_table_is_updated();
            self.update_font_texture(gl, edit_state.get_buffer());
        }
        if self.old_palette_checksum != edit_state.get_buffer_mut().palette.get_checksum() || edit_state.is_palette_dirty {
            self.old_palette_checksum = edit_state.get_buffer_mut().palette.get_checksum();
            self.redraw_terminal();
        }

        if self.redraw_view
            || calc.char_scroll_position != self.last_scroll_position
            || calc.char_size != self.last_char_size
            || calc.buffer_rect.size() != self.last_buffer_rect_size
            || edit_state.is_buffer_dirty()
        {
            self.last_scroll_position = calc.char_scroll_position;
            self.last_char_size = calc.char_size;
            self.last_buffer_rect_size = calc.buffer_rect.size();
            edit_state.set_buffer_clean();
            self.redraw_view = false;
            self.update_terminal_texture(gl, edit_state, calc, use_fg, use_bg);
        }

        if self.load_reference_image {
            if let Some(image) = &self.reference_image {
                self.update_reference_image_texture(gl, image);
            }
            self.load_reference_image = false;
        }

        if self.igs_executor.is_some() {
            self.update_igs_texture(gl);
        }

        if self.color_image_upated {
            if let Some((a, b)) = &self.color_image {
                self.update_color_image_texture(gl, *a, b);
            }
        }
    }

    fn check_blink_timers(&mut self) {
        let cur_ms = self.start_time.elapsed().as_millis();
        self.caret_blink.update(cur_ms);
        self.character_blink.update(cur_ms);
    }

    fn update_font_texture(&mut self, gl: &glow::Context, buf: &Buffer) {
        let size = if let Some(font) = buf.get_font(0) {
            font.size
        } else {
            log::error!("Error buffer doesn't have a font");
            return;
        };
        let w_ext = if buf.use_letter_spacing() { 1 } else { 0 };
        let w = size.width;
        let h = size.height;

        let mut font_data = Vec::new();
        let chars_in_line = 16;
        let width = (w + w_ext) * chars_in_line;
        let height = h * 256 / chars_in_line;
        let line_width = width * 4;
        self.font_lookup_table.clear();
        font_data.resize((line_width * height) as usize * buf.font_count(), 0);
        for (cur_font_num, font) in buf.font_iter().enumerate() {
            self.font_lookup_table.insert(*font.0, cur_font_num);
            let fontpage_start = cur_font_num as i32 * (line_width * height);
            for ch in 0..256 {
                let cur_font = font.1;
                if ch >= cur_font.length {
                    break;
                }
                let glyph = cur_font.get_glyph(unsafe { char::from_u32_unchecked(ch as u32) }).unwrap();

                let x = ch % chars_in_line;
                let y = ch / chars_in_line;

                let offset = x * (w + w_ext) * 4 + y * h * line_width + fontpage_start;
                let last_scan_line = h.min(cur_font.size.height);
                for y in 0..last_scan_line {
                    if let Some(scan_line) = glyph.data.get(y as usize) {
                        let mut po = (offset + y * line_width) as usize;

                        for x in 0..w {
                            if scan_line & (128 >> x) == 0 {
                                po += 4;
                            } else {
                                // unroll
                                font_data[po] = 0xFF;
                                po += 1;
                                font_data[po] = 0xFF;
                                po += 1;
                                font_data[po] = 0xFF;
                                po += 1;
                                font_data[po] = 0xFF;
                                po += 1;
                            }
                        }
                        if buf.use_letter_spacing() && (0xC0..=0xDF).contains(&ch) && !(0xB0..=0xBF).contains(&ch) && (scan_line & 1) != 0 {
                            // unroll
                            font_data[po] = 0xFF;
                            po += 1;
                            font_data[po] = 0xFF;
                            po += 1;
                            font_data[po] = 0xFF;
                            po += 1;
                            font_data[po] = 0xFF;
                        }
                    } else {
                        log::error!("error in font {} can't get line {y}", font.0);
                        font_data.extend(vec![0xFF; ((w + w_ext) as usize) * 4]);
                    }
                }
            }
        }

        unsafe {
            gl.delete_texture(self.font_texture);
            self.font_texture = create_font_texture(gl);

            gl.bind_texture(glow::TEXTURE_2D_ARRAY, Some(self.font_texture));
            gl.tex_image_3d(
                glow::TEXTURE_2D_ARRAY,
                0,
                glow::RGBA as i32,
                width,
                height,
                buf.font_count() as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                Some(&font_data),
            );
            crate::check_gl_error!(gl, "update_font_texture");
        }
    }

    fn update_reference_image_texture(&self, gl: &glow::Context, image: &RgbaImage) {
        unsafe {
            gl.bind_texture(glow::TEXTURE_2D, Some(self.reference_image_texture));
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA as i32,
                image.width() as i32,
                image.height() as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                Some(image.as_bytes()),
            );
            crate::check_gl_error!(gl, "update_reference_image_texture");
        }
    }

    fn update_color_image_texture(&self, gl: &glow::Context, size: Size, pixels: &[u8]) {
        unsafe {
            if pixels.len() != (size.width * size.height * 4) as usize {
                log::error!("Error in update_color_image_texture, wrong si0ze");
                return;
            }
            gl.bind_texture(glow::TEXTURE_2D, Some(self.reference_image_texture));

            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA as i32,
                size.width,
                size.height,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                Some(pixels),
            );
            crate::check_gl_error!(gl, "update_reference_image_texture");
        }
    }

    fn update_igs_texture(&self, gl: &glow::Context) {
        let Some((size, igs)) = &self.igs_executor else {
            return;
        };
        unsafe {
            gl.bind_texture(glow::TEXTURE_2D, Some(self.reference_image_texture));
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA as i32,
                size.width,
                size.height,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                Some(igs),
            );
            crate::check_gl_error!(gl, "update_reference_image_texture");
        }
    }

    fn update_terminal_texture(&self, gl: &glow::Context, edit_state: &EditState, calc: &TerminalCalc, use_fg: bool, use_bg: bool) {
        let buf = edit_state.get_buffer();
        let first_column = (calc.viewport_top().x / calc.char_size.x) as i32;
        let first_row = (calc.viewport_top().y / calc.char_size.y) as i32;
        let real_height = calc.real_height;
        let buf_w = calc.forced_width;
        let buf_h = calc.forced_height;

        let max_lines = max(0, real_height - buf_h);
        let scroll_back_line = max(0, max_lines - first_row);
        let first_line = 0.max(real_height.saturating_sub(calc.forced_height));
        let mut buffer_data = Vec::with_capacity((2 * (buf_w + 1) * 4 * buf_h) as usize);
        let mut y: i32 = 0;

        while y <= buf_h {
            let mut is_double_height = false;
            let cur_idx = buffer_data.len();
            for x in 0..=buf_w {
                let mut ch = if let Some(window) = &buf.terminal_state.text_window {
                    buf.get_char((first_column + x - window.left(), first_line - scroll_back_line + y - window.top()))
                } else {
                    buf.get_char((first_column + x, first_line - scroll_back_line + y))
                };
                if ch.attribute.is_double_height() {
                    is_double_height = true;
                }
                if ch.attribute.is_concealed() {
                    buffer_data.push(b' ');
                } else {
                    buffer_data.push(ch.ch as u8);
                }
                if !use_fg {
                    ch.attribute.set_foreground(7);
                    ch.attribute.set_is_bold(false);
                }
                let fg: u32 = if ch.attribute.is_bold() && ch.attribute.get_foreground() < 8 {
                    ch.attribute.get_foreground() + 8
                } else {
                    ch.attribute.get_foreground()
                };

                let (r, g, b) = buf.palette.get_rgb(fg);
                buffer_data.push(r);
                buffer_data.push(g);
                buffer_data.push(b);
            }

            if is_double_height {
                let double_line_start = buffer_data.len();
                buffer_data.extend_from_within(cur_idx..buffer_data.len());
                // clear all chars that are not double height.
                for x in 0..=buf_w {
                    let ch = buf.get_char((first_column + x, first_line - scroll_back_line + y));
                    if !ch.attribute.is_double_height() {
                        buffer_data[double_line_start + x as usize * 4] = b' ';
                    }
                }
            }

            if is_double_height {
                y += 2;
            } else {
                y += 1;
            }
        }

        // additional attributes
        y = 0;
        while y <= buf_h {
            let mut is_double_height = false;
            let cur_idx = buffer_data.len();

            for x in 0..=buf_w {
                let ch = buf.get_char((first_column + x, first_line - scroll_back_line + y));
                let is_selected = edit_state.get_is_mask_selected((first_column + x, first_line - scroll_back_line + y));
                let is_tool_overlay = edit_state
                    .get_tool_overlay_mask()
                    .get_is_selected((first_column + x, first_line - scroll_back_line + y));

                let mut attr = if ch.attribute.is_double_underlined() {
                    3
                } else {
                    u8::from(ch.attribute.is_underlined())
                };
                if ch.attribute.is_crossed_out() {
                    attr |= 4;
                }

                if ch.attribute.is_double_height() {
                    is_double_height = true;
                    attr |= 8;
                }

                buffer_data.push(attr);

                if buf.has_fonts() {
                    if let Some(font_number) = self.font_lookup_table.get(&ch.get_font_page()) {
                        buffer_data.push(*font_number as u8);
                    } else {
                        buffer_data.push(0);
                    }
                } else {
                    buffer_data.push(0);
                }

                let mut preview_flag = 0;
                if is_selected {
                    preview_flag |= 1;
                }
                if is_tool_overlay {
                    preview_flag |= 2;
                }
                buffer_data.push(preview_flag);
                if !ch.is_visible() {
                    buffer_data.push(128);
                } else {
                    buffer_data.push(if ch.attribute.is_blinking() { 255 } else { 0 });
                }
            }

            if is_double_height {
                let double_line_start = buffer_data.len();
                buffer_data.extend_from_within(cur_idx..buffer_data.len());
                for x in 0..=buf_w {
                    buffer_data[double_line_start + x as usize * 4] |= 16;
                }
            }

            if is_double_height {
                y += 2;
            } else {
                y += 1;
            }
        }

        // bg color.
        y = 0;
        while y <= buf_h {
            let mut is_double_height = false;
            let cur_idx = buffer_data.len();

            for x in 0..=buf_w {
                let mut ch = buf.get_char((first_column + x, first_line - scroll_back_line + y));
                if !use_bg {
                    ch.attribute.set_background(0);
                }
                if ch.attribute.is_double_height() {
                    is_double_height = true;
                }
                let (r, g, b) = buf.palette.get_rgb(ch.attribute.get_background());
                buffer_data.push(r);
                buffer_data.push(g);
                buffer_data.push(b);
                let color = if ch.attribute.get_foreground() == TextAttribute::TRANSPARENT_COLOR {
                    0
                } else if ch.attribute.get_background() == TextAttribute::TRANSPARENT_COLOR {
                    8
                } else {
                    255
                };
                buffer_data.push(color);
            }

            if is_double_height {
                buffer_data.extend_from_within(cur_idx..buffer_data.len());
            }

            if is_double_height {
                y += 2;
            } else {
                y += 1;
            }
        }

        unsafe {
            gl.bind_texture(glow::TEXTURE_2D_ARRAY, Some(self.terminal_render_texture));
            gl.tex_image_3d(
                glow::TEXTURE_2D_ARRAY,
                0,
                glow::RGBA as i32,
                buf_w + 1,
                buf_h + 1,
                3,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                Some(&buffer_data),
            );
            crate::check_gl_error!(gl, "update_terminal_texture");
        }
    }

    pub(crate) fn render_terminal(
        &self,
        gl: &glow::Context,
        view_state: &BufferView,
        render_buffer_size: Vec2,
        terminal_options: &TerminalOptions,
        has_focus: bool,
    ) {
        unsafe {
            gl.active_texture(glow::TEXTURE0 + FONT_TEXTURE_SLOT);
            gl.bind_texture(glow::TEXTURE_2D_ARRAY, Some(self.font_texture));

            gl.active_texture(glow::TEXTURE0 + BUFFER_TEXTURE_SLOT);
            gl.bind_texture(glow::TEXTURE_2D_ARRAY, Some(self.terminal_render_texture));

            gl.active_texture(glow::TEXTURE0 + REFERENCE_IMAGE_TEXTURE_SLOT);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.reference_image_texture));
            crate::check_gl_error!(gl, "render_terminal_bind_textures");

            self.run_shader(gl, view_state, render_buffer_size, terminal_options, has_focus);

            gl.bind_vertex_array(Some(self.vertex_array));
            gl.draw_arrays(glow::TRIANGLES, 0, 6);
            crate::check_gl_error!(gl, "render_terminal_end");
        }
    }

    unsafe fn run_shader(
        &self,
        gl: &glow::Context,
        buffer_view: &BufferView,
        render_buffer_size: egui::Vec2,
        terminal_options: &TerminalOptions,
        has_focus: bool,
    ) {
        let fontdim = buffer_view.get_buffer().get_font_dimensions();
        let font_height = fontdim.height as f32;
        let font_width = fontdim.width as f32 + if buffer_view.get_buffer().use_letter_spacing() { 1.0 } else { 0.0 };

        gl.use_program(Some(self.terminal_shader));
        gl.uniform_2_f32(
            gl.get_uniform_location(self.terminal_shader, "u_resolution").as_ref(),
            render_buffer_size.x,
            render_buffer_size.y,
        );

        gl.uniform_2_f32(
            gl.get_uniform_location(self.terminal_shader, "u_output_resolution").as_ref(),
            render_buffer_size.x + font_width,
            render_buffer_size.y + font_height,
        );
        let viewport_top = buffer_view.calc.viewport_top();
        let top_pos = viewport_top.floor();
        let c_width = buffer_view.calc.char_size.x;
        let c_height = buffer_view.calc.char_size.y;
        let scroll_offset_x = -(((viewport_top.x / c_width) * font_width) % font_width).floor();
        let scroll_offset_y = (((viewport_top.y / c_height) * font_height) % font_height).floor();
        gl.uniform_2_f32(
            gl.get_uniform_location(self.terminal_shader, "u_position").as_ref(),
            scroll_offset_x,
            scroll_offset_y - font_height,
        );

        gl.uniform_2_f32(
            gl.get_uniform_location(self.terminal_shader, "u_scroll_pos").as_ref(),
            (viewport_top.x / c_width) * font_height,
            (viewport_top.y / c_height) * font_height,
        );

        let mut caret_pos = buffer_view.get_caret().get_position();
        if let Some(layer) = buffer_view.edit_state.get_cur_layer() {
            caret_pos += layer.get_offset();
        }

        if let Some(window) = &buffer_view.get_buffer().terminal_state.text_window {
            caret_pos += window.top_left();
        }

        let caret_x = caret_pos.x as f32 * font_width - (top_pos.x / buffer_view.calc.char_size.x * font_width) - scroll_offset_x;

        let caret_h = if buffer_view.get_caret().insert_mode {
            fontdim.height as f32 / 2.0
        } else {
            match terminal_options.caret_shape {
                crate::CaretShape::Block => fontdim.height as f32,
                crate::CaretShape::Underline => 2.0,
            }
        };

        let caret_y = caret_pos.y as f32 * fontdim.height as f32 + fontdim.height as f32 - caret_h - (top_pos.y / buffer_view.calc.char_size.y * font_height)
            + scroll_offset_y;
        let caret_w = if self.caret_blink.is_on() && buffer_view.get_caret().is_visible() && (has_focus || terminal_options.force_focus) {
            font_width
        } else {
            0.0
        };
        //println!("has focus:{} visible: {}, w:{}", has_focus, buffer_view.get_caret().is_visible, caret_w);

        gl.uniform_4_f32(
            gl.get_uniform_location(self.terminal_shader, "u_caret_rectangle").as_ref(),
            caret_x / (render_buffer_size.x + font_width),
            caret_y / (render_buffer_size.y + font_height),
            (caret_x + caret_w) / (render_buffer_size.x + font_width),
            (caret_y + caret_h) / (render_buffer_size.y + font_height),
        );

        gl.uniform_1_f32(
            gl.get_uniform_location(self.terminal_shader, "u_character_blink").as_ref(),
            if self.character_blink.is_on() { 1.0 } else { 0.0 },
        );
        gl.uniform_2_f32(
            gl.get_uniform_location(self.terminal_shader, "u_terminal_size").as_ref(),
            buffer_view.calc.forced_width as f32 - 0.0001,
            buffer_view.calc.forced_height as f32 - 0.0001,
        );

        gl.uniform_1_i32(gl.get_uniform_location(self.terminal_shader, "u_fonts").as_ref(), FONT_TEXTURE_SLOT as i32);

        gl.uniform_1_i32(
            gl.get_uniform_location(self.terminal_shader, "u_terminal_buffer").as_ref(),
            BUFFER_TEXTURE_SLOT as i32,
        );

        gl.uniform_1_i32(
            gl.get_uniform_location(self.terminal_shader, "u_reference_image").as_ref(),
            REFERENCE_IMAGE_TEXTURE_SLOT as i32,
        );

        let has_ref_image = if self.show_reference_image && self.reference_image.is_some() || self.igs_executor.is_some() || self.color_image.is_some() {
            1.0
        } else {
            0.0
        };
        if let Some(img) = &self.reference_image {
            gl.uniform_2_f32(
                gl.get_uniform_location(self.terminal_shader, "u_reference_image_size").as_ref(),
                img.width() as f32,
                img.height() as f32,
            );
            gl.uniform_1_f32(
                gl.get_uniform_location(self.terminal_shader, "u_reference_image_alpha").as_ref(),
                terminal_options.marker_settings.reference_image_alpha,
            );
        }

        if let Some((size, _img)) = &self.igs_executor {
            gl.uniform_2_f32(
                gl.get_uniform_location(self.terminal_shader, "u_reference_image_size").as_ref(),
                size.width as f32,
                size.height as f32,
            );

            gl.uniform_1_f32(gl.get_uniform_location(self.terminal_shader, "u_reference_image_alpha").as_ref(), 1.0);
        }

        if let Some((size, _img)) = &self.color_image {
            gl.uniform_2_f32(
                gl.get_uniform_location(self.terminal_shader, "u_reference_image_size").as_ref(),
                size.width as f32,
                320 as f32,
            );

            gl.uniform_1_f32(gl.get_uniform_location(self.terminal_shader, "u_reference_image_alpha").as_ref(), 1.0);
        }

        gl.uniform_1_f32(gl.get_uniform_location(self.terminal_shader, "u_has_reference_image").as_ref(), has_ref_image);
        let (r, g, b) = terminal_options.monitor_settings.selection_fg.get_rgb_f32();

        gl.uniform_4_f32(gl.get_uniform_location(self.terminal_shader, "u_selection_fg").as_ref(), r, g, b, 1.0);

        let (r, g, b) = terminal_options.monitor_settings.selection_bg.get_rgb_f32();

        gl.uniform_4_f32(gl.get_uniform_location(self.terminal_shader, "u_selection_bg").as_ref(), r, g, b, 1.0);

        gl.uniform_1_f32(
            gl.get_uniform_location(self.terminal_shader, "u_selection_attr").as_ref(),
            if buffer_view.get_buffer().is_terminal_buffer { 1.0 } else { 0.0 },
        );

        crate::check_gl_error!(gl, "run_shader");
    }

    pub(crate) fn reset_caret_blink(&mut self) {
        let cur_ms = self.start_time.elapsed().as_millis();
        self.caret_blink.reset(cur_ms);
    }
}

unsafe fn compile_shader(gl: &glow::Context) -> glow::Program {
    let program = gl.create_program().expect("Cannot create program");

    let (vertex_shader_source, fragment_shader_source) = (crate::ui::buffer_view::SHADER_SOURCE, include_str!("terminal_renderer.shader.frag"));
    let shader_sources = [(glow::VERTEX_SHADER, vertex_shader_source), (glow::FRAGMENT_SHADER, fragment_shader_source)];

    let shaders: Vec<_> = shader_sources
        .iter()
        .map(|(shader_type, shader_source)| {
            let shader = gl.create_shader(*shader_type).expect("Cannot create shader");

            let shader_source = shader_source
                .replace("%LAYOUT0%", "layout(location = 0)")
                .replace("%LAYOUT1%", "layout(location = 1)");

            gl.shader_source(shader, &format!("{}\n{}", crate::get_shader_version(gl), shader_source));
            gl.compile_shader(shader);
            assert!(gl.get_shader_compile_status(shader), "{}", gl.get_shader_info_log(shader));
            gl.attach_shader(program, shader);
            shader
        })
        .collect();

    gl.link_program(program);
    assert!(gl.get_program_link_status(program), "{}", gl.get_program_info_log(program));

    for shader in shaders {
        gl.detach_shader(program, shader);
        gl.delete_shader(shader);
    }
    crate::check_gl_error!(gl, "compile_shader");

    program
}

unsafe fn create_buffer_texture(gl: &glow::Context) -> glow::Texture {
    let buffer_texture = gl.create_texture().unwrap();
    gl.bind_texture(glow::TEXTURE_2D_ARRAY, Some(buffer_texture));
    gl.tex_parameter_i32(glow::TEXTURE_2D_ARRAY, glow::TEXTURE_MIN_FILTER, glow::NEAREST as i32);
    gl.tex_parameter_i32(glow::TEXTURE_2D_ARRAY, glow::TEXTURE_MAG_FILTER, glow::NEAREST as i32);
    gl.tex_parameter_i32(glow::TEXTURE_2D_ARRAY, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32);
    gl.tex_parameter_i32(glow::TEXTURE_2D_ARRAY, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);
    crate::check_gl_error!(gl, "create_buffer_texture");

    buffer_texture
}

unsafe fn create_reference_image_texture(gl: &glow::Context) -> glow::Texture {
    let reference_image_texture: glow::Texture = gl.create_texture().unwrap();
    gl.bind_texture(glow::TEXTURE_2D, Some(reference_image_texture));
    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::NEAREST as i32);
    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::NEAREST as i32);
    crate::check_gl_error!(gl, "create_refeference_image_texture");

    reference_image_texture
}

unsafe fn create_font_texture(gl: &glow::Context) -> glow::Texture {
    let font_texture = gl.create_texture().unwrap();
    gl.bind_texture(glow::TEXTURE_2D_ARRAY, Some(font_texture));

    gl.tex_parameter_i32(glow::TEXTURE_2D_ARRAY, glow::TEXTURE_MIN_FILTER, glow::NEAREST as i32);
    gl.tex_parameter_i32(glow::TEXTURE_2D_ARRAY, glow::TEXTURE_MAG_FILTER, glow::NEAREST as i32);
    gl.tex_parameter_i32(glow::TEXTURE_2D_ARRAY, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32);
    gl.tex_parameter_i32(glow::TEXTURE_2D_ARRAY, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);
    crate::check_gl_error!(gl, "create_font_texture");

    font_texture
}
