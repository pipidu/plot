use sciplot_core::AxisScale;

#[derive(Clone, Copy, Debug)]
pub struct Tick {
    pub value: f64,
    pub major: bool,
}

pub fn forward(scale: AxisScale, value: f64) -> Option<f64> {
    match scale {
        AxisScale::Linear => value.is_finite().then_some(value),
        AxisScale::Log10 => (value > 0.0 && value.is_finite()).then(|| value.log10()),
        AxisScale::Reciprocal => (value.is_finite() && value.abs() > 1e-12).then(|| 1.0 / value),
    }
}

pub fn inverse(scale: AxisScale, mapped: f64) -> Option<f64> {
    match scale {
        AxisScale::Linear => mapped.is_finite().then_some(mapped),
        AxisScale::Log10 => mapped.is_finite().then(|| 10f64.powf(mapped)),
        AxisScale::Reciprocal => (mapped.is_finite() && mapped.abs() > 1e-12).then(|| 1.0 / mapped),
    }
}

pub fn unit(scale: AxisScale, value: f64, min: f64, max: f64, inverted: bool) -> Option<f32> {
    let t0 = forward(scale, min)?;
    let t1 = forward(scale, max)?;
    let t = forward(scale, value)?;
    let span = t1 - t0;
    if span.abs() < 1e-15 {
        return Some(0.5);
    }
    let mut u = ((t - t0) / span) as f32;
    if inverted {
        u = 1.0 - u;
    }
    Some(u)
}

pub fn pad_range(scale: AxisScale, min: f64, max: f64) -> (f64, f64) {
    if !min.is_finite() || !max.is_finite() {
        return match scale {
            AxisScale::Log10 => (0.1, 10.0),
            AxisScale::Reciprocal => (0.2, 5.0),
            AxisScale::Linear => (0.0, 1.0),
        };
    }
    match scale {
        AxisScale::Linear => {
            if (max - min).abs() < 1e-12 {
                let pad = min.abs().max(1.0) * 0.5;
                (min - pad, max + pad)
            } else {
                let pad = (max - min) * 0.05;
                (min - pad, max + pad)
            }
        }
        AxisScale::Log10 => {
            let a = min.max(1e-300).log10();
            let b = max.max(1e-300).log10();
            let pad = ((b - a).abs() * 0.05).max(0.05);
            (10f64.powf(a - pad), 10f64.powf(b + pad))
        }
        AxisScale::Reciprocal => {
            if (max - min).abs() < 1e-12 {
                let pad = min.abs().max(0.1) * 0.2;
                (min - pad, max + pad)
            } else {
                let pad = (max - min).abs() * 0.05;
                (min - pad, max + pad)
            }
        }
    }
}

pub fn linear_ticks(min: f64, max: f64, target: usize) -> Vec<Tick> {
    if max <= min || !min.is_finite() || !max.is_finite() {
        return Vec::new();
    }
    let span = max - min;
    let raw = span / target.max(2) as f64;
    if raw <= 0.0 || !raw.is_finite() {
        return Vec::new();
    }
    let exp = raw.log10().floor();
    let base = 10f64.powf(exp);
    let frac = raw / base;
    let step = if frac <= 1.0 {
        base
    } else if frac <= 2.0 {
        2.0 * base
    } else if frac <= 5.0 {
        5.0 * base
    } else {
        10.0 * base
    };
    if step <= 0.0 || !step.is_finite() {
        return Vec::new();
    }
    let mut value = (min / step).ceil() * step;
    if value < min {
        value += step;
    }
    let mut ticks = Vec::new();
    for _ in 0..48 {
        if value > max + step * 1e-6 {
            break;
        }
        if value >= min - step * 1e-8 {
            let snapped = (value / step).round() * step;
            ticks.push(Tick {
                value: snapped,
                major: true,
            });
        }
        value += step;
    }
    ticks
}

pub fn log_ticks(min: f64, max: f64) -> Vec<Tick> {
    if max <= min || min <= 0.0 || max <= 0.0 {
        return Vec::new();
    }
    let a = min.log10().floor() as i32;
    let b = max.log10().ceil() as i32;
    let decades = (b - a).max(1);
    let stride = if decades > 8 { (decades / 6).max(1) } else { 1 };
    let mut ticks = Vec::new();
    let mut d = a;
    while d <= b {
        let value = 10f64.powi(d);
        if value >= min * 0.999 && value <= max * 1.001 {
            ticks.push(Tick { value, major: true });
        }
        if stride == 1 && decades <= 6 {
            for m in 2..10 {
                let minor = value * m as f64;
                if minor > min && minor < max && minor < 10f64.powi(d + 1) {
                    ticks.push(Tick {
                        value: minor,
                        major: false,
                    });
                }
            }
        }
        d += stride;
    }
    ticks.sort_by(|p, q| p.value.total_cmp(&q.value));
    ticks
}

pub fn ticks_for(scale: AxisScale, min: f64, max: f64) -> Vec<Tick> {
    match scale {
        AxisScale::Linear => linear_ticks(min, max, 6),
        AxisScale::Log10 => log_ticks(min, max),
        AxisScale::Reciprocal => {
            let Some(t0) = forward(scale, min) else {
                return Vec::new();
            };
            let Some(t1) = forward(scale, max) else {
                return Vec::new();
            };
            let (lo, hi) = if t0 < t1 { (t0, t1) } else { (t1, t0) };
            let mut ticks = linear_ticks(lo, hi, 6);
            for tick in &mut ticks {
                if let Some(value) = inverse(scale, tick.value) {
                    tick.value = value;
                }
            }
            ticks.retain(|tick| tick.value >= min.min(max) && tick.value <= min.max(max));
            ticks.sort_by(|p, q| p.value.total_cmp(&q.value));
            ticks
        }
    }
}

pub fn minor_ticks(majors: &[Tick]) -> Vec<Tick> {
    let mut out = Vec::new();
    for pair in majors.windows(2) {
        let step = (pair[1].value - pair[0].value) / 4.0;
        if step.abs() < 1e-15 {
            continue;
        }
        for k in 1..4 {
            out.push(Tick {
                value: pair[0].value + step * k as f64,
                major: false,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_ticks_cover_a_simple_range() {
        let ticks = linear_ticks(0.0, 2.0, 5);
        assert!(ticks.iter().any(|tick| (tick.value - 0.0).abs() < 1e-9));
        assert!(ticks.iter().any(|tick| (tick.value - 1.0).abs() < 1e-9));
        assert!(ticks.iter().any(|tick| (tick.value - 2.0).abs() < 1e-9));
    }

    #[test]
    fn log_round_trips() {
        let mapped = forward(AxisScale::Log10, 100.0).unwrap();
        assert!((mapped - 2.0).abs() < 1e-12);
        let back = inverse(AxisScale::Log10, mapped).unwrap();
        assert!((back - 100.0).abs() < 1e-9);
        let u = unit(AxisScale::Log10, 10.0, 1.0, 100.0, false).unwrap();
        assert!((u - 0.5).abs() < 1e-5);
    }
}
