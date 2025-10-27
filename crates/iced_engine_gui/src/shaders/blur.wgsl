struct VertexOutput {
    @builtin(position) gl_Position: vec4<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    let uv = vec2(f32((index << 1) & 2), f32(index & 2));
    return VertexOutput(vec4(uv * vec2(2.0, -2.0) + vec2(-1.0, 1.0), 0.0, 1.0));
}

@group(0) @binding(0)
var tex: texture_2d<f32>;
@group(0) @binding(1)
var tex_s: sampler;

@group(0) @binding(2)
var<uniform> blur_amount: vec4<f32>;

fn blur(tex: texture_2d<f32>, samp: sampler, uv: vec2<f32>) -> vec4<f32> {
    var blur = textureSample(tex, samp, uv);
    blur += textureSample(tex, samp, uv + vec2(-3.5, -3.5) * blur_amount.xy);
    blur += textureSample(tex, samp, uv + vec2(-1.5, -1.5) * blur_amount.xy);
    blur += textureSample(tex, samp, uv + vec2(1.5, 1.5) * blur_amount.xy);
    blur += textureSample(tex, samp, uv + vec2(3.5, 3.5) * blur_amount.xy);

    return blur / 5.0;
}

struct FragmentOutput {
    @location(0) FragColor: vec4<f32>,
}

@fragment 
fn fs_main(@builtin(position) gl_Position: vec4<f32>) -> FragmentOutput {
    return FragmentOutput(blur(tex, tex_s, gl_Position.xy / vec2<f32>(textureDimensions(tex))));
}