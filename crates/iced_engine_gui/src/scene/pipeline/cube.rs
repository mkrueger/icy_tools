use crate::scene::pipeline::Vertex;
use iced::wgpu;

use glam::{Vec3, vec2, vec3};
use rand::{Rng, thread_rng};

/// A single instance of a cube.
#[derive(Debug, Clone)]
pub struct Cube {
    pub rotation: glam::Quat,
    pub position: Vec3,
    pub size: f32,
    rotation_dir: f32,
    rotation_axis: glam::Vec3,
}

impl Default for Cube {
    fn default() -> Self {
        Self {
            rotation: glam::Quat::IDENTITY,
            position: glam::Vec3::ZERO,
            size: 0.1,
            rotation_dir: 1.0,
            rotation_axis: glam::Vec3::Y,
        }
    }
}

impl Cube {
    pub fn new(size: f32, origin: Vec3) -> Self {
        let rnd = thread_rng().gen_range(0.0..=1.0f32);

        Self {
            rotation: glam::Quat::IDENTITY,
            position: origin + Vec3::new(0.1, 0.1, 0.1),
            size,
            rotation_dir: if rnd <= 0.5 { -1.0 } else { 1.0 },
            rotation_axis: if rnd <= 0.33 {
                glam::Vec3::Y
            } else if rnd <= 0.66 {
                glam::Vec3::X
            } else {
                glam::Vec3::Z
            },
        }
    }

    pub fn update(&mut self, size: f32, time: f32) {
        self.rotation = glam::Quat::from_axis_angle(self.rotation_axis, time / 2.0 * self.rotation_dir);
        self.size = size;
    }
}

#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Debug)]
#[repr(C)]
pub struct Raw {
    transformation: [[f32; 4]; 4], // Mat4 as 4x4 array
    normal: [[f32; 3]; 3],         // Mat3 as 3x3 array
    _padding: [f32; 3],
}

impl Raw {
    const ATTRIBS: [wgpu::VertexAttribute; 7] = wgpu::vertex_attr_array![
        //cube transformation matrix
        4 => Float32x4,
        5 => Float32x4,
        6 => Float32x4,
        7 => Float32x4,
        //normal rotation matrix
        8 => Float32x3,
        9 => Float32x3,
        10 => Float32x3,
    ];

    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }
}

impl Raw {
    pub fn from_cube(cube: &Cube) -> Raw {
        let transformation = glam::Mat4::from_scale_rotation_translation(glam::vec3(cube.size, cube.size, cube.size), cube.rotation, cube.position);
        let normal = glam::Mat3::from_quat(cube.rotation);

        Raw {
            transformation: transformation.to_cols_array_2d(),
            normal: [
                [normal.x_axis.x, normal.x_axis.y, normal.x_axis.z],
                [normal.y_axis.x, normal.y_axis.y, normal.y_axis.z],
                [normal.z_axis.x, normal.z_axis.y, normal.z_axis.z],
            ],
            _padding: [0.0; 3],
        }
    }

    pub fn vertices() -> [Vertex; 36] {
        [
            //face 1
            Vertex::new(vec3(-0.5, -0.5, -0.5), vec3(0.0, 0.0, -1.0), vec3(1.0, 0.0, 0.0), vec2(0.0, 1.0)),
            Vertex::new(vec3(0.5, -0.5, -0.5), vec3(0.0, 0.0, -1.0), vec3(1.0, 0.0, 0.0), vec2(1.0, 1.0)),
            Vertex::new(vec3(0.5, 0.5, -0.5), vec3(0.0, 0.0, -1.0), vec3(1.0, 0.0, 0.0), vec2(1.0, 0.0)),
            Vertex::new(vec3(0.5, 0.5, -0.5), vec3(0.0, 0.0, -1.0), vec3(1.0, 0.0, 0.0), vec2(1.0, 0.0)),
            Vertex::new(vec3(-0.5, 0.5, -0.5), vec3(0.0, 0.0, -1.0), vec3(1.0, 0.0, 0.0), vec2(0.0, 0.0)),
            Vertex::new(vec3(-0.5, -0.5, -0.5), vec3(0.0, 0.0, -1.0), vec3(1.0, 0.0, 0.0), vec2(0.0, 1.0)),
            //face 2
            Vertex::new(vec3(-0.5, -0.5, 0.5), vec3(0.0, 0.0, 1.0), vec3(1.0, 0.0, 0.0), vec2(0.0, 1.0)),
            Vertex::new(vec3(0.5, -0.5, 0.5), vec3(0.0, 0.0, 1.0), vec3(1.0, 0.0, 0.0), vec2(1.0, 1.0)),
            Vertex::new(vec3(0.5, 0.5, 0.5), vec3(0.0, 0.0, 1.0), vec3(1.0, 0.0, 0.0), vec2(1.0, 0.0)),
            Vertex::new(vec3(0.5, 0.5, 0.5), vec3(0.0, 0.0, 1.0), vec3(1.0, 0.0, 0.0), vec2(1.0, 0.0)),
            Vertex::new(vec3(-0.5, 0.5, 0.5), vec3(0.0, 0.0, 1.0), vec3(1.0, 0.0, 0.0), vec2(0.0, 0.0)),
            Vertex::new(vec3(-0.5, -0.5, 0.5), vec3(0.0, 0.0, 1.0), vec3(1.0, 0.0, 0.0), vec2(0.0, 1.0)),
            //face 3
            Vertex::new(vec3(-0.5, 0.5, 0.5), vec3(-1.0, 0.0, 0.0), vec3(0.0, 0.0, -1.0), vec2(0.0, 1.0)),
            Vertex::new(vec3(-0.5, 0.5, -0.5), vec3(-1.0, 0.0, 0.0), vec3(0.0, 0.0, -1.0), vec2(1.0, 1.0)),
            Vertex::new(vec3(-0.5, -0.5, -0.5), vec3(-1.0, 0.0, 0.0), vec3(0.0, 0.0, -1.0), vec2(1.0, 0.0)),
            Vertex::new(vec3(-0.5, -0.5, -0.5), vec3(-1.0, 0.0, 0.0), vec3(0.0, 0.0, -1.0), vec2(1.0, 0.0)),
            Vertex::new(vec3(-0.5, -0.5, 0.5), vec3(-1.0, 0.0, 0.0), vec3(0.0, 0.0, -1.0), vec2(0.0, 0.0)),
            Vertex::new(vec3(-0.5, 0.5, 0.5), vec3(-1.0, 0.0, 0.0), vec3(0.0, 0.0, -1.0), vec2(0.0, 1.0)),
            //face 4
            Vertex::new(vec3(0.5, 0.5, 0.5), vec3(1.0, 0.0, 0.0), vec3(0.0, 0.0, -1.0), vec2(0.0, 1.0)),
            Vertex::new(vec3(0.5, 0.5, -0.5), vec3(1.0, 0.0, 0.0), vec3(0.0, 0.0, -1.0), vec2(1.0, 1.0)),
            Vertex::new(vec3(0.5, -0.5, -0.5), vec3(1.0, 0.0, 0.0), vec3(0.0, 0.0, -1.0), vec2(1.0, 0.0)),
            Vertex::new(vec3(0.5, -0.5, -0.5), vec3(1.0, 0.0, 0.0), vec3(0.0, 0.0, -1.0), vec2(1.0, 0.0)),
            Vertex::new(vec3(0.5, -0.5, 0.5), vec3(1.0, 0.0, 0.0), vec3(0.0, 0.0, -1.0), vec2(0.0, 0.0)),
            Vertex::new(vec3(0.5, 0.5, 0.5), vec3(1.0, 0.0, 0.0), vec3(0.0, 0.0, -1.0), vec2(0.0, 1.0)),
            //face 5
            Vertex::new(vec3(-0.5, -0.5, -0.5), vec3(0.0, -1.0, 0.0), vec3(1.0, 0.0, 0.0), vec2(0.0, 1.0)),
            Vertex::new(vec3(0.5, -0.5, -0.5), vec3(0.0, -1.0, 0.0), vec3(1.0, 0.0, 0.0), vec2(1.0, 1.0)),
            Vertex::new(vec3(0.5, -0.5, 0.5), vec3(0.0, -1.0, 0.0), vec3(1.0, 0.0, 0.0), vec2(1.0, 0.0)),
            Vertex::new(vec3(0.5, -0.5, 0.5), vec3(0.0, -1.0, 0.0), vec3(1.0, 0.0, 0.0), vec2(1.0, 0.0)),
            Vertex::new(vec3(-0.5, -0.5, 0.5), vec3(0.0, -1.0, 0.0), vec3(1.0, 0.0, 0.0), vec2(0.0, 0.0)),
            Vertex::new(vec3(-0.5, -0.5, -0.5), vec3(0.0, -1.0, 0.0), vec3(1.0, 0.0, 0.0), vec2(0.0, 1.0)),
            //face 6
            Vertex::new(vec3(-0.5, 0.5, -0.5), vec3(0.0, 1.0, 0.0), vec3(1.0, 0.0, 0.0), vec2(0.0, 1.0)),
            Vertex::new(vec3(0.5, 0.5, -0.5), vec3(0.0, 1.0, 0.0), vec3(1.0, 0.0, 0.0), vec2(1.0, 1.0)),
            Vertex::new(vec3(0.5, 0.5, 0.5), vec3(0.0, 1.0, 0.0), vec3(1.0, 0.0, 0.0), vec2(1.0, 0.0)),
            Vertex::new(vec3(0.5, 0.5, 0.5), vec3(0.0, 1.0, 0.0), vec3(1.0, 0.0, 0.0), vec2(1.0, 0.0)),
            Vertex::new(vec3(-0.5, 0.5, 0.5), vec3(0.0, 1.0, 0.0), vec3(1.0, 0.0, 0.0), vec2(0.0, 0.0)),
            Vertex::new(vec3(-0.5, 0.5, -0.5), vec3(0.0, 1.0, 0.0), vec3(1.0, 0.0, 0.0), vec2(0.0, 1.0)),
        ]
    }
}
