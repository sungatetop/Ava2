//! `runtime` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。
//! 重点回归：`set_model_target_sync` 必须在 async 上下文可用——装配发生在
//! `#[tokio::main]` 驱动线程上，曾用 `blocking_lock` 导致 TUI/CLI 启动即 panic。

use std::sync::Arc;
use std::time::Duration;

use super::*;
use crate::context::RequestDraft;
use crate::llm::{LlmPort, LlmStream, LlmStreamItem};
use crate::system_prompt::SystemPrompt;
use crate::waterfall::BoxFuture;

/// 哑 LlmPort：本组测试不跑轮，被调用即返回错误落定。
struct UnusedLlm;

impl LlmPort for UnusedLlm {
    fn stream(&self, _draft: &RequestDraft, _cancel: CancellationToken) -> BoxFuture<LlmStream> {
        Box::pin(async move {
            let turn = crate::llm::AssistantTurn::error("unused", "unexpected llm call", false);
            Box::pin(futures_util::stream::iter(vec![LlmStreamItem::Done(turn)])) as LlmStream
        })
    }
}

fn test_config() -> Config {
    Config {
        model: "fake".to_string(),
        base_url: String::new(),
        api_key: String::new(),
        max_tokens: 100,
        temperature: None,
        max_retries: 1,
        tool_timeout: Duration::from_secs(5),
        max_parallel_tools: 10,
        include_identity: false,
        include_runtime_context: true,
        persona_prefix: String::new(),
        persona_suffix: String::new(),
        tool_order: None,
        context_window_tokens: 128_000,
        compact_threshold_ratio: 0.8,
        compact_retain_ratio: 0.16,
        compact_max_retries: 1,
        reserved_completion_tokens: 2048,
        compact_headroom_tokens: 65536,
        compact_model_policies: Vec::new(),
        workspace_enabled: false,
        workspace_max_bytes: 65_536,
        approval_preset: "workspace-write".to_string(),
        allow_parallel_in_progress: false,
        web_search_base_url: String::new(),
        web_search_model: String::new(),
        web_search_max_results: 8,
        web_search_max_queries: 4,
        web_fetch_max_output_chars: 200_000,
        session_query_max_results: 100,
        present_max_files: 8,
        terminal_enable_background: true,
        terminal_max_result_chars: 32_768,
        sandbox_executable: None,
        mcp_servers_config: None,
        workspace_deps_source: None,
        agent_preset: "coding".to_string(),
        agents_dir: None,
        subagent_model_selection: None,
        agent_team: None,
        a2a_token: None,
    }
}

fn build_runtime() -> Runtime {
    let config = test_config();
    let prompts = SystemPrompt::new(&config);
    let tools = ToolRegistry::new();
    let context = Context { system: None, messages: Vec::new(), tools: tools.definitions() };
    Runtime::in_memory(config, prompts, context, Arc::new(UnusedLlm), tools)
}

fn target(model: &str) -> Arc<crate::model_target::ModelTargetRef> {
    Arc::new(crate::model_target::ModelTargetRef::new(crate::model_target::ModelTarget {
        model: model.to_string(),
        base_url: "http://localhost".to_string(),
        api_key: "k".to_string(),
        effort: None,
    }))
}

#[tokio::test]
async fn set_model_target_sync_works_inside_async_context() {
    // 旧实现（blocking_lock）在 tokio::test 驱动线程上 panic：
    // "Cannot block the current thread from within a runtime"。
    let runtime = build_runtime();
    runtime.set_model_target_sync(target("m2")).expect("装配期锁空闲，应成功");
    let current = runtime.model_target().await.current().expect("应有初始目标");
    assert_eq!(current.model, "m2");
}

#[tokio::test]
async fn set_model_target_sync_fails_loud_when_lock_held() {
    let runtime = build_runtime();
    // 模拟并发持有：锁被占时同步变体应 fail-loud 返回错误，而非 panic。
    let guard = runtime.kernel.lock().await;
    assert!(runtime.set_model_target_sync(target("m2")).is_err());
    drop(guard);
    // 释放后恢复可用。
    runtime.set_model_target_sync(target("m3")).expect("锁已释放，应成功");
    let current = runtime.model_target().await.current().expect("应有初始目标");
    assert_eq!(current.model, "m3");
}
