// CRT Terminal Shader with sliding window texture slicing
// Uses 3 texture slices: previous, current, next (relative to scroll position)
// Each slice is max 8000px tall

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
}

// Match Rust layout: scalars + vec2 + scalars + padding + vec4 + slicing data
struct Uniforms {
    time: f32,
    brightness: f32,
    contrast: f32,
    gamma: f32,
    saturation: f32,
    monitor_type: f32,
    resolution: vec2<f32>,
    curvature_x: f32,
    curvature_y: f32,
    enable_curvature: f32,
    scanline_thickness: f32,
    scanline_sharpness: f32,
    scanline_phase: f32,
    enable_scanlines: f32,
    
    noise_level: f32,
    sync_wobble: f32,
    enable_noise: f32,
    
    bloom_threshold: f32,
    bloom_radius: f32,
    bloom_intensity: f32,
    enable_bloom: f32,

    _padding: vec2<f32>,  // Padding to align background_color to 16 bytes
    background_color: vec4<f32>,
    
    // Slicing uniforms
    num_slices: f32,             // Number of texture slices (1-3)
    total_image_height: f32,     // Total height in pixels across all slices
    scroll_offset_y: f32,        // Current scroll offset in pixels
    visible_height: f32,         // Visible viewport height in pixels
    slice_heights: vec4<f32>,    // Heights of each slice (x=slice0, y=slice1, z=slice2, w=first_slice_start_y)

    // X-axis scrolling uniforms (for zoom/pan)
    scroll_offset_x: f32,        // Current horizontal scroll offset in pixels
    visible_width: f32,          // Visible viewport width in pixels  
    texture_width: f32,          // Total texture width in pixels
    _x_padding: f32,             // Padding for alignment

    // Caret uniforms (rendered in shader to avoid texture cache invalidation)
    caret_pos: vec2<f32>,        // Caret position in pixels (x, y) relative to viewport
    caret_size: vec2<f32>,       // Caret size in pixels (width, height)
    caret_visible: f32,          // 1.0 = visible, 0.0 = hidden (for blinking)
    caret_mode: f32,             // 0 = Bar, 1 = Block, 2 = Underline
    _caret_padding: vec2<f32>,   // Padding for 16-byte alignment
}

struct MonitorColor {
    color: vec4<f32>,
}

// 3 texture slots for sliding window
@group(0) @binding(0) var t_slice0: texture_2d<f32>;
@group(0) @binding(1) var t_slice1: texture_2d<f32>;
@group(0) @binding(2) var t_slice2: texture_2d<f32>;

// Sampler at binding 3
@group(0) @binding(3) var terminal_sampler: sampler;

// Uniforms at binding 4
@group(0) @binding(4) var<uniform> uniforms: Uniforms;

// Monitor color at binding 5
@group(0) @binding(5) var<uniform> monitor_color: MonitorColor;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    let x = f32(i32(vertex_index & 1u) * 4 - 1);
    let y = f32(i32(vertex_index & 2u) * 2 - 1);
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.tex_coord = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

// Get slice height from vec4 (x=slice0, y=slice1, z=slice2)
fn get_slice_height(index: i32) -> f32 {
    if index == 0 { return uniforms.slice_heights.x; }
    else if index == 1 { return uniforms.slice_heights.y; }
    else { return uniforms.slice_heights.z; }
}

// Sample from the appropriate texture slice based on pixel Y coordinate
// Uses sliding window approach: 3 slices covering current viewport area
fn sample_sliced_texture(uv: vec2<f32>) -> vec4<f32> {
    let total_height = uniforms.total_image_height;
    let tex_width = uniforms.texture_width;
    if total_height <= 0.0 || tex_width <= 0.0 {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    
    // X-axis: uv.x goes 0-1 over the visible viewport width
    // scroll_offset_x tells us where in the full texture we're looking
    // visible_width tells us how much of the texture is visible
    let visible_w = uniforms.visible_width;
    let max_scroll_x = max(0.0, tex_width - visible_w);
    let scroll_x = clamp(uniforms.scroll_offset_x, 0.0, max_scroll_x);
    
    // Screen pixel X (relative to visible viewport)
    let screen_pixel_x = uv.x * visible_w;
    
    // Document pixel X (absolute position in full texture)
    let doc_pixel_x = scroll_x + screen_pixel_x;
    
    // Convert to texture UV
    let tex_uv_x = doc_pixel_x / tex_width;
    
    // Check if we're outside the texture in X
    if tex_uv_x < 0.0 || tex_uv_x > 1.0 {
        return uniforms.background_color;
    }
    
    // Y-axis: uv.y goes 0-1 over the visible viewport
    // scroll_offset_y tells us where in the full document we're looking
    // visible_height tells us how much of the document is visible
    
    let visible_h = uniforms.visible_height;
    let max_scroll_y = max(0.0, total_height - visible_h);
    let scroll_y = clamp(uniforms.scroll_offset_y, 0.0, max_scroll_y);
    
    // Screen pixel Y (relative to visible viewport)
    let screen_pixel_y = uv.y * visible_h;
    
    // Document pixel Y (absolute position in full document)
    let doc_pixel_y = scroll_y + screen_pixel_y;
    
    // Check if we're outside the document
    if doc_pixel_y < 0.0 || doc_pixel_y >= total_height {
        return uniforms.background_color;
    }
    
    // first_slice_start_y tells us where our sliding window starts in document space
    let first_slice_start_y = uniforms.slice_heights.w;
    
    // Convert document Y to sliding window Y
    let window_y = doc_pixel_y - first_slice_start_y;
    
    // If outside our rendered window, show background
    if window_y < 0.0 {
        return uniforms.background_color;
    }
    
    // Find which slice contains this Y coordinate
    var cumulative_height: f32 = 0.0;
    let num_slices = i32(uniforms.num_slices);
    
    for (var i: i32 = 0; i < num_slices; i++) {
        let slice_height = get_slice_height(i);
        let next_cumulative = cumulative_height + slice_height;
        
        if window_y < next_cumulative || i == num_slices - 1 {
            // This is the slice we need
            let local_y = (window_y - cumulative_height) / slice_height;
            let slice_uv = vec2<f32>(tex_uv_x, clamp(local_y, 0.0, 1.0));
            
            // Sample from the appropriate texture (3 slices)
            if i == 0 { return textureSample(t_slice0, terminal_sampler, slice_uv); }
            else if i == 1 { return textureSample(t_slice1, terminal_sampler, slice_uv); }
            else { return textureSample(t_slice2, terminal_sampler, slice_uv); }
        }
        cumulative_height = next_cumulative;
    }
    
    // Fallback - outside rendered area
    return uniforms.background_color;
}

// Sample for bloom - needs to handle slicing too
fn sample_sliced_texture_for_bloom(uv: vec2<f32>) -> vec4<f32> {
    return sample_sliced_texture(uv);
}

fn adjust_color(base: vec3<f32>) -> vec3<f32> {
    var color = base;

    let g = uniforms.gamma;
    if (g > 0.0001 && abs(g - 1.0) > 0.00001) {
        color = pow(color, vec3<f32>(1.0 / g));
    }

    let bm = uniforms.brightness;
    if (abs(bm - 1.0) > 0.00001) {
        color = color * bm;
    }

    let cf = uniforms.contrast;
    if (abs(cf - 1.0) > 0.00001) {
        color = (color - vec3<f32>(0.5)) * cf + vec3<f32>(0.5);
    }

    let s = uniforms.saturation;
    if (uniforms.monitor_type < 0.5 && abs(s - 1.0) > 0.00001) {
        let gray = dot(color, vec3<f32>(0.299, 0.587, 0.114));
        color = mix(vec3<f32>(gray), color, s);
    }

    return color;
}

fn apply_curvature(uv_in: vec2<f32>) -> vec2<f32> {
    if (uniforms.enable_curvature < 0.5) {
        return uv_in;
    }

    var uv = uv_in * 2.0 - 1.0;
    let offset = abs(uv.yx) / vec2(uniforms.curvature_x, uniforms.curvature_y);
    uv = uv + uv * offset * offset;
    uv = uv * 0.5 + 0.5;
    return uv;
}

fn apply_scanlines(color_in: vec3<f32>, tex_coord: vec2<f32>) -> vec3<f32> {
    if (uniforms.enable_scanlines < 0.5) {
        return color_in;
    }

    // Determine pixel row (using resolution uniform)
    let row_f = tex_coord.y * uniforms.resolution.y + uniforms.scanline_phase * uniforms.resolution.y;
    let row = floor(row_f);

    // Alternate pattern (0 or 1)
    let is_dark_row = f32(i32(row) & 1);

    // thickness controls how deep dark rows get; map to multiplier
    // thickness=0 -> 0.9 (subtle), thickness=1 -> 0.3 (strong)
    let min_mul = mix(0.9, 0.3, uniforms.scanline_thickness);

    // sharpness controls edge softness between rows using fractional position within row
    let frac = fract(row_f);
    // For dark rows, fade toward min_mul near row center; for bright rows, keep near 1.0
    // Use a bell-ish curve via smoothstep chain
    let shape = 1.0 - smoothstep(0.0, 1.0, abs(frac - 0.5) * 2.0);
    let edge_mix = mix(1.0, min_mul, pow(shape, mix(0.5, 4.0, uniforms.scanline_sharpness)));

    // Apply only to dark rows
    let mul = mix(1.0, edge_mix, is_dark_row);

    return color_in * mul;
}

fn rand(p: vec2<f32>) -> f32 {
    // Simple hash-based pseudo-random; deterministic per frame, animated via time
    let h = dot(p + uniforms.time * 0.37, vec2<f32>(12.9898, 78.233));
    return fract(sin(h) * 43758.5453);
}

fn apply_noise(color_in: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {
    if (uniforms.enable_noise < 0.5 || uniforms.noise_level <= 0.00001) {
        return color_in;
    }
    // Two samples for a bit more variation
    let n1 = rand(uv * 1.0);
    let n2 = rand(uv * 7.13 + vec2<f32>(uniforms.time * 0.11, uniforms.time * 0.07));
    let n = (n1 + n2) * 0.5;       // 0..1
    let grain = (n - 0.5) * 2.0;   // -1..1
    // Scale noise level; reduce influence on very dark pixels (simulate phosphor response)
    let luma = dot(color_in, vec3<f32>(0.299, 0.587, 0.114));
    let attenuation = mix(0.6, 1.0, luma); // darker areas less noisy
    let strength = uniforms.noise_level * 1.2 * attenuation;
    let noisy = color_in + grain * strength * 0.15; // 0.15 base amplitude
    return clamp(noisy, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn apply_sync_wobble(uv: vec2<f32>) -> vec2<f32> {
    if (uniforms.enable_noise < 0.5 || uniforms.noise_level <= 0.00001) {
        return uv;
    }
    
    // Create time-based sync distortion
    let wobble_speed = 3.5;
    let wobble_lines = 5.0;
    
    // Vertical position affects distortion amount
    let distort_amount = sin(uv.y * wobble_lines + uniforms.time * wobble_speed) * 0.5 + 0.5;
    
    // Add some noise to make it more irregular
    let noise = rand(vec2<f32>(uv.y * 10.0, uniforms.time)) - 0.5;
    
    // Horizontal offset based on sync wobble strength
    let offset_x = (distort_amount * noise) * uniforms.sync_wobble * 0.02;
    
    return vec2<f32>(uv.x + offset_x, uv.y);
}

fn apply_bloom(uv: vec2<f32>) -> vec3<f32> {
    if (uniforms.enable_bloom < 0.5 || uniforms.bloom_intensity <= 0.001) {
        return vec3<f32>(0.0);
    }

    // Sample the original texture (before effects)
    let center_color = sample_sliced_texture_for_bloom(uv).rgb;
    let adjusted = adjust_color(center_color);
    let center_luma = dot(adjusted, vec3<f32>(0.299, 0.587, 0.114));

    // Radius in pixels
    let radius = max(uniforms.bloom_radius, 0.5);
    let px = radius / uniforms.resolution;

    // Accumulate bright samples in larger area
    var glow = vec3<f32>(0.0);
    var total_weight = 0.0;

    // Larger kernel: 25-tap (5x5 grid)
    for (var dy: f32 = -2.0; dy <= 2.0; dy = dy + 1.0) {
        for (var dx: f32 = -2.0; dx <= 2.0; dx = dx + 1.0) {
            let offset = vec2<f32>(dx, dy) * px;
            let sample_uv = clamp(uv + offset, vec2<f32>(0.0), vec2<f32>(1.0));
            
            let sample_color = sample_sliced_texture_for_bloom(sample_uv).rgb;
            let sample_adjusted = adjust_color(sample_color);
            let sample_luma = dot(sample_adjusted, vec3<f32>(0.299, 0.587, 0.114));
            
            // Distance-based weight (Gaussian)
            let dist = length(vec2<f32>(dx, dy));
            let weight = exp(-dist * dist * 0.3); // Wider falloff
            
            // Soft threshold: accumulate all pixels, weight by how bright they are
            let threshold_blend = smoothstep(uniforms.bloom_threshold - 0.1, uniforms.bloom_threshold, sample_luma);
            
            glow = glow + sample_adjusted * threshold_blend * weight;
            total_weight = total_weight + weight * threshold_blend;
        }
    }

    if (total_weight > 0.001) {
        glow = glow / total_weight;
        // Much stronger multiplier
        return glow * uniforms.bloom_intensity * 1.5;
    }

    return vec3<f32>(0.0);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Apply wobble & curvature to display UV
    let wobbled_uv = apply_sync_wobble(in.tex_coord);
    let distorted_uv = apply_curvature(wobbled_uv);

    if (distorted_uv.x < 0.0 || distorted_uv.y < 0.0 || distorted_uv.x > 1.0 || distorted_uv.y > 1.0) {
        return uniforms.background_color;
    }

    // Sample from sliced textures with scroll offset handling
    let tex_color = sample_sliced_texture(distorted_uv);
    var color = adjust_color(tex_color.rgb);

    // Draw caret by inverting pixels (rendered in shader to avoid cache invalidation)
    // caret_pos and caret_size are in normalized UV coordinates (0-1)
    if (uniforms.caret_visible > 0.5) {
        let uv = distorted_uv;
        let caret_x = uniforms.caret_pos.x;
        let caret_y = uniforms.caret_pos.y;
        let caret_w = uniforms.caret_size.x;
        let caret_h = uniforms.caret_size.y;

        var in_caret = false;

        if (uniforms.caret_mode < 0.5) {
            // Bar mode: thin vertical line on left
            let bar_width = max(0.001, caret_w * 0.15);
            in_caret = uv.x >= caret_x && uv.x < caret_x + bar_width &&
                       uv.y >= caret_y && uv.y < caret_y + caret_h;
        } else if (uniforms.caret_mode < 1.5) {
            // Block mode: full character cell
            in_caret = uv.x >= caret_x && uv.x < caret_x + caret_w &&
                       uv.y >= caret_y && uv.y < caret_y + caret_h;
        } else {
            // Underline mode: thin horizontal line at bottom
            let underline_height = max(0.001, caret_h * 0.15);
            let underline_y = caret_y + caret_h - underline_height;
            in_caret = uv.x >= caret_x && uv.x < caret_x + caret_w &&
                       uv.y >= underline_y && uv.y < caret_y + caret_h;
        }

        if (in_caret) {
            // Invert colors for caret
            color = vec3<f32>(1.0) - color;
        }
    }

    // Calculate bloom from original undistorted coordinates
    let bloom_glow = apply_bloom(distorted_uv);
    
    // Apply post-processing effects
    color = apply_scanlines(color, distorted_uv);
    color = apply_noise(color, distorted_uv);
    
    // Add bloom on top (much more visible now)
    color = color + bloom_glow;
    
    // Final clamp before monitor type conversion
    color = clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));

    if (uniforms.monitor_type < 0.5) {
        return vec4<f32>(color, tex_color.a);
    } else if (uniforms.monitor_type < 1.5) {
        let gray = dot(color, vec3<f32>(0.299, 0.587, 0.114));
        return vec4<f32>(vec3<f32>(gray), tex_color.a);
    } else {
        let gray = dot(color, vec3<f32>(0.299, 0.587, 0.114));
        let tint = monitor_color.color.rgb;
        let max_comp = max(tint.r, max(tint.g, tint.b));
        let norm_tint = select(vec3<f32>(1.0), tint / max_comp, max_comp > 0.0001);
        var mono = gray * norm_tint * 1.5;
        mono = mono + norm_tint * 0.05;
        mono = mono / (mono + vec3<f32>(1.0)) * 2.0;
        return vec4<f32>(clamp(mono, vec3<f32>(0.0), vec3<f32>(1.0)), tex_color.a);
    }
}
