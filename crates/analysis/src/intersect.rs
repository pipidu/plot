//! 曲线求交。
//!
//! 分段模式把曲线看成折线，用均匀网格筛掉不可能相交的线段，再对候选线段做精确求交。
//! 折线不必按 x 单调，因此圆、回线也能求。平滑模式对函数表达式直接求根，
//! 对 x 单调的数据则先做三次样条，再用 Brent 法把变号区间收成根。
//! 结果可以用 x 区间过滤，也支持水平线 y=c 和竖直线 x=c。

use std::collections::HashMap;

use crate::interp::CubicSpline;
use sciplot_core::CompiledExpr;

#[derive(Clone, Debug)]
pub enum CurveInput {
    /// 数据折线，点按连接顺序给出，可以不按 x 排序。
    Polyline { name: String, points: Vec<(f64, f64)> },
    Horizontal { name: String, y: f64 },
    Vertical { name: String, x: f64 },
    /// `expr` 是关于 `x` 的表达式，分段模式会先在定义域内采样成折线。
    Analytic {
        name: String,
        expr: String,
        x_min: f64,
        x_max: f64,
        samples: usize,
    },
}

impl CurveInput {
    pub fn name(&self) -> &str {
        match self {
            CurveInput::Polyline { name, .. }
            | CurveInput::Horizontal { name, .. }
            | CurveInput::Vertical { name, .. }
            | CurveInput::Analytic { name, .. } => name,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntersectMode {
    /// 折线逐段求交，对采样曲线是精确的分段线性交点。
    Piecewise,
    /// 样条或表达式求根。
    Smooth,
}

#[derive(Clone, Debug)]
pub struct IntersectOptions {
    pub mode: IntersectMode,
    pub x_min: Option<f64>,
    pub x_max: Option<f64>,
    /// 去重和共线判断的绝对容差，单位与数据相同。
    pub tolerance: f64,
}

impl Default for IntersectOptions {
    fn default() -> Self {
        Self {
            mode: IntersectMode::Piecewise,
            x_min: None,
            x_max: None,
            tolerance: 1e-8,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Intersection {
    pub x: f64,
    pub y: f64,
    pub seg_a: Option<usize>,
    pub seg_b: Option<usize>,
    pub overlapping: bool,
}

#[derive(Clone, Debug)]
pub struct IntersectOutput {
    pub points: Vec<Intersection>,
    pub note: String,
}

pub fn intersect(a: &CurveInput, b: &CurveInput, options: &IntersectOptions) -> IntersectOutput {
    let tolerance = if options.tolerance.is_finite() && options.tolerance >= 0.0 {
        options.tolerance
    } else {
        1e-8
    };
    match options.mode {
        IntersectMode::Piecewise => piecewise(a, b, options, tolerance),
        IntersectMode::Smooth => smooth(a, b, options, tolerance),
    }
}

fn piecewise(a: &CurveInput, b: &CurveInput, options: &IntersectOptions, tolerance: f64) -> IntersectOutput {
    match (a, b) {
        (CurveInput::Horizontal { y: y1, .. }, CurveInput::Horizontal { y: y2, .. }) => {
            if (y1 - y2).abs() <= tolerance {
                IntersectOutput {
                    points: Vec::new(),
                    note: sciplot_core::t("两条水平线重合，交点有无穷多个").into(),
                }
            } else {
                IntersectOutput {
                    points: Vec::new(),
                    note: sciplot_core::t("两条水平线平行，没有交点").into(),
                }
            }
        }
        (CurveInput::Vertical { x: x1, .. }, CurveInput::Vertical { x: x2, .. }) => {
            if (x1 - x2).abs() <= tolerance {
                IntersectOutput {
                    points: Vec::new(),
                    note: sciplot_core::t("两条竖直线重合，交点有无穷多个").into(),
                }
            } else {
                IntersectOutput {
                    points: Vec::new(),
                    note: sciplot_core::t("两条竖直线平行，没有交点").into(),
                }
            }
        }
        (CurveInput::Horizontal { y, .. }, CurveInput::Vertical { x, .. })
        | (CurveInput::Vertical { x, .. }, CurveInput::Horizontal { y, .. }) => {
            let point = Intersection {
                x: *x,
                y: *y,
                seg_a: None,
                seg_b: None,
                overlapping: false,
            };
            finish(vec![point], options, tolerance, String::new())
        }
        _ => {
            let parts_a = materialize(a, options);
            let parts_b = materialize(b, options);
            let mut hits = Vec::new();
            match (horizontal_value(a), horizontal_value(b), vertical_value(a), vertical_value(b)) {
                (Some(y), _, _, _) => hits.extend(against_horizontal(&parts_b, y, tolerance, false)),
                (_, Some(y), _, _) => hits.extend(against_horizontal(&parts_a, y, tolerance, true)),
                (_, _, Some(x), _) => hits.extend(against_vertical(&parts_b, x, tolerance, false)),
                (_, _, _, Some(x)) => hits.extend(against_vertical(&parts_a, x, tolerance, true)),
                _ => hits.extend(polylines_intersect(&parts_a, &parts_b, tolerance)),
            }
            finish(hits, options, tolerance, String::new())
        }
    }
}

fn smooth(a: &CurveInput, b: &CurveInput, options: &IntersectOptions, tolerance: f64) -> IntersectOutput {
    if matches!(a, CurveInput::Vertical { .. }) || matches!(b, CurveInput::Vertical { .. }) {
        let mut out = piecewise(a, b, options, tolerance);
        if out.note.is_empty() {
            out.note = sciplot_core::t("竖直线没有 y=f(x) 形式，已按分段模式求交").into();
        }
        return out;
    }
    let eval_a = match as_function(a) {
        Ok(eval) => eval,
        Err(note) => return piecewise_with_note(a, b, options, tolerance, note),
    };
    let eval_b = match as_function(b) {
        Ok(eval) => eval,
        Err(note) => return piecewise_with_note(a, b, options, tolerance, note),
    };
    let domain = match overlap_domain(eval_a.domain, eval_b.domain, user_range(options)) {
        Some(domain) if domain.1 > domain.0 => domain,
        _ => {
            return IntersectOutput {
                points: Vec::new(),
                note: sciplot_core::t("两条曲线在所给 X 区间内没有重叠的定义域").into(),
            };
        }
    };
    if let (SmoothEval::Constant(y1), SmoothEval::Constant(y2)) = (&eval_a.eval, &eval_b.eval) {
        return if (y1 - y2).abs() <= tolerance {
            IntersectOutput {
                points: Vec::new(),
                note: sciplot_core::t("两条水平线重合，交点有无穷多个").into(),
            }
        } else {
            IntersectOutput {
                points: Vec::new(),
                note: sciplot_core::t("两条水平线平行，没有交点").into(),
            }
        };
    }
    let difference = |x: f64| eval_a.eval.eval(x) - eval_b.eval.eval(x);
    let roots = find_roots(&difference, domain.0, domain.1, 2400, tolerance);
    let points: Vec<Intersection> = roots
        .into_iter()
        .filter_map(|x| {
            let y = eval_a.eval.eval(x);
            if y.is_finite() {
                Some(Intersection {
                    x,
                    y,
                    seg_a: None,
                    seg_b: None,
                    overlapping: false,
                })
            } else {
                None
            }
        })
        .collect();
    let worst = points.iter().map(|point| difference(point.x).abs()).filter(|value| value.is_finite()).fold(0.0_f64, f64::max);
    let note = if points.is_empty() {
        sciplot_core::t("平滑求根").to_string()
    } else {
        sciplot_core::trf!("平滑求根，最大残差 |f−g| = {}", "Smooth roots, maximum residual |f−g| = {}", sciplot_core::format_number(worst))
    };
    finish(points, options, tolerance, note)
}

fn piecewise_with_note(a: &CurveInput, b: &CurveInput, options: &IntersectOptions, tolerance: f64, note: String) -> IntersectOutput {
    let mut out = piecewise(a, b, options, tolerance);
    out.note = sciplot_core::trf!("{note}，已改用分段线性求交", "{note}; fell back to piecewise-linear intersection", note = note);
    out
}

struct Prepared {
    domain: Option<(f64, f64)>,
    eval: SmoothEval,
}

enum SmoothEval {
    Expr(CompiledExpr),
    Spline(CubicSpline),
    Constant(f64),
}

impl SmoothEval {
    fn eval(&self, x: f64) -> f64 {
        match self {
            SmoothEval::Expr(expr) => expr.eval(&[("x", x)]).unwrap_or(f64::NAN),
            SmoothEval::Spline(spline) => spline.eval(x),
            SmoothEval::Constant(y) => *y,
        }
    }
}

fn as_function(input: &CurveInput) -> Result<Prepared, String> {
    match input {
        CurveInput::Horizontal { y, .. } => Ok(Prepared {
            domain: None,
            eval: SmoothEval::Constant(*y),
        }),
        CurveInput::Analytic { expr, x_min, x_max, .. } => {
            let (x_min, x_max) = ordered(*x_min, *x_max);
            Ok(Prepared {
                domain: Some((x_min, x_max)),
                eval: SmoothEval::Expr(CompiledExpr::compile(expr)?),
            })
        }
        CurveInput::Polyline { points, .. } => {
            let mut pts: Vec<(f64, f64)> = points
                .iter()
                .copied()
                .filter(|(x, y)| x.is_finite() && y.is_finite())
                .collect();
            if pts.len() < 2 {
                return Err(sciplot_core::t("数据点不足，无法做平滑求交").into());
            }
            let increasing = pts.windows(2).all(|w| w[1].0 > w[0].0);
            if !increasing {
                pts.sort_by(|a, b| a.0.total_cmp(&b.0));
                let strict = pts.windows(2).all(|w| w[1].0 > w[0].0 + 1e-12);
                if !strict {
                    return Err(sciplot_core::t("数据在 x 上有回折或重复，不能做样条求交").into());
                }
            }
            let xs: Vec<f64> = pts.iter().map(|p| p.0).collect();
            let ys: Vec<f64> = pts.iter().map(|p| p.1).collect();
            let spline = CubicSpline::new(&xs, &ys)?;
            Ok(Prepared {
                domain: Some((xs[0], xs[xs.len() - 1])),
                eval: SmoothEval::Spline(spline),
            })
        }
        CurveInput::Vertical { .. } => Err(sciplot_core::t("竖直线").into()),
    }
}

fn find_roots(f: &impl Fn(f64) -> f64, xmin: f64, xmax: f64, samples: usize, tolerance: f64) -> Vec<f64> {
    let samples = samples.max(16);
    let mut roots = Vec::new();
    let mut prev_x = xmin;
    let mut prev_y = f(xmin);
    if prev_y.is_finite() && prev_y.abs() <= tolerance {
        roots.push(xmin);
    }
    for i in 1..samples {
        let x = xmin + (xmax - xmin) * (i as f64) / (samples as f64 - 1.0);
        let y = f(x);
        if prev_y.is_finite() && y.is_finite() && prev_y * y < 0.0 {
            if let Some(root) = brent(prev_x, x, f, tolerance.max(1e-12)) {
                roots.push(root);
            }
        } else if y.is_finite() && y.abs() <= tolerance && prev_y.abs() > tolerance {
            roots.push(x);
        }
        prev_x = x;
        prev_y = y;
    }
    dedup_scalars(&mut roots, tolerance.max(1e-10));
    roots
}

fn dedup_scalars(values: &mut Vec<f64>, tolerance: f64) {
    values.sort_by(|a, b| a.total_cmp(b));
    values.dedup_by(|a, b| (*a - *b).abs() <= tolerance);
}

/// Brent 法。区间端点函数值必须异号。
pub fn brent(a0: f64, b0: f64, f: impl Fn(f64) -> f64, tol: f64) -> Option<f64> {
    let mut a = a0;
    let mut b = b0;
    let mut fa = f(a);
    let mut fb = f(b);
    if !fa.is_finite() || !fb.is_finite() || fa * fb > 0.0 {
        return None;
    }
    if fa.abs() < fb.abs() {
        std::mem::swap(&mut a, &mut b);
        std::mem::swap(&mut fa, &mut fb);
    }
    let mut c = a;
    let mut fc = fa;
    let mut d = b - a;
    let mut e = d;
    for _ in 0..80 {
        if fb.abs() <= tol {
            return Some(b);
        }
        if fa * fb > 0.0 {
            a = c;
            fa = fc;
            d = b - a;
            e = d;
        }
        if fa.abs() < fb.abs() {
            std::mem::swap(&mut a, &mut b);
            std::mem::swap(&mut fa, &mut fb);
            std::mem::swap(&mut c, &mut a);
            std::mem::swap(&mut fc, &mut fa);
        }
        let tol1 = 2.0 * f64::EPSILON * b.abs() + 0.5 * tol;
        let xm = 0.5 * (c - b);
        if xm.abs() <= tol1 {
            return Some(b);
        }
        let mut used_bisection = true;
        if e.abs() >= tol1 && fa.abs() > fb.abs() {
            let s = fb / fa;
            let (mut p, mut q) = if (a - c).abs() < f64::EPSILON {
                (2.0 * xm * s, 1.0 - s)
            } else {
                let q = fa / fc;
                let r = fb / fc;
                (
                    s * (2.0 * xm * q * (q - r) - (b - a) * (r - 1.0)),
                    (q - 1.0) * (r - 1.0) * (s - 1.0),
                )
            };
            if p > 0.0 {
                q = -q;
            }
            p = p.abs();
            let min1 = 3.0 * xm * q - (tol1 * q).abs();
            let min2 = (e * q).abs();
            if 2.0 * p < min1.abs().min(min2) {
                e = d;
                d = p / q;
                used_bisection = false;
            }
        }
        if used_bisection {
            d = xm;
            e = d;
        }
        a = b;
        fa = fb;
        if d.abs() > tol1 {
            b += d;
        } else {
            b += tol1.copysign(xm);
        }
        fb = f(b);
        if !fb.is_finite() {
            return None;
        }
    }
    Some(b)
}

#[derive(Clone, Copy)]
struct Seg {
    a: (f64, f64),
    b: (f64, f64),
    index: usize,
}

fn materialize(input: &CurveInput, options: &IntersectOptions) -> Vec<Vec<(f64, f64)>> {
    match input {
        CurveInput::Polyline { points, .. } => split_parts(points),
        CurveInput::Analytic {
            expr,
            x_min,
            x_max,
            samples,
            ..
        } => {
            let (mut lo, mut hi) = ordered(*x_min, *x_max);
            if let Some((u0, u1)) = user_range(options) {
                lo = lo.max(u0);
                hi = hi.min(u1);
            }
            if hi < lo {
                return Vec::new();
            }
            let compiled = match CompiledExpr::compile(expr) {
                Ok(expr) => expr,
                Err(_) => return Vec::new(),
            };
            let n = (*samples).max(2);
            let mut points = Vec::with_capacity(n);
            for i in 0..n {
                let x = if n == 1 { lo } else { lo + (hi - lo) * i as f64 / (n as f64 - 1.0) };
                match compiled.eval(&[("x", x)]) {
                    Ok(y) if y.is_finite() => points.push((x, y)),
                    _ => points.push((f64::NAN, f64::NAN)),
                }
            }
            split_parts(&points)
        }
        CurveInput::Horizontal { .. } | CurveInput::Vertical { .. } => Vec::new(),
    }
}

fn split_parts(points: &[(f64, f64)]) -> Vec<Vec<(f64, f64)>> {
    let mut parts = Vec::new();
    let mut current = Vec::new();
    for &(x, y) in points {
        if x.is_finite() && y.is_finite() {
            current.push((x, y));
        } else if current.len() >= 2 {
            parts.push(std::mem::take(&mut current));
        } else {
            current.clear();
        }
    }
    if current.len() >= 2 {
        parts.push(current);
    }
    parts
}

fn segments(parts: &[Vec<(f64, f64)>]) -> Vec<Seg> {
    let mut out = Vec::new();
    let mut index = 0;
    for part in parts {
        for pair in part.windows(2) {
            let a = pair[0];
            let b = pair[1];
            if (a.0 - b.0).hypot(a.1 - b.1) > 0.0 {
                out.push(Seg { a, b, index });
            }
            index += 1;
        }
    }
    out
}

fn polylines_intersect(a: &[Vec<(f64, f64)>], b: &[Vec<(f64, f64)>], tolerance: f64) -> Vec<Intersection> {
    let left = segments(a);
    let right = segments(b);
    if left.is_empty() || right.is_empty() {
        return Vec::new();
    }
    let mut hits = Vec::new();
    let grid = SegGrid::build(&right);
    let mut seen = vec![0u32; right.len()];
    let mut stamp = 1u32;
    for seg_a in &left {
        for candidate in grid.query(seg_a, &mut seen, &mut stamp) {
            push_hit(&mut hits, seg_a, &right[candidate], tolerance);
        }
    }
    hits
}

fn push_hit(out: &mut Vec<Intersection>, a: &Seg, b: &Seg, tolerance: f64) {
    match segment_hit(a.a, a.b, b.a, b.b, tolerance) {
        Hit::None => {}
        Hit::Point { x, y } => out.push(Intersection {
            x,
            y,
            seg_a: Some(a.index),
            seg_b: Some(b.index),
            overlapping: false,
        }),
        Hit::Overlap { p, q } => {
            out.push(Intersection {
                x: p.0,
                y: p.1,
                seg_a: Some(a.index),
                seg_b: Some(b.index),
                overlapping: true,
            });
            out.push(Intersection {
                x: q.0,
                y: q.1,
                seg_a: Some(a.index),
                seg_b: Some(b.index),
                overlapping: true,
            });
        }
    }
}

struct SegGrid {
    origin_x: f64,
    origin_y: f64,
    cell: f64,
    map: HashMap<(i32, i32), Vec<usize>>,
}

impl SegGrid {
    fn build(segs: &[Seg]) -> Self {
        let (min_x, min_y, max_x, max_y) = bounds(segs);
        let span = (max_x - min_x).hypot(max_y - min_y).max(1.0);
        let cell = (span / (segs.len() as f64).sqrt().max(1.0)).max(1e-9);
        let mut grid = Self {
            origin_x: min_x,
            origin_y: min_y,
            cell,
            map: HashMap::new(),
        };
        for (index, seg) in segs.iter().enumerate() {
            grid.insert(index, seg);
        }
        grid
    }

    fn insert(&mut self, index: usize, seg: &Seg) {
        let (x0, x1, y0, y1) = seg_box(seg);
        let (c0x, c0y) = self.cell_of(x0, y0);
        let (c1x, c1y) = self.cell_of(x1, y1);
        for ix in c0x..=c1x {
            for iy in c0y..=c1y {
                self.map.entry((ix, iy)).or_default().push(index);
            }
        }
    }

    fn query(&self, seg: &Seg, seen: &mut [u32], stamp: &mut u32) -> Vec<usize> {
        *stamp = stamp.wrapping_add(1);
        if *stamp == 0 {
            seen.fill(0);
            *stamp = 1;
        }
        let mark = *stamp;
        let (x0, x1, y0, y1) = seg_box(seg);
        let (c0x, c0y) = self.cell_of(x0, y0);
        let (c1x, c1y) = self.cell_of(x1, y1);
        let mut found = Vec::new();
        for ix in c0x..=c1x {
            for iy in c0y..=c1y {
                if let Some(bucket) = self.map.get(&(ix, iy)) {
                    for &index in bucket {
                        if seen[index] != mark {
                            seen[index] = mark;
                            found.push(index);
                        }
                    }
                }
            }
        }
        found
    }

    fn cell_of(&self, x: f64, y: f64) -> (i32, i32) {
        (
            ((x - self.origin_x) / self.cell).floor() as i32,
            ((y - self.origin_y) / self.cell).floor() as i32,
        )
    }
}

fn bounds(segs: &[Seg]) -> (f64, f64, f64, f64) {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for seg in segs {
        for p in [seg.a, seg.b] {
            min_x = min_x.min(p.0);
            min_y = min_y.min(p.1);
            max_x = max_x.max(p.0);
            max_y = max_y.max(p.1);
        }
    }
    (min_x, min_y, max_x, max_y)
}

fn seg_box(seg: &Seg) -> (f64, f64, f64, f64) {
    (seg.a.0.min(seg.b.0), seg.a.0.max(seg.b.0), seg.a.1.min(seg.b.1), seg.a.1.max(seg.b.1))
}

enum Hit {
    None,
    Point { x: f64, y: f64 },
    Overlap { p: (f64, f64), q: (f64, f64) },
}

fn segment_hit(p: (f64, f64), p2: (f64, f64), q: (f64, f64), q2: (f64, f64), tolerance: f64) -> Hit {
    let r = (p2.0 - p.0, p2.1 - p.1);
    let s = (q2.0 - q.0, q2.1 - q.1);
    let qp = (q.0 - p.0, q.1 - p.1);
    let denom = cross(r.0, r.1, s.0, s.1);
    let numer_t = cross(qp.0, qp.1, s.0, s.1);
    let numer_u = cross(qp.0, qp.1, r.0, r.1);
    let scale = (r.0.abs() + r.1.abs() + s.0.abs() + s.1.abs()).max(1.0);
    let area_tol = tolerance * scale;
    if denom.abs() > area_tol.max(1e-12) {
        let t = numer_t / denom;
        let u = numer_u / denom;
        if (-1e-9..=1.0 + 1e-9).contains(&t) && (-1e-9..=1.0 + 1e-9).contains(&u) {
            let t = t.clamp(0.0, 1.0);
            return Hit::Point {
                x: p.0 + t * r.0,
                y: p.1 + t * r.1,
            };
        }
        return Hit::None;
    }
    if numer_t.abs() > area_tol.max(1e-12) {
        return Hit::None;
    }
    let use_x = {
        let (vx, vy) = if r.0.abs() + r.1.abs() >= s.0.abs() + s.1.abs() {
            (r.0, r.1)
        } else {
            (s.0, s.1)
        };
        vx.abs() >= vy.abs()
    };
    let project = |pt: (f64, f64)| if use_x { pt.0 } else { pt.1 };
    let mut a1 = project(p);
    let mut a2 = project(p2);
    let mut b1 = project(q);
    let mut b2 = project(q2);
    if a1 > a2 {
        std::mem::swap(&mut a1, &mut a2);
    }
    if b1 > b2 {
        std::mem::swap(&mut b1, &mut b2);
    }
    let lo = a1.max(b1);
    let hi = a2.min(b2);
    if hi < lo - tolerance {
        return Hit::None;
    }
    let point_at = |value: f64| -> (f64, f64) {
        let pa = project(p);
        let pb = project(p2);
        if (pb - pa).abs() < 1e-15 {
            return p;
        }
        let t = ((value - pa) / (pb - pa)).clamp(0.0, 1.0);
        (p.0 + (p2.0 - p.0) * t, p.1 + (p2.1 - p.1) * t)
    };
    let start = point_at(lo);
    let end = point_at(hi);
    if (start.0 - end.0).hypot(start.1 - end.1) <= tolerance {
        Hit::Point { x: start.0, y: start.1 }
    } else {
        Hit::Overlap { p: start, q: end }
    }
}

fn cross(ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    ax * by - ay * bx
}

fn against_horizontal(parts: &[Vec<(f64, f64)>], y: f64, tolerance: f64, on_a: bool) -> Vec<Intersection> {
    let mut hits = Vec::new();
    for seg in segments(parts) {
        let (p, q) = (seg.a, seg.b);
        if (p.1 - y).abs() <= tolerance && (q.1 - y).abs() <= tolerance {
            let (seg_a, seg_b) = seg_ids(on_a, seg.index);
            hits.push(Intersection {
                x: p.0,
                y,
                seg_a,
                seg_b,
                overlapping: true,
            });
            hits.push(Intersection {
                x: q.0,
                y,
                seg_a,
                seg_b,
                overlapping: true,
            });
            continue;
        }
        let dy = q.1 - p.1;
        if dy.abs() < 1e-15 {
            continue;
        }
        let t = (y - p.1) / dy;
        if (-1e-9..=1.0 + 1e-9).contains(&t) {
            let t = t.clamp(0.0, 1.0);
            let (seg_a, seg_b) = seg_ids(on_a, seg.index);
            hits.push(Intersection {
                x: p.0 + t * (q.0 - p.0),
                y,
                seg_a,
                seg_b,
                overlapping: false,
            });
        }
    }
    hits
}

fn against_vertical(parts: &[Vec<(f64, f64)>], x: f64, tolerance: f64, on_a: bool) -> Vec<Intersection> {
    let mut hits = Vec::new();
    for seg in segments(parts) {
        let (p, q) = (seg.a, seg.b);
        if (p.0 - x).abs() <= tolerance && (q.0 - x).abs() <= tolerance {
            let (seg_a, seg_b) = seg_ids(on_a, seg.index);
            hits.push(Intersection {
                x,
                y: p.1,
                seg_a,
                seg_b,
                overlapping: true,
            });
            hits.push(Intersection {
                x,
                y: q.1,
                seg_a,
                seg_b,
                overlapping: true,
            });
            continue;
        }
        let dx = q.0 - p.0;
        if dx.abs() < 1e-15 {
            continue;
        }
        let t = (x - p.0) / dx;
        if (-1e-9..=1.0 + 1e-9).contains(&t) {
            let t = t.clamp(0.0, 1.0);
            let (seg_a, seg_b) = seg_ids(on_a, seg.index);
            hits.push(Intersection {
                x,
                y: p.1 + t * (q.1 - p.1),
                seg_a,
                seg_b,
                overlapping: false,
            });
        }
    }
    hits
}

fn seg_ids(segments_are_a: bool, index: usize) -> (Option<usize>, Option<usize>) {
    if segments_are_a {
        (Some(index), None)
    } else {
        (None, Some(index))
    }
}

fn horizontal_value(input: &CurveInput) -> Option<f64> {
    match input {
        CurveInput::Horizontal { y, .. } => Some(*y),
        _ => None,
    }
}

fn vertical_value(input: &CurveInput) -> Option<f64> {
    match input {
        CurveInput::Vertical { x, .. } => Some(*x),
        _ => None,
    }
}

fn finish(mut points: Vec<Intersection>, options: &IntersectOptions, tolerance: f64, note: String) -> IntersectOutput {
    if let Some((lo, hi)) = user_range(options) {
        points.retain(|point| point.x + tolerance >= lo && point.x - tolerance <= hi);
    }
    points.retain(|point| point.x.is_finite() && point.y.is_finite());
    points.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    let mut unique = Vec::new();
    for point in points {
        let duplicate = unique.iter().any(|kept: &Intersection| {
            (kept.x - point.x).hypot(kept.y - point.y) <= tolerance.max(1e-12)
        });
        if !duplicate {
            unique.push(point);
        }
    }
    IntersectOutput { points: unique, note }
}

fn user_range(options: &IntersectOptions) -> Option<(f64, f64)> {
    match (options.x_min, options.x_max) {
        (Some(a), Some(b)) => Some(ordered(a, b)),
        (Some(a), None) => Some((a, f64::INFINITY)),
        (None, Some(b)) => Some((f64::NEG_INFINITY, b)),
        (None, None) => None,
    }
}

fn overlap_domain(a: Option<(f64, f64)>, b: Option<(f64, f64)>, user: Option<(f64, f64)>) -> Option<(f64, f64)> {
    let mut domain = match (a, b) {
        (Some(a), Some(b)) => (a.0.max(b.0), a.1.min(b.1)),
        (Some(a), None) => a,
        (None, Some(b)) => b,
        (None, None) => (f64::NEG_INFINITY, f64::INFINITY),
    };
    if let Some(user) = user {
        domain.0 = domain.0.max(user.0);
        domain.1 = domain.1.min(user.1);
    }
    if domain.0.is_finite() && domain.1.is_finite() {
        Some(domain)
    } else if domain.0.is_finite() && domain.1.is_infinite() {
        Some((domain.0, domain.0 + 1.0))
    } else if domain.1.is_finite() && domain.0.is_infinite() {
        Some((domain.1 - 1.0, domain.1))
    } else {
        None
    }
}

fn ordered(a: f64, b: f64) -> (f64, f64) {
    if a <= b { (a, b) } else { (b, a) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn line(points: Vec<(f64, f64)>) -> CurveInput {
        CurveInput::Polyline { name: "L".into(), points }
    }

    #[test]
    fn crossing_lines_endpoint_parallel_and_overlap() {
        let a = line(vec![(0.0, 0.0), (2.0, 2.0)]);
        let b = line(vec![(0.0, 2.0), (2.0, 0.0)]);
        let hit = intersect(&a, &b, &IntersectOptions::default());
        assert_eq!(hit.points.len(), 1);
        assert!((hit.points[0].x - 1.0).abs() < 1e-9);
        assert!((hit.points[0].y - 1.0).abs() < 1e-9);

        let a = line(vec![(0.0, 0.0), (1.0, 1.0)]);
        let b = line(vec![(1.0, 1.0), (2.0, 0.0)]);
        let hit = intersect(&a, &b, &IntersectOptions::default());
        assert_eq!(hit.points.len(), 1);
        assert!((hit.points[0].x - 1.0).abs() < 1e-9);

        let a = line(vec![(0.0, 0.0), (1.0, 0.0)]);
        let b = line(vec![(0.0, 1.0), (1.0, 1.0)]);
        assert!(intersect(&a, &b, &IntersectOptions::default()).points.is_empty());

        let a = line(vec![(0.0, 0.0), (2.0, 0.0)]);
        let b = line(vec![(1.0, 0.0), (3.0, 0.0)]);
        let hit = intersect(&a, &b, &IntersectOptions::default());
        assert!(hit.points.len() >= 2, "{:?}", hit.points);
        assert!(hit.points.iter().any(|p| (p.x - 1.0).abs() < 1e-8));
        assert!(hit.points.iter().any(|p| (p.x - 2.0).abs() < 1e-8));
        assert!(hit.points.iter().any(|p| p.overlapping));
    }

    #[test]
    fn circle_horizontal_vertical_and_x_range() {
        let mut circle = Vec::new();
        for i in 0..=360 {
            let t = i as f64 * 2.0 * PI / 360.0;
            circle.push((t.cos(), t.sin()));
        }
        let circle = line(circle);
        let horizontal = CurveInput::Horizontal { name: "h".into(), y: 0.0 };
        let hit = intersect(&circle, &horizontal, &IntersectOptions::default());
        assert_eq!(hit.points.len(), 2, "{:?}", hit.points);
        assert!(hit.points.iter().any(|p| (p.x - 1.0).abs() < 1e-6 && p.y.abs() < 1e-6));
        assert!(hit.points.iter().any(|p| (p.x + 1.0).abs() < 1e-6 && p.y.abs() < 1e-6));

        let vertical = CurveInput::Vertical { name: "v".into(), x: 0.5 };
        let diag = line(vec![(0.0, 0.0), (2.0, 2.0)]);
        let hit = intersect(&diag, &vertical, &IntersectOptions::default());
        assert_eq!(hit.points.len(), 1);
        assert!((hit.points[0].y - 0.5).abs() < 1e-9);

        let a = line(vec![(0.0, 0.0), (2.0, 2.0)]);
        let b = line(vec![(0.0, 2.0), (2.0, 0.0)]);
        let outside = IntersectOptions {
            x_min: Some(1.5),
            x_max: Some(2.0),
            ..IntersectOptions::default()
        };
        assert!(intersect(&a, &b, &outside).points.is_empty());
        let inside = IntersectOptions {
            x_min: Some(0.0),
            x_max: Some(1.2),
            ..IntersectOptions::default()
        };
        assert_eq!(intersect(&a, &b, &inside).points.len(), 1);
    }

    #[test]
    fn smooth_sin_and_cos() {
        let sin = CurveInput::Analytic {
            name: "sin".into(),
            expr: "sin(x)".into(),
            x_min: 0.0,
            x_max: 2.0 * PI,
            samples: 800,
        };
        let cos = CurveInput::Analytic {
            name: "cos".into(),
            expr: "cos(x)".into(),
            x_min: 0.0,
            x_max: 2.0 * PI,
            samples: 800,
        };
        let smooth = IntersectOptions {
            mode: IntersectMode::Smooth,
            ..IntersectOptions::default()
        };
        let hit = intersect(&sin, &cos, &smooth);
        assert!(hit.points.len() >= 2, "{:?}", hit.points);
        assert!(hit.points.iter().any(|p| (p.x - PI / 4.0).abs() < 1e-6));
        assert!(hit.points.iter().any(|p| (p.x - 5.0 * PI / 4.0).abs() < 1e-6));

        let piecewise = IntersectOptions {
            mode: IntersectMode::Piecewise,
            ..IntersectOptions::default()
        };
        let hit = intersect(&sin, &cos, &piecewise);
        assert!(hit.points.iter().any(|p| (p.x - PI / 4.0).abs() < 5e-3));

        let level = CurveInput::Horizontal { name: "y=0.5".into(), y: 0.5 };
        let limited = IntersectOptions {
            mode: IntersectMode::Smooth,
            x_min: Some(0.0),
            x_max: Some(PI),
            ..IntersectOptions::default()
        };
        let hit = intersect(&sin, &level, &limited);
        assert!(hit.points.iter().any(|p| (p.x - PI / 6.0).abs() < 1e-5), "{:?}", hit.points);
        assert!(hit.points.iter().any(|p| (p.x - 5.0 * PI / 6.0).abs() < 1e-5), "{:?}", hit.points);
    }

    #[test]
    fn brent_finds_pi() {
        let root = brent(3.0, 4.0, |x| x.sin(), 1e-12).unwrap();
        assert!((root - PI).abs() < 1e-10);
    }
}
