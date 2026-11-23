//! `tools` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use super::basic::{ReadTool, WriteTool};
use super::edit::{EditTool, StrReplaceEditorTool};
use super::search::ToolSearchTool;
use super::{ToolPort, ToolRegistry};
use crate::approval::ToolRisk;
use crate::sandbox::Sandbox;
use crate::types::{ContentBlock, ToolCall, ToolDefinition};
use crate::waterfall::BoxFuture;

const TIMEOUT: Duration = Duration::from_secs(5);

/// 提取结果文本（错误/成功均取首块文本，供断言）。
fn result_text(result: &crate::types::ToolResult) -> String {
    result.content.iter().find_map(|block| match block {
        ContentBlock::Text { text } => Some(text.clone()),
        _ => None,
    }).unwrap_or_default()
}

/// 可配置自描述的测试工具（modes / defer_loading / 自定义定义 / 风险等级）。
struct StubTool {
    def: ToolDefinition,
    modes: Option<Vec<String>>,
    deferred: bool,
    risk: ToolRisk,
}

impl StubTool {
    fn new(name: &str, modes: Option<Vec<String>>, deferred: bool) -> Self {
        Self::with_risk(name, modes, deferred, ToolRisk::ReadOnly)
    }

    fn with_risk(name: &str, modes: Option<Vec<String>>, deferred: bool, risk: ToolRisk) -> Self {
        Self {
            def: ToolDefinition {
                name: name.to_string(),
                description: format!("{name} tool"),
                parameters: json!({ "type": "object", "properties": { "x": { "type": "string" } } }),
            },
            modes,
            deferred,
            risk,
        }
    }
}

impl ToolPort for StubTool {
    fn definition(&self) -> ToolDefinition {
        self.def.clone()
    }
    fn execute(&self, _call: ToolCall, _cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        Box::pin(async { Ok(vec![ContentBlock::Text { text: "stub ran".to_string() }]) })
    }
    fn modes(&self) -> Option<Vec<String>> {
        self.modes.clone()
    }
    fn defer_loading(&self) -> bool {
        self.deferred
    }
    fn risk(&self) -> ToolRisk {
        self.risk
    }
}

fn call(name: &str) -> ToolCall {
    ToolCall { id: "c1".to_string(), name: name.to_string(), input: json!({}) }
}

async fn run(registry: &ToolRegistry, name: &str) -> crate::types::ToolResult {
    registry.execute(&call(name), &CancellationToken::new(), TIMEOUT).await
}

/// 将 registry 的 Agent 工具面设为给定工具名集合（白名单过滤）。
fn set_agent(registry: &ToolRegistry, tools: &[&str]) {
    registry.state().set_agent(tools.iter().copied().map(ToString::to_string));
}

#[tokio::test]
async fn agent_set_filters_definitions_and_execution() {
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(StubTool::new("open_tool", None, false)));
    registry.register(Arc::new(StubTool::new("work_tool", Some(vec!["work".to_string()]), false)));
    // 未装配（缺省）：工具面为空，全部不可见
    let names: Vec<String> = registry.definitions().iter().map(|d| d.name.clone()).collect();
    assert_eq!(names, Vec::<String>::new());
    let result = run(&registry, "work_tool").await;
    assert_eq!(
        result,
        crate::types::ToolResult::error(&call("work_tool"), "tool \"work_tool\" not in current agent tool set")
    );
    // 切换 Agent 工具集：命中的工具可见且可执行
    set_agent(&registry, &["open_tool", "work_tool"]);
    let names: Vec<String> = registry.definitions().iter().map(|d| d.name.clone()).collect();
    assert_eq!(names, vec!["open_tool".to_string(), "work_tool".to_string()]);
    let result = run(&registry, "work_tool").await;
    assert!(!result.is_error);
    // 面外工具仍不可见
    let result = run(&registry, "nope").await;
    assert_eq!(result, crate::types::ToolResult::error(&call("nope"), "unknown tool: nope"));
}

#[tokio::test]
async fn deferred_tool_projects_stub_definition() {
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(StubTool::new("plain_tool", None, false)));
    registry.register(Arc::new(StubTool::new("heavy_tool", None, true)));
    set_agent(&registry, &["plain_tool", "heavy_tool"]);
    let definitions = registry.definitions();
    assert_eq!(definitions.len(), 2);
    let plain = definitions.iter().find(|d| d.name == "plain_tool").unwrap_or_else(|| panic!("missing plain_tool"));
    let heavy = definitions.iter().find(|d| d.name == "heavy_tool").unwrap_or_else(|| panic!("missing heavy_tool"));
    // 普通：原样；deferred：stub（空 parameters + 提示后缀）
    assert_eq!(plain.parameters, json!({ "type": "object", "properties": { "x": { "type": "string" } } }));
    assert_eq!(heavy.parameters, json!({ "type": "object", "properties": {} }));
    assert!(
        heavy.description.ends_with("[deferred: call tool_search to load the full schema]"),
        "unexpected description: {}",
        heavy.description
    );
}

#[tokio::test]
async fn deferred_tool_requires_unlock_before_execute() {
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(StubTool::new("heavy_tool", None, true)));
    set_agent(&registry, &["heavy_tool"]);
    let result = run(&registry, "heavy_tool").await;
    assert_eq!(
        result,
        crate::types::ToolResult::error(
            &call("heavy_tool"),
            "deferred tool \"heavy_tool\" requires tool_search first"
        )
    );
    registry.unlock("heavy_tool");
    let result = run(&registry, "heavy_tool").await;
    assert!(!result.is_error);
}

#[tokio::test]
async fn deferred_definitions_follow_agent_set_and_carry_full_schema() {
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(StubTool::new("open_tool", None, false)));
    registry.register(Arc::new(StubTool::new("chat_heavy", None, true)));
    registry.register(Arc::new(StubTool::new("work_heavy", Some(vec!["work".to_string()]), true)));
    // 仅含 chat_heavy 的 Agent：deferred 面 = chat_heavy；work_heavy（面外）与 open_tool（非 deferred）不出现。
    set_agent(&registry, &["chat_heavy"]);
    let names: Vec<String> = registry.deferred_definitions().iter().map(|d| d.name.clone()).collect();
    assert_eq!(names, vec!["chat_heavy".to_string()]);
    // 加入 work_heavy：deferred 面 = chat_heavy + work_heavy。
    set_agent(&registry, &["chat_heavy", "work_heavy"]);
    let names: Vec<String> = registry.deferred_definitions().iter().map(|d| d.name.clone()).collect();
    assert_eq!(names, vec!["chat_heavy".to_string(), "work_heavy".to_string()]);
    // 完整 schema（非 stub）
    let schema = registry.deferred_definitions()[0].parameters.clone();
    assert_eq!(schema, json!({ "type": "object", "properties": { "x": { "type": "string" } } }));
}

fn search_tool(registry: &ToolRegistry) -> Arc<ToolSearchTool> {
    Arc::new(ToolSearchTool::new(registry.clone(), registry.state()))
}

async fn execute_search(tool: &ToolSearchTool, query: Value) -> String {
    let call = ToolCall { id: "s1".to_string(), name: "tool_search".to_string(), input: query };
    let result = tool.execute(call, CancellationToken::new()).await;
    let Ok(blocks) = result else {
        panic!("tool_search failed: {result:?}");
    };
    blocks
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn tool_search_matches_case_insensitive_and_unlocks() {
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(StubTool::new("write_file", None, true)));
    set_agent(&registry, &["write_file"]);
    let tool = search_tool(&registry);
    // 大小写不敏感命中 name
    let text = execute_search(tool.as_ref(), json!({ "query": "WRITE_FILE" })).await;
    assert!(text.contains("name: write_file"), "unexpected output: {text}");
    // 完整 schema 全文（紧凑 JSON 序列化）
    assert!(text.contains(r#""type":"string""#), "full schema expected: {text}");
    // 命中即解锁：deferred 工具可执行
    let result = run(&registry, "write_file").await;
    assert!(!result.is_error);
}

#[tokio::test]
async fn tool_search_matches_description_and_empty_query_lists_all() {
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(StubTool::new("alpha_tool", None, true)));
    registry.register(Arc::new(StubTool::new("beta_tool", None, true)));
    set_agent(&registry, &["alpha_tool", "beta_tool"]);
    let tool = search_tool(&registry);
    let text = execute_search(tool.as_ref(), json!({ "query": "heavy beta" })).await;
    assert!(text.contains("beta_tool") && !text.contains("alpha_tool"), "unexpected output: {text}");
    let text = execute_search(tool.as_ref(), json!({ "query": "" })).await;
    assert!(text.contains("alpha_tool") && text.contains("beta_tool"), "unexpected output: {text}");
}

#[tokio::test]
async fn tool_search_reports_no_match() {
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(StubTool::new("alpha_tool", None, true)));
    set_agent(&registry, &["alpha_tool"]);
    let tool = search_tool(&registry);
    let text = execute_search(tool.as_ref(), json!({ "query": "zzz" })).await;
    assert_eq!(text, "no deferred tools match query \"zzz\"");
}

#[tokio::test]
async fn write_roundtrip_through_sandbox() {
    let root = std::env::temp_dir().join(format!("ava_onion_tools_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap_or_else(|error| panic!("create temp root failed: {error}"));
    let mut registry = ToolRegistry::new();
    let observation = Arc::new(crate::fs_observation::FsObservation::new());
    let sandbox = Arc::new(Sandbox::new(root.clone()));
    registry.register(Arc::new(WriteTool {
        sandbox: sandbox.clone(),
        observation: observation.clone(),
    }));
    registry.register(Arc::new(ReadTool {
        sandbox,
        observation,
    }));
    registry.unlock("write");
    set_agent(&registry, &["write", "read"]);
    // 写前先 read（观测门禁要求：登记 absent 凭证后才能创建文件）。
    let read_call = ToolCall {
        id: "r1".to_string(),
        name: "read".to_string(),
        input: json!({ "file_path": "out/note.txt" }),
    };
    let _ = registry.execute(&read_call, &CancellationToken::new(), TIMEOUT).await;
    let call = ToolCall {
        id: "w1".to_string(),
        name: "write".to_string(),
        input: json!({ "file_path": "out/note.txt", "content": "hello" }),
    };
    let result = registry.execute(&call, &CancellationToken::new(), TIMEOUT).await;
    assert!(!result.is_error);
    let written = std::fs::read_to_string(root.join("out").join("note.txt"))
        .unwrap_or_else(|error| panic!("read written file failed: {error}"));
    assert_eq!(written, "hello");
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn read_paginates_by_offset_and_limit() {
    let root = std::env::temp_dir().join(format!("onion_read_page_{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("a.txt"), "line1\nline2\nline3\nline4\n")
        .unwrap_or_else(|error| panic!("seed a failed: {error}"));
    let mut registry = ToolRegistry::new();
    let observation = Arc::new(crate::fs_observation::FsObservation::new());
    registry.register(Arc::new(ReadTool { sandbox: Arc::new(Sandbox::new(root.clone())), observation }));
    set_agent(&registry, &["read"]);
    // 分页读取中间两行（offset=2, limit=2）。
    let call = ToolCall {
        id: "r1".to_string(),
        name: "read".to_string(),
        input: json!({ "file_path": "a.txt", "offset": 2, "limit": 2 }),
    };
    let result = registry.execute(&call, &CancellationToken::new(), TIMEOUT).await;
    assert!(!result.is_error, "unexpected: {}", result_text(&result));
    let text = result_text(&result);
    assert!(text.contains("line2") && text.contains("line3"), "unexpected: {text}");
    assert!(!text.contains("line1"), "offset not honored: {text}");
    assert!(text.contains("lines 2\u{2013}3 of 4"), "no pagination footer: {text}");
    let _ = std::fs::remove_dir_all(&root);
}

// ---- shell（bash / pwsh）----

/// 探测可用的 shell 方言（Windows 优先 pwsh，其次 bash；WSL 的 system32
/// `bash.exe` 启动器对 `-c` 的 stdout 捕获不可靠，视为不可用）。
fn probe_shell() -> Option<&'static str> {
    if std::env::consts::OS == "windows" {
        for program in ["pwsh", "bash"] {
            if program == "bash"
                && std::process::Command::new("where.exe")
                    .arg("bash")
                    .output()
                    .ok()
                    .map(|o| String::from_utf8_lossy(&o.stdout).to_lowercase().contains("system32"))
                    .unwrap_or(false)
            {
                continue;
            }
            if std::process::Command::new(program).arg("-c").arg("exit 0").status().is_ok() {
                return Some(program);
            }
        }
        None
    } else {
        Some("bash")
    }
}

#[tokio::test]
async fn shell_tools_execute_and_capture_stdout() {
    let Some(program) = probe_shell() else { return };
    let executor = Arc::new(crate::shell_exec::oneshot::OneshotExecutor::new(None));
    let cwd = Arc::new(crate::session_cwd::SessionCwd::new(std::env::temp_dir()));
    let owner = super::jobs::owner_cell();
    let mut registry = ToolRegistry::new();
    if program == "bash" {
        registry.register(Arc::new(super::shell::BashTool {
            executor,
            background: None,
            cwd,
            owner,
            escalator: None,
        }));
    } else {
        registry.register(Arc::new(super::shell::PwshTool {
            executor,
            background: None,
            cwd,
            owner,
            escalator: None,
        }));
    }
    registry.state().set_agent([program.to_string()]);
    // 方言适切命令：bash 用 echo，pwsh 用 Write-Output。
    let command = if program == "bash" {
        "echo hello-shell"
    } else {
        "Write-Output hello-shell"
    };
    let call = ToolCall {
        id: "sh1".to_string(),
        name: if program == "bash" { "bash" } else { "pwsh" }.to_string(),
        input: json!({ "description": "print hello", "command": command }),
    };
    let result = registry.execute(&call, &CancellationToken::new(), TIMEOUT).await;
    assert!(!result.is_error, "unexpected: {}", result_text(&result));
    assert!(result_text(&result).contains("hello-shell"), "no stdout: {}", result_text(&result));
}

#[tokio::test]
async fn shell_tools_expose_escalating_risk_and_work_mode() {
    let executor = Arc::new(crate::shell_exec::oneshot::OneshotExecutor::new(None));
    let tool = super::shell::BashTool {
        executor,
        background: None,
        cwd: Arc::new(crate::session_cwd::SessionCwd::new(std::env::temp_dir())),
        owner: super::jobs::owner_cell(),
        escalator: None,
    };
    assert_eq!(tool.risk(), ToolRisk::Escalating);
    assert_eq!(tool.modes(), Some(vec!["work".to_string()]));
    // Agent 面未包含 bash：bash 不可见、执行被拒。
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(super::shell::BashTool {
        executor: Arc::new(crate::shell_exec::oneshot::OneshotExecutor::new(None)),
        background: None,
        cwd: Arc::new(crate::session_cwd::SessionCwd::new(std::env::temp_dir())),
        owner: super::jobs::owner_cell(),
        escalator: None,
    }));
    set_agent(&registry, &["read"]);
    let call = ToolCall {
        id: "sh2".to_string(),
        name: "bash".to_string(),
        input: json!({ "description": "x", "command": "echo x" }),
    };
    let result = registry.execute(&call, &CancellationToken::new(), TIMEOUT).await;
    assert!(result.is_error);
    assert!(result_text(&result).contains("not in current agent tool set"));
}

#[tokio::test]
async fn unknown_tool_still_reports_error() {
    let registry = ToolRegistry::new();
    let result = run(&registry, "nope").await;
    assert_eq!(result, crate::types::ToolResult::error(&call("nope"), "unknown tool: nope"));
}

/// risk_map 按注册表自动提取「工具名 → 风险」：默认只读（StubTool 未覆写时）
/// 与工具自描述的写风险（WriteTool）各自就位，新增工具不漏登记。
#[tokio::test]
async fn risk_map_exposes_declared_risk() {
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(StubTool::with_risk("plain_tool", None, false, ToolRisk::ReadOnly)));
    registry.register(Arc::new(StubTool::with_risk(
        "derived_tool",
        None,
        false,
        ToolRisk::WorkspaceWrite,
    )));
    registry.register(Arc::new(WriteTool {
        sandbox: Arc::new(Sandbox::new(std::env::temp_dir())),
        observation: Arc::new(crate::fs_observation::FsObservation::new()),
    }));
    let risk_map = registry.risk_map();
    assert_eq!(risk_map.get("plain_tool"), Some(&ToolRisk::ReadOnly));
    assert_eq!(risk_map.get("derived_tool"), Some(&ToolRisk::WorkspaceWrite));
    assert_eq!(risk_map.get("write"), Some(&ToolRisk::WorkspaceWrite));
    assert_eq!(risk_map.len(), 3);
}

// ---- read-before-write 观测门禁 + edit / str_replace_editor ----

/// 造一个临时目录，返回 (root, 共享观测表)。`tag` 须每测试唯一，避免并行冲突。
fn edit_fixture(tag: &str) -> (std::path::PathBuf, Arc<crate::fs_observation::FsObservation>) {
    let root = std::env::temp_dir().join(format!("ava_onion_edit_{}_{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap_or_else(|error| panic!("create temp root failed: {error}"));
    (root, Arc::new(crate::fs_observation::FsObservation::new()))
}

async fn exec_edit(
    registry: &ToolRegistry,
    input: Value,
) -> crate::types::ToolResult {
    let call = ToolCall { id: "e1".to_string(), name: "edit".to_string(), input };
    registry.execute(&call, &CancellationToken::new(), TIMEOUT).await
}

#[tokio::test]
async fn edit_rejects_unobserved_file() {
    let (root, observation) = edit_fixture("unobserved");
    std::fs::write(root.join("a.txt"), b"hello world")
        .unwrap_or_else(|error| panic!("seed a failed: {error}"));
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(EditTool {
        sandbox: Arc::new(Sandbox::new(root.clone())),
        observation: observation.clone(),
    }));
    registry.unlock("edit");
    set_agent(&registry, &["edit"]);
    let result = exec_edit(
        &registry,
        json!({ "file_path": "a.txt", "old_string": "hello", "new_string": "bye" }),
    )
    .await;
    assert!(result.is_error, "unexpected: {}", result_text(&result));
    assert!(
        result_text(&result).contains("FS_NOT_OBSERVED"),
        "unexpected: {}",
        result_text(&result)
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn edit_unique_literal_replace_and_refreshes_observation() {
    let (root, observation) = edit_fixture("unique");
    std::fs::write(root.join("a.txt"), b"hello world")
        .unwrap_or_else(|error| panic!("seed a failed: {error}"));
    let sandbox = Arc::new(Sandbox::new(root.clone()));
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(ReadTool { sandbox: sandbox.clone(), observation: observation.clone() }));
    registry.register(Arc::new(EditTool { sandbox: sandbox.clone(), observation: observation.clone() }));
    registry.unlock("edit");
    set_agent(&registry, &["read", "edit"]);
    // 先读（登记观测）
    let read = registry.execute(
        &ToolCall {
            id: "r1".to_string(),
            name: "read".to_string(),
            input: json!({ "file_path": "a.txt" }),
        },
        &CancellationToken::new(),
        TIMEOUT,
    ).await;
    assert!(!read.is_error, "unexpected: {}", result_text(&read));
    let result = exec_edit(
        &registry,
        json!({ "file_path": "a.txt", "old_string": "hello", "new_string": "bye" }),
    )
    .await;
    assert!(!result.is_error, "unexpected: {:?}", result.content);
    let after = std::fs::read_to_string(root.join("a.txt")).unwrap();
    assert_eq!(after, "bye world");
    // 改动后观测已刷新：再次 edit 同一内容（当前版本一致）应放行
    let result = exec_edit(
        &registry,
        json!({ "file_path": "a.txt", "old_string": "bye", "new_string": "hello" }),
    )
    .await;
    assert!(!result.is_error, "unexpected: {}", result_text(&result));
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn edit_rejects_ambiguous_old_string() {
    let (root, observation) = edit_fixture("ambiguous");
    std::fs::write(root.join("a.txt"), b"x x x")
        .unwrap_or_else(|error| panic!("seed a failed: {error}"));
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(ReadTool {
        sandbox: Arc::new(Sandbox::new(root.clone())),
        observation: observation.clone(),
    }));
    registry.register(Arc::new(EditTool {
        sandbox: Arc::new(Sandbox::new(root.clone())),
        observation: observation.clone(),
    }));
    registry.unlock("edit");
    set_agent(&registry, &["read", "edit"]);
    // 读登记
    registry.execute(
        &ToolCall { id: "r1".to_string(), name: "read".to_string(), input: json!({ "file_path": "a.txt" }) },
        &CancellationToken::new(),
        TIMEOUT,
    ).await;
    let result = exec_edit(
        &registry,
        json!({ "file_path": "a.txt", "old_string": "x", "new_string": "y" }),
    )
    .await;
    assert!(result.is_error, "unexpected: {}", result_text(&result));
    assert!(
        result_text(&result).contains("not unique"),
        "unexpected: {}",
        result_text(&result)
    );
    // replace_all 放行
    let result = exec_edit(
        &registry,
        json!({ "file_path": "a.txt", "old_string": "x", "new_string": "y", "replace_all": true }),
    )
    .await;
    assert!(!result.is_error, "unexpected: {}", result_text(&result));
    let after = std::fs::read_to_string(root.join("a.txt")).unwrap();
    assert_eq!(after, "y y y");
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn edit_fails_closed_when_file_changed_externally() {
    let (root, observation) = edit_fixture("stale");
    std::fs::write(root.join("a.txt"), b"hello world")
        .unwrap_or_else(|error| panic!("seed a failed: {error}"));
    let sandbox = Arc::new(Sandbox::new(root.clone()));
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(ReadTool { sandbox: sandbox.clone(), observation: observation.clone() }));
    registry.register(Arc::new(EditTool { sandbox: sandbox.clone(), observation: observation.clone() }));
    registry.unlock("edit");
    set_agent(&registry, &["read", "edit"]);
    registry.execute(
        &ToolCall { id: "r1".to_string(), name: "read".to_string(), input: json!({ "file_path": "a.txt" }) },
        &CancellationToken::new(),
        TIMEOUT,
    ).await;
    // 读取后外部改动
    std::fs::write(root.join("a.txt"), b"hello world changed").unwrap();
    let result = exec_edit(
        &registry,
        json!({ "file_path": "a.txt", "old_string": "hello", "new_string": "bye" }),
    )
    .await;
    assert!(result.is_error, "unexpected: {}", result_text(&result));
    assert!(
        result_text(&result).contains("FS_STALE_VERSION"),
        "unexpected: {}",
        result_text(&result)
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn str_replace_editor_view_create_insert_flow() {
    let (root, observation) = edit_fixture("sre_flow");
    let sandbox = Arc::new(Sandbox::new(root.clone()));
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(StrReplaceEditorTool {
        sandbox: sandbox.clone(),
        observation: observation.clone(),
    }));
    set_agent(&registry, &["str_replace_editor"]);
    // create：不受观测门禁
    let call = ToolCall {
        id: "c1".to_string(),
        name: "str_replace_editor".to_string(),
        input: json!({ "command": "create", "path": "b.txt", "file_text": "line1\nline2" }),
    };
    let result = registry.execute(&call, &CancellationToken::new(), TIMEOUT).await;
    assert!(!result.is_error, "unexpected: {}", result_text(&result));
    let created = std::fs::read_to_string(root.join("b.txt")).unwrap();
    assert_eq!(created, "line1\nline2");
    // view：登记观测并返回内容
    let call = ToolCall {
        id: "v1".to_string(),
        name: "str_replace_editor".to_string(),
        input: json!({ "command": "view", "path": "b.txt" }),
    };
    let result = registry.execute(&call, &CancellationToken::new(), TIMEOUT).await;
    assert!(!result.is_error, "unexpected: {}", result_text(&result));
    assert_eq!(result.content, vec![ContentBlock::Text { text: "line1\nline2".to_string() }]);
    // insert：观测一致则放行
    let call = ToolCall {
        id: "i1".to_string(),
        name: "str_replace_editor".to_string(),
        input: json!({ "command": "insert", "path": "b.txt", "insert_line": 1, "new_str": "mid" }),
    };
    let result = registry.execute(&call, &CancellationToken::new(), TIMEOUT).await;
    assert!(!result.is_error, "unexpected: {}", result_text(&result));
    let after = std::fs::read_to_string(root.join("b.txt")).unwrap();
    assert_eq!(after, "line1\nmid\nline2");
    // str_replace：观测一致（insert 后已刷新）则放行
    let call = ToolCall {
        id: "s1".to_string(),
        name: "str_replace_editor".to_string(),
        input: json!({ "command": "str_replace", "path": "b.txt", "old_str": "mid", "new_str": "M" }),
    };
    let result = registry.execute(&call, &CancellationToken::new(), TIMEOUT).await;
    assert!(!result.is_error, "unexpected: {}", result_text(&result));
    let after = std::fs::read_to_string(root.join("b.txt")).unwrap();
    assert_eq!(after, "line1\nM\nline2");
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn str_replace_editor_rejects_unobserved_modify() {
    let (root, observation) = edit_fixture("sre_unobs");
    std::fs::write(root.join("b.txt"), b"abc")
        .unwrap_or_else(|error| panic!("seed b failed: {error}"));
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(StrReplaceEditorTool {
        sandbox: Arc::new(Sandbox::new(root.clone())),
        observation: observation.clone(),
    }));
    set_agent(&registry, &["str_replace_editor"]);
    let call = ToolCall {
        id: "s1".to_string(),
        name: "str_replace_editor".to_string(),
        input: json!({ "command": "str_replace", "path": "b.txt", "old_str": "a", "new_str": "x" }),
    };
    let result = registry.execute(&call, &CancellationToken::new(), TIMEOUT).await;
    assert!(result.is_error, "unexpected: {}", result_text(&result));
    assert!(
        result_text(&result).contains("FS_NOT_OBSERVED"),
        "unexpected: {}",
        result_text(&result)
    );
    let _ = std::fs::remove_dir_all(&root);
}

// ---- jobs 控制面（job_list / job_output / job_kill）----

use onion_jobs::{JobOutcome, JobRegistry, JobSpec, JobKind, OutputRing, SessionId};
use super::jobs::{jobs_tools, owner_cell, wire_owner};

/// 造一个带 owner 的 registry 与三个 jobs 工具。
fn jobs_fixture(session: &str) -> (Arc<JobRegistry>, ToolRegistry) {
    let registry = Arc::new(JobRegistry::new());
    let owner = owner_cell();
    wire_owner(&owner, session.to_string());
    let (list, output, kill) = jobs_tools(registry.clone(), owner);
    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(list));
    tools.register(Arc::new(output));
    tools.register(Arc::new(kill));
    set_agent(&tools, &["job_list", "job_output", "job_kill"]);
    (registry, tools)
}

/// 直接在 registry 里塞一条任务（模拟后台 spawn）。
fn seed_job(registry: &JobRegistry, id: &str, owner: &str, command: &str) {
    registry.register(
        JobSpec {
            id: onion_jobs::JobId(id.to_string()),
            owner: SessionId(owner.to_string()),
            kind: JobKind::Shell,
            command: command.to_string(),
        },
        OutputRing::new(),
    );
}

#[tokio::test]
async fn job_list_lists_owner_scoped_jobs() {
    let (registry, tools) = jobs_fixture("sess-1");
    seed_job(&registry, "job-a", "sess-1", "echo a");
    seed_job(&registry, "job-b", "sess-2", "echo b");
    let result = run(&tools, "job_list").await;
    assert!(!result.is_error, "unexpected: {}", result_text(&result));
    let text = result_text(&result);
    assert!(text.contains("job-a") && text.contains("running"), "unexpected: {text}");
    assert!(!text.contains("job-b"), "owner leak: {text}");
}

#[tokio::test]
async fn job_list_fails_closed_when_owner_unwired() {
    let registry = Arc::new(JobRegistry::new());
    let (list, _, _) = jobs_tools(registry, owner_cell());
    let mut tools = ToolRegistry::new();
    tools.register(Arc::new(list));
    set_agent(&tools, &["job_list"]);
    let result = run(&tools, "job_list").await;
    assert!(result.is_error, "expected fail-closed");
    assert!(result_text(&result).contains("not wired"), "unexpected: {}", result_text(&result));
}

#[tokio::test]
async fn job_output_reads_tail_and_status() {
    let (registry, tools) = jobs_fixture("sess-1");
    seed_job(&registry, "job-1", "sess-1", "echo hi");
    registry.push_output("job-1", "line1\n");
    registry.push_output("job-1", "line2\n");
    let call = ToolCall {
        id: "j1".to_string(),
        name: "job_output".to_string(),
        input: json!({ "job_id": "job-1", "wait": false }),
    };
    let result = tools.execute(&call, &CancellationToken::new(), TIMEOUT).await;
    assert!(!result.is_error, "unexpected: {}", result_text(&result));
    let text = result_text(&result);
    assert!(text.contains("line1") && text.contains("line2"), "unexpected: {text}");
    assert!(text.contains("running"), "no status: {text}");
}

#[tokio::test]
async fn job_output_missing_job_reports_error() {
    let (_, tools) = jobs_fixture("sess-1");
    let call = ToolCall {
        id: "j2".to_string(),
        name: "job_output".to_string(),
        input: json!({ "job_id": "job-nope" }),
    };
    let result = tools.execute(&call, &CancellationToken::new(), TIMEOUT).await;
    assert!(result.is_error, "expected error");
    assert!(result_text(&result).contains("not found"), "unexpected: {}", result_text(&result));
}

#[tokio::test]
async fn job_kill_first_wins_and_idempotent() {
    let (registry, tools) = jobs_fixture("sess-1");
    seed_job(&registry, "job-1", "sess-1", "sleep 30");
    let call = ToolCall {
        id: "k1".to_string(),
        name: "job_kill".to_string(),
        input: json!({ "job_id": "job-1" }),
    };
    let result = tools.execute(&call, &CancellationToken::new(), TIMEOUT).await;
    assert!(!result.is_error, "unexpected: {}", result_text(&result));
    assert!(result_text(&result).contains("kill requested"), "unexpected: {}", result_text(&result));
    // first-wins 已落地 Killed。
    let view = registry.get("job-1").unwrap();
    assert_eq!(view.status, onion_jobs::JobStatus::Settled(JobOutcome::Killed));
    // 再次 kill：AlreadySettled。
    let result = tools.execute(&call, &CancellationToken::new(), TIMEOUT).await;
    assert!(result_text(&result).contains("already finished"), "unexpected: {}", result_text(&result));
    // 不存在。
    let call = ToolCall {
        id: "k2".to_string(),
        name: "job_kill".to_string(),
        input: json!({ "job_id": "job-nope" }),
    };
    let result = tools.execute(&call, &CancellationToken::new(), TIMEOUT).await;
    assert!(result_text(&result).contains("not found"), "unexpected: {}", result_text(&result));
}

#[tokio::test]
async fn jobs_tools_expose_risk_and_mode() {
    let (list, output, kill) = jobs_tools(Arc::new(JobRegistry::new()), owner_cell());
    assert_eq!(list.risk(), ToolRisk::ReadOnly);
    assert_eq!(output.risk(), ToolRisk::ReadOnly);
    assert_eq!(kill.risk(), ToolRisk::Escalating);
    assert_eq!(list.modes(), Some(vec!["work".to_string()]));
    assert_eq!(kill.modes(), Some(vec!["work".to_string()]));
}

// ---- glob / grep（S5）----

use super::glob_grep::{GlobTool, GrepTool};

/// 造一个带若干文件的沙箱 fixture，返回 (root, sandbox, registry)。
/// `tag` 需各测试唯一（并行测试共享同一进程，避免目录互删）。
fn search_fixture(tag: &str) -> (std::path::PathBuf, Arc<Sandbox>, ToolRegistry) {
    let root = std::env::temp_dir().join(format!("onion_search_{}_{}", std::process::id(), tag));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src/nested")).unwrap();
    std::fs::write(root.join("src/main.rs"), b"fn main() { hello_world(); }\n").unwrap();
    std::fs::write(root.join("src/nested/lib.rs"), b"pub fn hello_world() {}\n").unwrap();
    std::fs::write(root.join("README.md"), b"# hello\n").unwrap();
    std::fs::write(root.join(".hidden"), b"secret hello\n").unwrap();
    let sandbox = Arc::new(Sandbox::new(root.clone()));
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(GlobTool::new(sandbox.clone())));
    registry.register(Arc::new(GrepTool::new(sandbox.clone())));
    set_agent(&registry, &["glob", "grep"]);
    (root, sandbox, registry)
}

#[tokio::test]
async fn glob_finds_files_by_basename_at_any_depth() {
    let (root, _sandbox, registry) = search_fixture("glob_basename");
    // 无 `/` 模式：`*.rs` 匹配任意深度。
    let call = ToolCall {
        id: "g1".to_string(),
        name: "glob".to_string(),
        input: json!({ "pattern": "*.rs" }),
    };
    let result = registry.execute(&call, &CancellationToken::new(), TIMEOUT).await;
    assert!(!result.is_error, "unexpected: {}", result_text(&result));
    let text = result_text(&result);
    assert!(text.contains("src/main.rs"), "missing main.rs: {text}");
    assert!(text.contains("src/nested/lib.rs"), "missing nested lib.rs: {text}");
    assert!(!text.contains("README.md"), "non-rs leaked: {text}");
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn glob_no_match_returns_no_files_found() {
    let (root, _sandbox, registry) = search_fixture("glob_nomatch");
    let call = ToolCall {
        id: "g2".to_string(),
        name: "glob".to_string(),
        input: json!({ "pattern": "**.py" }),
    };
    let result = registry.execute(&call, &CancellationToken::new(), TIMEOUT).await;
    assert!(!result.is_error);
    assert_eq!(result_text(&result), "No files found");
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn glob_scoped_to_path_subdir() {
    let (root, _sandbox, registry) = search_fixture("glob_scoped");
    let call = ToolCall {
        id: "g3".to_string(),
        name: "glob".to_string(),
        input: json!({ "pattern": "*.rs", "path": "src" }),
    };
    let result = registry.execute(&call, &CancellationToken::new(), TIMEOUT).await;
    assert!(!result.is_error, "unexpected: {}", result_text(&result));
    let text = result_text(&result);
    // 相对搜索根展示：src 下的 main.rs 显示为 main.rs，nested/lib.rs 保持相对。
    assert!(text.contains("main.rs"), "missing main.rs: {text}");
    assert!(text.contains("nested/lib.rs"), "missing nested/lib.rs: {text}");
    assert!(!text.contains("README.md"));
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn glob_searches_directory_outside_root() {
    // 读类放开（读在所有模式下放行）：glob 的 path 参数可指向权限根外
    // 的任意可读目录（如数据根内的 spill 日志目录）；写侧遏制不变，由 sandbox_tests 覆盖。
    let (root, _sandbox, registry) = search_fixture("glob_outside");
    let outside = std::env::temp_dir().join("ava_onion_glob_outside");
    let _ = std::fs::remove_dir_all(&outside);
    std::fs::create_dir_all(&outside).unwrap_or_else(|error| panic!("seed outside dir failed: {error}"));
    std::fs::write(outside.join("outer.rs"), b"fn outer() {}")
        .unwrap_or_else(|error| panic!("seed outside file failed: {error}"));
    let call = ToolCall {
        id: "g4".to_string(),
        name: "glob".to_string(),
        input: json!({ "pattern": "*", "path": "../ava_onion_glob_outside" }),
    };
    let result = registry.execute(&call, &CancellationToken::new(), TIMEOUT).await;
    assert!(!result.is_error, "unexpected: {}", result_text(&result));
    assert!(result_text(&result).contains("outer.rs"), "missing outer.rs: {}", result_text(&result));
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&outside);
}

#[tokio::test]
async fn grep_searches_contents_grouped_by_file() {
    let (root, _sandbox, registry) = search_fixture("grep_basic");
    let call = ToolCall {
        id: "s1".to_string(),
        name: "grep".to_string(),
        input: json!({ "pattern": "hello" }),
    };
    let result = registry.execute(&call, &CancellationToken::new(), TIMEOUT).await;
    assert!(!result.is_error, "unexpected: {}", result_text(&result));
    let text = result_text(&result);
    assert!(text.contains("src/main.rs"), "missing main.rs block: {text}");
    assert!(text.contains("Line 1"), "missing line number: {text}");
    assert!(text.contains("hello_world"), "missing preview: {text}");
    assert!(text.contains(".hidden"), "hidden file should be searched: {text}");
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn grep_include_filters_files() {
    let (root, _sandbox, registry) = search_fixture("grep_include");
    let call = ToolCall {
        id: "s2".to_string(),
        name: "grep".to_string(),
        input: json!({ "pattern": "hello", "include": "*.rs" }),
    };
    let result = registry.execute(&call, &CancellationToken::new(), TIMEOUT).await;
    assert!(!result.is_error, "unexpected: {}", result_text(&result));
    let text = result_text(&result);
    assert!(text.contains("main.rs"), "missing main.rs: {text}");
    assert!(!text.contains("README.md"), "non-rs leaked: {text}");
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn grep_no_match_returns_no_matches() {
    let (root, _sandbox, registry) = search_fixture("grep_nomatch");
    let call = ToolCall {
        id: "s3".to_string(),
        name: "grep".to_string(),
        input: json!({ "pattern": "zzz_none" }),
    };
    let result = registry.execute(&call, &CancellationToken::new(), TIMEOUT).await;
    assert!(!result.is_error);
    assert_eq!(result_text(&result), "No matches found");
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn grep_rejects_comma_include() {
    let (root, _sandbox, registry) = search_fixture("grep_comma");
    let call = ToolCall {
        id: "s4".to_string(),
        name: "grep".to_string(),
        input: json!({ "pattern": "hello", "include": "*.rs,*.md" }),
    };
    let result = registry.execute(&call, &CancellationToken::new(), TIMEOUT).await;
    assert!(result.is_error);
    assert!(result_text(&result).contains("single positive glob"));
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn glob_and_grep_are_readonly_all_modes() {
    let (root, sandbox, _registry) = search_fixture("readonly");
    let glob = GlobTool::new(sandbox.clone());
    let grep = GrepTool::new(sandbox);
    assert_eq!(glob.risk(), ToolRisk::ReadOnly);
    assert_eq!(grep.risk(), ToolRisk::ReadOnly);
    assert_eq!(glob.modes(), None);
    assert_eq!(grep.modes(), None);
    // 名称与 schema。
    assert_eq!(glob.definition().name, "glob");
    assert_eq!(grep.definition().name, "grep");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn replace_swaps_registration_atomically() {
    let registry = ToolRegistry::new();
    registry.register(Arc::new(StubTool::new("old_tool", None, false)));
    registry.register(Arc::new(StubTool::new("keep", None, false)));
    let removed = registry.replace(
        &["old_tool".to_string()],
        vec![Arc::new(StubTool::new("new_tool", None, false)) as Arc<dyn ToolPort>],
    );
    assert_eq!(removed, vec!["old_tool".to_string()]);
    let mut names: Vec<String> = registry.risk_map().keys().cloned().collect();
    names.sort();
    assert_eq!(names, vec!["keep".to_string(), "new_tool".to_string()]);
    // replace 只换注册面，不触碰白名单（增删由调用方配对管理）。
    assert!(registry.state().agent_tools().is_empty());
}

#[test]
fn agent_whitelist_delta_add_and_remove() {
    let registry = ToolRegistry::new();
    set_agent(&registry, &["base_tool"]);
    registry.state().add_agent_tools(["mcp__srv__new".to_string()]);
    assert_eq!(
        registry.state().agent_tools(),
        ["base_tool".to_string(), "mcp__srv__new".to_string()].into_iter().collect()
    );
    registry.state().remove_agent_tools(["mcp__srv__new".to_string()]);
    assert_eq!(registry.state().agent_tools(), ["base_tool".to_string()].into_iter().collect());
}
