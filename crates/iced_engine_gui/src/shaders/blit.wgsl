struct VertexOutput {
    @builtin(position) gl_Position: vec4<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) Index: u32) -> VertexOutput {
    let vertex = vec2(f32((Index << 1u) & 2u), f32(Index & 2u));
    return VertexOutput(vec4(vertex * vec2(2.0, -2.0) + vec2(-1.0, 1.0), 0.0, 1.0));
}

struct FragmentOutput {
    @location(0) FragColor: vec4<f32>,
}

@group(0) @binding(0) 
var Texture: texture_2d<f32>;
@group(0) @binding(1) 
var Sampler: sampler;

struct Uniforms {
    screen_size: vec2<f32>,
    preserve_aspect: u32,
    use_srgb: u32,
}

@group(0) @binding(2)
var<uniform> uniforms: Uniforms;

@fragment
fn fs_main(@builtin(position) gl_Position: vec4<f32>) -> FragmentOutput {
    let target_size = select(vec2<f32>(textureDimensions(Texture)), uniforms.screen_size, uniforms.preserve_aspect == 0u);
    let uv = gl_Position.xy / target_size;
    let factor = select(2.2, 1.0, uniforms.use_srgb == 0u);

    let color = pow(textureSample(Texture, Sampler, uv), vec4(vec3(factor), 1.0));

    return FragmentOutput(select(color, vec4(0.0, 0.0, 0.0, 0.0), uv.x > 1.0 || uv.y > 1.0));
}