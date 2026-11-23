//! 跨实例审批桥（P2-2）：B 侧人机交互挂起 → 按 `reportTo` 直推 A 侧裁决会话。
//!
//! 仅 `plan-review` 与 `ask-user` 两类人机交互项跨实例推送；工具执行在实例内
//! 自动放行（见 `docs/onion/solutions/a2a-collaboration-next-steps-plan.md` §2.5）。
//!
//! 路由来源：委派方在 `message.metadata.reportTo` 声明 `{instanceId, sessionId}`
//! （`A2aSendTool` 自动填），B 侧按 `instanceId` 在静态对端表（`ONION_A2A_PEERS`）
//! 中查 URL——查不到即不推送（无兜底分支）。

use std::time::Duration;

use serde_json::{json, Value};

use crate::a2a::{ContextId, Message, Part, Role};
use crate::rest::dto::PeerInfo;
use crate::session::PendingKind;

/// 确认请求推送超时（A 侧裁决轮次可能包含一次真实模型调用与 `a2a_approve`）。
const PUSH_TIMEOUT: Duration = Duration::from_secs(600);

/// 推送目标：A 侧 `/a2a` 基址 + A 侧裁决会话 id。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Route {
    pub peer_url: String,
    pub session_id: String,
}

/// 跨实例审批桥（peers 表 + 自身对外 URL + mesh 令牌）。
pub(crate) struct Bridge {
    peers: Vec<PeerInfo>,
    /// 本实例对外基址（写入确认请求，供 A 侧 `a2a_approve` 回填）。
    own_url: String,
    token: Option<String>,
    client: reqwest::Client,
}

impl Bridge {
    pub(crate) fn new(peers: Vec<PeerInfo>, own_url: String, token: Option<String>) -> Self {
        Self {
            peers,
            own_url,
            token,
            client: reqwest::Client::new(),
        }
    }

    /// `reportTo{instanceId, sessionId}` → 推送目标；对端不在 peers 表即 `None`。
    pub(crate) fn route(&self, report_to: Option<&Value>) -> Option<Route> {
        let report_to = report_to?;
        let instance_id = report_to.get("instanceId")?.as_str()?;
        let session_id = report_to.get("sessionId")?.as_str()?;
        let peer = self.peers.iter().find(|peer| peer.name == instance_id)?;
        Some(Route {
            peer_url: peer.url.trim_end_matches('/').to_string(),
            session_id: session_id.to_string(),
        })
    }

    /// 把挂起的人机交互项组装为 `message/send` 直推 A 侧裁决会话。
    ///
    /// 推送在后台任务中执行（`message/send` 会阻塞到 A 侧裁决轮次结束）；
    /// 失败仅记录日志，B 侧任务保持 `input-required`。
    pub(crate) fn push_confirm(
        &self,
        route: &Route,
        task_id: &str,
        request_id: &str,
        kind: &PendingKind,
    ) {
        let client = self.client.clone();
        let token = self.token.clone();
        let own_url = self.own_url.clone();
        let route = route.clone();
        let task_id = task_id.to_string();
        let request_id = request_id.to_string();
        let kind = kind.clone();
        tokio::spawn(async move {
            let rpc = build_confirm_rpc(&route, &own_url, &task_id, &request_id, &kind);
            let endpoint = format!("{}/a2a", route.peer_url);
            let mut request = client.post(&endpoint).json(&rpc);
            if let Some(token) = &token {
                request = request.header("x-onion-token", token.as_str());
            }
            match tokio::time::timeout(PUSH_TIMEOUT, request.send()).await {
                Ok(Ok(response)) => {
                    let status = response.status().as_u16();
                    let body = response.text().await.unwrap_or_default();
                    eprintln!(
                        "[onion-server] 确认请求已推送 {endpoint}（HTTP {status}）：{body}"
                    );
                }
                Ok(Err(error)) => {
                    eprintln!("[onion-server] 确认请求推送失败 {endpoint}: {error}");
                }
                Err(_) => {
                    eprintln!(
                        "[onion-server] 确认请求推送超时 {endpoint}（{}s）",
                        PUSH_TIMEOUT.as_secs()
                    );
                }
            }
        });
    }
}

/// 组装确认请求的 `message/send` JSON-RPC（纯函数，便于线格式单测）。
pub(crate) fn build_confirm_rpc(
    route: &Route,
    own_url: &str,
    task_id: &str,
    request_id: &str,
    kind: &PendingKind,
) -> Value {
    let message = Message {
        role: Role::User,
        parts: vec![Part::text(confirm_text(own_url, task_id, request_id, kind))],
        kind: Some("message".to_string()),
        message_id: Some(format!("confirm-{}", onion::session::epoch_millis())),
        task_id: None,
        context_id: Some(ContextId::new(route.session_id.clone())),
        metadata: Some(json!({
            "confirmRequest": {
                "requestId": request_id,
                "item": kind.label(),
                "taskId": task_id,
                "peerUrl": own_url,
                "payload": kind,
            }
        })),
    };
    json!({
        "jsonrpc": "2.0",
        "id": request_id,
        "method": "message/send",
        "params": { "message": message },
    })
}

/// 确认请求正文（A 侧模型据此调用 `a2a_approve`）。
fn confirm_text(own_url: &str, task_id: &str, request_id: &str, kind: &PendingKind) -> String {
    format!(
        "[A2A 确认请求] 对端实例 `{own_url}` 的任务 `{task_id}` 挂起，等待你的裁决。\n\
         requestId: `{request_id}`\nitem: {item}\n\n{payload}\n\n\
         裁决方式：调用工具 a2a_approve，参数 \
         {{\"url\": \"{own_url}\", \"taskId\": \"{task_id}\", \"requestId\": \"{request_id}\", \
         \"decision\": \"approve\" 或 \"deny\"}}。\
         ask_user 类型必须带 answers：[{{\"id\": \"<问题 id>\", \"selected\": [\"<选项标签>\"], \
         \"custom\": \"<可选自定义答复>\"}}]。",
        item = kind.label(),
        payload = kind.summary(),
    )
}

#[cfg(test)]
#[path = "bridge_tests.rs"]
mod tests;
