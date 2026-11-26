use nalgebra::DMatrix;
use sciplot_core::{Matrix, MatrixId};

pub fn invert_matrix(matrix: &Matrix) -> Result<Matrix, String> {
    if matrix.rows != matrix.cols {
        return Err("只有方阵可以求逆".into());
    }
    let n = matrix.rows;
    if n == 0 {
        return Err("矩阵是空的".into());
    }
    let mut data = Vec::with_capacity(n * n);
    for index in 0..n * n {
        data.push(matrix.data.get(index).copied().flatten().ok_or("矩阵含空单元格，无法求逆")?);
    }
    let mat = DMatrix::from_row_slice(n, n, &data);
    let inverse = mat.try_inverse().ok_or("矩阵奇异或病态，无法求逆")?;
    let mut out = Matrix::new(MatrixId(0), format!("{}⁻¹", matrix.name), n, n);
    out.x_min = matrix.x_min;
    out.x_max = matrix.x_max;
    out.y_min = matrix.y_min;
    out.y_max = matrix.y_max;
    for row in 0..n {
        for col in 0..n {
            out.set(row, col, Some(inverse[(row, col)]));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inverts_2x2() {
        let mut matrix = Matrix::new(MatrixId(1), "M", 2, 2);
        matrix.set(0, 0, Some(1.0));
        matrix.set(0, 1, Some(2.0));
        matrix.set(1, 0, Some(3.0));
        matrix.set(1, 1, Some(4.0));
        let inverse = invert_matrix(&matrix).unwrap();
        assert!((inverse.get(0, 0).unwrap() + 2.0).abs() < 1e-9);
        assert!((inverse.get(0, 1).unwrap() - 1.0).abs() < 1e-9);
        assert!((inverse.get(1, 0).unwrap() - 1.5).abs() < 1e-9);
        assert!((inverse.get(1, 1).unwrap() + 0.5).abs() < 1e-9);
    }
}
