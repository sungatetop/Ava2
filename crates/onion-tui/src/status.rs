//! status.rs：/status 快照（设计文档 §10）。
//!
//! idle 与 running 均可用的 terminal-only 详细快照（非 durable，不进
//! transcript）。数据源 = durable 事件快照（append-origin 唯一事实源）+ Config：
//! - 会话身份（session id、model）；
//! - 生命周期计数：turns / steps / tool calls / compactions；
//! - usage 桶：input / output / cache_read / cache_write 累计 + KV 命中率；
//! - 上下文占用：估算值 + Config 预算。

use onion::session::SessionEvent;
use onion::types::Usage;

/// durable 事件流的 token 估算（口径唯一实现在 onion，宿主仅转发）。
pub use onion::compaction::estimate::estimate_events;

/// /status 快照（宿主渲染到反馈区，非 durable）。
pub struct StatusSnapshot {
    pub session_id: String,
    pub model: String,
    pub turns: u64,
    pub steps: u64,
    pub tool_calls: u64,
    pub compactions: u64,
    pub usage: Usage,
    pub estimated_tokens: u64,
    pub context_window_tokens: u64,
}

impl StatusSnapshot {
    /// 从 durable 事件快照折叠（生命周期计数 + usage 桶）。
    pub fn from_events(events: &[SessionEvent], context_window_tokens: u64) -> Self {
        let mut session_id = String::new();
        let mut model = String::new();
        let mut turns = 0;
        let mut steps = 0;
        let mut tool_calls = 0;
        let mut compactions = 0;
        let mut usage = Usage {
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
        };
        for event in events {
            match event {
                SessionEvent::SessionStarted { session_id: id, model: m } => {
                    session_id = id.clone();
                    model = m.clone();
                }
                SessionEvent::TurnEnd { .. } => turns += 1,
                SessionEvent::StepEnd { .. } => steps += 1,
                SessionEvent::ToolCall { .. } => tool_calls += 1,
                SessionEvent::CompactionEnd { .. } => compactions += 1,
                SessionEvent::AssistantMessage { usage: u, .. } => {
                    usage.input_tokens += u.input_tokens;
                    usage.output_tokens += u.output_tokens;
                    usage.cache_read_tokens += u.cache_read_tokens;
                    usage.cache_write_tokens += u.cache_write_tokens;
                }
                _ => {}
            }
        }
        let estimated = estimate_events(events);
        Self {
            session_id,
            model,
            turns,
            steps,
            tool_calls,
            compactions,
            usage,
            estimated_tokens: estimated,
            context_window_tokens,
        }
    }

    /// 渲染快照行集（非 durable，不进 transcript）。
    pub fn render(&self) -> Vec<String> {
        let mut lines = Vec::new();
        lines.push("── 会话快照 ─────────────────────────".to_string());
        lines.push(format!("会话 id   {}", self.session_id));
        lines.push(format!("模型      {}", self.model));
        lines.push(format!(
            "生命周期  turns {} · steps {} · tool calls {} · compactions {}",
            self.turns, self.steps, self.tool_calls, self.compactions
        ));
        lines.push(format!(
            "usage     input {} · output {} · cache_read {} · cache_write {}",
            self.usage.input_tokens,
            self.usage.output_tokens,
            self.usage.cache_read_tokens,
            self.usage.cache_write_tokens
        ));
        let denom = self.usage.input_tokens + self.usage.cache_read_tokens;
        let hit = if denom == 0 {
            "—".to_string()
        } else {
            format!("{:.1}%", self.usage.cache_read_tokens as f64 / denom as f64 * 100.0)
        };
        lines.push(format!("KV 命中率  {hit}"));
        let ratio = if self.context_window_tokens == 0 {
            0.0
        } else {
            self.estimated_tokens as f64 / self.context_window_tokens as f64
        };
        lines.push(format!(
            "上下文    {} / {} tokens（{:.1}%）",
            self.estimated_tokens, self.context_window_tokens,
            ratio * 100.0
        ));
        lines
    }
}

#[cfg(test)]
#[path = "status_tests.rs"]
mod tests;
