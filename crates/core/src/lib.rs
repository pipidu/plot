//! SciPlot 数据模型：表格、矩阵、图形、公式与项目文件。
//! 本 crate 不依赖 GUI，方便单独测试。

mod csv_io;
mod demo;
mod formula;
mod graph;
mod history;
mod matrix;
mod project;
mod resolve;
mod table;
mod util;

pub use csv_io::{export_csv, import_csv};
pub use demo::demo_project;
pub use formula::{
    apply_formula, parameter_names, parse_initials, substitute_params, CompiledExpr, EvalColumn,
};
pub use graph::*;
pub use history::History;
pub use matrix::Matrix;
pub use project::{Project, ProjectFile, FORMAT_VERSION};
pub use resolve::{resolve_curve, sample_expr, GridData, SeriesData};
pub use table::{
    column_letter, Column, ColumnMode, ColumnType, Table, TableId,
};
pub use util::{
    default_true, format_number, palette_color, parse_date, unique_name, PALETTE,
};

/// 图形、矩阵、备注共用的新式 ID。表格 ID 单独列出以便类型区分。
macro_rules! id_ty {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
        pub struct $name(pub u64);

        impl $name {
            pub fn raw(self) -> u64 {
                self.0
            }
        }
    };
}

id_ty!(MatrixId);
id_ty!(GraphId);
id_ty!(NoteId);
