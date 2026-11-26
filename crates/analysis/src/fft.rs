use rustfft::num_complex::Complex;
use rustfft::FftPlanner;

#[derive(Clone, Debug)]
pub struct Spectrum {
    pub frequency: Vec<f64>,
    pub amplitude: Vec<f64>,
    pub phase: Vec<f64>,
}

pub fn fft_spectrum(x: &[f64], y: &[f64]) -> Result<Spectrum, String> {
    let (dt, samples) = uniformish(x, y)?;
    let n = samples.len();
    let mut buffer: Vec<Complex<f64>> = samples.iter().map(|&v| Complex::new(v, 0.0)).collect();
    let mut planner = FftPlanner::new();
    planner.plan_fft_forward(n).process(&mut buffer);
    let mut frequency = Vec::new();
    let mut amplitude = Vec::new();
    let mut phase = Vec::new();
    let half = n / 2;
    for (k, sample) in buffer.iter().take(half + 1).enumerate() {
        let scale = if k == 0 || (n % 2 == 0 && k == half) {
            1.0 / n as f64
        } else {
            2.0 / n as f64
        };
        frequency.push(k as f64 / (n as f64 * dt));
        amplitude.push(sample.norm() * scale);
        phase.push(sample.arg());
    }
    Ok(Spectrum {
        frequency,
        amplitude,
        phase,
    })
}

pub fn fft_filter(x: &[f64], y: &[f64], cutoff: f64, highpass: bool) -> Result<Vec<f64>, String> {
    if cutoff < 0.0 {
        return Err(sciplot_core::t("截止频率不能为负").into());
    }
    let (dt, samples) = uniformish(x, y)?;
    let n = samples.len();
    let mut buffer: Vec<Complex<f64>> = samples.iter().map(|&v| Complex::new(v, 0.0)).collect();
    let mut planner = FftPlanner::new();
    planner.plan_fft_forward(n).process(&mut buffer);
    for (k, sample) in buffer.iter_mut().enumerate() {
        let freq = if k <= n / 2 {
            k as f64 / (n as f64 * dt)
        } else {
            (n - k) as f64 / (n as f64 * dt)
        };
        let remove = if highpass { freq < cutoff } else { freq > cutoff };
        if remove {
            *sample = Complex::new(0.0, 0.0);
        }
    }
    planner.plan_fft_inverse(n).process(&mut buffer);
    let scale = 1.0 / n as f64;
    Ok(buffer.iter().map(|v| v.re * scale).collect())
}

fn uniformish(x: &[f64], y: &[f64]) -> Result<(f64, Vec<f64>), String> {
    let pairs: Vec<(f64, f64)> = x
        .iter()
        .zip(y.iter())
        .filter(|(a, b)| a.is_finite() && b.is_finite())
        .map(|(a, b)| (*a, *b))
        .collect();
    if pairs.len() < 4 {
        return Err(sciplot_core::t("FFT 至少需要 4 个有限点").into());
    }
    let mut dx = Vec::new();
    for w in pairs.windows(2) {
        let step = w[1].0 - w[0].0;
        if step > 0.0 {
            dx.push(step);
        }
    }
    if dx.is_empty() {
        return Err(sciplot_core::t("x 没有递增区间，无法确定采样间隔").into());
    }
    dx.sort_by(|a, b| a.total_cmp(b));
    let dt = dx[dx.len() / 2];
    if dt <= 0.0 || !dt.is_finite() {
        return Err(sciplot_core::t("采样间隔无效").into());
    }
    let lo = dx[0];
    let hi = *dx.last().unwrap_or(&dt);
    if lo < dt * 0.99 || hi > dt * 1.01 {
        return Err(sciplot_core::t("X 间隔不均匀，请先插值").into());
    }
    Ok((dt, pairs.into_iter().map(|(_, y)| y).collect()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sine_peak_and_lowpass() {
        let n = 64;
        let freq = 4.0;
        let x: Vec<f64> = (0..n).map(|i| i as f64).collect();
        let y: Vec<f64> = (0..n)
            .map(|i| (2.0 * std::f64::consts::PI * freq * i as f64 / n as f64).sin())
            .collect();
        let spectrum = fft_spectrum(&x, &y).unwrap();
        let (index, amplitude) = spectrum
            .amplitude
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .unwrap();
        assert_eq!(index, 4);
        assert!((amplitude - 1.0).abs() < 0.05, "amplitude {amplitude}");

        let filtered = fft_filter(&x, &y, 0.0, false).unwrap();
        let energy: f64 = filtered.iter().map(|v| v * v).sum();
        assert!(energy < 1e-6, "lowpass of everything should be ~0, energy {energy}");
    }

    #[test]
    fn uneven_spacing_is_rejected() {
        let mut x: Vec<f64> = (0..32).map(|i| i as f64).collect();
        x.extend((0..32).map(|i| 31.0 + (i + 1) as f64 * 5.0));
        let y: Vec<f64> = x.iter().map(|v| (v * 0.2).sin()).collect();
        let err = fft_spectrum(&x, &y).unwrap_err();
        assert!(err.contains("不均匀"), "{err}");
    }
}
