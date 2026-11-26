use nalgebra::DMatrix;

pub fn moving_average(y: &[f64], window: usize) -> Result<Vec<f64>, String> {
    if window == 0 || window % 2 == 0 {
        return Err("移动平均的窗口必须是正奇数".into());
    }
    let half = window / 2;
    let mut out = vec![f64::NAN; y.len()];
    for (i, slot) in out.iter_mut().enumerate() {
        let start = i.saturating_sub(half);
        let end = (i + half).min(y.len() - 1);
        let mut acc = 0.0;
        let mut count = 0.0;
        for value in &y[start..=end] {
            if value.is_finite() {
                acc += value;
                count += 1.0;
            }
        }
        if count > 0.0 {
            *slot = acc / count;
        }
    }
    Ok(out)
}

pub fn savitzky_golay(y: &[f64], window: usize, order: usize) -> Result<Vec<f64>, String> {
    if window < 3 || window % 2 == 0 {
        return Err("Savitzky-Golay 窗口必须是不小于 3 的奇数".into());
    }
    if order >= window {
        return Err("多项式阶数必须小于窗口长度".into());
    }
    let half = (window / 2) as isize;
    let table = sg_table(half, order)?;
    let n = y.len();
    if n < window {
        return Err(format!("数据长度 {n} 小于平滑窗口 {window}"));
    }
    let mut out = vec![f64::NAN; n];
    for (i, slot) in out.iter_mut().enumerate() {
        let (start, at) = if (i as isize) < half {
            (0usize, i as isize - half)
        } else if i as isize + half >= n as isize {
            let start = n - window;
            (start, i as isize - (start as isize + half))
        } else {
            (i - half as usize, 0)
        };
        let coeff = table.get(&at).ok_or("缺少平滑系数")?;
        let mut acc = 0.0;
        let mut bad = false;
        for (k, &c) in coeff.iter().enumerate() {
            let value = y[start + k];
            if !value.is_finite() {
                bad = true;
                break;
            }
            acc += c * value;
        }
        if !bad {
            *slot = acc;
        }
    }
    Ok(out)
}

fn sg_table(half: isize, order: usize) -> Result<std::collections::BTreeMap<isize, Vec<f64>>, String> {
    let width = (2 * half + 1) as usize;
    let cols = order + 1;
    let mut design = DMatrix::zeros(width, cols);
    for row in 0..width {
        let x = (row as isize - half) as f64;
        let mut power = 1.0;
        for col in 0..cols {
            design[(row, col)] = power;
            power *= x;
        }
    }
    let normal = design.transpose() * &design;
    let inverse = normal.try_inverse().ok_or("无法计算 Savitzky-Golay 系数")?;
    let mut table = std::collections::BTreeMap::new();
    for at in -half..=half {
        let mut monomial = nalgebra::DVector::zeros(cols);
        let mut power = 1.0;
        for col in 0..cols {
            monomial[col] = power;
            power *= at as f64;
        }
        let coeff = &design * (&inverse * monomial);
        table.insert(at, coeff.iter().copied().collect());
    }
    Ok(table)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_and_line_are_unchanged() {
        let constant = vec![5.0; 9];
        let avg = moving_average(&constant, 5).unwrap();
        assert!(avg.iter().all(|v| (*v - 5.0).abs() < 1e-12));
        let sg = savitzky_golay(&constant, 5, 2).unwrap();
        assert!(sg.iter().all(|v| (*v - 5.0).abs() < 1e-8));

        let line: Vec<f64> = (0..11).map(|i| 1.0 + 2.0 * i as f64).collect();
        let sg = savitzky_golay(&line, 5, 2).unwrap();
        for (i, value) in sg.iter().enumerate() {
            assert!((value - line[i]).abs() < 1e-6, "index {i}: {value}");
        }
    }
}
