// Segmented Control + Glyphs (one-pass)
//
// Single pipeline that draws:
// 1) segmented control background (pill, shadow, border, selection/hover)
// 2) all glyph quads (text + char preview) from an atlas
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

    // segmented control
    num_segments: u32,
    selected_mask: u32,
    hovered_segment: u32, // 0xFFFF_FFFF = none
    _pad1: u32,

    corner_radius: f32,
    _pad2: vec3<f32>,

    // background color from theme (used to clear the full widget rect)
    bg_color: vec4<f32>,

    // segment widths (up to 8 segments, packed as 2 x vec4)
    segment_widths: array<vec4<f32>, 2>,
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
const FLAG_SEGMENTED_BG: u32 = 16u;

// Design constants (match segmented_control_shader.wgsl)
const SHADOW_PADDING: f32 = 6.0;
const BORDER_WIDTH: f32 = 1.0;

const SHADOW_OFFSET: vec2<f32> = vec2<f32>(2.0, 2.0);
const SHADOW_BLUR: f32 = 3.0;
const SHADOW_ALPHA: f32 = 0.4;

const SELECTED_BG: vec3<f32> = vec3<f32>(0.25, 0.45, 0.7);
const HOVER_BG: vec3<f32> = vec3<f32>(0.28, 0.28, 0.35);
const BORDER_COLOR: vec3<f32> = vec3<f32>(0.35, 0.35, 0.4);
const SELECTED_BORDER: vec3<f32> = vec3<f32>(0.4, 0.6, 0.85);
const HOVER_GLOW: vec3<f32> = vec3<f32>(0.5, 0.7, 1.0);
const SEPARATOR_COLOR: vec3<f32> = vec3<f32>(0.3, 0.32, 0.36);

fn rounded_rect_sdf(p: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let r = min(radius, min(half_size.x, half_size.y));
    let d = abs(p) - half_size + vec2<f32>(r);
    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0) - r;
}

// Rounded rectangle with per-corner radii (tl, tr, br, bl)
fn rounded_rect_sdf_4(p: vec2<f32>, half_size: vec2<f32>, r_tl: f32, r_tr: f32, r_br: f32, r_bl: f32) -> f32 {
    var r: f32;
    if p.x < 0.0 {
        r = select(r_bl, r_tl, p.y < 0.0);
    } else {
        r = select(r_br, r_tr, p.y < 0.0);
    }
    r = min(r, min(half_size.x, half_size.y));
    let d = abs(p) - half_size + vec2<f32>(r);
    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0) - r;
}

fn get_segment_bounds(index: u32) -> vec2<f32> {
    var x = 0.0;
    for (var i = 0u; i < index; i++) {
        x += get_segment_width(i);
    }
    return vec2<f32>(x, x + get_segment_width(index));
}

fn get_segment_width(index: u32) -> f32 {
    let array_idx = index / 4u;
    let component = index % 4u;
    let widths = u.segment_widths[array_idx];
    switch component {
        case 0u: { return widths.x; }
        case 1u: { return widths.y; }
        case 2u: { return widths.z; }
        default: { return widths.w; }
    }
}

fn get_segment_at(x: f32) -> i32 {
    var seg_x = 0.0;
    for (var i = 0u; i < u.num_segments; i++) {
        let w = get_segment_width(i);
        if i == u.num_segments - 1u {
            if x >= seg_x {
                return i32(i);
            }
        } else if x >= seg_x && x < seg_x + w {
            return i32(i);
        }
        seg_x += w;
    }
    return -1;
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

fn segmented_background(pixel: vec2<f32>) -> vec4<f32> {
    // Always clear the full widget rect to theme bg to avoid stale pixels.
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

    // Content area
    let content_x = ctrl_x + BORDER_WIDTH;
    let content_y = ctrl_y + BORDER_WIDTH;
    let content_w = ctrl_w - BORDER_WIDTH * 2.0;
    let content_h = ctrl_h - BORDER_WIDTH * 2.0;

    // Determine segment for border coloring
    let seg_x_for_border = clamp(pixel.x - content_x, 0.0, content_w - 0.001);
    let border_seg_idx = get_segment_at(seg_x_for_border);
    let has_hover = u.hovered_segment != 0xFFFFFFFFu;

    var border_rgb = BORDER_COLOR;
    if border_seg_idx >= 0 {
        let seg_u = u32(border_seg_idx);
        let hov_u = u.hovered_segment;
        let is_seg_selected = (u.selected_mask & (1u << seg_u)) != 0u;
        if is_seg_selected {
            border_rgb = SELECTED_BORDER;
        } else if has_hover && seg_u == hov_u {
            border_rgb = mix(BORDER_COLOR, HOVER_GLOW, 0.6);
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Inside control: background fill with segment highlights
    // ─────────────────────────────────────────────────────────────────────────
    var color = u.bg_color.rgb;

    let local_x = clamp(pixel.x - content_x, 0.0, content_w - 0.001);
    let local_y = pixel.y - content_y;

    let seg_idx = get_segment_at(local_x);

    if seg_idx >= 0 {
        let seg_u = u32(seg_idx);
        let bounds = get_segment_bounds(seg_u);

        // Segment fill area
        let seg_x = bounds.x;
        let is_last = seg_u == u.num_segments - 1u;
        let seg_w = select(bounds.y - bounds.x, content_w - seg_x, is_last);
        let seg_center = vec2<f32>(content_x + seg_x + seg_w * 0.5, content_y + content_h * 0.5);
        let seg_half = vec2<f32>(seg_w * 0.5, content_h * 0.5);

        // Corner radii: round only at ends
        let is_first = seg_u == 0u;
        let inner_r = max(radius - BORDER_WIDTH, 0.0);
        let small_r = 0.0;

        let r_tl = select(small_r, inner_r, is_first);
        let r_bl = select(small_r, inner_r, is_first);
        let r_tr = select(small_r, inner_r, is_last);
        let r_br = select(small_r, inner_r, is_last);

        let seg_sdf = rounded_rect_sdf_4(pixel - seg_center, seg_half, r_tl, r_tr, r_br, r_bl);

        let is_selected = (u.selected_mask & (1u << seg_u)) != 0u;
        let is_hovered = has_hover && seg_u == u.hovered_segment && !is_selected;

        let seg_a = smoothstep(0.5, -0.5, seg_sdf);

        if is_selected {
            color = mix(color, SELECTED_BG, seg_a);

            // Inner glow effect
            if seg_sdf < 0.0 {
                let edge_dist = -seg_sdf;
                let glow_width = 3.0;
                if edge_dist < glow_width {
                    let glow_t = 1.0 - edge_dist / glow_width;
                    color = mix(color, HOVER_GLOW, glow_t * 0.25);
                }

                // Subtle top highlight
                let top_dist = pixel.y - content_y;
                if top_dist < 2.0 {
                    color = mix(color, HOVER_GLOW, (1.0 - top_dist / 2.0) * 0.15);
                }
            }
        } else if is_hovered {
            color = mix(color, HOVER_BG, seg_a);

            // Subtle inner glow
            if seg_sdf < 0.0 {
                let edge_dist = -seg_sdf;
                let glow_width = 2.0;
                if edge_dist < glow_width {
                    let glow_t = 1.0 - edge_dist / glow_width;
                    color = mix(color, HOVER_GLOW, glow_t * 0.15);
                }
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Separators between segments
    // ─────────────────────────────────────────────────────────────────────────
    for (var i = 1u; i < u.num_segments; i++) {
        let bounds = get_segment_bounds(i);
        let sep_x = content_x + bounds.x;

        // Don't draw separator next to selected or hovered segments
        let prev_sel = (u.selected_mask & (1u << (i - 1u))) != 0u;
        let curr_sel = (u.selected_mask & (1u << i)) != 0u;
        let prev_hov = has_hover && i32(i) - 1 == i32(u.hovered_segment);
        let curr_hov = has_hover && i == u.hovered_segment;

        if !prev_sel && !curr_sel && !prev_hov && !curr_hov {
            let dist = abs(pixel.x - sep_x);
            if dist < 0.6 {
                let sep_h = content_h * 0.6;
                let sep_top = content_y + (content_h - sep_h) * 0.5;
                if pixel.y > sep_top && pixel.y < sep_top + sep_h {
                    let sep_t = 1.0 - dist / 0.6;
                    color = mix(color, SEPARATOR_COLOR, sep_t * 0.5);
                }
            }
        }
    }

    // Composite: border outside inner area, content inside; AA with outer edge.
    let final_rgb = mix(border_rgb, color, inner_a);
    return vec4<f32>(final_rgb, outer_a);
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    // Background instance
    if (in.flags & FLAG_SEGMENTED_BG) != 0u {
        return segmented_background(in.p);
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
