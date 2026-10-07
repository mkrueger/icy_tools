//! Fixed, source-relative measurements for comparing conversion trials.

use serde::Serialize;

use super::style::{linear, oklab};

#[derive(Clone, Copy)]
pub(super) struct Sample {
    mean: [f64; 3],
    quarters: [[f64; 3]; 4],
}

impl Sample {
    pub fn new(pixels: &[[f64; 3]], width: usize, height: usize) -> Self {
        let mut sum = [0.0; 3];
        let mut quarters = [[0.0; 3]; 4];
        let mut counts = [0; 4];
        for (index, rgb) in pixels.iter().copied().enumerate() {
            let quarter = usize::from(index % width >= width.div_ceil(2)) + 2 * usize::from(index / width >= height.div_ceil(2));
            counts[quarter] += 1;
            for (c, value) in linear(rgb).into_iter().enumerate() {
                sum[c] += value;
                quarters[quarter][c] += value;
            }
        }
        let mean = oklab(sum.map(|v| v / pixels.len() as f64));
        for (values, count) in quarters.iter_mut().zip(counts) {
            *values = if count == 0 { mean } else { oklab(values.map(|v| v / count as f64)) };
        }
        Self { mean, quarters }
    }
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter().zip(b).map(|(a, b)| (a - b).powi(2)).sum()
}

#[derive(Clone, Copy, Debug, Serialize)]
pub(super) struct Metrics {
    pub objective: f64,
    pub color_error: f64,
    pub detail_error: f64,
    pub edge_error: f64,
    pub excess_texture: f64,
}

impl Metrics {
    pub fn measure(source: &[Sample], rendered: &[Sample], width: usize) -> Self {
        let mut color = 0.0;
        let mut detail = 0.0;
        let mut edges = 0.0;
        let mut texture = 0.0;
        let mut pairs = 0;
        for (index, (reference, actual)) in source.iter().zip(rendered).enumerate() {
            color += distance(reference.mean, actual.mean);
            detail += reference.quarters.iter().zip(actual.quarters).map(|(&a, b)| distance(a, b)).sum::<f64>() / 4.0;
            for neighbor in [
                (index % width + 1 < width).then_some(index + 1),
                (index + width < source.len()).then_some(index + width),
            ]
            .into_iter()
            .flatten()
            {
                let source_delta = std::array::from_fn(|c| reference.mean[c] - source[neighbor].mean[c]);
                let rendered_delta = std::array::from_fn(|c| actual.mean[c] - rendered[neighbor].mean[c]);
                edges += distance(source_delta, rendered_delta);
                let expected = distance(reference.mean, source[neighbor].mean);
                let observed = distance(actual.mean, rendered[neighbor].mean);
                texture += (observed - expected).max(0.0) * (-expected / 0.0025).exp();
                pairs += 1;
            }
        }
        let color_error = color / source.len() as f64;
        let detail_error = detail / source.len() as f64;
        let edge_error = edges / pairs.max(1) as f64;
        let excess_texture = texture / pairs.max(1) as f64;
        Self {
            objective: 0.5 * color_error + 0.5 * detail_error + 0.25 * edge_error + excess_texture,
            color_error,
            detail_error,
            edge_error,
            excess_texture,
        }
    }

    pub fn improves(self, before: Self) -> bool {
        self.objective + 1e-9 < before.objective
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measurements_reward_reproduction_and_penalize_lost_detail_and_false_edges() {
        let solid = |v| Sample::new(&[[v; 3]; 128], 8, 16);
        let split: Vec<_> = (0..128).map(|i| if i < 64 { [0.0; 3] } else { [255.0; 3] }).collect();
        let source = vec![solid(120.0), Sample::new(&split, 8, 16), solid(120.0)];
        let exact = Metrics::measure(&source, &source, 3);
        assert_eq!(exact.objective, 0.0);
        let flat = Metrics::measure(&source, &[solid(120.0), solid(188.0), solid(120.0)], 3);
        assert!(flat.detail_error > 0.05, "matching the mean must not excuse a lost contour");
        assert!(exact.improves(flat));
        assert!(!flat.improves(exact));
        assert!(!exact.improves(exact));
        let smooth = [solid(120.0); 3];
        let noisy = Metrics::measure(&smooth, &[solid(120.0), solid(255.0), solid(120.0)], 3);
        assert!(noisy.excess_texture > 0.01);
        assert!(noisy.edge_error > 0.01);
    }
}
