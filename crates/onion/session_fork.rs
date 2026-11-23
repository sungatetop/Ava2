//! 会话 fork/分支：把源会话 `[1..=boundary]` 前缀复制为子会话日志。
//!
//! fork 复制前缀后追加一条
//! [`SessionEvent::ForkSeed`] 播种标记（`session/end-seed { inherited: true }` 形态），
//! 再合成关闭事件收尾：
//!
//! 1. 未配对的 `ToolCall` 先补一条错误 `ToolResult`（避免恢复后模型悬挂等待结果）；
//! 2. 未闭合的 `StepStart` 补 `StepEnd`；
//! 3. 未闭合的 `TurnStart` 补 `TurnEnd { reason: "forked" }`。
//!
//! 子会话身份来自目录名（恢复时以 attach 参数 id 为准），前缀中的源
//! `SessionStarted` 保留原样（历史事实，恢复时按 childId 重建）。
//! 重建零特殊化：子会话历史已含父前缀，重放自身日志即可；`ForkSeed` 为 log-only
//! 事件，上下文投影不消费，lineage 读取面（`session_trace`）据此输出血缘。

use std::io::{self, Write};
use std::path::Path;

use crate::session::SessionEvent;
use crate::session_query::EventRecord;
use crate::types::{ToolCall, ToolResult};

/// fork 合成关闭的 turn 结束原因（`"forked"`）。
pub const FORKED_TURN_REASON: &str = "forked";

/// fork 输入非法（边界越界）。
#[derive(Debug, PartialEq, Eq)]
pub enum ForkError {
    /// 边界不在 `1..=last_seq` 内（源为空时 `last_seq` 为 `None`）。
    InvalidBoundary {
        /// 调用方给出的边界 seq。
        boundary: u64,
        /// 源日志最后一条 seq（空源为 `None`）。
        last_seq: Option<u64>,
    },
}

impl std::fmt::Display for ForkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ForkError::InvalidBoundary { boundary, last_seq } => match last_seq {
                Some(last) => write!(f, "fork boundary {boundary} out of range 1..={last}"),
                None => write!(f, "fork boundary {boundary} invalid: source log is empty"),
            },
        }
    }
}

/// [`build_fork_seed`] 的输出：子会话完整事件序列（前缀 + 播种标记 + 合成关闭）。
#[derive(Debug, PartialEq)]
pub struct ForkedLog {
    /// 子会话日志事件（seq 重排为 1 起连续，落盘后 seq 由行号推导一致）。
    pub records: Vec<EventRecord>,
    /// 继承前缀条数（不含播种标记与合成关闭事件）。
    pub inherited_events: u64,
}

/// 构造 fork 子会话日志（纯函数，不落盘）。
///
/// - 源为空且未指定边界 → 空子会话；
/// - 边界缺省取源日志最后一条 seq；
/// - 显式边界不在 `1..=last_seq` → [`ForkError::InvalidBoundary`]。
///
/// 播种标记与合成关闭事件的时间戳统一沿用边界事件 ts。
pub fn build_fork_seed(
    source: &[EventRecord],
    boundary: Option<u64>,
    parent_session_id: &str,
) -> Result<ForkedLog, ForkError> {
    let last_seq = source.last().map(|record| record.seq);
    let resolved = match boundary {
        None => match last_seq {
            Some(last) => last,
            None => return Ok(ForkedLog { records: Vec::new(), inherited_events: 0 }),
        },
        Some(explicit) => {
            let valid = last_seq.is_some_and(|last| explicit >= 1 && explicit <= last);
            if !valid {
                return Err(ForkError::InvalidBoundary { boundary: explicit, last_seq });
            }
            explicit
        }
    };
    // 前缀复制：ts 沿用源事件，seq 重排为 1 起连续（容错跳行导致的空洞）。
    let mut records: Vec<EventRecord> = source
        .iter()
        .take_while(|record| record.seq <= resolved)
        .scan(0u64, |seq, record| {
            *seq += 1;
            Some(EventRecord { seq: *seq, ts: record.ts, event: record.event.clone() })
        })
        .collect();
    let inherited = records.len() as u64;
    let tail_ts = records.last().map(|record| record.ts).unwrap_or(0);
    records.push(EventRecord {
        seq: inherited + 1,
        ts: tail_ts,
        event: SessionEvent::ForkSeed {
            parent_session_id: parent_session_id.to_string(),
            parent_boundary_seq: resolved,
            inherited_events: inherited,
        },
    });
    append_fork_closers(&mut records, inherited + 2, tail_ts);
    Ok(ForkedLog { records, inherited_events: inherited })
}

/// 合成关闭事件：未配对工具调用补错误结果，未闭合
/// step/turn 补关闭。`next_seq` 起连续编号，时间统一取边界事件 ts。
fn append_fork_closers(records: &mut Vec<EventRecord>, next_seq: u64, ts: u64) {
    let mut seq = next_seq;
    let mut open_calls: Vec<(u32, u32, ToolCall)> = Vec::new();
    let mut open_step: Option<(u32, u32)> = None;
    let mut open_turn: Option<u32> = None;
    for record in records.iter() {
        match &record.event {
            SessionEvent::ToolCall { turn, step, call } => open_calls.push((*turn, *step, call.clone())),
            SessionEvent::ToolResult { result, .. } => {
                open_calls.retain(|(_, _, call)| call.id != result.tool_call_id);
            }
            SessionEvent::StepStart { turn, step } => open_step = Some((*turn, *step)),
            SessionEvent::StepEnd { .. } => open_step = None,
            SessionEvent::TurnStart { turn } => open_turn = Some(*turn),
            SessionEvent::TurnEnd { .. } => open_turn = None,
            _ => {}
        }
    }
    for (turn, step, call) in open_calls {
        records.push(EventRecord {
            seq,
            ts,
            event: SessionEvent::ToolResult {
                turn,
                step,
                result: ToolResult::error(&call, "forked: tool did not complete"),
            },
        });
        seq += 1;
    }
    if let Some((turn, step)) = open_step {
        records.push(EventRecord { seq, ts, event: SessionEvent::StepEnd { turn, step } });
        seq += 1;
    }
    if let Some(turn) = open_turn {
        records.push(EventRecord {
            seq,
            ts,
            event: SessionEvent::TurnEnd { turn, reason: FORKED_TURN_REASON.to_string() },
        });
    }
}

/// 子会话日志落盘：新 id 目录无并发写者，不取写租约；逐行
/// `{"ts": …, "event": …}`（seq 由 1-based 行号推导，不落盘）。
pub fn write_forked_log(child_dir: &Path, records: &[EventRecord]) -> io::Result<()> {
    std::fs::create_dir_all(child_dir)?;
    let file = crate::paths::session_log_file(child_dir);
    let mut out = String::new();
    for record in records {
        let line = serde_json::json!({ "ts": record.ts, "event": &record.event });
        out.push_str(&serde_json::to_string(&line)?);
        out.push('\n');
    }
    let mut handle = std::io::BufWriter::new(std::fs::File::create(&file)?);
    handle.write_all(out.as_bytes())?;
    let handle = handle.into_inner().map_err(io::IntoInnerError::into_error)?;
    handle.sync_all()
}

#[cfg(test)]
#[path = "session_fork_tests.rs"]
mod tests;
