// Segmented Control Background Shader
// Matches the Tool Panel styling exactly:
// - Rounded corners only at left (first segment) and right (last segment) ends
// - Multi-layer drop shadow (same as tool panel)
// - Solid border with selection glow
// - Hover/selection highlights with inner glow
// 
// Text is rendered as a Canvas overlay on top of this shader.

struct Uniforms {
    // Widget top-left (in physical screen coordinates)
    widget_origin: vec2<f32>,
    // Widget dimensions (physical pixels)
    widget_size: vec2<f32>,
    // Number of segments
    num_segments: u32,
    // Selected segment bitmask (bit 0 = segment 0, bit 1 = segment 1, etc.)
    // Supports multi-select mode where multiple segments can be selected
    selected_mask: u32,
    // Hovered segment index (0xFFFFFFFF = none)
    hovered_segment: u32,
    // Padding / flags (unused)
    _flags: u32,
    // Corner radius for pill shape
    corner_radius: f32,
    // Time for animations
    time: f32,
    // Padding
    _padding: vec2<f32>,
    // Background color from theme
    bg_color: vec4<f32>,
    // Segment widths (up to 8 segments, packed as 2 x vec4)
    segment_widths: array<vec4<f32>, 2>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(-1.0, -1.0)
    );
    
    var uvs = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 1.0)
    );
    
    var output: VertexOutput;
    output.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    output.uv = uvs[vertex_index];
    return output;
}

// ═══════════════════════════════════════════════════════════════════════════
// Design Constants (matching Tool Panel exactly)
// ═══════════════════════════════════════════════════════════════════════════

const SHADOW_PADDING: f32 = 6.0;
const BORDER_WIDTH: f32 = 1.0;

// Drop shadow (same as tool panel)
const SHADOW_OFFSET: vec2<f32> = vec2<f32>(2.0, 2.0);
const SHADOW_BLUR: f32 = 3.0;
const SHADOW_ALPHA: f32 = 0.4;


// Colors (matching tool_panel_shader.wgsl exactly)
const SELECTED_BG: vec3<f32> = vec3<f32>(0.25, 0.45, 0.7);
const HOVER_BG: vec3<f32> = vec3<f32>(0.28, 0.28, 0.35);
const BORDER_COLOR: vec3<f32> = vec3<f32>(0.35, 0.35, 0.4);
const SELECTED_BORDER: vec3<f32> = vec3<f32>(0.4, 0.6, 0.85);
const HOVER_GLOW: vec3<f32> = vec3<f32>(0.5, 0.7, 1.0);
const SEPARATOR_COLOR: vec3<f32> = vec3<f32>(0.3, 0.32, 0.36);

// ═══════════════════════════════════════════════════════════════════════════
// SDF Functions
// ═══════════════════════════════════════════════════════════════════════════

// Standard rounded rectangle SDF
fn rounded_rect_sdf(p: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let r = min(radius, min(half_size.x, half_size.y));
    let d = abs(p) - half_size + vec2<f32>(r);
    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0) - r;
}

// Rounded rectangle with per-corner radii (tl, tr, br, bl)
fn rounded_rect_sdf_4(p: vec2<f32>, half_size: vec2<f32>, r_tl: f32, r_tr: f32, r_br: f32, r_bl: f32) -> f32 {
    // Select radius based on quadrant
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

// ═══════════════════════════════════════════════════════════════════════════
// Segment Helpers
// ═══════════════════════════════════════════════════════════════════════════

fn get_segment_width(index: u32) -> f32 {
    let array_idx = index / 4u;
    let component = index % 4u;
    let widths = uniforms.segment_widths[array_idx];
    switch component {
        case 0u: { return widths.x; }
        case 1u: { return widths.y; }
        case 2u: { return widths.z; }
        default: { return widths.w; }
    }
}

fn get_segment_bounds(index: u32) -> vec2<f32> {
    var x = 0.0;
    for (var i = 0u; i < index; i++) {
        x += get_segment_width(i);
    }
    return vec2<f32>(x, x + get_segment_width(index));
}

fn get_segment_at(x: f32) -> i32 {
    var seg_x = 0.0;
    for (var i = 0u; i < uniforms.num_segments; i++) {
        let w = get_segment_width(i);
        // If segment widths don't sum up perfectly to the content width (rounding),
        // the remaining pixels should belong to the last segment.
        if i == uniforms.num_segments - 1u {
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

// ═══════════════════════════════════════════════════════════════════════════
// Fragment Shader
// ═══════════════════════════════════════════════════════════════════════════

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let pixel = input.position.xy - uniforms.widget_origin;
    let size = uniforms.widget_size;
    
    // Control area (inside shadow padding)
    let ctrl_x = SHADOW_PADDING;
    let ctrl_y = SHADOW_PADDING;
    let ctrl_w = size.x - SHADOW_PADDING * 2.0;
    let ctrl_h = size.y - SHADOW_PADDING * 2.0;
    let ctrl_center = vec2<f32>(ctrl_x + ctrl_w * 0.5, ctrl_y + ctrl_h * 0.5);
    let ctrl_half = vec2<f32>(ctrl_w * 0.5, ctrl_h * 0.5);
    
    let radius = uniforms.corner_radius;
    
    // Main control SDF
    let ctrl_sdf = rounded_rect_sdf(pixel - ctrl_center, ctrl_half, radius);
    
    // ─────────────────────────────────────────────────────────────────────────
    // Outside control: draw drop shadow (same style as tool panel)
    // ─────────────────────────────────────────────────────────────────────────
    if ctrl_sdf > 0.5 {
        // Shadow using same parameters as tool panel
        let shadow_pos = pixel - ctrl_center - SHADOW_OFFSET;
        let shadow_sdf = rounded_rect_sdf(shadow_pos, ctrl_half, radius);
        
        // Gaussian-like blur falloff
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
    // Outer AA mask
    let outer_a = smoothstep(0.5, -0.5, ctrl_sdf);

    // Inner AA mask (content area inside the border)
    let inner_half = ctrl_half - vec2<f32>(BORDER_WIDTH, BORDER_WIDTH);
    let inner_radius = max(radius - BORDER_WIDTH, 0.0);
    let inner_sdf = rounded_rect_sdf(pixel - ctrl_center, inner_half, inner_radius);
    let inner_a = smoothstep(0.5, -0.5, inner_sdf);

    // Content area (inside the border)
    // Keep X inside the border so segment boundaries line up with the overlay.
    // Inset Y/H like `fkey_toolbar_shader.wgsl` so the border consumes 1px top/bottom.
    let content_x = ctrl_x + BORDER_WIDTH;
    let content_y = ctrl_y + BORDER_WIDTH;
    let content_w = ctrl_w - BORDER_WIDTH * 2.0;
    let content_h = ctrl_h - BORDER_WIDTH * 2.0;

    // Determine which segment a border pixel belongs to (split frame into segment sections)
    let seg_x_for_border = clamp(pixel.x - content_x, 0.0, content_w - 0.001);
    let border_seg_idx = get_segment_at(seg_x_for_border);
    let has_hover = uniforms.hovered_segment != 0xFFFFFFFFu;

    var border_rgb = BORDER_COLOR;
    if border_seg_idx >= 0 {
        let seg_u = u32(border_seg_idx);
        let hov_u = uniforms.hovered_segment;
        // Check if segment is selected using bitmask
        let is_seg_selected = (uniforms.selected_mask & (1u << seg_u)) != 0u;
        if is_seg_selected {
            border_rgb = SELECTED_BORDER;
        } else if has_hover && seg_u == hov_u {
            border_rgb = mix(BORDER_COLOR, HOVER_GLOW, 0.6);
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Inside control: background fill with segment highlights
    // ─────────────────────────────────────────────────────────────────────────
    var color = uniforms.bg_color.rgb;

    // Local position within content area
    // Clamp x into the content range to avoid falling outside due to float rounding.
    let local_x = clamp(pixel.x - content_x, 0.0, content_w - 0.001);
    let local_y = pixel.y - content_y;

    // Which segment?
    let seg_idx = get_segment_at(local_x);
    
    if seg_idx >= 0 {
        let seg_u = u32(seg_idx);
        let bounds = get_segment_bounds(seg_u);
        
        // Segment fill area (full segment, no inset)
        let seg_x = bounds.x;
        // For last segment, extend to full content width to avoid gaps
        let is_last = seg_u == uniforms.num_segments - 1u;
        let seg_w = select(bounds.y - bounds.x, content_w - seg_x, is_last);
        let seg_center = vec2<f32>(content_x + seg_x + seg_w * 0.5, content_y + content_h * 0.5);
        let seg_half = vec2<f32>(seg_w * 0.5, content_h * 0.5);
        
        // Corner radii: round only at ends
        let is_first = seg_u == 0u;
        let inner_r = max(radius - BORDER_WIDTH, 0.0);
        let small_r = 0.0;  // Sharp corners for middle segments
        
        // tl, tr, br, bl
        let r_tl = select(small_r, inner_r, is_first);
        let r_bl = select(small_r, inner_r, is_first);
        let r_tr = select(small_r, inner_r, is_last);
        let r_br = select(small_r, inner_r, is_last);
        
        let seg_sdf = rounded_rect_sdf_4(pixel - seg_center, seg_half, r_tl, r_tr, r_br, r_bl);
        
        // Check if this segment is selected using bitmask
        let is_selected = (uniforms.selected_mask & (1u << seg_u)) != 0u;
        let is_hovered = has_hover && seg_u == uniforms.hovered_segment && !is_selected;

        // Anti-aliased segment fill (fixes tiny gaps at the rightmost segment due to rounding)
        let seg_a = smoothstep(0.5, -0.5, seg_sdf);

        if is_selected {
            // Selected: fill with selected color
            color = mix(color, SELECTED_BG, seg_a);

            // Inner glow effect (brighter at edges) - only inside
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
            // Hover: fill with hover color (same as tool panel)
            color = mix(color, HOVER_BG, seg_a);

            // Subtle inner glow - only inside
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
    for (var i = 1u; i < uniforms.num_segments; i++) {
        let bounds = get_segment_bounds(i);
        let sep_x = content_x + bounds.x;
        
        // Don't draw separator next to selected or hovered segments
        let prev_sel = (uniforms.selected_mask & (1u << (i - 1u))) != 0u;
        let curr_sel = (uniforms.selected_mask & (1u << i)) != 0u;
        let prev_hov = has_hover && i32(i) - 1 == i32(uniforms.hovered_segment);
        let curr_hov = has_hover && i == uniforms.hovered_segment;
        
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
