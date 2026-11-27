/// 自然三次样条。x 不递增时先按 x 排序；重复的 x 只留第一个。少于 3 个点时原样返回。
pub fn natural_spline(points: &[(f64, f64)], per_segment: usize) -> Vec<(f64, f64)> {
    let mut unique: Vec<(f64, f64)> = Vec::new();
    let mut ordered: Vec<(f64, f64)> = points
        .iter()
        .copied()
        .filter(|(x, y)| x.is_finite() && y.is_finite())
        .collect();
    ordered.sort_by(|a, b| a.0.total_cmp(&b.0));
    for point in ordered {
        if unique.last().is_some_and(|prev: &(f64, f64)| (prev.0 - point.0).abs() < 1e-12) {
            continue;
        }
        unique.push(point);
    }
    if unique.len() < 3 {
        return unique;
    }
    let n = unique.len() - 1;
    let h: Vec<f64> = unique.windows(2).map(|w| w[1].0 - w[0].0).collect();
    if h.iter().any(|step| *step <= 0.0) {
        return unique;
    }
    let mut lower = vec![0.0; n];
    let mut diag = vec![0.0; n];
    let mut upper = vec![0.0; n];
    let mut rhs = vec![0.0; n];
    // 未知数是内部二阶导 m_1..m_{n-1}，m_0 = m_n = 0。这里的数组下标 i 对应 m_{i+1}。
    let inner = n - 1;
    if inner == 0 {
        return unique;
    }
    for i in 0..inner {
        let k = i + 1;
        lower[i] = h[k - 1];
        diag[i] = 2.0 * (h[k - 1] + h[k]);
        upper[i] = h[k];
        rhs[i] = 6.0 * ((unique[k + 1].1 - unique[k].1) / h[k] - (unique[k].1 - unique[k - 1].1) / h[k - 1]);
    }
    let second = thomas(&lower[..inner], &diag[..inner], &upper[..inner], &rhs[..inner]);
    let mut moment = vec![0.0; unique.len()];
    for (i, value) in second.into_iter().enumerate() {
        moment[i + 1] = value;
    }
    let samples = per_segment.max(1);
    let mut out = Vec::with_capacity(n * samples + 1);
    for i in 0..n {
        let x0 = unique[i].0;
        let x1 = unique[i + 1].0;
        let y0 = unique[i].1;
        let y1 = unique[i + 1].1;
        let hi = h[i];
        let m0 = moment[i];
        let m1 = moment[i + 1];
        for step in 0..samples {
            let t = step as f64 / samples as f64;
            let x = x0 + (x1 - x0) * t;
            let dx = x - x0;
            let a = y0;
            let b = (y1 - y0) / hi - hi * (2.0 * m0 + m1) / 6.0;
            let c = m0 / 2.0;
            let d = (m1 - m0) / (6.0 * hi);
            let y = a + b * dx + c * dx * dx + d * dx * dx * dx;
            out.push((x, y));
        }
    }
    out.push(*unique.last().unwrap());
    out
}

fn thomas(lower: &[f64], diag: &[f64], upper: &[f64], rhs: &[f64]) -> Vec<f64> {
    let n = diag.len();
    if n == 0 {
        return Vec::new();
    }
    let mut c_prime = vec![0.0; n];
    let mut d_prime = vec![0.0; n];
    c_prime[0] = upper[0] / diag[0];
    d_prime[0] = rhs[0] / diag[0];
    for i in 1..n {
        let denom = diag[i] - lower[i] * c_prime[i - 1];
        if denom.abs() < 1e-15 {
            return vec![0.0; n];
        }
        c_prime[i] = if i + 1 == n { 0.0 } else { upper[i] / denom };
        d_prime[i] = (rhs[i] - lower[i] * d_prime[i - 1]) / denom;
    }
    let mut x = vec![0.0; n];
    x[n - 1] = d_prime[n - 1];
    for i in (0..n - 1).rev() {
        x[i] = d_prime[i] - c_prime[i] * x[i + 1];
    }
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spline_passes_through_samples_of_a_parabola() {
        let points = [(0.0, 0.0), (1.0, 1.0), (2.0, 4.0), (3.0, 9.0)];
        let curve = natural_spline(&points, 4);
        assert!(curve.len() > points.len());
        for (x, y) in points {
            let nearest = curve
                .iter()
                .map(|point| (point.0 - x).abs())
                .fold(f64::INFINITY, f64::min);
            assert!(nearest < 1e-6, "{x}");
            let at = curve.iter().find(|point| (point.0 - x).abs() < 1e-6).unwrap();
            assert!((at.1 - y).abs() < 1e-6, "{} vs {y}", at.1);
        }
    }
}
