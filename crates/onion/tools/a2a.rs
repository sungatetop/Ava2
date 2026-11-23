//! `a2a_call` / `a2a_send`：A2A 出站工具族（对齐 onion-server-a2a-guide §4.4 P0）。
//!
//! - `a2a_call`（ReadOnly）：GET 探测对端——`/.well-known/agent-card.json`、
//!   `/healthz`、`/readyz`；发现能力/存活/就绪。
//! - `a2a_send`（WorkspaceWrite）：POST `/a2a` 发 JSON-RPC `message/send`，委派任务。
//!   工具执行默认实例内自动放行（不跨实例推人工审批）；仅 plan-review 与
//!   ask-user 两类人机交互项走外部确认（见 docs/onion/solutions/
//!   a2a-collaboration-next-steps-plan.md §2.5）。
//!
//! 安全：`a2a_call` 只探公开面（AgentCard / healthz / readyz），不带任何头；
//! `a2a_send` 在配置了 mesh 令牌（`ONION_A2A_TOKEN`）时随请求带 `X-Onion-Token`，
//! 对端 `/a2a` 接受 mesh 令牌或运维令牌任一。运维面 `/v1/*` 仍只认
//! `ONION_SERVER_TOKEN`（两个信任域分离）。

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::ToolPort;
use crate::approval::ToolRisk;
use crate::types::{ContentBlock, ToolCall, ToolDefinition};
use crate::waterfall::BoxFuture;

/// `a2a_call` 探测超时（对齐 web_fetch 10s 常规口径）。
const CALL_TIMEOUT: Duration = Duration::from_secs(10);

/// `a2a_send` 委派超时（委派任务可能较慢）。
const SEND_TIMEOUT: Duration = Duration::from_secs(30);

/// `a2a_approve` 回填超时（对端裁决后需等该侧轮次跑完，故比 `a2a_send` 宽松）。
const APPROVE_TIMEOUT: Duration = Duration::from_secs(180);

/// `a2a_call`：GET 探测对端（AgentCard / healthz / readyz），ReadOnly。
pub struct A2aCallTool {
    client: reqwest::Client,
}

impl A2aCallTool {
    pub fn new() -> Self {
        Self { client: reqwest::Client::new() }
    }
}

impl Default for A2aCallTool {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolPort for A2aCallTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "a2a_call".to_string(),
            description: "Probe a peer onion instance over A2A discovery surface: fetch the \
                well-known agent card (capabilities/skills), healthz or readyz. Input: \
                {\"url\": string, \"path\": \"card\"|\"health\"|\"ready\"} — url is the peer's \
                base URL (e.g. http://127.0.0.1:9801), path selects what to read. Read-only; \
                no credentials are sent."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "Peer base URL, e.g. http://127.0.0.1:9801"
                    },
                    "path": {
                        "type": "string",
                        "enum": ["card", "health", "ready"],
                        "description": "What to probe: card = /.well-known/agent-card.json, \
                            health = /healthz, ready = /readyz. Defaults to card."
                    }
                },
                "required": ["url"]
            }),
        }
    }

    fn risk(&self) -> ToolRisk {
        ToolRisk::ReadOnly
    }

    fn execute(&self, call: ToolCall, cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let client = self.client.clone();
        Box::pin(async move {
            let url = call.input.get("url").and_then(serde_json::Value::as_str).unwrap_or("");
            if url.trim().is_empty() {
                return Err("url must be a non-empty string".to_string());
            }
            if !(url.starts_with("http://") || url.starts_with("https://")) {
                return Err("url must be an http(s) URL".to_string());
            }
            let path = call
                .input
                .get("path")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("card");
            let path = match path {
                "health" => "/healthz",
                "ready" => "/readyz",
                _ => "/.well-known/agent-card.json",
            };
            let full = format!("{}{}", url.trim_end_matches('/'), path);
            if cancel.is_cancelled() {
                return Err("a2a_call aborted".to_string());
            }
            let response = tokio::time::timeout(
                CALL_TIMEOUT,
                client
                    .get(&full)
                    .header("accept", "application/json")
                    .send(),
            )
            .await
            .map_err(|_| format!("a2a_call timed out after {}s", CALL_TIMEOUT.as_secs()))?
            .map_err(|e| format!("a2a_call failed: {e}"))?;
            let status = response.status().as_u16();
            let body = response
                .text()
                .await
                .map_err(|e| format!("a2a_call read failed: {e}"))?;
            Ok(vec![ContentBlock::Text { text: format!("HTTP {status}\n\n{body}") }])
        })
    }
}

/// 委派链接记录（`<sessionsDir>/<sessionId>/tasks/links.jsonl` 单行，append-only）。
///
/// 只记录「谁委派了谁、远端 task 是什么」，不迁 Task 本体；不做 GC（会话目录
/// 删除时随之清理）。`reportTo` 为发起方回传地址，跨实例审批桥按它路由。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DelegationLink {
    pub ts: u64,
    pub peer_url: String,
    pub remote_task_id: String,
    pub remote_context_id: String,
    pub remote_state: String,
    pub message_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_to: Option<serde_json::Value>,
}

/// 委派链接文件路径：`<sessionsDir>/<sessionId>/tasks/links.jsonl`。
pub fn links_path(sessions_dir: &Path, session_id: &str) -> PathBuf {
    crate::paths::session_dir(sessions_dir, session_id).join("tasks").join("links.jsonl")
}

/// 读取会话的委派链接（文件缺失即空；解析失败的行跳过）。
pub fn read_links(sessions_dir: &Path, session_id: &str) -> Vec<DelegationLink> {
    let Ok(text) = std::fs::read_to_string(links_path(sessions_dir, session_id)) else {
        return Vec::new();
    };
    text.lines().filter_map(|line| serde_json::from_str(line).ok()).collect()
}

/// 追加一行委派链接（`tasks/` 不存在则创建）。
fn append_link(sessions_dir: &Path, session_id: &str, link: &DelegationLink) -> Result<(), String> {
    let path = links_path(sessions_dir, session_id);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("links dir create failed: {e}"))?;
    }
    let mut line = serde_json::to_string(link).map_err(|e| format!("links encode failed: {e}"))?;
    line.push('\n');
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("links open failed: {e}"))?;
    file.write_all(line.as_bytes()).map_err(|e| format!("links append failed: {e}"))
}

/// 从 `message/send` 成功应答中提取委派链接（非成功 Task 应答即 `None`）。
fn task_link(
    body: &str,
    peer_url: &str,
    message_id: &str,
    report_to: &serde_json::Value,
) -> Option<DelegationLink> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    let result = value.get("result")?;
    Some(DelegationLink {
        ts: crate::session::epoch_millis(),
        peer_url: peer_url.trim_end_matches('/').to_string(),
        remote_task_id: result.get("id")?.as_str()?.to_string(),
        remote_context_id: result.get("contextId")?.as_str()?.to_string(),
        remote_state: result.get("status")?.get("state")?.as_str()?.to_string(),
        message_id: message_id.to_string(),
        report_to: Some(report_to.clone()),
    })
}

/// 组装 `message/send` 的 JSON-RPC 请求（纯函数，便于线格式单测）。
///
/// A2A `params` 是 `MessageSendParams { message, … }`，message 本体必须嵌在
/// `params.message` 下（否则对端 `-32602 invalid messageSendParams`）。
/// `reportTo` / `sender` 落在 `params.message.metadata`（`Message.metadata` 已有字段）：
/// 前者供审批桥回传路由，后者携带发送方身份供对端区分来源。
fn build_send_rpc(
    message_id: &str,
    message: &str,
    context_id: Option<&str>,
    report_to: Option<&serde_json::Value>,
    sender: Option<&serde_json::Value>,
) -> serde_json::Value {
    let mut payload = json!({
        "role": "user",
        "kind": "message",
        "messageId": message_id,
        "parts": [{ "kind": "text", "text": message }],
    });
    if let Some(context_id) = context_id {
        payload["contextId"] = json!(context_id);
    }
    let mut metadata = serde_json::Map::new();
    if let Some(report_to) = report_to {
        metadata.insert("reportTo".to_string(), report_to.clone());
    }
    if let Some(sender) = sender {
        metadata.insert("sender".to_string(), sender.clone());
    }
    if !metadata.is_empty() {
        payload["metadata"] = serde_json::Value::Object(metadata);
    }
    json!({
        "jsonrpc": "2.0",
        "id": message_id,
        "method": "message/send",
        "params": { "message": payload },
    })
}

/// `a2a_send`：POST `/a2a` 发 `message/send`（委派任务），WorkspaceWrite。
///
/// 成功发送后向 `<sessionsDir>/<sessionId>/tasks/links.jsonl` 追加委派链接；
/// 会话 id 经 [`A2aSendTool::wire_owner`] 延迟接线（未接线 fail-closed）。
pub struct A2aSendTool {
    client: reqwest::Client,
    /// A2A mesh 令牌（`ONION_A2A_TOKEN`）：非空时随请求带 `X-Onion-Token`。
    token: Option<String>,
    /// 会话落盘容器（`<...>/sessions/<normalized-cwd>/`）。
    sessions_dir: PathBuf,
    /// 本实例标识（`ONION_INSTANCE_ID`），写入委派链接的 `reportTo.instanceId`
    /// 与出站消息 `metadata.sender.instanceId`。
    instance_id: String,
    /// 本实例显示名（`ONION_INSTANCE_NAME`，缺省同实例标识），写入
    /// `metadata.sender.name`，供对端区分发送方。
    instance_name: String,
    /// 当前会话 id（构造早于 Runtime 装配，装配后接线）。
    owner: OnceLock<String>,
}

impl A2aSendTool {
    pub fn new(token: Option<String>, sessions_dir: PathBuf, instance_id: String, instance_name: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            token,
            sessions_dir,
            instance_id,
            instance_name,
            owner: OnceLock::new(),
        }
    }

    /// 接线当前会话 id（宿主 Runtime 装配完成后调用）。
    pub fn wire_owner(&self, session_id: impl Into<String>) {
        let _ = self.owner.set(session_id.into());
    }

    /// 取已接线会话 id；未接线返回 Err（fail-closed）。
    fn session_id(&self) -> Result<&str, String> {
        self.owner
            .get()
            .map(String::as_str)
            .ok_or_else(|| "session owner not wired (a2a_send requires a session)".to_string())
    }
}

impl ToolPort for A2aSendTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "a2a_send".to_string(),
            description: "Delegate a task to a peer onion instance: POST the A2A JSON-RPC \
                `message/send` method to the peer's /a2a endpoint and return the resulting \
                Task JSON (status.state = completed/failed/…; artifacts[].parts[].text). \
                Input: {\"url\": string, \"message\": string, \"contextId\": string(optional), \
                \"sender\": {instanceId?, name?, sessionId?}(optional)} \
                — url is the peer's base URL, message is the task text, contextId optionally \
                pins a peer session, sender carries your identity to the peer (defaults to this \
                instance's id/name and the current session). Runs without an approval prompt."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "Peer base URL, e.g. http://127.0.0.1:9801"
                    },
                    "message": {
                        "type": "string",
                        "description": "Task text to delegate."
                    },
                    "contextId": {
                        "type": "string",
                        "description": "Optional peer session id (contextId) to pin the task to."
                    },
                    "reportTo": {
                        "type": "object",
                        "description": "Optional caller-supplied report target written to \
                            message.metadata.reportTo; defaults to {instanceId, sessionId} of the \
                            current session."
                    },
                    "sender": {
                        "type": "object",
                        "description": "Optional sender identity written to message.metadata.sender; \
                            defaults to {instanceId, name, sessionId} of the current instance and \
                            session. Set name to the display name the peer should see."
                    }
                },
                "required": ["url", "message"]
            }),
        }
    }

    fn risk(&self) -> ToolRisk {
        ToolRisk::WorkspaceWrite
    }

    fn execute(&self, call: ToolCall, cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let client = self.client.clone();
        let token = self.token.clone();
        let sessions_dir = self.sessions_dir.clone();
        let instance_id = self.instance_id.clone();
        let instance_name = self.instance_name.clone();
        let session_id = match self.session_id() {
            Ok(id) => id.to_string(),
            Err(error) => return Box::pin(async move { Err(error) }),
        };
        Box::pin(async move {
            let report_to = call
                .input
                .get("reportTo")
                .cloned()
                .unwrap_or_else(|| json!({ "instanceId": instance_id, "sessionId": session_id }));
            // 发送方身份：默认取本实例 id/显示名 + 当前会话；调用方显式给出的
            // `sender` 字段覆盖同名键（便于扮演特定人物）。
            let mut sender =
                json!({ "instanceId": instance_id, "name": instance_name, "sessionId": session_id });
            if let (Some(given), Some(defaults)) =
                (call.input.get("sender").and_then(serde_json::Value::as_object), sender.as_object_mut())
            {
                for (key, value) in given {
                    defaults.insert(key.clone(), value.clone());
                }
            }
            let url = call.input.get("url").and_then(serde_json::Value::as_str).unwrap_or("");
            let message = call
                .input
                .get("message")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            if url.trim().is_empty() {
                return Err("url must be a non-empty string".to_string());
            }
            if !(url.starts_with("http://") || url.starts_with("https://")) {
                return Err("url must be an http(s) URL".to_string());
            }
            if message.trim().is_empty() {
                return Err("message must be a non-empty string".to_string());
            }
            let context_id = call
                .input
                .get("contextId")
                .and_then(serde_json::Value::as_str)
                .filter(|s| !s.trim().is_empty());
            let endpoint = format!("{}/a2a", url.trim_end_matches('/'));
            let message_id = uuid::Uuid::new_v4().to_string();
            let rpc = build_send_rpc(&message_id, message, context_id, Some(&report_to), Some(&sender));
            if cancel.is_cancelled() {
                return Err("a2a_send aborted".to_string());
            }
            let mut request = client.post(&endpoint).json(&rpc);
            if let Some(token) = &token {
                request = request.header("x-onion-token", token.as_str());
            }
            let response = tokio::time::timeout(SEND_TIMEOUT, request.send())
                .await
                .map_err(|_| format!("a2a_send timed out after {}s", SEND_TIMEOUT.as_secs()))?
                .map_err(|e| format!("a2a_send failed: {e}"))?;
            let status = response.status().as_u16();
            let body = response
                .text()
                .await
                .map_err(|e| format!("a2a_send read failed: {e}"))?;
            if status == 200 {
                if let Some(link) = task_link(&body, url, &message_id, &report_to) {
                    append_link(&sessions_dir, &session_id, &link)?;
                }
            }
            Ok(vec![ContentBlock::Text { text: format!("HTTP {status}\n\n{body}") }])
        })
    }
}

/// `build_provide_input_rpc`：组装 `tasks/provideInput` 的 JSON-RPC 请求
/// （纯函数，便于线格式单测）。`decision` 用于 plan-review（approve / deny），
/// `answers` 用于 ask-user（`[{id, selected, custom}]`）。
fn build_provide_input_rpc(
    task_id: &str,
    request_id: &str,
    decision: Option<&str>,
    answers: Option<&serde_json::Value>,
) -> serde_json::Value {
    let mut params = json!({ "taskId": task_id, "requestId": request_id });
    if let Some(decision) = decision {
        params["decision"] = json!(decision);
    }
    if let Some(answers) = answers {
        params["answers"] = answers.clone();
    }
    json!({
        "jsonrpc": "2.0",
        "id": request_id,
        "method": "tasks/provideInput",
        "params": params,
    })
}

/// `a2a_approve`：把裁决回填给挂起中的委派任务（`tasks/provideInput`），
/// WorkspaceWrite。
///
/// 触发场景：对端委派任务返回 `status.state = input-required`，或收到带
/// `confirmRequest` 元数据的确认请求消息。回填后对端继续该轮次，本工具返回
/// 收敛后的 Task JSON（`status.state = completed/…`）。
pub struct A2aApproveTool {
    client: reqwest::Client,
    /// A2A mesh 令牌（`ONION_A2A_TOKEN`）：非空时随请求带 `X-Onion-Token`。
    token: Option<String>,
}

impl A2aApproveTool {
    pub fn new(token: Option<String>) -> Self {
        Self { client: reqwest::Client::new(), token }
    }
}

impl ToolPort for A2aApproveTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "a2a_approve".to_string(),
            description: "Resolve a pending human-interaction item of a delegated A2A task: \
                POST the A2A JSON-RPC `tasks/provideInput` method to the peer's /a2a endpoint. \
                Input: {\"url\": string, \"taskId\": string, \"requestId\": string, \
                \"decision\": \"approve\"|\"deny\" (plan review), \"answers\": [{\"id\", \
                \"selected\": [...], \"custom\": string}] (ask_user)}. Returns the peer Task JSON \
                once its turn converges. Tool-execution approvals need no call — peers auto-allow \
                them."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "Peer base URL, e.g. http://127.0.0.1:9801"
                    },
                    "taskId": {
                        "type": "string",
                        "description": "Peer task id carried by the confirmation request."
                    },
                    "requestId": {
                        "type": "string",
                        "description": "Pending request id carried by the confirmation request."
                    },
                    "decision": {
                        "type": "string",
                        "enum": ["approve", "deny"],
                        "description": "plan-review verdict: approve runs the plan; deny keeps \
                            planning."
                    },
                    "answers": {
                        "type": "array",
                        "description": "ask-user answers, one per question: {id, selected, custom}.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "string" },
                                "selected": { "type": "array", "items": { "type": "string" } },
                                "custom": { "type": "string" }
                            },
                            "required": ["id"]
                        }
                    }
                },
                "required": ["url", "taskId", "requestId"]
            }),
        }
    }

    fn risk(&self) -> ToolRisk {
        ToolRisk::WorkspaceWrite
    }

    /// 阻断型：回填后要等对端轮次跑完才拿到终态 Task，耗时由对端决定，不参与
    /// 注册表统一 `tool_timeout`（自身 [`APPROVE_TIMEOUT`] 兜底）。
    fn blocking(&self) -> bool {
        true
    }

    fn execute(&self, call: ToolCall, cancel: CancellationToken) -> BoxFuture<Result<Vec<ContentBlock>, String>> {
        let client = self.client.clone();
        let token = self.token.clone();
        Box::pin(async move {
            let url = call.input.get("url").and_then(serde_json::Value::as_str).unwrap_or("");
            let task_id = call
                .input
                .get("taskId")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            let request_id = call
                .input
                .get("requestId")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            if url.trim().is_empty() {
                return Err("url must be a non-empty string".to_string());
            }
            if !(url.starts_with("http://") || url.starts_with("https://")) {
                return Err("url must be an http(s) URL".to_string());
            }
            if task_id.trim().is_empty() {
                return Err("taskId must be a non-empty string".to_string());
            }
            if request_id.trim().is_empty() {
                return Err("requestId must be a non-empty string".to_string());
            }
            let decision = call
                .input
                .get("decision")
                .and_then(serde_json::Value::as_str)
                .filter(|s| !s.trim().is_empty());
            let answers = call.input.get("answers").filter(|value| !value.is_null());
            if decision.is_none() && answers.is_none() {
                return Err("decision (plan review) or answers (ask_user) is required".to_string());
            }
            let endpoint = format!("{}/a2a", url.trim_end_matches('/'));
            let rpc = build_provide_input_rpc(task_id, request_id, decision, answers);
            if cancel.is_cancelled() {
                return Err("a2a_approve aborted".to_string());
            }
            let mut request = client.post(&endpoint).json(&rpc);
            if let Some(token) = &token {
                request = request.header("x-onion-token", token.as_str());
            }
            let response = tokio::time::timeout(APPROVE_TIMEOUT, request.send())
                .await
                .map_err(|_| format!("a2a_approve timed out after {}s", APPROVE_TIMEOUT.as_secs()))?
                .map_err(|e| format!("a2a_approve failed: {e}"))?;
            let status = response.status().as_u16();
            let body = response
                .text()
                .await
                .map_err(|e| format!("a2a_approve read failed: {e}"))?;
            Ok(vec![ContentBlock::Text { text: format!("HTTP {status}\n\n{body}") }])
        })
    }
}

#[cfg(test)]
#[path = "a2a_tests.rs"]
mod tests;