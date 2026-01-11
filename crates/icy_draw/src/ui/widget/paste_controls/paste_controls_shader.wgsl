// Paste Controls Shader
// Two-button widget with colored backgrounds (success/danger)

// ═══════════════════════════════════════════════════════════════════════════
// Uniforms
// ═══════════════════════════════════════════════════════════════════════════

struct Uniforms {
    widget_size: vec2<f32>,
    icon_size: f32,
    icon_padding: f32,
    time: f32,
    cols: u32,
    rows: u32,
    hovered_button: i32,   // -1 = none, 0 = anchor, 1 = cancel
    bg_color: vec4<f32>,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var icon_atlas: texture_2d<f32>;
@group(0) @binding(2) var icon_sampler: sampler;

// ═══════════════════════════════════════════════════════════════════════════
// Colors
// ═══════════════════════════════════════════════════════════════════════════

// Button background colors
const SUCCESS_BG: vec4<f32> = vec4<f32>(0.15, 0.35, 0.15, 1.0);   // Dark green
const DANGER_BG: vec4<f32> = vec4<f32>(0.35, 0.12, 0.12, 1.0);    // Dark red

// Border colors
const SUCCESS_BORDER: vec4<f32> = vec4<f32>(0.3, 0.6, 0.3, 1.0);
const DANGER_BORDER: vec4<f32> = vec4<f32>(0.6, 0.25, 0.25, 1.0);

// Glow colors
const SUCCESS_GLOW: vec4<f32> = vec4<f32>(0.4, 0.8, 0.4, 0.3);
const DANGER_GLOW: vec4<f32> = vec4<f32>(0.8, 0.3, 0.3, 0.3);

// General colors
const SHADOW_COLOR: vec4<f32> = vec4<f32>(0.0, 0.0, 0.0, 0.4);

// ═══════════════════════════════════════════════════════════════════════════
// Constants
// ═══════════════════════════════════════════════════════════════════════════

const CORNER_RADIUS: f32 = 6.0;
const BORDER_WIDTH: f32 = 1.5;
const SHADOW_OFFSET: f32 = 3.0;
const SHADOW_BLUR: f32 = 4.0;
const GLOW_RADIUS: f32 = 8.0;
const ICON_INSET: f32 = 4.0;

// ═══════════════════════════════════════════════════════════════════════════
// Vertex Shader
// ═══════════════════════════════════════════════════════════════════════════

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Full-screen quad
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 1.0)
    );
    
    let pos = positions[vertex_index];
    
    var output: VertexOutput;
    output.position = vec4<f32>(pos * 2.0 - 1.0, 0.0, 1.0);
    output.position.y = -output.position.y;
    output.uv = pos;
    
    return output;
}

// ═══════════════════════════════════════════════════════════════════════════
// Helper Functions
// ═══════════════════════════════════════════════════════════════════════════

fn rounded_rect_sdf(p: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let q = abs(p) - half_size + radius;
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - radius;
}

fn drop_shadow(sdf: f32, offset: f32, blur: f32) -> f32 {
    return 1.0 - smoothstep(-blur, blur, sdf - offset);
}

// ═══════════════════════════════════════════════════════════════════════════
// Fragment Shader
// ═══════════════════════════════════════════════════════════════════════════

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let pixel = input.uv * uniforms.widget_size;
    
    // Start with background color
    var color = uniforms.bg_color;
    
    let cell_size = uniforms.icon_size + uniforms.icon_padding;
    let content_width = f32(uniforms.cols) * cell_size + uniforms.icon_padding;
    let x_offset = (uniforms.widget_size.x - content_width) * 0.5;
    
    // Draw buttons
    for (var i: u32 = 0u; i < 2u; i = i + 1u) {
        let col = i % uniforms.cols;
        let row = i / uniforms.cols;
        
        let button_x = x_offset + uniforms.icon_padding + f32(col) * cell_size + uniforms.icon_size * 0.5;
        let button_y = uniforms.icon_padding + f32(row) * cell_size + uniforms.icon_size * 0.5;
        let button_center = vec2<f32>(button_x, button_y);
        
        let half_size = vec2<f32>(uniforms.icon_size * 0.5, uniforms.icon_size * 0.5);
        let to_center = pixel - button_center;
        
        // SDF for button shape
        let sdf = rounded_rect_sdf(to_center, half_size, CORNER_RADIUS);
        
        // Determine colors based on button type
        var bg_col: vec4<f32>;
        var border_col: vec4<f32>;
        var glow_col: vec4<f32>;
        
        if (i == 0u) {
            // Anchor button - success/green
            bg_col = SUCCESS_BG;
            border_col = SUCCESS_BORDER;
            glow_col = SUCCESS_GLOW;
        } else {
            // Cancel button - danger/red
            bg_col = DANGER_BG;
            border_col = DANGER_BORDER;
            glow_col = DANGER_GLOW;
        }
        
        let is_hovered = uniforms.hovered_button == i32(i);
        
        // Draw drop shadow
        let shadow_alpha = drop_shadow(sdf, SHADOW_OFFSET, SHADOW_BLUR) * SHADOW_COLOR.a;
        color = mix(color, vec4<f32>(SHADOW_COLOR.rgb, 1.0), shadow_alpha * 0.5);
        
        // Draw button background (constant color, no change on hover)
        if (sdf < 0.0) {
            color = bg_col;
            
            // Draw hover glow only inside the button (so backdrop doesn't change)
            if (is_hovered) {
                let glow_dist = max(0.0, -sdf - CORNER_RADIUS);
                let glow_alpha = glow_col.a * exp(-glow_dist * glow_dist / (GLOW_RADIUS * GLOW_RADIUS * 2.0));
                color = mix(color, vec4<f32>(glow_col.rgb, 1.0), glow_alpha);
            }
        }
        
        // Draw border
        let border_dist = abs(sdf);
        if (border_dist < BORDER_WIDTH) {
            let border_alpha = 1.0 - smoothstep(0.0, BORDER_WIDTH, border_dist);
            color = mix(color, border_col, border_alpha);
        }
        
        // Draw icon from atlas (2x1 atlas: [Anchor, Cancel])
        if (sdf < -ICON_INSET) {
            let icon_inset = uniforms.icon_size - ICON_INSET * 2.0;
            let icon_uv = (to_center + half_size - ICON_INSET) / icon_inset;
            
            if (icon_uv.x >= 0.0 && icon_uv.x <= 1.0 && icon_uv.y >= 0.0 && icon_uv.y <= 1.0) {
                // Sample from atlas (2 icons horizontally)
                let atlas_uv = vec2<f32>(
                    (f32(i) + icon_uv.x) / 2.0,
                    icon_uv.y
                );
                let icon_color = textureSample(icon_atlas, icon_sampler, atlas_uv);
                
                // Brighten icon on hover
                var final_icon = icon_color;
                if (is_hovered) {
                    final_icon = vec4<f32>(min(icon_color.rgb * 1.3, vec3<f32>(1.0)), icon_color.a);
                }
                
                color = mix(color, final_icon, final_icon.a);
            }
        }
    }
    
    return color;
}
