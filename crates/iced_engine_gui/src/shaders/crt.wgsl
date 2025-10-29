struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
}

// Match Rust layout: scalars + vec2 + scalars + padding
struct Uniforms {
    time: f32,
    brightness: f32,
    contrast: f32,
    gamma: f32,
    saturation: f32,
    monitor_type: f32,
    resolution: vec2<f32>,
    curvature_x: f32,
    curvature_y: f32,
    enable_curvature: f32,
    scanline_thickness: f32,
    scanline_sharpness: f32,
    scanline_phase: f32,
    enable_scanlines: f32,
    
    noise_level: f32,
    sync_wobble: f32,
    enable_noise: f32,
    
    bloom_threshold: f32,
    bloom_radius: f32,
    bloom_intensity: f32,
    enable_bloom: f32,

}

struct MonitorColor {
    color: vec4<f32>,
}

@group(0) @binding(0) var terminal_texture: texture_2d<f32>;
@group(0) @binding(1) var terminal_sampler: sampler;
@group(0) @binding(2) var<uniform> uniforms: Uniforms;
@group(0) @binding(3) var<uniform> monitor_color: MonitorColor;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    let x = f32(i32(vertex_index & 1u) * 4 - 1);
    let y = f32(i32(vertex_index & 2u) * 2 - 1);
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.tex_coord = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

fn adjust_color(base: vec3<f32>) -> vec3<f32> {
    var color = base;

    let g = uniforms.gamma;
    if (g > 0.0001 && abs(g - 1.0) > 0.00001) {
        color = pow(color, vec3<f32>(1.0 / g));
    }

    let bm = uniforms.brightness;
    if (abs(bm - 1.0) > 0.00001) {
        color = color * bm;
    }

    let cf = uniforms.contrast;
    if (abs(cf - 1.0) > 0.00001) {
        color = (color - vec3<f32>(0.5)) * cf + vec3<f32>(0.5);
    }

    let s = uniforms.saturation;
    if (uniforms.monitor_type < 0.5 && abs(s - 1.0) > 0.00001) {
        let gray = dot(color, vec3<f32>(0.299, 0.587, 0.114));
        color = mix(vec3<f32>(gray), color, s);
    }

    return color;
}

fn apply_curvature(uv_in: vec2<f32>) -> vec2<f32> {
    if (uniforms.enable_curvature < 0.5) {
        return uv_in;
    }

    var uv = uv_in * 2.0 - 1.0;
    let offset = abs(uv.yx) / vec2(uniforms.curvature_x, uniforms.curvature_y);
    uv = uv + uv * offset * offset;
    uv = uv * 0.5 + 0.5;
    return uv;
}

fn apply_scanlines(color_in: vec3<f32>, tex_coord: vec2<f32>) -> vec3<f32> {
    if (uniforms.enable_scanlines < 0.5) {
        return color_in;
    }

    // Determine pixel row (using resolution uniform)
    let row_f = tex_coord.y * uniforms.resolution.y + uniforms.scanline_phase * uniforms.resolution.y;
    let row = floor(row_f);

    // Alternate pattern (0 or 1)
    let is_dark_row = f32(i32(row) & 1);

    // thickness controls how deep dark rows get; map to multiplier
    // thickness=0 -> 0.9 (subtle), thickness=1 -> 0.3 (strong)
    let min_mul = mix(0.9, 0.3, uniforms.scanline_thickness);

    // sharpness controls edge softness between rows using fractional position within row
    let frac = fract(row_f);
    // For dark rows, fade toward min_mul near row center; for bright rows, keep near 1.0
    // Use a bell-ish curve via smoothstep chain
    let shape = 1.0 - smoothstep(0.0, 1.0, abs(frac - 0.5) * 2.0);
    let edge_mix = mix(1.0, min_mul, pow(shape, mix(0.5, 4.0, uniforms.scanline_sharpness)));

    // Apply only to dark rows
    let mul = mix(1.0, edge_mix, is_dark_row);

    return color_in * mul;
}

fn rand(p: vec2<f32>) -> f32 {
    // Simple hash-based pseudo-random; deterministic per frame, animated via time
    let h = dot(p + uniforms.time * 0.37, vec2<f32>(12.9898, 78.233));
    return fract(sin(h) * 43758.5453);
}

fn apply_noise(color_in: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {
    if (uniforms.enable_noise < 0.5 || uniforms.noise_level <= 0.00001) {
        return color_in;
    }
    // Two samples for a bit more variation
    let n1 = rand(uv * 1.0);
    let n2 = rand(uv * 7.13 + vec2<f32>(uniforms.time * 0.11, uniforms.time * 0.07));
    let n = (n1 + n2) * 0.5;       // 0..1
    let grain = (n - 0.5) * 2.0;   // -1..1
    // Scale noise level; reduce influence on very dark pixels (simulate phosphor response)
    let luma = dot(color_in, vec3<f32>(0.299, 0.587, 0.114));
    let attenuation = mix(0.6, 1.0, luma); // darker areas less noisy
    let strength = uniforms.noise_level * 2.0 * attenuation;
    let noisy = color_in + grain * strength * 0.15; // 0.15 base amplitude
    return clamp(noisy, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn apply_sync_wobble(uv: vec2<f32>) -> vec2<f32> {
    if (uniforms.enable_noise < 0.5 || uniforms.noise_level <= 0.00001) {
        return uv;
    }
    
    // Create time-based sync distortion
    let wobble_speed = 3.5;
    let wobble_lines = 5.0;
    
    // Vertical position affects distortion amount
    let distort_amount = sin(uv.y * wobble_lines + uniforms.time * wobble_speed) * 0.5 + 0.5;
    
    // Add some noise to make it more irregular
    let noise = rand(vec2<f32>(uv.y * 10.0, uniforms.time)) - 0.5;
    
    // Horizontal offset based on sync wobble strength
    let offset_x = (distort_amount * noise) * uniforms.sync_wobble * 0.02;
    
    return vec2<f32>(uv.x + offset_x, uv.y);
}

fn apply_bloom(uv: vec2<f32>) -> vec3<f32> {
    if (uniforms.enable_bloom < 0.5 || uniforms.bloom_intensity <= 0.001) {
        return vec3<f32>(0.0);
    }

    // Sample the original texture (before effects)
    let center_color = textureSample(terminal_texture, terminal_sampler, uv).rgb;
    
    // Apply color adjustments to match what we're displaying
    let adjusted = adjust_color(center_color);
    let luma = dot(adjusted, vec3<f32>(0.299, 0.587, 0.114));

    // Threshold check
    let t = uniforms.bloom_threshold;
    if (luma < t) {
        return vec3<f32>(0.0);
    }

    // Radius in pixels
    let radius = max(uniforms.bloom_radius, 1.0);
    let px = radius / uniforms.resolution;

    // Accumulate bright samples
    var glow = vec3<f32>(0.0);
    var total_weight = 0.0;

    // 9-tap kernel (center + 8 surrounding)
    for (var dy: f32 = -1.0; dy <= 1.0; dy = dy + 1.0) {
        for (var dx: f32 = -1.0; dx <= 1.0; dx = dx + 1.0) {
            let offset = vec2<f32>(dx, dy) * px;
            let sample_uv = clamp(uv + offset, vec2<f32>(0.0), vec2<f32>(1.0));
            
            // Sample and adjust
            let sample_color = textureSample(terminal_texture, terminal_sampler, sample_uv).rgb;
            let sample_adjusted = adjust_color(sample_color);
            let sample_luma = dot(sample_adjusted, vec3<f32>(0.299, 0.587, 0.114));
            
            // Only accumulate bright pixels
            if (sample_luma > t) {
                // Distance-based weight
                let dist = length(vec2<f32>(dx, dy));
                let weight = exp(-dist * dist * 0.5); // Gaussian falloff
                
                // Extract excess brightness
                let excess = (sample_luma - t) / (1.0 - t);
                glow = glow + sample_adjusted * excess * weight;
                total_weight = total_weight + weight;
            }
        }
    }

    if (total_weight > 0.001) {
        glow = glow / total_weight;
        // Scale by intensity and add slight color boost
        return glow * uniforms.bloom_intensity * 0.5;
    }

    return vec3<f32>(0.0);
}


@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let wobbled_uv = apply_sync_wobble(in.tex_coord);
    let distorted_uv = apply_curvature(wobbled_uv);

    if (distorted_uv.x < 0.0 || distorted_uv.y < 0.0 || distorted_uv.x > 1.0 || distorted_uv.y > 1.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    let tex_color = textureSample(terminal_texture, terminal_sampler, distorted_uv);
    var color = adjust_color(tex_color.rgb);

    // Calculate bloom from original bright areas
    let bloom_glow = apply_bloom(distorted_uv);
    
    // Apply post-processing effects
    color = apply_scanlines(color, distorted_uv);
    color = apply_noise(color, distorted_uv);
    
    // Add bloom on top
    color = color + bloom_glow;
    
    // Final clamp before monitor type conversion
    color = clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));

    if (uniforms.monitor_type < 0.5) {
        return vec4<f32>(color, tex_color.a);
    } else if (uniforms.monitor_type < 1.5) {
        let gray = dot(color, vec3<f32>(0.299, 0.587, 0.114));
        return vec4<f32>(vec3<f32>(gray), tex_color.a);
    } else {
        let gray = dot(color, vec3<f32>(0.299, 0.587, 0.114));
        let tint = monitor_color.color.rgb;
        let max_comp = max(tint.r, max(tint.g, tint.b));
        let norm_tint = select(vec3<f32>(1.0), tint / max_comp, max_comp > 0.0001);
        var mono = gray * norm_tint * 1.5;
        mono = mono + norm_tint * 0.05;
        mono = mono / (mono + vec3<f32>(1.0)) * 2.0;
        return vec4<f32>(clamp(mono, vec3<f32>(0.0), vec3<f32>(1.0)), tex_color.a);
    }
}