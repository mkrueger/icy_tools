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

    // Marker uniforms (raster grid and guide crosshair)
    raster_spacing: vec2<f32>,   // Raster grid spacing in pixels (cell_width, cell_height), (0,0) = disabled
    _raster_spacing_padding: vec2<f32>,  // Padding to align raster_color to 16-byte boundary
    raster_color: vec4<f32>,     // Raster grid color (RGBA)
    raster_alpha: f32,           // Raster grid alpha (0.0 - 1.0)
    raster_enabled: f32,         // 1.0 = enabled, 0.0 = disabled
    _raster_padding: vec2<f32>,  // Padding for 16-byte alignment

    guide_pos: vec2<f32>,        // Guide crosshair position in pixels (x, y), negative = disabled
    _guide_pos_padding: vec2<f32>,  // Padding to align guide_color to 16-byte boundary
    guide_color: vec4<f32>,      // Guide crosshair color (RGBA)
    guide_alpha: f32,            // Guide crosshair alpha (0.0 - 1.0)
    guide_enabled: f32,          // 1.0 = enabled, 0.0 = disabled
    _marker_padding: vec2<f32>,  // Padding for 16-byte alignment

    // Reference image uniforms
    ref_image_enabled: f32,      // 1.0 = enabled, 0.0 = disabled
    ref_image_alpha: f32,        // Alpha/opacity (0.0 - 1.0)
    ref_image_mode: f32,         // 0 = Stretch, 1 = Original, 2 = Tile
    _ref_padding: f32,           // Padding for alignment
    ref_image_offset: vec2<f32>, // Offset in pixels (x, y)
    ref_image_scale: vec2<f32>,  // Scale factor (x, y) - for Original mode
    ref_image_size: vec2<f32>,   // Reference image size in pixels (width, height)
    _ref_padding2: vec2<f32>,    // Padding for 16-byte alignment

    // Layer bounds uniforms (for showing current layer border)
    layer_rect: vec4<f32>,       // Layer bounds rectangle (x, y, x+width, y+height) in document pixels
    layer_color: vec4<f32>,      // Layer bounds border color (RGBA)
    layer_enabled: f32,          // 1.0 = enabled, 0.0 = disabled
    _layer_padding1: f32,        // Padding
    _layer_padding2: f32,        // Padding
    _layer_padding3: f32,        // Padding (completes 16-byte alignment)

    // Selection uniforms (for highlighting selected area)
    selection_rect: vec4<f32>,   // Selection rectangle (x, y, x+width, y+height) in document pixels
    selection_enabled: f32,      // 1.0 = enabled, 0.0 = disabled
    selection_mask_enabled: f32, // 1.0 = use texture mask, 0.0 = use rectangle only
    _selection_padding1: f32,    // Padding
    _selection_padding2: f32,    // Padding (completes 16-byte alignment)

    // Font dimensions for selection mask sampling
    font_width: f32,             // Font width in pixels
    font_height: f32,            // Font height in pixels
    selection_mask_size: vec2<f32>, // Selection mask size in cells (width, height)

    // Terminal area within the full viewport (for rendering selection outside document bounds)
    terminal_area: vec4<f32>,    // (start_x, start_y, end_x, end_y) in normalized UV coordinates
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

// Reference image texture at binding 6
@group(0) @binding(6) var t_reference_image: texture_2d<f32>;

// Selection mask texture at binding 7
@group(0) @binding(7) var t_selection_mask: texture_2d<f32>;

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

// Render pixels outside the terminal area (but inside the widget)
// Only draws selection rectangle borders - no terminal content
fn render_outside_terminal(doc_pixel: vec2<f32>, viewport_uv: vec2<f32>) -> vec4<f32> {
    var color = uniforms.background_color;
    
    // Check if we should draw selection rectangle border
    if (uniforms.selection_enabled > 0.5 && uniforms.selection_mask_enabled < 0.5) {
        // Rectangle selection mode - draw marching ants border
        let sel_left = uniforms.selection_rect.x;
        let sel_top = uniforms.selection_rect.y;
        let sel_right = uniforms.selection_rect.z;
        let sel_bottom = uniforms.selection_rect.w;
        
        // Border thickness in pixels
        let border_thickness = 1.5;
        
        // Check if on border (within threshold of selection edges)
        let on_left_edge = abs(doc_pixel.x - sel_left) < border_thickness && 
                           doc_pixel.y >= sel_top - border_thickness && doc_pixel.y <= sel_bottom + border_thickness;
        let on_right_edge = abs(doc_pixel.x - sel_right) < border_thickness && 
                            doc_pixel.y >= sel_top - border_thickness && doc_pixel.y <= sel_bottom + border_thickness;
        let on_top_edge = abs(doc_pixel.y - sel_top) < border_thickness && 
                          doc_pixel.x >= sel_left - border_thickness && doc_pixel.x <= sel_right + border_thickness;
        let on_bottom_edge = abs(doc_pixel.y - sel_bottom) < border_thickness && 
                             doc_pixel.x >= sel_left - border_thickness && doc_pixel.x <= sel_right + border_thickness;
        
        if (on_left_edge || on_right_edge || on_top_edge || on_bottom_edge) {
            // Outside the terminal we only render a solid white selection border.
            color = vec4<f32>(1.0, 1.0, 1.0, 1.0);
        }
    }
    
    return color;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // The viewport now covers the entire widget area, not just the terminal.
    // terminal_area defines where the actual terminal is within the viewport:
    // (start_x, start_y, end_x, end_y) in normalized UV coordinates (0-1)
    let term_start = uniforms.terminal_area.xy;
    let term_end = uniforms.terminal_area.zw;
    let term_size = term_end - term_start;
    
    // Transform viewport UV to terminal UV
    // viewport_uv is in full widget space, terminal_uv is in terminal space (0-1)
    let viewport_uv = in.tex_coord;
    let terminal_uv = (viewport_uv - term_start) / term_size;
    
    // Check if we're inside the terminal area
    let inside_terminal = viewport_uv.x >= term_start.x && viewport_uv.x <= term_end.x &&
                          viewport_uv.y >= term_start.y && viewport_uv.y <= term_end.y;
    
    // Apply wobble & curvature to terminal UV (only meaningful inside terminal)
    let wobbled_uv = apply_sync_wobble(terminal_uv);
    let distorted_uv = apply_curvature(wobbled_uv);
    
    // Scroll and visible dimensions
    let visible_w = uniforms.visible_width;
    let visible_h = uniforms.visible_height;
    let scroll_x = uniforms.scroll_offset_x;
    let scroll_y = uniforms.scroll_offset_y;
    
    // Calculate document pixel position for OUTSIDE terminal (using terminal_uv)
    // This is used for render_outside_terminal
    let outside_screen_pixel = terminal_uv * vec2<f32>(visible_w, visible_h);
    let outside_doc_pixel = outside_screen_pixel + vec2<f32>(scroll_x, scroll_y);

    // If outside terminal area, render background with selection overlay only
    if (!inside_terminal) {
        return render_outside_terminal(outside_doc_pixel, viewport_uv);
    }

    // If UV is outside 0-1 range after curvature (outside curved screen)
    if (distorted_uv.x < 0.0 || distorted_uv.y < 0.0 || distorted_uv.x > 1.0 || distorted_uv.y > 1.0) {
        return uniforms.background_color;
    }

    // Calculate document pixel position for INSIDE terminal (using distorted_uv)
    // This is used for all effects: selection, layer bounds, caret, etc.
    let screen_pixel = distorted_uv * vec2<f32>(visible_w, visible_h);
    let doc_pixel = screen_pixel + vec2<f32>(scroll_x, scroll_y);

    // Sample from sliced textures with scroll offset handling
    let tex_color = sample_sliced_texture(distorted_uv);
    var color = adjust_color(tex_color.rgb);

    // Blend reference image (if enabled)
    // Uses doc_pixel calculated at the beginning of fs_main
    if (uniforms.ref_image_enabled > 0.5) {
        // Apply offset
        let adjusted_pixel = doc_pixel - uniforms.ref_image_offset;
        
        // Calculate reference image UV based on mode
        var ref_uv: vec2<f32>;
        var in_bounds = true;
        
        if (uniforms.ref_image_mode < 0.5) {
            // Mode 0: Stretch - stretch reference image to cover entire document
            let doc_size = vec2<f32>(uniforms.texture_width, uniforms.total_image_height);
            ref_uv = adjusted_pixel / doc_size;
            in_bounds = ref_uv.x >= 0.0 && ref_uv.x <= 1.0 && ref_uv.y >= 0.0 && ref_uv.y <= 1.0;
        } else if (uniforms.ref_image_mode < 1.5) {
            // Mode 1: Original - display at original size with scale
            let scaled_size = uniforms.ref_image_size * uniforms.ref_image_scale;
            ref_uv = adjusted_pixel / scaled_size;
            in_bounds = ref_uv.x >= 0.0 && ref_uv.x <= 1.0 && ref_uv.y >= 0.0 && ref_uv.y <= 1.0;
        } else {
            // Mode 2: Tile - tile the reference image
            let scaled_size = uniforms.ref_image_size * uniforms.ref_image_scale;
            ref_uv = fract(adjusted_pixel / scaled_size);
            in_bounds = true; // Always in bounds for tiling
        }
        
        if (in_bounds) {
            let ref_color = textureSample(t_reference_image, terminal_sampler, ref_uv);
            // Blend with alpha: result = ref * alpha + color * (1 - alpha * ref_alpha)
            let blend_alpha = uniforms.ref_image_alpha * ref_color.a;
            color = mix(color, ref_color.rgb, blend_alpha);
        }
    }

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

    // Draw raster grid (Moebius-style: dashed vertical/horizontal lines)
    // raster_spacing defines the grid cell size in pixels
    // Lines are drawn at every grid interval, with scrolling taken into account
    if (uniforms.raster_enabled > 0.5) {
        let raster_w = uniforms.raster_spacing.x;
        let raster_h = uniforms.raster_spacing.y;
        
        if (raster_w > 0.0 && raster_h > 0.0) {
            // Use doc_pixel calculated at the beginning of fs_main
            
            // Check if we're on a grid line
            // Use modulo to find position within grid cell
            let cell_x = doc_pixel.x % raster_w;
            let cell_y = doc_pixel.y % raster_h;
            
            // Create dotted/short-dash pattern like Moebius (2 pixels on, 2 pixels off)
            let dash_length = 2.0;
            let dash_pattern_x = (doc_pixel.y % (dash_length * 2.0)) < dash_length;
            let dash_pattern_y = (doc_pixel.x % (dash_length * 2.0)) < dash_length;
            
            // Draw line if at grid edge (1 pixel thick)
            let line_thickness = 1.0;
            let on_vertical_line = cell_x < line_thickness && dash_pattern_x;
            let on_horizontal_line = cell_y < line_thickness && dash_pattern_y;
            
            if (on_vertical_line || on_horizontal_line) {
                // Use "difference" blending: invert pixels so lines are always visible
                // On dark background -> bright line, on bright background -> dark line
                let inverted = vec3<f32>(1.0) - color;
                color = mix(color, inverted, uniforms.raster_alpha);
            }
        }
    }

    // Draw guide (Moebius-style: dashed border at right and bottom edge)
    // guide_pos defines the boundary size in pixels (width, height)
    if (uniforms.guide_enabled > 0.5) {
        let guide_w = uniforms.guide_pos.x;
        let guide_h = uniforms.guide_pos.y;
        
        if (guide_w > 0.0 && guide_h > 0.0) {
            // Use doc_pixel calculated at the beginning of fs_main
            
            // Create dashed pattern (4 pixels on, 4 pixels off)
            let dash_length = 4.0;
            let dash_pattern_x = (doc_pixel.y % (dash_length * 2.0)) < dash_length;
            let dash_pattern_y = (doc_pixel.x % (dash_length * 2.0)) < dash_length;
            
            // Draw vertical line at guide_w (right edge of guide area)
            let line_thickness = 1.0;
            let on_right_border = abs(doc_pixel.x - guide_w) < line_thickness && 
                                  doc_pixel.y >= 0.0 && doc_pixel.y <= guide_h &&
                                  dash_pattern_x;
            
            // Draw horizontal line at guide_h (bottom edge of guide area)
            let on_bottom_border = abs(doc_pixel.y - guide_h) < line_thickness && 
                                   doc_pixel.x >= 0.0 && doc_pixel.x <= guide_w &&
                                   dash_pattern_y;
            
            if (on_right_border || on_bottom_border) {
                // Use "difference" blending: invert pixels so lines are always visible
                // On dark background -> bright line, on bright background -> dark line
                let inverted = vec3<f32>(1.0) - color;
                color = mix(color, inverted, uniforms.guide_alpha);
            }
        }
    }


    let layer_left = uniforms.layer_rect.x;
    let layer_top = uniforms.layer_rect.y;
    let layer_right = uniforms.layer_rect.z;
    let layer_bottom = uniforms.layer_rect.w;
    
    // Draw layer bounds (dashed border around current layer)
    // layer_rect contains (x, y, x+width, y+height) in document pixel coordinates
    // Inside selection: marching ants (black/white animated) - drawn if selection is active
    // Outside selection: colored dashed border - only if layer_enabled is on
    // Only process if layer_enabled is on OR selection is active
    if (uniforms.layer_enabled > 0.5 || uniforms.selection_enabled > 0.5) {
        // Line thickness (1 pixel)
        let line_thickness = 1.0;
        
        // Check if we're on the layer border
        let in_y_range = doc_pixel.y >= layer_top && doc_pixel.y <= layer_bottom;
        let in_x_range = doc_pixel.x >= layer_left && doc_pixel.x <= layer_right;
        
        // Left border
        let on_left_border = abs(doc_pixel.x - layer_left) < line_thickness && in_y_range;
        // Right border  
        let on_right_border = abs(doc_pixel.x - layer_right) < line_thickness && in_y_range;
        // Top border
        let on_top_border = abs(doc_pixel.y - layer_top) < line_thickness && in_x_range;
        // Bottom border
        let on_bottom_border = abs(doc_pixel.y - layer_bottom) < line_thickness && in_x_range;
        
        let on_layer_border = on_left_border || on_right_border || on_top_border || on_bottom_border;
        
        if (on_layer_border) {
            // Check if we're inside the selection rectangle
            let sel_left = uniforms.selection_rect.x;
            let sel_top = uniforms.selection_rect.y;
            let sel_right = uniforms.selection_rect.z;
            let sel_bottom = uniforms.selection_rect.w;
            
            let in_selection = uniforms.selection_enabled > 0.5 &&
                                doc_pixel.x >= sel_left && doc_pixel.x < sel_right &&
                                doc_pixel.y >= sel_top && doc_pixel.y < sel_bottom;
            
            // Create dash pattern - use position along the edge
            // Vertical edges (left/right) use Y, horizontal edges (top/bottom) use X
            let dash_length = 4.0;
            var edge_pos = 0.0;
            if (on_left_border || on_right_border) {
                edge_pos = doc_pixel.y;
            } else {
                edge_pos = doc_pixel.x;
            }
            
            if (in_selection) {
                // Inside selection: marching ants (animated black/white)
                let time_offset = uniforms.time * 8.0;
                let dash_phase = floor((edge_pos + time_offset) / dash_length);
                let is_white = (dash_phase % 2.0) == 0.0;
                
                if (is_white) {
                    color = vec3<f32>(1.0, 1.0, 1.0);  // White
                } else {
                    color = vec3<f32>(0.0, 0.0, 0.0);  // Black
                }
            } else if (uniforms.layer_enabled > 0.5) {
                // Outside selection: colored dashed border (static)
                // Only draw if layer_enabled is on
                let dash_phase = floor(edge_pos / dash_length);
                let is_color = (dash_phase % 2.0) == 0.0;
                
                if (is_color) {
                    color = uniforms.layer_color.rgb;
                } else {
                    color = vec3<f32>(0.0, 0.0, 0.0);  // Black
                }
            }
            // If layer_enabled is off AND not in selection: don't draw anything
        }
    }

    // Track if we're drawing selection border (to skip post-processing effects)
    var on_selection_border = false;

    // Draw selection with animated marching ants border (ported from old egui version)
    // selection_rect contains (x, y, x+width, y+height) in document pixel coordinates
    // Uses doc_pixel and screen_pixel calculated at the beginning of fs_main
    if (uniforms.selection_enabled > 0.5) {
        let sel_left = uniforms.selection_rect.x;
        let sel_top = uniforms.selection_rect.y;
        let sel_right = uniforms.selection_rect.z;
        let sel_bottom = uniforms.selection_rect.w;
        
        // Check if inside the current layer bounds (for animated vs static marching ants)
        // layer_rect is (x, y, x+width, y+height) where y increases downward
        let in_layer = doc_pixel.x >= layer_left && doc_pixel.x < layer_right &&
                       doc_pixel.y >= layer_top && doc_pixel.y < layer_bottom;

        // Helper function inline: sample selection mask at cell position
        // Returns true if cell is selected
        let cell_x = floor(doc_pixel.x / uniforms.font_width);
        let cell_y = floor(doc_pixel.y / uniforms.font_height);
        
        // Sample current pixel and its 8 neighbors from the selection mask
        var sel_current = false;
        var sel_up = false;
        var sel_down = false;
        var sel_left_n = false;
        var sel_right_n = false;
        var sel_up_left = false;
        var sel_up_right = false;
        var sel_down_left = false;
        var sel_down_right = false;
        
        if (uniforms.selection_mask_enabled > 0.5) {
            // Sample at cell level (the mask has one pixel per cell)
            let mask_w = uniforms.selection_mask_size.x;
            let mask_h = uniforms.selection_mask_size.y;
            
            // Current cell
            if (cell_x >= 0.0 && cell_x < mask_w && cell_y >= 0.0 && cell_y < mask_h) {
                let uv = vec2<f32>((cell_x + 0.5) / mask_w, (cell_y + 0.5) / mask_h);
                sel_current = textureSample(t_selection_mask, terminal_sampler, uv).r > 0.5;
            }
            
            // Up neighbor
            if (cell_x >= 0.0 && cell_x < mask_w && cell_y - 1.0 >= 0.0 && cell_y - 1.0 < mask_h) {
                let uv = vec2<f32>((cell_x + 0.5) / mask_w, (cell_y - 0.5) / mask_h);
                sel_up = textureSample(t_selection_mask, terminal_sampler, uv).r > 0.5;
            }
            
            // Down neighbor
            if (cell_x >= 0.0 && cell_x < mask_w && cell_y + 1.0 >= 0.0 && cell_y + 1.0 < mask_h) {
                let uv = vec2<f32>((cell_x + 0.5) / mask_w, (cell_y + 1.5) / mask_h);
                sel_down = textureSample(t_selection_mask, terminal_sampler, uv).r > 0.5;
            }
            
            // Left neighbor
            if (cell_x - 1.0 >= 0.0 && cell_x - 1.0 < mask_w && cell_y >= 0.0 && cell_y < mask_h) {
                let uv = vec2<f32>((cell_x - 0.5) / mask_w, (cell_y + 0.5) / mask_h);
                sel_left_n = textureSample(t_selection_mask, terminal_sampler, uv).r > 0.5;
            }
            
            // Right neighbor
            if (cell_x + 1.0 >= 0.0 && cell_x + 1.0 < mask_w && cell_y >= 0.0 && cell_y < mask_h) {
                let uv = vec2<f32>((cell_x + 1.5) / mask_w, (cell_y + 0.5) / mask_h);
                sel_right_n = textureSample(t_selection_mask, terminal_sampler, uv).r > 0.5;
            }
            
            // Diagonal neighbors
            if (cell_x - 1.0 >= 0.0 && cell_x - 1.0 < mask_w && cell_y - 1.0 >= 0.0 && cell_y - 1.0 < mask_h) {
                let uv = vec2<f32>((cell_x - 0.5) / mask_w, (cell_y - 0.5) / mask_h);
                sel_up_left = textureSample(t_selection_mask, terminal_sampler, uv).r > 0.5;
            }
            if (cell_x + 1.0 >= 0.0 && cell_x + 1.0 < mask_w && cell_y - 1.0 >= 0.0 && cell_y - 1.0 < mask_h) {
                let uv = vec2<f32>((cell_x + 1.5) / mask_w, (cell_y - 0.5) / mask_h);
                sel_up_right = textureSample(t_selection_mask, terminal_sampler, uv).r > 0.5;
            }
            if (cell_x - 1.0 >= 0.0 && cell_x - 1.0 < mask_w && cell_y + 1.0 >= 0.0 && cell_y + 1.0 < mask_h) {
                let uv = vec2<f32>((cell_x - 0.5) / mask_w, (cell_y + 1.5) / mask_h);
                sel_down_left = textureSample(t_selection_mask, terminal_sampler, uv).r > 0.5;
            }
            if (cell_x + 1.0 >= 0.0 && cell_x + 1.0 < mask_w && cell_y + 1.0 >= 0.0 && cell_y + 1.0 < mask_h) {
                let uv = vec2<f32>((cell_x + 1.5) / mask_w, (cell_y + 1.5) / mask_h);
                sel_down_right = textureSample(t_selection_mask, terminal_sampler, uv).r > 0.5;
            }
        } else {
            // Rectangle-based selection - check if pixel is in rectangle
            sel_current = doc_pixel.x >= sel_left && doc_pixel.x < sel_right &&
                         doc_pixel.y >= sel_top && doc_pixel.y < sel_bottom;
            // For rectangle mode, neighbors at +/-1 pixel
            sel_up = (doc_pixel.y - 1.0) >= sel_top && (doc_pixel.y - 1.0) < sel_bottom &&
                    doc_pixel.x >= sel_left && doc_pixel.x < sel_right;
            sel_down = (doc_pixel.y + 1.0) >= sel_top && (doc_pixel.y + 1.0) < sel_bottom &&
                      doc_pixel.x >= sel_left && doc_pixel.x < sel_right;
            sel_left_n = (doc_pixel.x - 1.0) >= sel_left && (doc_pixel.x - 1.0) < sel_right &&
                        doc_pixel.y >= sel_top && doc_pixel.y < sel_bottom;
            sel_right_n = (doc_pixel.x + 1.0) >= sel_left && (doc_pixel.x + 1.0) < sel_right &&
                         doc_pixel.y >= sel_top && doc_pixel.y < sel_bottom;
            sel_up_left = (doc_pixel.x - 1.0) >= sel_left && (doc_pixel.x - 1.0) < sel_right &&
                         (doc_pixel.y - 1.0) >= sel_top && (doc_pixel.y - 1.0) < sel_bottom;
            sel_up_right = (doc_pixel.x + 1.0) >= sel_left && (doc_pixel.x + 1.0) < sel_right &&
                          (doc_pixel.y - 1.0) >= sel_top && (doc_pixel.y - 1.0) < sel_bottom;
            sel_down_left = (doc_pixel.x - 1.0) >= sel_left && (doc_pixel.x - 1.0) < sel_right &&
                           (doc_pixel.y + 1.0) >= sel_top && (doc_pixel.y + 1.0) < sel_bottom;
            sel_down_right = (doc_pixel.x + 1.0) >= sel_left && (doc_pixel.x + 1.0) < sel_right &&
                            (doc_pixel.y + 1.0) >= sel_top && (doc_pixel.y + 1.0) < sel_bottom;
        }
        
        // Check if current pixel is on the edge of selection
        let is_on_edge = sel_current && (!sel_up || !sel_down || !sel_left_n || !sel_right_n ||
                                         !sel_up_left || !sel_up_right || !sel_down_left || !sel_down_right);
        
        // Check if current pixel is adjacent to selection (for outer border)
        let is_adjacent = !sel_current && (sel_up || sel_down || sel_left_n || sel_right_n ||
                                           sel_up_left || sel_up_right || sel_down_left || sel_down_right);
        
        if (sel_current) {
            if (is_on_edge) {
                // Edge of selection - marching ants
                // Determine edge position along the edge direction
                // Check which edge we're on and use the appropriate coordinate
                var edge_pos = 0.0;
                let at_left = !sel_left_n;
                let at_right = !sel_right_n;
                let at_top = !sel_up;
                let at_bottom = !sel_down;
                
                // For vertical edges use Y, for horizontal edges use X
                if (at_left || at_right) {
                    edge_pos = doc_pixel.y;
                } else {
                    edge_pos = doc_pixel.x;
                }
                
                on_selection_border = true;
                if (!in_layer) {
                    // Outside layer: solid white border
                    color = vec3<f32>(1.0, 1.0, 1.0);
                } else {
                    // Inside layer: marching ants
                    let dash_length = 4.0;
                    let dash_phase = floor((edge_pos + uniforms.time * 8.0) / dash_length);
                    let is_white = (dash_phase % 2.0) == 0.0;
                    if (is_white) {
                        color = vec3<f32>(1.0, 1.0, 1.0);  // White
                    } else {
                        color = vec3<f32>(0.0, 0.0, 0.0);  // Black
                    }
                }
            } else {
                // Interior of selection: slightly dim (multiply by 0.9)
                color = color * 0.9;
            }
        } else if (is_adjacent) {
            // Adjacent to selection - dim the pixels
            color = color * 0.6;
        }
        
        // Draw selection rectangle border (for selections extending beyond document)
        // This draws the border of the selection_rect uniform, which can extend beyond buffer
        // 1 pixel wide border
        let on_sel_rect_left = abs(doc_pixel.x - sel_left) < 1.0 && 
                               doc_pixel.y >= sel_top && doc_pixel.y <= sel_bottom;
        let on_sel_rect_right = abs(doc_pixel.x - sel_right) < 1.0 && 
                                doc_pixel.y >= sel_top && doc_pixel.y <= sel_bottom;
        let on_sel_rect_top = abs(doc_pixel.y - sel_top) < 1.0 && 
                              doc_pixel.x >= sel_left && doc_pixel.x <= sel_right;
        let on_sel_rect_bottom = abs(doc_pixel.y - sel_bottom) < 1.0 && 
                                 doc_pixel.x >= sel_left && doc_pixel.x <= sel_right;
        
        if (on_sel_rect_left || on_sel_rect_right || on_sel_rect_top || on_sel_rect_bottom) {
            // Marching ants - pattern runs along the edge
            on_selection_border = true;
            if (!in_layer) {
                // Outside layer: solid white border
                color = vec3<f32>(1.0, 1.0, 1.0);
            } else {
                // Inside layer: marching ants
                let dash_length = 4.0;
                
                // Vertical edges use Y, horizontal edges use X
                var edge_pos = 0.0;
                if (on_sel_rect_left || on_sel_rect_right) {
                    edge_pos = doc_pixel.y;
                } else {
                    edge_pos = doc_pixel.x;
                }
                
                let dash_phase = floor((edge_pos + uniforms.time * 8.0) / dash_length);
                let is_white = (dash_phase % 2.0) == 0.0;
                if (is_white) {
                    color = vec3<f32>(1.0, 1.0, 1.0);
                } else {
                    color = vec3<f32>(0.0, 0.0, 0.0);
                }
            }
        }
    }

    // Calculate bloom from original undistorted coordinates
    let bloom_glow = apply_bloom(distorted_uv);
    
    // Apply post-processing effects (skip for selection border to keep clean marching ants)
    if (!on_selection_border) {
        color = apply_scanlines(color, distorted_uv);
        color = apply_noise(color, distorted_uv);
        
        // Add bloom on top (much more visible now)
        color = color + bloom_glow;
    }
    
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
