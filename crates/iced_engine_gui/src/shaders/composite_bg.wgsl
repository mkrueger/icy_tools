struct VertexOutput {
    @location(0) @interpolate(flat) BgColor: u32,
    @builtin(position) gl_Position: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> ScreenSize: vec4<f32>;

@vertex
fn vs_main(
    @location(0) VertexCoord: vec2<f32>,
    @location(1) BgColor: u32,
) -> VertexOutput {
    let gl_Position = vec4<f32>((2.0 * VertexCoord / ScreenSize.xy - 1.0) * vec2(1.0, -1.0), 0.0, 1.0);
    return VertexOutput(BgColor, gl_Position);
}

struct FragmentOutput {
    @location(0) FragColor: vec4<f32>,
}

fn unpack_color(color: u32) -> vec4<f32> {
    return vec4<f32>(
        f32(color >> 24u) / 255.0,
        f32((color >> 16u) & 0xFFu) / 255.0,
        f32((color >> 8u) & 0xFFu) / 255.0,
        f32(color & 0xFFu) / 255.0,
    );
}


@fragment
fn fs_main(@location(0) @interpolate(flat) BgColor: u32) -> FragmentOutput {
    let bgColorUnpacked = unpack_color(BgColor);
    return FragmentOutput(bgColorUnpacked);
}