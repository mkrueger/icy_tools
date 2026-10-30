//! Terminal frames and the current wgpu renderer.

use parking_lot::RwLock;
use std::sync::Arc;

use crate::{MonitorSettings, RenderInfo, TextureSliceData};
use wgpu;

// Compile the same renderer for egui's wgpu and the legacy UI's older wgpu.
macro_rules! gpu_api {
    ($legacy:expr, $current:expr) => {
        $current
    };
}

macro_rules! gpu_pipeline {
    ($device:expr, $($field:ident: $value:expr,)*) => {
        $device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            $($field: $value,)*
            multiview_mask: None,
        })
    };
}

#[path = "shader_backend.rs"]
mod backend;
pub use backend::TerminalShaderRenderer;

/// The terminal shader program (high-level interface).
#[derive(Debug, Clone)]
pub struct TerminalShader {
    /// Texture slices for blink_off state.
    pub slices_blink_off: Vec<TextureSliceData>,
    /// Texture slices for blink_on state.
    pub slices_blink_on: Vec<TextureSliceData>,
    /// Number of text layers; graphics layers follow at the same indices.
    pub text_slice_count: usize,
    /// Unicode text texels per raw document pixel; graphics are not scaled.
    pub text_raster_scale: u32,
    /// Heights of each slice in logical raw document pixels.
    pub slice_heights: Vec<u32>,
    /// Logical document width; Unicode textures can have a higher resolution.
    pub texture_width: u32,
    pub total_content_height: f32,
    pub monitor_settings: Arc<MonitorSettings>,
    pub instance_id: u64,
    /// Incremented when tiles are re-rendered.
    pub render_generation: u64,
    pub zoom: f32,
    /// Visible viewport dimensions, which can differ from widget bounds.
    pub viewport_width: f32,
    pub viewport_height: f32,
    pub render_info: Arc<RwLock<RenderInfo>>,
    pub font_width: f32,
    pub font_height: f32,
    pub scan_lines: bool,
    pub background_color: [f32; 4],
    pub scroll_offset_y: f32,
    pub visible_height: f32,
    pub aspect_ratio_y: f32,
    pub first_slice_start_y: f32,
    pub scroll_offset_x: f32,
    pub visible_width: f32,
    pub caret_pos: [f32; 2],
    pub caret_size: [f32; 2],
    pub caret_visible: bool,
    /// 0 = Bar, 1 = Block, 2 = Underline.
    pub caret_mode: u8,
    pub blink_on: bool,
    pub raster_spacing: Option<(f32, f32)>,
    pub raster_color: [f32; 4],
    pub raster_alpha: f32,
    pub guide_pos: Option<(f32, f32)>,
    pub guide_color: [f32; 4],
    pub guide_alpha: f32,
    /// RGBA bytes, width, height.
    pub reference_image_data: Option<(Arc<Vec<u8>>, u32, u32)>,
    pub reference_image_enabled: bool,
    pub reference_image_alpha: f32,
    /// 0 = Stretch, 1 = Original, 2 = Tile.
    pub reference_image_mode: u8,
    pub reference_image_offset: [f32; 2],
    pub reference_image_scale: f32,
    pub layer_rect: Option<[f32; 4]>,
    pub layer_color: [f32; 4],
    pub show_layer_bounds: bool,
    pub paste_mode: bool,
    pub selection_rect: Option<[f32; 4]>,
    pub selection_color: [f32; 4],
    /// RGBA bytes, width in cells, height in cells.
    pub selection_mask_data: Option<(Vec<u8>, u32, u32)>,
    /// RGBA bytes, width in cells, height in cells.
    pub tool_overlay_mask_data: Option<(Vec<u8>, u32, u32)>,
    pub tool_overlay_rect: Option<[f32; 4]>,
    /// 1.0 for normal cells, 0.5 for half-blocks.
    pub tool_overlay_cell_height_scale: f32,
    pub brush_preview_rect: Option<[f32; 4]>,
}
