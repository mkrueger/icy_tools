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
    pad0: f32,
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

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let distorted_uv = apply_curvature(in.tex_coord);

    if (distorted_uv.x < 0.0 || distorted_uv.y < 0.0 || distorted_uv.x > 1.0 || distorted_uv.y > 1.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    let tex_color = textureSample(terminal_texture, terminal_sampler, distorted_uv);
    var color = adjust_color(tex_color.rgb);

    color = apply_scanlines(color, distorted_uv);

    if (uniforms.monitor_type < 0.5) {
        return vec4<f32>(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), tex_color.a);
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