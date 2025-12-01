// Tile Grid Shader
// Renders textured tiles with rounded borders, drop shadows, and glow effects

// ============================================================================
// Color Constants - Easy to customize
// ============================================================================

// Border colors
const BORDER_COLOR_NORMAL: vec4<f32> = vec4<f32>(0.0, 0.0, 0.0, 1.0);
const BORDER_COLOR_SELECTED: vec4<f32> = vec4<f32>(0.667, 0.667, 0.667, 1.0);
const BORDER_COLOR_HOVERED: vec4<f32> = vec4<f32>(0.0, 0.0, 0.0, 1.0);

// Glow colors
const GLOW_COLOR_SELECTED: vec3<f32> = vec3<f32>(0.3, 0.6, 0.95);
const GLOW_COLOR_HOVERED: vec3<f32> = vec3<f32>(0.7, 0.75, 0.8);

// Glow parameters
const GLOW_RADIUS_SELECTED: f32 = 8.0;
const GLOW_RADIUS_HOVERED: f32 = 6.0;
const GLOW_INTENSITY_SELECTED: f32 = 0.5;
const GLOW_INTENSITY_HOVERED: f32 = 0.3;

// Background gradient colors
const GRADIENT_TOP: vec4<f32> = vec4<f32>(0.20, 0.20, 0.22, 1.0);
const GRADIENT_BOTTOM: vec4<f32> = vec4<f32>(0.12, 0.12, 0.14, 1.0);

// Label area background (DOS color 7 - light gray)
const LABEL_BG_COLOR: vec4<f32> = vec4<f32>(0.667, 0.667, 0.667, 1.0);

// Padding area fill color (area between border and image)
const PADDING_FILL_COLOR: vec4<f32> = vec4<f32>(0.0, 0.0, 0.0, 1.0);

// Selection tint for image overlay
const SELECTION_TINT: vec4<f32> = vec4<f32>(0.3, 0.5, 0.8, 1.0);
const SELECTION_TINT_AMOUNT: f32 = 0.15;

// Hover boost parameters
const HOVER_GAMMA: f32 = 0.9;
const HOVER_BOOST: f32 = 1.5;

// Shadow color
const SHADOW_COLOR: vec3<f32> = vec3<f32>(0.0, 0.0, 0.0);

// ============================================================================
// Data Structures
// ============================================================================

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
    @location(1) local_pos: vec2<f32>,
};

struct Uniforms {
    tile_size: vec2<f32>,
    is_selected: f32,
    is_hovered: f32,
    border_radius: f32,
    border_width: f32,
    inner_padding: f32,
    shadow_offset_x: f32,
    shadow_offset_y: f32,
    shadow_blur: f32,
    shadow_opacity: f32,
    image_height: f32,
    tag_height: f32,
    _padding: f32,
};

@group(0) @binding(0) var t_texture: texture_2d<f32>;
@group(0) @binding(1) var s_sampler: sampler;
@group(0) @binding(2) var<uniform> uniforms: Uniforms;

// ============================================================================
// Helper Functions
// ============================================================================

/// Signed distance function for a rounded rectangle
/// Returns negative values inside, positive outside
fn sd_rounded_rect(p: vec2<f32>, size: vec2<f32>, radius: f32) -> f32 {
    let q = abs(p) - size + vec2<f32>(radius);
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - radius;
}

// ============================================================================
// Vertex Shader
// ============================================================================

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Generate a fullscreen quad (2 triangles = 6 vertices)
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
    // Local position in pixels (0,0 at top-left, tile_size at bottom-right)
    output.local_pos = (positions[vertex_index] * 0.5 + 0.5) * uniforms.tile_size;
    return output;
}

// ============================================================================
// Fragment Shader
// ============================================================================

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let total_size = uniforms.tile_size;
    let border_radius = uniforms.border_radius;
    let border_width = uniforms.border_width;
    let inner_padding = uniforms.inner_padding;
    
    // Shadow extends beyond the tile to the right and below
    let shadow_extra_x = uniforms.shadow_offset_x + uniforms.shadow_blur;
    let shadow_extra_y = uniforms.shadow_offset_y + uniforms.shadow_blur;
    let content_size = vec2<f32>(total_size.x - shadow_extra_x, total_size.y - shadow_extra_y);
    let half_content = content_size * 0.5;
    
    // Position in the total viewport (0,0 at top-left, bottom-right is total_size)
    let total_pos = input.local_pos;
    
    // Content is positioned at top-left of viewport
    // pos is relative to content center, where content spans from (0,0) to content_size
    let pos = total_pos - half_content;
    
    // Calculate the total padding (border + inner padding)
    let total_padding = border_width + inner_padding;
    
    // Image area dimensions - use actual image_height, not full tile height
    // Image is at the top of the content area
    let image_width = content_size.x - total_padding * 2.0;
    let image_height = uniforms.image_height;
    
    // Image center is offset from tile center (image is at top, label at bottom)
    // In shader coords: pos.y < 0 is top, pos.y > 0 is bottom
    // Image top edge should be at -half_content.y + total_padding
    // Image center Y should be at -half_content.y + total_padding + image_height/2
    let image_center_y = -half_content.y + total_padding + image_height / 2.0;
    let image_half_size = vec2<f32>(image_width / 2.0, image_height / 2.0);
    // Subtract image_center_y to shift the coordinate system so image center is at y=0
    let image_pos = vec2<f32>(pos.x, pos.y + image_center_y);
    
    // Distance to tile outer edge
    let dist_outer = sd_rounded_rect(pos, half_content, border_radius);
    
    // Distance to image area (rectangular, offset to top of tile)
    let dist_image = sd_rounded_rect(image_pos, image_half_size, 0.0);
    
    // Shadow calculation (offset position relative to content)
    // Shadow should appear below and to the right
    let shadow_offset = vec2<f32>(uniforms.shadow_offset_x, uniforms.shadow_offset_y);
    let shadow_pos = pos - shadow_offset;
    let dist_shadow = sd_rounded_rect(shadow_pos, half_content, border_radius);
    let shadow_alpha = (1.0 - smoothstep(-uniforms.shadow_blur, uniforms.shadow_blur, dist_shadow)) * uniforms.shadow_opacity;
    let shadow_color = vec4<f32>(SHADOW_COLOR, shadow_alpha);
    
    // === GLOW EFFECT FOR SELECTION AND HOVER ===
    // Render glow outside the tile for selected items
    if (uniforms.is_selected > 0.5 && dist_outer > 0.0 && dist_outer < GLOW_RADIUS_SELECTED) {
        let glow_intensity = (1.0 - dist_outer / GLOW_RADIUS_SELECTED) * GLOW_INTENSITY_SELECTED;
        let glow_color = vec4<f32>(GLOW_COLOR_SELECTED, glow_intensity);
        return vec4<f32>(
            mix(shadow_color.rgb, glow_color.rgb, glow_color.a),
            max(shadow_color.a, glow_color.a)
        );
    }
    // Render subtle glow for hovered items
    if (uniforms.is_hovered > 0.5 && dist_outer > 0.0 && dist_outer < GLOW_RADIUS_HOVERED) {
        let glow_intensity = (1.0 - dist_outer / GLOW_RADIUS_HOVERED) * GLOW_INTENSITY_HOVERED;
        let glow_color = vec4<f32>(GLOW_COLOR_HOVERED, glow_intensity);
        return vec4<f32>(
            mix(shadow_color.rgb, glow_color.rgb, glow_color.a),
            max(shadow_color.a, glow_color.a)
        );
    }
    
    // Outside tile completely - just shadow
    if (dist_outer > 0.5) {
        return shadow_color;
    }
    
    // Border color based on state
    var border_color: vec4<f32>;
    if (uniforms.is_selected > 0.5) {
        border_color = BORDER_COLOR_SELECTED;
    } else if (uniforms.is_hovered > 0.5) {
        border_color = BORDER_COLOR_HOVERED;
    } else {
        border_color = BORDER_COLOR_NORMAL;
    }
    
    // === GRADIENT BACKGROUND ===
    // Vertical gradient from top (lighter) to bottom (darker)
    let gradient_t = (pos.y + half_content.y) / content_size.y;
    let bg_color = mix(GRADIENT_TOP, GRADIENT_BOTTOM, gradient_t);
    
    // Inside image area - sample texture
    if (dist_image < 0.0) {
        // Calculate texture coordinates for just the image area
        let tex_pos = image_pos + image_half_size;
        let tex_size = image_half_size * 2.0;
        var tex_coord = tex_pos / tex_size;
        // Flip Y coordinate to correct upside-down rendering
        tex_coord.y = 1.0 - tex_coord.y;
        
        var color = textureSample(t_texture, s_sampler, tex_coord);
        
        // Apply selection tint or hover intensity boost
        /*if (uniforms.is_selected > 0.5) {
            color = mix(color, SELECTION_TINT, SELECTION_TINT_AMOUNT);
        } else */if (uniforms.is_hovered > 0.5) {
            // Increase intensity - multiply to keep blacks black
            // Use a curve that boosts mids/highs more than darks
            color = vec4<f32>(pow(color.rgb, vec3<f32>(HOVER_GAMMA)) * HOVER_BOOST, color.a);
            color = clamp(color, vec4<f32>(0.0), vec4<f32>(1.0));
        }
        
        return color;
    }
    
    // In border area or padding/label area
    if (dist_outer < 0.5) {
        // Smooth edge for anti-aliasing
        let outer_alpha = 1.0 - smoothstep(-0.5, 0.5, dist_outer);
        
        // Check if we're in the actual border or the inner padding
        let border_inner_dist = dist_outer + border_width;
        if (border_inner_dist < 0.0) {
            // Inside the border: padding + image + label

            // Inner top/bottom inside border+padding
            let inner_top = -half_content.y + total_padding;
            let inner_bottom =  half_content.y - total_padding;

            // The image was positioned using image_center_y and image_half_size
            let image_top_y    = image_center_y + image_half_size.y;
            let image_bottom_y = image_center_y - image_half_size.y;

            // Label starts just below image + padding, ends at inner bottom
            let label_start_y = image_bottom_y + border_width;
            let label_end_y   = label_start_y + total_size.y - (image_height + 2.0 * total_padding + 4.0 * inner_padding);

            let inner_left  = -half_content.x + total_padding;
            let inner_right =  half_content.x - total_padding;

            // Only fill a *strip* below the image, inside padding, and only if we have a tag
            if (pos.x >= inner_left &&
                pos.x <= inner_right &&
                pos.y >= label_start_y &&
                pos.y <= label_end_y) {
                return mix(shadow_color, LABEL_BG_COLOR, outer_alpha);
            }
            // In the inner padding area - use padding fill color
            return mix(shadow_color, PADDING_FILL_COLOR, outer_alpha);
        } else {
            // In the border
            return mix(shadow_color, border_color, outer_alpha);
        }
    }
    
    return shadow_color;
}
