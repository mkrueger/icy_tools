// Tile Grid Shader v3 - Layered Architecture
// Renders textured tiles with rounded borders, drop shadows, and glow effects
//
// Coordinate system: 0,0 = top-left, Y grows downward (standard screen coords)
// All rectangles are pre-computed in Rust and passed as uniforms
//
// LAYER ORDER (back to front):
//   0. Background (transparent)
//   1. Shadow
//   2. Glow (if selected/hovered)
//   3. Border
//   4. Padding (black fill between border and content)
//   5. Image (main texture)
//   6. Label background (gray)
//   7. Label text (rendered via tag texture)

// ============================================================================
// Color Constants
// ============================================================================

// Border colors by state
const BORDER_COLOR_NORMAL: vec4<f32> = vec4<f32>(0.0, 0.0, 0.0, 1.0);
const BORDER_COLOR_SELECTED: vec4<f32> = vec4<f32>(0.667, 0.667, 0.667, 1.0);
const BORDER_COLOR_HOVERED: vec4<f32> = vec4<f32>(0.0, 0.0, 0.0, 1.0);

// Glow effect parameters
const GLOW_COLOR_SELECTED: vec3<f32> = vec3<f32>(0.3, 0.6, 0.95);
const GLOW_COLOR_HOVERED: vec3<f32> = vec3<f32>(0.7, 0.75, 0.8);
const GLOW_RADIUS_SELECTED: f32 = 8.0;
const GLOW_RADIUS_HOVERED: f32 = 6.0;
const GLOW_INTENSITY_SELECTED: f32 = 0.5;
const GLOW_INTENSITY_HOVERED: f32 = 0.3;

// Fill colors
const LABEL_BG_COLOR: vec4<f32> = vec4<f32>(0.667, 0.667, 0.667, 1.0);
const PADDING_FILL_COLOR: vec4<f32> = vec4<f32>(0.0, 0.0, 0.0, 1.0);
const SHADOW_COLOR: vec3<f32> = vec3<f32>(0.0, 0.0, 0.0);

// Hover effect
const HOVER_GAMMA: f32 = 0.9;
const HOVER_BOOST: f32 = 1.5;

// ============================================================================
// Data Structures
// ============================================================================

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
    @location(1) local_pos: vec2<f32>,
};

// Uniforms with pre-computed rectangles (all in pixels, 0,0 = top-left)
struct Uniforms {
    tile_size: vec2<f32>,
    _pad0: vec2<f32>,
    content_rect: vec4<f32>,      // Tile content (without shadow)
    image_rect: vec4<f32>,        // Where image is rendered
    label_rect: vec4<f32>,        // Where label background is rendered
    is_selected: f32,
    is_hovered: f32,
    border_radius: f32,
    border_width: f32,
    shadow_blur: f32,
    shadow_opacity: f32,
    _padding: vec2<f32>,
};

@group(0) @binding(0) var t_image: texture_2d<f32>;
@group(0) @binding(1) var s_sampler: sampler;
@group(0) @binding(2) var<uniform> uniforms: Uniforms;

// ============================================================================
// Helper Functions
// ============================================================================

/// Check if point is inside rectangle (x, y, width, height)
fn point_in_rect(p: vec2<f32>, rect: vec4<f32>) -> bool {
    return p.x >= rect.x && p.x < rect.x + rect.z &&
           p.y >= rect.y && p.y < rect.y + rect.w;
}

/// Signed distance to rounded rectangle (negative = inside)
fn sd_rounded_rect(p: vec2<f32>, rect: vec4<f32>, radius: f32) -> f32 {
    let center = vec2<f32>(rect.x + rect.z * 0.5, rect.y + rect.w * 0.5);
    let half_size = vec2<f32>(rect.z * 0.5, rect.w * 0.5);
    let local_p = p - center;
    let q = abs(local_p) - half_size + vec2<f32>(radius);
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - radius;
}

/// Calculate UV coordinates for sampling within a rectangle
/// local_pos has Y=0 at top, textures have V=0 at bottom, so no flip needed here
/// (the flip is already done in vertex shader for local_pos)
fn rect_uv(p: vec2<f32>, rect: vec4<f32>) -> vec2<f32> {
    return vec2<f32>(
        (p.x - rect.x) / rect.z,
        (p.y - rect.y) / rect.w  // No flip - local_pos Y is already screen-oriented
    );
}

/// Alpha blend: place foreground over background
fn blend_over(bg: vec4<f32>, fg: vec4<f32>) -> vec4<f32> {
    let a = fg.a + bg.a * (1.0 - fg.a);
    if (a < 0.001) {
        return vec4<f32>(0.0);
    }
    let rgb = (fg.rgb * fg.a + bg.rgb * bg.a * (1.0 - fg.a)) / a;
    return vec4<f32>(rgb, a);
}

// ============================================================================
// Layer Functions - Each returns a color for its layer
// ============================================================================

/// Layer 1: Shadow (soft drop shadow outside content)
fn layer_shadow(dist_content: f32) -> vec4<f32> {
    let alpha = (1.0 - smoothstep(0.0, uniforms.shadow_blur, dist_content)) * uniforms.shadow_opacity;
    return vec4<f32>(SHADOW_COLOR, alpha);
}

/// Layer 2: Glow effect (selection/hover indicator outside content)
fn layer_glow(dist_content: f32) -> vec4<f32> {
    if (dist_content <= 0.0) {
        return vec4<f32>(0.0);  // No glow inside content
    }
    
    if (uniforms.is_selected > 0.5 && dist_content < GLOW_RADIUS_SELECTED) {
        let intensity = (1.0 - dist_content / GLOW_RADIUS_SELECTED) * GLOW_INTENSITY_SELECTED;
        return vec4<f32>(GLOW_COLOR_SELECTED, intensity);
    }
    
    if (uniforms.is_hovered > 0.5 && dist_content < GLOW_RADIUS_HOVERED) {
        let intensity = (1.0 - dist_content / GLOW_RADIUS_HOVERED) * GLOW_INTENSITY_HOVERED;
        return vec4<f32>(GLOW_COLOR_HOVERED, intensity);
    }
    
    return vec4<f32>(0.0);
}

/// Layer 3: Border (colored frame around content)
fn layer_border(p: vec2<f32>, dist_content: f32) -> vec4<f32> {
    // Outside content = no border
    if (dist_content > 0.0) {
        return vec4<f32>(0.0);
    }
    
    // Calculate inner content (content minus border width)
    let inner = vec4<f32>(
        uniforms.content_rect.x + uniforms.border_width,
        uniforms.content_rect.y + uniforms.border_width,
        uniforms.content_rect.z - uniforms.border_width * 2.0,
        uniforms.content_rect.w - uniforms.border_width * 2.0
    );
    
    // Inside inner content = no border
    if (point_in_rect(p, inner)) {
        return vec4<f32>(0.0);
    }
    
    // In border zone - return border color
    if (uniforms.is_selected > 0.5) {
        return BORDER_COLOR_SELECTED;
    } else if (uniforms.is_hovered > 0.5) {
        return BORDER_COLOR_HOVERED;
    }
    return BORDER_COLOR_NORMAL;
}

/// Layer 4: Padding (black fill between border and image/label)
fn layer_padding(p: vec2<f32>) -> vec4<f32> {
    // Calculate inner content
    let inner = vec4<f32>(
        uniforms.content_rect.x + uniforms.border_width,
        uniforms.content_rect.y + uniforms.border_width,
        uniforms.content_rect.z - uniforms.border_width * 2.0,
        uniforms.content_rect.w - uniforms.border_width * 2.0
    );
    
    // Only draw padding if inside inner content but outside image and label
    if (!point_in_rect(p, inner)) {
        return vec4<f32>(0.0);
    }
    if (point_in_rect(p, uniforms.image_rect)) {
        return vec4<f32>(0.0);
    }
    if (point_in_rect(p, uniforms.label_rect)) {
        return vec4<f32>(0.0);
    }
    
    return PADDING_FILL_COLOR;
}

/// Layer 5: Image (main thumbnail texture)
fn layer_image(p: vec2<f32>) -> vec4<f32> {
    if (!point_in_rect(p, uniforms.image_rect)) {
        return vec4<f32>(0.0);
    }
    
    let uv = rect_uv(p, uniforms.image_rect);
    var color = textureSample(t_image, s_sampler, uv);
    
    // Apply hover effect (brightness boost)
    if (uniforms.is_hovered > 0.5) {
        color = vec4<f32>(
            pow(color.rgb, vec3<f32>(HOVER_GAMMA)) * HOVER_BOOST,
            color.a
        );
        color = clamp(color, vec4<f32>(0.0), vec4<f32>(1.0));
    }
    
    return color;
}

/// Layer 6: Label background (gray area for filename)
fn layer_label_bg(p: vec2<f32>) -> vec4<f32> {
    if (!point_in_rect(p, uniforms.label_rect)) {
        return vec4<f32>(0.0);
    }
    return LABEL_BG_COLOR;
}

// Note: Tag text is rendered in a separate render pass with the same shader
// using TileUniforms::new_for_tag() which sets image_rect = full tile

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
    
    var tex_coords = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(1.0, 0.0),
    );

    var output: VertexOutput;
    output.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    output.tex_coord = tex_coords[vertex_index];
    
    // Convert NDC to pixel coords with Y flipped (0,0 = top-left)
    let ndc_normalized = positions[vertex_index] * 0.5 + 0.5;  // 0..1
    output.local_pos = vec2<f32>(
        ndc_normalized.x * uniforms.tile_size.x,
        (1.0 - ndc_normalized.y) * uniforms.tile_size.y  // Flip Y
    );
    return output;
}

// ============================================================================
// Fragment Shader - Linear Layer Composition
// ============================================================================

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let p = input.local_pos;
    
    // Calculate distance once for all layers that need it
    let dist_content = sd_rounded_rect(p, uniforms.content_rect, uniforms.border_radius);
    
    // =========================================================================
    // LAYER COMPOSITION (back to front)
    // Each layer returns vec4 with alpha. We blend them in order.
    // =========================================================================
    
    // Start with transparent background
    var result = vec4<f32>(0.0);
    
    // Layer 1: Shadow
    result = blend_over(result, layer_shadow(dist_content));
    
    // Layer 2: Glow
    result = blend_over(result, layer_glow(dist_content));
    
    // Layer 3: Border
    result = blend_over(result, layer_border(p, dist_content));
    
    // Layer 4: Padding
    result = blend_over(result, layer_padding(p));
    
    // Layer 5: Image
    result = blend_over(result, layer_image(p));
    
    // Layer 6: Label background (tag is rendered separately)
    result = blend_over(result, layer_label_bg(p));
    
    return result;
}
