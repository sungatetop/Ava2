//! `system_prompt` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。
//! 覆盖：段排序与拼接、严格插值（成功 / 未知 / 未赋值 / 格式错误 / 字面）、
//! interpolate:false、complete 段、工具序、快照渲染、抑制、动态求值、重复注册。

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use super::*;
use crate::types::ToolDefinition;

fn section(name: &str, order: i64, text: &str) -> PromptSection {
    PromptSection {
        name: name.to_string(),
        order,
        text: SectionText::Static(text.to_string()),
        interpolate: true,
        complete: false,
    }
}

fn assembly_with(sections: Vec<PromptSection>) -> PromptAssembly {
    let mut prompts = SystemPrompt::new(&bare_config());
    for section in sections {
        prompts.section(section).unwrap();
    }
    prompts.assemble().unwrap()
}

fn bare_config() -> Config {
    Config {
        model: "fake".to_string(),
        base_url: String::new(),
        api_key: String::new(),
        max_tokens: 100,
        temperature: None,
        max_retries: 1,
        tool_timeout: std::time::Duration::from_secs(5),
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
        workspace_enabled: true,
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

fn tool(name: &str) -> ToolDefinition {
    ToolDefinition { name: name.to_string(), description: String::new(), parameters: serde_json::json!({}) }
}

#[test]
fn sections_sorted_by_order_then_name() {
    let assembly = assembly_with(vec![
        section("b", 10, "B"),
        section("a", 10, "A"),
        section("z", 0, "Z"),
    ]);
    assert_eq!(render_prompt(&assembly).unwrap(), "Z\n\nA\n\nB");
}

#[test]
fn empty_sections_dropped() {
    let assembly = assembly_with(vec![
        section("empty", 0, ""),
        section("kept", 1, "kept"),
    ]);
    assert_eq!(render_prompt(&assembly).unwrap(), "kept");
}

#[test]
fn variable_interpolation_basic() {
    let mut prompts = SystemPrompt::new(&bare_config());
    prompts.section(section("s", 0, "model is {{model}}, os {{os}}")).unwrap();
    prompts.variable("model", Arc::new(|| Some("Qwen".to_string()))).unwrap();
    prompts.variable("os", Arc::new(|| Some("windows".to_string()))).unwrap();
    let assembly = prompts.assemble().unwrap();
    assert_eq!(render_prompt(&assembly).unwrap(), "model is Qwen, os windows");
}

#[test]
fn unknown_variable_errors() {
    let mut prompts = SystemPrompt::new(&bare_config());
    prompts.section(section("s", 0, "x {{missing}}")).unwrap();
    prompts.variable("known", Arc::new(|| Some("v".to_string()))).unwrap();
    let error = render_prompt(&prompts.assemble().unwrap()).unwrap_err();
    assert_eq!(
        error,
        PromptError::UnknownVariable {
            kind: "section",
            name: "s".to_string(),
            variable: "missing".to_string(),
            known: vec!["known".to_string()],
        }
    );
}

#[test]
fn unassigned_variable_errors() {
    let mut prompts = SystemPrompt::new(&bare_config());
    prompts.section(section("s", 0, "x {{later}}")).unwrap();
    prompts.variable("later", Arc::new(|| None)).unwrap();
    let error = render_prompt(&prompts.assemble().unwrap()).unwrap_err();
    assert!(matches!(error, PromptError::UnassignedVariable { variable, .. } if variable == "later"));
}

#[test]
fn malformed_references_errors() {
    for text in ["{{}}", "{{a b}}", "{{a{b}}"] {
        let mut prompts = SystemPrompt::new(&bare_config());
        prompts.section(section("s", 0, text)).unwrap();
        let error = render_prompt(&prompts.assemble().unwrap()).unwrap_err();
        assert!(
            matches!(error, PromptError::MalformedReference { .. }),
            "“{text}”应为格式错误引用"
        );
    }
}

#[test]
fn lone_open_brace_is_literal_prose() {
    // 其后无 `}}` 的孤立 `{{` 是字面文本。
    let assembly = assembly_with(vec![section("s", 0, "a {{ b")]);
    assert_eq!(render_prompt(&assembly).unwrap(), "a {{ b");
    // 替换值不再扫描：值中的 `{{` 保持字面。
    let mut prompts = SystemPrompt::new(&bare_config());
    prompts.section(section("s", 0, "{{v}}")).unwrap();
    prompts.variable("v", Arc::new(|| Some("{{not_a_ref}}".to_string()))).unwrap();
    assert_eq!(render_prompt(&prompts.assemble().unwrap()).unwrap(), "{{not_a_ref}}");
}

#[test]
fn interpolate_false_preserves_literal() {
    let mut prompts = SystemPrompt::new(&bare_config());
    prompts.section(PromptSection {
        name: "literal".to_string(),
        order: 0,
        text: SectionText::Static("keep {{raw}}".to_string()),
        interpolate: false,
        complete: false,
    })
    .unwrap();
    let assembly = prompts.assemble().unwrap();
    assert_eq!(render_prompt(&assembly).unwrap(), "keep {{raw}}");
}

#[test]
fn complete_section_becomes_sole_prompt() {
    let mut prompts = SystemPrompt::new(&bare_config());
    prompts.section(section("normal", 0, "normal")).unwrap();
    prompts
        .section(PromptSection {
            name: "complete".to_string(),
            order: 10,
            text: SectionText::Static("the whole prompt".to_string()),
            interpolate: false,
            complete: true,
        })
        .unwrap();
    let assembly = prompts.assemble().unwrap();
    assert_eq!(render_prompt(&assembly).unwrap(), "the whole prompt");
    // complete 段只收编 sections；上下文与工具保留。
    assert_eq!(assembly.contexts.len(), 0);
}

#[test]
fn multiple_complete_sections_fail() {
    let mut prompts = SystemPrompt::new(&bare_config());
    prompts.section(PromptSection {
        name: "c1".to_string(), order: 0, text: SectionText::Static("1".to_string()),
        interpolate: false, complete: true,
    })
    .unwrap();
    prompts.section(PromptSection {
        name: "c2".to_string(), order: 1, text: SectionText::Static("2".to_string()),
        interpolate: false, complete: true,
    })
    .unwrap();
    assert_eq!(prompts.assemble().unwrap_err(), PromptError::MultipleComplete(vec!["c1".into(), "c2".into()]));
}

#[test]
fn tool_order_rest_insertion() {
    let mut prompts = SystemPrompt::new(&bare_config());
    prompts.tool_order = Some(vec!["read_file".to_string(), TOOL_ORDER_REST.to_string()]);
    prompts.add_tool_provider(Arc::new(|| {
        vec![tool("echo"), tool("write"), tool("list_dir"), tool("read_file")]
    }));
    let names: Vec<String> = prompts.assemble().unwrap().tools.into_iter().map(|t| t.name).collect();
    // 显式序在前，rest 处插入未列出工具的字典序（echo < list_dir < write）。
    assert_eq!(names, vec!["read_file", "echo", "list_dir", "write"]);
}

#[test]
fn tool_order_defaults_to_lexicographic() {
    let mut prompts = SystemPrompt::new(&bare_config());
    prompts.add_tool_provider(Arc::new(|| vec![tool("write"), tool("echo")]));
    let names: Vec<String> = prompts.assemble().unwrap().tools.into_iter().map(|t| t.name).collect();
    assert_eq!(names, vec!["echo", "write"]);
}

#[test]
fn tool_order_validation() {
    let mut prompts = SystemPrompt::new(&bare_config());
    prompts.add_tool_provider(Arc::new(|| vec![tool("echo")]));
    // 无 rest 标记
    prompts.tool_order = Some(vec!["echo".to_string()]);
    assert_eq!(prompts.assemble().unwrap_err(), PromptError::ToolOrderMissingRest);
    // rest 标记重复：第二次出现先触发重复检查
    prompts.tool_order =
        Some(vec![TOOL_ORDER_REST.to_string(), TOOL_ORDER_REST.to_string()]);
    assert_eq!(
        prompts.assemble().unwrap_err(),
        PromptError::ToolOrderDuplicate(TOOL_ORDER_REST.to_string())
    );
    // 普通名重复列出（与 rest 无关，同样拒绝）
    prompts.tool_order = Some(vec!["echo".to_string(), "echo".to_string(), TOOL_ORDER_REST.to_string()]);
    assert_eq!(prompts.assemble().unwrap_err(), PromptError::ToolOrderDuplicate("echo".into()));
    // provider 返回保留名
    prompts.tool_order = Some(vec![TOOL_ORDER_REST.to_string()]);
    let mut reserved = SystemPrompt::new(&bare_config());
    reserved.add_tool_provider(Arc::new(|| vec![tool(TOOL_ORDER_REST)]));
    reserved.tool_order = Some(vec![TOOL_ORDER_REST.to_string()]);
    assert_eq!(
        reserved.assemble().unwrap_err(),
        PromptError::ReservedToolName(TOOL_ORDER_REST.to_string())
    );
}

#[test]
fn context_snapshot_prefix_and_order() {
    let mut prompts = SystemPrompt::new(&bare_config());
    prompts
        .context(PromptContext {
            name: "late".to_string(),
            order: 200,
            text: SectionText::Static("late text".to_string()),
            interpolate: false,
            required: false,
        })
        .unwrap();
    prompts
        .context(PromptContext {
            name: "early".to_string(),
            order: 100,
            text: SectionText::Static("early text".to_string()),
            interpolate: false,
            required: false,
        })
        .unwrap();
    let snapshot = render_context_snapshot(&prompts.assemble().unwrap()).unwrap();
    assert_eq!(
        snapshot,
        "Current runtime context. This snapshot supersedes earlier runtime-context snapshots.\n\nearly text\n\nlate text"
    );
}

#[test]
fn suppress_runtime_context_keeps_required() {
    let mut prompts = SystemPrompt::new(&bare_config());
    prompts
        .context(PromptContext {
            name: "optional".to_string(),
            order: 0,
            text: SectionText::Static("opt".to_string()),
            interpolate: false,
            required: false,
        })
        .unwrap();
    prompts
        .context(PromptContext {
            name: "required".to_string(),
            order: 1,
            text: SectionText::Static("req".to_string()),
            interpolate: false,
            required: true,
        })
        .unwrap();
    prompts.suppress_runtime_context();
    let snapshot = render_context_snapshot(&prompts.assemble().unwrap()).unwrap();
    assert!(snapshot.ends_with("req"), "required 上下文保留：{snapshot}");
    assert!(!snapshot.contains("opt"), "可选上下文被抑制");
}

#[test]
fn dynamic_provider_evaluated_per_assembly() {
    let counter = Arc::new(AtomicU32::new(0));
    let tick = counter;
    let mut prompts = SystemPrompt::new(&bare_config());
    prompts.section(PromptSection {
        name: "clock".to_string(),
        order: 0,
        text: SectionText::Dynamic(Arc::new(move || {
            format!("n={}", tick.fetch_add(1, Ordering::SeqCst))
        })),
        interpolate: false,
        complete: false,
    })
    .unwrap();
    assert_eq!(render_prompt(&prompts.assemble().unwrap()).unwrap(), "n=0");
    assert_eq!(render_prompt(&prompts.assemble().unwrap()).unwrap(), "n=1");
}

#[test]
fn plan_policy_section_active_inactive_and_tool_stability() {
    // plan:policy 分节行为：active 时输出策略文本、inactive 空串消失；
    // 工具目录两模式逐字节相同（plan 只改 prompt 分节，不改工具目录）。
    let active = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let mut prompts = SystemPrompt::new(&bare_config());
    prompts.section(PromptSection {
        name: "plan:policy".to_string(),
        order: super::section_orders::PLAN_POLICY,
        text: SectionText::Dynamic({
            let active = active.clone();
            Arc::new(move || {
                if !active.load(Ordering::SeqCst) {
                    String::new()
                } else {
                    "You are in plan mode.".to_string()
                }
            })
        }),
        interpolate: false,
        complete: false,
    })
    .unwrap();
    prompts
        .section(PromptSection {
            name: "mission".to_string(),
            order: 1000,
            text: SectionText::Static("mission body".to_string()),
            interpolate: false,
            complete: false,
        })
        .unwrap();
    prompts.add_tool_provider(Arc::new(|| vec![tool("echo"), tool("write")]));

    // active：plan:policy 文本出现，工具目录稳定。
    let active_assembly = prompts.assemble().unwrap();
    let active_prompt = render_prompt(&active_assembly).unwrap();
    assert!(active_prompt.contains("You are in plan mode."), "active 应含 plan 策略文本");
    let active_tools: Vec<String> = active_assembly.tools.iter().map(|t| t.name.clone()).collect();

    // inactive：plan:policy 空串消失，工具目录逐字节相同。
    active.store(false, Ordering::SeqCst);
    let inactive_assembly = prompts.assemble().unwrap();
    let inactive_prompt = render_prompt(&inactive_assembly).unwrap();
    assert!(!inactive_prompt.contains("You are in plan mode."), "inactive 不应含 plan 策略文本");
    assert!(inactive_prompt.contains("mission body"), "inactive 仍应保留其他段");
    let inactive_tools: Vec<String> = inactive_assembly.tools.iter().map(|t| t.name.clone()).collect();
    assert_eq!(active_tools, inactive_tools, "工具目录两模式必须逐字节相同");
}

#[test]
fn duplicate_registrations_rejected() {
    let mut prompts = SystemPrompt::new(&bare_config());
    prompts.section(section("dup", 0, "a")).unwrap();
    assert_eq!(prompts.section(section("dup", 1, "b")).unwrap_err(), PromptError::DuplicateSection("dup".into()));
    prompts
        .context(PromptContext {
            name: "ctx".to_string(), order: 0, text: SectionText::Static("x".to_string()),
            interpolate: false, required: false,
        })
        .unwrap();
    let again = PromptContext {
        name: "ctx".to_string(), order: 1, text: SectionText::Static("y".to_string()),
        interpolate: false, required: false,
    };
    assert_eq!(prompts.context(again).unwrap_err(), PromptError::DuplicateContext("ctx".into()));
    prompts.variable("v", Arc::new(|| Some("1".to_string()))).unwrap();
    assert_eq!(
        prompts.variable("v", Arc::new(|| Some("2".to_string()))).unwrap_err(),
        PromptError::DuplicateVariable("v".into())
    );
    assert!(matches!(
        prompts.variable("Bad-Name", Arc::new(|| None)).unwrap_err(),
        PromptError::InvalidVariableName(_)
    ));
}

#[test]
fn new_applies_config_baselines() {
    let mut config = bare_config();
    config.include_identity = true;
    config.persona_prefix = "prefix {{model}}".to_string();
    config.persona_suffix = "suffix".to_string();
    let mut prompts = SystemPrompt::new(&config);
    prompts.variable("model", Arc::new(|| Some("M".to_string()))).unwrap();
    let text = render_prompt(&prompts.assemble().unwrap()).unwrap();
    assert_eq!(
        text,
        "You are an AI agent powered by the onion harness.\n\nprefix M\n\nsuffix"
    );
    // 抑制开关由 Config 驱动
    config.include_runtime_context = false;
    let mut prompts = SystemPrompt::new(&config);
    prompts
        .context(PromptContext {
            name: "opt".to_string(), order: 0, text: SectionText::Static("x".to_string()),
            interpolate: false, required: false,
        })
        .unwrap();
    assert_eq!(render_context_snapshot(&prompts.assemble().unwrap()).unwrap(), "");
}
