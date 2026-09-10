use egui::{PaintCallbackInfo, Rect};
use egui_wgpu::{wgpu, CallbackResources, CallbackTrait, ScreenDescriptor};

use super::{TerminalShader, TerminalShaderRenderer};

pub struct TerminalCallback {
    pub frame: TerminalShader,
    pub bounds: Rect,
    pub context: egui::Context,
}

impl CallbackTrait for TerminalCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen: &ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let renderer = resources.get_mut::<TerminalShaderRenderer>().expect("terminal renderer initialized");
        let previous_scale = self.frame.render_info.read().display_scale;
        self.frame.prepare_frame(
            renderer,
            device,
            queue,
            [self.bounds.min.x, self.bounds.min.y, self.bounds.width(), self.bounds.height()],
            screen.pixels_per_point,
        );
        {
            let mut info = self.frame.render_info.write();
            info.bounds_x = self.bounds.min.x;
            info.bounds_y = self.bounds.min.y;
        }
        // The frame was built with the previous scale, so a changed scale needs a corrected follow-up frame.
        if previous_scale != self.frame.render_info.read().display_scale {
            self.context.request_repaint();
        }
        Vec::new()
    }

    fn paint(&self, info: PaintCallbackInfo, pass: &mut wgpu::RenderPass<'static>, resources: &CallbackResources) {
        let renderer = resources.get::<TerminalShaderRenderer>().expect("terminal renderer initialized");
        let clip = info.clip_rect_in_pixels();
        self.frame.paint(
            renderer,
            pass,
            [clip.left_px as u32, clip.top_px as u32, clip.width_px as u32, clip.height_px as u32],
        );
    }
}
