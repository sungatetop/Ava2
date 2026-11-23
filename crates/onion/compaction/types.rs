//! 压缩词汇表：触发通道 / 结果 / 错误。

use serde::{Deserialize, Serialize};

use crate::types::Usage;

/// 压缩触发通道（pressure 用启发式 estimate，无精确 surface 计量）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactionTrigger {
    /// pre_step 压力评估触发。
    Pressure,
    /// 请求溢出恢复触发。
    ContextOverflow,
    /// 人工 `/compact` 触发。
    Manual,
}

/// 一次压缩的稳定标识。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactionId(pub String);

impl CompactionId {
    pub fn new(seq: u64) -> Self {
        Self(format!("cmp-{}-{}", crate::session::epoch_millis(), seq))
    }
}

/// 压缩失败原因（裁剪子集，无 surface 计量层）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompactionError {
    /// 已有压缩在途。
    Busy,
    /// 无可压缩区域（全部落在保护边界内）。
    Nothing,
    /// 用户取消。
    Cancelled,
    /// 摘要器失败（LLM 调用失败重试耗尽）。
    Summarizer,
    /// 收缩校验失败（摘要不小于原区域）。
    NoShrink,
}

impl CompactionError {
    pub fn label(self) -> &'static str {
        match self {
            Self::Busy => "busy",
            Self::Nothing => "nothing",
            Self::Cancelled => "cancelled",
            Self::Summarizer => "summarizer",
            Self::NoShrink => "no-shrink",
        }
    }
}

/// 一次成功压缩的结果。
#[derive(Clone, Debug, PartialEq)]
pub struct CompactionResult {
    pub id: CompactionId,
    pub trigger: CompactionTrigger,
    /// 被替换区域起点（投影 messages 索引；onion 固定 0，头部 system 不在 messages 内）。
    pub shadowed_start: usize,
    /// 被替换区域条数。
    pub shadowed_count: usize,
    pub shadowed_tokens: u64,
    /// 摘要 token 估算（收缩校验用）。
    pub summary_tokens: u64,
    pub usage: Usage,
    /// `CompactionSummary` 事件的 durable 日志下标。
    pub summary_seq: u64,
}
