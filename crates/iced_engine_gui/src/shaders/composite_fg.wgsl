struct VertexOutput {
    @location(0) UV: vec2<f32>,
    @location(1) @interpolate(flat) FgColor: u32,
    @location(2) @interpolate(flat) UnderlinePos: u32,
    @location(3) @interpolate(flat) UnderlineColor: u32,
    @builtin(position) gl_Position: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> ScreenSize: vec4<f32>;

@vertex
fn vs_main(
    @location(0) VertexCoord: vec2<f32>,
    @location(1) UV: vec2<f32>,
    @location(2) FgColor: u32,
    @location(3) UnderlinePos: u32,
    @location(4) UnderlineColor: u32,
) -> VertexOutput {
    let gl_Position = vec4<f32>((2.0 * VertexCoord / ScreenSize.xy - 1.0) * vec2(1.0, -1.0), 0.0, 1.0);
    return VertexOutput(UV, FgColor, UnderlinePos, UnderlineColor, gl_Position);
}

struct FragmentOutput {
    @location(0) FragColor: vec4<f32>,
}

@group(1) @binding(0) 
var Atlas: texture_2d<f32>;
@group(1) @binding(1) 
var Mask: texture_2d<f32>;
@group(1) @binding(2) 
var Sampler: sampler;

@group(1) @binding(3) 
var<uniform> AtlasSize: vec4<f32>;

fn unpack_color(color: u32) -> vec4<f32> {
    return vec4<f32>(
        f32(color >> 24u) / 255.0,
        f32((color >> 16u) & 0xFFu) / 255.0,
        f32((color >> 8u) & 0xFFu) / 255.0,
        f32(color & 0xFFu) / 255.0,
    );
}


@fragment
fn fs_main(
    @location(0) UV: vec2<f32>,
    @location(1) @interpolate(flat) FgColor: u32,
    @location(2) @interpolate(flat) UnderlinePos: u32,
    @location(3) @interpolate(flat) UnderlineColor: u32,
) -> FragmentOutput {
    let underLineColorUnpacked = unpack_color(UnderlineColor);

    var fgColorUnpacked = unpack_color(FgColor);
    var textureColor = textureSample(Atlas, Sampler, UV / AtlasSize.xy);

    let alpha = textureColor.a * fgColorUnpacked.a;
    textureColor.a = alpha;
    fgColorUnpacked.a = alpha;

    let mask = textureSample(Mask, Sampler, UV / AtlasSize.xy);

    var fgColor = select(fgColorUnpacked, textureColor, mask.r == 1.0);

    let yMax = UnderlinePos & 0xFFFFu;
    let yMin = UnderlinePos >> 16u;
    fgColor = select(fgColor, underLineColorUnpacked, u32(UV.y) >= yMin && u32(UV.y) < yMax);

    return FragmentOutput(fgColor);
}