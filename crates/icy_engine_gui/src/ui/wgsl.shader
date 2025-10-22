struct Uniforms {
    transform: vec4<f32>;
    atlas_size: vec2<f32>;
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var atlas_tex: texture_2d<f32>;
@group(0) @binding(2) var atlas_sampler: sampler;

struct VSIn {
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,     // unorm8 -> 0..1
};

struct VSOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(in: VSIn) -> VSOut {
    let sx = uniforms.transform.x;
    let sy = uniforms.transform.y;
    let tx = uniforms.transform.z;
    let ty = uniforms.transform.w;
    var out: VSOut;
    out.position = vec4<f32>(in.pos.x * sx + tx, in.pos.y * sy + ty, 0.0, 1.0);
    out.uv = in.uv;
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VSOut) -> @location(0) vec4<f32> {
    // If you later add alpha or font mask: sample atlas_tex at in.uv
    let glyph_color = in.color;
    return glyph_color;
}