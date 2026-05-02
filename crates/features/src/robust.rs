use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RobustStats {
    pub n: usize,
    pub median: f64,
    pub mad: f64,
    pub scale: f64,
    pub z: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RingWindow {
    capacity: usize,
    values: VecDeque<f64>,
}

impl RingWindow {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            values: VecDeque::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, value: f64) {
        if self.capacity == 0 || !value.is_finite() {
            return;
        }
        if self.values.len() == self.capacity {
            self.values.pop_front();
        }
        self.values.push_back(value);
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn values_recent(&self) -> Vec<f64> {
        self.values.iter().copied().collect()
    }

    pub fn stats(&self, current: f64, min_samples: usize, z_clip: f64) -> Option<RobustStats> {
        if !current.is_finite() || min_samples == 0 || self.values.len() < min_samples {
            return None;
        }

        let values = self.values_recent();
        let median_value = median(values.clone())?;
        let deviations = values
            .iter()
            .map(|value| (value - median_value).abs())
            .collect::<Vec<_>>();
        let mad = median(deviations)?;
        let mut scale = 1.4826 * mad;
        if scale == 0.0 {
            scale = sample_stddev(&values)?;
        }
        if scale == 0.0 && (current - median_value).abs() == 0.0 {
            return Some(RobustStats {
                n: values.len(),
                median: median_value,
                mad,
                scale: 0.0,
                z: 0.0,
            });
        }
        if scale == 0.0 || !scale.is_finite() {
            return None;
        }

        let clip = if z_clip.is_finite() && z_clip > 0.0 {
            z_clip
        } else {
            f64::INFINITY
        };
        let z = ((current - median_value) / scale).clamp(-clip, clip);

        Some(RobustStats {
            n: values.len(),
            median: median_value,
            mad,
            scale,
            z,
        })
    }

    pub fn percentile_rank(&self, current: f64) -> Option<f64> {
        if !current.is_finite() || self.values.is_empty() {
            return None;
        }

        let count = self
            .values
            .iter()
            .filter(|value| **value <= current)
            .count();
        Some(count as f64 / self.values.len() as f64)
    }
}

fn median(mut values: Vec<f64>) -> Option<f64> {
    if values.is_empty() || values.iter().any(|value| !value.is_finite()) {
        return None;
    }

    values.sort_by(|a, b| a.total_cmp(b));
    let mid = values.len() / 2;
    if values.len() % 2 == 0 {
        Some((values[mid - 1] + values[mid]) / 2.0)
    } else {
        Some(values[mid])
    }
}

fn sample_stddev(values: &[f64]) -> Option<f64> {
    if values.len() < 2 {
        return None;
    }

    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let variance = values
        .iter()
        .map(|value| {
            let diff = value - mean;
            diff * diff
        })
        .sum::<f64>()
        / (values.len() - 1) as f64;
    Some(variance.sqrt())
}
