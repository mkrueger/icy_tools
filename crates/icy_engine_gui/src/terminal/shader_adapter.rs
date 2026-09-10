use icy_ui::{advanced::graphics::Viewport, wgpu, widget::shader, Rectangle};

use super::{TerminalShader, TerminalShaderRenderer};

impl shader::Pipeline for TerminalShaderRenderer {
    fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        Self::new(device, format)
    }
}

impl shader::Primitive for TerminalShader {
    type Pipeline = TerminalShaderRenderer;

    fn prepare(&self, pipeline: &mut Self::Pipeline, device: &wgpu::Device, queue: &wgpu::Queue, bounds: &Rectangle, viewport: &Viewport) {
        crate::set_scale_factor(viewport.scale_factor() as f32);
        self.prepare_frame(
            pipeline,
            device,
            queue,
            [bounds.x, bounds.y, bounds.width, bounds.height],
            viewport.scale_factor() as f32,
        );
    }

    fn render(&self, pipeline: &Self::Pipeline, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView, clip_bounds: &Rectangle<u32>) {
        self.render_to_target(pipeline, encoder, target, [clip_bounds.x, clip_bounds.y, clip_bounds.width, clip_bounds.height]);
    }
}
