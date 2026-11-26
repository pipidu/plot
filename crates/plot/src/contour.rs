use sciplot_core::GridData;

#[derive(Clone, Copy)]
enum Edge {
    Bottom,
    Right,
    Top,
    Left,
}

/// 在网格中心点之间做 marching squares，返回数据坐标下的线段。
pub fn contour_segments(grid: &GridData, level: f64) -> Vec<((f64, f64), (f64, f64))> {
    if grid.rows < 2 || grid.cols < 2 {
        return Vec::new();
    }
    let mut segments = Vec::new();
    for row in 0..grid.rows - 1 {
        for col in 0..grid.cols - 1 {
            let v = [
                grid.at(row, col),
                grid.at(row, col + 1),
                grid.at(row + 1, col + 1),
                grid.at(row + 1, col),
            ];
            if v.iter().any(|value| !value.is_finite()) {
                continue;
            }
            let p = [
                xy(grid, row, col),
                xy(grid, row, col + 1),
                xy(grid, row + 1, col + 1),
                xy(grid, row + 1, col),
            ];
            let mut bits = 0u8;
            if v[0] >= level {
                bits |= 1;
            }
            if v[1] >= level {
                bits |= 2;
            }
            if v[2] >= level {
                bits |= 4;
            }
            if v[3] >= level {
                bits |= 8;
            }
            let center = (v[0] + v[1] + v[2] + v[3]) * 0.25;
            let edges = match bits {
                1 | 14 => &[(Edge::Bottom, Edge::Left)][..],
                2 | 13 => &[(Edge::Bottom, Edge::Right)][..],
                3 | 12 => &[(Edge::Left, Edge::Right)][..],
                4 | 11 => &[(Edge::Right, Edge::Top)][..],
                6 | 9 => &[(Edge::Bottom, Edge::Top)][..],
                7 | 8 => &[(Edge::Top, Edge::Left)][..],
                5 => {
                    if center >= level {
                        &[(Edge::Bottom, Edge::Right), (Edge::Top, Edge::Left)][..]
                    } else {
                        &[(Edge::Bottom, Edge::Left), (Edge::Right, Edge::Top)][..]
                    }
                }
                10 => {
                    if center >= level {
                        &[(Edge::Bottom, Edge::Left), (Edge::Right, Edge::Top)][..]
                    } else {
                        &[(Edge::Bottom, Edge::Right), (Edge::Top, Edge::Left)][..]
                    }
                }
                _ => &[][..],
            };
            for (a, b) in edges {
                segments.push((edge_point(*a, &p, &v, level), edge_point(*b, &p, &v, level)));
            }
        }
    }
    segments
}

fn xy(grid: &GridData, row: usize, col: usize) -> (f64, f64) {
    (
        axis_at(grid.x0, grid.x1, col, grid.cols),
        axis_at(grid.y0, grid.y1, row, grid.rows),
    )
}

fn axis_at(min: f64, max: f64, index: usize, count: usize) -> f64 {
    if count <= 1 {
        return (min + max) * 0.5;
    }
    min + (max - min) * (index as f64) / (count as f64 - 1.0)
}

fn edge_point(edge: Edge, p: &[(f64, f64); 4], v: &[f64; 4], level: f64) -> (f64, f64) {
    let (i, j) = match edge {
        Edge::Bottom => (0, 1),
        Edge::Right => (1, 2),
        Edge::Top => (3, 2),
        Edge::Left => (0, 3),
    };
    lerp(p[i], p[j], v[i], v[j], level)
}

fn lerp(a: (f64, f64), b: (f64, f64), va: f64, vb: f64, level: f64) -> (f64, f64) {
    let denom = vb - va;
    let t = if denom.abs() < 1e-15 {
        0.5
    } else {
        ((level - va) / denom).clamp(0.0, 1.0)
    };
    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plane_level_is_a_vertical_line() {
        let cols = 11;
        let rows = 5;
        let mut z = Vec::new();
        for _row in 0..rows {
            for col in 0..cols {
                let x = col as f64 / (cols as f64 - 1.0);
                z.push(x);
            }
        }
        let grid = GridData {
            rows,
            cols,
            x0: 0.0,
            x1: 1.0,
            y0: 0.0,
            y1: 1.0,
            z,
        };
        let segments = contour_segments(&grid, 0.5);
        assert!(segments.len() >= rows - 1);
        for ((x1, _), (x2, _)) in segments {
            assert!((x1 - 0.5).abs() < 1e-6, "x1={x1}");
            assert!((x2 - 0.5).abs() < 1e-6, "x2={x2}");
        }
    }
}
