//! Host 独立 schedule 存储：`tasks` 表 JSONL 落盘。
//!
//! - 存储位置：`<ONION_HOME>/schedule/tasks.jsonl`，每行一个 `ScheduleTask`；
//! - 键为 `record.id`（全局唯一 `schedule-{uuid}`）；
//! - 任务独立于 Session 事件流，比 Session 活得久（Session 归档/关闭后仍存在）；
//! - 内存投影 + 原子写（tmp + rename），损坏行容错跳过。

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use crate::schedule::types::{ScheduleId, ScheduleTask};

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// 任务表存储目录：`<root>/schedule`。
pub fn schedule_dir(root: &Path) -> PathBuf {
    root.join("schedule")
}

/// 任务表文件：`<schedule_dir>/tasks.jsonl`。
pub fn tasks_file(root: &Path) -> PathBuf {
    schedule_dir(root).join("tasks.jsonl")
}

/// Host schedule 存储：内存投影 + JSONL 追加 + 原子重写。
pub struct HostScheduleStore {
    path: PathBuf,
    tasks: Mutex<BTreeMap<ScheduleId, ScheduleTask>>,
}

impl HostScheduleStore {
    /// 打开（或创建）任务表：回放既有 JSONL 行重建内存投影。
    pub fn open(root: &Path) -> std::io::Result<Self> {
        std::fs::create_dir_all(schedule_dir(root))?;
        let path = tasks_file(root);
        let mut tasks = BTreeMap::new();
        if let Ok(content) = std::fs::read_to_string(&path) {
            for line in content.lines() {
                if let Ok(task) = serde_json::from_str::<ScheduleTask>(line) {
                    // 键 = record.id（键与 record id 必须一致）。
                    tasks.insert(task.record.id().to_string(), task);
                }
            }
        }
        Ok(Self { path, tasks: Mutex::new(tasks) })
    }

    /// 全部任务（存储序）。
    pub fn all(&self) -> Vec<ScheduleTask> {
        lock(&self.tasks).values().cloned().collect()
    }

    /// 按 id 查询任务。
    pub fn get(&self, id: &str) -> Option<ScheduleTask> {
        lock(&self.tasks).get(id).cloned()
    }

    /// 写入（或覆盖）一条任务，原子落盘。
    pub fn put(&self, task: ScheduleTask) -> std::io::Result<()> {
        {
            let mut tasks = lock(&self.tasks);
            tasks.insert(task.record.id().to_string(), task);
        }
        self.flush()
    }

    /// 删除一条任务，原子落盘。
    pub fn delete(&self, id: &str) -> std::io::Result<bool> {
        let removed = lock(&self.tasks).remove(id).is_some();
        if removed {
            self.flush()?;
        }
        Ok(removed)
    }

    /// 原子重写整表（tmp + rename），保证读侧永不看到半写状态。
    fn flush(&self) -> std::io::Result<()> {
        let tmp = self.path.with_extension("jsonl.tmp");
        {
            let mut file = File::create(&tmp)?;
            for task in lock(&self.tasks).values() {
                let line = serde_json::to_string(task).map_err(std::io::Error::other)?;
                writeln!(file, "{line}")?;
            }
            file.flush()?;
        }
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }

    /// 读取任务表原始行（测试用）。
    pub fn raw_lines(&self) -> Vec<String> {
        std::fs::read_to_string(&self.path)
            .map(|content| content.lines().map(str::to_string).collect())
            .unwrap_or_default()
    }
}

/// 打开（或创建）并读取任务表（便捷入口，保留 BufReader 形态供审计）。
pub fn read_tasks_file(path: &Path) -> std::io::Result<Vec<ScheduleTask>> {
    let file = OpenOptions::new().read(true).open(path)?;
    let reader = BufReader::new(file);
    let mut tasks = Vec::new();
    for line in reader.lines() {
        if let Ok(task) = serde_json::from_str::<ScheduleTask>(&line?) {
            tasks.push(task);
        }
    }
    Ok(tasks)
}

#[cfg(test)]
#[path = "storage_tests.rs"]
mod tests;
