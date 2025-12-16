// Glyph atlas renderer for FKey toolbar
//
// Draws instanced quads sampling a monochrome glyph atlas (alpha mask).
// Each instance provides position/size in *clip-local pixels*.

struct Uniforms {
    // clip rect size in pixels (the viewport/scissor rect we render into)
    clip_size: vec2<f32>,
    // atlas size in pixels
    atlas_size: vec2<f32>,
    // glyph cell size in pixels
    glyph_size: vec2<f32>,
    // padding
    _pad: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> u: Uniforms;

@group(0) @binding(1)
var atlas_tex: texture_2d<f32>;

@group(0) @binding(2)
var atlas_samp: sampler;

struct VertexIn {
    @location(0) unit_pos: vec2<f32>,   // (0,0) .. (1,1)
    @location(1) unit_uv: vec2<f32>,    // (0,0) .. (1,1)

    // instance attributes
    @location(2) inst_pos: vec2<f32>,   // top-left in clip-local pixels
    @location(3) inst_size: vec2<f32>,  // size in pixels
    @location(4) fg: vec4<f32>,
    @location(5) bg: vec4<f32>,
    @location(6) glyph: u32,
    @location(7) flags: u32,
};

struct VertexOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) fg: vec4<f32>,
    @location(2) bg: vec4<f32>,
    @interpolate(flat) @location(3) flags: u32,
    @location(4) unit_uv: vec2<f32>,  // Original 0..1 UV for procedural shapes
};

fn glyph_uv_origin(g: u32) -> vec2<f32> {
    let col = f32(g % 16u);
    let row = f32(g / 16u);
    return vec2<f32>(col * u.glyph_size.x, row * u.glyph_size.y);
}

@vertex
fn vs_main(v: VertexIn) -> VertexOut {
    // pixel position within the clip rect
    let p = v.inst_pos + v.unit_pos * v.inst_size;

    // Convert pixel position to NDC within clip viewport
    let ndc_x = (p.x / u.clip_size.x) * 2.0 - 1.0;
    // flip Y (pixel coords grow down)
    let ndc_y = 1.0 - (p.y / u.clip_size.y) * 2.0;

    // atlas UV - direct mapping without inset since we use Nearest filtering
    let origin = glyph_uv_origin(v.glyph);
    let uv = (origin + v.unit_uv * u.glyph_size) / u.atlas_size;

    var out: VertexOut;
    out.pos = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.uv = uv;
    out.fg = v.fg;
    out.bg = v.bg;
    out.flags = v.flags;
    out.unit_uv = v.unit_uv;  // Pass through for procedural shapes
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    // flags bit 1: background-only (fill quad with bg color)
    if (in.flags & 2u) != 0u {
        return in.bg;
    }

    // flags bit 2: left arrow (◄) - filled triangle like SVG icon
    if (in.flags & 4u) != 0u {
        let uv = in.unit_uv;
        // Triangle proportions from arrow_left.svg: width 200, height 400 (1:2 ratio)
        // So half_height should be roughly 2x the half_width
        let tip_x = 0.3;
        let base_x = 0.7;
        let center_y = 0.5;
        let half_height = 0.4;  // taller triangle (matches SVG proportions)
        
        // Check if point is inside triangle
        let t = (uv.x - tip_x) / (base_x - tip_x);
        if uv.x >= tip_x && uv.x <= base_x {
            let y_min = center_y - half_height * t;
            let y_max = center_y + half_height * t;
            if uv.y >= y_min && uv.y <= y_max {
                return in.fg;
            }
        }
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    // flags bit 3: right arrow (►) - filled triangle like SVG icon
    if (in.flags & 8u) != 0u {
        let uv = in.unit_uv;
        // Triangle proportions from arrow_right.svg: width 200, height 400 (1:2 ratio)
        let tip_x = 0.7;
        let base_x = 0.3;
        let center_y = 0.5;
        let half_height = 0.4;  // taller triangle
        
        // Check if point is inside triangle (mirrored)
        let t = (tip_x - uv.x) / (tip_x - base_x);
        if uv.x >= base_x && uv.x <= tip_x {
            let y_min = center_y - half_height * t;
            let y_max = center_y + half_height * t;
            if uv.y >= y_min && uv.y <= y_max {
                return in.fg;
            }
        }
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    // Atlas is stored as RGBA, we use alpha as mask.
    let a = textureSample(atlas_tex, atlas_samp, in.uv).a;

    // flags bit 0: draw background
    let draw_bg = (in.flags & 1u) != 0u;

    if draw_bg {
        // Mix bg and fg based on glyph alpha
        return mix(in.bg, in.fg, a);
    }

    // No background: only draw foreground where glyph is set.
    // Keep alpha so blending works.
    return vec4<f32>(in.fg.rgb, in.fg.a * a);
}
