// F-Key Toolbar Background Shader
// Matches the SegmentedControl/Tool Panel styling:
// - Rounded corners
// - Drop shadow
// - Solid border with hover glow per slot
// 
// Text/glyphs are rendered as a Canvas overlay on top of this shader.

struct Uniforms {
    // Widget top-left (in physical screen coordinates)
    widget_origin: vec2<f32>,
    // Widget dimensions (physical pixels)
    widget_size: vec2<f32>,
    // Number of slots (12 for F1-F12)
    num_slots: u32,
    // Hovered slot index (0xFFFFFFFF = none)
    hovered_slot: u32,
    // Hovered element type: 0=slot_label, 1=slot_char, 2=nav_prev, 3=nav_next
    hover_type: u32,
    // Padding
    _flags: u32,
    // Corner radius
    corner_radius: f32,
    // Time for animations
    time: f32,
    // Slot width
    slot_width: f32,
    // Slot spacing
    slot_spacing: f32,
    // Background color from theme
    bg_color: vec4<f32>,
    // Content start X (after shadow padding + border)
    content_start_x: f32,
    // Label width
    label_width: f32,
    // Nav section start X
    nav_start_x: f32,
    // Nav button size
    nav_size: f32,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(-1.0, -1.0)
    );
    
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

// ═══════════════════════════════════════════════════════════════════════════
// Design Constants (matching SegmentedControl/Tool Panel)
// ═══════════════════════════════════════════════════════════════════════════

const SHADOW_PADDING: f32 = 6.0;
const BORDER_WIDTH: f32 = 1.0;

// Drop shadow
const SHADOW_OFFSET: vec2<f32> = vec2<f32>(2.0, 2.0);
const SHADOW_BLUR: f32 = 3.0;
const SHADOW_ALPHA: f32 = 0.4;

// Colors (matching tool_panel_shader.wgsl / segmented_control_shader.wgsl)
const HOVER_BG: vec3<f32> = vec3<f32>(0.28, 0.28, 0.35);
const BORDER_COLOR: vec3<f32> = vec3<f32>(0.35, 0.35, 0.4);
const HOVER_BORDER: vec3<f32> = vec3<f32>(0.45, 0.55, 0.7);
const HOVER_GLOW: vec3<f32> = vec3<f32>(0.5, 0.7, 1.0);

// ═══════════════════════════════════════════════════════════════════════════
// SDF Functions
// ═══════════════════════════════════════════════════════════════════════════

fn rounded_rect_sdf(p: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let r = min(radius, min(half_size.x, half_size.y));
    let d = abs(p) - half_size + vec2<f32>(r);
    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0) - r;
}

// ═══════════════════════════════════════════════════════════════════════════
// Slot/Nav Helpers
// ═══════════════════════════════════════════════════════════════════════════

// Get slot index at x position (returns -1 if not in any slot)
fn get_slot_at(x: f32) -> i32 {
    let start_x = uniforms.content_start_x;
    let slot_total = uniforms.slot_width + uniforms.slot_spacing;
    
    for (var i = 0u; i < uniforms.num_slots; i++) {
        let slot_x = start_x + f32(i) * slot_total;
        if x >= slot_x && x < slot_x + uniforms.slot_width {
            return i32(i);
        }
    }
    return -1;
}

// Check if x is in label area of slot
fn is_in_label_area(x: f32, slot: u32) -> bool {
    let start_x = uniforms.content_start_x;
    let slot_total = uniforms.slot_width + uniforms.slot_spacing;
    let slot_x = start_x + f32(slot) * slot_total;
    return x < slot_x + uniforms.label_width;
}

// Check if position is in nav prev button
fn is_in_nav_prev(x: f32, y: f32, content_h: f32) -> bool {
    let nav_y = (content_h - uniforms.nav_size) * 0.5;
    return x >= uniforms.nav_start_x && x < uniforms.nav_start_x + uniforms.nav_size &&
           y >= nav_y && y < nav_y + uniforms.nav_size;
}

// Check if position is in nav next button
fn is_in_nav_next(x: f32, y: f32, content_h: f32) -> bool {
    let next_x = uniforms.nav_start_x + uniforms.nav_size + 16.0;
    let nav_y = (content_h - uniforms.nav_size) * 0.5;
    return x >= next_x && x < next_x + uniforms.nav_size &&
           y >= nav_y && y < nav_y + uniforms.nav_size;
}

// ═══════════════════════════════════════════════════════════════════════════
// Fragment Shader
// ═══════════════════════════════════════════════════════════════════════════

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let pixel = input.position.xy - uniforms.widget_origin;
    let size = uniforms.widget_size;
    
    // Control area (inside shadow padding)
    let ctrl_x = SHADOW_PADDING;
    let ctrl_y = SHADOW_PADDING;
    let ctrl_w = size.x - SHADOW_PADDING * 2.0;
    let ctrl_h = size.y - SHADOW_PADDING * 2.0;
    let ctrl_center = vec2<f32>(ctrl_x + ctrl_w * 0.5, ctrl_y + ctrl_h * 0.5);
    let ctrl_half = vec2<f32>(ctrl_w * 0.5, ctrl_h * 0.5);
    
    let radius = uniforms.corner_radius;
    
    // Main control SDF
    let ctrl_sdf = rounded_rect_sdf(pixel - ctrl_center, ctrl_half, radius);
    
    // ─────────────────────────────────────────────────────────────────────────
    // Outside control: draw drop shadow
    // ─────────────────────────────────────────────────────────────────────────
    if ctrl_sdf > 0.5 {
        let shadow_pos = pixel - ctrl_center - SHADOW_OFFSET;
        let shadow_sdf = rounded_rect_sdf(shadow_pos, ctrl_half, radius);
        
        let blur_factor = 1.0 / SHADOW_BLUR;
        let shadow_alpha = SHADOW_ALPHA * exp(-max(shadow_sdf, 0.0) * blur_factor);
        
        if shadow_alpha > 0.01 {
            return vec4<f32>(0.0, 0.0, 0.0, shadow_alpha);
        }
        return vec4<f32>(0.0);
    }
    
    // ─────────────────────────────────────────────────────────────────────────
    // Border + content masking
    // ─────────────────────────────────────────────────────────────────────────
    let outer_a = smoothstep(0.5, -0.5, ctrl_sdf);
    
    let inner_half = ctrl_half - vec2<f32>(BORDER_WIDTH, BORDER_WIDTH);
    let inner_radius = max(radius - BORDER_WIDTH, 0.0);
    let inner_sdf = rounded_rect_sdf(pixel - ctrl_center, inner_half, inner_radius);
    let inner_a = smoothstep(0.5, -0.5, inner_sdf);
    
    // Content area
    let content_x = ctrl_x + BORDER_WIDTH;
    let content_y = ctrl_y + BORDER_WIDTH;
    let content_w = ctrl_w - BORDER_WIDTH * 2.0;
    let content_h = ctrl_h - BORDER_WIDTH * 2.0;
    
    // Local position within content area
    let local_x = pixel.x - content_x;
    let local_y = pixel.y - content_y;
    
    // Border color - always neutral (no hover highlighting on border)
    let border_rgb = BORDER_COLOR;
    
    // ─────────────────────────────────────────────────────────────────────────
    // Inside control: background fill with hover highlights
    // ─────────────────────────────────────────────────────────────────────────
    var color = uniforms.bg_color.rgb;
    
    // Check for hover highlight on slots - removed backdrop, FG color is handled in canvas overlay
    // No background highlight needed - slots stay with bg_color
    
    // Check for nav button hover - removed backdrop, FG color is handled in SVG layer
    // No background highlight needed - nav buttons stay with bg_color
    
    // Composite: border outside inner area, content inside
    let final_rgb = mix(border_rgb, color, inner_a);
    return vec4<f32>(final_rgb, outer_a);
}
