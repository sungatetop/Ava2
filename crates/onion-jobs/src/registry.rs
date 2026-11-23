//! 任务注册表：kind 无关的 spawn / settle(first-wins) / kill / list / owner 清理。
//!
//! 并发语义：`settle` 与 `kill` 均 first-wins——首个终态落地后，后续调用返回
//! `AlreadySettled` 语义，不覆盖已落地事实（settle first-wins）。
//! 注册表不持有执行器（进程/PTY 归 `shell_exec`），只管状态与归属，故可独立测试。
//!
//! 取消钩子：执行器不一定是进程（如 workflow 跑在 tokio 任务里），这类任务没有
//! 可 kill 的进程句柄，故可经 [`JobRegistry::register_with_cancel`] 附带一个
//! 「取消回调」；`kill` 在落地 Killed 后触发它，取消动作因此仍经同一控制面
//! （`job_kill`）发出，无需工具层直连各执行器。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::types::{JobOutcome, JobSpec, JobStatus, JobView, KillResult, SessionId};
use crate::OutputRing;

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// 取消回调（kill 时触发；必须可重入注册表——注册表在调用前已释放内部锁）。
pub type CancelHook = Arc<dyn Fn() + Send + Sync>;

/// 任务注册表（进程级单例，按 owner 隔离视图）。
pub struct JobRegistry {
    jobs: Mutex<HashMap<String, Entry>>,
    /// 在飞任务的取消回调（与 `jobs` 分锁：回调内可能重入注册表，不得持锁调用）。
    cancels: Mutex<HashMap<String, CancelHook>>,
}

struct Entry {
    spec: JobSpec,
    status: JobStatus,
    output: OutputRing,
}

impl JobRegistry {
    pub fn new() -> Self {
        Self { jobs: Mutex::new(HashMap::new()), cancels: Mutex::new(HashMap::new()) }
    }

    /// 注册任务（spawn 后调用，进入 Running）。
    pub fn register(&self, spec: JobSpec, ring: OutputRing) {
        let mut jobs = lock(&self.jobs);
        jobs.insert(spec.id.0.clone(), Entry { spec, status: JobStatus::Running, output: ring });
    }

    /// 注册任务并附带取消回调（非进程型执行器用：kill 时回调取消其内部执行）。
    pub fn register_with_cancel(&self, spec: JobSpec, ring: OutputRing, cancel: CancelHook) {
        let id = spec.id.0.clone();
        self.register(spec, ring);
        lock(&self.cancels).insert(id, cancel);
    }

    /// 落地终态（first-wins）。返回 true 表示本次落地，false 表示已被先前终态占用。
    pub fn settle(&self, id: &str, outcome: JobOutcome) -> bool {
        let settled = {
            let mut jobs = lock(&self.jobs);
            let Some(entry) = jobs.get_mut(id) else {
                return false;
            };
            if !entry.status.is_running() {
                return false;
            }
            entry.status = JobStatus::Settled(outcome);
            true
        };
        if settled {
            // 任务已终态，取消回调不再有意义。
            lock(&self.cancels).remove(id);
        }
        settled
    }

    /// 请求取消：在飞任务标记 Killed 并触发取消回调；已结束任务不覆盖。
    pub fn kill(&self, id: &str) -> KillResult {
        let result = {
            let mut jobs = lock(&self.jobs);
            let Some(entry) = jobs.get_mut(id) else {
                return KillResult::NotFound;
            };
            if !entry.status.is_running() {
                return KillResult::AlreadySettled;
            }
            entry.status = JobStatus::Settled(JobOutcome::Killed);
            KillResult::Requested
        };
        // 回调在释放 `jobs` 锁之后触发：回调实现可能重入注册表（push_output 等）。
        if let Some(cancel) = lock(&self.cancels).remove(id) {
            cancel();
        }
        result
    }

    /// 读取任务视图（不存在返回 None）。
    pub fn get(&self, id: &str) -> Option<JobView> {
        let jobs = lock(&self.jobs);
        jobs.get(id).map(|entry| JobView { spec: entry.spec.clone(), status: entry.status.clone() })
    }

    /// 列出某会话的任务（按 id 排序，保证列表确定性）。
    pub fn list(&self, owner: &SessionId) -> Vec<JobView> {
        let jobs = lock(&self.jobs);
        let mut views: Vec<JobView> = jobs
            .values()
            .filter(|entry| &entry.spec.owner == owner)
            .map(|entry| JobView { spec: entry.spec.clone(), status: entry.status.clone() })
            .collect();
        views.sort_by(|a, b| a.spec.id.cmp(&b.spec.id));
        views
    }

    /// 追加输出到任务输出环。
    pub fn push_output(&self, id: &str, chunk: &str) {
        let mut jobs = lock(&self.jobs);
        if let Some(entry) = jobs.get_mut(id) {
            entry.output.push(chunk);
        }
    }

    /// 读取输出窗口（尾部 `limit` 行）。
    pub fn read_output(&self, id: &str, limit: usize) -> Option<String> {
        let jobs = lock(&self.jobs);
        jobs.get(id).map(|entry| entry.output.tail(limit))
    }

    /// owner 清理：会话结束连带删除其全部任务。
    pub fn cleanup_owner(&self, owner: &SessionId) -> usize {
        let removed: Vec<String> = {
            let mut jobs = lock(&self.jobs);
            let before = jobs.len();
            let removed: Vec<String> = jobs
                .iter()
                .filter(|(_, entry)| &entry.spec.owner == owner)
                .map(|(id, _)| id.clone())
                .collect();
            jobs.retain(|_, entry| &entry.spec.owner != owner);
            if jobs.len() == before {
                return 0;
            }
            removed
        };
        let mut cancels = lock(&self.cancels);
        for id in &removed {
            cancels.remove(id);
        }
        removed.len()
    }
}

impl Default for JobRegistry {
    fn default() -> Self {
        Self::new()
    }
}
