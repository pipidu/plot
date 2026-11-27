use nalgebra::{DMatrix, DVector};
use sciplot_core::{parameter_names, substitute_params, tr, CompiledExpr};

#[derive(Clone, Debug)]
pub enum FitModel {
    Linear,
    Polynomial(usize),
    /// `y = a·exp(b·x)`，没有基线。
    Exponential,
    /// 一阶衰减 `y = y0 + A1·exp(-x/t1)`。
    ExpDecay1,
    /// 二阶衰减 `y = y0 + A1·exp(-x/t1) + A2·exp(-x/t2)`。
    ExpDecay2,
    /// 三阶衰减 `y = y0 + A1·exp(-x/t1) + A2·exp(-x/t2) + A3·exp(-x/t3)`。
    ExpDecay3,
    /// 指数增长 `y = y0 + A·exp(x/t)`。
    ExpGrowth,
    Gaussian,
    Lorentzian,
    /// `y = (A1-A2)/(1+exp((x-x0)/dx))+A2`。
    Boltzmann,
    /// 多峰。`centers` 是用户给出的峰位（图上点选或手填）。
    MultiPeak { gaussian: bool, centers: Vec<f64> },
    Custom {
        expr: String,
        initial: Vec<(String, f64)>,
    },
}

impl FitModel {
    pub fn label(&self) -> &'static str {
        match self {
            FitModel::Linear => tr!("线性", "Linear"),
            FitModel::Polynomial(_) => tr!("多项式", "Polynomial"),
            FitModel::Exponential => tr!("指数", "Exponential"),
            FitModel::ExpDecay1 => tr!("一阶指数衰减", "Exponential decay, 1st order"),
            FitModel::ExpDecay2 => tr!("二阶指数衰减", "Exponential decay, 2nd order"),
            FitModel::ExpDecay3 => tr!("三阶指数衰减", "Exponential decay, 3rd order"),
            FitModel::ExpGrowth => tr!("指数增长", "Exponential growth"),
            FitModel::Gaussian => tr!("高斯", "Gaussian"),
            FitModel::Lorentzian => tr!("洛伦兹", "Lorentzian"),
            FitModel::Boltzmann => tr!("Boltzmann", "Boltzmann"),
            FitModel::MultiPeak { gaussian: true, .. } => tr!("多峰高斯", "Multi-peak Gaussian"),
            FitModel::MultiPeak { gaussian: false, .. } => tr!("多峰洛伦兹", "Multi-peak Lorentzian"),
            FitModel::Custom { .. } => tr!("自定义", "Custom"),
        }
    }

    pub fn nonlinear(&self) -> bool {
        !matches!(self, FitModel::Linear | FitModel::Polynomial(_))
    }
}

/// 权重。仪器误差和数据列都按 `1/σ²`；直接权重用列里的数本身。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Weighting {
    #[default]
    None,
    /// `1/σ²`，σ 来自 y 误差。
    Instrumental,
    /// `1/|y|`。
    Statistical,
    /// 数据列当作 σ，权重 `1/σ²`。
    Dataset,
    /// 数据列就是权重。
    Direct,
}

impl Weighting {
    pub fn label(self) -> &'static str {
        match self {
            Weighting::None => tr!("不加权", "No weighting"),
            Weighting::Instrumental => tr!("仪器误差 1/σ²", "Instrumental 1/σ²"),
            Weighting::Statistical => tr!("统计 1/|y|", "Statistical 1/|y|"),
            Weighting::Dataset => tr!("数据列当作误差", "Data column as errors"),
            Weighting::Direct => tr!("数据列直接作权重", "Data column as weights"),
        }
    }
}

/// 非线性拟合用哪一种算法。线性和多项式始终走加权最小二乘。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FitAlgorithm {
    #[default]
    ScaledLm,
    UnscaledLm,
    NelderMead,
}

impl FitAlgorithm {
    pub fn label(self) -> &'static str {
        match self {
            FitAlgorithm::ScaledLm => tr!("缩放 Levenberg–Marquardt", "Scaled Levenberg–Marquardt"),
            FitAlgorithm::UnscaledLm => tr!("不缩放 Levenberg–Marquardt", "Unscaled Levenberg–Marquardt"),
            FitAlgorithm::NelderMead => tr!("Nelder–Mead", "Nelder–Mead"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct FitOptions {
    pub weighting: Weighting,
    /// 与 `x`、`y` 对齐的 y 误差。仪器权重用它。
    pub errors: Vec<f64>,
    /// 与 `x`、`y` 对齐的数据列。数据集权重或直接权重用它。
    pub column: Vec<f64>,
    pub algorithm: FitAlgorithm,
    pub tolerance: f64,
    pub max_iterations: usize,
    /// 从拟合插件载入时写上插件名，结果说明里会带上。
    pub plugin_name: Option<String>,
}

impl Default for FitOptions {
    fn default() -> Self {
        Self {
            weighting: Weighting::None,
            errors: Vec::new(),
            column: Vec::new(),
            algorithm: FitAlgorithm::ScaledLm,
            tolerance: 1e-12,
            max_iterations: 80,
            plugin_name: None,
        }
    }
}

struct Solve {
    algorithm: FitAlgorithm,
    tolerance: f64,
    max_iterations: usize,
}

impl Solve {
    fn from_options(options: &FitOptions) -> Self {
        Self {
            algorithm: options.algorithm,
            tolerance: if options.tolerance.is_finite() && options.tolerance > 0.0 {
                options.tolerance
            } else {
                1e-12
            },
            max_iterations: options.max_iterations.max(1),
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
        if self.names.first().map(String::as_str) == Some(sciplot_core::t("截距")) && self.values.len() >= 2 {
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
        let mut text = sciplot_core::trf!("n = {}    R² = {}    χ² = {}（残差平方和）\n{}\n", "n = {}    R² = {}    χ² = {} (residual sum of squares)\n{}\n",
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
    fit_with(x, y, model, &FitOptions::default())
}

pub fn fit_with(x: &[f64], y: &[f64], model: &FitModel, options: &FitOptions) -> Result<FitResult, String> {
    let rows = collect_rows(x, y, options)?;
    let xs: Vec<f64> = rows.iter().map(|row| row.x).collect();
    let ys: Vec<f64> = rows.iter().map(|row| row.y).collect();
    let ws: Vec<f64> = rows.iter().map(|row| row.w).collect();
    let solve = Solve::from_options(options);
    let mut result = match model {
        FitModel::Linear => {
            let mut result = fit_polynomial_w(&xs, &ys, &ws, 1)?;
            result.names = vec![sciplot_core::t("截距").into(), sciplot_core::t("斜率").into()];
            result
        }
        FitModel::Polynomial(degree) => fit_polynomial_w(&xs, &ys, &ws, *degree)?,
        FitModel::Exponential => fit_exponential(&xs, &ys, &ws, &solve)?,
        FitModel::ExpDecay1 => fit_decay(&xs, &ys, &ws, 1, &solve)?,
        FitModel::ExpDecay2 => fit_decay(&xs, &ys, &ws, 2, &solve)?,
        FitModel::ExpDecay3 => fit_decay(&xs, &ys, &ws, 3, &solve)?,
        FitModel::ExpGrowth => fit_growth(&xs, &ys, &ws, &solve)?,
        FitModel::Gaussian => fit_peak(&xs, &ys, &ws, Peak::Gaussian, &solve)?,
        FitModel::Lorentzian => fit_peak(&xs, &ys, &ws, Peak::Lorentzian, &solve)?,
        FitModel::Boltzmann => fit_boltzmann(&xs, &ys, &ws, &solve)?,
        FitModel::MultiPeak { gaussian, centers } => {
            fit_multi(&xs, &ys, &ws, *gaussian, centers, &solve)?
        }
        FitModel::Custom { expr, initial } => fit_custom(&xs, &ys, &ws, expr, initial, &solve)?,
    };
    if let Some(name) = options.plugin_name.as_ref().filter(|name| !name.is_empty()) {
        let prefix = sciplot_core::trf!("插件「{name}」。", "Plugin \"{name}\". ", name = name);
        result.note = if result.note.is_empty() {
            prefix
        } else {
            format!("{prefix}{}", result.note)
        };
    }
    if options.weighting != Weighting::None {
        let label = options.weighting.label();
        let extra = sciplot_core::trf!("权重：{label}。", "Weighting: {label}. ", label = label);
        result.note = if result.note.is_empty() {
            extra
        } else {
            format!("{extra}{}", result.note)
        };
    }
    if model.nonlinear() && options.algorithm != FitAlgorithm::ScaledLm {
        let label = options.algorithm.label();
        let extra = sciplot_core::trf!("算法：{label}。", "Algorithm: {label}. ", label = label);
        result.note = if result.note.is_empty() {
            extra
        } else {
            format!("{extra}{}", result.note)
        };
    } else if !model.nonlinear() && options.algorithm != FitAlgorithm::ScaledLm {
        let extra = sciplot_core::tr!(
            "线性和多项式用加权最小二乘，不走所选的迭代算法。",
            "Linear and polynomial fits use weighted least squares, not the selected iterative algorithm."
        );
        result.note = if result.note.is_empty() {
            extra.to_string()
        } else {
            format!("{extra}{}", result.note)
        };
    }
    Ok(result)
}

struct Row {
    x: f64,
    y: f64,
    w: f64,
}

fn collect_rows(x: &[f64], y: &[f64], options: &FitOptions) -> Result<Vec<Row>, String> {
    let n = x.len().min(y.len());
    let mut rows = Vec::new();
    for i in 0..n {
        if !x[i].is_finite() || !y[i].is_finite() {
            continue;
        }
        let w = match options.weighting {
            Weighting::None => 1.0,
            Weighting::Statistical => {
                let magnitude = y[i].abs();
                if magnitude < 1e-15 {
                    continue;
                }
                1.0 / magnitude
            }
            Weighting::Instrumental => match positive_sigma(options.errors.get(i).copied()) {
                Some(sigma) => 1.0 / (sigma * sigma),
                None => continue,
            },
            Weighting::Dataset => match positive_sigma(options.column.get(i).copied()) {
                Some(sigma) => 1.0 / (sigma * sigma),
                None => continue,
            },
            Weighting::Direct => {
                let weight = options.column.get(i).copied().unwrap_or(f64::NAN);
                if !weight.is_finite() || weight <= 0.0 {
                    continue;
                }
                weight
            }
        };
        if w.is_finite() && w > 0.0 {
            rows.push(Row { x: x[i], y: y[i], w });
        }
    }
    if rows.len() < 2 {
        let why = match options.weighting {
            Weighting::None => sciplot_core::t("拟合至少需要两个有限数据点"),
            Weighting::Instrumental => sciplot_core::tr!(
                "仪器权重需要至少两个带有限 y 误差的点",
                "Instrumental weights need at least two points with a finite y error"
            ),
            Weighting::Statistical => sciplot_core::tr!(
                "统计权重需要至少两个 y 不为 0 的点",
                "Statistical weights need at least two points with non-zero y"
            ),
            Weighting::Dataset | Weighting::Direct => sciplot_core::tr!(
                "权重列里至少要有两个有效正数，并且和数据对齐",
                "The weight column needs at least two positive finite values aligned with the data"
            ),
        };
        return Err(why.to_string());
    }
    Ok(rows)
}

fn positive_sigma(value: Option<f64>) -> Option<f64> {
    let value = value?;
    if value.is_finite() && value.abs() >= 1e-15 {
        Some(value.abs())
    } else {
        None
    }
}

fn fit_polynomial(x: &[f64], y: &[f64], degree: usize) -> Result<FitResult, String> {
    let weights = vec![1.0; x.len()];
    fit_polynomial_w(x, y, &weights, degree)
}

fn fit_polynomial_w(x: &[f64], y: &[f64], weights: &[f64], degree: usize) -> Result<FitResult, String> {
    let n = x.len().min(y.len()).min(weights.len());
    let p = degree + 1;
    if n < p {
        return Err(sciplot_core::trf!("多项式 {degree} 阶至少需要 {p} 个点，当前只有 {n} 个", "A polynomial of degree {degree} needs at least {p} points; only {n} are available", degree = degree, p = p, n = n));
    }
    let (center, scale) = center_scale(&x[..n]);
    let mapped: Vec<f64> = x.iter().take(n).map(|xi| (xi - center) / scale).collect();
    let mut design = DMatrix::zeros(n, p);
    let mut target = DVector::zeros(n);
    for (row, &ti) in mapped.iter().enumerate() {
        let sw = weights[row].sqrt();
        let mut power = 1.0;
        for col in 0..p {
            design[(row, col)] = power * sw;
            power *= ti;
        }
        target[row] = y[row] * sw;
    }
    let normal = design.transpose() * &design;
    let rhs = design.transpose() * target;
    let solution = normal
        .clone()
        .lu()
        .solve(&rhs)
        .ok_or(sciplot_core::t("多项式拟合失败，设计矩阵秩不足"))?;
    let scaled: Vec<f64> = solution.iter().copied().collect();
    let predicted: Vec<f64> = (0..n)
        .map(|row| {
            let sw = weights[row].sqrt().max(1e-15);
            (0..p).map(|col| design[(row, col)] / sw * scaled[col]).sum()
        })
        .collect();
    let (r2, chi2) = goodness_w(&y[..n], &predicted, &weights[..n]);
    let sigma2 = if n > p { chi2 / (n - p) as f64 } else { 0.0 };
    let mut warnings = Vec::new();
    if degree > 9 {
        warnings.push(sciplot_core::t("阶数较高，系数可能不稳定"));
    }
    let stderr = match normal.clone().try_inverse() {
        Some(cov) => {
            let cond = inf_norm(&normal) * inf_norm(&cov);
            if !cond.is_finite() || cond > 1e12 {
                warnings.push(sciplot_core::t("设计矩阵病态，系数不可靠"));
            }
            transformed_stderr(&cov, sigma2, center, scale, p)
        }
        None => {
            warnings.push(sciplot_core::t("设计矩阵病态，系数不可靠"));
            vec![f64::NAN; p]
        }
    };
    let values = expand_coefficients(&scaled, center, scale);
    let names = (0..p).map(|i| format!("a{i}")).collect();
    let note = if warnings.is_empty() {
        String::new()
    } else {
        format!("{}。", warnings.join("。"))
    };
    Ok(FitResult {
        names,
        expr: polynomial_expr(&values),
        values,
        stderr,
        r2,
        chi2,
        n,
        note,
    })
}

fn center_scale(x: &[f64]) -> (f64, f64) {
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for &value in x {
        lo = lo.min(value);
        hi = hi.max(value);
    }
    let center = 0.5 * (lo + hi);
    let half = 0.5 * (hi - lo);
    let scale = if half > 1e-15 { half } else { 1.0 };
    (center, scale)
}

fn expand_coefficients(scaled: &[f64], center: f64, scale: f64) -> Vec<f64> {
    let mut values = vec![0.0; scaled.len()];
    for (k, &coeff) in scaled.iter().enumerate() {
        if coeff == 0.0 || !coeff.is_finite() {
            continue;
        }
        for (j, slot) in values.iter_mut().enumerate().take(k + 1) {
            *slot += coeff * monomial_factor(k, j, center, scale);
        }
    }
    values
}

fn transformed_stderr(cov: &DMatrix<f64>, sigma2: f64, center: f64, scale: f64, p: usize) -> Vec<f64> {
    let mut stderr = Vec::with_capacity(p);
    for j in 0..p {
        let mut var = 0.0;
        for k in j..p {
            for m in j..p {
                var += monomial_factor(k, j, center, scale) * cov[(k, m)] * monomial_factor(m, j, center, scale);
            }
        }
        stderr.push((sigma2 * var.max(0.0)).sqrt());
    }
    stderr
}

/// `(x - center)^k / scale^k` 展开后 `x^j` 的系数，`j <= k`。
fn monomial_factor(k: usize, j: usize, center: f64, scale: f64) -> f64 {
    if j > k || scale == 0.0 {
        return 0.0;
    }
    let sign = if (k - j) % 2 == 0 { 1.0 } else { -1.0 };
    binom(k, j) * sign * center.powi((k - j) as i32) / scale.powi(k as i32)
}

fn binom(k: usize, j: usize) -> f64 {
    if j > k {
        return 0.0;
    }
    let j = j.min(k - j);
    let mut value = 1.0;
    for i in 0..j {
        value *= (k - i) as f64;
        value /= (i + 1) as f64;
    }
    value
}

fn inf_norm(matrix: &DMatrix<f64>) -> f64 {
    let mut max = 0.0_f64;
    for row in 0..matrix.nrows() {
        let mut sum = 0.0;
        for col in 0..matrix.ncols() {
            sum += matrix[(row, col)].abs();
        }
        max = max.max(sum);
    }
    max
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

fn fit_exponential(x: &[f64], y: &[f64], w: &[f64], solve: &Solve) -> Result<FitResult, String> {
    let (a0, b0) = exponential_guess(x, y);
    let model = |p: &[f64], xi: f64| p[0] * (p[1] * xi).exp();
    let expr_of = |p: &[f64]| format!("{}*exp({}*x)", coeff_text(p[0]), coeff_text(p[1]));
    nonlinear(x, y, w, vec![a0, b0], &["a", "b"], model, expr_of, "y = a·exp(b·x)", solve)
}

#[derive(Clone, Copy)]
enum Peak {
    Gaussian,
    Lorentzian,
}

fn fit_peak(x: &[f64], y: &[f64], w: &[f64], kind: Peak, solve: &Solve) -> Result<FitResult, String> {
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
            nonlinear(
                x,
                y,
                w,
                vec![a0, b0, c0],
                &["a", "μ", "σ"],
                model,
                expr_of,
                sciplot_core::t("高斯 y = a·exp(-(x-μ)²/(2σ²))"),
                solve,
            )
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
            nonlinear(
                x,
                y,
                w,
                vec![a0, b0, c0],
                &["a", "x0", "γ"],
                model,
                expr_of,
                sciplot_core::t("洛伦兹 y = a·γ²/((x-x0)²+γ²)"),
                solve,
            )
        }
    }
}

fn fit_custom(
    x: &[f64],
    y: &[f64],
    w: &[f64],
    expr: &str,
    initial: &[(String, f64)],
    solve: &Solve,
) -> Result<FitResult, String> {
    let names = parameter_names(expr);
    if names.is_empty() {
        return Err(sciplot_core::t("自定义表达式需要待拟合参数，例如 a*exp(-b*x)+c").into());
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
            .zip(w.iter())
            .map(|((&xi, &yi), &wi)| {
                let mut vars = Vec::with_capacity(names.len() + 1);
                vars.push(("x", xi));
                for (name, &value) in names.iter().zip(p.iter()) {
                    vars.push((name.as_str(), value));
                }
                let residual = match compiled.eval(&vars) {
                    Ok(value) if value.is_finite() => value - yi,
                    _ => 1.0e3,
                };
                residual * wi.sqrt()
            })
            .collect()
    };
    let (values, stderr, note) = solve_fit(&p0, &residual, solve);
    let predicted: Vec<f64> = x
        .iter()
        .map(|&xi| {
            let mut vars = Vec::with_capacity(names.len() + 1);
            vars.push(("x", xi));
            for (name, &value) in names.iter().zip(values.iter()) {
                vars.push((name.as_str(), value));
            }
            compiled.eval(&vars).unwrap_or(f64::NAN)
        })
        .collect();
    let (r2, chi2) = goodness_w(y, &predicted, w);
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

#[allow(clippy::too_many_arguments)]
fn nonlinear<F, E>(
    x: &[f64],
    y: &[f64],
    w: &[f64],
    p0: Vec<f64>,
    names: &[&str],
    model: F,
    expr_of: E,
    note_prefix: &str,
    solve: &Solve,
) -> Result<FitResult, String>
where
    F: Fn(&[f64], f64) -> f64,
    E: Fn(&[f64]) -> String,
{
    let residual = |p: &[f64]| -> Vec<f64> {
        x.iter()
            .zip(y.iter())
            .zip(w.iter())
            .map(|((&xi, &yi), &wi)| {
                let value = model(p, xi);
                let residual = if value.is_finite() { value - yi } else { 1.0e3 };
                residual * wi.sqrt()
            })
            .collect()
    };
    let (values, stderr, note) = solve_fit(&p0, &residual, solve);
    let predicted: Vec<f64> = x.iter().map(|&xi| model(&values, xi)).collect();
    let (r2, chi2) = goodness_w(y, &predicted, w);
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

fn solve_fit<F: Fn(&[f64]) -> Vec<f64>>(p0: &[f64], residual: &F, solve: &Solve) -> (Vec<f64>, Vec<f64>, String) {
    if solve.algorithm == FitAlgorithm::NelderMead {
        let (p, note) = nelder_mead(p0, residual, solve.tolerance, solve.max_iterations);
        let stderr = standard_errors(&p, residual, sumsq(&residual(&p)));
        return (p, stderr, note);
    }
    levenberg(
        p0,
        residual,
        solve.algorithm == FitAlgorithm::ScaledLm,
        solve.tolerance,
        solve.max_iterations,
    )
}

fn levenberg<F: Fn(&[f64]) -> Vec<f64>>(
    p0: &[f64],
    residual: &F,
    scaled: bool,
    tolerance: f64,
    max_iterations: usize,
) -> (Vec<f64>, Vec<f64>, String) {
    let mut p = p0.to_vec();
    let mut lambda = 1e-3;
    let mut note = String::new();
    for _ in 0..max_iterations.max(1) {
        let r = residual(&p);
        if r.iter().any(|v| !v.is_finite()) {
            note = sciplot_core::t("迭代中出现非有限残差，已保留当前参数").into();
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
                let damp = if scaled {
                    lambda * system[(i, i)].abs().max(1e-8)
                } else {
                    lambda
                };
                system[(i, i)] += damp;
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
                if improvement < tolerance {
                    let stderr = standard_errors(&p, residual, cost_trial);
                    return (p, stderr, note);
                }
                break;
            }
            lambda = (lambda * 10.0).min(1e12);
        }
        if !improved {
            note = sciplot_core::t("已达到步长限制，结果可能是局部最优").into();
            break;
        }
    }
    let r = residual(&p);
    let stderr = standard_errors(&p, residual, sumsq(&r));
    (p, stderr, note)
}

fn nelder_mead<F: Fn(&[f64]) -> Vec<f64>>(
    p0: &[f64],
    residual: &F,
    tolerance: f64,
    max_iterations: usize,
) -> (Vec<f64>, String) {
    let n = p0.len();
    if n == 0 {
        return (Vec::new(), String::new());
    }
    let cost = |p: &[f64]| {
        let r = residual(p);
        if r.iter().any(|value| !value.is_finite()) {
            1.0e300
        } else {
            sumsq(&r)
        }
    };
    let mut simplex: Vec<(Vec<f64>, f64)> = Vec::with_capacity(n + 1);
    simplex.push((p0.to_vec(), cost(p0)));
    for i in 0..n {
        let mut point = p0.to_vec();
        let step = (0.05 * (1.0 + point[i].abs())).max(1e-3);
        point[i] += step;
        let value = cost(&point);
        simplex.push((point, value));
    }
    let limit = max_iterations.max(n + 1).saturating_mul(8);
    for _ in 0..limit {
        simplex.sort_by(|a, b| a.1.total_cmp(&b.1));
        let best = simplex[0].1;
        let worst = simplex[n].1;
        if (worst - best) <= tolerance * (1.0 + best.abs()) {
            break;
        }
        let mut centroid = vec![0.0; n];
        for (point, _) in simplex.iter().take(n) {
            for (slot, value) in centroid.iter_mut().zip(point) {
                *slot += *value;
            }
        }
        for slot in &mut centroid {
            *slot /= n as f64;
        }
        let worst_point = simplex[n].0.clone();
        let reflected: Vec<f64> = centroid
            .iter()
            .zip(&worst_point)
            .map(|(center, worst)| center + (center - worst))
            .collect();
        let reflected_cost = cost(&reflected);
        if reflected_cost < simplex[0].1 {
            let expanded: Vec<f64> = centroid
                .iter()
                .zip(&worst_point)
                .map(|(center, worst)| center + 2.0 * (center - worst))
                .collect();
            let expanded_cost = cost(&expanded);
            if expanded_cost < reflected_cost {
                simplex[n] = (expanded, expanded_cost);
            } else {
                simplex[n] = (reflected, reflected_cost);
            }
        } else if reflected_cost < simplex[n - 1].1 {
            simplex[n] = (reflected, reflected_cost);
        } else {
            let toward = if reflected_cost < worst { &reflected } else { &worst_point };
            let contracted: Vec<f64> = centroid
                .iter()
                .zip(toward)
                .map(|(center, other)| 0.5 * (center + other))
                .collect();
            let contracted_cost = cost(&contracted);
            if contracted_cost < worst.min(reflected_cost) {
                simplex[n] = (contracted, contracted_cost);
            } else {
                let best_point = simplex[0].0.clone();
                for (point, value) in simplex.iter_mut().skip(1) {
                    for (slot, best) in point.iter_mut().zip(&best_point) {
                        *slot = *best + 0.5 * (*slot - *best);
                    }
                    *value = cost(point);
                }
            }
        }
    }
    simplex.sort_by(|a, b| a.1.total_cmp(&b.1));
    let note = if simplex[0].1 > 1.0e200 {
        sciplot_core::t("Nelder–Mead 没有找到有限残差").into()
    } else {
        String::new()
    };
    (simplex[0].0.clone(), note)
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

fn goodness_w(y: &[f64], predicted: &[f64], weights: &[f64]) -> (f64, f64) {
    let n = y.len().min(predicted.len()).min(weights.len());
    if n == 0 {
        return (f64::NAN, f64::NAN);
    }
    let mut wsum = 0.0;
    let mut wy = 0.0;
    for i in 0..n {
        let w = weights[i];
        wsum += w;
        wy += w * y[i];
    }
    let mean = if wsum > 0.0 { wy / wsum } else { 0.0 };
    let mut ss_res = 0.0;
    let mut ss_tot = 0.0;
    for i in 0..n {
        let w = weights[i];
        ss_res += w * (y[i] - predicted[i]).powi(2);
        ss_tot += w * (y[i] - mean).powi(2);
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

fn safe_time(value: f64) -> f64 {
    if value.abs() < 1e-8 {
        1e-8_f64.copysign(if value == 0.0 { 1.0 } else { value })
    } else {
        value
    }
}

fn ends_by_x(x: &[f64], y: &[f64]) -> (f64, f64, f64, f64, f64) {
    let mut left = 0usize;
    let mut right = 0usize;
    for (index, &xi) in x.iter().enumerate() {
        if xi < x[left] {
            left = index;
        }
        if xi > x[right] {
            right = index;
        }
    }
    let span = (x[right] - x[left]).abs().max(1e-3);
    (x[left], y[left], x[right], y[right], span)
}

fn fit_boltzmann(x: &[f64], y: &[f64], w: &[f64], solve: &Solve) -> Result<FitResult, String> {
    if x.len() < 4 {
        return Err(sciplot_core::tr!("Boltzmann 拟合至少需要 4 个点", "A Boltzmann fit needs at least 4 points").into());
    }
    let (x_left, y_left, x_right, y_right, span) = ends_by_x(x, y);
    let a1 = y_left;
    let a2 = y_right;
    let mid = 0.5 * (a1 + a2);
    let mut x0 = 0.5 * (x_left + x_right);
    let mut best = f64::INFINITY;
    for i in 0..x.len() {
        let gap = (y[i] - mid).abs();
        if gap < best {
            best = gap;
            x0 = x[i];
        }
    }
    let dx = span / 10.0 * if a1 >= a2 { 1.0 } else { -1.0 };
    let model = |p: &[f64], xi: f64| {
        let dx = safe_time(p[3]);
        (p[0] - p[1]) / (1.0 + ((xi - p[2]) / dx).exp()) + p[1]
    };
    let expr_of = |p: &[f64]| {
        format!(
            "({}-({}))/(1+exp((x-({}))/({})))+({})",
            coeff_text(p[0]),
            coeff_text(p[1]),
            coeff_text(p[2]),
            coeff_text(safe_time(p[3])),
            coeff_text(p[1])
        )
    };
    let mut result = nonlinear(
        x,
        y,
        w,
        vec![a1, a2, x0, dx],
        &["A1", "A2", "x0", "dx"],
        model,
        expr_of,
        sciplot_core::t("Boltzmann y = (A1-A2)/(1+exp((x-x0)/dx))+A2"),
        solve,
    )?;
    // (A1, A2, dx) 和 (A2, A1, -dx) 是同一条曲线。统一成 dx > 0，A1 是左侧渐近值。
    if result.values.get(3).copied().unwrap_or(0.0) < 0.0 {
        result.values.swap(0, 1);
        result.values[3] = -result.values[3];
        if result.stderr.len() >= 2 {
            result.stderr.swap(0, 1);
        }
        let p = &result.values;
        result.expr = format!(
            "({}-({}))/(1+exp((x-({}))/({})))+({})",
            coeff_text(p[0]),
            coeff_text(p[1]),
            coeff_text(p[2]),
            coeff_text(safe_time(p[3])),
            coeff_text(p[1])
        );
    }
    Ok(result)
}

fn fit_decay(x: &[f64], y: &[f64], w: &[f64], order: usize, solve: &Solve) -> Result<FitResult, String> {
    let order = order.clamp(1, 3);
    let need = 1 + 2 * order;
    if x.len() < need {
        return Err(sciplot_core::trf!(
            "{order} 阶指数衰减至少需要 {need} 个点",
            "Exponential decay of order {order} needs at least {need} points",
            order = order,
            need = need
        ));
    }
    let (_x_left, y_left, _x_right, y_right, span) = ends_by_x(x, y);
    let y0 = y_right;
    let delta = y_left - y0;
    let shares = match order {
        1 => vec![1.0],
        2 => vec![0.6, 0.4],
        _ => vec![0.5, 0.3, 0.2],
    };
    let times = match order {
        1 => vec![span / 3.0],
        2 => vec![span / 8.0, span / 2.0],
        _ => vec![span / 12.0, span / 4.0, span],
    };
    let mut p0 = vec![y0];
    let mut names: Vec<&str> = vec!["y0"];
    const AMPS: [&str; 3] = ["A1", "A2", "A3"];
    const TAUS: [&str; 3] = ["t1", "t2", "t3"];
    for i in 0..order {
        let amp = if delta == 0.0 { 1.0 } else { shares[i] * delta };
        p0.push(amp);
        p0.push(times[i].max(1e-3));
        names.push(AMPS[i]);
        names.push(TAUS[i]);
    }
    let model = move |p: &[f64], xi: f64| {
        let mut value = p[0];
        for i in 0..order {
            let amp = p[1 + 2 * i];
            let tau = safe_time(p[2 + 2 * i]);
            value += amp * (-xi / tau).exp();
        }
        value
    };
    let expr_of = move |p: &[f64]| {
        let mut expr = coeff_text(p[0]);
        for i in 0..order {
            expr.push_str(&format!(
                "+{}*exp(-x/({}))",
                coeff_text(p[1 + 2 * i]),
                coeff_text(safe_time(p[2 + 2 * i]))
            ));
        }
        expr
    };
    let note = match order {
        1 => sciplot_core::t("一阶衰减 y = y0 + A1·exp(-x/t1)"),
        2 => sciplot_core::t("二阶衰减 y = y0 + A1·exp(-x/t1) + A2·exp(-x/t2)"),
        _ => sciplot_core::t("三阶衰减 y = y0 + A1·exp(-x/t1) + A2·exp(-x/t2) + A3·exp(-x/t3)"),
    };
    nonlinear(x, y, w, p0, &names, model, expr_of, note, solve)
}

fn fit_growth(x: &[f64], y: &[f64], w: &[f64], solve: &Solve) -> Result<FitResult, String> {
    if x.len() < 3 {
        return Err(sciplot_core::tr!("指数增长至少需要 3 个点", "Exponential growth needs at least 3 points").into());
    }
    let (x_left, y_left, _x_right, _y_right, span) = ends_by_x(x, y);
    let t = (span / 3.0).max(1e-3);
    let a = if y_left == 0.0 { 1.0 } else { y_left * (-x_left / t).exp() };
    let model = |p: &[f64], xi: f64| p[0] + p[1] * (xi / safe_time(p[2])).exp();
    let expr_of = |p: &[f64]| {
        format!(
            "{}+{}*exp(x/({}))",
            coeff_text(p[0]),
            coeff_text(p[1]),
            coeff_text(safe_time(p[2]))
        )
    };
    nonlinear(
        x,
        y,
        w,
        vec![0.0, a, t],
        &["y0", "A", "t"],
        model,
        expr_of,
        sciplot_core::t("指数增长 y = y0 + A·exp(x/t)"),
        solve,
    )
}

fn fit_multi(
    x: &[f64],
    y: &[f64],
    w: &[f64],
    gaussian: bool,
    centers: &[f64],
    solve: &Solve,
) -> Result<FitResult, String> {
    let centers: Vec<f64> = centers.iter().copied().filter(|value| value.is_finite()).collect();
    if centers.is_empty() || centers.len() > 8 {
        return Err(sciplot_core::tr!(
            "多峰拟合需要 1 到 8 个峰位",
            "A multi-peak fit needs 1 to 8 peak positions"
        )
        .into());
    }
    let need = 1 + 3 * centers.len();
    if x.len() < need {
        return Err(sciplot_core::trf!(
            "{n} 个峰至少需要 {need} 个点",
            "{n} peaks need at least {need} points",
            n = centers.len(),
            need = need
        ));
    }
    let xmin = x.iter().copied().fold(f64::INFINITY, f64::min);
    let xmax = x.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let span = (xmax - xmin).abs().max(1e-3);
    let width = (span / (4.0 * centers.len() as f64)).max(1e-3);
    let baseline = y.iter().copied().fold(f64::INFINITY, f64::min);
    let mut p0 = vec![baseline];
    for &center in &centers {
        let height = y_at(x, y, center) - baseline;
        p0.push(if height == 0.0 { 1.0 } else { height });
        p0.push(center);
        p0.push(width);
    }
    let npeaks = centers.len();
    let model = move |p: &[f64], xi: f64| {
        let mut value = p.first().copied().unwrap_or(0.0);
        for i in 0..npeaks {
            let amp = p.get(1 + 3 * i).copied().unwrap_or(0.0);
            let center = p.get(2 + 3 * i).copied().unwrap_or(0.0);
            let sigma = p.get(3 + 3 * i).copied().unwrap_or(width).abs().max(1e-6);
            value += if gaussian {
                amp * (-(xi - center).powi(2) / (2.0 * sigma * sigma)).exp()
            } else {
                amp * sigma * sigma / ((xi - center).powi(2) + sigma * sigma)
            };
        }
        value
    };
    let expr_of = move |p: &[f64]| {
        let mut expr = coeff_text(p.first().copied().unwrap_or(0.0));
        for i in 0..npeaks {
            let amp = coeff_text(p.get(1 + 3 * i).copied().unwrap_or(0.0));
            let center = coeff_text(p.get(2 + 3 * i).copied().unwrap_or(0.0));
            let sigma = coeff_text(p.get(3 + 3 * i).copied().unwrap_or(width).abs().max(1e-6));
            if gaussian {
                expr.push_str(&format!("+{amp}*exp(-(x-({center}))^2/(2*({sigma})^2))"));
            } else {
                expr.push_str(&format!("+{amp}*({sigma})^2/((x-({center}))^2+({sigma})^2)"));
            }
        }
        expr
    };
    let mut names = vec!["y0".to_string()];
    for i in 1..=npeaks {
        names.push(format!("a{i}"));
        names.push(format!("x{i}"));
        names.push(format!("s{i}"));
    }
    let name_refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let note = if gaussian {
        sciplot_core::t("多峰高斯 y = y0 + Σ a·exp(-(x-xc)²/(2s²))")
    } else {
        sciplot_core::t("多峰洛伦兹 y = y0 + Σ a·s²/((x-xc)²+s²)")
    };
    nonlinear(x, y, w, p0, &name_refs, model, expr_of, note, solve)
}

fn y_at(x: &[f64], y: &[f64], target: f64) -> f64 {
    let mut best = y.first().copied().unwrap_or(0.0);
    let mut gap = f64::INFINITY;
    for (xi, yi) in x.iter().zip(y.iter()) {
        let next = (xi - target).abs();
        if next < gap {
            gap = next;
            best = *yi;
        }
    }
    best
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
    fn high_degree_polynomial_keeps_the_quadratic_and_warns() {
        let x: Vec<f64> = (0..20).map(|i| i as f64).collect();
        let y: Vec<f64> = x.iter().map(|v| v * v).collect();
        let fit = fit_series(&x, &y, &FitModel::Polynomial(15)).unwrap();
        assert!((fit.values[2] - 1.0).abs() < 1e-3, "{:?}", fit.values);
        assert!(fit.values[0].abs() < 1e-2, "{:?}", fit.values);
        assert!(fit.values[1].abs() < 1e-2, "{:?}", fit.values);
        assert!(fit.note.contains("阶数") || fit.note.contains("病态"), "{}", fit.note);
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

    #[test]
    fn boltzmann_and_exponential_presets_recover_the_curve() {
        let x: Vec<f64> = (0..41).map(|i| -1.0 + i as f64 * 0.1).collect();
        let y: Vec<f64> = x
            .iter()
            .map(|v| (2.0 - 5.0) / (1.0 + ((v - 1.0) / 0.4).exp()) + 5.0)
            .collect();
        let fit = fit_series(&x, &y, &FitModel::Boltzmann).unwrap();
        assert!((fit.values[0] - 2.0).abs() < 0.05, "A1 {:?}", fit.values);
        assert!((fit.values[1] - 5.0).abs() < 0.05, "A2 {:?}", fit.values);
        assert!((fit.values[2] - 1.0).abs() < 0.05, "x0 {:?}", fit.values);
        assert!((fit.values[3] - 0.4).abs() < 0.05, "dx {:?}", fit.values);
        assert!(fit.r2 > 0.99, "{}", fit.r2);

        let x: Vec<f64> = (0..31).map(|i| i as f64 * 0.2).collect();
        let y: Vec<f64> = x.iter().map(|v| 0.5 + 3.0 * (-v / 1.5).exp()).collect();
        let fit = fit_series(&x, &y, &FitModel::ExpDecay1).unwrap();
        assert!((fit.values[0] - 0.5).abs() < 0.05, "y0 {:?}", fit.values);
        assert!((fit.values[1] - 3.0).abs() < 0.08, "A1 {:?}", fit.values);
        assert!((fit.values[2] - 1.5).abs() < 0.08, "t1 {:?}", fit.values);

        let y2: Vec<f64> = x
            .iter()
            .map(|v| 0.4 + 2.0 * (-v / 0.4).exp() + 1.0 * (-v / 1.6).exp())
            .collect();
        let fit = fit_series(&x, &y2, &FitModel::ExpDecay2).unwrap();
        assert!(fit.r2 > 0.98, "decay2 {:?} {}", fit.values, fit.r2);
        assert!((fit.values[0] - 0.4).abs() < 0.15, "y0 {:?}", fit.values);

        let y: Vec<f64> = x.iter().map(|v| 0.2 + 1.5 * (v / 2.0).exp()).collect();
        let fit = fit_series(&x, &y, &FitModel::ExpGrowth).unwrap();
        assert!(fit.r2 > 0.99, "growth {:?} {}", fit.values, fit.r2);
        assert!((fit.values[2].abs() - 2.0).abs() < 0.2, "t {:?}", fit.values);
    }

    #[test]
    fn two_gaussians_follow_the_clicked_centers() {
        let x: Vec<f64> = (0..81).map(|i| -2.0 + i as f64 * 0.05).collect();
        let y: Vec<f64> = x
            .iter()
            .map(|v| {
                0.2 + 2.0 * (-(v + 1.0).powi(2) / (2.0 * 0.25 * 0.25)).exp()
                    + 1.2 * (-(v - 1.0).powi(2) / (2.0 * 0.3 * 0.3)).exp()
            })
            .collect();
        let fit = fit_series(
            &x,
            &y,
            &FitModel::MultiPeak {
                gaussian: true,
                centers: vec![-1.0, 1.0],
            },
        )
        .unwrap();
        assert!(fit.r2 > 0.98, "{} {:?}", fit.r2, fit.values);
        let mut centers = vec![fit.values[2], fit.values[5]];
        centers.sort_by(f64::total_cmp);
        assert!((centers[0] + 1.0).abs() < 0.08, "{centers:?}");
        assert!((centers[1] - 1.0).abs() < 0.08, "{centers:?}");
    }

    #[test]
    fn weights_pull_a_line_away_from_an_outlier_and_nelder_mead_still_fits() {
        let x = [0.0, 1.0, 2.0, 3.0];
        let y = [0.0, 1.0, 2.0, 100.0];
        let options = FitOptions {
            weighting: Weighting::Direct,
            column: vec![1.0, 1.0, 1.0, 1e-8],
            ..FitOptions::default()
        };
        let fit = fit_with(&x, &y, &FitModel::Linear, &options).unwrap();
        assert!(fit.values[1] < 1.5, "slope {:?}", fit.values);
        assert!(fit.note.contains("权重"), "{}", fit.note);

        let options = FitOptions {
            algorithm: FitAlgorithm::NelderMead,
            max_iterations: 400,
            ..FitOptions::default()
        };
        let fit = fit_with(
            &x,
            &[1.0, 3.0, 5.0, 7.0],
            &FitModel::Custom {
                expr: "a*x+b".into(),
                initial: vec![("a".into(), 0.0), ("b".into(), 0.0)],
            },
            &options,
        )
        .unwrap();
        assert!((fit.values[0] - 2.0).abs() < 1e-2, "{:?}", fit.values);
        assert!((fit.values[1] - 1.0).abs() < 1e-2, "{:?}", fit.values);
        assert!(fit.note.contains("Nelder"), "{}", fit.note);
    }
}
