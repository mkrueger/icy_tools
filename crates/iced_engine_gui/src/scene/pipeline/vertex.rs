use iced::wgpu;

#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub tangent: [f32; 3],
    pub uv: [f32; 2],
}

impl Vertex {
    const ATTRIBS: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
        //position
        0 => Float32x3,
        //normal
        1 => Float32x3,
        //tangent
        2 => Float32x3,
        //uv
        3 => Float32x2,
    ];

    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }

    // Helper constructors if needed
    pub fn new(pos: glam::Vec3, normal: glam::Vec3, tangent: glam::Vec3, uv: glam::Vec2) -> Self {
        Self {
            pos: pos.to_array(),
            normal: normal.to_array(),
            tangent: tangent.to_array(),
            uv: uv.to_array(),
        }
    }

    // Helper getters that return glam types if needed
    pub fn pos_vec(&self) -> glam::Vec3 {
        glam::Vec3::from_array(self.pos)
    }

    pub fn normal_vec(&self) -> glam::Vec3 {
        glam::Vec3::from_array(self.normal)
    }

    pub fn tangent_vec(&self) -> glam::Vec3 {
        glam::Vec3::from_array(self.tangent)
    }

    pub fn uv_vec(&self) -> glam::Vec2 {
        glam::Vec2::from_array(self.uv)
    }
}
