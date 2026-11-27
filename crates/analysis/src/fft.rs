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
    let band = if highpass {
        FftBand::HighPass { cutoff }
    } else {
        FftBand::LowPass { cutoff }
    };
    fft_filter_band(x, y, band)
}

/// 低通、高通、带通、带阻。频率比较用闭区间：带通保留 `low <= f <= high`，带阻去掉这一段。
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FftBand {
    LowPass { cutoff: f64 },
    HighPass { cutoff: f64 },
    BandPass { low: f64, high: f64 },
    BandStop { low: f64, high: f64 },
}

impl FftBand {
    pub fn label(self) -> &'static str {
        match self {
            FftBand::LowPass { .. } => sciplot_core::tr!("低通", "Low pass"),
            FftBand::HighPass { .. } => sciplot_core::tr!("高通", "High pass"),
            FftBand::BandPass { .. } => sciplot_core::tr!("带通", "Band pass"),
            FftBand::BandStop { .. } => sciplot_core::tr!("带阻", "Band stop"),
        }
    }
}

pub fn fft_filter_band(x: &[f64], y: &[f64], band: FftBand) -> Result<Vec<f64>, String> {
    let (low, high, pass) = band_edges(band)?;
    let (dt, samples) = uniformish(x, y)?;
    let n = samples.len();
    let mut buffer: Vec<Complex<f64>> = samples.iter().map(|&v| Complex::new(v, 0.0)).collect();
    let mut planner = FftPlanner::new();
    planner.plan_fft_forward(n).process(&mut buffer);
    for (k, sample) in buffer.iter_mut().enumerate() {
        let freq = bin_frequency(k, n, dt);
        let inside = freq >= low && freq <= high;
        let remove = if pass { !inside } else { inside };
        if remove {
            *sample = Complex::new(0.0, 0.0);
        }
    }
    planner.plan_fft_inverse(n).process(&mut buffer);
    let scale = 1.0 / n as f64;
    Ok(buffer.iter().map(|v| v.re * scale).collect())
}

fn band_edges(band: FftBand) -> Result<(f64, f64, bool), String> {
    let (low, high, pass) = match band {
        FftBand::LowPass { cutoff } => (0.0, cutoff, true),
        FftBand::HighPass { cutoff } => (cutoff, f64::INFINITY, true),
        FftBand::BandPass { low, high } => (low, high, true),
        FftBand::BandStop { low, high } => (low, high, false),
    };
    if !low.is_finite() && low.is_sign_negative() {
        return Err(sciplot_core::t("截止频率不能为负").into());
    }
    if low < 0.0 || (high < 0.0 && high.is_finite()) {
        return Err(sciplot_core::t("截止频率不能为负").into());
    }
    if high < low {
        return Err(sciplot_core::tr!(
            "带通或带阻的下限必须小于上限",
            "A band filter needs its lower frequency below the upper one"
        )
        .into());
    }
    Ok((low, high, pass))
}

fn bin_frequency(k: usize, n: usize, dt: f64) -> f64 {
    if k <= n / 2 {
        k as f64 / (n as f64 * dt)
    } else {
        (n - k) as f64 / (n as f64 * dt)
    }
}

/// 正向或逆向复数 FFT 的选项。`normalize` 把正向变换除以点数；逆向会先乘回来，所以同一组选项来回变换能还原时域。
/// `shift` 把零频移到中间（偶数长度时再移一次就还原）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FftShape {
    pub normalize: bool,
    pub shift: bool,
}

#[derive(Clone, Debug)]
pub struct FftSeries {
    pub axis: Vec<f64>,
    pub real: Vec<f64>,
    pub imag: Vec<f64>,
    pub amplitude: Vec<f64>,
    pub phase: Vec<f64>,
}

pub fn fft_forward(y: &[f64], dt: f64, shape: FftShape) -> Result<FftSeries, String> {
    fft_forward_complex(y, &[], dt, shape)
}

/// 正向 FFT。`imag` 为空时输入是实信号；长度和 `real` 一样时按复数输入。
pub fn fft_forward_complex(real: &[f64], imag: &[f64], dt: f64, shape: FftShape) -> Result<FftSeries, String> {
    check_series(real, dt)?;
    let n = real.len();
    let mut buffer: Vec<Complex<f64>> = if imag.is_empty() {
        real.iter().copied().map(|value| Complex::new(value, 0.0)).collect()
    } else {
        if imag.len() != n {
            return Err(sciplot_core::tr!(
                "FFT 的实部和虚部长度要一样",
                "FFT needs the real and imaginary parts to have the same length"
            )
            .into());
        }
        if imag.iter().any(|value| !value.is_finite()) {
            return Err(sciplot_core::tr!("FFT 的虚部有非有限数值", "The imaginary part of the FFT is not finite").into());
        }
        real.iter()
            .zip(imag.iter())
            .map(|(re, im)| Complex::new(*re, *im))
            .collect()
    };
    let mut planner = FftPlanner::new();
    planner.plan_fft_forward(n).process(&mut buffer);
    if shape.normalize {
        let scale = 1.0 / n as f64;
        for value in &mut buffer {
            *value *= scale;
        }
    }
    if shape.shift {
        buffer.rotate_right(n / 2);
    }
    Ok(pack_series(buffer, frequencies(n, dt, shape.shift)))
}

pub fn fft_inverse(real: &[f64], imag: &[f64], dt: f64, shape: FftShape) -> Result<FftSeries, String> {
    if real.len() != imag.len() {
        return Err(sciplot_core::tr!(
            "逆 FFT 的实部和虚部长度要一样",
            "Inverse FFT needs the real and imaginary parts to have the same length"
        )
        .into());
    }
    check_series(real, dt)?;
    if imag.iter().any(|value| !value.is_finite()) {
        return Err(sciplot_core::tr!("逆 FFT 的虚部有非有限数值", "The imaginary part of the inverse FFT is not finite").into());
    }
    let n = real.len();
    let mut buffer: Vec<Complex<f64>> = real
        .iter()
        .zip(imag.iter())
        .map(|(re, im)| Complex::new(*re, *im))
        .collect();
    if shape.shift {
        buffer.rotate_left(n / 2);
    }
    if shape.normalize {
        let scale = n as f64;
        for value in &mut buffer {
            *value *= scale;
        }
    }
    let mut planner = FftPlanner::new();
    planner.plan_fft_inverse(n).process(&mut buffer);
    let scale = 1.0 / n as f64;
    for value in &mut buffer {
        *value *= scale;
    }
    let axis: Vec<f64> = (0..n).map(|i| i as f64 * dt).collect();
    Ok(pack_series(buffer, axis))
}

fn check_series(y: &[f64], dt: f64) -> Result<(), String> {
    if y.len() < 4 {
        return Err(sciplot_core::t("FFT 至少需要 4 个有限点").into());
    }
    if y.iter().any(|value| !value.is_finite()) {
        return Err(sciplot_core::tr!("FFT 输入有非有限数值", "The FFT input contains a non-finite value").into());
    }
    if !(dt.is_finite() && dt > 0.0) {
        return Err(sciplot_core::t("采样间隔无效").into());
    }
    Ok(())
}

fn frequencies(n: usize, dt: f64, shift: bool) -> Vec<f64> {
    let mut freq = Vec::with_capacity(n);
    for k in 0..n {
        let value = if k <= n / 2 {
            k as f64 / (n as f64 * dt)
        } else {
            (k as f64 - n as f64) / (n as f64 * dt)
        };
        freq.push(value);
    }
    if shift {
        freq.rotate_right(n / 2);
    }
    freq
}

fn pack_series(buffer: Vec<Complex<f64>>, axis: Vec<f64>) -> FftSeries {
    let mut real = Vec::with_capacity(buffer.len());
    let mut imag = Vec::with_capacity(buffer.len());
    let mut amplitude = Vec::with_capacity(buffer.len());
    let mut phase = Vec::with_capacity(buffer.len());
    for value in buffer {
        real.push(value.re);
        imag.push(value.im);
        amplitude.push(value.norm());
        phase.push(value.arg());
    }
    FftSeries {
        axis,
        real,
        imag,
        amplitude,
        phase,
    }
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

    #[test]
    fn band_pass_keeps_one_tone_and_band_stop_removes_it() {
        let n = 128;
        let x: Vec<f64> = (0..n).map(|i| i as f64).collect();
        let y: Vec<f64> = (0..n)
            .map(|i| {
                let t = i as f64 / n as f64;
                (2.0 * std::f64::consts::PI * 4.0 * t).sin() + 0.5 * (2.0 * std::f64::consts::PI * 16.0 * t).sin()
            })
            .collect();
        let pure: Vec<f64> = (0..n)
            .map(|i| (2.0 * std::f64::consts::PI * 4.0 * i as f64 / n as f64).sin())
            .collect();
        let passed = fft_filter_band(
            &x,
            &y,
            FftBand::BandPass {
                low: 3.0 / n as f64,
                high: 8.0 / n as f64,
            },
        )
        .unwrap();
        let err: f64 = passed.iter().zip(&pure).map(|(a, b)| (a - b).powi(2)).sum();
        assert!(err < 1e-6, "band pass {err}");
        let stopped = fft_filter_band(
            &x,
            &y,
            FftBand::BandStop {
                low: 3.0 / n as f64,
                high: 8.0 / n as f64,
            },
        )
        .unwrap();
        let leftover: f64 = stopped.iter().zip(&pure).map(|(a, b)| a * b).sum::<f64>().abs();
        assert!(leftover < 1e-4, "band stop still contains the low tone {leftover}");
    }

    #[test]
    fn inverse_fft_roundtrips_with_normalization_and_shift() {
        let y: Vec<f64> = (0..32).map(|i| (i as f64 * 0.3).sin() + 0.2).collect();
        let shape = FftShape {
            normalize: true,
            shift: true,
        };
        let spectrum = fft_forward(&y, 0.5, shape).unwrap();
        assert_eq!(spectrum.real.len(), y.len());
        assert_eq!(spectrum.imag.len(), y.len());
        assert!((spectrum.axis[y.len() / 2]).abs() < 1e-9, "shifted zero frequency {:?}", spectrum.axis);
        let back = fft_inverse(&spectrum.real, &spectrum.imag, 0.5, shape).unwrap();
        for (original, restored) in y.iter().zip(&back.real) {
            assert!((original - restored).abs() < 1e-8, "{original} vs {restored}");
        }
        assert!(back.imag.iter().all(|value| value.abs() < 1e-8));
        let dc = fft_forward(
            &[1.0; 16],
            1.0,
            FftShape {
                normalize: true,
                shift: true,
            },
        )
        .unwrap();
        assert!((dc.amplitude[8] - 1.0).abs() < 1e-8, "{:?}", dc.amplitude);
        assert!(dc.amplitude[8] > dc.amplitude[0]);
    }
}
