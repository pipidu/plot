use sciplot_core::CompiledExpr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InterpMethod {
    Linear,
    CubicSpline,
    Akima,
}

impl InterpMethod {
    pub fn label(self) -> &'static str {
        match self {
            InterpMethod::Linear => "线性",
            InterpMethod::CubicSpline => "三次样条",
            InterpMethod::Akima => "Akima",
        }
    }
}

pub struct CubicSpline {
    x: Vec<f64>,
    y: Vec<f64>,
    /// 各节点处的二阶导数。自然边界条件两端为 0。
    m: Vec<f64>,
}

impl CubicSpline {
    pub fn new(x: &[f64], y: &[f64]) -> Result<Self, String> {
        let (x, y) = prepare(x, y)?;
        let n = x.len();
        let mut m = vec![0.0; n];
        if n > 2 {
            let mut lower = vec![0.0; n];
            let mut diag = vec![0.0; n];
            let mut upper = vec![0.0; n];
            let mut rhs = vec![0.0; n];
            for i in 1..n - 1 {
                let h0 = x[i] - x[i - 1];
                let h1 = x[i + 1] - x[i];
                lower[i] = h0;
                diag[i] = 2.0 * (h0 + h1);
                upper[i] = h1;
                rhs[i] = 6.0 * ((y[i + 1] - y[i]) / h1 - (y[i] - y[i - 1]) / h0);
            }
            let mids = n - 2;
            let mut cp = vec![0.0; mids];
            let mut dp = vec![0.0; mids];
            cp[0] = upper[1] / diag[1];
            dp[0] = rhs[1] / diag[1];
            for k in 1..mids {
                let i = k + 1;
                let denom = diag[i] - lower[i] * cp[k - 1];
                if denom.abs() < 1e-18 {
                    return Err("样条方程组奇异".into());
                }
                let up = if k + 1 < mids { upper[i] } else { 0.0 };
                cp[k] = up / denom;
                dp[k] = (rhs[i] - lower[i] * dp[k - 1]) / denom;
            }
            m[n - 2] = dp[mids - 1];
            for k in (0..mids - 1).rev() {
                m[k + 1] = dp[k] - cp[k] * m[k + 2];
            }
        }
        Ok(Self { x, y, m })
    }

    pub fn eval(&self, xq: f64) -> f64 {
        let n = self.x.len();
        if !xq.is_finite() {
            return f64::NAN;
        }
        let i = if xq <= self.x[0] {
            0
        } else if xq >= self.x[n - 1] {
            n - 2
        } else {
            match self.x.binary_search_by(|v| v.partial_cmp(&xq).unwrap_or(std::cmp::Ordering::Equal)) {
                Ok(hit) => return self.y[hit],
                Err(idx) => idx - 1,
            }
        };
        let h = self.x[i + 1] - self.x[i];
        let t = xq - self.x[i];
        let m0 = self.m[i];
        let m1 = self.m[i + 1];
        let a = self.y[i];
        let b = (self.y[i + 1] - self.y[i]) / h - h * (2.0 * m0 + m1) / 6.0;
        let c = m0 / 2.0;
        let d = (m1 - m0) / (6.0 * h);
        a + b * t + c * t * t + d * t * t * t
    }
}

pub struct Akima {
    x: Vec<f64>,
    y: Vec<f64>,
    slope: Vec<f64>,
}

impl Akima {
    pub fn new(x: &[f64], y: &[f64]) -> Result<Self, String> {
        let (x, y) = prepare(x, y)?;
        let n = x.len();
        if n == 2 {
            let slope = (y[1] - y[0]) / (x[1] - x[0]);
            return Ok(Self {
                x,
                y,
                slope: vec![slope, slope],
            });
        }
        let mut delta = vec![0.0; n + 3];
        for i in 0..n - 1 {
            delta[i + 2] = (y[i + 1] - y[i]) / (x[i + 1] - x[i]);
        }
        delta[1] = 2.0 * delta[2] - delta[3];
        delta[0] = 2.0 * delta[1] - delta[2];
        delta[n + 1] = 2.0 * delta[n] - delta[n - 1];
        delta[n + 2] = 2.0 * delta[n + 1] - delta[n];
        let mut slope = vec![0.0; n];
        for i in 0..n {
            let dm2 = delta[i];
            let dm1 = delta[i + 1];
            let d0 = delta[i + 2];
            let d1 = delta[i + 3];
            let w1 = (d1 - d0).abs();
            let w2 = (dm1 - dm2).abs();
            slope[i] = if w1 + w2 == 0.0 {
                0.5 * (dm1 + d0)
            } else {
                (w1 * dm1 + w2 * d0) / (w1 + w2)
            };
        }
        Ok(Self { x, y, slope })
    }

    pub fn eval(&self, xq: f64) -> f64 {
        let n = self.x.len();
        if !xq.is_finite() {
            return f64::NAN;
        }
        let i = if xq <= self.x[0] {
            0
        } else if xq >= self.x[n - 1] {
            n - 2
        } else {
            match self.x.binary_search_by(|v| v.partial_cmp(&xq).unwrap_or(std::cmp::Ordering::Equal)) {
                Ok(hit) => return self.y[hit],
                Err(idx) => idx - 1,
            }
        };
        let h = self.x[i + 1] - self.x[i];
        let t = ((xq - self.x[i]) / h).clamp(0.0, 1.0);
        hermite(t, h, self.y[i], self.y[i + 1], self.slope[i], self.slope[i + 1])
    }
}

fn hermite(t: f64, h: f64, y0: f64, y1: f64, s0: f64, s1: f64) -> f64 {
    let t2 = t * t;
    let t3 = t2 * t;
    let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
    let h10 = t3 - 2.0 * t2 + t;
    let h01 = -2.0 * t3 + 3.0 * t2;
    let h11 = t3 - t2;
    h00 * y0 + h10 * h * s0 + h01 * y1 + h11 * h * s1
}

#[allow(dead_code)]
pub fn linear_eval(x: &[f64], y: &[f64], xq: f64) -> Result<f64, String> {
    let (x, y) = prepare(x, y)?;
    Ok(eval_linear(&x, &y, xq))
}

fn eval_linear(x: &[f64], y: &[f64], xq: f64) -> f64 {
    if xq <= x[0] {
        return extrapolate(x[0], y[0], x[1], y[1], xq);
    }
    if xq >= x[x.len() - 1] {
        let n = x.len();
        return extrapolate(x[n - 2], y[n - 2], x[n - 1], y[n - 1], xq);
    }
    match x.binary_search_by(|v| v.partial_cmp(&xq).unwrap_or(std::cmp::Ordering::Equal)) {
        Ok(i) => y[i],
        Err(i) => {
            let t = (xq - x[i - 1]) / (x[i] - x[i - 1]);
            y[i - 1] + t * (y[i] - y[i - 1])
        }
    }
}

fn extrapolate(x0: f64, y0: f64, x1: f64, y1: f64, xq: f64) -> f64 {
    let dx = x1 - x0;
    if dx.abs() < 1e-15 {
        y0
    } else {
        y0 + (y1 - y0) * (xq - x0) / dx
    }
}

pub fn interpolate(x: &[f64], y: &[f64], xq: &[f64], method: InterpMethod) -> Result<Vec<f64>, String> {
    match method {
        InterpMethod::Linear => {
            let (x, y) = prepare(x, y)?;
            Ok(xq.iter().map(|&q| eval_linear(&x, &y, q)).collect())
        }
        InterpMethod::CubicSpline => {
            let spline = CubicSpline::new(x, y)?;
            Ok(xq.iter().map(|&q| spline.eval(q)).collect())
        }
        InterpMethod::Akima => {
            let akima = Akima::new(x, y)?;
            Ok(xq.iter().map(|&q| akima.eval(q)).collect())
        }
    }
}

fn prepare(x: &[f64], y: &[f64]) -> Result<(Vec<f64>, Vec<f64>), String> {
    let mut pairs: Vec<(f64, f64)> = x
        .iter()
        .zip(y.iter())
        .filter(|(a, b)| a.is_finite() && b.is_finite())
        .map(|(a, b)| (*a, *b))
        .collect();
    if pairs.len() < 2 {
        return Err("插值至少需要两个有限点".into());
    }
    pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
    pairs.dedup_by(|a, b| (a.0 - b.0).abs() <= 1e-12 * (1.0 + a.0.abs()));
    if pairs.len() < 2 {
        return Err("x 值全部重复，无法插值".into());
    }
    for w in pairs.windows(2) {
        if w[1].0 <= w[0].0 {
            return Err("x 必须严格递增".into());
        }
    }
    Ok((pairs.iter().map(|p| p.0).collect(), pairs.iter().map(|p| p.1).collect()))
}

/// 供拟合表达式在规则网格上取样。analysis 内部也会直接用 core 的编译器。
#[allow(dead_code)]
pub fn eval_expr_grid(expr: &str, xs: &[f64]) -> Result<Vec<f64>, String> {
    let compiled = CompiledExpr::compile(expr)?;
    Ok(xs
        .iter()
        .map(|&x| compiled.eval(&[("x", x)]).unwrap_or(f64::NAN))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spline_and_akima_reproduce_a_line() {
        let x = [0.0, 1.0, 2.0, 3.0, 4.0];
        let y: Vec<f64> = x.iter().map(|v| 2.0 * v + 1.0).collect();
        let queries = [0.5, 1.5, 2.2, 3.8];
        for method in [InterpMethod::Linear, InterpMethod::CubicSpline, InterpMethod::Akima] {
            let got = interpolate(&x, &y, &queries, method).unwrap();
            for (q, g) in queries.iter().zip(got.iter()) {
                assert!((g - (2.0 * q + 1.0)).abs() < 1e-8, "{method:?} at {q} -> {g}");
            }
        }
    }

    #[test]
    fn spline_hits_nodes() {
        let x = [0.0, 1.0, 2.0, 3.0];
        let y = [0.0, 1.0, 0.0, 1.0];
        let spline = CubicSpline::new(&x, &y).unwrap();
        for i in 0..x.len() {
            assert!((spline.eval(x[i]) - y[i]).abs() < 1e-8);
        }
    }
}
