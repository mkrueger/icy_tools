// Label Shader - Renders centered label text on gray background
// Separate pass from main tile shader for proper scaling
//
// This shader renders:
// 1. A solid gray background rectangle
// 2. The label texture centered within the rectangle
//
// The label texture is pre-rendered at native resolution and scaled
// to fit the display width while maintaining aspect ratio.

// ============================================================================
// Color Constants
// ============================================================================

const LABEL_BG_COLOR: vec4<f32> = vec4<f32>(0.667, 0.667, 0.667, 1.0);

// ============================================================================
// Data Structures
// ============================================================================

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) local_pos: vec2<f32>,
};

// Hover brightness boost
const HOVER_BRIGHTNESS: f32 = 1.3;

// Uniforms for label rendering
// Must match Rust LabelUniforms struct exactly (48 bytes)
struct LabelUniforms {
    // Viewport size (width, height in pixels)
    viewport_size: vec2<f32>,
    // Padding
    _pad0: vec2<f32>,
    // Label texture dimensions (raw texture size)
    texture_size: vec2<f32>,
    // Display dimensions (scaled to fit tile width)
    display_size: vec2<f32>,
    // Packed: (is_hovered, 0, 0, 0) - using vec4 to avoid vec3 alignment issues
    hover_and_pad: vec4<f32>,
};

@group(0) @binding(0) var t_label: texture_2d<f32>;
@group(0) @binding(1) var s_sampler: sampler;
@group(0) @binding(2) var<uniform> uniforms: LabelUniforms;

// ============================================================================
// Vertex Shader
// ============================================================================

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Full-screen quad vertices
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(1.0, 1.0),
    );

    var output: VertexOutput;
    output.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    
    // Convert NDC to pixel coords (0,0 = top-left)
    let ndc_normalized = positions[vertex_index] * 0.5 + 0.5;  // 0..1
    output.local_pos = vec2<f32>(
        ndc_normalized.x * uniforms.viewport_size.x,
        (1.0 - ndc_normalized.y) * uniforms.viewport_size.y  // Flip Y
    );
    return output;
}

// ============================================================================
// Fragment Shader
// ============================================================================

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let p = input.local_pos;
    
    // The viewport is exactly the label area, so we fill it entirely
    
    // Calculate where the texture should be centered
    // The texture is scaled to fit display_size, centered in viewport_size
    let padding_x = (uniforms.viewport_size.x - uniforms.display_size.x) * 0.5;
    let padding_y = (uniforms.viewport_size.y - uniforms.display_size.y) * 0.5;
    
    // Check if we're in the texture area
    let in_texture_x = p.x >= padding_x && p.x < padding_x + uniforms.display_size.x;
    let in_texture_y = p.y >= padding_y && p.y < padding_y + uniforms.display_size.y;
    
    if (in_texture_x && in_texture_y) {
        // Sample the texture with proper UV mapping
        let local_x = p.x - padding_x;
        let local_y = p.y - padding_y;
        let uv = vec2<f32>(
            local_x / uniforms.display_size.x,
            local_y / uniforms.display_size.y
        );
        
        let tex_color = textureSample(t_label, s_sampler, uv);
        
        // Blend texture over background (for text with transparency)
        let bg = LABEL_BG_COLOR;
        let fg = tex_color;
        let a = fg.a + bg.a * (1.0 - fg.a);
        if (a < 0.001) {
            return bg;
        }
        let rgb = (fg.rgb * fg.a + bg.rgb * bg.a * (1.0 - fg.a)) / a;
        return apply_hover(vec4<f32>(rgb, a));
    }
    
    // Outside texture area - solid gray background
    return apply_hover(LABEL_BG_COLOR);
}

// Apply hover brightness effect
fn apply_hover(color: vec4<f32>) -> vec4<f32> {
    if (uniforms.hover_and_pad.x > 0.5) {
        return vec4<f32>(color.rgb * HOVER_BRIGHTNESS, color.a);
    }
    return color;
}
