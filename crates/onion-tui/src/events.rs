//! events.rs：LiveEvent → TUI 动作。
//!
//! 事件泵订阅 `EventBus`（broadcast 256），将 `LiveEvent` 消费为组件树操作；
//! `Durable(SessionEvent)` 交给 transcript 投影 fold，`AssistantStream` 流帧
//! 更新 pending 块，`StateChanged` 更新状态行。

use std::sync::Arc;

use onion::runtime::Runtime;
use onion::session::{LiveEvent, StreamChunk};

use crate::transcript::Transcript;

/// 事件泵：订阅 live 总线，把事件翻译为组件树操作。
pub struct EventPump {
    runtime: Arc<Runtime>,
    transcript: Transcript,
}

impl EventPump {
    pub fn new(runtime: Arc<Runtime>) -> Self {
        Self {
            runtime,
            transcript: Transcript::new(),
        }
    }

    /// resume 路径：从 durable 日志全量重放。
    pub fn replay(&mut self) {
        let events = self.runtime.session().snapshot();
        self.transcript.replay(&events);
    }

    /// 运行中增量路径：处理一条 live 事件。
    pub fn handle(&mut self, event: LiveEvent) {
        match event {
            LiveEvent::Durable(event) => self.transcript.apply(&event),
            // 流帧：占位 assistant 块 + 增量文本逐字上屏（打字机）；durable
            // `AssistantMessage` 落定时原地替换为权威文本。
            LiveEvent::AssistantStream(chunk) => match chunk {
                StreamChunk::Start { turn, step } => self.transcript.begin_stream(turn, step),
                StreamChunk::Text(text) => self.transcript.push_stream_text(&text),
                StreamChunk::Thinking(thinking) => self.transcript.push_stream_thinking(&thinking),
                // 工具参数增量不单独渲染（durable `ToolCall` 落定即出卡片）。
                StreamChunk::ToolCallStart { .. }
                | StreamChunk::ToolCallInput { .. }
                | StreamChunk::End => {}
            },
            LiveEvent::StateChanged { .. } => {}
            // inbox 提示（steer/inject/followup 徽标）暂不渲染。
            LiveEvent::InboxInserted { .. } | LiveEvent::InboxClaimed { .. } => {}
            // subagent 生命周期事件（start / end）暂不渲染（会话内已有 relay 结果）。
            LiveEvent::SubagentStart { .. } | LiveEvent::SubagentEnd { .. } => {}
        }
    }

    pub fn transcript(&self) -> &Transcript {
        &self.transcript
    }

    /// 视图级清空（`Ctrl+U`）：只清投影，durable 日志不动。
    pub fn clear_transcript(&mut self) {
        self.transcript.clear_view();
    }
}
