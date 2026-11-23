//! 有界输出环：后台任务输出累积与尾部窗口读取。
//!
//! 内存有界（防长跑任务无界增长）；超限时最旧内容被丢弃并记一次溢出标记，
//! 完整日志由执行器 spill 到 `<root>/logs/jobs/<job_id>.log`。

use std::collections::VecDeque;

/// 输出环容量（行）。
const DEFAULT_CAPACITY: usize = 2000;

pub struct OutputRing {
    lines: VecDeque<String>,
    capacity: usize,
    truncated: bool,
}

impl OutputRing {
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self { lines: VecDeque::new(), capacity: capacity.max(1), truncated: false }
    }

    /// 追加一段输出（按行切分，空行保留）。
    pub fn push(&mut self, chunk: &str) {
        for line in chunk.lines() {
            if self.lines.len() == self.capacity {
                self.lines.pop_front();
                self.truncated = true;
            }
            self.lines.push_back(line.to_string());
        }
    }

    /// 尾部 `limit` 行窗口（不足则全量）。
    pub fn tail(&self, limit: usize) -> String {
        let skip = self.lines.len().saturating_sub(limit);
        self.lines.iter().skip(skip).cloned().collect::<Vec<_>>().join("\n")
    }

    /// 是否发生过截断（控制面据此提示「完整日志见 spill 路径」）。
    pub fn truncated(&self) -> bool {
        self.truncated
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

impl Default for OutputRing {
    fn default() -> Self {
        Self::new()
    }
}
