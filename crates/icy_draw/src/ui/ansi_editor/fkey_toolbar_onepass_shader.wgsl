// FKey Toolbar + Glyphs (one-pass)
//
// Single pipeline that draws:
// 1) fkey toolbar background (pill, shadow, border)
// 2) all glyph quads (labels, chars, arrows) from an atlas
//
// The background is drawn as a single instance with a special flag.
// All coordinates are in *clip-local physical pixels*.

struct Uniforms {
    // clip rect size in pixels (the viewport/scissor rect we render into)
    clip_size: vec2<f32>,
    // atlas size in pixels
    atlas_size: vec2<f32>,
    // glyph cell size in pixels
    glyph_size: vec2<f32>,
    _pad0: vec2<f32>,

    // fkey toolbar
    num_slots: u32,
    hovered_slot: u32,
    hover_type: u32, // 0=slot_label, 1=slot_char, 2=nav_prev, 3=nav_next
    _pad1: u32,

    corner_radius: f32,
    _pad2: vec3<f32>,

    // background color from theme (used to clear the full widget rect)
    bg_color: vec4<f32>,

    // Layout parameters (physical pixels)
    content_start_x: f32,
    slot_width: f32,
    slot_spacing: f32,
    label_width: f32,
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
    @location(4) unit_uv: vec2<f32>,
    @location(5) p: vec2<f32>, // clip-local pixel coordinate
};

// Flags (must match Rust side)
const FLAG_DRAW_BG: u32 = 1u;
const FLAG_BG_ONLY: u32 = 2u;
const FLAG_ARROW_LEFT: u32 = 4u;
const FLAG_ARROW_RIGHT: u32 = 8u;
const FLAG_FKEY_BG: u32 = 16u;

// Design constants (match fkey_toolbar_shader.wgsl)
const SHADOW_PADDING: f32 = 6.0;
const BORDER_WIDTH: f32 = 1.0;

const SHADOW_OFFSET: vec2<f32> = vec2<f32>(2.0, 2.0);
const SHADOW_BLUR: f32 = 3.0;
const SHADOW_ALPHA: f32 = 0.4;

const BORDER_COLOR: vec3<f32> = vec3<f32>(0.35, 0.35, 0.4);

fn rounded_rect_sdf(p: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let r = min(radius, min(half_size.x, half_size.y));
    let d = abs(p) - half_size + vec2<f32>(r);
    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0) - r;
}

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
    let ndc_y = 1.0 - (p.y / u.clip_size.y) * 2.0;

    let origin = glyph_uv_origin(v.glyph);
    let uv = (origin + v.unit_uv * u.glyph_size) / u.atlas_size;

    var out: VertexOut;
    out.pos = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.uv = uv;
    out.fg = v.fg;
    out.bg = v.bg;
    out.flags = v.flags;
    out.unit_uv = v.unit_uv;
    out.p = p;
    return out;
}

fn fkey_background(pixel: vec2<f32>) -> vec4<f32> {
    let size = u.clip_size;

    // Control area inside shadow padding
    let ctrl_x = SHADOW_PADDING;
    let ctrl_y = SHADOW_PADDING;
    let ctrl_w = size.x - SHADOW_PADDING * 2.0;
    let ctrl_h = size.y - SHADOW_PADDING * 2.0;
    let ctrl_center = vec2<f32>(ctrl_x + ctrl_w * 0.5, ctrl_y + ctrl_h * 0.5);
    let ctrl_half = vec2<f32>(ctrl_w * 0.5, ctrl_h * 0.5);

    let radius = u.corner_radius;

    let ctrl_sdf = rounded_rect_sdf(pixel - ctrl_center, ctrl_half, radius);

    // ─────────────────────────────────────────────────────────────────────────
    // Outside control: draw drop shadow
    // ─────────────────────────────────────────────────────────────────────────
    if ctrl_sdf > 0.5 {
        let shadow_pos = pixel - ctrl_center - SHADOW_OFFSET;
        let shadow_sdf = rounded_rect_sdf(shadow_pos, ctrl_half, radius);
        let blur_factor = 1.0 / SHADOW_BLUR;
        let shadow_alpha = SHADOW_ALPHA * exp(-max(shadow_sdf, 0.0) * blur_factor);
        if shadow_alpha > 0.01 {
            return vec4<f32>(0.0, 0.0, 0.0, shadow_alpha);
        }
        return vec4<f32>(0.0);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Border + content masking
    // ─────────────────────────────────────────────────────────────────────────
    let outer_a = smoothstep(0.5, -0.5, ctrl_sdf);

    let inner_half = ctrl_half - vec2<f32>(BORDER_WIDTH, BORDER_WIDTH);
    let inner_radius = max(radius - BORDER_WIDTH, 0.0);
    let inner_sdf = rounded_rect_sdf(pixel - ctrl_center, inner_half, inner_radius);
    let inner_a = smoothstep(0.5, -0.5, inner_sdf);

    // Border color - neutral
    let border_rgb = BORDER_COLOR;

    // Content fill - just bg_color (no hover backgrounds needed)
    let color = u.bg_color.rgb;

    // Composite: border outside inner area, content inside
    let final_rgb = mix(border_rgb, color, inner_a);
    return vec4<f32>(final_rgb, outer_a);
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    // Background instance
    if (in.flags & FLAG_FKEY_BG) != 0u {
        return fkey_background(in.p);
    }

    // flags: background-only (fill quad with bg color)
    if (in.flags & FLAG_BG_ONLY) != 0u {
        return in.bg;
    }

    // flags: left arrow
    if (in.flags & FLAG_ARROW_LEFT) != 0u {
        let uv = in.unit_uv;
        let tip_x = 0.3;
        let base_x = 0.7;
        let center_y = 0.5;
        let half_height = 0.4;
        let t = (uv.x - tip_x) / (base_x - tip_x);
        if uv.x >= tip_x && uv.x <= base_x {
            let y_min = center_y - half_height * t;
            let y_max = center_y + half_height * t;
            if uv.y >= y_min && uv.y <= y_max {
                return in.fg;
            }
        }
        return vec4<f32>(0.0);
    }

    // flags: right arrow
    if (in.flags & FLAG_ARROW_RIGHT) != 0u {
        let uv = in.unit_uv;
        let tip_x = 0.7;
        let base_x = 0.3;
        let center_y = 0.5;
        let half_height = 0.4;
        let t = (tip_x - uv.x) / (tip_x - base_x);
        if uv.x >= base_x && uv.x <= tip_x {
            let y_min = center_y - half_height * t;
            let y_max = center_y + half_height * t;
            if uv.y >= y_min && uv.y <= y_max {
                return in.fg;
            }
        }
        return vec4<f32>(0.0);
    }

    // Atlas alpha mask
    let a = textureSample(atlas_tex, atlas_samp, in.uv).a;

    let draw_bg = (in.flags & FLAG_DRAW_BG) != 0u;

    if draw_bg {
        return mix(in.bg, in.fg, a);
    }

    return vec4<f32>(in.fg.rgb, in.fg.a * a);
}
