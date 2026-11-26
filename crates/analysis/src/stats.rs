#[derive(Clone, Debug)]
pub struct Summary {
    pub n: usize,
    pub min: f64,
    pub max: f64,
    pub sum: f64,
    pub mean: f64,
    pub median: f64,
    pub stdev: f64,
}

impl Summary {
    pub fn report(&self, title: &str) -> String {
        format!(
            "{title}\n  个数 = {}\n  最小值 = {:.6}\n  最大值 = {:.6}\n  求和 = {:.6}\n  均值 = {:.6}\n  中位数 = {:.6}\n  标准差 = {:.6}",
            self.n, self.min, self.max, self.sum, self.mean, self.median, self.stdev
        )
    }
}

pub fn summarize(values: &[f64]) -> Option<Summary> {
    let mut data: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
    if data.is_empty() {
        return None;
    }
    data.sort_by(|a, b| a.total_cmp(b));
    let n = data.len();
    let sum: f64 = data.iter().sum();
    let mean = sum / n as f64;
    let median = if n % 2 == 1 {
        data[n / 2]
    } else {
        0.5 * (data[n / 2 - 1] + data[n / 2])
    };
    let stdev = if n > 1 {
        let var = data.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n as f64 - 1.0);
        var.sqrt()
    } else {
        0.0
    };
    Some(Summary {
        n,
        min: data[0],
        max: data[n - 1],
        sum,
        mean,
        median,
        stdev,
    })
}

/// 成对剔除非有限值后的 Pearson 相关系数。
pub fn pearson(a: &[f64], b: &[f64]) -> Result<f64, String> {
    let pairs: Vec<(f64, f64)> = a
        .iter()
        .zip(b.iter())
        .filter(|(x, y)| x.is_finite() && y.is_finite())
        .map(|(x, y)| (*x, *y))
        .collect();
    if pairs.len() < 2 {
        return Err("相关性至少需要两对有限数值".into());
    }
    let n = pairs.len() as f64;
    let mean_x = pairs.iter().map(|p| p.0).sum::<f64>() / n;
    let mean_y = pairs.iter().map(|p| p.1).sum::<f64>() / n;
    let mut sxx = 0.0;
    let mut syy = 0.0;
    let mut sxy = 0.0;
    for (x, y) in pairs {
        let dx = x - mean_x;
        let dy = y - mean_y;
        sxx += dx * dx;
        syy += dy * dy;
        sxy += dx * dy;
    }
    if sxx <= 0.0 || syy <= 0.0 {
        return Err("其中一列没有变化，相关系数无定义".into());
    }
    Ok(sxy / (sxx * syy).sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mean_and_perfect_correlation() {
        let summary = summarize(&[1.0, 2.0, 3.0, 4.0]).unwrap();
        assert!((summary.mean - 2.5).abs() < 1e-12);
        assert!((summary.median - 2.5).abs() < 1e-12);
        let r = pearson(&[1.0, 2.0, 3.0], &[2.0, 4.0, 6.0]).unwrap();
        assert!((r - 1.0).abs() < 1e-12);
    }
}
