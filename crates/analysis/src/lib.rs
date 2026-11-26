//! 拟合、微积分、插值、平滑、FFT、统计，以及曲线求交。

mod calculus;
mod fft;
mod fit;
mod interp;
mod intersect;
mod linalg;
mod smooth;
mod stats;

pub use calculus::{cumulative_integral, definite_integral, derivative};
pub use fft::{fft_filter, fft_spectrum, Spectrum};
pub use fit::{fit_series, FitModel, FitResult};
pub use interp::{interpolate, Akima, CubicSpline, InterpMethod};
pub use intersect::{intersect, CurveInput, IntersectMode, IntersectOptions, IntersectOutput, Intersection};
pub use linalg::invert_matrix;
pub use smooth::{moving_average, savitzky_golay};
pub use stats::{pearson, summarize, Summary};
