use egui::Vec2;
use glow::HasContext;

use crate::ui::buffer_view::SHADER_SOURCE;
use crate::TerminalOptions;

use super::output_renderer::MONO_COLORS;

pub struct TextureRenderer {
    output_shader: glow::Program,
    vertex_array: glow::VertexArray,
}

impl TextureRenderer {
    pub fn new(gl: &glow::Context) -> Self {
        unsafe {
            let output_shader = compile_output_shader(gl);
            let vertex_array = gl.create_vertex_array().expect("Cannot create vertex array");
            Self { output_shader, vertex_array }
        }
    }

    pub fn destroy(&self, gl: &glow::Context) {
        unsafe {
            gl.delete_program(self.output_shader);
            gl.delete_vertex_array(self.vertex_array);
        }
    }

    pub unsafe fn render_to_buffer(
        &self,
        gl: &glow::Context,
        input_texture: glow::Texture,
        render_buffer_size: Vec2,
        options: &TerminalOptions,
    ) -> (Vec2, Vec<u8>) {
        gl.disable(glow::SCISSOR_TEST);

        let monitor_settings = &options.monitor_settings;
        let framebuffer = gl.create_framebuffer().unwrap();
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(framebuffer));

        let output_texture = self.create_output_texture(gl, render_buffer_size);
        crate::check_gl_error!(gl, "render_to_buffer_startup_bind_framebuffer");
        gl.framebuffer_texture(glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT0, Some(output_texture), 0);
        gl.draw_buffers(&[glow::COLOR_ATTACHMENT0]);
        if gl.check_framebuffer_status(glow::FRAMEBUFFER) != glow::FRAMEBUFFER_COMPLETE {
            log::error!("Framebuffer is not complete");
        }
        crate::check_gl_error!(gl, "render_to_buffer_startup_bind_output_texture");
        gl.viewport(0, 0, render_buffer_size.x as i32, render_buffer_size.y as i32);
        crate::check_gl_error!(gl, "render_to_buffer_startup_set_viewport");
        gl.clear(glow::COLOR_BUFFER_BIT);
        gl.clear_color(0.0, 0., 0., 0.0);
        crate::check_gl_error!(gl, "render_to_buffer_startup_clear");

        gl.use_program(Some(self.output_shader));

        gl.active_texture(glow::TEXTURE0 + super::output_renderer::INPUT_TEXTURE_SLOT);
        gl.bind_texture(glow::TEXTURE_2D, Some(input_texture));
        gl.uniform_1_i32(
            gl.get_uniform_location(self.output_shader, "u_render_texture").as_ref(),
            super::output_renderer::INPUT_TEXTURE_SLOT as i32,
        );

        let eff = if monitor_settings.use_filter { 1.0 } else { 0.0 };
        gl.uniform_1_f32(gl.get_uniform_location(self.output_shader, "u_effect").as_ref(), eff);

        gl.uniform_1_f32(
            gl.get_uniform_location(self.output_shader, "u_use_monochrome").as_ref(),
            if monitor_settings.monitor_type > 0 { 1.0 } else { 0.0 },
        );

        if monitor_settings.monitor_type > 0 {
            let r = MONO_COLORS[monitor_settings.monitor_type - 1].0 as f32 / 255.0;
            let g = MONO_COLORS[monitor_settings.monitor_type - 1].1 as f32 / 255.0;
            let b = MONO_COLORS[monitor_settings.monitor_type - 1].2 as f32 / 255.0;
            gl.uniform_3_f32(gl.get_uniform_location(self.output_shader, "u_monchrome_mask").as_ref(), r, g, b);
        }

        gl.uniform_1_f32(gl.get_uniform_location(self.output_shader, "gamma").as_ref(), monitor_settings.gamma / 50.0);

        gl.uniform_1_f32(
            gl.get_uniform_location(self.output_shader, "contrast").as_ref(),
            monitor_settings.contrast / 50.0,
        );

        gl.uniform_1_f32(
            gl.get_uniform_location(self.output_shader, "saturation").as_ref(),
            monitor_settings.saturation / 50.0,
        );

        gl.uniform_1_f32(
            gl.get_uniform_location(self.output_shader, "brightness").as_ref(),
            monitor_settings.brightness / 30.0,
        );

        gl.uniform_1_f32(gl.get_uniform_location(self.output_shader, "blur").as_ref(), monitor_settings.blur / 30.0);

        gl.uniform_1_f32(
            gl.get_uniform_location(self.output_shader, "curvature").as_ref(),
            monitor_settings.curvature / 30.0,
        );
        gl.uniform_1_f32(
            gl.get_uniform_location(self.output_shader, "u_scanlines").as_ref(),
            0.5 * (monitor_settings.scanlines / 100.0),
        );

        gl.uniform_2_f32(
            gl.get_uniform_location(self.output_shader, "u_resolution").as_ref(),
            render_buffer_size.x,
            render_buffer_size.y,
        );

        gl.bind_vertex_array(Some(self.vertex_array));
        gl.draw_arrays(glow::TRIANGLES, 0, 6);
        crate::check_gl_error!(gl, "render_to_buffer_draw_arrays");

        let mut pixels = vec![0; (render_buffer_size.x * render_buffer_size.y * 4.0) as usize];

        gl.read_pixels(
            0,
            0,
            render_buffer_size.x as i32,
            render_buffer_size.y as i32,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelPackData::Slice(&mut pixels),
        );
        /*
        gl.bind_texture(glow::TEXTURE_2D, Some(output_texture));
        gl.get_tex_image(
            glow::TEXTURE_2D,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelPackData::Slice(&mut pixels),
        );*/

        gl.delete_framebuffer(framebuffer);
        gl.delete_texture(output_texture);
        gl.delete_texture(input_texture);
        crate::check_gl_error!(gl, "render_to_buffer_read_pixels");
        (render_buffer_size, pixels)
    }

    unsafe fn create_output_texture(&self, gl: &glow::Context, render_buffer_size: Vec2) -> glow::Texture {
        let result = gl.create_texture().unwrap();
        gl.bind_texture(glow::TEXTURE_2D, Some(result));

        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA as i32,
            render_buffer_size.x as i32,
            render_buffer_size.y as i32,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            None,
        );
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);

        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::NEAREST as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::NEAREST as i32);
        crate::check_gl_error!(gl, "create_output_texture");
        result
    }
}

unsafe fn compile_output_shader(gl: &glow::Context) -> glow::Program {
    let draw_program = gl.create_program().expect("Cannot create program");
    let (vertex_shader_source, fragment_shader_source) = (SHADER_SOURCE, include_str!("texture_renderer.shader.frag"));
    let shader_sources = [(glow::VERTEX_SHADER, vertex_shader_source), (glow::FRAGMENT_SHADER, fragment_shader_source)];

    let shaders: Vec<_> = shader_sources
        .iter()
        .map(|(shader_type, shader_source)| {
            let shader = gl.create_shader(*shader_type).expect("Cannot create shader");
            gl.shader_source(shader, &format!("{}\n{}", crate::get_shader_version(gl), shader_source));
            gl.compile_shader(shader);
            assert!(gl.get_shader_compile_status(shader), "{}", gl.get_shader_info_log(shader));
            gl.attach_shader(draw_program, shader);
            shader
        })
        .collect();

    gl.link_program(draw_program);
    assert!(gl.get_program_link_status(draw_program), "{}", gl.get_program_info_log(draw_program));

    for shader in shaders {
        gl.detach_shader(draw_program, shader);
        gl.delete_shader(shader);
    }
    draw_program
}
