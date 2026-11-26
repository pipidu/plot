/// 中心差分。端点用单侧差分。x 不要求等间距，但应按采样顺序排列。
pub fn derivative(x: &[f64], y: &[f64]) -> Vec<f64> {
    let n = x.len().min(y.len());
    let mut out = vec![f64::NAN; n];
    if n == 0 {
        return out;
    }
    if n == 1 {
        return out;
    }
    for i in 0..n {
        let (x0, y0, x1, y1) = if i == 0 {
            (x[0], y[0], x[1], y[1])
        } else if i + 1 == n {
            (x[n - 2], y[n - 2], x[n - 1], y[n - 1])
        } else {
            (x[i - 1], y[i - 1], x[i + 1], y[i + 1])
        };
        let dx = x1 - x0;
        if dx.abs() > 0.0 && y0.is_finite() && y1.is_finite() {
            out[i] = (y1 - y0) / dx;
        }
    }
    out
}

/// 累积梯形积分，起点为 0。
pub fn cumulative_integral(x: &[f64], y: &[f64]) -> Vec<f64> {
    let n = x.len().min(y.len());
    let mut out = vec![f64::NAN; n];
    if n == 0 {
        return out;
    }
    out[0] = if y[0].is_finite() { 0.0 } else { f64::NAN };
    let mut acc = 0.0;
    let mut valid = y[0].is_finite();
    for i in 1..n {
        let dx = x[i] - x[i - 1];
        if valid && y[i].is_finite() && dx.is_finite() {
            acc += 0.5 * (y[i] + y[i - 1]) * dx;
            out[i] = acc;
        } else {
            valid = y[i].is_finite();
            acc = 0.0;
            out[i] = if valid { 0.0 } else { f64::NAN };
        }
    }
    out
}

pub fn definite_integral(x: &[f64], y: &[f64], xmin: f64, xmax: f64) -> Result<f64, String> {
    let (xmin, xmax) = if xmin <= xmax { (xmin, xmax) } else { (xmax, xmin) };
    let n = x.len().min(y.len());
    if n < 2 {
        return Err(sciplot_core::t("积分至少需要两个点").into());
    }
    let mut acc = 0.0;
    let mut any = false;
    for i in 1..n {
        if !x[i - 1].is_finite() || !x[i].is_finite() || !y[i - 1].is_finite() || !y[i].is_finite() {
            continue;
        }
        let mut a = x[i - 1];
        let mut b = x[i];
        let mut ya = y[i - 1];
        let mut yb = y[i];
        if a > b {
            std::mem::swap(&mut a, &mut b);
            std::mem::swap(&mut ya, &mut yb);
        }
        let lo = a.max(xmin);
        let hi = b.min(xmax);
        if hi <= lo {
            continue;
        }
        let denom = b - a;
        let y_lo = if denom.abs() < 1e-15 {
            ya
        } else {
            ya + (yb - ya) * (lo - a) / denom
        };
        let y_hi = if denom.abs() < 1e-15 {
            yb
        } else {
            ya + (yb - ya) * (hi - a) / denom
        };
        acc += 0.5 * (y_lo + y_hi) * (hi - lo);
        any = true;
    }
    if !any {
        return Err(sciplot_core::t("给定区间内没有可用数据").into());
    }
    Ok(acc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derivative_of_square_and_integral_of_sin() {
        let x: Vec<f64> = (0..21).map(|i| i as f64 * 0.1).collect();
        let y: Vec<f64> = x.iter().map(|v| v * v).collect();
        let d = derivative(&x, &y);
        assert!((d[10] - 2.0 * x[10]).abs() < 1e-9);

        let n = 400;
        let xs: Vec<f64> = (0..n).map(|i| i as f64 / (n as f64 - 1.0) * std::f64::consts::PI).collect();
        let ys: Vec<f64> = xs.iter().map(|v| v.sin()).collect();
        let area = definite_integral(&xs, &ys, 0.0, std::f64::consts::PI).unwrap();
        assert!((area - 2.0).abs() < 1e-3);
        let cum = cumulative_integral(&xs, &ys);
        assert!((cum[n - 1] - area).abs() < 1e-9);
    }
}
