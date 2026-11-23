//! `skill::tools` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。
use std::sync::{Arc, Mutex};

use serde_json::json;

use super::SkillTool;
use crate::session::SessionLog;
use crate::skill::registry::SkillRegistry;
use crate::skill::{SkillInvocationPolicy, SkillRegistration, SkillSource};
use crate::tools::ToolPort;
use crate::types::{ContentBlock, ToolCall};

fn registry() -> Arc<Mutex<SkillRegistry>> {
    Arc::new(Mutex::new(SkillRegistry::new()))
}

fn session() -> Arc<SessionLog> {
    Arc::new(SessionLog::in_memory())
}

fn tool(registry: Arc<Mutex<SkillRegistry>>) -> SkillTool {
    SkillTool::new(registry)
}

fn registration(name: &str, content: &str, model_invocable: bool) -> SkillRegistration {
    SkillRegistration {
        name: name.to_string(),
        description: format!("desc {name}"),
        when_to_use: None,
        source: SkillSource::Runtime,
        resource_base: None,
        content: content.to_string(),
        invocation: Some(SkillInvocationPolicy {
            model_invocable,
            user_invocable: true,
        }),
        provider: None,
    }
}

fn call(name: &str, input: serde_json::Value) -> ToolCall {
    ToolCall { id: "call-1".to_string(), name: name.to_string(), input }
}

async fn run_text(tool: &SkillTool, call: ToolCall) -> Result<String, String> {
    let def = tool.definition();
    assert_eq!(def.name, "skill");
    let blocks = tool
        .execute(call, tokio_util::sync::CancellationToken::new())
        .await?;
    let mut text = String::new();
    for block in blocks {
        if let ContentBlock::Text { text: t } = block {
            text.push_str(&t);
        }
    }
    Ok(text)
}

#[tokio::test]
async fn skill_requires_owning_session() {
    let tool = tool(registry());
    let result = tool
        .execute(
            call("skill", json!({ "name": "foo" })),
            tokio_util::sync::CancellationToken::new(),
        )
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn skill_missing_name_rejected() {
    let sess = session();
    let tool = tool(registry());
    tool.wire_session(sess.clone());
    let result = run_text(&tool, call("skill", json!({}))).await;
    assert!(result.unwrap_err().contains("missing 'name'"));
}

#[tokio::test]
async fn skill_invalid_name_rejected() {
    let sess = session();
    let tool = tool(registry());
    tool.wire_session(sess.clone());
    let result = run_text(&tool, call("skill", json!({ "name": "Bad_Name" }))).await;
    assert!(result.unwrap_err().contains("invalid skill name"));
}

#[tokio::test]
async fn skill_unknown_rejected() {
    let sess = session();
    let tool = tool(registry());
    tool.wire_session(sess.clone());
    let result = run_text(&tool, call("skill", json!({ "name": "missing" }))).await;
    assert!(result.unwrap_err().contains("unknown or no longer available"));
}

#[tokio::test]
async fn skill_not_model_invocable_rejected() {
    let reg = registry();
    {
        let mut guard = reg.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.register(registration("user-only", "User only body.", false));
    }
    let sess = session();
    let tool = tool(reg);
    tool.wire_session(sess.clone());
    let result = run_text(&tool, call("skill", json!({ "name": "user-only" }))).await;
    assert!(result.unwrap_err().contains("not available for model invocation"));
}

#[tokio::test]
async fn skill_loads_and_renders_content() {
    let reg = registry();
    {
        let mut guard = reg.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.register(registration("my-skill", "Do the thing.", true));
    }
    let sess = session();
    let tool = tool(reg);
    tool.wire_session(sess.clone());
    let text = run_text(&tool, call("skill", json!({ "name": "my-skill" }))).await.unwrap();
    assert!(text.starts_with("<skill_content name=\"my-skill\">"), "{text}");
    assert!(text.contains("Do the thing."), "{text}");
    assert!(text.contains("<skill_instructions>"), "{text}");
}
