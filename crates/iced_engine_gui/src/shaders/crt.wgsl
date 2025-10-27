struct VertexOutput {
    @builtin(position) gl_Position: vec4<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    let uv = vec2(f32((index << 1) & 2), f32(index & 2));
    return VertexOutput(vec4(uv * vec2(2.0, -2.0) + vec2(-1.0, 1.0), 0.0, 1.0));
}

struct FragmentOutput {
    @location(0) CrtColor: vec4<f32>,
    @location(1) AccumulateOut: vec4<f32>,
}

@group(0) @binding(0)
var text: texture_2d<f32>;
@group(0) @binding(1)
var text_s: sampler;

@group(1) @binding(0)
var accumulate_tex: texture_2d<f32>;
@group(1) @binding(1)
var accumulate_tex_s: sampler;

@group(2) @binding(0)
var blur_tex: texture_2d<f32>;
@group(2) @binding(1)
var blur_tex_s: sampler;

struct Uniforms {
    modulate_crt: vec3<f32>,
    resolution: vec2<f32>,
    brightness: f32,
    modulate_accumulate: f32,
    modulate_blend: f32,
    slow_fade: i32,
    curve_factor: f32,
    ghost_factor: f32,
    scanline_factor: f32,
    corner_radius: f32,
    mask_type: f32,
    mask_strength: f32,
    use_srgb: i32,
    milliseconds: u32,
}

@group(3) @binding(0)
var<uniform> uniforms: Uniforms;

// From Timothy Lottes
fn mask(pos: vec2<f32>, dark: f32) -> vec3<f32> {
    var mask = vec3(dark);

    if uniforms.mask_type == 1.0 {
        // TV
        let odd = f32(fract(pos.x * 1.0 / 6.0) < 0.5);
        let line = select(1.0, dark, fract((pos.y + odd) * 0.5) < 0.5);

        let x = fract(pos.x * 1.0 / 3.0);
        if x < 1.0 / 3.0 {
            mask.r = 1.0;
        } else if x < 2.0 / 3.0 {
            mask.g = 1.0;
        } else {
            mask.b = 1.0;
        }

        mask *= line;
    } else if uniforms.mask_type == 2.0 {
        // Aperture-grille
        let x = fract(pos.x * 1.0 / 3.0);
        if x < 1.0 / 3.0 {
            mask.r = 1.0;
        } else if x < 2.0 / 3.0 {
            mask.g = 1.0;
        } else {
            mask.b = 1.0;
        }
    } else if uniforms.mask_type == 3.0 {
        // Stretched VGA
        var x = pos.x + pos.y * 3.0;
        x = fract(pos.x * 1.0 / 6.0);
        if x < 1.0 / 3.0 {
            mask.r = 1.0;
        } else if x < 2.0 / 3.0 {
            mask.g = 1.0;
        } else {
            mask.b = 1.0;
        }
    } else if uniforms.mask_type == 4.0 {
        // VGA
        var adjusted = floor(pos.xy * vec2(1.0, 0.5));
        var x = adjusted.x + adjusted.y * 3.0;
        x = fract(pos.x * 1.0 / 6.0);
        if x < 1.0 / 3.0 {
            mask.r = 1.0;
        } else if x < 2.0 / 3.0 {
            mask.g = 1.0;
        } else {
            mask.b = 1.0;
        }
    }

    return mask;
}

fn curve(uv: vec2<f32>) -> vec2<f32> {
    var curved_uv = (uv * 2.0) - 1.0;

    curved_uv *= vec2(
        1.0 + curved_uv.y * curved_uv.y * 0.031,
        1.0 + curved_uv.x * curved_uv.x * 0.041
    );
    curved_uv = (curved_uv / 2.0) + 0.5;

    return (curved_uv - uv) * vec2<f32>(uniforms.curve_factor) + uv;
}

fn distance(uv: vec2<f32>) -> f32 {
    let in_quad = abs(uv * 2.0 - 1.0);
    let extend = uniforms.resolution / 2.0;
    let coords = in_quad * (extend + uniforms.corner_radius);
    let delta = max(coords - extend, vec2<f32>(0.0));

    return length(delta);
}

fn accumulate(uv: vec2<f32>) -> vec4<f32> {
    let out = textureSample(text, text_s, uv) * uniforms.modulate_accumulate;

    if uniforms.slow_fade == 0 {
        return out;
    }

    let timePassed = f32(uniforms.milliseconds % 333) / 333.0;
    let current = textureSample(accumulate_tex, accumulate_tex_s, uv) - timePassed;

    return mix(max(current, out), out, timePassed);
}


@fragment 
fn fs_main(@builtin(position) gl_Position: vec4<f32>) -> FragmentOutput {
    let uv = gl_Position.xy / uniforms.resolution;

    let factor = select(2.2, 1.0, uniforms.use_srgb == 0);
    let acc = accumulate(uv);

    // Curve
    let curved_uv = mix(curve(uv), uv, 0.4);

    // Main color
    var col: vec3<f32>;
    if uniforms.slow_fade == 0 {
        col = textureSample(text, text_s, curved_uv).rgb + uniforms.brightness;
    } else {
        col = textureSample(accumulate_tex, accumulate_tex_s, curved_uv).rgb + uniforms.brightness;
    }

    // Ghosting
    let roff = vec2(curved_uv.x - 30.0 / uniforms.resolution.x, curved_uv.y - 15.0 / uniforms.resolution.y);
    let red = textureSample(blur_tex, blur_tex_s, roff).rgb * vec3(0.5, 0.25, 0.25);

    let goff = vec2(curved_uv.x - 35.0 / uniforms.resolution.x, curved_uv.y - 20.0 / uniforms.resolution.y);
    let green = textureSample(blur_tex, blur_tex_s, goff).rgb * vec3(0.25, 0.5, 0.25);

    let boff = vec2(curved_uv.x - 40.0 / uniforms.resolution.x, curved_uv.y - 25.0 / uniforms.resolution.y);
    let blue = textureSample(blur_tex, blur_tex_s, boff).rgb * vec3(0.25, 0.25, 0.5);

    col += uniforms.ghost_factor * red * 0.5 * (1.0 - col);
    col += uniforms.ghost_factor * green * 0.5 * (1.0 - col);
    col += uniforms.ghost_factor * blue * 0.5 * (1.0 - col);

    // Scanlines
    let scans = sin(curved_uv.y * uniforms.resolution.y * 2.0) / 4.0 + 0.75;
    var col_orig = col;
    col *= vec3(scans);
    col = (col - col_orig) * vec3(uniforms.scanline_factor) + col_orig;

    // Mask
    col *= mask(uv * uniforms.resolution, 1.0 - uniforms.mask_strength);

    var distance = distance(curved_uv) - uniforms.corner_radius;
    distance = smoothstep(0.0, 100.0, distance);

    let crtColor = vec4(mix(col * vec3(uniforms.modulate_crt), vec3(0.0, 0.0, 0.0), vec3(distance)), 1.0);
    let clampedCrt = select(crtColor, vec4(vec3(0.0), 1.0), curved_uv.x < 0.0 || curved_uv.x > 1.0 || curved_uv.y < 0.0 || curved_uv.y > 1.0);

    return FragmentOutput(pow(clampedCrt, vec4(factor)), acc);
}