//! 拟合、微积分、插值、平滑、FFT、统计，以及曲线求交。

mod calculus;
mod fft;
mod fit;
mod interp;
mod intersect;
mod linalg;
mod plugin;
mod signal;
mod smooth;
mod stats;

pub use calculus::{cumulative_integral, definite_integral, derivative};
pub use fft::{
    fft_filter, fft_filter_band, fft_forward, fft_forward_complex, fft_inverse, fft_spectrum, FftBand, FftSeries,
    FftShape, Spectrum,
};
pub use fit::{fit_series, fit_with, FitAlgorithm, FitModel, FitOptions, FitResult, Weighting};
pub use plugin::{load_fit_plugins, parse_fit_plugin, FitPlugin};
pub use signal::{autocorrelate, convolve, correlate, deconvolve, Correlation};
pub use interp::{interpolate, Akima, CubicSpline, InterpMethod};
pub use intersect::{intersect, CurveInput, IntersectMode, IntersectOptions, IntersectOutput, Intersection};
pub use linalg::invert_matrix;
pub use smooth::{moving_average, savitzky_golay};
pub use stats::{pearson, summarize, Summary};
