use sciplot_core::{Axis, AxisScale};

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
            if min <= 0.0 || max <= 0.0 || !min.is_finite() || !max.is_finite() {
                return (0.1, 10.0);
            }
            let a = min.log10();
            let b = max.log10();
            let pad = ((b - a).abs() * 0.05).max(0.05);
            (10f64.powf(a - pad), 10f64.powf(b + pad))
        }
        AxisScale::Reciprocal => {
            if min == 0.0 || max == 0.0 || (min < 0.0 && max > 0.0) {
                return (0.2, 5.0);
            }
            let (mut lo, mut hi) = if (max - min).abs() < 1e-12 {
                let pad = min.abs().max(0.1) * 0.2;
                (min - pad, max + pad)
            } else {
                let pad = (max - min).abs() * 0.05;
                (min - pad, max + pad)
            };
            if min > 0.0 && lo <= 0.0 {
                lo = min * 0.5;
            }
            if max < 0.0 && hi >= 0.0 {
                hi = max * 0.5;
            }
            (lo, hi)
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

fn log_major_ticks(min: f64, max: f64, requested: i32) -> Vec<Tick> {
    if max <= min || min <= 0.0 || max <= 0.0 {
        return Vec::new();
    }
    let a = min.log10().floor() as i32;
    let b = max.log10().ceil() as i32;
    let decades = (b - a).max(1);
    let stride = if requested >= 1 {
        requested
    } else if decades > 8 {
        (decades / 6).max(1)
    } else {
        1
    };
    let mut ticks = Vec::new();
    let mut d = a;
    while d <= b {
        let value = 10f64.powi(d);
        if value >= min * 0.999 && value <= max * 1.001 {
            ticks.push(Tick { value, major: true });
        }
        d += stride;
        if ticks.len() > 200 {
            break;
        }
    }
    ticks
}

pub fn ticks_for(scale: AxisScale, min: f64, max: f64) -> Vec<Tick> {
    ticks_for_axis(
        &Axis {
            scale,
            ..Axis::default()
        },
        min,
        max,
    )
    .0
}

/// 按坐标轴上的步长生成主刻度。步长过小导致超过 200 条时退回自动刻度，并给出说明。
pub fn ticks_for_axis(axis: &Axis, min: f64, max: f64) -> (Vec<Tick>, Option<&'static str>) {
    match axis.scale {
        AxisScale::Linear => {
            if let Some(step) = axis.step {
                if step <= 0.0 || !step.is_finite() {
                    return (linear_ticks(min, max, 6), Some(sciplot_core::t("步长必须为正数，已改用自动刻度")));
                }
                let ticks = stepped_ticks(min, max, step);
                if ticks.len() > 200 {
                    return (linear_ticks(min, max, 6), Some(sciplot_core::t("步长过小，已改用自动刻度")));
                }
                return (ticks, None);
            }
            (linear_ticks(min, max, 6), None)
        }
        AxisScale::Log10 => {
            let stride = axis.step.map(|step| step.round().max(1.0) as i32).unwrap_or(0);
            let ticks = log_major_ticks(min, max, stride);
            if ticks.len() > 200 {
                return (log_major_ticks(min, max, 0), Some(sciplot_core::t("步长过小，已改用自动刻度")));
            }
            (ticks, None)
        }
        AxisScale::Reciprocal => reciprocal_ticks(axis, min, max),
    }
}

fn reciprocal_ticks(axis: &Axis, min: f64, max: f64) -> (Vec<Tick>, Option<&'static str>) {
    let scale = AxisScale::Reciprocal;
    let (Some(t0), Some(t1)) = (forward(scale, min), forward(scale, max)) else {
        return (Vec::new(), None);
    };
    let (lo, hi) = if t0 < t1 { (t0, t1) } else { (t1, t0) };
    let (mut ticks, note) = if let Some(step) = axis.step {
        if step <= 0.0 || !step.is_finite() {
            (linear_ticks(lo, hi, 6), Some(sciplot_core::t("步长必须为正数，已改用自动刻度")))
        } else {
            let stepped = stepped_ticks(lo, hi, step);
            if stepped.len() > 200 {
                (linear_ticks(lo, hi, 6), Some(sciplot_core::t("步长过小，已改用自动刻度")))
            } else {
                (stepped, None)
            }
        }
    } else {
        (linear_ticks(lo, hi, 6), None)
    };
    for tick in &mut ticks {
        if let Some(value) = inverse(scale, tick.value) {
            tick.value = value;
        }
    }
    let lo_v = min.min(max);
    let hi_v = min.max(max);
    ticks.retain(|tick| tick.value >= lo_v && tick.value <= hi_v && tick.value.is_finite());
    ticks.sort_by(|p, q| p.value.total_cmp(&q.value));
    (ticks, note)
}

fn stepped_ticks(min: f64, max: f64, step: f64) -> Vec<Tick> {
    if max <= min || step <= 0.0 || !step.is_finite() || !min.is_finite() || !max.is_finite() {
        return Vec::new();
    }
    let mut value = (min / step).ceil() * step;
    if value < min {
        value += step;
    }
    let mut ticks = Vec::new();
    for _ in 0..220 {
        if value > max + step * 1e-6 {
            break;
        }
        let snapped = (value / step).round() * step;
        if snapped >= min - step * 1e-8 && snapped <= max + step * 1e-6 {
            ticks.push(Tick {
                value: snapped,
                major: true,
            });
        }
        value += step;
    }
    ticks
}

pub fn minor_ticks_for(scale: AxisScale, majors: &[Tick], parts: usize) -> Vec<Tick> {
    let parts = parts.clamp(2, 20);
    let mut out = Vec::new();
    for pair in majors.windows(2) {
        let a = pair[0].value;
        let b = pair[1].value;
        match scale {
            AxisScale::Log10 => {
                if a <= 0.0 || b <= 0.0 {
                    continue;
                }
                let la = a.log10();
                let lb = b.log10();
                for k in 1..parts {
                    out.push(Tick {
                        value: 10f64.powf(la + (lb - la) * k as f64 / parts as f64),
                        major: false,
                    });
                }
            }
            AxisScale::Reciprocal => {
                let (Some(ta), Some(tb)) = (forward(scale, a), forward(scale, b)) else {
                    continue;
                };
                for k in 1..parts {
                    let mapped = ta + (tb - ta) * k as f64 / parts as f64;
                    if let Some(value) = inverse(scale, mapped) {
                        out.push(Tick { value, major: false });
                    }
                }
            }
            AxisScale::Linear => {
                let step = (b - a) / parts as f64;
                if step.abs() < 1e-15 {
                    continue;
                }
                for k in 1..parts {
                    out.push(Tick {
                        value: a + step * k as f64,
                        major: false,
                    });
                }
            }
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
    fn custom_linear_step_lands_on_the_requested_values() {
        let axis = Axis {
            step: Some(2.5),
            ..Axis::default()
        };
        let (ticks, note) = ticks_for_axis(&axis, 0.0, 10.0);
        assert!(note.is_none());
        let values: Vec<f64> = ticks.iter().map(|tick| tick.value).collect();
        assert_eq!(values.len(), 5);
        for (got, want) in values.iter().zip([0.0, 2.5, 5.0, 7.5, 10.0]) {
            assert!((got - want).abs() < 1e-9, "{values:?}");
        }
    }

    #[test]
    fn tiny_step_falls_back_to_automatic_ticks() {
        let axis = Axis {
            step: Some(1e-9),
            ..Axis::default()
        };
        let (ticks, note) = ticks_for_axis(&axis, 0.0, 1.0);
        assert_eq!(note, Some("步长过小，已改用自动刻度"));
        assert!(ticks.len() <= 48, "{}", ticks.len());
    }

    #[test]
    fn log_step_skips_decades() {
        let axis = Axis {
            scale: AxisScale::Log10,
            step: Some(2.0),
            ..Axis::default()
        };
        let (ticks, note) = ticks_for_axis(&axis, 1.0, 1.0e6);
        assert!(note.is_none());
        let values: Vec<f64> = ticks.into_iter().map(|tick| tick.value).collect();
        assert!(values.iter().any(|v| (*v - 1.0).abs() < 1e-9), "{values:?}");
        assert!(values.iter().any(|v| (*v - 100.0).abs() < 1e-6), "{values:?}");
        assert!(values.iter().any(|v| (*v - 10_000.0).abs() < 1e-2), "{values:?}");
        assert!(values.iter().all(|v| (*v - 10.0).abs() > 1.0), "{values:?}");
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
