//! Fixed symmetric band matrix factorization. Geometry is factored once.

pub(super) struct BandCholesky {
    size: usize,
    band: usize,
    lower: Vec<f64>,
}

impl BandCholesky {
    pub fn new(diagonal: &[f64], neighbors: &[Vec<(usize, f64)>]) -> Result<Self, String> {
        let size = diagonal.len();
        let band = neighbors
            .iter()
            .enumerate()
            .flat_map(|(i, ns)| ns.iter().map(move |(j, _)| i.abs_diff(*j)))
            .max()
            .unwrap_or(0);
        let stride = band + 1;
        let mut lower = vec![0.0; size * stride];
        for i in 0..size {
            lower[i * stride] = diagonal[i];
            for &(j, value) in &neighbors[i] {
                if j < i {
                    lower[i * stride + i - j] = value;
                }
            }
        }
        for i in 0..size {
            for j in i.saturating_sub(band)..=i {
                let mut value = lower[i * stride + i - j];
                for k in i.saturating_sub(band).max(j.saturating_sub(band))..j {
                    value -= lower[i * stride + i - k] * lower[j * stride + j - k];
                }
                lower[i * stride + i - j] = if i == j {
                    if !value.is_finite() || value <= 0.0 {
                        return Err(format!("velocity matrix is not SPD at {i}: {value}"));
                    }
                    value.sqrt()
                } else {
                    value / lower[j * stride]
                };
            }
        }
        Ok(Self { size, band, lower })
    }

    pub fn solve(&self, rhs: &[f64]) -> Vec<f64> {
        assert_eq!(rhs.len(), self.size);
        let stride = self.band + 1;
        let mut x = rhs.to_vec();
        for i in 0..self.size {
            let mut value = x[i];
            for j in i.saturating_sub(self.band)..i {
                value -= self.lower[i * stride + i - j] * x[j];
            }
            x[i] = value / self.lower[i * stride];
        }
        for i in (0..self.size).rev() {
            let mut value = x[i];
            for j in i + 1..(i + self.band + 1).min(self.size) {
                value -= self.lower[j * stride + j - i] * x[j];
            }
            x[i] = value / self.lower[i * stride];
        }
        x
    }
}
