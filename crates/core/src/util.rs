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
/// 6 位有效数字，给图上的标注和拟合结果用。
pub fn format_short(v: f64) -> String {
    if !v.is_finite() {
        return String::new();
    }
    if v == 0.0 {
        return "0".to_string();
    }
    let negative = v < 0.0;
    let abs = v.abs();
    let exp = abs.log10().floor() as i32;
    let text = if !(-3..6).contains(&exp) {
        format!("{abs:.5e}")
    } else {
        let decimals = (5 - exp).max(0) as usize;
        format!("{abs:.decimals$}").trim_end_matches('0').trim_end_matches('.').to_string()
    };
    if negative {
        format!("-{text}")
    } else {
        text
    }
}

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

/// 固定小数位。`places` 超过 12 时按 12 位。
pub fn format_fixed(v: f64, places: u8) -> String {
    if !v.is_finite() {
        return String::new();
    }
    let places = places.min(12) as usize;
    format!("{v:.places$}")
}

/// 科学计数法，保留指定的小数位。
pub fn format_scientific(v: f64, places: u8) -> String {
    if !v.is_finite() {
        return String::new();
    }
    let places = places.min(12) as usize;
    format!("{v:.places$e}")
}

/// 解析日期，返回 Unix 纪元起的天数。接受 `YYYY-MM-DD`、`YYYY/MM/DD`、`YYYY.MM.DD`、
/// `DD.MM.YYYY`、`DD/MM/YYYY` 和 `YYYYMMDD`。带时刻的字符串不在这里解析。
pub fn parse_date(text: &str) -> Option<f64> {
    crate::datetime::parse_date_only(text)
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
        assert!(parse_date("2024-02-29").is_some());
        assert!(parse_date("2023-02-29").is_none());
        assert!(parse_date("2024-02-31").is_none());
        assert!(parse_date("2024-04-31").is_none());
    }

    #[test]
    fn unique_suffix() {
        let existing = vec![crate::locale::t("图").to_string(), crate::locale::t("图2").to_string()];
        assert_eq!(unique_name(&existing, "图"), crate::locale::t("图3"));
        assert_eq!(unique_name(&existing, "表"), crate::locale::t("表"));
    }
}
