// Tool Panel Shader
// Renders tool buttons with icons, shadows, hover effects, and selection highlighting

struct Uniforms {
    widget_size: vec2<f32>,
    icon_size: f32,
    icon_padding: f32,
    time: f32,
    cols: u32,
    rows: u32,
    num_buttons: u32,
    // Background color from theme
    bg_color: vec3<f32>,
    _padding2: f32,
    // Per-button data: [blend_progress, is_selected, is_hovered, tool_index]
    button_data: array<vec4<f32>, 9>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var icon_atlas: texture_2d<f32>;

@group(0) @binding(2)
var icon_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Full-screen quad - note: in wgpu, Y=-1 is bottom, Y=1 is top in NDC
    // But screen coordinates have Y=0 at top
    // The viewport is set to the widget bounds, so we just need to map UV correctly
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, 1.0),   // top-left
        vec2<f32>(1.0, 1.0),    // top-right
        vec2<f32>(1.0, -1.0),   // bottom-right
        vec2<f32>(-1.0, 1.0),   // top-left
        vec2<f32>(1.0, -1.0),   // bottom-right
        vec2<f32>(-1.0, -1.0)   // bottom-left
    );
    
    // UV: (0,0) at top-left, (1,1) at bottom-right
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

// Constants
const ATLAS_COLS: f32 = 4.0;
const ATLAS_ROWS: f32 = 4.0;
const SHADOW_OFFSET: vec2<f32> = vec2<f32>(2.0, 2.0);
const SHADOW_BLUR: f32 = 3.0;
const SHADOW_ALPHA: f32 = 0.4;

// Colors (BG_COLOR is now from uniforms.bg_color)
const SELECTED_BG: vec3<f32> = vec3<f32>(0.25, 0.45, 0.7);
const HOVER_BG: vec3<f32> = vec3<f32>(0.28, 0.28, 0.35);
const BORDER_COLOR: vec3<f32> = vec3<f32>(0.35, 0.35, 0.4);
const SELECTED_BORDER: vec3<f32> = vec3<f32>(0.4, 0.6, 0.85);
const HOVER_GLOW: vec3<f32> = vec3<f32>(0.5, 0.7, 1.0);

// Smooth rectangle SDF
fn rounded_rect_sdf(p: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let d = abs(p) - half_size + vec2<f32>(radius);
    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0) - radius;
}

// Drop shadow
fn drop_shadow(p: vec2<f32>, half_size: vec2<f32>, offset: vec2<f32>, blur: f32) -> f32 {
    let shadow_p = p - offset;
    let d = rounded_rect_sdf(shadow_p, half_size, 3.0);
    return exp(-max(d, 0.0) * 2.0 / blur);
}

// Get button bounds for a given slot (centered horizontally in widget)
fn get_button_bounds(slot: u32) -> vec4<f32> {
    let col = slot % uniforms.cols;
    let row = slot / uniforms.cols;
    
    // Calculate content width and center offset
    let content_width = f32(uniforms.cols) * (uniforms.icon_size + uniforms.icon_padding) + uniforms.icon_padding;
    let x_offset = (uniforms.widget_size.x - content_width) * 0.5;
    
    let x = x_offset + uniforms.icon_padding + f32(col) * (uniforms.icon_size + uniforms.icon_padding);
    let y = uniforms.icon_padding + f32(row) * (uniforms.icon_size + uniforms.icon_padding);
    
    return vec4<f32>(x, y, x + uniforms.icon_size, y + uniforms.icon_size);
}

// Sample icon from atlas
fn sample_icon(uv: vec2<f32>, tool_index: u32) -> vec4<f32> {
    let col = tool_index % u32(ATLAS_COLS);
    let row = tool_index / u32(ATLAS_COLS);
    
    let atlas_uv = vec2<f32>(
        (f32(col) + uv.x) / ATLAS_COLS,
        (f32(row) + uv.y) / ATLAS_ROWS
    );
    
    return textureSample(icon_atlas, icon_sampler, atlas_uv);
}

// Sample icon with shadow effect
fn sample_icon_with_shadow(uv: vec2<f32>, tool_index: u32, icon_size: f32) -> vec4<f32> {
    // Shadow offset in UV space
    let shadow_offset_uv = SHADOW_OFFSET / icon_size;
    
    // Sample shadow (offset and blurred)
    let shadow_uv = uv - shadow_offset_uv;
    var shadow_alpha = 0.0;
    if (shadow_uv.x >= 0.0 && shadow_uv.x <= 1.0 && shadow_uv.y >= 0.0 && shadow_uv.y <= 1.0) {
        let shadow_sample = sample_icon(shadow_uv, tool_index);
        shadow_alpha = shadow_sample.a * SHADOW_ALPHA;
    }
    
    // Sample main icon
    let icon = sample_icon(uv, tool_index);
    
    // Composite shadow under icon
    let shadow_color = vec4<f32>(0.0, 0.0, 0.0, shadow_alpha);
    
    // Blend: icon over shadow
    let result_rgb = icon.rgb * icon.a + shadow_color.rgb * shadow_color.a * (1.0 - icon.a);
    let result_a = icon.a + shadow_color.a * (1.0 - icon.a);
    
    return vec4<f32>(result_rgb, result_a);
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // UV is already correct: (0,0) at top-left, (1,1) at bottom-right
    let pixel = input.uv * uniforms.widget_size;
    
    var final_color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    
    let num_buttons = uniforms.num_buttons;
    
    // First pass: draw shadows
    for (var i = 0u; i < num_buttons; i++) {
        let bounds = get_button_bounds(i);
        let center = vec2<f32>((bounds.x + bounds.z) * 0.5, (bounds.y + bounds.w) * 0.5);
        let half_size = vec2<f32>(uniforms.icon_size * 0.5, uniforms.icon_size * 0.5);
        
        let shadow = drop_shadow(pixel - center, half_size, SHADOW_OFFSET, SHADOW_BLUR);
        if (shadow > 0.01) {
            let shadow_color = vec4<f32>(0.0, 0.0, 0.0, shadow * SHADOW_ALPHA);
            final_color = mix(final_color, shadow_color, shadow * SHADOW_ALPHA);
        }
    }
    
    // Second pass: draw buttons
    for (var i = 0u; i < num_buttons; i++) {
        let bounds = get_button_bounds(i);
        
        // Check if pixel is inside button
        if (pixel.x >= bounds.x && pixel.x <= bounds.z && pixel.y >= bounds.y && pixel.y <= bounds.w) {
            let button_data = uniforms.button_data[i];
            let blend_progress = button_data.x;
            let is_selected = button_data.y > 0.5;
            let is_hovered = button_data.z > 0.5;
            let tool_index = u32(button_data.w);
            
            let center = vec2<f32>((bounds.x + bounds.z) * 0.5, (bounds.y + bounds.w) * 0.5);
            let half_size = vec2<f32>(uniforms.icon_size * 0.5, uniforms.icon_size * 0.5);
            let local_p = pixel - center;
            
            // Rounded rectangle SDF
            let d = rounded_rect_sdf(local_p, half_size - vec2<f32>(1.0), 4.0);
            
            // Background color based on state
            var bg_color = uniforms.bg_color;
            if (is_selected) {
                bg_color = SELECTED_BG;
            } else if (is_hovered) {
                bg_color = HOVER_BG;
            }
            
            // Border color
            var border_color = BORDER_COLOR;
            if (is_selected) {
                border_color = SELECTED_BORDER;
            }
            
            // Draw button background
            if (d < 0.0) {
                final_color = vec4<f32>(bg_color, 1.0);
                
                // Inner glow on hover
                if (is_hovered && !is_selected) {
                    let glow_intensity = exp(d * 0.15) * 0.3;
                    final_color = vec4<f32>(mix(bg_color, HOVER_GLOW, glow_intensity), 1.0);
                }
            }
            
            // Draw border (antialiased)
            let border_width = 1.0;
            let border_aa = smoothstep(-border_width - 0.5, -border_width + 0.5, d) * 
                           smoothstep(0.5, -0.5, d);
            if (border_aa > 0.0) {
                final_color = mix(final_color, vec4<f32>(border_color, 1.0), border_aa);
            }
            
            // Draw icon
            if (d < -2.0) {
                // Calculate icon UV (with some padding)
                let icon_padding = uniforms.icon_size * 0.15;
                let icon_area = uniforms.icon_size - icon_padding * 2.0;
                let icon_uv = (pixel - vec2<f32>(bounds.x + icon_padding, bounds.y + icon_padding)) / icon_area;
                
                if (icon_uv.x >= 0.0 && icon_uv.x <= 1.0 && icon_uv.y >= 0.0 && icon_uv.y <= 1.0) {
                    let icon_color = sample_icon_with_shadow(icon_uv, tool_index, icon_area);
                    
                    // Blend icon over background
                    if (icon_color.a > 0.01) {
                        // Tint selected icons slightly
                        var tinted_icon = icon_color.rgb;
                        if (is_selected) {
                            tinted_icon = mix(icon_color.rgb, vec3<f32>(1.0), 0.1);
                        }
                        
                        final_color = vec4<f32>(
                            mix(final_color.rgb, tinted_icon, icon_color.a),
                            max(final_color.a, icon_color.a)
                        );
                    }
                }
            }
            
            // Selection glow (outer)
            if (is_selected && d > 0.0 && d < 3.0) {
                let glow = exp(-d * 0.8) * 0.4;
                final_color = mix(final_color, vec4<f32>(SELECTED_BORDER, 1.0), glow);
            }
            
            // Hover glow (outer)
            if (is_hovered && !is_selected && d > 0.0 && d < 4.0) {
                let glow = exp(-d * 0.6) * 0.25;
                final_color = mix(final_color, vec4<f32>(HOVER_GLOW, 0.5), glow);
            }
            
            break; // Found our button, no need to continue
        }
    }
    
    return final_color;
}
