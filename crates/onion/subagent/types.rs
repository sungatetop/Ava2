//! subagent 词汇表：激活句柄 / Provider seam / 错误 / 运行状态。
//!
//! 类型契约先行，Provider seam（宿主注入如何创建 subagent 会话）与
//! Runtime（进程单例）分离——types.rs 不反向依赖 SessionLog（durable 层）。

use std::sync::Arc;

use tokio_util::sync::CancellationToken;

/// 根代理名：`list_agents` 首行标识，保留名不可用于子代理。
pub const ROOT_AGENT_NAME: &str = "lead";

/// subagent 运行状态（list_agents 下发模型的视图）。
///
/// 状态语义：
/// - `running` = 有 turn 正在执行；
/// - `inactive` = 没有 turn 在执行（**不代表任务已完成/成功或失败**）。
///
/// 下游子树诊断态（「这一行的
/// **下游子树**读不出来」）不维护：onion 只维护扁平活跃表（不遍历子树、
/// 不持久化子代理描述符），故不产生诊断态。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentStatus {
    /// 有 turn 正在执行。
    Running,
    /// 没有 turn 在执行（settle / 取消 / 失败后统一收敛到此态）。
    Inactive,
}

impl AgentStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Inactive => "inactive",
        }
    }

    /// 是否已无 turn 在执行（wait_agent 的「已落定」判据）。
    pub fn is_inactive(self) -> bool {
        matches!(self, Self::Inactive)
    }
}

/// subagent 激活句柄。
///
/// `name` = 模型可读代理名（显式指定或默认 `agent-N`，活跃表内唯一，
/// `lead` 保留给根代理）；`description` = 委派任务简述（展示用）。
/// `inbox` = subagent 会话自己的 inbox（send_message 的投递目标）；
/// None = 会话无 inbox 句柄（send_message 拒投）。
#[derive(Clone)]
pub struct SubagentActivation {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub depth: u32,
    pub status: AgentStatus,
    pub cancel: CancellationToken,
    pub inbox: Option<Arc<crate::inbox::Inbox>>,
}

/// subagent 体系错误。
#[derive(Debug, thiserror::Error)]
pub enum SubagentError {
    /// 深度超限。
    #[error("subagent depth {depth} exceeds max {max}")]
    Depth { depth: u32, max: u32 },
    /// 未知子会话（id 不存在，或会话已 ended 而不可续聊）。
    #[error("unknown subagent: {0}")]
    Unknown(String),
    /// 不可恢复（`NOT_RESUMABLE`）：无 descriptor、`one-shot` 描述符，
    /// 或子会话日志不可读——此时只能重新派生一个子代理。
    #[error(
        "subagent `{0}` cannot be resumed (no continuable descriptor); spawn a new subagent instead"
    )]
    NotResumable(String),
    /// inbox 容量满等透传错误。
    #[error("relay failed: {0}")]
    Inbox(#[from] crate::inbox::InboxError),
    /// subagent 会话装配失败（落盘日志目录不可写等 IO 错误）。
    #[error("subagent session assemble failed: {0}")]
    Session(#[from] std::io::Error),
    /// 子会话组合工厂失败（如委派 cwd 越出权限根 / 目录不存在）。
    #[error("subagent session factory failed: {0}")]
    Factory(String),
    /// 调用参数非法（当前仅 `delivery` 取值）。
    #[error("invalid subagent argument: {0}")]
    Invalid(String),
}
