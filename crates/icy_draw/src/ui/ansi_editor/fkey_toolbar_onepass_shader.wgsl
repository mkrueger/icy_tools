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
    @location(0) fg: vec4<f32>,
    @location(1) bg: vec4<f32>,
    @interpolate(flat) @location(2) flags: u32,
    @interpolate(flat) @location(3) glyph: u32,
    @interpolate(flat) @location(4) inst_pos: vec2<f32>,
    @interpolate(flat) @location(5) inst_size: vec2<f32>,
    @location(6) unit_uv: vec2<f32>,
    @location(7) p: vec2<f32>, // clip-local pixel coordinate
};

// Flags (must match Rust side)
const FLAG_DRAW_BG: u32 = 1u;
const FLAG_BG_ONLY: u32 = 2u;
const FLAG_ARROW_LEFT: u32 = 4u;
const FLAG_ARROW_RIGHT: u32 = 8u;
const FLAG_FKEY_BG: u32 = 16u;
const FLAG_LINEAR_SAMPLE: u32 = 32u;

// Design constants (match fkey_toolbar_shader.wgsl)
const SHADOW_PADDING: f32 = 6.0;
const BORDER_WIDTH: f32 = 1.0;

const SHADOW_OFFSET: vec2<f32> = vec2<f32>(1.5, 2.0);
const SHADOW_BLUR: f32 = 2.5;
const SHADOW_ALPHA: f32 = 0.25;

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

    var out: VertexOut;
    out.pos = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.fg = v.fg;
    out.bg = v.bg;
    out.flags = v.flags;
    out.glyph = v.glyph;
    out.inst_pos = v.inst_pos;
    out.inst_size = v.inst_size;
    out.unit_uv = v.unit_uv;
    out.p = p;
    return out;
}

fn atlas_alpha(p: vec2<f32>, inst_pos: vec2<f32>, inst_size: vec2<f32>, glyph: u32) -> f32 {
    // Pixel-perfect nearest sampling:
    // Floor both the fragment position and inst_pos to get exact pixel indices.
    let pixel = floor(p);
    let origin = glyph_uv_origin(glyph);
    let local = pixel - floor(inst_pos); // pixel index within the quad

    // Convert local pixel coordinate to a texel coordinate inside the glyph cell.
    // Using floor() makes the replication stable for integer scales.
    var texel = floor(local * (u.glyph_size / max(inst_size, vec2<f32>(1.0))));
    texel = clamp(texel, vec2<f32>(0.0), u.glyph_size - vec2<f32>(1.0));

    let uv = (origin + texel + vec2<f32>(0.5, 0.5)) / u.atlas_size;
    return textureSample(atlas_tex, atlas_samp, uv).a;
}

fn atlas_alpha_linear(p: vec2<f32>, inst_pos: vec2<f32>, inst_size: vec2<f32>, glyph: u32) -> f32 {
    // Linear (bilinear) sampling for smooth scaling of label chars.
    let origin = glyph_uv_origin(glyph);
    let local = p - inst_pos; // sub-pixel position within the quad

    // Map local position to texel coordinate (continuous, not floored)
    let texel = local * (u.glyph_size / max(inst_size, vec2<f32>(1.0)));
    let texel_clamped = clamp(texel, vec2<f32>(0.0), u.glyph_size - vec2<f32>(1.0));

    let uv = (origin + texel_clamped + vec2<f32>(0.5, 0.5)) / u.atlas_size;
    return textureSample(atlas_tex, atlas_samp, uv).a;
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
    // Outside control: draw subtle drop shadow (right/bottom only)
    // ─────────────────────────────────────────────────────────────────────────
    if ctrl_sdf > 0.5 {
        // Shadow shape is offset to bottom-right
        let shadow_pos = pixel - ctrl_center - SHADOW_OFFSET;
        let shadow_sdf = rounded_rect_sdf(shadow_pos, ctrl_half, radius);
        
        // Only draw shadow if we're close to the shadow shape edge
        // AND the pixel is in the shadow direction (right or below the control)
        let rel_to_ctrl = pixel - ctrl_center;
        let in_shadow_region = rel_to_ctrl.x > -ctrl_half.x * 0.5 || rel_to_ctrl.y > -ctrl_half.y * 0.5;
        
        if in_shadow_region && shadow_sdf < SHADOW_BLUR && shadow_sdf > -1.0 {
            let shadow_t = 1.0 - smoothstep(-1.0, SHADOW_BLUR, shadow_sdf);
            let shadow_alpha = SHADOW_ALPHA * shadow_t;
            if shadow_alpha > 0.005 {
                return vec4<f32>(0.0, 0.0, 0.0, shadow_alpha);
            }
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

    // Atlas alpha mask - use linear sampling for labels, nearest for preview glyphs
    var a: f32;
    if (in.flags & FLAG_LINEAR_SAMPLE) != 0u {
        a = atlas_alpha_linear(in.p, in.inst_pos, in.inst_size, in.glyph);
    } else {
        a = atlas_alpha(in.p, in.inst_pos, in.inst_size, in.glyph);
    }

    let draw_bg = (in.flags & FLAG_DRAW_BG) != 0u;

    if draw_bg {
        return mix(in.bg, in.fg, a);
    }

    return vec4<f32>(in.fg.rgb, in.fg.a * a);
}