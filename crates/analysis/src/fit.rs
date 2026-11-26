use nalgebra::{DMatrix, DVector};
use sciplot_core::{parameter_names, substitute_params, CompiledExpr};

#[derive(Clone, Debug)]
pub enum FitModel {
    Linear,
    Polynomial(usize),
    Exponential,
    Gaussian,
    Lorentzian,
    Custom {
        expr: String,
        initial: Vec<(String, f64)>,
    },
}

impl FitModel {
    pub fn label(&self) -> &'static str {
        match self {
            FitModel::Linear => "线性",
            FitModel::Polynomial(_) => "多项式",
            FitModel::Exponential => "指数",
            FitModel::Gaussian => "高斯",
            FitModel::Lorentzian => "洛伦兹",
            FitModel::Custom { .. } => "自定义",
        }
    }
}

#[derive(Clone, Debug)]
pub struct FitResult {
    pub names: Vec<String>,
    pub values: Vec<f64>,
    pub stderr: Vec<f64>,
    pub r2: f64,
    pub chi2: f64,
    pub n: usize,
    pub expr: String,
    pub note: String,
}

impl FitResult {
    /// 给学生看的表达式。线性和多项式写成 `y = 0.941 − 0.299x`，计算用的 `expr` 不变。
    pub fn pretty_expr(&self) -> String {
        if self.names.first().map(String::as_str) == Some("截距") && self.values.len() >= 2 {
            return pretty_linear(self.values[0], self.values[1]);
        }
        let polynomial = self.names.len() == self.values.len()
            && self.names.iter().enumerate().all(|(index, name)| name == &format!("a{index}"));
        if polynomial {
            return pretty_poly(&self.values);
        }
        format!("y = {}", self.expr)
    }

    pub fn report(&self) -> String {
        let mut text = format!(
            "n = {}    R² = {}    χ² = {}（残差平方和）\n{}\n",
            self.n,
            sig_digits(self.r2, 6),
            sig_digits(self.chi2, 6),
            self.pretty_expr()
        );
        for index in 0..self.names.len() {
            let error = self.stderr.get(index).copied().unwrap_or(f64::NAN);
            text.push_str(&format!(
                "  {} = {}  ± {}\n",
                self.names[index],
                sig_digits(self.values[index], 6),
                sig_digits(error, 6)
            ));
        }
        if !self.note.is_empty() {
            text.push_str(&self.note);
            text.push('\n');
        }
        text
    }
}

pub fn fit_series(x: &[f64], y: &[f64], model: &FitModel) -> Result<FitResult, String> {
    let (x, y) = finite_pairs(x, y)?;
    match model {
        FitModel::Linear => {
            let mut result = fit_polynomial(&x, &y, 1)?;
            result.names = vec!["截距".into(), "斜率".into()];
            Ok(result)
        }
        FitModel::Polynomial(degree) => fit_polynomial(&x, &y, *degree),
        FitModel::Exponential => fit_exponential(&x, &y),
        FitModel::Gaussian => fit_peak(&x, &y, Peak::Gaussian),
        FitModel::Lorentzian => fit_peak(&x, &y, Peak::Lorentzian),
        FitModel::Custom { expr, initial } => fit_custom(&x, &y, expr, initial),
    }
}

fn finite_pairs(x: &[f64], y: &[f64]) -> Result<(Vec<f64>, Vec<f64>), String> {
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    for (x, y) in x.iter().zip(y.iter()) {
        if x.is_finite() && y.is_finite() {
            xs.push(*x);
            ys.push(*y);
        }
    }
    if xs.len() < 2 {
        return Err("拟合至少需要两个有限数据点".into());
    }
    Ok((xs, ys))
}

fn fit_polynomial(x: &[f64], y: &[f64], degree: usize) -> Result<FitResult, String> {
    let n = x.len();
    let p = degree + 1;
    if n < p {
        return Err(format!("多项式 {degree} 阶至少需要 {p} 个点，当前只有 {n} 个"));
    }
    let mut design = DMatrix::zeros(n, p);
    for (row, &xi) in x.iter().enumerate() {
        let mut power = 1.0;
        for col in 0..p {
            design[(row, col)] = power;
            power *= xi;
        }
    }
    let target = DVector::from_column_slice(y);
    let normal = design.transpose() * &design;
    let rhs = design.transpose() * target;
    let solution = normal
        .clone()
        .lu()
        .solve(&rhs)
        .ok_or("多项式拟合失败，设计矩阵秩不足")?;
    let values: Vec<f64> = solution.iter().copied().collect();
    let predicted: Vec<f64> = (0..n)
        .map(|row| (0..p).map(|col| design[(row, col)] * values[col]).sum())
        .collect();
    let (r2, chi2) = goodness(y, &predicted);
    let sigma2 = if n > p { chi2 / (n - p) as f64 } else { 0.0 };
    let stderr = match normal.try_inverse() {
        Some(cov) => (0..p).map(|i| (sigma2 * cov[(i, i)].max(0.0)).sqrt()).collect(),
        None => vec![f64::NAN; p],
    };
    let names = (0..p).map(|i| format!("a{i}")).collect();
    Ok(FitResult {
        names,
        expr: polynomial_expr(&values),
        values,
        stderr,
        r2,
        chi2,
        n,
        note: String::new(),
    })
}

fn polynomial_expr(coeff: &[f64]) -> String {
    let mut terms = Vec::new();
    for (power, &c) in coeff.iter().enumerate() {
        if c.abs() < 1e-14 {
            continue;
        }
        let number = coeff_text(c);
        let term = match power {
            0 => number,
            1 => format!("{number}*x"),
            _ => format!("{number}*x^{power}"),
        };
        terms.push(term);
    }
    if terms.is_empty() {
        "0".to_string()
    } else {
        terms.join("+")
    }
}

fn fit_exponential(x: &[f64], y: &[f64]) -> Result<FitResult, String> {
    let (a0, b0) = exponential_guess(x, y);
    let model = |p: &[f64], xi: f64| p[0] * (p[1] * xi).exp();
    let expr_of = |p: &[f64]| format!("{}*exp({}*x)", coeff_text(p[0]), coeff_text(p[1]));
    nonlinear(x, y, vec![a0, b0], &["a", "b"], model, expr_of, "y = a·exp(b·x)")
}

#[derive(Clone, Copy)]
enum Peak {
    Gaussian,
    Lorentzian,
}

fn fit_peak(x: &[f64], y: &[f64], kind: Peak) -> Result<FitResult, String> {
    let (a0, b0, c0) = peak_guess(x, y);
    match kind {
        Peak::Gaussian => {
            let model = |p: &[f64], xi: f64| {
                let c = p[2].abs().max(1e-8);
                p[0] * (-(xi - p[1]).powi(2) / (2.0 * c * c)).exp()
            };
            let expr_of = |p: &[f64]| {
                format!(
                    "{}*exp(-(x-({}))^2/(2*({})^2))",
                    coeff_text(p[0]),
                    coeff_text(p[1]),
                    coeff_text(p[2].abs().max(1e-8))
                )
            };
            nonlinear(x, y, vec![a0, b0, c0], &["a", "μ", "σ"], model, expr_of, "高斯 y = a·exp(-(x-μ)²/(2σ²))")
        }
        Peak::Lorentzian => {
            let model = |p: &[f64], xi: f64| {
                let c = p[2].abs().max(1e-8);
                p[0] * c * c / ((xi - p[1]).powi(2) + c * c)
            };
            let expr_of = |p: &[f64]| {
                let c = coeff_text(p[2].abs().max(1e-8));
                let b = coeff_text(p[1]);
                format!("{}*({c})^2/((x-({b}))^2+({c})^2)", coeff_text(p[0]))
            };
            nonlinear(x, y, vec![a0, b0, c0], &["a", "x0", "γ"], model, expr_of, "洛伦兹 y = a·γ²/((x-x0)²+γ²)")
        }
    }
}

fn fit_custom(x: &[f64], y: &[f64], expr: &str, initial: &[(String, f64)]) -> Result<FitResult, String> {
    let names = parameter_names(expr);
    if names.is_empty() {
        return Err("自定义表达式需要待拟合参数，例如 a*exp(-b*x)+c".into());
    }
    let compiled = CompiledExpr::compile(expr)?;
    let p0: Vec<f64> = names
        .iter()
        .map(|name| {
            initial
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| *v)
                .unwrap_or(1.0)
        })
        .collect();
    let residual = |p: &[f64]| -> Vec<f64> {
        x.iter()
            .zip(y.iter())
            .map(|(&xi, &yi)| {
                let mut vars = Vec::with_capacity(names.len() + 1);
                vars.push(("x", xi));
                for (name, &value) in names.iter().zip(p.iter()) {
                    vars.push((name.as_str(), value));
                }
                match compiled.eval(&vars) {
                    Ok(value) if value.is_finite() => value - yi,
                    _ => 1.0e3,
                }
            })
            .collect()
    };
    let (values, stderr, note) = levenberg(&p0, &residual);
    let predicted = predict_from_residual(y, &residual(&values));
    let (r2, chi2) = goodness(y, &predicted);
    let pairs: Vec<(String, f64)> = names.iter().cloned().zip(values.iter().copied()).collect();
    Ok(FitResult {
        names,
        values,
        stderr,
        r2,
        chi2,
        n: x.len(),
        expr: substitute_params(expr, &pairs),
        note,
    })
}

fn nonlinear<F, E>(
    x: &[f64],
    y: &[f64],
    p0: Vec<f64>,
    names: &[&str],
    model: F,
    expr_of: E,
    note_prefix: &str,
) -> Result<FitResult, String>
where
    F: Fn(&[f64], f64) -> f64,
    E: Fn(&[f64]) -> String,
{
    let residual = |p: &[f64]| -> Vec<f64> {
        x.iter()
            .zip(y.iter())
            .map(|(&xi, &yi)| {
                let value = model(p, xi);
                if value.is_finite() {
                    value - yi
                } else {
                    1.0e3
                }
            })
            .collect()
    };
    let (values, stderr, note) = levenberg(&p0, &residual);
    let predicted = predict_from_residual(y, &residual(&values));
    let (r2, chi2) = goodness(y, &predicted);
    let mut full_note = note_prefix.to_string();
    if !note.is_empty() {
        full_note.push('；');
        full_note.push_str(&note);
    }
    Ok(FitResult {
        names: names.iter().map(|n| (*n).to_string()).collect(),
        expr: expr_of(&values),
        values,
        stderr,
        r2,
        chi2,
        n: x.len(),
        note: full_note,
    })
}

fn exponential_guess(x: &[f64], y: &[f64]) -> (f64, f64) {
    if y.iter().all(|v| *v > 0.0) {
        let ly: Vec<f64> = y.iter().map(|v| v.ln()).collect();
        if let Ok(linear) = fit_polynomial(x, &ly, 1) {
            return (linear.values[0].exp(), linear.values[1]);
        }
    }
    let amplitude = y.iter().copied().fold(0.0_f64, |acc: f64, v| if v.abs() > acc.abs() { v } else { acc });
    (if amplitude == 0.0 { 1.0 } else { amplitude }, 0.0)
}

fn peak_guess(x: &[f64], y: &[f64]) -> (f64, f64, f64) {
    let (index, amplitude) = y
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.abs().total_cmp(&b.abs()))
        .map(|(i, v)| (i, *v))
        .unwrap_or((0, 1.0));
    let center = x[index];
    let xmin = x.iter().copied().fold(f64::INFINITY, f64::min);
    let xmax = x.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let width = ((xmax - xmin) / 6.0).abs().max(1e-2);
    (amplitude, center, width)
}

fn levenberg<F: Fn(&[f64]) -> Vec<f64>>(p0: &[f64], residual: &F) -> (Vec<f64>, Vec<f64>, String) {
    let mut p = p0.to_vec();
    let mut lambda = 1e-3;
    let mut note = String::new();
    for _ in 0..80 {
        let r = residual(&p);
        if r.iter().any(|v| !v.is_finite()) {
            note = "迭代中出现非有限残差，已保留当前参数".into();
            break;
        }
        let cost = sumsq(&r);
        let jac = jacobian(&p, &r, residual);
        let normal = jac.transpose() * &jac;
        let gradient = jac.transpose() * DVector::from_column_slice(&r);
        let mut improved = false;
        for _attempt in 0..10 {
            let mut system = normal.clone();
            for i in 0..p.len() {
                system[(i, i)] += lambda * system[(i, i)].abs().max(1e-8);
            }
            let Some(delta) = system.lu().solve(&gradient) else {
                lambda = (lambda * 10.0).min(1e12);
                continue;
            };
            let trial: Vec<f64> = p.iter().enumerate().map(|(i, v)| v - delta[i]).collect();
            let r_trial = residual(&trial);
            if r_trial.iter().any(|v| !v.is_finite()) {
                lambda = (lambda * 10.0).min(1e12);
                continue;
            }
            let cost_trial = sumsq(&r_trial);
            if cost_trial < cost {
                let improvement = (cost - cost_trial) / (1.0 + cost);
                p = trial;
                lambda = (lambda * 0.3).max(1e-12);
                improved = true;
                if improvement < 1e-12 {
                    let stderr = standard_errors(&p, residual, cost_trial);
                    return (p, stderr, note);
                }
                break;
            }
            lambda = (lambda * 10.0).min(1e12);
        }
        if !improved {
            note = "已达到步长限制，结果可能是局部最优".into();
            break;
        }
    }
    let r = residual(&p);
    let stderr = standard_errors(&p, residual, sumsq(&r));
    (p, stderr, note)
}

fn jacobian<F: Fn(&[f64]) -> Vec<f64>>(p: &[f64], r0: &[f64], residual: &F) -> DMatrix<f64> {
    let mut jac = DMatrix::zeros(r0.len(), p.len());
    for k in 0..p.len() {
        let step = (1e-6 * (1.0 + p[k].abs())).max(1e-8);
        let mut shifted = p.to_vec();
        shifted[k] += step;
        let r1 = residual(&shifted);
        for row in 0..r0.len().min(r1.len()) {
            jac[(row, k)] = (r1[row] - r0[row]) / step;
        }
    }
    jac
}

fn standard_errors<F: Fn(&[f64]) -> Vec<f64>>(p: &[f64], residual: &F, chi2: f64) -> Vec<f64> {
    let r = residual(p);
    let jac = jacobian(p, &r, residual);
    let normal = jac.transpose() * &jac;
    let dof = r.len().saturating_sub(p.len()).max(1) as f64;
    let sigma2 = chi2 / dof;
    match normal.try_inverse() {
        Some(cov) => (0..p.len()).map(|i| (sigma2 * cov[(i, i)].max(0.0)).sqrt()).collect(),
        None => vec![f64::NAN; p.len()],
    }
}

fn predict_from_residual(y: &[f64], residual: &[f64]) -> Vec<f64> {
    y.iter().zip(residual.iter()).map(|(yi, ri)| yi + ri).collect()
}

fn goodness(y: &[f64], predicted: &[f64]) -> (f64, f64) {
    let n = y.len().min(predicted.len());
    if n == 0 {
        return (f64::NAN, f64::NAN);
    }
    let mean = y.iter().take(n).sum::<f64>() / n as f64;
    let mut ss_res = 0.0;
    let mut ss_tot = 0.0;
    for i in 0..n {
        ss_res += (y[i] - predicted[i]).powi(2);
        ss_tot += (y[i] - mean).powi(2);
    }
    let r2 = if ss_tot < 1e-18 { 1.0 } else { 1.0 - ss_res / ss_tot };
    (r2, ss_res)
}

fn sumsq(values: &[f64]) -> f64 {
    values.iter().map(|v| v * v).sum()
}

fn pretty_linear(intercept: f64, slope: f64) -> String {
    let head = sig_digits(intercept, 6);
    if slope.abs() < 1e-14 {
        return format!("y = {head}");
    }
    let sign = if slope < 0.0 { "−" } else { "+" };
    let magnitude = sig_digits(slope.abs(), 6);
    if magnitude == "1" {
        format!("y = {head} {sign} x")
    } else {
        format!("y = {head} {sign} {magnitude}x")
    }
}

fn pretty_poly(coeff: &[f64]) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (power, &value) in coeff.iter().enumerate() {
        if value.abs() < 1e-14 {
            continue;
        }
        let sign = if value < 0.0 { "−" } else { "+" };
        let magnitude = sig_digits(value.abs(), 6);
        let body = match power {
            0 => magnitude,
            1 if magnitude == "1" => "x".into(),
            1 => format!("{magnitude}x"),
            _ if magnitude == "1" => format!("x^{power}"),
            _ => format!("{magnitude}x^{power}"),
        };
        if parts.is_empty() {
            if value < 0.0 {
                parts.push(format!("−{body}"));
            } else {
                parts.push(body);
            }
        } else {
            parts.push(format!("{sign} {body}"));
        }
    }
    if parts.is_empty() {
        "y = 0".into()
    } else {
        format!("y = {}", parts.join(" "))
    }
}

fn sig_digits(value: f64, digits: usize) -> String {
    if !value.is_finite() {
        return "—".into();
    }
    if value == 0.0 {
        return "0".into();
    }
    let digits = digits.max(1);
    let negative = value < 0.0;
    let abs = value.abs();
    let exp = abs.log10().floor() as i32;
    if exp >= digits as i32 || exp < -3 {
        let text = format!("{:.*e}", digits - 1, abs);
        return if negative { format!("-{text}") } else { text };
    }
    let decimals = (digits as i32 - 1 - exp).max(0) as usize;
    let text = format!("{abs:.decimals$}");
    let text = text.trim_end_matches('0').trim_end_matches('.').to_string();
    let text = if text.is_empty() { "0".into() } else { text };
    if negative {
        format!("-{text}")
    } else {
        text
    }
}

fn coeff_text(value: f64) -> String {
    let text = format!("{value:.8}");
    let text = text.trim_end_matches('0').trim_end_matches('.').to_string();
    if value < 0.0 {
        format!("({text})")
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_and_quadratic_are_exact() {
        let x = [0.0, 1.0, 2.0, 3.0];
        let y = [1.0, 3.0, 5.0, 7.0];
        let fit = fit_series(&x, &y, &FitModel::Linear).unwrap();
        assert!((fit.values[0] - 1.0).abs() < 1e-8);
        assert!((fit.values[1] - 2.0).abs() < 1e-8);
        assert!((fit.r2 - 1.0).abs() < 1e-10);
        assert_eq!(fit.pretty_expr(), "y = 1 + 2x");
        assert!(fit.report().contains("残差平方和"));

        let yq: Vec<f64> = x.iter().map(|v| 1.0 + 2.0 * v + 3.0 * v * v).collect();
        let fit = fit_series(&x, &yq, &FitModel::Polynomial(2)).unwrap();
        assert!((fit.values[0] - 1.0).abs() < 1e-6, "{:?}", fit.values);
        assert!((fit.values[1] - 2.0).abs() < 1e-6, "{:?}", fit.values);
        assert!((fit.values[2] - 3.0).abs() < 1e-6, "{:?}", fit.values);
    }

    #[test]
    fn gaussian_and_custom_line() {
        let x: Vec<f64> = (0..41).map(|i| -1.0 + i as f64 * 0.1).collect();
        let y: Vec<f64> = x
            .iter()
            .map(|v| 3.0 * (-(v - 1.0).powi(2) / (2.0 * 0.4 * 0.4)).exp())
            .collect();
        let fit = fit_series(&x, &y, &FitModel::Gaussian).unwrap();
        assert!((fit.values[0] - 3.0).abs() < 1e-3, "a {:?}", fit.values);
        assert!((fit.values[1] - 1.0).abs() < 1e-3, "mu {:?}", fit.values);
        assert!((fit.values[2].abs() - 0.4).abs() < 1e-3, "sigma {:?}", fit.values);
        assert!(fit.r2 > 0.999);

        let x = [0.0, 1.0, 2.0, 3.0];
        let y = [1.0, 3.0, 5.0, 7.0];
        let fit = fit_series(
            &x,
            &y,
            &FitModel::Custom {
                expr: "a*x+b".into(),
                initial: vec![("a".into(), 0.0), ("b".into(), 0.0)],
            },
        )
        .unwrap();
        assert!((fit.values[0] - 2.0).abs() < 1e-4, "{:?}", fit.values);
        assert!((fit.values[1] - 1.0).abs() < 1e-4, "{:?}", fit.values);
    }
}
