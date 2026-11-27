//! SciPlot 数据模型：表格、矩阵、图形、公式与项目文件。
//! 本 crate 不依赖 GUI，方便单独测试。

mod branch;
mod catalog;
mod expr;
mod csv_io;
mod datetime;
mod demo;
mod exchange;
mod formula;
mod graph;
mod history;
mod lang_extra;
mod lang_formats;
mod locale;
mod matrix;
mod organize;
mod official;
mod origin;
mod project;
mod resolve;
mod surface_data;
mod table;
mod template;
mod util;

pub use csv_io::{
    export_csv, export_csv_with, import_csv, import_csv_files, import_csv_options, import_csv_with, CsvDelimiter,
    CsvEncoding, CsvHeader, CsvImportOptions, CsvLayout, DecimalMark,
};
pub use demo::demo_project;
pub use exchange::{detach_save_path, load_project_bytes, save_project_bytes, save_project_path, ProjectFormat};
pub use formula::{
    apply_formula, apply_formula_bar, apply_formula_with, commit_cell, eval_column, eval_column_with, eval_note, explain_error,
    is_python_formula, parameter_names, parse_initials, recalculate_table, set_python_column_hook, substitute_params,
    CompiledExpr, EvalColumn, NoteLine, NoteRun, PythonColumnFn, RecalcReport,
};
pub use graph::*;
pub use history::History;
pub use locale::{fill_trf, language, set_language, t, translate, Language};
pub use matrix::Matrix;
pub use organize::{AppendReport, ProjectObject};
pub use project::{Folder, Project, ProjectFile, FORMAT_VERSION};
pub use resolve::{resolve_curve, sample_expr, GridData, SeriesData};
pub use surface_data::{column_3d, Column3D};
pub use template::{GraphTemplate, LabelKind, LayerTemplate, TextLabel, TEMPLATE_FORMAT, TEMPLATE_NOTE, TEMPLATE_VERSION};
pub use table::{
    column_letter, Column, ColumnMode, ColumnType, NormalizeMode, NumericFormat, SortMode, Table, TableId,
};
pub use datetime::{add_months, format_axis_value, month_start, parse_clock, AxisClock, DateOrder, TemporalKind};
pub use util::{
    default_true, format_fixed, format_number, format_scientific, format_short, palette_color, parse_date, unique_name, PALETTE,
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
id_ty!(FolderId);
