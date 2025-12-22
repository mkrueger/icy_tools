// Minimap shader with sliding window texture support
// Uses up to 10 texture slices (matching Terminal)

struct Uniforms {
    viewport_rect: vec4<f32>,    // x, y, width, height (normalized 0-1 in texture space)
    viewport_color: vec4<f32>,   // RGBA color for viewport (primary accent)
    visible_uv_range: vec4<f32>, // min_y, max_y, unused, unused (what part of texture is visible)
    render_dimensions: vec4<f32>, // texture_width, texture_height, available_width, available_height
    border_thickness: f32,       // Border thickness in pixels
    show_viewport: f32,          // 1.0 to show, 0.0 to hide
    num_slices: f32,             // Number of texture slices (1-10)
    total_image_height: f32,     // Total height in pixels across all slices
    slice_heights: array<vec4<f32>, 3>, // slice_heights[0] = [h0, h1, h2, first_slice_start_y]
    // Checkerboard colors for transparency (from icy_engine_gui::CheckerboardColors)
    checker_color1: vec4<f32>,   // First checkerboard color (RGBA)
    checker_color2: vec4<f32>,   // Second checkerboard color (RGBA)
    checker_params: vec4<f32>,   // x=cell_size, y=enabled (>0.5), z=unused, w=unused

    canvas_bg: vec4<f32>,        // Solid minimap background (RGBA)
}

// 10 texture slots for sliding window
@group(0) @binding(0) var t_slice0: texture_2d<f32>;
@group(0) @binding(1) var t_slice1: texture_2d<f32>;
@group(0) @binding(2) var t_slice2: texture_2d<f32>;
@group(0) @binding(3) var t_slice3: texture_2d<f32>;
@group(0) @binding(4) var t_slice4: texture_2d<f32>;
@group(0) @binding(5) var t_slice5: texture_2d<f32>;
@group(0) @binding(6) var t_slice6: texture_2d<f32>;
@group(0) @binding(7) var t_slice7: texture_2d<f32>;
@group(0) @binding(8) var t_slice8: texture_2d<f32>;
@group(0) @binding(9) var t_slice9: texture_2d<f32>;

// Sampler at binding 10
@group(0) @binding(10) var s_sampler: sampler;

// Uniforms at binding 11
@group(0) @binding(11) var<uniform> uniforms: Uniforms;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// Fullscreen triangle vertices
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0)
    );
    
    var uvs = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(2.0, 1.0),
        vec2<f32>(0.0, -1.0)
    );
    
    var output: VertexOutput;
    output.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    output.uv = uvs[vertex_index];
    return output;
}

// Get slice height from packed array
// slice_heights[0] = [h0, h1, h2, first_slice_start_y]
// slice_heights[1] = [h3, h4, h5, h6]
// slice_heights[2] = [h7, h8, h9, 0]
fn get_slice_height(index: i32) -> f32 {
    if index == 0 { return uniforms.slice_heights[0][0]; }
    if index == 1 { return uniforms.slice_heights[0][1]; }
    if index == 2 { return uniforms.slice_heights[0][2]; }
    if index == 3 { return uniforms.slice_heights[1][0]; }
    if index == 4 { return uniforms.slice_heights[1][1]; }
    if index == 5 { return uniforms.slice_heights[1][2]; }
    if index == 6 { return uniforms.slice_heights[1][3]; }
    if index == 7 { return uniforms.slice_heights[2][0]; }
    if index == 8 { return uniforms.slice_heights[2][1]; }
    if index == 9 { return uniforms.slice_heights[2][2]; }
    return 0.0;
}

// Get first_slice_start_y (where the sliding window starts in document space)
fn get_first_slice_start_y() -> f32 {
    return uniforms.slice_heights[0][3];
}

// Get checkerboard or solid background color based on screen position
fn get_background_color(screen_pos: vec2<f32>) -> vec4<f32> {
    if uniforms.checker_params.y > 0.5 {
        // Checkerboard enabled
        let cell_size = uniforms.checker_params.x;
        let cx = u32(floor(screen_pos.x / cell_size));
        let cy = u32(floor(screen_pos.y / cell_size));
        if ((cx + cy) & 1u) == 1u {
            return uniforms.checker_color2;
        } else {
            return uniforms.checker_color1;
        }
    } else {
        // Fallback to solid background
        return uniforms.canvas_bg;
    }
}

fn get_canvas_background() -> vec4<f32> {
    // Outside the minimap content we keep a solid background.
    return uniforms.canvas_bg;
}

fn sample_slice(index: i32, uv: vec2<f32>) -> vec4<f32> {
    if index == 0 { return textureSample(t_slice0, s_sampler, uv); }
    if index == 1 { return textureSample(t_slice1, s_sampler, uv); }
    if index == 2 { return textureSample(t_slice2, s_sampler, uv); }
    if index == 3 { return textureSample(t_slice3, s_sampler, uv); }
    if index == 4 { return textureSample(t_slice4, s_sampler, uv); }
    if index == 5 { return textureSample(t_slice5, s_sampler, uv); }
    if index == 6 { return textureSample(t_slice6, s_sampler, uv); }
    if index == 7 { return textureSample(t_slice7, s_sampler, uv); }
    if index == 8 { return textureSample(t_slice8, s_sampler, uv); }
    if index == 9 { return textureSample(t_slice9, s_sampler, uv); }
    return textureSample(t_slice0, s_sampler, uv);
}

// Sample from the appropriate texture slice based on pixel Y coordinate
// Uses sliding window approach: N slices covering current viewport area
fn sample_sliced_texture(uv: vec2<f32>) -> vec4<f32> {
    let total_height = uniforms.total_image_height;
    if total_height <= 0.0 {
        return get_canvas_background();
    }

    // uv.y is in texture space (0-1 over the rendered *window*)
    // Convert directly to window pixel Y.
    // Note: `first_slice_start_y` is a document-space offset and must NOT be applied here.
    // Applying it would make `window_y` negative for any scrolled window, resulting in black.
    let window_y = uv.y * total_height;
    
    // Calculate total window height (sum of all tile heights)
    let num_slices = i32(uniforms.num_slices);
    var total_window_height: f32 = 0.0;
    for (var j: i32 = 0; j < num_slices; j++) {
        total_window_height += get_slice_height(j);
    }
    
    // If outside our rendered window (after last tile), show transparent/black
    if window_y >= total_window_height {
        return get_canvas_background();
    }
    
    // Find which slice contains this Y coordinate
    var cumulative_height: f32 = 0.0;
    
    for (var i: i32 = 0; i < num_slices; i++) {
        let slice_height = get_slice_height(i);
        let next_cumulative = cumulative_height + slice_height;
        
        if window_y < next_cumulative {
            // This is the slice we need
            let local_y = (window_y - cumulative_height) / slice_height;
            let slice_uv = vec2<f32>(uv.x, clamp(local_y, 0.0, 1.0));
            
            return sample_slice(i, slice_uv);
        }
        cumulative_height = next_cumulative;
    }
    
    // Fallback - outside rendered area
    return get_canvas_background();
}

// Signed distance to a rectangle (negative inside, positive outside)
fn sd_rect(p: vec2<f32>, rect_min: vec2<f32>, rect_max: vec2<f32>) -> f32 {
    let center = (rect_min + rect_max) * 0.5;
    let half_size = (rect_max - rect_min) * 0.5;
    let d = abs(p - center) - half_size;
    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0);
}

// Smooth step function for anti-aliased edges
fn smooth_edge(d: f32, edge_width: f32) -> f32 {
    return 1.0 - smoothstep(-edge_width, edge_width, d);
}

// Check if a point is inside a rectangle
fn is_inside_rect(uv: vec2<f32>, rect: vec4<f32>) -> bool {
    let rect_min = rect.xy;
    let rect_max = rect.xy + rect.zw;
    return uv.x >= rect_min.x && uv.x <= rect_max.x &&
           uv.y >= rect_min.y && uv.y <= rect_max.y;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let screen_uv = input.uv;
    let screen_pos = input.position.xy;
    
    // Get render dimensions for aspect-ratio-correct rendering
    let tex_w = uniforms.render_dimensions.x;
    let tex_h = uniforms.render_dimensions.y;
    let avail_w = uniforms.render_dimensions.z;
    let avail_h = uniforms.render_dimensions.w;
    
    // Calculate the scale factor (X fills available width)
    let scale = avail_w / tex_w;
    
    // Scaled height of content (maintaining aspect ratio)
    let scaled_h = tex_h * scale;
    
    // Calculate what fraction of available height is actually used
    let used_height_ratio = min(scaled_h / avail_h, 1.0);
    
    // Solid canvas background (outside content)
    let canvas_bg = get_canvas_background();
    // Background used for transparency blending
    let transparency_bg = get_background_color(screen_pos);
    
    // If we're below the content area, show background
    if screen_uv.y > used_height_ratio {
        return canvas_bg;
    }
    
    // Remap screen UV to texture space (only the used portion)
    // screen_uv.y goes from 0 to used_height_ratio for the content
    let content_uv_y = screen_uv.y / used_height_ratio;
    
    // Get the visible UV range (what part of the texture is currently shown on screen)
    let visible_min_y = uniforms.visible_uv_range.x;
    let visible_max_y = uniforms.visible_uv_range.y;
    let visible_height = visible_max_y - visible_min_y;
    
    // Transform content UV to texture UV based on visible range
    // content_uv_y=0 maps to visible_min_y, content_uv_y=1 maps to visible_max_y
    let texture_uv = vec2<f32>(screen_uv.x, visible_min_y + content_uv_y * visible_height);
    
    // Sample the texture using multi-slice approach with transformed UV
    var tex_color = sample_sliced_texture(texture_uv);
    
    // Blend texture with checkerboard background for transparency
    var color = vec4<f32>(
        mix(transparency_bg.rgb, tex_color.rgb, tex_color.a),
        1.0
    );
    
    // Use total image height for calculations
    let tex_height = uniforms.total_image_height;
    
    // For viewport overlay, we need to work in content space (0 to used_height_ratio)
    let content_uv = vec2<f32>(screen_uv.x, content_uv_y);
    
    // Apply viewport overlay if enabled
    if uniforms.show_viewport > 0.5 {
        // Transform viewport rect from texture space to screen space
        let vp_y = uniforms.viewport_rect.y;
        let vp_h = uniforms.viewport_rect.w;
        let vp_y_end = vp_y + vp_h;
        
        // Check if viewport overlaps with visible range
        let vp_visible_start = max(vp_y, visible_min_y);
        let vp_visible_end = min(vp_y_end, visible_max_y);
        
        if vp_visible_end > vp_visible_start {
            // Transform to content UV space (0-1 in the visible content area)
            let content_y = (vp_visible_start - visible_min_y) / visible_height;
            let content_h = (vp_visible_end - vp_visible_start) / visible_height;
            
            // X stays the same (full width rendering)
            let content_rect = vec4<f32>(uniforms.viewport_rect.x, content_y, uniforms.viewport_rect.z, content_h);
            
            let rect_min = content_rect.xy;
            let rect_max = content_rect.xy + content_rect.zw;
            let inside_viewport = is_inside_rect(content_uv, content_rect);
            
            // Calculate signed distance to rectangle border
            let dist = sd_rect(content_uv, rect_min, rect_max);
            
            // Convert border thickness from pixels to UV space
            let visible_pixel_height = tex_height * visible_height;
            let pixel_size = 1.0 / visible_pixel_height;
            let border_width = uniforms.border_thickness * pixel_size;
            let inner_border = border_width * 0.5;
            let outer_border = border_width * 0.5;
            
            // Create a glowing border effect
            let inner_glow_width = border_width * 3.0;
            let inner_glow = smooth_edge(dist + inner_border, inner_glow_width) * 0.15;
            
            // Main border with anti-aliasing
            let on_border = smooth_edge(abs(dist) - inner_border, pixel_size * 1.5);
            
            // Outer glow (outside the viewport, fades outward)
            let outer_glow_width = border_width * 4.0;
            let outer_glow = smooth_edge(dist - outer_border, outer_glow_width) * 0.2;
            
            // Combine effects
            if !inside_viewport {
                // Desaturate and darken areas outside viewport
                let luminance = dot(color.rgb, vec3<f32>(0.299, 0.587, 0.114));
                let desaturated = mix(color.rgb, vec3<f32>(luminance), 0.5);
                color = vec4<f32>(desaturated * 0.4, color.a);
                
                // Add outer glow
                let glow_color = uniforms.viewport_color.rgb * 0.6;
                color = vec4<f32>(mix(color.rgb, glow_color, outer_glow), color.a);
            } else {
                // Inside viewport: subtle inner glow
                let glow_color = uniforms.viewport_color.rgb;
                color = vec4<f32>(mix(color.rgb, glow_color, inner_glow), color.a);
            }
            
            // Draw the main border with a gradient effect
            if on_border > 0.01 {
                let gradient = (screen_uv.x + screen_uv.y) * 0.5 + 0.5;
                let border_color_light = uniforms.viewport_color.rgb * 1.3;
                let border_color_dark = uniforms.viewport_color.rgb * 0.8;
                let final_border_color = mix(border_color_dark, border_color_light, gradient);
                
                color = vec4<f32>(mix(color.rgb, final_border_color, on_border * uniforms.viewport_color.a), color.a);
            }
        }
    }
    
    return color;
}
