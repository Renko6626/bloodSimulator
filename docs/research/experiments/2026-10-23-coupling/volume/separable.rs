//! Exact (up to roundoff) inverse of the constant-coefficient velocity Laplacian
//! for the prototype's periodic-x, no-slip-y MAC channel. This is not a solver
//! for a branched domain or for moving cut-cell geometry.

use std::f64::consts::PI;

pub struct SeparableInverse {
    n: usize,
    bit_reverse: Vec<usize>,
    twiddle_re: Vec<f64>,
    twiddle_im: Vec<f64>,
    u_den: Vec<f64>,
    u_upper: Vec<f64>,
    v_den: Vec<f64>,
    v_upper: Vec<f64>,
}

impl SeparableInverse {
    pub fn new(n: usize) -> Result<Self, String> {
        if n < 2 || !n.is_power_of_two() {
            return Err("separable inverse requires a power-of-two n >= 2".into());
        }
        let nn = n.checked_mul(n).ok_or("grid size overflow")?;
        let nv = n.checked_mul(n - 1).ok_or("grid size overflow")?;
        nn.checked_add(nv).ok_or("grid size overflow")?;
        let bits = n.trailing_zeros();
        let bit_reverse = (0..n)
            .map(|i| i.reverse_bits() >> (usize::BITS - bits))
            .collect();
        let (twiddle_re, twiddle_im): (Vec<_>, Vec<_>) = (0..n / 2)
            .map(|k| {
                let theta = -2.0 * PI * k as f64 / n as f64;
                (theta.cos(), theta.sin())
            })
            .unzip();
        let mut u_den = vec![0.0; nn];
        let mut u_upper = vec![0.0; nn];
        let mut v_den = vec![0.0; nv];
        let mut v_upper = vec![0.0; nv];
        let ih2 = (n as f64) * (n as f64);
        let off = -ih2;
        for k in 0..n {
            let lambda = 4.0 * (PI * k as f64 / n as f64).sin().powi(2) * ih2;
            for j in 0..n {
                let idx = k * n + j;
                let diagonal = (if j == 0 || j + 1 == n { 3.0 } else { 2.0 }) * ih2 + lambda;
                let den = diagonal - if j == 0 { 0.0 } else { off * u_upper[idx - 1] };
                u_den[idx] = den;
                if j + 1 < n {
                    u_upper[idx] = off / den;
                }
            }
            for j in 0..n - 1 {
                let idx = k * (n - 1) + j;
                let den = 2.0 * ih2 + lambda - if j == 0 { 0.0 } else { off * v_upper[idx - 1] };
                v_den[idx] = den;
                if j + 2 < n {
                    v_upper[idx] = off / den;
                }
            }
        }
        Ok(Self {
            n,
            bit_reverse,
            twiddle_re,
            twiddle_im,
            u_den,
            u_upper,
            v_den,
            v_upper,
        })
    }

    /// `b` and `x`: n*n u entries, then n*(n-1) v entries; x is fastest.
    /// Every call overwrites all of `x`, regardless of its initial contents.
    pub fn solve(&self, b: &[f64], x: &mut [f64]) {
        let n = self.n;
        let u_len = n * n;
        let len = u_len + n * (n - 1);
        assert_eq!(b.len(), len, "separable inverse RHS length mismatch");
        assert_eq!(x.len(), len, "separable inverse output length mismatch");
        let mut re = b.to_vec();
        let mut im = vec![0.0; len];
        for offset in (0..len).step_by(n) {
            self.fft_row(
                &mut re[offset..offset + n],
                &mut im[offset..offset + n],
                false,
            );
        }
        self.solve_modes(
            &mut re[..u_len],
            &mut im[..u_len],
            n,
            &self.u_den,
            &self.u_upper,
        );
        self.solve_modes(
            &mut re[u_len..],
            &mut im[u_len..],
            n - 1,
            &self.v_den,
            &self.v_upper,
        );
        for offset in (0..len).step_by(n) {
            self.fft_row(
                &mut re[offset..offset + n],
                &mut im[offset..offset + n],
                true,
            );
        }
        x.copy_from_slice(&re);
    }

    fn solve_modes(&self, re: &mut [f64], im: &mut [f64], rows: usize, den: &[f64], upper: &[f64]) {
        let n = self.n;
        let off = -(n as f64) * (n as f64);
        for k in 0..n {
            let mut previous_re = 0.0;
            let mut previous_im = 0.0;
            for j in 0..rows {
                let idx = j * n + k;
                let factor = k * rows + j;
                let value_re = (re[idx] - off * previous_re) / den[factor];
                let value_im = (im[idx] - off * previous_im) / den[factor];
                re[idx] = value_re;
                im[idx] = value_im;
                previous_re = value_re;
                previous_im = value_im;
            }
            for j in (0..rows - 1).rev() {
                let idx = j * n + k;
                let next = idx + n;
                let factor = k * rows + j;
                re[idx] -= upper[factor] * re[next];
                im[idx] -= upper[factor] * im[next];
            }
        }
    }

    fn fft_row(&self, re: &mut [f64], im: &mut [f64], inverse: bool) {
        let n = self.n;
        for i in 0..n {
            let j = self.bit_reverse[i];
            if j > i {
                re.swap(i, j);
                im.swap(i, j);
            }
        }
        let mut width = 2;
        while width <= n {
            let half = width / 2;
            let twiddle_step = n / width;
            for base in (0..n).step_by(width) {
                for j in 0..half {
                    let left = base + j;
                    let right = left + half;
                    let w_re = self.twiddle_re[j * twiddle_step];
                    let w_im = if inverse {
                        -self.twiddle_im[j * twiddle_step]
                    } else {
                        self.twiddle_im[j * twiddle_step]
                    };
                    let t_re = w_re * re[right] - w_im * im[right];
                    let t_im = w_re * im[right] + w_im * re[right];
                    let a_re = re[left];
                    let a_im = im[left];
                    re[left] = a_re + t_re;
                    im[left] = a_im + t_im;
                    re[right] = a_re - t_re;
                    im[right] = a_im - t_im;
                }
            }
            width *= 2;
        }
        if inverse {
            let scale = 1.0 / n as f64;
            for i in 0..n {
                re[i] *= scale;
                im[i] *= scale;
            }
        }
    }
}
