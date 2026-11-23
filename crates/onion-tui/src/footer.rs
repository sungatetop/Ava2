//! footer.rs：idle / running 双态状态栏（设计文档 §8）。
//!
//! - idle：上下文占用率 + model；
//! - running：elapsed 活动时长 + Escape 中断提示；
//!
//! 占用率估算复用 `onion::compaction::estimate` 口径（chars/4），数据源 =
//! durable 事件快照（append-origin 投影，见 transcript.rs）。

use std::time::Instant;

use crate::transcript::Transcript;

/// footer 状态（idle / running 双态）。
pub enum Footer {
    /// 空闲：上下文占用率 + model。
    Idle,
    /// 轮进行中：elapsed + Escape 提示。
    Running { started_at: Instant },
}

impl Footer {
    /// 渲染 footer 行（idle：占用率 + 轮次统计 + model；running：elapsed +
    /// Escape 提示）。`turn_stats` = 最近一轮报告（turn, steps），无轮历史为
    /// None；model 名的高亮段由调用方按返回串中的字节范围叠加。
    pub fn render(
        &self,
        transcript: &Transcript,
        model: &str,
        budget: u64,
        turn_stats: Option<(u32, u32)>,
    ) -> String {
        match self {
            Footer::Idle => {
                let estimated = estimate_transcript(transcript);
                let ratio = if budget == 0 {
                    0.0
                } else {
                    estimated as f64 / budget as f64
                };
                let stats = match turn_stats {
                    Some((turn, steps)) => format!(" · t{turn} · {steps} steps"),
                    None => String::new(),
                };
                format!("context {ratio:.1}%{stats} · {model}", ratio = ratio * 100.0)
            }
            Footer::Running { started_at } => {
                let elapsed = started_at.elapsed().as_secs();
                format!(
                    "running {m:02}:{s:02} · Esc 中断",
                    m = elapsed / 60,
                    s = elapsed % 60,
                )
            }
        }
    }
}

/// transcript 投影的 token 估算（chars/4，对齐 compaction::estimate 口径）。
pub fn estimate_transcript(transcript: &Transcript) -> u64 {
    let mut chars: f64 = 0.0;
    for node in transcript.nodes() {
        match node {
            crate::transcript::Node::Header { session_id, model } => {
                chars += (session_id.chars().count() + model.chars().count()) as f64;
            }
            crate::transcript::Node::TurnSeparator { turn: _, reason } => {
                chars += 4.0;
                if let Some(reason) = reason {
                    chars += reason.chars().count() as f64;
                }
            }
            crate::transcript::Node::UserMessage { message } => {
                chars += message_text_len(message) as f64;
            }
            crate::transcript::Node::InjectionNote { kind, chars: note_chars } => {
                // 注记行本身 + 被折叠正文的真实 token 占用（模型仍消费全文）。
                chars += (kind.chars().count() + 12 + note_chars) as f64;
            }
            crate::transcript::Node::AssistantBlock { text, thinking, .. } => {
                chars += (text.chars().count() + thinking.chars().count()) as f64;
            }
            crate::transcript::Node::ToolCard { name, input_summary, output, .. } => {
                chars += name.chars().count() as f64;
                chars += input_summary.chars().count() as f64;
                if let Some(output) = output {
                    chars += output.chars().count() as f64;
                }
            }
            crate::transcript::Node::AssistantAttempt { error, .. } => {
                chars += error.chars().count() as f64;
            }
            crate::transcript::Node::CompactionCard { trigger, summary, .. } => {
                chars += trigger.chars().count() as f64;
                if let Some(summary) = summary {
                    chars += summary.chars().count() as f64;
                }
            }
            crate::transcript::Node::GoalBar { objective, phase, .. } => {
                chars += (objective.chars().count() + phase.chars().count()) as f64;
            }
            crate::transcript::Node::PlanBadge { .. } => chars += 4.0,
            crate::transcript::Node::HookLine { point, handler_id, .. } => {
                chars += (point.chars().count() + handler_id.chars().count()) as f64;
            }
            crate::transcript::Node::WorkspaceLine { summary } => {
                chars += summary.chars().count() as f64;
            }
            crate::transcript::Node::TodoList { todos } => {
                for todo in todos {
                    chars += todo.content.chars().count() as f64;
                }
            }
        }
    }
    (chars / 4.0).ceil() as u64
}

/// AgentMessage 文本长度（Text/Thinking 块）。
fn message_text_len(message: &onion::types::AgentMessage) -> usize {
    message
        .blocks
        .iter()
        .map(|b| match b {
            onion::types::ContentBlock::Text { text } => text.chars().count(),
            onion::types::ContentBlock::Thinking { thinking } => thinking.chars().count(),
            _ => 0,
        })
        .sum()
}

#[cfg(test)]
#[path = "footer_tests.rs"]
mod tests;
