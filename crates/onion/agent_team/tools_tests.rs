//! tools 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。
//! 覆盖：9 工具 definition schema / 未接线 fail-closed / 参数映射辅助函数。

use std::sync::Arc;
use std::time::Duration;

use super::{action_from_str, status_from_str, TeamToolSet};
use crate::agent_team::config::TeamConfig;
use crate::agent_team::service::TeamService;
use crate::agent_team::types::{TeamTaskAction, TeamTaskStatus};
use crate::config::Config;
use crate::context::RequestDraft;
use crate::llm::{LlmPort, LlmStream, LlmStreamItem};
use crate::subagent::runtime::{ChildSession, SubagentRuntime};
use crate::tools::ToolRegistry;
use crate::waterfall::BoxFuture;
use tokio_util::sync::CancellationToken;

/// 哑 LlmPort：本组测试不驱动真实轮。
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

/// 构造一个 TeamService（内存态，不驱动子会话）。
fn team_service() -> Arc<TeamService> {
    let config = test_config();
    let runtime = Arc::new(SubagentRuntime::new(
        1,
        config,
        Arc::new(UnusedLlm),
        Arc::new(|_cwd: Option<&str>, _preset: Option<&str>| {
            Ok(ChildSession::new(&test_config(), ToolRegistry::new()))
        }),
        None,
    ));
    Arc::new(TeamService::new(TeamConfig::default(), runtime).expect("config 合法"))
}

/// 全部 9 个工具名（装配白名单）。
const ALL_TOOLS: [&str; 9] = [
    "spawn_teammate",
    "send_message",
    "list_agents",
    "wait_agent",
    "interrupt_agent",
    "team_task_create",
    "team_task_list",
    "team_task_get",
    "team_task_update",
];

#[test]
fn registers_all_nine_tools() {
    let set = TeamToolSet::new(team_service());
    let mut registry = ToolRegistry::new();
    set.register_all(&mut registry);
    registry.state().set_agent(ALL_TOOLS.iter().map(ToString::to_string));
    let names: Vec<String> = registry.definitions().into_iter().map(|d| d.name).collect();
    assert_eq!(names, ALL_TOOLS.map(str::to_string));
}

#[test]
fn tool_definitions_have_schema() {
    let set = TeamToolSet::new(team_service());
    let mut registry = ToolRegistry::new();
    set.register_all(&mut registry);
    registry.state().set_agent(ALL_TOOLS.iter().map(ToString::to_string));
    let defs = registry.definitions();
    let spawn = defs.iter().find(|d| d.name == "spawn_teammate").expect("spawn 存在");
    assert!(spawn.parameters["properties"]["name"].is_object());
    // provider 不暴露给模型：required = [name, description, prompt]。
    let spawn_required: Vec<&str> = spawn.parameters["required"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();
    assert_eq!(spawn_required, vec!["name", "description", "prompt"]);
    assert!(spawn.parameters["properties"].get("provider").is_none());
    // interrupt_agent 用 target 参数。
    let interrupt = defs.iter().find(|d| d.name == "interrupt_agent").expect("interrupt 存在");
    assert!(interrupt.parameters["properties"]["target"].is_object());
    assert!(interrupt.parameters["properties"].get("agent_name").is_none());
    // team_task_list 支持 cursor/limit 分页。
    let list = defs.iter().find(|d| d.name == "team_task_list").expect("list 存在");
    assert!(list.parameters["properties"]["cursor"].is_object());
    assert!(list.parameters["properties"]["limit"].is_object());
    let create = defs.iter().find(|d| d.name == "team_task_create").expect("create 存在");
    assert_eq!(create.parameters["required"].as_array().map(Vec::len), Some(1));
    let update = defs.iter().find(|d| d.name == "team_task_update").expect("update 存在");
    assert!(update.parameters["properties"]["action"].is_object());
    assert_eq!(defs.len(), 9);
}

#[tokio::test]
async fn un_wired_membership_fails_closed() {
    // 未 wire：membership 应 fail-closed 报 NotMember（工具执行经它拒绝）。
    let svc = team_service();
    let err = svc.membership("lead").await.unwrap_err();
    assert_eq!(err.to_string(), "not a team member");
}

#[test]
fn task_action_and_status_mapping() {
    assert_eq!(action_from_str("claim"), TeamTaskAction::Claim);
    assert_eq!(action_from_str("complete"), TeamTaskAction::Complete);
    assert_eq!(action_from_str("reassign"), TeamTaskAction::Reassign);
    assert_eq!(action_from_str("unknown"), TeamTaskAction::Delete);
    assert_eq!(status_from_str("in_progress"), TeamTaskStatus::InProgress);
    assert_eq!(status_from_str("completed"), TeamTaskStatus::Completed);
    assert_eq!(status_from_str("pending"), TeamTaskStatus::Pending);
}
