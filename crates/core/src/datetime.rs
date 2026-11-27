//! 日期、时刻和日期时间。
//!
//! 日期和日期时间都存成 Unix 纪元（1970-01-01）起的天数，小数部分是一天里的时刻。
//! 时刻列存的是从午夜起的秒数。显示格式用 `YYYY` `MM` `DD` `HH` `mm` `ss`。

/// 年在前、日在前，还是月在前。格式串决定有歧义的 `01/02/2020` 怎么读。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DateOrder {
    Ymd,
    Dmy,
    Mdy,
}

/// 日期、时刻、日期时间。和列类型对应，但不引用表格模块，避免循环依赖。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TemporalKind {
    Date,
    Time,
    DateTime,
}

pub fn default_pattern(kind: TemporalKind) -> &'static str {
    match kind {
        TemporalKind::Date => "YYYY-MM-DD",
        TemporalKind::Time => "HH:mm:ss",
        TemporalKind::DateTime => "YYYY-MM-DD HH:mm:ss",
    }
}

/// 按时间种类把用户输入变成存储用的数值。空格式时，年在最后的写法默认按日-月-年。
pub fn parse_temporal(kind: TemporalKind, text: &str, pattern: &str) -> Option<f64> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let order = order_from_pattern(pattern);
    match kind {
        TemporalKind::Date | TemporalKind::DateTime => parse_days(text, order).or_else(|| parse_plain(text)),
        TemporalKind::Time => parse_clock(text).or_else(|| parse_plain(text)),
    }
}

/// 只解析日期（不含时刻），返回整天数。给旧的 `parse_date` 和「这一列是不是日期」用。
pub fn parse_date_only(text: &str) -> Option<f64> {
    let text = text.trim();
    if text.is_empty() || split_date_time(text).1.is_some() {
        return None;
    }
    let (year, month, day) = parse_date_parts(text, DateOrder::Ymd)?;
    Some(days_from_civil(year, month, day))
}

pub fn parse_days(text: &str, order: DateOrder) -> Option<f64> {
    if let Some(days) = parse_chinese(text) {
        return Some(days);
    }
    let (date, time) = split_date_time(text);
    let (year, month, day) = parse_date_parts(date, order)?;
    let mut days = days_from_civil(year, month, day);
    if let Some(clock) = time {
        days += parse_clock(clock)? / 86_400.0;
    }
    Some(days)
}

/// `HH:MM`、`HH:MM:SS`、可带小数秒和 AM/PM。返回从午夜起的秒数。
pub fn parse_clock(text: &str) -> Option<f64> {
    let (body, afternoon) = strip_ampm(text);
    let body = body.as_str();
    let mut parts = body.split(':');
    let hour: u32 = parts.next()?.trim().parse().ok()?;
    let minute: u32 = parts.next()?.trim().parse().ok()?;
    let second: f64 = match parts.next() {
        Some(sec) => sec.trim().parse().ok()?,
        None => 0.0,
    };
    if parts.next().is_some() || minute > 59 || !(0.0..60.0).contains(&second) {
        return None;
    }
    let hour = match afternoon {
        Some(true) if hour == 12 => 12,
        Some(true) if hour < 12 => hour + 12,
        Some(false) if hour == 12 => 0,
        Some(false) if hour < 12 => hour,
        Some(_) => return None,
        None if hour <= 23 => hour,
        None => return None,
    };
    Some(hour as f64 * 3600.0 + minute as f64 * 60.0 + second)
}

pub fn format_temporal(kind: TemporalKind, value: f64, pattern: &str) -> String {
    if !value.is_finite() {
        return String::new();
    }
    let pattern = if pattern.trim().is_empty() {
        default_pattern(kind)
    } else {
        pattern.trim()
    };
    match kind {
        TemporalKind::Time => format_hms(value.rem_euclid(86_400.0), pattern),
        TemporalKind::Date | TemporalKind::DateTime => {
            let (days, seconds) = split_days(value);
            let (year, month, day) = ymd_from_days(days);
            format_calendar(pattern, year, month, day, seconds)
        }
    }
}

/// 坐标轴刻度。`span` 是轴上最大值减最小值，单位和存储一致。
pub fn format_axis_value(kind: AxisClock, pattern: &str, value: f64, span: f64) -> String {
    let pattern = if pattern.trim().is_empty() {
        auto_axis_pattern(kind, span)
    } else {
        pattern.trim()
    };
    match kind {
        AxisClock::Number => crate::util::format_number(value),
        AxisClock::Date => format_temporal(TemporalKind::Date, value, pattern),
        AxisClock::DateTime => format_temporal(TemporalKind::DateTime, value, pattern),
        AxisClock::Time => format_temporal(TemporalKind::Time, value, pattern),
    }
}

/// 和 `graph::TickKind` 对应，放在这里是为了不让日期模块去依赖图形模块。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AxisClock {
    Number,
    Date,
    DateTime,
    Time,
}

pub fn auto_axis_pattern(kind: AxisClock, span: f64) -> &'static str {
    let span = span.abs();
    match kind {
        AxisClock::Number => "",
        AxisClock::Time => {
            if span >= 60.0 {
                "HH:mm"
            } else {
                "HH:mm:ss"
            }
        }
        AxisClock::Date => {
            if span >= 365.0 {
                "YYYY"
            } else if span >= 28.0 {
                "YYYY-MM"
            } else if span >= 1.0 {
                "YYYY-MM-DD"
            } else {
                "MM-DD HH:mm"
            }
        }
        AxisClock::DateTime => {
            if span >= 365.0 {
                "YYYY"
            } else if span >= 28.0 {
                "YYYY-MM"
            } else if span >= 2.0 {
                "YYYY-MM-DD"
            } else if span >= 1.0 / 24.0 {
                "MM-DD HH:mm"
            } else {
                "HH:mm:ss"
            }
        }
    }
}

pub fn days_from_ymd(year: i32, month: u32, day: u32) -> Option<f64> {
    if !(1..=12).contains(&month) || day == 0 || day > days_in_month(year, month) {
        return None;
    }
    Some(days_from_civil(year, month, day))
}

pub fn order_from_pattern(pattern: &str) -> DateOrder {
    let pattern = pattern.trim();
    if pattern.is_empty() {
        return DateOrder::Dmy;
    }
    let mut best: Option<(usize, DateOrder)> = None;
    for (token, order) in [
        ("YYYY", DateOrder::Ymd),
        ("yyyy", DateOrder::Ymd),
        ("YY", DateOrder::Ymd),
        ("yy", DateOrder::Ymd),
        ("DD", DateOrder::Dmy),
        ("dd", DateOrder::Dmy),
        ("MM", DateOrder::Mdy),
        ("mm", DateOrder::Mdy),
    ] {
        if let Some(pos) = pattern.find(token) {
            if best.map(|(at, _)| pos < at).unwrap_or(true) {
                best = Some((pos, order));
            }
        }
    }
    best.map(|(_, order)| order).unwrap_or(DateOrder::Dmy)
}

fn parse_plain(text: &str) -> Option<f64> {
    text.parse::<f64>().ok().filter(|value| value.is_finite())
}

fn parse_chinese(text: &str) -> Option<f64> {
    let (date, clock) = text.split_once('日')?;
    let (year, rest) = date.split_once('年')?;
    let (month, day) = rest.split_once('月')?;
    let year: i32 = year.trim().parse().ok()?;
    let month: u32 = month.trim().parse().ok()?;
    let day: u32 = day.trim().parse().ok()?;
    let mut days = days_from_ymd(year, month, day)?;
    let clock = clock.trim();
    if !clock.is_empty() {
        days += parse_chinese_clock(clock)? / 86_400.0;
    }
    Some(days)
}

fn parse_chinese_clock(text: &str) -> Option<f64> {
    let (hour, rest) = text.split_once('时')?;
    let hour: u32 = hour.trim().parse().ok()?;
    let (minute, rest) = rest.split_once('分').unwrap_or((rest, ""));
    let minute: u32 = minute.trim().parse().ok()?;
    let second = if let Some((sec, tail)) = rest.split_once('秒') {
        if !tail.trim().is_empty() {
            return None;
        }
        sec.trim().parse().ok()?
    } else if rest.trim().is_empty() {
        0.0
    } else {
        return None;
    };
    if hour > 23 || minute > 59 || !(0.0..60.0).contains(&second) {
        return None;
    }
    Some(hour as f64 * 3600.0 + minute as f64 * 60.0 + second)
}

fn split_date_time(text: &str) -> (&str, Option<&str>) {
    let text = text.trim();
    if let Some((date, time)) = text.split_once(['T', 't']) {
        if time.contains(':') {
            return (date.trim(), Some(time.trim()));
        }
    }
    if let Some((date, time)) = text.rsplit_once(' ') {
        if time.contains(':') && date.contains(['-', '/', '.', '年']) {
            return (date.trim(), Some(time.trim()));
        }
    }
    (text, None)
}

fn strip_ampm(text: &str) -> (String, Option<bool>) {
    let trimmed = text.trim();
    let lower = trimmed.to_ascii_lowercase();
    let (body, afternoon) = if let Some(body) = lower.strip_suffix("a.m.") {
        (body, Some(false))
    } else if let Some(body) = lower.strip_suffix("p.m.") {
        (body, Some(true))
    } else if let Some(body) = lower.strip_suffix("am") {
        (body, Some(false))
    } else if let Some(body) = lower.strip_suffix("pm") {
        (body, Some(true))
    } else {
        return (trimmed.to_string(), None);
    };
    let body = body.trim();
    if body.is_empty() {
        (trimmed.to_string(), None)
    } else {
        (body.to_string(), afternoon)
    }
}

fn parse_date_parts(text: &str, order: DateOrder) -> Option<(i32, u32, u32)> {
    let text = text.trim();
    if text.len() == 8 && text.bytes().all(|b| b.is_ascii_digit()) {
        let year: i32 = text[0..4].parse().ok()?;
        let month: u32 = text[4..6].parse().ok()?;
        let day: u32 = text[6..8].parse().ok()?;
        days_from_ymd(year, month, day)?;
        return Some((year, month, day));
    }
    let sep = if text.contains('-') {
        '-'
    } else if text.contains('/') {
        '/'
    } else if text.contains('.') {
        '.'
    } else {
        return None;
    };
    let parts: Vec<&str> = text.split(sep).map(str::trim).filter(|s| !s.is_empty()).collect();
    if parts.len() != 3 {
        return None;
    }
    let a: i32 = parts[0].parse().ok()?;
    let b: u32 = parts[1].parse().ok()?;
    let c: i32 = parts[2].parse().ok()?;
    let (year, month, day) = if (1000..=9999).contains(&a) {
        (a, b, c as u32)
    } else if (1000..=9999).contains(&c) {
        let (month, day) = disambiguate(a as u32, b, order, sep)?;
        (c, month, day)
    } else {
        return None;
    };
    days_from_ymd(year, month, day)?;
    Some((year, month, day))
}

fn disambiguate(first: u32, second: u32, order: DateOrder, sep: char) -> Option<(u32, u32)> {
    let order = if sep == '.' && order == DateOrder::Ymd {
        DateOrder::Dmy
    } else {
        order
    };
    match (first > 12, second > 12, order) {
        (true, true, _) => None,
        (true, false, _) => Some((second, first)),
        (false, true, _) => Some((first, second)),
        (false, false, DateOrder::Mdy) => Some((first, second)),
        (false, false, _) => Some((second, first)),
    }
}

fn split_days(days: f64) -> (i64, f64) {
    let mut whole = days.floor() as i64;
    let mut seconds = (days - whole as f64) * 86_400.0;
    if seconds >= 86_399.999_5 {
        whole += 1;
        seconds = 0.0;
    }
    if seconds < 0.0 {
        seconds = 0.0;
    }
    (whole, seconds)
}

fn format_hms(seconds: f64, pattern: &str) -> String {
    format_calendar(pattern, 1970, 1, 1, seconds.rem_euclid(86_400.0))
}

fn format_calendar(pattern: &str, year: i32, month: u32, day: u32, seconds: f64) -> String {
    let seconds = seconds.rem_euclid(86_400.0);
    let total = seconds.round() as u32;
    let hour = total / 3600;
    let minute = (total % 3600) / 60;
    let second = total % 60;
    let clock = pattern_has_clock(pattern);
    let mut out = String::new();
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let rest: String = chars[i..].iter().collect();
        if let Some((token, kind)) = take_token(&rest) {
            let text = match kind {
                Token::Year4 => format!("{year:04}"),
                Token::Year2 => format!("{:02}", year.rem_euclid(100)),
                Token::Month => format!("{month:02}"),
                Token::Day => format!("{day:02}"),
                Token::Hour => format!("{hour:02}"),
                Token::Minute if clock => format!("{minute:02}"),
                Token::Minute => format!("{month:02}"),
                Token::Second => format!("{second:02}"),
            };
            out.push_str(&text);
            i += token.chars().count();
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

#[derive(Clone, Copy)]
enum Token {
    Year4,
    Year2,
    Month,
    Day,
    Hour,
    Minute,
    Second,
}

fn take_token(text: &str) -> Option<(&str, Token)> {
    const TOKENS: &[(&str, Token)] = &[
        ("YYYY", Token::Year4),
        ("yyyy", Token::Year4),
        ("YY", Token::Year2),
        ("yy", Token::Year2),
        ("MM", Token::Month),
        ("DD", Token::Day),
        ("dd", Token::Day),
        ("HH", Token::Hour),
        ("hh", Token::Hour),
        ("mm", Token::Minute),
        ("ss", Token::Second),
        ("SS", Token::Second),
    ];
    TOKENS.iter().copied().find(|(token, _)| text.starts_with(token))
}

fn pattern_has_clock(pattern: &str) -> bool {
    pattern.contains("HH") || pattern.contains("hh") || pattern.contains("ss") || pattern.contains("SS")
}

pub fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn is_leap(year: i32) -> bool {
    let divisible = |n: i32| year.rem_euclid(n) == 0;
    divisible(4) && (!divisible(100) || divisible(400))
}

/// Howard Hinnant 的公历转天数，1970-01-01 为 0。
pub fn days_from_civil(year: i32, month: u32, day: u32) -> f64 {
    let y = if month <= 2 { year - 1 } else { year } as i64;
    let era = y.div_euclid(400);
    let yoe = (y - era * 400) as u64;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy as u64;
    let days = era * 146097 + doe as i64 - 719468;
    days as f64
}

pub fn ymd_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };
    (year as i32, month as u32, day as u32)
}

/// 下一天、下一月或下一年的日序号，给坐标刻度对齐用。
pub fn add_months(days: f64, months: i32) -> f64 {
    let (year, month, _) = ymd_from_days(split_days(days).0);
    let mut m = month as i32 - 1 + months;
    let mut y = year;
    while m < 0 {
        m += 12;
        y -= 1;
    }
    y += m / 12;
    m %= 12;
    days_from_civil(y, (m + 1) as u32, 1)
}

pub fn month_start(days: f64) -> f64 {
    let (year, month, _) = ymd_from_days(split_days(days).0);
    days_from_civil(year, month, 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_and_richer_dates_roundtrip() {
        assert_eq!(parse_date_only("1970-01-01"), Some(0.0));
        assert_eq!(parse_date_only("1970/01/02"), Some(1.0));
        assert!(parse_date_only("not-a-date").is_none());
        assert!(parse_date_only("2024-02-29").is_some());
        assert!(parse_date_only("2023-02-29").is_none());
        assert!(parse_date_only("2024-02-31").is_none());
        assert!(parse_date_only("2024-04-31").is_none());

        let jan2 = parse_days("2020-01-02", DateOrder::Ymd).unwrap();
        assert_eq!(parse_date_only("2020.01.02"), Some(jan2));
        assert_eq!(parse_days("02.01.2020", DateOrder::Dmy), Some(jan2));
        assert_eq!(parse_days("02/01/2020", DateOrder::Dmy), Some(jan2));
        assert_eq!(parse_days("01/02/2020", DateOrder::Mdy), Some(jan2));
        assert_eq!(parse_days("20200102", DateOrder::Ymd), Some(jan2));
        assert_eq!(parse_days("2020年1月2日", DateOrder::Ymd), Some(jan2));
        assert_eq!(ymd_from_days(jan2 as i64), (2020, 1, 2));
        assert_eq!(format_temporal(TemporalKind::Date, jan2, "YYYY/MM/DD"), "2020/01/02");
        assert_eq!(format_temporal(TemporalKind::Date, jan2, ""), "2020-01-02");
    }

    #[test]
    fn time_of_day_and_datetime() {
        assert_eq!(parse_clock("13:45"), Some(13.0 * 3600.0 + 45.0 * 60.0));
        assert_eq!(parse_clock("1:05:06 PM"), Some(13.0 * 3600.0 + 5.0 * 60.0 + 6.0));
        assert_eq!(parse_clock("12:00 AM"), Some(0.0));
        assert!(parse_clock("24:00").is_none());
        let noon = parse_days("1970-01-01 12:00:00", DateOrder::Ymd).unwrap();
        assert!((noon - 0.5).abs() < 1e-9, "{noon}");
        let typed = parse_temporal(TemporalKind::DateTime, "1970-01-02T00:30:00", "").unwrap();
        assert!((typed - 1.0 - 30.0 / 1440.0).abs() < 1e-9, "{typed}");
        assert_eq!(
            format_temporal(TemporalKind::DateTime, typed, "YYYY-MM-DD HH:mm"),
            "1970-01-02 00:30"
        );
        assert_eq!(format_temporal(TemporalKind::Time, 13.0 * 3600.0 + 45.0 * 60.0, ""), "13:45:00");
        assert_eq!(format_temporal(TemporalKind::Time, 3661.0, "HH:mm"), "01:01");
    }

    #[test]
    fn axis_pattern_shrinks_with_the_span() {
        assert_eq!(auto_axis_pattern(AxisClock::Date, 400.0), "YYYY");
        assert_eq!(auto_axis_pattern(AxisClock::Date, 40.0), "YYYY-MM");
        assert_eq!(auto_axis_pattern(AxisClock::DateTime, 0.2), "MM-DD HH:mm");
        assert_eq!(auto_axis_pattern(AxisClock::Time, 30.0), "HH:mm:ss");
        let label = format_axis_value(AxisClock::Date, "", 0.0, 10.0);
        assert_eq!(label, "1970-01-01");
    }
}
