// Tag Shader - Simple texture-only rendering for filename labels
// No effects, shadows, or borders - just sample and display the tag texture

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
};

@group(0) @binding(0) var t_tag: texture_2d<f32>;
@group(0) @binding(1) var s_sampler: sampler;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Fullscreen quad (2 triangles)
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(1.0, 1.0),
    );
    
    // UV coords with V flipped for correct orientation
    var tex_coords = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),  // bottom-left -> top-left of texture
        vec2<f32>(1.0, 0.0),  // bottom-right -> top-right of texture
        vec2<f32>(0.0, 1.0),  // top-left -> bottom-left of texture
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
    );

    var output: VertexOutput;
    output.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    output.tex_coord = tex_coords[vertex_index];
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_tag, s_sampler, input.tex_coord);
}
