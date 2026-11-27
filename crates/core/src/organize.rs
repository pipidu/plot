//! 项目文件夹，以及把另一个项目追加进来。

use std::collections::HashMap;

use crate::graph::{Curve, DataSource, SurfaceSource};
use crate::util::unique_name;
use crate::project::Note;
use crate::{Folder, FolderId, Graph, GraphId, MatrixId, NoteId, Project, TableId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectObject {
    Table(TableId),
    Matrix(MatrixId),
    Graph(GraphId),
    Note(NoteId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppendReport {
    pub folder: FolderId,
    pub tables: usize,
    pub matrices: usize,
    pub graphs: usize,
    pub notes: usize,
    pub folders: usize,
}

impl Project {
    pub(crate) fn placement(&self) -> Option<FolderId> {
        self.active_folder.filter(|id| self.folder(*id).is_some())
    }

    pub fn folder(&self, id: FolderId) -> Option<&Folder> {
        self.folders.iter().find(|folder| folder.id == id)
    }

    pub fn child_folders(&self, parent: Option<FolderId>) -> Vec<FolderId> {
        let mut ids: Vec<FolderId> = self
            .folders
            .iter()
            .filter(|folder| folder.parent == parent)
            .map(|folder| folder.id)
            .collect();
        ids.sort_by_key(|id| {
            self.folder(*id).map(|folder| folder.name.clone()).unwrap_or_default()
        });
        ids
    }

    pub fn new_folder(&mut self, name: &str, parent: Option<FolderId>) -> Result<FolderId, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err(crate::locale::t("名称不能为空").into());
        }
        if let Some(parent) = parent {
            if self.folder(parent).is_none() {
                return Err(crate::tr!("没有这个文件夹", "That folder does not exist").into());
            }
        }
        let id = FolderId(self.alloc());
        let siblings: Vec<String> = self
            .folders
            .iter()
            .filter(|folder| folder.parent == parent)
            .map(|folder| folder.name.clone())
            .collect();
        let name = unique_name(&siblings, name);
        self.folders.push(Folder { id, name, parent });
        Ok(id)
    }

    pub fn rename_folder(&mut self, id: FolderId, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err(crate::locale::t("名称不能为空").into());
        }
        let parent = self.folder(id).map(|folder| folder.parent).ok_or_else(|| crate::tr!("没有这个文件夹", "That folder does not exist").to_string())?;
        if self.folders.iter().any(|folder| folder.id != id && folder.parent == parent && folder.name == name) {
            return Err(crate::tr!("已有同名文件夹", "A folder with that name already exists").into());
        }
        self.folders.iter_mut().find(|folder| folder.id == id).expect("folder").name = name.to_string();
        Ok(())
    }

    /// 删掉文件夹。里面的对象和子文件夹改挂到它的上级，不删内容。
    pub fn delete_folder(&mut self, id: FolderId) -> Result<(), String> {
        let parent = self.folder(id).map(|folder| folder.parent).ok_or_else(|| crate::tr!("没有这个文件夹", "That folder does not exist").to_string())?;
        for folder in &mut self.folders {
            if folder.parent == Some(id) {
                folder.parent = parent;
            }
        }
        self.folders.retain(|folder| folder.id != id);
        self.retarget_folder_slot(id, parent);
        if self.active_folder == Some(id) {
            self.active_folder = parent;
        }
        Ok(())
    }

    pub fn move_to_folder(&mut self, object: ProjectObject, folder: Option<FolderId>) -> Result<(), String> {
        if let Some(folder) = folder {
            if self.folder(folder).is_none() {
                return Err(crate::tr!("没有这个文件夹", "That folder does not exist").into());
            }
        }
        let slot = match object {
            ProjectObject::Table(id) => self.table_mut(id).map(|table| &mut table.folder).ok_or_else(|| crate::locale::t("找不到表格").to_string())?,
            ProjectObject::Matrix(id) => self.matrix_mut(id).map(|matrix| &mut matrix.folder).ok_or_else(|| crate::locale::t("找不到矩阵").to_string())?,
            ProjectObject::Graph(id) => self.graph_mut(id).map(|graph| &mut graph.folder).ok_or_else(|| crate::locale::t("找不到图形").to_string())?,
            ProjectObject::Note(id) => self.note_mut(id).map(|note| &mut note.folder).ok_or_else(|| crate::locale::t("备注已删除").to_string())?,
        };
        *slot = folder;
        Ok(())
    }

    /// 项目树按文件夹过滤。`view` 为空表示根。勾了包含子文件夹时，子文件夹里的对象也算在内。
    pub fn folder_visible(&self, view: Option<FolderId>, include_descendants: bool, item: Option<FolderId>) -> bool {
        if view == item {
            return true;
        }
        if !include_descendants {
            return false;
        }
        self.is_within(item, view)
    }

    pub fn sanitize_folders(&mut self) {
        self.break_folder_cycles();
        let known: Vec<FolderId> = self.folders.iter().map(|folder| folder.id).collect();
        for folder in &mut self.folders {
            if let Some(parent) = folder.parent {
                if !known.contains(&parent) || parent == folder.id {
                    folder.parent = None;
                }
            }
        }
        let fix = |slot: &mut Option<FolderId>, known: &[FolderId]| {
            if let Some(id) = *slot {
                if !known.contains(&id) {
                    *slot = None;
                }
            }
        };
        for table in &mut self.tables {
            fix(&mut table.folder, &known);
        }
        for matrix in &mut self.matrices {
            fix(&mut matrix.folder, &known);
        }
        for graph in &mut self.graphs {
            fix(&mut graph.folder, &known);
        }
        for note in &mut self.notes {
            fix(&mut note.folder, &known);
        }
        fix(&mut self.active_folder, &known);
    }

    /// 把 `other` 合并进当前项目，不替换已有内容。新内容放进一个以其项目名命名的文件夹。
    pub fn append_project(&mut self, other: &Project) -> AppendReport {
        let other = other.clone();
        let root_name = if other.name.trim().is_empty() {
            crate::locale::t("追加的项目")
        } else {
            other.name.as_str()
        };
        let root = self.new_folder(root_name, self.placement()).unwrap_or_else(|_| FolderId(self.alloc()));
        if self.folder(root).is_none() {
            self.folders.push(Folder {
                id: root,
                name: root_name.to_string(),
                parent: self.placement(),
            });
        }
        let folder_map = self.map_folders(&other, root);
        let map_folder = |folder: Option<FolderId>| -> Option<FolderId> {
            match folder {
                None => Some(root),
                Some(id) => folder_map.get(&id).copied().or(Some(root)),
            }
        };
        let mut tables = HashMap::new();
        for table in &other.tables {
            let mut copy = table.clone();
            copy.folder = map_folder(table.folder);
            let id = self.insert_table(copy);
            tables.insert(table.id, id);
        }
        let mut matrices = HashMap::new();
        for matrix in &other.matrices {
            let mut copy = matrix.clone();
            copy.folder = map_folder(matrix.folder);
            let id = self.insert_matrix(copy);
            matrices.insert(matrix.id, id);
        }
        for note in &other.notes {
            let mut copy = note.clone();
            copy.folder = map_folder(note.folder);
            self.insert_note(copy);
        }
        for graph in &other.graphs {
            let mut copy = graph.clone();
            copy.folder = map_folder(graph.folder);
            self.retarget_graph(&mut copy, &tables, &matrices);
            self.insert_graph(copy);
        }
        let copied = other.log.len().min(200);
        if copied > 0 {
            self.log_line(crate::trf!(
                "—— 追加自「{name}」的日志 ——",
                "—— Log appended from \"{name}\" ——",
                name = other.name
            ));
            let start = other.log.len() - copied;
            for line in &other.log[start..] {
                self.log_line(line.clone());
            }
        }
        self.log_line(crate::trf!(
            "已追加项目「{name}」：表格 {}，矩阵 {}，图形 {}，备注 {}。",
            "Appended project \"{name}\": {} tables, {} matrices, {} graphs, {} notes.",
            other.tables.len(),
            other.matrices.len(),
            other.graphs.len(),
            other.notes.len(),
            name = other.name
        ));
        AppendReport {
            folder: root,
            tables: other.tables.len(),
            matrices: other.matrices.len(),
            graphs: other.graphs.len(),
            notes: other.notes.len(),
            folders: other.folders.len(),
        }
    }

    fn map_folders(&mut self, other: &Project, root: FolderId) -> HashMap<FolderId, FolderId> {
        let mut map = HashMap::new();
        let mut pending = other.folders.clone();
        for _ in 0..pending.len().saturating_add(1) {
            if pending.is_empty() {
                break;
            }
            let before = pending.len();
            let mut later = Vec::new();
            for folder in pending {
                let ready = match folder.parent {
                    None => true,
                    Some(parent) => map.contains_key(&parent),
                };
                if !ready {
                    later.push(folder);
                    continue;
                }
                let parent = match folder.parent {
                    None => Some(root),
                    Some(parent) => map.get(&parent).copied(),
                };
                if let Ok(id) = self.new_folder(&folder.name, parent) {
                    map.insert(folder.id, id);
                }
            }
            pending = later;
            if pending.len() == before {
                for folder in pending {
                    if let Ok(id) = self.new_folder(&folder.name, Some(root)) {
                        map.insert(folder.id, id);
                    }
                }
                break;
            }
        }
        map
    }

    fn retarget_graph(&mut self, graph: &mut Graph, tables: &HashMap<TableId, TableId>, matrices: &HashMap<MatrixId, MatrixId>) {
        for layer in &mut graph.layers {
            layer.curves.retain_mut(|curve| retarget_curve(curve, tables, matrices));
            for annotation in &mut layer.annotations {
                annotation.id = self.alloc();
            }
        }
        if let Some(spec) = &mut graph.surface {
            match &mut spec.source {
                SurfaceSource::Matrix(id) => {
                    if let Some(next) = matrices.get(id) {
                        *id = *next;
                    }
                }
                SurfaceSource::Table { table, .. } => {
                    if let Some(next) = tables.get(table) {
                        *table = *next;
                    }
                }
                SurfaceSource::Function { .. } => {}
            }
        }
    }

    fn retarget_folder_slot(&mut self, from: FolderId, to: Option<FolderId>) {
        for table in &mut self.tables {
            if table.folder == Some(from) {
                table.folder = to;
            }
        }
        for matrix in &mut self.matrices {
            if matrix.folder == Some(from) {
                matrix.folder = to;
            }
        }
        for graph in &mut self.graphs {
            if graph.folder == Some(from) {
                graph.folder = to;
            }
        }
        for note in &mut self.notes {
            if note.folder == Some(from) {
                note.folder = to;
            }
        }
    }

    fn is_within(&self, folder: Option<FolderId>, ancestor: Option<FolderId>) -> bool {
        let mut current = folder;
        for _ in 0..64 {
            let Some(id) = current else {
                break;
            };
            if Some(id) == ancestor {
                return true;
            }
            current = self.folder(id).and_then(|folder| folder.parent);
        }
        ancestor.is_none() && folder.is_some()
    }

    fn break_folder_cycles(&mut self) {
        let ids: Vec<FolderId> = self.folders.iter().map(|folder| folder.id).collect();
        for id in ids {
            let mut seen = Vec::new();
            let mut current = Some(id);
            while let Some(at) = current {
                if seen.contains(&at) {
                    if let Some(folder) = self.folders.iter_mut().find(|folder| folder.id == at) {
                        folder.parent = None;
                    }
                    break;
                }
                seen.push(at);
                current = self.folder(at).and_then(|folder| folder.parent);
            }
        }
    }

    pub fn insert_note(&mut self, mut note: Note) -> NoteId {
        note.id = NoteId(self.alloc());
        let names: Vec<String> = self.notes.iter().map(|item| item.name.clone()).collect();
        note.name = unique_name(&names, &note.name);
        let id = note.id;
        self.notes.push(note);
        id
    }

    pub fn insert_graph(&mut self, mut graph: Graph) -> GraphId {
        graph.id = GraphId(self.alloc());
        let names: Vec<String> = self.graphs.iter().map(|item| item.name.clone()).collect();
        graph.name = unique_name(&names, &graph.name);
        let id = graph.id;
        self.graphs.push(graph);
        id
    }
}

fn retarget_curve(curve: &mut Curve, tables: &HashMap<TableId, TableId>, matrices: &HashMap<MatrixId, MatrixId>) -> bool {
    match &mut curve.source {
        DataSource::Columns { table, .. } => {
            if let Some(next) = tables.get(table) {
                *table = *next;
                true
            } else {
                false
            }
        }
        DataSource::Matrix { matrix } => {
            if let Some(next) = matrices.get(matrix) {
                *matrix = *next;
                true
            } else {
                false
            }
        }
        DataSource::Function { .. } | DataSource::Parametric { .. } | DataSource::Polar { .. } | DataSource::Points { .. } => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::CurveKind;

    #[test]
    fn folders_filter_and_keep_children_when_deleted() {
        let mut project = Project::new("实验");
        let table = project.new_table("根表");
        let folder = project.new_folder("数据", None).unwrap();
        let nested = project.new_folder("原始", Some(folder)).unwrap();
        project.active_folder = Some(nested);
        let inner = project.new_table("内表");
        assert_eq!(project.table(inner).unwrap().folder, Some(nested));
        assert!(project.folder_visible(None, false, project.table(table).unwrap().folder));
        assert!(!project.folder_visible(None, false, Some(nested)));
        assert!(project.folder_visible(Some(folder), true, Some(nested)));
        assert!(!project.folder_visible(Some(folder), false, Some(nested)));
        assert!(project.folder_visible(Some(nested), false, Some(nested)));
        project.delete_folder(nested).unwrap();
        assert_eq!(project.table(inner).unwrap().folder, Some(folder));
        assert!(project.folder(nested).is_none());
        project.move_to_folder(ProjectObject::Table(table), Some(folder)).unwrap();
        assert_eq!(project.table(table).unwrap().folder, Some(folder));
        let loaded = Project::from_json(&project.to_json().unwrap()).unwrap();
        assert_eq!(loaded.folders.len(), 1);
        assert_eq!(loaded.table(inner).unwrap().folder, loaded.table(table).unwrap().folder);
    }

    #[test]
    fn append_merges_without_replacing_and_retargets_curves() {
        let mut current = Project::new("当前");
        let kept = current.new_table("留下");
        current.table_mut(kept).unwrap().columns[0].set_numbers(&[1.0, 2.0]);
        current.table_mut(kept).unwrap().columns[1].set_numbers(&[3.0, 4.0]);
        let kept_graph = current.plot_table(kept, CurveKind::Line).unwrap();

        let mut extra = Project::new("另一份");
        let added = extra.new_table("新表");
        extra.table_mut(added).unwrap().columns[0].set_numbers(&[0.0, 1.0]);
        extra.table_mut(added).unwrap().columns[1].set_numbers(&[0.0, 2.0]);
        let added_graph = extra.plot_table(added, CurveKind::Scatter).unwrap();
        extra.new_note("说明");
        extra.log_line("来自另一份");

        let report = current.append_project(&extra);
        assert_eq!(current.tables.len(), 2);
        assert_eq!(current.table(kept).unwrap().name, "留下");
        assert!(current.folder(report.folder).is_some());
        let added_now = current.tables.iter().find(|table| table.name == "新表").unwrap();
        assert_eq!(added_now.folder, Some(report.folder));
        assert_ne!(added_now.id, kept);
        match &current.graph(kept_graph).unwrap().layers[0].curves[0].source {
            DataSource::Columns { table, .. } => assert_eq!(*table, kept),
            other => panic!("unexpected {other:?}"),
        }
        let graph = current.graphs.iter().find(|graph| graph.id != kept_graph).unwrap();
        match &graph.layers[0].curves[0].source {
            DataSource::Columns { table, .. } => assert_eq!(*table, added_now.id),
            other => panic!("unexpected {other:?}"),
        }
        assert!(current.notes.iter().any(|note| note.name == "说明"));
        assert!(current.log.iter().any(|line| line.contains("来自另一份")));
        assert_ne!(added_graph, graph.id);
    }
}
