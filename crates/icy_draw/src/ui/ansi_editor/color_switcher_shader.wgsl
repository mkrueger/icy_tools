// Color Switcher Shader
// Renders foreground/background color rectangles with shadows and swap animation

struct Uniforms {
    // Colors (RGBA)
    foreground_color: vec4<f32>,
    background_color: vec4<f32>,
    default_fg_color: vec4<f32>,  // DOS color 7 (light gray)
    // Animation
    swap_progress: f32,      // 0.0 = normal, 1.0 = swapped
    time: f32,               // For potential pulse effects
    // Layout
    widget_size: vec2<f32>,
    // Hover states
    hover_swap: f32,         // 1.0 if hovering swap area
    hover_default: f32,      // 1.0 if hovering default area
    rect_size: f32,
    shadow_margin: f32,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var swap_icon_texture: texture_2d<f32>;

@group(0) @binding(2)
var swap_icon_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Full-screen quad
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, 1.0)
    );
    
    var uvs = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 0.0)
    );
    
    var output: VertexOutput;
    output.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    output.uv = uvs[vertex_index];
    return output;
}

// Realistic drop shadow with soft falloff
fn drop_shadow(uv: vec2<f32>, box_min: vec2<f32>, box_max: vec2<f32>, offset: vec2<f32>, blur: f32) -> f32 {
    let shadow_min = box_min + offset;
    let shadow_max = box_max + offset;
    let center = (shadow_min + shadow_max) * 0.5;
    let half_size = (shadow_max - shadow_min) * 0.5;
    let d = abs(uv - center) - half_size;
    let dist = length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0);
    // Soft gaussian-like falloff
    return exp(-dist * dist / (blur * blur * 0.5));
}

// Check if point is inside a rectangle (no rounding for sharp edges)
fn inside_rect(uv: vec2<f32>, box_min: vec2<f32>, box_max: vec2<f32>) -> bool {
    return uv.x >= box_min.x && uv.x <= box_max.x && uv.y >= box_min.y && uv.y <= box_max.y;
}

// Draw a large color rectangle with 1px black outer + 1px white inner border
fn draw_large_rect(uv: vec2<f32>, box_min: vec2<f32>, box_max: vec2<f32>, color: vec3<f32>) -> vec4<f32> {
    // Check layers from outside to inside
    if (!inside_rect(uv, box_min, box_max)) {
        return vec4<f32>(0.0);
    }
    
    // 1px black outer border
    let black_inner_min = box_min + vec2<f32>(1.0);
    let black_inner_max = box_max - vec2<f32>(1.0);
    if (!inside_rect(uv, black_inner_min, black_inner_max)) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    
    // 1px white inner border
    let white_inner_min = black_inner_min + vec2<f32>(1.0);
    let white_inner_max = black_inner_max - vec2<f32>(1.0);
    if (!inside_rect(uv, white_inner_min, white_inner_max)) {
        return vec4<f32>(1.0, 1.0, 1.0, 1.0);
    }
    
    // Color fill
    return vec4<f32>(color, 1.0);
}

// Draw a small default color rectangle with 1px border (inverted color)
fn draw_small_rect(uv: vec2<f32>, box_min: vec2<f32>, box_max: vec2<f32>, color: vec3<f32>) -> vec4<f32> {
    if (!inside_rect(uv, box_min, box_max)) {
        return vec4<f32>(0.0);
    }
    
    // 1px border in inverted color
    let inner_min = box_min + vec2<f32>(1.0);
    let inner_max = box_max - vec2<f32>(1.0);
    if (!inside_rect(uv, inner_min, inner_max)) {
        // Inverted border color
        let inv = vec3<f32>(1.0) - color;
        return vec4<f32>(inv, 1.0);
    }
    
    // Color fill
    return vec4<f32>(color, 1.0);
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Flip Y axis for rendering (screen Y=0 at top, but we draw with Y=0 at bottom)
    let flipped_uv = vec2<f32>(input.uv.x, 1.0 - input.uv.y);
    let uv = flipped_uv * uniforms.widget_size;
    let size = uniforms.widget_size.x;
    
    // Rectangle size - proportional to widget
    let rect_size = uniforms.rect_size;
    let small_rect_size = rect_size / 2.5;  // Small default color rectangles
    let shadow_margin = uniforms.shadow_margin;
    let overlap = small_rect_size / 2.0;  // How much the rectangles overlap in the center
    
    // Animation interpolation - positions swap during animation
    let t = uniforms.swap_progress;
    let ease_t = t * t * (3.0 - 2.0 * t); // Smooth step
    
    // Keep colors as-is during animation (they will be swapped after animation completes)
    let fg_color = uniforms.foreground_color.rgb;
    let bg_color = uniforms.background_color.rgb;
    
    // Transparent background
    var final_color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    
    // Layout (like the reference image):
    // - FOREGROUND: top-left (overlapping, in front)
    // - BACKGROUND: bottom-right (behind)
    // - Swap icon: top-right corner
    // - Default colors: bottom-left corner
    
    // Base positions (no animation)
    let fg_base_min = floor(vec2<f32>(size / 2 - rect_size + overlap, size / 2 - overlap));
    let bg_base_min = floor(vec2<f32>(size / 2.0 - overlap, size / 2.0 - rect_size + overlap));
    
    // Target positions (swapped)
    let fg_target_min = bg_base_min;  // FG moves to where BG was
    let bg_target_min = fg_base_min;  // BG moves to where FG was
    
    // Interpolate positions based on animation progress
    let fg_min = mix(fg_base_min, fg_target_min, ease_t);
    let bg_min = mix(bg_base_min, bg_target_min, ease_t);
    
    let fg_max = fg_min + vec2<f32>(rect_size, rect_size); 
    let bg_max = bg_min + vec2<f32>(rect_size, rect_size);
    
    // --- Draw shadows first (light source from top-left, shadows go bottom-right) ---
    let shadow_offset = vec2<f32>(1.0, -1.0);  // Right and down (Y is flipped), subtle offset
    let shadow_blur = 2.5;
    
    // Only draw shadows where no rectangles will be drawn
    let in_bg_rect = inside_rect(uv, bg_min, bg_max);
    let in_fg_rect = inside_rect(uv, fg_min, fg_max);
    
    // Background rect shadow
    if (!in_bg_rect) {
        let bg_shadow = drop_shadow(uv, bg_min, bg_max, shadow_offset, shadow_blur);
        if (bg_shadow > 0.01) {
            final_color = mix(final_color, vec4<f32>(0.0, 0.0, 0.0, bg_shadow * 0.4), bg_shadow * 0.4);
        }
    }
    
    // Foreground rect shadow (slightly stronger, in front)
    if (!in_fg_rect) {
        let fg_shadow = drop_shadow(uv, fg_min, fg_max, shadow_offset, shadow_blur);
        if (fg_shadow > 0.01) {
            final_color = mix(final_color, vec4<f32>(0.0, 0.0, 0.0, fg_shadow * 0.45), fg_shadow * 0.45);
        }
    }
    
    // --- Draw color rectangles with dynamic z-order based on animation ---
    // At t < 0.5: FG is in front, BG is behind
    // At t >= 0.5: BG is in front, FG is behind (they're swapping)
    if (ease_t < 0.5) {
        // Normal order: BG behind, FG in front
        let bg_rect = draw_large_rect(uv, bg_min, bg_max, bg_color);
        if (bg_rect.a > 0.0) {
            final_color = bg_rect;
        }
        
        let fg_rect = draw_large_rect(uv, fg_min, fg_max, fg_color);
        if (fg_rect.a > 0.0) {
            final_color = fg_rect;
        }
    } else {
        // Swapped order: FG behind, BG in front (completing the swap)
        let fg_rect = draw_large_rect(uv, fg_min, fg_max, fg_color);
        if (fg_rect.a > 0.0) {
            final_color = fg_rect;
        }
        
        let bg_rect = draw_large_rect(uv, bg_min, bg_max, bg_color);
        if (bg_rect.a > 0.0) {
            final_color = bg_rect;
        }
    }
    
    // --- Draw small default color rectangles (bottom-left corner, fixed position) ---
    // Use base positions so these don't move during animation
    let def_bg_size = small_rect_size;
    
    let def_fg_min = floor(vec2<f32>(
                        fg_base_min.x, 
                        bg_base_min.y)) + 1.0;
    let def_fg_max = def_fg_min + def_bg_size;
    
    let def_bg_min = floor(def_fg_min + shadow_margin / 2.0);
    let def_bg_max = def_bg_min + def_bg_size;
    
    let def_bg_rect = draw_small_rect(uv, def_bg_min, def_bg_max, vec3<f32>(0.0, 0.0, 0.0));
    if (def_bg_rect.a > 0.0) {
        final_color = def_bg_rect;
    }
    
    // Default FG (DOS color 7 - light gray) - in front, at corner (aligned with FG rect left edge)
    let def_fg_rect = draw_small_rect(uv, def_fg_min, def_fg_max, uniforms.default_fg_color.rgb);
    if (def_fg_rect.a > 0.0) {
        final_color = def_fg_rect;
    }
    
    // --- Draw swap icon (top-right corner, fixed position) ---
    // Use base positions so icon doesn't move during animation
    let fg_base_max = fg_base_min + vec2<f32>(rect_size, rect_size);
    let bg_base_max = bg_base_min + vec2<f32>(rect_size, rect_size);
    let icon_padding = shadow_margin / 4.0;
    let icon_min = vec2<f32>(fg_base_max.x - icon_padding, bg_base_max.y + icon_padding);
    let icon_max = vec2<f32>(bg_base_max.x - icon_padding, fg_base_max.y - icon_padding);
    
    // Calculate UV for swap icon texture
    let icon_uv_x = (uv.x - icon_min.x) / (icon_max.x - icon_min.x);
    let icon_uv_y = (uv.y - icon_min.y) / (icon_max.y - icon_min.y);
    
    if (icon_uv_x >= 0.0 && icon_uv_x <= 1.0 && icon_uv_y >= 0.0 && icon_uv_y <= 1.0) {
        // Sample the swap icon texture
        let tex_uv = vec2<f32>(icon_uv_x, 1.0 - icon_uv_y);
        let icon_color = textureSample(swap_icon_texture, swap_icon_sampler, tex_uv);
        
        // Apply hover highlight
        var icon_tint = icon_color.rgb;
        if (uniforms.hover_swap > 0.5) {
            icon_tint = mix(icon_tint, vec3<f32>(0.4, 0.8, 1.0), 0.5);
        }
        
        if (icon_color.a > 0.1) {
            final_color = vec4<f32>(icon_tint, icon_color.a);
        }
    }
    
    // Highlight default area on hover
    if (uniforms.hover_default > 0.5) {
        let def_area_max = vec2<f32>(def_bg_size * 1.5, def_bg_size * 1.5);
        if (uv.x < def_area_max.x && uv.y < def_area_max.y) {
            final_color = mix(final_color, vec4<f32>(0.3, 0.6, 1.0, 0.3), 0.3);
        }
    }
    
    return final_color;
}
