//! Listener trait：洋葱外层监听器 + 各边界决策类型。
//!
//! 所有瀑布方法默认 `next(input)`（纯透传 = 观测）；覆写即在该边界拦截。
//! `turn_stopping` 边界：串行、无 next()、无决策，
//! listener 在窗口内经 `ctx.inbox` 注入下一条 step 输入。

use std::sync::Arc;

use crate::context::RequestDraft;
use crate::inbox::Inbox;
use crate::session::{LiveEvent, SessionLog};
use crate::types::{AgentMessage, ToolCall, ToolResult};
use crate::waterfall::{BoxFuture, Next};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClaimSource {
    /// 开轮 Claiming（外部输入 + next-turn 队首）。
    TurnOpen,
    /// step 续跑 Admitting（工具欠请求 / next-step 整批 / turn-stopping 续跑）。
    NextStep,
}

/// pre-step 瀑布输入：本步领取的批次快照。
#[derive(Clone, Debug)]
pub struct ClaimedInput {
    pub turn: u32,
    pub step: u32,
    pub source: ClaimSource,
    pub messages: Vec<AgentMessage>,
    pub starts_request_series: bool,
}

pub enum PreStepDecision {
    Enter { messages: Vec<AgentMessage>, starts_request_series: bool },
    /// 拒绝本步：领取的消息保持移除（持久取消），空轮关闭不开 step。
    Reject { reason: String },
}

/// request 瀑布输出：路由确认后的请求草稿。
pub struct StepRoute {
    pub draft: RequestDraft,
}

/// 请求错误（落定失败/传输失败/取消），进 request-error 瀑布。
#[derive(Clone)]
pub struct RequestError {
    pub message: String,
    pub cancelled: bool,
    /// 结构化失败分类（adapter 侧失败归类；None = 未分类裸错误轮，
    /// 仅出现在测试 Fake 中——内核链尾按「立即重试」兜底）。
    pub failure: Option<crate::llm::LlmFailure>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RetryDecision {
    /// 重试（`delay_ms` = 退避等待；0 = 立即重试）。
    Retry { delay_ms: u64 },
    Abort,
}

pub enum ToolAdmission {
    Allow { call: ToolCall },
    /// 拒绝执行：合成错误 ToolResult，维持 assistant tool_use / tool_result 配对。
    Deny { call: ToolCall, reason: String },
}

/// 注入句柄 + 只读视图（全 owned，future 取 'static 无生命周期传染）。
#[derive(Clone)]
pub struct ListenerCtx {
    pub inbox: Arc<Inbox>,
    pub session: Arc<SessionLog>,
    /// 状态机快照字符串（如 "Running(Stepping(Streaming))"）。
    pub state: String,
}

/// 同步事件扇出回调（调用方在发射点同步 invoke，异常被包含）。
/// 由 Runtime 装配期注入 SessionLog / Inbox / Machine，事件经它到达全部 listener。
pub type EventSink = Arc<dyn Fn(&LiveEvent) + Send + Sync>;

/// 洋葱外层监听器。
pub trait Listener: Send + Sync {
    /// 纯观测：全部 live 事件（durable 镜像 + inbox + 状态 + 流帧）。
    fn on_event(&self, _ctx: &ListenerCtx, _event: &LiveEvent) {}

    /// 步准入瀑布（step 开始前拦截点）。
    fn pre_step(
        &self,
        _ctx: ListenerCtx,
        input: ClaimedInput,
        next: Next<ClaimedInput, PreStepDecision>,
    ) -> BoxFuture<PreStepDecision> {
        next(input)
    }

    /// 请求装配瀑布：可改写草稿（max_tokens 等）。
    fn request(
        &self,
        _ctx: ListenerCtx,
        draft: RequestDraft,
        next: Next<RequestDraft, StepRoute>,
    ) -> BoxFuture<StepRoute> {
        next(draft)
    }

    /// 请求错误瀑布：重试或放弃。
    fn request_error(
        &self,
        _ctx: ListenerCtx,
        error: RequestError,
        next: Next<RequestError, RetryDecision>,
    ) -> BoxFuture<RetryDecision> {
        next(error)
    }

    /// 工具准入瀑布（执行前拦截点）。
    fn tool(
        &self,
        _ctx: ListenerCtx,
        call: ToolCall,
        next: Next<ToolCall, ToolAdmission>,
    ) -> BoxFuture<ToolAdmission> {
        next(call)
    }

    /// 工具执行后回调。无决策，纯观测/注入：
    /// listener 可经 `ctx.inbox.steer()` 携带 feedback 与 additionalContext
    /// 注入下一次 step。默认空实现，对既有 listener 零影响。
    fn tool_post(&self, _ctx: ListenerCtx, _call: &ToolCall, _result: &ToolResult) -> BoxFuture<()> {
        Box::pin(async {})
    }

    /// 轮收尾检查（turn-stopping 边界）：串行、无 next()、无决策。
    /// listener 可经 `ctx.inbox.steer()` 注入下一条 step 输入（最后注入窗口）。
    fn turn_stopping(&self, _ctx: ListenerCtx) -> BoxFuture<()> {
        Box::pin(async {})
    }
}
