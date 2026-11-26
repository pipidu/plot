/// 序列化时 `true` 的缺省值（`bool` 的 Default 是 false）。
pub fn default_true() -> bool {
    true
}

/// Matplotlib 风格的分类色，循环使用。
pub const PALETTE: [[u8; 4]; 10] = [
    [31, 119, 180, 255],
    [255, 127, 14, 255],
    [44, 160, 44, 255],
    [214, 39, 40, 255],
    [148, 103, 189, 255],
    [140, 86, 75, 255],
    [227, 119, 194, 255],
    [127, 127, 127, 255],
    [188, 189, 34, 255],
    [23, 190, 207, 255],
];

pub fn palette_color(index: usize) -> [u8; 4] {
    PALETTE[index % PALETTE.len()]
}

pub fn unique_name(existing: &[String], base: &str) -> String {
    if !existing.iter().any(|n| n == base) {
        return base.to_string();
    }
    for i in 2..10_000 {
        let candidate = format!("{base}{i}");
        if !existing.iter().any(|n| n == &candidate) {
            return candidate;
        }
    }
    format!("{base}-{}", existing.len() + 1)
}

/// 适合表格和刻度的数字格式：去掉无意义的尾零，极大极小用科学计数法。
pub fn format_number(v: f64) -> String {
    if !v.is_finite() {
        return String::new();
    }
    if v == 0.0 {
        return "0".to_string();
    }
    let abs = v.abs();
    if abs >= 1e6 || abs < 1e-4 {
        let s = format!("{v:.6e}");
        return trim_sci(&s);
    }
    let s = format!("{v:.8}");
    trim_plain(&s)
}

fn trim_plain(s: &str) -> String {
    if !s.contains('.') {
        return s.to_string();
    }
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn trim_sci(s: &str) -> String {
    let Some((mant, exp)) = s.split_once('e') else {
        return s.to_string();
    };
    format!("{}e{}", trim_plain(mant), exp)
}

/// 解析 `YYYY-MM-DD` 或 `YYYY/MM/DD`，返回 Unix 纪元起的天数。
pub fn parse_date(text: &str) -> Option<f64> {
    let text = text.trim();
    let parts: Vec<&str> = if text.contains('-') {
        text.split('-').collect()
    } else if text.contains('/') {
        text.split('/').collect()
    } else {
        return None;
    };
    if parts.len() != 3 {
        return None;
    }
    let year: i32 = parts[0].parse().ok()?;
    let month: u32 = parts[1].parse().ok()?;
    let day: u32 = parts[2].parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some(days_from_civil(year, month, day))
}

/// Howard Hinnant 的公历转天数算法，1970-01-01 为 0。
fn days_from_civil(year: i32, month: u32, day: u32) -> f64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u32;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era as i64 * 146097 + doe as i64 - 719468;
    days as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_and_trims() {
        assert_eq!(format_number(1.5), "1.5");
        assert_eq!(format_number(0.0), "0");
        assert!(format_number(1e7).contains('e') || format_number(1e7).contains('E'));
    }

    #[test]
    fn date_epoch() {
        assert_eq!(parse_date("1970-01-01"), Some(0.0));
        assert_eq!(parse_date("1970/01/02"), Some(1.0));
        assert!(parse_date("not-a-date").is_none());
    }

    #[test]
    fn unique_suffix() {
        let existing = vec!["图".to_string(), "图2".to_string()];
        assert_eq!(unique_name(&existing, "图"), "图3");
        assert_eq!(unique_name(&existing, "表"), "表");
    }
}
