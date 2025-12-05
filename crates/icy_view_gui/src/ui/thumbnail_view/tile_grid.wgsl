// Tile Grid Shader v4 - Multi-Texture Layered Architecture
// Renders textured tiles with rounded borders, drop shadows, and glow effects
// Supports up to 10 vertically stacked texture slices for very tall images (up to 80,000px)
//
// Coordinate system: 0,0 = top-left, Y grows downward (standard screen coords)
// All rectangles are pre-computed in Rust and passed as uniforms
//
// TEXTURE SLICING:
//   - GPU textures are limited to ~8192px height
//   - Very tall images (e.g., 80,000px) are split into up to 10 slices
//   - Each slice is MAX_SLICE_HEIGHT pixels (8000px)
//   - Shader samples from the correct slice based on Y coordinate
//
// LAYER ORDER (back to front):
//   0. Background (transparent)
//   1. Shadow
//   2. Glow (if selected/hovered)
//   3. Border
//   4. Padding (black fill between border and content)
//   5. Image (main texture - may span multiple slices)
//   6. Label background (gray)
//   7. Label text (rendered via tag texture)

// ============================================================================
// Texture Slicing Constants
// ============================================================================

const MAX_SLICE_HEIGHT: f32 = 8000.0;
const MAX_TEXTURE_SLICES: u32 = 10u;

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
    // Texture slice info
    num_slices: f32,              // Number of active texture slices (1-10)
    total_image_height: f32,      // Total logical height of the image in pixels
    // Slice heights: height of each slice in pixels (0 if unused)
    slice_heights: array<vec4<f32>, 3>,  // 12 floats for 10 slices + 2 padding
};

// 10 texture slices for very tall images
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
@group(0) @binding(10) var s_sampler: sampler;
@group(0) @binding(11) var<uniform> uniforms: Uniforms;

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

/// Get slice height by index (from packed array)
fn get_slice_height(index: u32) -> f32 {
    let arr_idx = index / 4u;
    let component = index % 4u;
    if (arr_idx == 0u) {
        if (component == 0u) { return uniforms.slice_heights[0].x; }
        if (component == 1u) { return uniforms.slice_heights[0].y; }
        if (component == 2u) { return uniforms.slice_heights[0].z; }
        return uniforms.slice_heights[0].w;
    } else if (arr_idx == 1u) {
        if (component == 0u) { return uniforms.slice_heights[1].x; }
        if (component == 1u) { return uniforms.slice_heights[1].y; }
        if (component == 2u) { return uniforms.slice_heights[1].z; }
        return uniforms.slice_heights[1].w;
    } else {
        if (component == 0u) { return uniforms.slice_heights[2].x; }
        if (component == 1u) { return uniforms.slice_heights[2].y; }
        return 0.0;  // Only 10 slices, indices 10-11 unused
    }
}

/// Sample from the appropriate texture slice based on Y position
fn sample_sliced_image(uv: vec2<f32>) -> vec4<f32> {
    // Fast path for single-slice images (most common case)
    if (uniforms.num_slices <= 1.0) {
        return textureSample(t_slice0, s_sampler, uv);
    }
    
    // Multi-slice: UV.y (0-1) maps to the full texture height
    let pixel_y = uv.y * uniforms.total_image_height;
    
    // Find which slice contains this pixel
    var accumulated_height: f32 = 0.0;
    var slice_idx: u32 = 0u;
    var slice_start: f32 = 0.0;
    let num_slices = u32(uniforms.num_slices);
    
    for (var i: u32 = 0u; i < num_slices; i = i + 1u) {
        let slice_height = get_slice_height(i);
        let slice_end = accumulated_height + slice_height;
        
        if (pixel_y < slice_end) {
            slice_idx = i;
            slice_start = accumulated_height;
            break;
        }
        accumulated_height = slice_end;
        slice_idx = i;
        slice_start = accumulated_height;
    }
    
    // Calculate UV within the slice
    let slice_height = get_slice_height(slice_idx);
    let local_y = pixel_y - slice_start;
    let slice_uv = vec2<f32>(uv.x, local_y / slice_height);
    
    // DEBUG: Color-code slices to visualize which slice is being sampled
    // Uncomment this to see slice boundaries
    // let debug_colors = array<vec4<f32>, 6>(
    //     vec4<f32>(1.0, 0.0, 0.0, 1.0),  // Red
    //     vec4<f32>(0.0, 1.0, 0.0, 1.0),  // Green
    //     vec4<f32>(0.0, 0.0, 1.0, 1.0),  // Blue
    //     vec4<f32>(1.0, 1.0, 0.0, 1.0),  // Yellow
    //     vec4<f32>(1.0, 0.0, 1.0, 1.0),  // Magenta
    //     vec4<f32>(0.0, 1.0, 1.0, 1.0)   // Cyan
    // );
    // return debug_colors[slice_idx % 6u];
    
    // Sample from the appropriate slice
    switch(slice_idx) {
        case 0u: { return textureSample(t_slice0, s_sampler, slice_uv); }
        case 1u: { return textureSample(t_slice1, s_sampler, slice_uv); }
        case 2u: { return textureSample(t_slice2, s_sampler, slice_uv); }
        case 3u: { return textureSample(t_slice3, s_sampler, slice_uv); }
        case 4u: { return textureSample(t_slice4, s_sampler, slice_uv); }
        case 5u: { return textureSample(t_slice5, s_sampler, slice_uv); }
        case 6u: { return textureSample(t_slice6, s_sampler, slice_uv); }
        case 7u: { return textureSample(t_slice7, s_sampler, slice_uv); }
        case 8u: { return textureSample(t_slice8, s_sampler, slice_uv); }
        case 9u: { return textureSample(t_slice9, s_sampler, slice_uv); }
        default: { return textureSample(t_slice0, s_sampler, slice_uv); }
    }
}

/// Layer 5: Image (main thumbnail texture - may span multiple slices)
fn layer_image(p: vec2<f32>) -> vec4<f32> {
    if (!point_in_rect(p, uniforms.image_rect)) {
        return vec4<f32>(0.0);
    }
    
    let uv = rect_uv(p, uniforms.image_rect);
    var color = sample_sliced_image(uv);
    
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
