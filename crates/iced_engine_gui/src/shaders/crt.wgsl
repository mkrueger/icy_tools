struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
}

struct Uniforms {
    time: f32,
    scan_line_intensity: f32,
    curvature: f32,
    bloom: f32,
    gamma: f32,
    contrast: f32,
    saturation: f32,
    brightness: f32,
    light: f32,
    blur: f32,
    resolution: vec2<f32>,
    use_filter: f32,
    monitor_type: f32,
}

struct MonitorColor {
    color: vec4<f32>, // RGBA for monochrome tinting
}

@group(0) @binding(0) var terminal_texture: texture_2d<f32>;
@group(0) @binding(1) var terminal_sampler: sampler;
@group(0) @binding(2) var<uniform> uniforms: Uniforms;
@group(0) @binding(3) var<uniform> monitor_color: MonitorColor;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    
    // Generate a large triangle that covers the entire screen
    let x = f32(i32(vertex_index & 1u) * 4 - 1);
    let y = f32(i32(vertex_index & 2u) * 2 - 1);
    
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.tex_coord = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    
    return out;
}

fn postEffects(rgb: vec3<f32>) -> vec4<f32> {
    var color = rgb;
    
    // ONLY apply monitor color conversion for non-color modes
    // Color mode = 0.0, so skip conversion
    if (uniforms.monitor_type >= 1.0) {
        // Calculate luminance using perceptual weights
        let gray = dot(color, vec3<f32>(0.299, 0.587, 0.114));
        
        // Grayscale mode (1.0): just return luminance
        if (uniforms.monitor_type < 1.5) {
            color = vec3<f32>(gray);
        } else {
            // Monochrome tint modes (2.0+)
            let tint = monitor_color.color.rgb;
            
            // Find the maximum component to normalize against
            let max_comp = max(tint.r, max(tint.g, tint.b));
            let norm_tint = select(vec3<f32>(1.0), tint / max_comp, max_comp > 0.0001);
            
            // Apply tint to grayscale value
            color = gray * norm_tint;
            
            // Boost brightness for monochrome monitors
            color = color * 1.5;
            
            // Add a slight base glow to simulate phosphor minimum brightness
            color = color + norm_tint * 0.05;
            
            // Soft clamp to avoid harsh cutoff while preserving bright areas
            color = color / (color + vec3<f32>(1.0)) * 2.0;
        }
    }
    
    
    if (uniforms.use_filter > 0.5) {
        // Apply gamma correction
        color = pow(color, vec3<f32>(uniforms.gamma));
        
        // For monochrome modes, boost brightness before contrast/saturation adjustments
        if (uniforms.monitor_type > 1.5) {
            color = color * 1.2;
        }
        
        let luminance = dot(vec3<f32>(0.2125, 0.7154, 0.0721), color * uniforms.brightness);
        color = mix(
            vec3<f32>(0.5),
            mix(vec3<f32>(luminance), color * uniforms.brightness, uniforms.saturation),
            uniforms.contrast
        );
        
        // Final brightness compensation for monochrome modes
        if (uniforms.monitor_type > 1.5) {
            // Increase overall brightness and add a subtle glow
            color = color * 1.1 + monitor_color.color.rgb * 0.02;
        }
    }
    
    // Ensure we don't exceed valid range
    color = clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));
    
    return vec4<f32>(color, 1.0);
}

// Gaussian blur for CRT glow effect
fn gaussian(uv: vec2<f32>) -> vec3<f32> {
    let b = uniforms.blur / (uniforms.resolution.x / uniforms.resolution.y);
    let inv_res = 1.0 / uniforms.resolution;
    
    var col = vec3<f32>(0.0);
    
    // 3x3 Gaussian kernel
    col += textureSample(terminal_texture, terminal_sampler, uv + vec2<f32>(-b, -b) * inv_res).rgb * 0.077847;
    col += textureSample(terminal_texture, terminal_sampler, uv + vec2<f32>(-b, 0.0) * inv_res).rgb * 0.123317;
    col += textureSample(terminal_texture, terminal_sampler, uv + vec2<f32>(-b, b) * inv_res).rgb * 0.077847;
    
    col += textureSample(terminal_texture, terminal_sampler, uv + vec2<f32>(0.0, -b) * inv_res).rgb * 0.123317;
    col += textureSample(terminal_texture, terminal_sampler, uv).rgb * 0.195346;
    col += textureSample(terminal_texture, terminal_sampler, uv + vec2<f32>(0.0, b) * inv_res).rgb * 0.123317;
    
    col += textureSample(terminal_texture, terminal_sampler, uv + vec2<f32>(b, -b) * inv_res).rgb * 0.077847;
    col += textureSample(terminal_texture, terminal_sampler, uv + vec2<f32>(b, 0.0) * inv_res).rgb * 0.123317;
    col += textureSample(terminal_texture, terminal_sampler, uv + vec2<f32>(b, b) * inv_res).rgb * 0.077847;
    
    return col;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Handle filter disabled case - direct rendering
    if (uniforms.use_filter < 0.5) {
        let tex_color = textureSample(terminal_texture, terminal_sampler, in.tex_coord);
        // For non-filter mode, still apply monitor type conversion if needed
        if (uniforms.monitor_type < 0.5) {
            // Color mode: pass through unchanged
            return tex_color;
        } else if (uniforms.monitor_type < 1.5) {
            // Grayscale mode
            let gray = dot(tex_color.rgb, vec3<f32>(0.299, 0.587, 0.114));
            return vec4<f32>(vec3<f32>(gray), tex_color.a);
        } else {
            // Monochrome tint modes
            let gray = dot(tex_color.rgb, vec3<f32>(0.299, 0.587, 0.114));
            let tint = monitor_color.color.rgb;
            let max_comp = max(tint.r, max(tint.g, tint.b));
            let norm_tint = select(vec3<f32>(1.0), tint / max_comp, max_comp > 0.0001);
            return vec4<f32>(gray * norm_tint * 1.5, tex_color.a);
        }
    }

    // Apply CRT distortion for curved monitors
    var uv = in.tex_coord;
    if (uniforms.curvature > 0.0) {
        // Apply barrel distortion
        let centered = (uv - 0.5) * 2.0;
        let r2 = dot(centered, centered);
        let distortion = 1.0 + uniforms.curvature * r2;
        uv = (centered / distortion) * 0.5 + 0.5;
        
        // Check if we're outside the distorted area
        if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
            return vec4<f32>(0.0, 0.0, 0.0, 1.0);
        }
    }

    // Apply Gaussian blur if needed
    var tex_color: vec3<f32>;
    if (uniforms.blur > 0.0) {
        tex_color = gaussian(uv);
    } else {
        tex_color = textureSample(terminal_texture, terminal_sampler, uv).rgb;
    }

    // Apply post-processing effects (scanlines, color adjustments, etc.)
    var color = postEffects(tex_color);

    // Apply scanlines
    if (uniforms.scan_line_intensity > 0.0) {
        let pixel_y = in.tex_coord.y * uniforms.resolution.y;
        let line = sin(pixel_y * 3.14159265) * 0.5 + 0.5;
        let scanline = mix(1.0, line, uniforms.scan_line_intensity);
        color = vec4<f32>(color.rgb * scanline, color.a);
    }

    // Simulate screen edge darkening (vignette)
    if (uniforms.use_filter > 0.5) {
        let centered = abs(in.tex_coord - 0.5) * 2.0;
        let vignette = 1.0 - dot(centered, centered) * uniforms.light;
        color = vec4<f32>(color.rgb * vignette, color.a);
    }

    return color;
}