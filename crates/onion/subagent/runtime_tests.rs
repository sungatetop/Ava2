//! `subagent/runtime` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。
//!
//! 覆盖三条关键契约：
//! - 血缘落盘：spawn 在子会话日志首部写 `SubagentDescriptor`（父会话 id / 深度 / 名字）；
//! - 续聊：子代理 settle 后 `send` 在同一 Runtime 上重新开轮，未知 id 报 `Unknown`；
//! - 冷恢复：进程重启后按日志 `restore` 续聊，`one-shot` 描述符报 `NotResumable`。
//!
//! 真实模型链路由 `examples/subagent_real_smoke.rs` 覆盖；本文件用计数哑 LLM 只验证
//! 「是否真的重开一轮」这类本地不变量。

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use super::*;
use crate::context::RequestDraft;
use crate::llm::{AssistantTurn, LlmPort, LlmStream, LlmStreamItem};
use crate::types::{AgentMessage, ContentBlock, StopReason, Usage};
use crate::waterfall::BoxFuture;

/// 计数哑 LLM：每次请求返回一条固定文本（不触网），用于验证续聊是否真的重开一轮。
struct CountingLlm {
    calls: Arc<AtomicUsize>,
}

impl LlmPort for CountingLlm {
    fn stream(
        &self,
        _draft: &RequestDraft,
        _cancel: tokio_util::sync::CancellationToken,
    ) -> BoxFuture<LlmStream> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            let turn = AssistantTurn {
                message: AgentMessage::assistant(vec![ContentBlock::Text {
                    text: "done".to_string(),
                }]),
                stop_reason: StopReason::EndTurn,
                error: None,
                failure: None,
                cancelled: false,
                usage: Usage::default(),
                model: "fake".to_string(),
            };
            Box::pin(futures_util::stream::iter(vec![LlmStreamItem::Done(turn)])) as LlmStream
        })
    }
}

/// 测试用 runtime：落盘到临时目录（血缘与冷恢复都需要 durable 日志）。
fn runtime(dir: &std::path::Path, calls: Arc<AtomicUsize>) -> Arc<SubagentRuntime> {
    let config = Config::from_env();
    let factory_config = config.clone();
    Arc::new(SubagentRuntime::new(
        1,
        config,
        Arc::new(CountingLlm { calls }),
        Arc::new(move |_cwd: Option<&str>, _preset: Option<&str>| {
            Ok(ChildSession::new(&factory_config, ToolRegistry::new()))
        }),
        Some(dir.to_path_buf()),
    ))
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("onion_subagent_{name}_{}", std::process::id()))
}

/// 读子会话日志里的 `SubagentDescriptor` 事件（血缘权威）。
fn descriptor_of(dir: &std::path::Path, session_id: &str) -> Option<crate::session::SessionEvent> {
    let log = crate::paths::session_log_file(&crate::paths::session_dir(dir, session_id));
    let content = std::fs::read_to_string(log).ok()?;
    content.lines().find_map(|line| {
        let parsed = serde_json::from_str::<serde_json::Value>(line).ok()?;
        let event = serde_json::from_value::<crate::session::SessionEvent>(
            parsed.get("event")?.clone(),
        )
        .ok()?;
        matches!(event, crate::session::SessionEvent::SubagentDescriptor { .. }).then_some(event)
    })
}

/// 轮询等待血缘事件落盘（spawn 立即 append，回放读取需文件已 flush）。
async fn wait_for_descriptor(
    dir: &std::path::Path,
    session_id: &str,
) -> crate::session::SessionEvent {
    for _ in 0..50 {
        if let Some(event) = descriptor_of(dir, session_id) {
            return event;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("子会话日志未落 SubagentDescriptor 血缘事件");
}

/// 等待子代理 settle（在飞归零后留一拍余量，等日志写完）。
async fn wait_settled(runtime: &SubagentRuntime) {
    for _ in 0..200 {
        if runtime.inflight() == 0 {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("子代理未在预期时间内 settle");
}

/// 等待旧写者的会话租约释放（进程重启语义：旧进程句柄全部关闭后才能再获取）。
/// settle 驱动任务经 tokio::spawn 持有子 Runtime 的 Arc，drop 父句柄不会同步释放。
async fn wait_lease_free(dir: &std::path::Path, session_id: &str) {
    let session_dir = crate::paths::session_dir(dir, session_id);
    for _ in 0..100 {
        if let Ok(lease) = crate::session_lock::SessionWriteLease::acquire(&session_dir) {
            drop(lease);
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("旧写者租约 2 秒内未释放");
}

/// Agent preset persona 里的 `{{model}}` 必须在子会话可插值：漏注册会让
/// `SystemPrompt::assemble` 直接失败（开轮被拒），表现为子代理「无输出」。
#[test]
fn child_prompts_interpolate_model_variable() {
    let config = Config::from_env();
    let child = ChildSession::with_directive(
        &config,
        ToolRegistry::new(),
        Some("You are running as {{model}}."),
    );
    let assembly = child.prompts.assemble().expect("子会话提示词组装不应失败");
    let text = crate::system_prompt::render_prompt(&assembly).expect("渲染");
    assert!(text.contains(&config.model), "persona 应插值出模型名：{text}");
}

#[tokio::test]
async fn spawn_writes_descriptor_lineage() {
    let dir = temp_dir("lineage");
    let calls = Arc::new(AtomicUsize::new(0));
    let runtime = runtime(&dir, calls);
    let options = DelegationOptions {
        description: Some("写一份报告".to_string()),
        ..DelegationOptions::default()
    };

    let activation = runtime
        .spawn_delegated("do it".to_string(), 1, None, options)
        .expect("spawn");

    let event = wait_for_descriptor(&dir, &activation.id).await;
    let crate::session::SessionEvent::SubagentDescriptor {
        mode,
        name,
        label,
        depth,
        delegation_mode,
        ..
    } = event
    else {
        panic!("子会话日志首条血缘事件应为 SubagentDescriptor");
    };
    assert_eq!(mode, "continuable");
    assert_eq!(name.as_deref(), Some(activation.name.as_str()));
    assert_eq!(label.as_deref(), Some("写一份报告"));
    assert_eq!(depth, 1);
    assert_eq!(delegation_mode.as_deref(), Some("spawn"), "spawn 委派方式须留痕");
    let _ = std::fs::remove_dir_all(&dir);
}

/// fork 委派同样留痕（`delegationMode: 'fork'`）：两者血缘可区分。
#[tokio::test]
async fn fork_writes_delegation_mode() {
    let dir = temp_dir("fork-mode");
    let calls = Arc::new(AtomicUsize::new(0));
    let runtime = runtime(&dir, calls);
    let parent = crate::session::SessionLog::open(&dir).expect("parent log");

    let activation = runtime
        .fork_delegated(&parent, "go on".to_string(), 1, None, DelegationOptions::default())
        .expect("fork");

    let event = wait_for_descriptor(&dir, &activation.id).await;
    let crate::session::SessionEvent::SubagentDescriptor { delegation_mode, .. } = event else {
        panic!("子会话日志首条血缘事件应为 SubagentDescriptor");
    };
    assert_eq!(delegation_mode.as_deref(), Some("fork"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn send_after_settle_resumes_same_child() {
    let dir = temp_dir("resume");
    let calls = Arc::new(AtomicUsize::new(0));
    let runtime = runtime(&dir, calls.clone());

    let activation = runtime
        .spawn_delegated("first".to_string(), 1, None, DelegationOptions::default())
        .expect("spawn");
    wait_settled(&runtime).await;
    assert_eq!(calls.load(Ordering::SeqCst), 1, "首轮应恰好一次模型请求");

    // settle 后继续派任务：同一 Runtime 重新开轮（continuable 语义）。
    let resumed = runtime
        .send(&activation.id, AgentMessage::user("second"), Delivery::Steer)
        .expect("send after settle");
    assert!(resumed, "已 settle 的子代理 send 应重新开轮");
    wait_settled(&runtime).await;
    assert_eq!(calls.load(Ordering::SeqCst), 2, "续聊应再发一次模型请求");
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn send_unknown_agent_is_rejected() {
    let dir = temp_dir("unknown");
    let calls = Arc::new(AtomicUsize::new(0));
    let runtime = runtime(&dir, calls);

    let error = runtime
        .send("no-such-agent", AgentMessage::user("hi"), Delivery::Steer)
        .expect_err("未知子会话必须拒绝");
    assert!(matches!(error, SubagentError::Unknown(id) if id == "no-such-agent"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// 冷恢复：进程重启后内存句柄表为空，但子会话日志仍在，
/// `send_message` 应能按 id 恢复并续聊（复用同 id，历史由回放重建）。
#[tokio::test]
async fn cold_restore_resumes_settled_child() {
    let dir = temp_dir("cold-restore");
    let calls = Arc::new(AtomicUsize::new(0));
    let first = runtime(&dir, calls.clone());

    let activation = first
        .spawn_delegated("first".to_string(), 1, None, DelegationOptions::default())
        .expect("spawn");
    wait_settled(&first).await;
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    // 模拟进程重启：释放旧写者并等租约真正释放（旧进程句柄全部关闭），
    // 再在同一日志目录上新建 runtime（children 句柄表为空）。
    drop(first);
    wait_lease_free(&dir, &activation.id).await;
    let restarted = runtime(&dir, calls.clone());
    let resumed = restarted
        .send_or_restore(&activation.id, AgentMessage::user("second"), Delivery::Steer)
        .expect("冷恢复后应可续聊");
    assert!(resumed, "冷恢复后的首次投递必须重新开轮");
    wait_settled(&restarted).await;
    assert_eq!(calls.load(Ordering::SeqCst), 2, "冷恢复续聊应再发一次模型请求");
    assert_eq!(restarted.child_name(&activation.id), Some(activation.name.clone()));
    let _ = std::fs::remove_dir_all(&dir);
}

/// `one-shot` 描述符不可恢复（`NOT_RESUMABLE`）：只提示重新派生。
#[tokio::test]
async fn cold_restore_rejects_one_shot_descriptor() {
    let dir = temp_dir("cold-oneshot");
    let calls = Arc::new(AtomicUsize::new(0));
    let log = crate::session::SessionLog::open(&dir).expect("open");
    let session_id = log.session_id().to_string();
    log.append(crate::session::SessionEvent::SubagentDescriptor {
        version: crate::session::SUBAGENT_DESCRIPTOR_VERSION,
        mode: "one-shot".to_string(),
        provider: "session".to_string(),
        name: Some("legacy".to_string()),
        label: None,
        agent_model: None,
        agent_reasoning_effort: None,
        cwd: None,
        parent_session_id: "parent-cold".to_string(),
        depth: 1,
        delegation_mode: None,
    });
    drop(log);

    let runtime = runtime(&dir, calls);
    let error = runtime
        .send_or_restore(&session_id, AgentMessage::user("hi"), Delivery::Steer)
        .expect_err("one-shot 子代理不可恢复");
    assert!(
        matches!(error, SubagentError::NotResumable(ref id) if id == &session_id),
        "错误应为 NotResumable，实际 {error:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
