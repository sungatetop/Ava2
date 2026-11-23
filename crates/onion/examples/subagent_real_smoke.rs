//! subagent 委派 / 续聊真实 API 冒烟。
//!
//! 验证六件事（真实模型调用，走 `Config::from_env`，缺省 local 供应商）：
//! 1. 模型能按 schema 调用 `subagent {prompt, name, cwd, description}` 派生真实子会话；
//! 2. `wait_agent` 等调用方 inbox 的下一个 step 输入：子代理回报到达即返回结构化结论；
//! 3. 子代理 settle 后父会话被唤醒（`parent_wake` 回调确实触发）；
//! 4. `cwd` 越界（`..` 逃逸 / 非目录）被宿主工厂拒绝；
//! 5. 子会话日志留下 `subagent_descriptor` 血缘（name / parent / depth / mode）；
//! 6. 续聊（continuable）：`send_or_restore` 让子会话再开一轮，
//!    日志新增 `turn_start`，真实模型再次产出 `assistant_message`。
//!
//! 运行：
//! ```powershell
//! $env:ONION_HOME="<仓库根>\.onion-test"
//! cargo run -p onion --example subagent_real_smoke
//! ```

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use onion::config::Config;
use onion::inbox::Inbox;
use onion::llm::OpenAiSseLlm;
use onion::provider_catalog::{ModelEntry, ProviderCatalog, ProviderEntry};
use onion::runtime::Runtime;
use onion::session::SessionEvent;
use onion::session_cwd::SessionCwd;
use onion::subagent::model_selection::AllowedModelRoute;
use onion::subagent::relay::Delivery;
use onion::subagent::runtime::{ChildSession, SubagentRuntime};
use onion::system_prompt::SystemPrompt;
use onion::tools::subagent::SubagentToolSet;
use onion::tools::ToolRegistry;
use onion::types::{AgentMessage, ContentBlock};

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// 子会话工厂：按宿主规则解析委派 cwd（越界 / 不存在一律拒绝）。
type ChildFactory =
    dyn Fn(Option<&str>, Option<&str>) -> Result<ChildSession, String> + Send + Sync;

#[tokio::main]
async fn main() {
    let workspace = std::env::temp_dir().join(format!("onion_real_smoke_{}", std::process::id()));
    let log_dir = workspace.join("sessions");
    if let Err(error) = std::fs::create_dir_all(&workspace) {
        println!("[smoke] 无法创建临时工作区 {}：{error}", workspace.display());
        std::process::exit(1);
    }
    if let Err(error) = std::fs::create_dir_all(workspace.join("docs")) {
        println!("[smoke] 无法创建 docs 目录：{error}");
        std::process::exit(1);
    }

    let mut config = Config::from_env();
    config.include_identity = false;
    config.workspace_enabled = false;
    println!("[smoke] provider {} model {}", config.base_url, config.model);

    // ── 子会话工厂：按宿主规则解析委派 cwd（越界 / 不存在一律拒绝） ──
    let factory_root = workspace.clone();
    let factory_config = config.clone();
    let seen_cwd: Arc<Mutex<Vec<Option<PathBuf>>>> = Arc::new(Mutex::new(Vec::new()));
    let factory_seen = seen_cwd.clone();
    let factory: Arc<ChildFactory> =
        Arc::new(move |delegated_cwd: Option<&str>, _preset: Option<&str>| {
            let initial = match delegated_cwd {
                Some(raw) => {
                    let probe = SessionCwd::new(factory_root.clone());
                    let resolved = probe.resolve(raw)?;
                    if !resolved.is_dir() {
                        return Err(format!("subagent cwd does not exist: {raw}"));
                    }
                    Some(resolved)
                }
                None => None,
            };
            lock(&factory_seen).push(initial);
            // 子代理工具面留空：本冒烟只验证委派链路，不让子代理再调工具。
            Ok(ChildSession::new(&factory_config, ToolRegistry::new()))
        });

    // ── 子代理运行时（真实 API；落盘以便校验子会话日志） ──
    let subagents = Arc::new(SubagentRuntime::new(
        1,
        config.clone(),
        Arc::new(OpenAiSseLlm::new(config.base_url.clone(), config.api_key.clone())),
        factory.clone(),
        Some(log_dir.clone()),
    ));

    // 父会话唤醒计数：子代理 settle 回投后必须触发（否则父会话 idle 时结果滞留）。
    let wakes = Arc::new(AtomicUsize::new(0));
    {
        let wakes = wakes.clone();
        subagents.set_parent_wake(Arc::new(move || {
            wakes.fetch_add(1, Ordering::SeqCst);
        }));
    }
    // settle 钩子：记录子会话 id（续聊断言需要按 id 定位）。
    let settled_ids: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    {
        let settled_ids = settled_ids.clone();
        subagents.add_settle_hook(Arc::new(move |id| lock(&settled_ids).push(id.to_string())));
    }

    let mut catalog = ProviderCatalog::builtin(&config.model);
    catalog.providers = vec![ProviderEntry {
        id: "local".to_string(),
        name: "local".to_string(),
        base_url: config.base_url.clone(),
        api_key: config.api_key.clone(),
        models: vec![ModelEntry {
            id: config.model.clone(),
            name: config.model.clone(),
            description: None,
            reasoning: None,
        }],
    }];

    // ── 父会话：注册 subagent 工具族 → 装配 Runtime → 回接 inbox/log ──
    let toolset = SubagentToolSet::new(
        subagents.clone(),
        catalog,
        AllowedModelRoute::new("local", config.model.clone()),
        Vec::new(),
    );
    let mut registry = ToolRegistry::new();
    toolset.register_all(&mut registry);
    // 工具面白名单：`definitions()` 按 agent 白名单过滤，未设白名单时为空，
    // 必须先放行已注册工具（装配点专用 API）。
    registry.allow_all_registered();
    let context = onion::context::Context {
        system: None,
        messages: Vec::new(),
        tools: registry.definitions(),
    };
    let parent = Runtime::in_memory(
        config.clone(),
        SystemPrompt::new(&config),
        context,
        Arc::new(OpenAiSseLlm::new(config.base_url.clone(), config.api_key.clone())),
        registry,
    );
    {
        let inbox: Arc<Inbox> = parent.inbox();
        let log = parent.session();
        toolset.wire_parent(inbox, log);
    }

    let prompt = "You must use tools, in this exact order:\n\
         1. Call `subagent` with arguments {\"prompt\":\"Reply with the single word OK, then stop.\",\"name\":\"docs-scout\",\"cwd\":\"docs\",\"description\":\"smoke test\"}.\n\
         2. Immediately call `wait_agent` with arguments {} (it takes no agent argument).\n\
         3. Then reply with the single word DONE and stop."
    .to_string();

    println!("[smoke] 驱动父会话（真实模型）…");
    let turn = match tokio::time::timeout(
        Duration::from_secs(300),
        parent.run(vec![AgentMessage::user(prompt)]),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => {
            println!("[smoke] FAIL 父会话超时（300s），现场保留：{}", workspace.display());
            std::process::exit(1);
        }
    };
    match &turn {
        Ok(Some(report)) => println!("[smoke] 父会话收轮：steps={} reason={:?}", report.steps, report.reason),
        Ok(None) => println!("[smoke] 父会话收轮：无报告"),
        Err(error) => println!("[smoke] 父会话出错：{error}"),
    }

    // ── 断言 ──
    let events = parent.session().snapshot();
    let calls: Vec<String> = events
        .iter()
        .filter_map(|event| match event {
            SessionEvent::ToolCall { call, .. } => Some(call.name.clone()),
            _ => None,
        })
        .collect();
    let results: Vec<(String, bool, String)> = events
        .iter()
        .filter_map(|event| match event {
            SessionEvent::ToolResult { result, .. } => Some((
                result.name.clone(),
                result.is_error,
                result
                    .content
                    .iter()
                    .filter_map(|block| match block {
                        ContentBlock::Text { text } => Some(text.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join(""),
            )),
            _ => None,
        })
        .collect();

    println!("\n[smoke] 工具调用序列：{calls:?}");
    for (name, is_error, text) in &results {
        println!("[smoke] 结果 {name} err={is_error} → {text}");
    }

    let mut failures: Vec<String> = Vec::new();
    if !calls.iter().any(|name| name == "subagent") {
        failures.push("模型未调用 subagent".to_string());
    }
    if !calls.iter().any(|name| name == "wait_agent") {
        failures.push("模型未调用 wait_agent".to_string());
    }
    if calls.iter().any(|name| name == "list_agents") {
        failures.push("模型退化为 list_agents 轮询".to_string());
    }
    for (name, is_error, text) in &results {
        if *is_error {
            failures.push(format!("工具 {name} 报错：{text}"));
        }
    }

    // 子代理可能晚于父轮次 settle：等待 settle 钩子与 parent_wake（最多 180s）。
    let deadline = std::time::Instant::now() + Duration::from_secs(180);
    while (settled_ids_empty(&settled_ids) || wakes.load(Ordering::SeqCst) == 0)
        && std::time::Instant::now() < deadline
    {
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    if settled_ids_empty(&settled_ids) {
        failures.push("子代理未在预期时间内 settle".to_string());
    }
    if wakes.load(Ordering::SeqCst) == 0 {
        failures.push("子代理 settle 后父会话未被唤醒（parent_wake 未触发）".to_string());
    } else {
        println!("[smoke] parent_wake 触发次数：{}", wakes.load(Ordering::SeqCst));
    }

    let child_id = lock(&settled_ids).first().cloned().unwrap_or_default();

    // ── 续聊（continuable）：真实模型再开一轮 ──
    if child_id.is_empty() {
        failures.push("无子会话 id，跳过续聊断言".to_string());
    } else {
        let turns_before = turn_count(&log_dir, &child_id);
        match subagents.send_or_restore(
            &child_id,
            AgentMessage::user("Reply with the single word RESUMED, then stop."),
            Delivery::Steer,
        ) {
            Ok(true) => {}
            Ok(false) => failures.push("settle 后的子代理 send_or_restore 未重新开轮".to_string()),
            Err(error) => failures.push(format!("续聊投递失败：{error}")),
        }
        // 等续聊轮 settle（在飞归零）。
        let resume_deadline = std::time::Instant::now() + Duration::from_secs(180);
        while subagents.inflight() > 0 && std::time::Instant::now() < resume_deadline {
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        let turns_after = turn_count(&log_dir, &child_id);
        println!("[smoke] 子会话轮次：续聊前 {turns_before} → 续聊后 {turns_after}");
        if turns_after <= turns_before {
            failures.push(format!("续聊未在子会话日志开出新一轮（{turns_before} → {turns_after}）"));
        }
    }

    // 子会话必须留下 SubagentDescriptor（血缘）+ 真实模型产出的 assistant 消息。
    let mut child_logs = 0;
    let mut child_text = String::new();
    let mut lineage_ok = false;
    if let Ok(entries) = std::fs::read_dir(&log_dir) {
        for entry in entries.flatten() {
            let Ok(files) = std::fs::read_dir(entry.path()) else {
                continue;
            };
            let content: String = files
                .flatten()
                .filter(|file| file.path().extension().is_some_and(|ext| ext == "jsonl"))
                .filter_map(|file| std::fs::read_to_string(file.path()).ok())
                .collect();
            if content.is_empty() {
                continue;
            }
            child_logs += 1;
            for line in content.lines() {
                let Ok(parsed) = serde_json::from_str::<serde_json::Value>(line) else {
                    continue;
                };
                let raw = parsed.get("event").cloned().unwrap_or_default();
                if let Ok(SessionEvent::SubagentDescriptor {
                    mode, name, parent_session_id, depth, ..
                }) = serde_json::from_value::<SessionEvent>(raw)
                {
                    if mode == "continuable"
                        && name.as_deref() == Some("docs-scout")
                        && depth == 1
                        && !parent_session_id.is_empty()
                    {
                        lineage_ok = true;
                    }
                }
            }
            child_text.push_str(&content);
        }
    }
    if child_logs == 0 {
        failures.push("未找到子会话日志".to_string());
    }
    if !lineage_ok {
        failures.push("子会话缺少 continuable SubagentDescriptor 血缘（name/parent/depth）".to_string());
    }
    if !child_text.contains("\"assistant_message\"") {
        failures.push("子会话未产出真实模型回复（无 assistant_message）".to_string());
    }

    // cwd 越界必须被拒（确定性检查，不依赖模型）。
    if factory(Some("../outside"), None).is_ok() {
        failures.push("cwd 越界未被拒绝".to_string());
    }
    if factory(Some("no-such-dir"), None).is_ok() {
        failures.push("cwd 目录不存在未被拒绝".to_string());
    }

    println!(
        "[smoke] 子会话日志 {} 份，累计 {} 字节",
        child_logs,
        child_text.len()
    );

    if failures.is_empty() {
        let _ = std::fs::remove_dir_all(&workspace);
        println!("\n[smoke] PASS：subagent + wait_agent + parent_wake + cwd 校验 + 续聊 全部通过");
    } else {
        println!("\n[smoke] FAIL（现场保留：{}）：", workspace.display());
        for failure in &failures {
            println!("  - {failure}");
        }
        std::process::exit(1);
    }
}

fn settled_ids_empty(ids: &Arc<Mutex<Vec<String>>>) -> bool {
    lock(ids).is_empty()
}

/// 子会话日志里已开出多少轮（`turn_start` 计数）。
fn turn_count(log_dir: &std::path::Path, session_id: &str) -> usize {
    let file = onion::paths::session_log_file(&onion::paths::session_dir(log_dir, session_id));
    let Ok(content) = std::fs::read_to_string(file) else {
        return 0;
    };
    content
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|parsed| {
            parsed
                .get("event")
                .and_then(|event| event.get("type"))
                .and_then(|kind| kind.as_str())
                == Some("turn_start")
        })
        .count()
}
