pub fn viridis(t: f32) -> [u8; 4] {
    const STOPS: [[f32; 3]; 6] = [
        [0.267, 0.005, 0.329],
        [0.282, 0.141, 0.458],
        [0.231, 0.322, 0.545],
        [0.165, 0.478, 0.558],
        [0.478, 0.722, 0.353],
        [0.993, 0.906, 0.144],
    ];
    let t = t.clamp(0.0, 1.0);
    let scaled = t * (STOPS.len() - 1) as f32;
    let index = (scaled.floor() as usize).min(STOPS.len() - 2);
    let frac = scaled - index as f32;
    let a = STOPS[index];
    let b = STOPS[index + 1];
    [
        ((a[0] + (b[0] - a[0]) * frac) * 255.0).round() as u8,
        ((a[1] + (b[1] - a[1]) * frac) * 255.0).round() as u8,
        ((a[2] + (b[2] - a[2]) * frac) * 255.0).round() as u8,
        255,
    ]
}

pub fn with_alpha(color: [u8; 4], alpha: u8) -> [u8; 4] {
    [color[0], color[1], color[2], alpha]
}
