use crate::Project;

/// 以项目快照实现撤销/重做。每次修改前压入修改前的 JSON。
#[derive(Clone, Debug, Default)]
pub struct History {
    undo: Vec<(String, String)>,
    redo: Vec<(String, String)>,
    limit: usize,
}

impl History {
    pub fn new() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            limit: 40,
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn push(&mut self, label: &str, before: &Project) {
        if let Ok(json) = serde_json::to_string(before) {
            self.undo.push((label.to_string(), json));
            if self.undo.len() > self.limit {
                let extra = self.undo.len() - self.limit;
                self.undo.drain(0..extra);
            }
            self.redo.clear();
        }
    }

    pub fn undo(&mut self, current: &Project) -> Option<(String, Project)> {
        let (label, json) = self.undo.pop()?;
        if let Ok(now) = serde_json::to_string(current) {
            self.redo.push((label.clone(), now));
        }
        let project = serde_json::from_str(&json).ok()?;
        Some((label, project))
    }

    pub fn redo(&mut self, current: &Project) -> Option<(String, Project)> {
        let (label, json) = self.redo.pop()?;
        if let Ok(now) = serde_json::to_string(current) {
            self.undo.push((label.clone(), now));
        }
        let project = serde_json::from_str(&json).ok()?;
        Some((label, project))
    }
}
