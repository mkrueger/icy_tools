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
        let previous = self.frame.render_info.read().clone();
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
        // Editor overlays use the previous render geometry and need a follow-up frame when it changes.
        let current = self.frame.render_info.read();
        if previous.display_scale != current.display_scale
            || previous.bounds_x != current.bounds_x
            || previous.bounds_y != current.bounds_y
            || previous.viewport_x != current.viewport_x
            || previous.viewport_y != current.viewport_y
            || previous.viewport_width != current.viewport_width
            || previous.viewport_height != current.viewport_height
        {
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
