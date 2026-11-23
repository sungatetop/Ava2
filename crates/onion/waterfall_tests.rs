//! `waterfall` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::sync::{Arc, Mutex};

use super::*;
use crate::listener::{ClaimedInput, ClaimSource, Listener, ListenerCtx, PreStepDecision};
use crate::types::AgentMessage;

#[derive(Default)]
struct Trace(Mutex<Vec<String>>);

impl Trace {
    fn push(&self, entry: impl Into<String>) {
        self.0.lock().unwrap().push(entry.into());
    }
    fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.0.lock().unwrap())
    }
}

fn test_ctx() -> ListenerCtx {
    let session = Arc::new(crate::session::SessionLog::in_memory());
    let bus = session.bus().clone();
    ListenerCtx {
        inbox: Arc::new(crate::inbox::Inbox::new(bus)),
        session,
        state: "Idle".to_string(),
    }
}

fn test_input() -> ClaimedInput {
    ClaimedInput {
        turn: 1,
        step: 1,
        source: ClaimSource::TurnOpen,
        messages: vec![AgentMessage::user("hi")],
        starts_request_series: true,
    }
}

struct Layer {
    name: &'static str,
    trace: Arc<Trace>,
}

impl Listener for Layer {
    fn pre_step(
        &self,
        _ctx: ListenerCtx,
        input: ClaimedInput,
        next: Next<ClaimedInput, PreStepDecision>,
    ) -> BoxFuture<PreStepDecision> {
        let trace = self.trace.clone();
        let name = self.name;
        Box::pin(async move {
            trace.push(format!("enter-{name}"));
            let decision = next(input).await;
            trace.push(format!("exit-{name}"));
            decision
        })
    }
}

struct Blocker;

impl Listener for Blocker {
    fn pre_step(
        &self,
        _ctx: ListenerCtx,
        _input: ClaimedInput,
        _next: Next<ClaimedInput, PreStepDecision>,
    ) -> BoxFuture<PreStepDecision> {
        Box::pin(async { PreStepDecision::Reject { reason: "blocked".to_string() } })
    }
}

#[tokio::test]
async fn lifo_order() {
    let trace = Arc::new(Trace::default());
    let listeners: Vec<Arc<dyn Listener>> = vec![
        Arc::new(Layer { name: "a", trace: trace.clone() }),
        Arc::new(Layer { name: "b", trace: trace.clone() }),
    ];
    let trace2 = trace.clone();
    let decision = fold(
        &listeners,
        test_ctx(),
        test_input(),
        move |input| {
            let trace = trace2;
            Box::pin(async move {
                trace.push("terminal");
                PreStepDecision::Enter {
                    messages: input.messages,
                    starts_request_series: input.starts_request_series,
                }
            })
        },
        |listener, ctx, input, next| listener.pre_step(ctx, input, next),
    )
    .await;
    assert!(matches!(decision, PreStepDecision::Enter { .. }));
    assert_eq!(trace.take(), vec!["enter-a", "enter-b", "terminal", "exit-b", "exit-a"]);
}

#[tokio::test]
async fn short_circuit_skips_inner_and_terminal() {
    let trace = Arc::new(Trace::default());
    let listeners: Vec<Arc<dyn Listener>> = vec![
        Arc::new(Blocker),
        Arc::new(Layer { name: "inner", trace: trace.clone() }),
    ];
    let trace2 = trace.clone();
    let decision = fold(
        &listeners,
        test_ctx(),
        test_input(),
        move |_| {
            let trace = trace2;
            Box::pin(async move {
                trace.push("terminal");
                PreStepDecision::Enter {
                    messages: Vec::new(),
                    starts_request_series: false,
                }
            })
        },
        |listener, ctx, input, next| listener.pre_step(ctx, input, next),
    )
    .await;
    assert!(matches!(decision, PreStepDecision::Reject { .. }));
    // 短路：内层 listener 与终端均未执行
    assert!(trace.take().is_empty());
}

#[tokio::test]
async fn passthrough_with_no_listeners() {
    let listeners: Vec<Arc<dyn Listener>> = vec![];
    let decision = fold(
        &listeners,
        test_ctx(),
        test_input(),
        |input| {
            Box::pin(async move {
                PreStepDecision::Enter {
                    messages: input.messages,
                    starts_request_series: input.starts_request_series,
                }
            })
        },
        |listener, ctx, input, next| listener.pre_step(ctx, input, next),
    )
    .await;
    match decision {
        PreStepDecision::Enter { messages, starts_request_series } => {
            assert_eq!(messages.len(), 1);
            assert!(starts_request_series);
        }
        PreStepDecision::Reject { .. } => panic!("terminal 默认应为 enter"),
    }
}
