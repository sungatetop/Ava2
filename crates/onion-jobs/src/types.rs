//! 后台任务核心类型（与 ava-toolkit `jobs` 语义对齐）。
//!
//! kind 无关：任务按 `JobKind` 区分来源（shell / 未来其他执行器），控制面
//! （`job_list` / `job_output` / `job_kill`）只依赖本模块类型，不感知 kind。

use std::fmt;

/// 任务标识（注册表内唯一）。
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct JobId(pub String);

impl fmt::Display for JobId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// 任务归属会话（owner 隔离：清理与可见性按会话切分）。
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionId(pub String);

/// 任务来源种类（控制面 kind 无关，诊断用）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobKind {
    /// 一次性 shell / pwsh 命令晋升为后台任务。
    Shell,
    /// `terminal_send` 的 `run_in_background` 后台化（PTY 输入派发）。
    PtySend,
    /// `workflow` 工具的 `run_in_background` 后台化（声明式编排跑在后台 job 里）。
    Workflow,
}

impl fmt::Display for JobKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JobKind::Shell => f.write_str("shell"),
            JobKind::PtySend => f.write_str("pty_send"),
            JobKind::Workflow => f.write_str("workflow"),
        }
    }
}

/// 任务终态。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JobOutcome {
    /// 正常结束（携带退出码）。
    Exited { code: i32 },
    /// 被取消（kill 或会话结束连带）。
    Killed,
    /// 启动或执行期失败（携带原因）。
    Failed { error: String },
}

impl JobOutcome {
    /// 是否为成功结束（`job_*` 控制面与宿主呈示的成败判定）。
    pub fn is_success(&self) -> bool {
        matches!(self, JobOutcome::Exited { code: 0 })
    }
}

/// 任务生命周期状态。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JobStatus {
    /// 在飞。
    Running,
    /// 已落地终态。
    Settled(JobOutcome),
}

impl JobStatus {
    pub fn is_running(&self) -> bool {
        matches!(self, JobStatus::Running)
    }
}

/// 任务注册时的规格。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobSpec {
    pub id: JobId,
    pub owner: SessionId,
    pub kind: JobKind,
    /// 展示用命令摘要（完整命令可能很长，控制面只回摘要）。
    pub command: String,
}

/// 任务视图（控制面读取的不可变快照）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobView {
    pub spec: JobSpec,
    pub status: JobStatus,
}

/// 取消结果（区分「不存在」与「已结束」两种幂等失败）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KillResult {
    /// 已发出取消，任务将在输出环关闭后 settle。
    Requested,
    NotFound,
    AlreadySettled,
}
