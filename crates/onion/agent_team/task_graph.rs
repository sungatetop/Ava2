//! blockedBy DAG 校验。
//!
//! 对候选 blockedBy 列表做图校验：缺 blocker / 自指 / 重复 / 环 → 映射错误码。

// 错误码映射。
pub const GRAPH_ERROR_MISSING: &str = "task_not_found";
pub const GRAPH_ERROR_DUPLICATE: &str = "invalid_argument";
pub const GRAPH_ERROR_CYCLE: &str = "dependency_cycle";

/// 一次图校验结果：blockedBy 中每个 id 是否已存在（completed 或非 completed）。
#[derive(Debug, Clone)]
pub struct GraphCheck {
    /// 归一化去重后的 blocker 列表。
    pub blockers: Vec<String>,
}

impl GraphCheck {
    /// 对给定 blocker 候选做存在性校验，返回归一化列表或错误。
    ///
    /// - `self_id`：当前任务 id（用于自指检测）；
    /// - `exists`：判定一个 id 是否是已存在的任务（含 deleted tombstone）。
    pub fn validate(
        candidates: &[String],
        self_id: &str,
        exists: impl Fn(&str) -> bool,
    ) -> Result<GraphCheck, GraphError> {
        let mut seen = Vec::new();
        for id in candidates {
            if id == self_id {
                return Err(GraphError::Cycle(self_id.to_string()));
            }
            if seen.contains(id) {
                return Err(GraphError::Duplicate(id.clone()));
            }
            if !exists(id) {
                return Err(GraphError::Missing(id.clone()));
            }
            seen.push(id.clone());
        }
        Ok(GraphCheck { blockers: seen })
    }
}

/// 图校验错误（映射到 TeamError 错误码）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    Missing(String),
    Duplicate(String),
    Cycle(String),
}

impl GraphError {
    /// 映射错误码名。
    pub fn code(&self) -> &'static str {
        match self {
            GraphError::Missing(_) => GRAPH_ERROR_MISSING,
            GraphError::Duplicate(_) => GRAPH_ERROR_DUPLICATE,
            GraphError::Cycle(_) => GRAPH_ERROR_CYCLE,
        }
    }

    /// 检测 blockedBy 是否成环（DFS）。
    ///
    /// `successors`：id → 其 blockedBy 列表；返回首个成环的 id。
    pub fn find_cycle(
        id: &str,
        successors: &dyn Fn(&str) -> Vec<String>,
    ) -> Option<String> {
        fn visit(
            current: &str,
            successors: &dyn Fn(&str) -> Vec<String>,
            visiting: &mut Vec<String>,
            visited: &mut Vec<String>,
        ) -> Option<String> {
            if visiting.contains(&current.to_string()) {
                return Some(current.to_string());
            }
            if visited.contains(&current.to_string()) {
                return None;
            }
            visiting.push(current.to_string());
            for next in successors(current) {
                if let Some(cycle) = visit(&next, successors, visiting, visited) {
                    return Some(cycle);
                }
            }
            visiting.pop();
            visited.push(current.to_string());
            None
        }
        visit(id, successors, &mut Vec::new(), &mut Vec::new())
    }
}

#[cfg(test)]
#[path = "task_graph_tests.rs"]
mod tests;
