// File List Shader
// Renders file list items with pre-rendered icon textures and text

// ============================================================================
// Data Structures
// ============================================================================

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

struct Uniforms {
    item_width: f32,
    item_height: f32,
    is_selected: f32,
    is_hovered: f32,
    // Theme colors (RGBA)
    bg_color: vec4<f32>,
    bg_selected: vec4<f32>,
    bg_hovered: vec4<f32>,
};

@group(0) @binding(0) var t_content: texture_2d<f32>;
@group(0) @binding(1) var s_sampler: sampler;
@group(0) @binding(2) var<uniform> uniforms: Uniforms;

// ============================================================================
// Vertex Shader
// ============================================================================

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(1.0, 1.0),
    );
    
    var uvs = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(1.0, 0.0),
    );

    var output: VertexOutput;
    output.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    output.uv = uvs[vertex_index];
    return output;
}

// ============================================================================
// Fragment Shader  
// ============================================================================

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Determine background color from uniforms
    var bg: vec4<f32>;
    if (uniforms.is_selected > 0.5) {
        bg = uniforms.bg_selected;
    } else if (uniforms.is_hovered > 0.5) {
        bg = uniforms.bg_hovered;
    } else {
        bg = uniforms.bg_color;
    }
    
    // Sample the pre-rendered content texture (icon + text already composited)
    let content = textureSample(t_content, s_sampler, input.uv);
    
    // Blend content over background using alpha
    return mix(bg, vec4<f32>(content.rgb, 1.0), content.a);
}
