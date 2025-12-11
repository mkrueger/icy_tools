// Minimap shader - simplified version without CRT effects
// Displays the buffer texture with an elegant viewport overlay

struct Uniforms {
    viewport_rect: vec4<f32>,    // x, y, width, height (normalized 0-1 in texture space)
    viewport_color: vec4<f32>,   // RGBA color for viewport (primary accent)
    visible_uv_range: vec4<f32>, // min_y, max_y, unused, unused (what part of texture is visible)
    border_thickness: f32,       // Border thickness in pixels
    show_viewport: f32,          // 1.0 to show, 0.0 to hide
    _padding: vec2<f32>,
}

@group(0) @binding(0)
var t_texture: texture_2d<f32>;
@group(0) @binding(1)
var s_sampler: sampler;
@group(0) @binding(2)
var<uniform> uniforms: Uniforms;

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
    let uv = input.uv;
    
    // Sample the texture
    var color = textureSample(t_texture, s_sampler, uv);
    
    // Get texture dimensions for calculations
    let tex_dims = vec2<f32>(textureDimensions(t_texture));
    
    // Apply viewport overlay if enabled
    if uniforms.show_viewport > 0.5 {
        // Get the visible UV range (what part of the texture is currently shown on screen)
        let visible_min_y = uniforms.visible_uv_range.x;
        let visible_max_y = uniforms.visible_uv_range.y;
        let visible_height = visible_max_y - visible_min_y;
        
        // Transform viewport rect from texture space to screen space
        // viewport_rect is in texture coordinates (0-1), we need to map it to visible coordinates
        let vp_y = uniforms.viewport_rect.y;
        let vp_h = uniforms.viewport_rect.w;
        let vp_y_end = vp_y + vp_h;
        
        // Check if viewport overlaps with visible range
        let vp_visible_start = max(vp_y, visible_min_y);
        let vp_visible_end = min(vp_y_end, visible_max_y);
        
        if vp_visible_end > vp_visible_start {
            // Transform to screen UV space (0-1 in the visible area)
            let screen_y = (vp_visible_start - visible_min_y) / visible_height;
            let screen_h = (vp_visible_end - vp_visible_start) / visible_height;
            
            // X stays the same (full width rendering)
            let screen_rect = vec4<f32>(uniforms.viewport_rect.x, screen_y, uniforms.viewport_rect.z, screen_h);
            
            let rect_min = screen_rect.xy;
            let rect_max = screen_rect.xy + screen_rect.zw;
            let inside_viewport = is_inside_rect(uv, screen_rect);
            
            // Calculate signed distance to rectangle border
            let dist = sd_rect(uv, rect_min, rect_max);
            
            // Convert border thickness from pixels to UV space
            // Use visible height for more consistent border thickness
            let visible_pixel_height = tex_dims.y * visible_height;
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
                let gradient = (uv.x + uv.y) * 0.5 + 0.5;
                let border_color_light = uniforms.viewport_color.rgb * 1.3;
                let border_color_dark = uniforms.viewport_color.rgb * 0.8;
                let final_border_color = mix(border_color_dark, border_color_light, gradient);
                
                color = vec4<f32>(mix(color.rgb, final_border_color, on_border * uniforms.viewport_color.a), color.a);
            }
        }
    }
    
    return color;
}
