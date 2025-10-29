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
    use_filter: f32,  // 1.0 if filter enabled, 0.0 otherwise
    monitor_type: f32, // Enum as float (0=Color, 1=Grayscale, 2=Amber, etc.)
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

// Apply monitor type color conversion
fn applyMonitorColor(rgb: vec3<f32>) -> vec3<f32> {
    // Color mode: unchanged
    if (uniforms.monitor_type < 0.5) {
        return rgb;
    }

    // Calculate luminance using perceptual weights
    let gray = dot(rgb, vec3<f32>(0.299, 0.587, 0.114));
    
    // Grayscale mode: just return luminance
    if (uniforms.monitor_type < 1.5) {
        return vec3<f32>(gray);
    }

    // Monochrome tint modes (amber/green/etc.)
    let tint = monitor_color.color.rgb;
    
    // Find the maximum component to normalize against
    let max_comp = max(tint.r, max(tint.g, tint.b));
    let norm_tint = select(vec3<f32>(1.0), tint / max_comp, max_comp > 0.0001);
    
    // Apply tint to grayscale value
    var colored = gray * norm_tint;
    
    // Boost brightness significantly for monochrome monitors
    // Old CRT phosphors were actually quite bright
    colored = colored * 1.5;
    
    // Add a slight base glow to simulate phosphor minimum brightness
    colored = colored + norm_tint * 0.05;
    
    // Soft clamp to avoid harsh cutoff while preserving bright areas
    colored = colored / (colored + vec3<f32>(1.0)) * 2.0;
    
    return colored;
}

fn postEffects(rgb: vec3<f32>) -> vec4<f32> {
    var color = applyMonitorColor(rgb);

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
    }
    
    // Final brightness compensation for monochrome modes
    if (uniforms.monitor_type > 1.5) {
        // Increase overall brightness and add a subtle glow
        color = color * 1.1 + monitor_color.color.rgb * 0.02;
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
    var uv = in.tex_coord;
    
    // Clamp UV coordinates to [0, 1] range
    uv = clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0));
    
    // Only apply CRT effects if filter is enabled
    if (uniforms.use_filter > 0.5) {
        // Apply CRT screen curvature
        let st = uv - vec2<f32>(0.5);
        let d = length(st * 0.5 * st * 0.5 * uniforms.curvature);
        uv = st * d + st + vec2<f32>(0.5);
        
        // Ensure UV coordinates stay within bounds after curvature
        if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
            return vec4<f32>(0.0, 0.0, 0.0, 1.0);
        }
    }
    
    // Calculate distance for effects
    let st = uv - vec2<f32>(0.5);
    let d = length(st * 0.5 * st * 0.5 * uniforms.curvature);
    
    // Border mask - fade edges to black for curved screen effect
    var border_mask = 1.0;
    if (uniforms.use_filter > 0.5 && uniforms.curvature > 0.0) {
        let edge_dist = max(abs(st.x), abs(st.y));
        border_mask = max(0.0, 1.0 - 2.0 * edge_dist);
        border_mask = min(border_mask * 200.0, 1.0);
    }
    
    // Sample the terminal texture with blur/glow
    var col = vec3<f32>(0.0);
    if (uniforms.use_filter > 0.5 && uniforms.blur > 0.0) {
        col = gaussian(uv);
    } else {
        col = textureSample(terminal_texture, terminal_sampler, uv).rgb;
    }
    
    // Apply vignette/light falloff - less aggressive for monochrome
    if (uniforms.use_filter > 0.5 && uniforms.light > 0.0) {
        var light_strength = uniforms.light;
        // Reduce vignette effect for monochrome modes
        if (uniforms.monitor_type > 1.5) {
            light_strength *= 0.5;
        }
        let light_falloff = 1.0 - min(1.0, d * light_strength);
        col *= light_falloff;
    }
    
    // Scanlines effect - adjusted for monochrome
    if (uniforms.use_filter > 0.5 && uniforms.scan_line_intensity > 0.0) {
        let y = uv.y;
        
        var show_scanlines = 1.0;
        if (uniforms.resolution.y < 360.0) {
            show_scanlines = 0.0;
        }
        
        // Reduce scanline darkness for monochrome modes
        var scanline_strength = uniforms.scan_line_intensity;
        if (uniforms.monitor_type > 1.5) {
            scanline_strength *= 0.6; // Make scanlines less aggressive
        }
        
        let s = 1.0 - smoothstep(320.0, 1440.0, uniforms.resolution.y) + 1.0;
        let scanline = cos(y * uniforms.resolution.y * s) * scanline_strength;
        col = abs(show_scanlines - 1.0) * col + show_scanlines * (col - col * scanline);
        
        // RGB separation effect - gentler for monochrome
        var rgb_sep_strength = 1.0;
        if (uniforms.monitor_type > 1.5) {
            rgb_sep_strength = 0.5; // Reduce RGB separation darkening
        }
        let rgb_sep = (0.01 + ceil((uv.x * uniforms.resolution.x) % 3.0) * (0.995 - 1.01)) * show_scanlines * rgb_sep_strength;
        col *= 1.0 - rgb_sep;
    }
    
    // Apply border mask
    col *= border_mask;
    
    // Add phosphor glow effect - stronger for monochrome
    if (uniforms.use_filter > 0.5 && uniforms.bloom > 0.0) {
        let glow_offset = vec2<f32>(1.0 / uniforms.resolution.x, 0.0);
        let glow = textureSample(terminal_texture, terminal_sampler, uv + glow_offset).rgb;
        var bloom_strength = uniforms.bloom * 0.3;
        if (uniforms.monitor_type > 1.5) {
            bloom_strength *= 1.5; // More glow for phosphor monitors
        }
        col = mix(col, glow, bloom_strength);
    }
    
    // Add screen flicker - subtler for readability
    if (uniforms.use_filter > 0.5) {
        let flicker = 1.0 + sin(uniforms.time * 60.0) * 0.005; // Reduced from 0.01
        col *= flicker;
    }
    
    // Apply final post-processing
    return postEffects(col);
}