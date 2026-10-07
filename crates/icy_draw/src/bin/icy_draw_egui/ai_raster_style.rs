//! Perceptual cell candidates and bounded, edge-aware style refinement.

use super::{closest, sum};
use serde::Serialize;
use serde_json::{json, Value};

const ALTERNATIVES: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Preset {
    Faithful,
    Scene,
    Toon,
    PixelArt,
}

impl Preset {
    pub fn parse(name: &str) -> Result<Self, String> {
        match name {
            "faithful" => Ok(Self::Faithful),
            "scene" => Ok(Self::Scene),
            "toon" => Ok(Self::Toon),
            "pixel_art" => Ok(Self::PixelArt),
            _ => Err("preset must be one of faithful, scene, toon, pixel_art".into()),
        }
    }

    pub fn default_mode(self) -> &'static str {
        match self {
            Self::Scene => "blocks",
            Self::Faithful | Self::Toon | Self::PixelArt => "half_blocks",
        }
    }

    pub fn coherence(self) -> f64 {
        match self {
            Self::Scene => 0.0015,
            Self::Toon => 0.003,
            Self::Faithful | Self::PixelArt => 0.0,
        }
    }
}

pub(super) fn linear(rgb: [f64; 3]) -> [f64; 3] {
    rgb.map(|value| {
        let value = value / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    })
}

pub(super) fn oklab(rgb: [f64; 3]) -> [f64; 3] {
    let [r, g, b] = rgb;
    let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt();
    let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
    let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();
    [
        0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
        1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
        0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s,
    ]
}

fn srgb_from_linear(rgb: [f64; 3]) -> [u8; 3] {
    rgb.map(|channel| {
        let channel = channel.clamp(0.0, 1.0);
        let value = if channel <= 0.0031308 {
            12.92 * channel
        } else {
            1.055 * channel.powf(1.0 / 2.4) - 0.055
        };
        (255.0 * value).round() as u8
    })
}

fn srgb_from_oklab([l, a, b]: [f64; 3]) -> [u8; 3] {
    let m = (l - 0.1055613458 * a - 0.0638541728 * b).powi(3);
    let s = (l - 0.0894841775 * a - 1.2914855480 * b).powi(3);
    let l = (l + 0.3963377774 * a + 0.2158037573 * b).powi(3);
    srgb_from_linear([
        4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
        -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
        -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s,
    ])
}

pub(super) fn half_grid(source: &image::RgbaImage, columns: u32, rows: u32) -> image::RgbaImage {
    let pixels: Vec<_> = source
        .pixels()
        .map(|pixel| {
            let rgb = linear([pixel[0], pixel[1], pixel[2]].map(f64::from));
            let alpha = f64::from(pixel[3]) / 255.0;
            [rgb[0] * alpha, rgb[1] * alpha, rgb[2] * alpha, alpha]
        })
        .collect();
    let sx = f64::from(source.width()) / f64::from(columns);
    let sy = f64::from(source.height()) / f64::from(rows * 2);
    image::RgbaImage::from_fn(columns, rows * 2, |x, y| {
        let (left, top) = (f64::from(x) * sx, f64::from(y) * sy);
        let (right, bottom) = (left + sx, top + sy);
        let mut total = [0.0; 4];
        for py in top.floor() as u32..(bottom.ceil() as u32).min(source.height()) {
            for px in left.floor() as u32..(right.ceil() as u32).min(source.width()) {
                let area = (right.min(f64::from(px + 1)) - left.max(f64::from(px))) * (bottom.min(f64::from(py + 1)) - top.max(f64::from(py)));
                for (sum, value) in total.iter_mut().zip(pixels[(py * source.width() + px) as usize]) {
                    *sum += value * area;
                }
            }
        }
        if total[3] <= f64::EPSILON {
            return image::Rgba([0; 4]);
        }
        let [r, g, b] = srgb_from_linear([total[0] / total[3], total[1] / total[3], total[2] / total[3]]);
        image::Rgba([r, g, b, (total[3] / (sx * sy) * 255.0).round() as u8])
    })
}

pub(super) fn enhance_detail(source: &image::RgbaImage, strength: f64, radius: f32) -> image::RgbaImage {
    let stride = source.width() as usize + 1;
    let mut integral = vec![[0.0; 2]; stride * (source.height() as usize + 1)];
    for y in 0..source.height() {
        let mut row = [0.0; 2];
        for x in 0..source.width() {
            let pixel = source.get_pixel(x, y);
            let alpha = f64::from(pixel[3]) / 255.0;
            row[0] += oklab(linear([pixel[0], pixel[1], pixel[2]].map(f64::from)))[0] * alpha;
            row[1] += alpha;
            let above = y as usize * stride + x as usize + 1;
            integral[above + stride] = std::array::from_fn(|c| integral[above][c] + row[c]);
        }
    }
    let radius = radius as u32;
    image::RgbaImage::from_fn(source.width(), source.height(), |x, y| {
        let pixel = source.get_pixel(x, y);
        let left = x.saturating_sub(radius) as usize;
        let top = y.saturating_sub(radius) as usize;
        let right = (x + radius + 1).min(source.width()) as usize;
        let bottom = (y + radius + 1).min(source.height()) as usize;
        let total: [f64; 2] = std::array::from_fn(|c| {
            integral[bottom * stride + right][c] - integral[bottom * stride + left][c] - integral[top * stride + right][c] + integral[top * stride + left][c]
        });
        if pixel[3] == 0 || total[1] <= f64::EPSILON {
            return *pixel;
        }
        let mut lab = oklab(linear([pixel[0], pixel[1], pixel[2]].map(f64::from)));
        lab[0] = (lab[0] + strength * (lab[0] - total[0] / total[1])).clamp(0.0, 1.0);
        let [r, g, b] = srgb_from_oklab(lab);
        image::Rgba([r, g, b, pixel[3]])
    })
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter().zip(b).map(|(a, b)| (a - b).powi(2)).sum()
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub(super) struct Tuning {
    pub brightness: f64,
    pub contrast: f64,
    pub saturation: f64,
    pub shade_penalty: f64,
    pub coherence: f64,
    pub lightness_levels: u8,
    pub hue_families: bool,
    pub local_contrast: f64,
}

impl Tuning {
    pub fn parse(args: &Value, preset: Preset) -> Result<Self, String> {
        let number = |key: &str, default: f64, min: f64, max: f64| match args.get(key) {
            None => Ok(default),
            Some(value) => value
                .as_f64()
                .filter(|v| v.is_finite() && (min..=max).contains(v))
                .ok_or_else(|| format!("{key} must be a finite number in {min}..{max}")),
        };
        let levels = match args.get("lightness_levels") {
            None => {
                if preset == Preset::Toon {
                    5
                } else {
                    0
                }
            }
            Some(value) => value
                .as_u64()
                .filter(|v| *v == 0 || (2..=16).contains(v))
                .ok_or("lightness_levels must be 0 (off) or an integer in 2..16")? as u8,
        };
        Ok(Self {
            brightness: number("brightness", 0.0, -0.25, 0.25)?,
            contrast: number("contrast", 1.0, 0.5, 2.0)?,
            saturation: number("saturation", 1.0, 0.0, 2.0)?,
            shade_penalty: number("shade_penalty", 0.10, 0.0, 2.0)?,
            coherence: number("coherence", preset.coherence(), 0.0, 0.02)?,
            local_contrast: number("local_contrast", 0.0, 0.0, 2.0)?,
            lightness_levels: levels,
            hue_families: match args.get("hue_families") {
                None => false,
                Some(value) => value.as_bool().ok_or("hue_families must be a boolean")?,
            },
        })
    }

    pub fn schema() -> Value {
        json!({
            "brightness": {"type": "number", "minimum": -0.25, "maximum": 0.25, "description": "Oklab lightness offset; default 0"},
            "contrast": {"type": "number", "minimum": 0.5, "maximum": 2.0, "description": "Lightness contrast around 0.5; default 1"},
            "saturation": {"type": "number", "minimum": 0.0, "maximum": 2.0, "description": "Oklab chroma multiplier; default 1"},
            "shade_penalty": {"type": "number", "minimum": 0.0, "maximum": 2.0, "description": "Scene shade texture cost; lower allows more shading, default 0.10"},
            "coherence": {"type": "number", "minimum": 0.0, "maximum": 0.02, "description": "Neighbor-consistency weight; larger suppresses near-tie color speckles"},
            "local_contrast": {"type": "number", "minimum": 0.0, "maximum": 2.0, "description": "Enhance local lightness detail before palette reduction; 0 disables, default 0"},
            "lightness_levels": {"type": "integer", "enum": [0,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16], "description": "Lightness quantization intervals, 0 disables; Toon default 5"},
            "hue_families": {"type": "boolean", "description": "Guide colors by the source hue, allowing neutral shade blenders and separate foreground/background hues at boundaries. Default false."}
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Candidate {
    pub code: u8,
    pub foreground: usize,
    pub background: usize,
    pub error: f64,
}

impl Candidate {
    fn pair(self) -> (usize, usize) {
        (self.foreground.min(self.background), self.foreground.max(self.background))
    }
}

pub(super) struct Cell {
    pub mean: [f64; 3],
    pub candidates: Vec<Candidate>,
}

impl Cell {
    fn insert(&mut self, candidate: Candidate) {
        if self.candidates.iter().any(|old| old.pair() == candidate.pair() && old.error <= candidate.error) {
            return;
        }
        self.candidates.retain(|old| old.pair() != candidate.pair());
        let index = self.candidates.partition_point(|old| old.error <= candidate.error);
        if index < ALTERNATIVES {
            self.candidates.insert(index, candidate);
            self.candidates.truncate(ALTERNATIVES);
        }
    }
}

pub(super) struct Matcher {
    pub preset: Preset,
    pub tuning: Tuning,
    linear_palette: Vec<[f64; 3]>,
    palette: Vec<[f64; 3]>,
}

impl Matcher {
    pub fn new(preset: Preset, palette: &[[f64; 3]]) -> Self {
        let linear_palette: Vec<_> = palette.iter().copied().map(linear).collect();
        let palette = linear_palette.iter().copied().map(oklab).collect();
        Self {
            preset,
            tuning: Tuning {
                brightness: 0.0,
                contrast: 1.0,
                saturation: 1.0,
                shade_penalty: 0.10,
                coherence: preset.coherence(),
                lightness_levels: if preset == Preset::Toon { 5 } else { 0 },
                hue_families: false,
                local_contrast: 0.0,
            },
            linear_palette,
            palette,
        }
    }

    fn tune(&self, mut color: [f64; 3]) -> [f64; 3] {
        color[0] = ((color[0] - 0.5) * self.tuning.contrast + 0.5 + self.tuning.brightness).clamp(0.0, 1.0);
        color[1] *= self.tuning.saturation;
        color[2] *= self.tuning.saturation;
        if self.tuning.lightness_levels > 0 {
            let levels = f64::from(self.tuning.lightness_levels);
            color[0] = (color[0] * levels).round() / levels;
        }
        color
    }

    pub fn solve_halves(&self, rgb: [[f64; 3]; 2], foreground: &[usize], background: &[usize], column: usize, row: usize, dither: bool) -> Cell {
        let mut halves = rgb.map(|rgb| self.tune(oklab(linear(rgb))));
        let mean = std::array::from_fn(|c| (halves[0][c] + halves[1][c]) / 2.0);
        if dither {
            const BAYER: [[f64; 4]; 4] = [[0., 8., 2., 10.], [12., 4., 14., 6.], [3., 11., 1., 9.], [15., 7., 13., 5.]];
            for (half, color) in halves.iter_mut().enumerate() {
                color[0] = (color[0] + (BAYER[(row * 2 + half) % 4][column % 4] - 7.5) / 512.0).clamp(0.0, 1.0);
            }
        }
        let cost = |half: usize, index: usize| {
            let delta: [f64; 3] = std::array::from_fn(|c| halves[half][c] - self.palette[index][c]);
            delta[0] * delta[0] + 1.5 * (delta[1] * delta[1] + delta[2] * delta[2])
        };
        let nearest = |half, choices: &[usize]| {
            self.colors(choices, halves[half])
                .into_iter()
                .min_by(|&a, &b| cost(half, a).total_cmp(&cost(half, b)))
                .expect("validated nonempty palette")
        };
        let fg = [nearest(0, foreground), nearest(1, foreground)];
        let bg = [nearest(0, background), nearest(1, background)];
        let best = if fg[0] == fg[1] {
            let (code, foreground, background) = if background.contains(&fg[0]) {
                (32, foreground[0], fg[0])
            } else {
                (219, fg[0], background[0])
            };
            Candidate {
                code,
                foreground,
                background,
                error: (cost(0, fg[0]) + cost(1, fg[1])) / 2.0,
            }
        } else {
            let upper = Candidate {
                code: 223,
                foreground: fg[0],
                background: bg[1],
                error: (cost(0, fg[0]) + cost(1, bg[1])) / 2.0,
            };
            let lower = Candidate {
                code: 220,
                foreground: fg[1],
                background: bg[0],
                error: (cost(0, bg[0]) + cost(1, fg[1])) / 2.0,
            };
            if upper.error <= lower.error {
                upper
            } else {
                lower
            }
        };
        Cell { mean, candidates: vec![best] }
    }

    pub fn solve(&self, samples: &[[f64; 3]], masks: &[(u8, Vec<usize>)], foreground: &[usize], background: &[usize]) -> Cell {
        let unquantized: Vec<_> = samples.iter().copied().map(linear).map(oklab).collect();
        let mean = sum(&unquantized).map(|v| v / unquantized.len() as f64);
        let variance = unquantized.iter().map(|&color| distance(color, mean)).sum::<f64>() / unquantized.len() as f64;
        let samples: Vec<_> = unquantized.into_iter().map(|color| self.tune(color)).collect();
        let count = samples.len() as f64;
        let total = sum(&samples);
        let squares: f64 = samples.iter().flatten().map(|v| v * v).sum();
        let mut cell = Cell {
            mean: total.map(|v| v / count),
            candidates: Vec::with_capacity(ALTERNATIVES + 1),
        };
        let shade_foreground = self.shade_colors(foreground, cell.mean);
        let shade_background = self.shade_colors(background, cell.mean);
        let has_blocks = masks.iter().any(|(code, _)| *code == 219);
        for (code, on) in masks {
            let structural = matches!(code, 32 | 176..=178 | 219..=223);
            let mut lit = [0.0; 3];
            for &index in on {
                for (channel, value) in lit.iter_mut().zip(samples[index]) {
                    *channel += value;
                }
            }
            let unlit = std::array::from_fn(|c| total[c] - lit[c]);
            let lit_mean = if on.is_empty() { cell.mean } else { lit.map(|v| v / on.len() as f64) };
            let dark_count = samples.len() - on.len();
            let unlit_mean = if dark_count == 0 { cell.mean } else { unlit.map(|v| v / dark_count as f64) };
            let foreground_choices = self.colors(foreground, lit_mean);
            let background_choices = self.colors(background, unlit_mean);
            let (fe, fg) = closest(lit, on.len(), &foreground_choices, &self.palette);
            let (be, bg) = closest(unlit, dark_count, &background_choices, &self.palette);
            let error = ((squares + fe + be) / count).max(0.0);
            if self.preset == Preset::Scene && !structural && has_blocks && variance < 0.0025 && error > 1e-12 {
                continue;
            }
            cell.insert(Candidate {
                code: *code,
                foreground: fg,
                background: bg,
                error,
            });

            if self.preset == Preset::Scene && (176..=178).contains(code) && !on.is_empty() && on.len() < samples.len() {
                let coverage = on.len() as f64 / count;
                for &fg in &shade_foreground {
                    for &bg in &shade_background {
                        let blend = oklab(std::array::from_fn(|c| {
                            self.linear_palette[fg][c] * coverage + self.linear_palette[bg][c] * (1.0 - coverage)
                        }));
                        let error = (squares - 2.0 * total.into_iter().zip(blend).map(|(a, b)| a * b).sum::<f64>()) / count
                            + blend.into_iter().map(|v| v * v).sum::<f64>();
                        let texture = self.tuning.shade_penalty * coverage * (1.0 - coverage) * distance(self.palette[fg], self.palette[bg]);
                        cell.insert(Candidate {
                            code: *code,
                            foreground: fg,
                            background: bg,
                            error: error.max(0.0) + texture,
                        });
                    }
                }
            }
        }
        cell
    }

    fn colors(&self, choices: &[usize], source: [f64; 3]) -> Vec<usize> {
        if !self.tuning.hue_families {
            return choices.to_vec();
        }
        let hue = source[2].atan2(source[1]).to_degrees().rem_euclid(360.0);
        let threshold = 0.03 * self.tuning.saturation.max(1.0) * if (85.0..125.0).contains(&hue) { 2.5 } else { 1.0 };
        let chromatic = source[1].hypot(source[2]) >= threshold;
        let selected: Vec<_> = choices
            .iter()
            .copied()
            .filter(|&index| {
                let color = self.palette[index];
                let neutral = color[1].hypot(color[2]) < 0.025;
                let candidate_hue = color[2].atan2(color[1]).to_degrees().rem_euclid(360.0);
                let same_hue = if (35.0..85.0).contains(&hue) {
                    // DOS has no beige: brown, red and yellow form the warm ramp.
                    (10.0..115.0).contains(&candidate_hue)
                } else {
                    (candidate_hue - hue).to_radians().cos() >= 35_f64.to_radians().cos()
                };
                (!chromatic && neutral) || (chromatic && !neutral && same_hue) || (neutral && (color[0] < 0.12 || color[0] > 0.9))
            })
            .collect();
        // A custom palette may have no member of the requested family.
        if selected.is_empty() {
            choices.to_vec()
        } else {
            selected
        }
    }

    fn shade_colors(&self, choices: &[usize], source: [f64; 3]) -> Vec<usize> {
        let mut colors = self.colors(choices, source);
        // Mid greys desaturate a chromatic shade without changing its hue.
        // Excluding them forces skin tones into saturated red/yellow pairs.
        for &index in choices {
            if self.palette[index][1].hypot(self.palette[index][2]) < 0.025 && !colors.contains(&index) {
                colors.push(index);
            }
        }
        colors
    }
}

/// Two synchronous passes avoid scan-direction bias and keep work bounded.
pub(super) fn refine(cells: &[Cell], width: usize, weight: f64) -> Vec<Candidate> {
    let mut selected: Vec<_> = cells.iter().map(|cell| cell.candidates[0]).collect();
    if weight == 0.0 {
        return selected;
    }
    for _ in 0..2 {
        let previous = selected.clone();
        for (index, cell) in cells.iter().enumerate() {
            // Do not disturb exact reproductions, even if their neighbors differ.
            if cell.candidates[0].error <= 1e-12 {
                continue;
            }
            let neighbors = [
                (index % width > 0).then(|| index - 1),
                (index % width + 1 < width).then_some(index + 1),
                (index >= width).then(|| index - width),
                (index + width < cells.len()).then_some(index + width),
            ];
            let score = |candidate: Candidate| {
                candidate.error
                    + neighbors
                        .iter()
                        .flatten()
                        .map(|&neighbor| {
                            if candidate.pair() == previous[neighbor].pair() {
                                0.0
                            } else {
                                weight * (-distance(cell.mean, cells[neighbor].mean) / 0.0025).exp()
                            }
                        })
                        .sum::<f64>()
            };
            let mut best = cell.candidates[0];
            let mut best_score = score(best);
            for &candidate in &cell.candidates[1..] {
                let value = score(candidate);
                if value < best_score {
                    best = candidate;
                    best_score = value;
                }
            }
            selected[index] = best;
        }
    }
    selected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perceptual_transform_matches_reference_colors() {
        for (rgb, expected) in [
            ([0.0; 3], [0.0; 3]),
            ([255.0; 3], [1.0, 0.0, 0.0]),
            ([255.0, 0.0, 0.0], [0.62795536, 0.22486306, 0.12584630]),
        ] {
            let actual = oklab(linear(rgb));
            assert!(distance(actual, expected) < 1e-12, "{actual:?} != {expected:?}");
        }
    }

    #[test]
    fn hue_families_preserve_warm_colors_neutrals_and_legal_palette_choices() {
        let palette = [
            [0.0; 3],
            [85.0; 3],
            [170.0; 3],
            [255.0; 3],
            [170.0, 85.0, 0.0],
            [170.0, 0.0, 0.0],
            [0.0, 170.0, 0.0],
        ];
        let mut matcher = Matcher::new(Preset::Scene, &palette);
        let warm = oklab(linear([145.0, 120.0, 100.0]));
        let choices = [0, 1, 2, 3, 4, 5, 6];
        assert_eq!(matcher.colors(&choices, warm), choices, "disabled tuning preserves existing behavior");
        matcher.tuning.hue_families = true;
        assert_eq!(matcher.colors(&choices, warm), [0, 3, 4, 5]);
        assert_eq!(matcher.colors(&choices, oklab(linear([120.0; 3]))), [0, 1, 2, 3]);
        assert_eq!(matcher.colors(&[0, 4, 6], warm), [0, 4], "never add illegal background colors");
        assert_eq!(matcher.colors(&[6], warm), [6], "custom palettes without the family remain representable");
        assert!(Tuning::parse(&serde_json::json!({"hue_families": "yes"}), Preset::Scene).is_err());
    }

    #[test]
    fn half_grid_averages_linear_light_and_alpha_with_fractional_coverage() {
        let stripe = image::RgbaImage::from_fn(3, 2, |x, _| if x == 1 { image::Rgba([255; 4]) } else { image::Rgba([0, 0, 0, 255]) });
        let grid = half_grid(&stripe, 2, 1);
        let expected = srgb_from_linear([1.0 / 3.0; 3])[0];
        assert!(grid.pixels().all(|pixel| pixel.0 == [expected, expected, expected, 255]));
        let transparent = image::RgbaImage::from_fn(4, 2, |x, _| if x < 2 { image::Rgba([0, 255, 0, 255]) } else { image::Rgba([255, 0, 0, 0]) });
        assert!(half_grid(&transparent, 1, 1).pixels().all(|pixel| pixel.0 == [0, 255, 0, 128]));
    }

    #[test]
    fn half_pixel_solver_keeps_bright_colors_legal_without_ice() {
        let palette: Vec<_> = (0..16)
            .map(|i| {
                let (r, g, b) = icy_engine::Palette::default().rgb(i);
                [r, g, b].map(f64::from)
            })
            .collect();
        let matcher = Matcher::new(Preset::Scene, &palette);
        let colors: Vec<_> = (0..16).collect();
        for ice in [false, true] {
            let backgrounds = &colors[..if ice { 16 } else { 8 }];
            for (top, bottom) in [(12, 9), (9, 4), (4, 12), (12, 12)] {
                let cell = matcher.solve_halves([palette[top], palette[bottom]], &colors, backgrounds, 0, 0, false);
                let best = cell.candidates[0];
                assert!([32, 219, 220, 223].contains(&best.code));
                assert!(best.background < backgrounds.len());
                if ice || top < 8 || bottom < 8 || top == bottom {
                    assert!(best.error < 1e-12, "{best:?}");
                }
            }
        }
    }

    #[test]
    fn detail_enhancement_preserves_color_alpha_and_flat_regions() {
        for rgb in [[0, 0, 0], [255, 255, 255], [145, 120, 100], [0, 170, 170], [255, 0, 0]] {
            assert_eq!(srgb_from_oklab(oklab(linear(rgb.map(f64::from)))), rgb);
            let flat = image::RgbaImage::from_pixel(20, 20, image::Rgba([rgb[0], rgb[1], rgb[2], 255]));
            assert_eq!(enhance_detail(&flat, 1.0, 4.0), flat);
        }
        let source = image::RgbaImage::from_fn(32, 16, |x, _| {
            let value = if x < 16 { 96 } else { 128 };
            image::Rgba([value, value, value, 255])
        });
        let enhanced = enhance_detail(&source, 1.0, 4.0);
        assert!(i32::from(enhanced.get_pixel(16, 8)[0]) - i32::from(enhanced.get_pixel(15, 8)[0]) > 48);
        let transparent = image::RgbaImage::from_fn(32, 16, |x, _| {
            if x < 16 {
                image::Rgba([255, 0, 255, 0])
            } else {
                image::Rgba([100, 120, 140, 128])
            }
        });
        assert_eq!(enhance_detail(&transparent, 1.0, 4.0), transparent, "hidden RGB must not create colored halos");
        assert!(Tuning::parse(&json!({"local_contrast": 2.1}), Preset::Scene).is_err());
    }

    #[test]
    fn hue_guidance_blends_neutrals_without_admitting_bright_unrelated_hues() {
        let palette = [
            [0.0; 3],
            [85.0; 3],
            [170.0; 3],
            [255.0; 3],
            [170.0, 85.0, 0.0],
            [170.0, 0.0, 0.0],
            [0.0, 170.0, 0.0],
            [85.0, 255.0, 255.0],
            [255.0, 255.0, 85.0],
        ];
        let mut matcher = Matcher::new(Preset::Scene, &palette);
        matcher.tuning.hue_families = true;
        let warm = oklab(linear([145.0, 120.0, 100.0]));
        let choices: Vec<_> = (0..palette.len()).collect();
        let colors = matcher.shade_colors(&choices, warm);
        assert!(colors.contains(&1) && colors.contains(&2) && colors.contains(&4));
        assert!(!colors.contains(&7), "bright cyan is not white");
        assert!(
            !matcher.colors(&choices, oklab(linear([20.0, 30.0, 160.0]))).contains(&8),
            "bright yellow is not white"
        );
        let masks = vec![
            (32, vec![]),
            (219, (0..128).collect()),
            (176, (0..32).collect()),
            (177, (0..64).collect()),
            (178, (0..96).collect()),
        ];
        let best = matcher.solve(&[[145.0, 120.0, 100.0]; 128], &masks, &choices, &choices).candidates[0];
        assert!((176..=178).contains(&best.code));
        assert!(matches!((best.foreground, best.background), (1 | 2, 4) | (4, 1 | 2)), "{best:?}");
        let mut boundary = vec![palette[5]; 64];
        boundary.extend_from_slice(&[palette[6]; 64]);
        let halves = vec![(223, (0..64).collect()), (220, (64..128).collect())];
        let best = matcher.solve(&boundary, &halves, &choices, &choices).candidates[0];
        assert!(
            best.error < 1e-12 && best.pair() == (5, 6),
            "half blocks must preserve both source hues: {best:?}"
        );
    }

    #[test]
    fn shape_gating_preserves_exact_low_contrast_glyphs() {
        let matcher = Matcher::new(Preset::Scene, &[[100.0; 3], [110.0; 3]]);
        let mut pixels = [[100.0; 3]; 128];
        pixels[0] = [110.0; 3];
        let masks = vec![(32, vec![]), (219, (0..128).collect()), (46, vec![0])];
        let best = matcher.solve(&pixels, &masks, &[0, 1], &[0, 1]).candidates[0];
        assert_eq!(best.code, 46);
        assert!(best.error < 1e-12);
    }

    #[test]
    fn scene_shades_use_measured_coverage_and_linear_light() {
        let matcher = Matcher::new(Preset::Scene, &[[0.0; 3], [255.0; 3]]);
        let masks = vec![(32, vec![]), (219, (0..128).collect()), (177, (0..64).collect())];
        let cell = matcher.solve(&[[188.0; 3]; 128], &masks, &[0, 1], &[0, 1]);
        let best = cell.candidates[0];
        assert_eq!(best.code, 177, "a smooth midtone should use a blended shade, not a solid");
        assert_ne!(best.foreground, best.background);
        let pixel = Matcher::new(Preset::PixelArt, &[[0.0; 3], [255.0; 3]]).solve(&[[188.0; 3]; 128], &masks, &[0, 1], &[0, 1]);
        assert_ne!(pixel.candidates[0].code, 177, "literal pixel matching must remain distinct");
        let empty_shade = matcher.solve(&[[188.0; 3]; 128], &[(177, vec![])], &[0, 1], &[0, 1]);
        assert_eq!(empty_shade.candidates[0].background, 1, "custom empty shade glyphs cannot blend");
    }

    #[test]
    fn coherence_removes_near_tie_speckles_but_respects_edges_and_exact_matches() {
        let normal = Candidate {
            code: 219,
            foreground: 1,
            background: 0,
            error: 0.001,
        };
        let speckle = Candidate { foreground: 2, ..normal };
        let make = |center_mean, error| {
            vec![
                Cell {
                    mean: [0.5, 0.0, 0.0],
                    candidates: vec![normal],
                },
                Cell {
                    mean: center_mean,
                    candidates: vec![Candidate { error, ..speckle }, Candidate { error: 0.0015, ..normal }],
                },
                Cell {
                    mean: [0.5, 0.0, 0.0],
                    candidates: vec![normal],
                },
            ]
        };
        let smooth = make([0.5, 0.0, 0.0], 0.001);
        assert_eq!(refine(&smooth, 3, 0.0)[1].foreground, 2);
        assert_eq!(refine(&smooth, 3, Preset::Scene.coherence())[1].foreground, 1);
        assert_eq!(refine(&smooth, 3, Preset::Scene.coherence()), refine(&smooth, 3, Preset::Scene.coherence()));
        assert_eq!(refine(&make([0.9, 0.0, 0.0], 0.001), 3, Preset::Scene.coherence())[1].foreground, 2);
        assert_eq!(refine(&make([0.5, 0.0, 0.0], 0.0), 3, Preset::Scene.coherence())[1].foreground, 2);
        assert_eq!(refine(&smooth, 1, Preset::Scene.coherence())[1].foreground, 1);
    }

    #[test]
    fn alternatives_are_bounded_and_retain_distinct_color_pairs() {
        let mut cell = Cell {
            mean: [0.0; 3],
            candidates: Vec::new(),
        };
        for foreground in (0..16).rev() {
            for code in [176, 177, 178] {
                cell.insert(Candidate {
                    code,
                    foreground,
                    background: 0,
                    error: foreground as f64,
                });
            }
        }
        assert_eq!(cell.candidates.len(), ALTERNATIVES);
        assert_eq!(cell.candidates.iter().map(|c| c.foreground).collect::<Vec<_>>(), vec![0, 1, 2, 3]);
    }
}
