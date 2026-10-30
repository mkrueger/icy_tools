use icy_ui::wgpu;
use std::ops::Deref;

macro_rules! gpu_api {
    ($legacy:expr, $current:expr) => {
        $legacy
    };
}

macro_rules! gpu_pipeline {
    ($device:expr, $($field:ident: $value:expr,)*) => {
        $device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            $($field: $value,)*
            multiview: None,
        })
    };
}

#[path = "shader_backend.rs"]
mod backend;
pub use backend::TerminalShaderRenderer;

#[derive(Debug, Clone)]
pub struct TerminalShader(pub super::TerminalShader);

impl Deref for TerminalShader {
    type Target = super::TerminalShader;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
