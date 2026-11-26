use serde::{Deserialize, Serialize};

use crate::formula::CompiledExpr;
use crate::MatrixId;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Matrix {
    pub id: MatrixId,
    pub name: String,
    pub rows: usize,
    pub cols: usize,
    /// 行优先。`None` 表示空单元格。
    pub data: Vec<Option<f64>>,
    pub x_min: f64,
    pub x_max: f64,
    pub y_min: f64,
    pub y_max: f64,
    pub formula: String,
    pub comment: String,
}

impl Default for Matrix {
    fn default() -> Self {
        Self {
            id: MatrixId(0),
            name: "矩阵".to_string(),
            rows: 1,
            cols: 1,
            data: vec![None],
            x_min: 0.0,
            x_max: 1.0,
            y_min: 0.0,
            y_max: 1.0,
            formula: String::new(),
            comment: String::new(),
        }
    }
}

impl Matrix {
    pub fn new(id: MatrixId, name: impl Into<String>, rows: usize, cols: usize) -> Self {
        let rows = rows.max(1);
        let cols = cols.max(1);
        Self {
            id,
            name: name.into(),
            rows,
            cols,
            data: vec![None; rows * cols],
            ..Self::default()
        }
    }

    pub fn index(&self, row: usize, col: usize) -> usize {
        row * self.cols + col
    }

    pub fn get(&self, row: usize, col: usize) -> Option<f64> {
        self.data.get(self.index(row, col)).copied().flatten()
    }

    pub fn set(&mut self, row: usize, col: usize, value: Option<f64>) {
        if row < self.rows && col < self.cols {
            let i = self.index(row, col);
            self.data[i] = value.filter(|v| v.is_finite());
        }
    }

    pub fn resize(&mut self, rows: usize, cols: usize) {
        let rows = rows.max(1);
        let cols = cols.max(1);
        let mut next = vec![None; rows * cols];
        for r in 0..rows.min(self.rows) {
            for c in 0..cols.min(self.cols) {
                next[r * cols + c] = self.get(r, c);
            }
        }
        self.rows = rows;
        self.cols = cols;
        self.data = next;
    }

    /// x、y 取第一格到最后一格的中心坐标。
    pub fn xy_at(&self, row: usize, col: usize) -> (f64, f64) {
        (coord(self.x_min, self.x_max, col, self.cols), coord(self.y_min, self.y_max, row, self.rows))
    }

    /// 用含 `x`、`y` 的公式填充每个单元格。
    pub fn fill_formula(&mut self, expr: &str) -> Result<(), String> {
        let compiled = CompiledExpr::compile(expr)?;
        for r in 0..self.rows {
            for c in 0..self.cols {
                let (x, y) = self.xy_at(r, c);
                match compiled.eval(&[("x", x), ("y", y)]) {
                    Ok(v) if v.is_finite() => self.set(r, c, Some(v)),
                    Ok(_) => self.set(r, c, None),
                    Err(err) => {
                        return Err(format!("第 {} 行第 {} 列：{err}", r + 1, c + 1));
                    }
                }
            }
        }
        self.formula = expr.to_string();
        Ok(())
    }

    pub fn transpose(&self) -> Self {
        let mut out = Self::new(self.id, format!("{}ᵀ", self.name), self.cols, self.rows);
        out.x_min = self.y_min;
        out.x_max = self.y_max;
        out.y_min = self.x_min;
        out.y_max = self.x_max;
        out.formula = self.formula.clone();
        for r in 0..self.rows {
            for c in 0..self.cols {
                out.set(c, r, self.get(r, c));
            }
        }
        out
    }
}

pub fn coord(min: f64, max: f64, index: usize, count: usize) -> f64 {
    if count <= 1 {
        return (min + max) * 0.5;
    }
    min + (max - min) * (index as f64) / (count as f64 - 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formula_and_transpose() {
        let mut m = Matrix::new(MatrixId(1), "M", 2, 3);
        m.x_min = 0.0;
        m.x_max = 2.0;
        m.y_min = 0.0;
        m.y_max = 1.0;
        m.fill_formula("x + 10 * y").unwrap();
        assert_eq!(m.get(0, 0), Some(0.0));
        assert_eq!(m.get(0, 2), Some(2.0));
        assert_eq!(m.get(1, 1), Some(11.0));
        let t = m.transpose();
        assert_eq!((t.rows, t.cols), (3, 2));
        assert_eq!(t.get(2, 0), Some(2.0));
        assert_eq!(t.get(1, 1), Some(11.0));
    }
}
