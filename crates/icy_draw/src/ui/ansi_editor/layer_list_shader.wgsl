// Layer List Background Shader
// Matches the Segmented Control / FKey toolbar styling:
// - main-area background fill
// - per-row hover + selection fills
// - inner glow + border accents
//
// Foreground (text/icons/previews) is rendered by the owner-rendered list widget.

struct Uniforms {
    // Widget top-left (in physical screen coordinates)
    widget_origin: vec2<f32>,
    // Widget dimensions (physical pixels)
    widget_size: vec2<f32>,

    // Scroll offset in physical pixels
    scroll_y: f32,
    // Row height in physical pixels
    row_height: f32,

    // Row count
    row_count: u32,
    // Selected row index (0xFFFFFFFF = none)
    selected_row: u32,
    // Hovered row index (0xFFFFFFFF = none)
    hovered_row: u32,
    // Padding / flags (unused)
    _flags: u32,

    // Preview rectangle within a row (x, y, w, h) in physical pixels
    preview_rect: vec4<f32>,
    // Atlas parameters (atlas_w, atlas_h, tile_w, tile_h)
    atlas_size: vec4<f32>,
    // Atlas params (first_list_idx, count, cols, flags)
    atlas_params: vec4<u32>,

    // Preview styling
    preview_bg_color: vec4<f32>,
    preview_border_color: vec4<f32>,
    // (border_width_px, radius_px, _, _)
    preview_style: vec4<f32>,

    // Background color from theme
    bg_color: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var preview_atlas: texture_2d<f32>;

@group(0) @binding(2)
var preview_sampler: sampler;

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

    var out: VertexOutput;
    out.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    out.uv = uvs[vertex_index];
    return out;
}

// Colors (matching segmented_control_shader.wgsl)
const SELECTED_BG: vec3<f32> = vec3<f32>(0.25, 0.45, 0.7);
const HOVER_BG: vec3<f32> = vec3<f32>(0.28, 0.28, 0.35);
const BORDER_COLOR: vec3<f32> = vec3<f32>(0.35, 0.35, 0.4);
const SELECTED_BORDER: vec3<f32> = vec3<f32>(0.4, 0.6, 0.85);
const HOVER_GLOW: vec3<f32> = vec3<f32>(0.5, 0.7, 1.0);
const SEPARATOR_COLOR: vec3<f32> = vec3<f32>(0.22, 0.23, 0.26);

fn rounded_rect_sdf(p: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let r = min(radius, min(half_size.x, half_size.y));
    let d = abs(p) - half_size + vec2<f32>(r);
    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0) - r;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let pixel = input.position.xy - uniforms.widget_origin;
    let size = uniforms.widget_size;

    // Base background
    var rgb = uniforms.bg_color.rgb;
    // This widget is a background layer; render it opaque.
    // (Some themes/palettes may provide alpha=0 colors; we still want previews visible.)
    var a = 1.0;

    // Row calculations (in widget-local pixels)
    let local_y = pixel.y;
    let y_in_content = local_y + uniforms.scroll_y;

    if uniforms.row_height <= 0.5 {
        return vec4<f32>(rgb, a);
    }

    let row_f = floor(y_in_content / uniforms.row_height);
    if row_f < 0.0 {
        return vec4<f32>(rgb, a);
    }

    let row_idx = u32(row_f);
    if row_idx >= uniforms.row_count {
        return vec4<f32>(rgb, a);
    }

    let row_top = row_f * uniforms.row_height - uniforms.scroll_y;
    let row_bottom = row_top + uniforms.row_height;

    // Separator line (1px) at the bottom of each row (except the last visible pixel line)
    let sep = (row_bottom - local_y);
    if sep >= 0.0 && sep < 1.0 {
        // Slightly blend separator with background
        let sep_rgb = mix(rgb, SEPARATOR_COLOR, 0.8);
        return vec4<f32>(sep_rgb, a);
    }

    // Determine row state
    let has_hover = uniforms.hovered_row != 0xFFFFFFFFu;
    let is_hovered = has_hover && row_idx == uniforms.hovered_row;
    let is_selected = uniforms.selected_row != 0xFFFFFFFFu && row_idx == uniforms.selected_row;

    // Optional hover/selection highlight (behind preview)
    if is_hovered || is_selected {
        // Highlight rectangle inset
        let inset_x = 6.0;
        let inset_y = 2.0;
        let rect_x = inset_x;
        let rect_y = row_top + inset_y;
        let rect_w = max(size.x - inset_x * 2.0, 1.0);
        let rect_h = max(uniforms.row_height - inset_y * 2.0, 1.0);

        let center = vec2<f32>(rect_x + rect_w * 0.5, rect_y + rect_h * 0.5);
        let half = vec2<f32>(rect_w * 0.5, rect_h * 0.5);
        let radius = 3.0;

        let sdf = rounded_rect_sdf(pixel - center, half, radius);
        let mask = smoothstep(0.5, -0.5, sdf);

        // Fill color
        var fill = HOVER_BG;
        var border = BORDER_COLOR;
        var glow = HOVER_GLOW;

        if is_selected {
            fill = SELECTED_BG;
            border = SELECTED_BORDER;
            glow = SELECTED_BORDER;
        }

        // Inner glow (strongest near edges inside the highlight)
        let edge = clamp(-sdf, 0.0, 8.0);
        let glow_strength = exp(-edge * 0.9);
        let glow_alpha = 0.35 * glow_strength;

        // Border (1px)
        let border_width = 1.0;
        let inner_half = half - vec2<f32>(border_width, border_width);
        let inner_radius = max(radius - border_width, 0.0);
        let inner_sdf = rounded_rect_sdf(pixel - center, inner_half, inner_radius);
        let inner_mask = smoothstep(0.5, -0.5, inner_sdf);
        let border_mask = clamp(mask - inner_mask, 0.0, 1.0);

        // Compose
        let filled = mix(rgb, fill, mask * 0.55);
        let with_glow = mix(filled, glow, glow_alpha * mask);
        rgb = mix(with_glow, border, border_mask * 0.9);
    }

    // Preview background + border + overlay (rounded)
    let pr = uniforms.preview_rect;
    let px0 = pr.x;
    let py0 = row_top + pr.y;
    let pw = pr.z;
    let ph = pr.w;
    if pixel.x >= px0 && pixel.x <= (px0 + pw) && pixel.y >= py0 && pixel.y <= (py0 + ph) {
        let border_w = max(uniforms.preview_style.x, 0.0);
        let radius = max(uniforms.preview_style.y, 0.0);

        let center = vec2<f32>(px0 + pw * 0.5, py0 + ph * 0.5);
        let half = vec2<f32>(pw * 0.5, ph * 0.5);

        let sdf = rounded_rect_sdf(pixel - center, half, radius);
        let outer_mask = smoothstep(0.6, -0.6, sdf);

        // Border mask (outer - inner)
        let inner_half = half - vec2<f32>(border_w, border_w);
        let inner_radius = max(radius - border_w, 0.0);
        let inner_sdf = rounded_rect_sdf(pixel - center, inner_half, inner_radius);
        let inner_mask = smoothstep(0.6, -0.6, inner_sdf);
        let border_mask = clamp(outer_mask - inner_mask, 0.0, 1.0);

        // Draw preview background inside rounded rect
        let base = uniforms.preview_bg_color.rgb;
        let alt = base * 0.85;
        let check_size = 8.0;
        let cx = u32(floor((pixel.x - px0) / check_size));
        let cy = u32(floor((pixel.y - py0) / check_size));
        let checker = select(base, alt, ((cx + cy) & 1u) == 1u);
        rgb = mix(rgb, checker, inner_mask * uniforms.preview_bg_color.a);
        // Draw border
        rgb = mix(rgb, uniforms.preview_border_color.rgb, border_mask * uniforms.preview_border_color.a);

        // Sample atlas into the inner area
        let first_idx = uniforms.atlas_params.x;
        let count = uniforms.atlas_params.y;
        let cols = uniforms.atlas_params.z;

        if row_idx >= first_idx && row_idx < (first_idx + count) {
            let slot = row_idx - first_idx;
            let col = slot % cols;
            let row = slot / cols;

            let atlas_w = uniforms.atlas_size.x;
            let atlas_h = uniforms.atlas_size.y;
            let tile_w = uniforms.atlas_size.z;
            let tile_h = uniforms.atlas_size.w;

            let t = (pixel - vec2<f32>(px0, py0)) / vec2<f32>(pw, ph);
            let tile_origin = vec2<f32>(f32(col) * tile_w, f32(row) * tile_h);
            let uv = (tile_origin + t * vec2<f32>(tile_w, tile_h)) / vec2<f32>(atlas_w, atlas_h);

            let sample = textureSample(preview_atlas, preview_sampler, uv);
            rgb = mix(rgb, sample.rgb, sample.a * inner_mask);
        }
    }

    return vec4<f32>(rgb, a);
}
