use crate::scene::Camera;

use iced::{Color, Rectangle};

#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Uniforms {
    camera_proj: [[f32; 4]; 4], // 4x4 matrix as array
    camera_pos: [f32; 4],       // Vec4 as array
    light_color: [f32; 4],      // Vec4 as array
}

impl Uniforms {
    pub fn new(camera: &Camera, bounds: Rectangle, light_color: Color) -> Self {
        let camera_proj = camera.build_view_proj_matrix(bounds);
        let camera_pos = camera.position();
        let light_color_vec = glam::Vec4::from(light_color.into_linear());

        Self {
            camera_proj: camera_proj.to_cols_array_2d(),
            camera_pos: camera_pos.to_array(),
            light_color: light_color_vec.to_array(),
        }
    }
}
