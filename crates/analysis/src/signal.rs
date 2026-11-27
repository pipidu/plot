//! 两条信号的线性卷积、反卷积，以及协方差函数。
//!
//! 卷积是非循环的全卷积，结果长度是 `n + m - 1`，横坐标用样本序号。
//! 反卷积用多项式除法，要求响应的第一个点不为 0；这样对刚才的卷积可以还原。
//! 相关是去均值后的协方差，除以第一条信号的长度（有偏估计）。横坐标是滞后，可以为负。

#[derive(Clone, Debug)]
pub struct Correlation {
    pub lag: Vec<f64>,
    pub value: Vec<f64>,
}

pub fn convolve(a: &[f64], b: &[f64]) -> Result<Vec<f64>, String> {
    let a = finite_signal(a)?;
    let b = finite_signal(b)?;
    if a.is_empty() || b.is_empty() {
        return Err(sciplot_core::tr!("卷积需要两条非空信号", "Convolution needs two non-empty signals").into());
    }
    let mut out = vec![0.0; a.len() + b.len() - 1];
    for (i, &ai) in a.iter().enumerate() {
        for (j, &bj) in b.iter().enumerate() {
            out[i + j] += ai * bj;
        }
    }
    Ok(out)
}

pub fn deconvolve(observed: &[f64], kernel: &[f64]) -> Result<Vec<f64>, String> {
    let observed = finite_signal(observed)?;
    let kernel = finite_signal(kernel)?;
    if kernel.is_empty() || observed.is_empty() {
        return Err(sciplot_core::tr!("反卷积需要信号和响应", "Deconvolution needs a signal and a response").into());
    }
    let scale = kernel.iter().fold(0.0_f64, |acc, value| acc.max(value.abs()));
    if kernel[0].abs() <= scale * 1e-8 {
        return Err(sciplot_core::tr!(
            "反卷积要求响应的第一个点不为 0",
            "Deconvolution needs a response whose first sample is not zero"
        )
        .into());
    }
    if observed.len() < kernel.len() {
        return Err(sciplot_core::tr!(
            "信号比响应短，无法反卷积",
            "The signal is shorter than the response, so it cannot be deconvolved"
        )
        .into());
    }
    let n = observed.len() - kernel.len() + 1;
    let mut signal = vec![0.0; n];
    let lead = kernel[0];
    for i in 0..n {
        let mut acc = observed[i];
        for j in 1..kernel.len() {
            if i >= j {
                acc -= signal[i - j] * kernel[j];
            }
        }
        signal[i] = acc / lead;
    }
    Ok(signal)
}

pub fn autocorrelate(signal: &[f64]) -> Result<Correlation, String> {
    let signal = finite_signal(signal)?;
    correlate(&signal, &signal)
}

pub fn correlate(a: &[f64], b: &[f64]) -> Result<Correlation, String> {
    let a = finite_signal(a)?;
    let b = finite_signal(b)?;
    if a.len() < 2 || b.len() < 2 {
        return Err(sciplot_core::tr!(
            "信号相关至少需要两条各含 2 个点的信号",
            "Signal correlation needs two signals of at least 2 points"
        )
        .into());
    }
    let mean_a = mean(&a);
    let mean_b = mean(&b);
    let norm = a.len() as f64;
    let k_min = 1 - b.len() as i64;
    let k_max = a.len() as i64 - 1;
    let mut lag = Vec::new();
    let mut value = Vec::new();
    for k in k_min..=k_max {
        let mut sum = 0.0;
        for (i, &ai) in a.iter().enumerate() {
            let j = i as i64 - k;
            if j >= 0 && (j as usize) < b.len() {
                sum += (ai - mean_a) * (b[j as usize] - mean_b);
            }
        }
        lag.push(k as f64);
        value.push(sum / norm);
    }
    Ok(Correlation { lag, value })
}

fn finite_signal(values: &[f64]) -> Result<Vec<f64>, String> {
    if values.iter().any(|value| !value.is_finite()) {
        return Err(sciplot_core::tr!("信号里有非有限数值", "The signal contains a non-finite value").into());
    }
    Ok(values.to_vec())
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convolution_roundtrips_through_deconvolution() {
        let signal = [1.0, 2.0, 3.0];
        let kernel = [1.0, 1.0];
        let observed = convolve(&signal, &kernel).unwrap();
        assert_eq!(observed, vec![1.0, 3.0, 5.0, 3.0]);
        let back = deconvolve(&observed, &kernel).unwrap();
        assert!((back[0] - 1.0).abs() < 1e-9, "{back:?}");
        assert!((back[1] - 2.0).abs() < 1e-9, "{back:?}");
        assert!((back[2] - 3.0).abs() < 1e-9, "{back:?}");
        let err = deconvolve(&observed, &[0.0, 1.0]).unwrap_err();
        assert!(err.contains("第一个点"), "{err}");
    }

    #[test]
    fn autocorrelation_is_the_biased_covariance() {
        let signal = [1.0, 2.0, 3.0];
        let corr = autocorrelate(&signal).unwrap();
        assert_eq!(corr.lag, vec![-2.0, -1.0, 0.0, 1.0, 2.0]);
        assert!((corr.value[2] - 2.0 / 3.0).abs() < 1e-12, "{:?}", corr.value);
        assert!(corr.value[1].abs() < 1e-12, "{:?}", corr.value);
        assert!((corr.value[0] - corr.value[4]).abs() < 1e-12, "{:?}", corr.value);
        assert!((corr.value[0] + 1.0 / 3.0).abs() < 1e-12, "{:?}", corr.value);
    }
}
