//! AskUserService：阻塞问询 seam（legacy/timed 两模式）。
//!
//! 工具 execute 挂起直到宿主（main）呈示问题并回填人工答复，答复作为普通
//! 工具结果回到 agent loop。与审批/plan-review 同用「mpsc 请求 → oneshot 答复」
//! 挂起-答复 seam：工具经 mpsc 通道发请求给宿主，宿主呈示后经 oneshot 回填。
//!
//! timed 模式：携带 `timeout`（秒，`-1`=无限）时，前台等待
//! 逾限返回 pending 结果（不中断 agent loop），问题保持可答；用户迟到答复经
//! `AnswerToPending` source 注入 agent inbox。

use std::sync::{Arc, Mutex, OnceLock};

use tokio::sync::{mpsc, oneshot};

use crate::session::SessionLog;
use crate::types::{AgentMessage, ContentBlock, MessageSource, Role};

use super::types::{AskUserAnswer, AskUserQuestion};

/// 问询请求（tool → main）。
pub struct AskUserRequest {
    pub questions: Vec<AskUserQuestion>,
    pub reply: oneshot::Sender<AskUserAnswer>,
    /// 前台等待秒数：`None` = legacy 无限阻塞；
    /// `Some(-1)` = timed 无限阻塞；`Some(n)` = timed 前台等待 n 秒后 pending。
    pub timeout: Option<i64>,
    /// 发起该调用的 tool call id（timed 迟到回答注入锚点）。
    pub call_id: String,
}

/// timed 前台等待的结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimedOutcome {
    /// 用户在超时前提交了答复批次。
    Answered(AskUserAnswer),
    /// 前台等待逾限，未收到答复批次（问题仍可答）。
    Pending,
}

/// AskUserService：持有 session 接线槽 + 问询通道。
pub struct AskUserService {
    session: Arc<OnceLock<Arc<SessionLog>>>,
    requests: Mutex<Option<mpsc::UnboundedSender<AskUserRequest>>>,
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl AskUserService {
    pub fn new() -> Self {
        Self {
            session: Arc::new(OnceLock::new()),
            requests: Mutex::new(None),
        }
    }

    /// 延迟接线：锚定会话（durable 审计事件写入）。
    pub fn wire_session(&self, session: Arc<SessionLog>) {
        let _ = self.session.set(session);
    }

    /// 注入问询通道（main 持有 receiver）。
    pub fn set_request_channel(&self, tx: mpsc::UnboundedSender<AskUserRequest>) {
        *lock(&self.requests) = Some(tx);
    }

    /// 发起问询：挂起直到宿主回填答复。
    /// - `timeout = None`：legacy 无限阻塞；
    /// - `timeout = Some(-1)`：timed 无限阻塞（`timeout: -1`）；
    /// - `timeout = Some(n>0)`：timed 前台等待 n 秒，逾限返回 `TimedOutcome::Pending`。
    pub async fn ask(
        &self,
        questions: Vec<AskUserQuestion>,
        timeout: Option<i64>,
        call_id: String,
    ) -> Result<TimedOutcome, String> {
        let tx = lock(&self.requests)
            .clone()
            .ok_or_else(|| "ask_user_question channel not wired".to_string())?;
        let (reply, receiver) = oneshot::channel();
        let request = AskUserRequest { questions, reply, timeout, call_id };
        tx.send(request)
            .map_err(|_| "ask_user_question channel closed".to_string())?;
        let secs = match timeout {
            // legacy（None）与 timed 无限（Some(-1)）都无限阻塞。
            None | Some(-1) => {
                return receiver
                    .await
                    .map(TimedOutcome::Answered)
                    .map_err(|_| "ask_user_question cancelled".to_string());
            }
            Some(n) if n > 0 => n as u64,
            // 防御：非法 timeout 视同无限阻塞。
            Some(_) => {
                return receiver
                    .await
                    .map(TimedOutcome::Answered)
                    .map_err(|_| "ask_user_question cancelled".to_string());
            }
        };
        // timed：前台等待 n 秒；逾限返回 Pending（receiver 随后 drop，宿主侧
        // 迟到答复的 oneshot send 将失败，宿主据此转为注入而非回填）。
        match tokio::time::timeout(std::time::Duration::from_secs(secs), receiver).await {
            Ok(Ok(answer)) => Ok(TimedOutcome::Answered(answer)),
            Ok(Err(_)) => Err("ask_user_question cancelled".to_string()),
            Err(_elapsed) => Ok(TimedOutcome::Pending),
        }
    }
}

impl Default for AskUserService {
    fn default() -> Self {
        Self::new()
    }
}

/// 构造 timed 迟到的答复消息。
///
/// 前台等待逾限后工具侧 receiver 已 drop，答复不能再回填工具结果，改以普通
/// user 消息注入 agent inbox（`MessageSource::AnswerToPending` 标记来源，
/// 正文携带 `callId` + 原始问题 + 答复批次）。
pub fn answer_to_pending_message(
    call_id: &str,
    questions: &[AskUserQuestion],
    answer: &AskUserAnswer,
) -> AgentMessage {
    let text = serde_json::json!({
        "kind": "answer_to_pending_question",
        "tool": "ask_user_question",
        "callId": call_id,
        "questions": questions,
        "answers": answer.answers,
    })
    .to_string();
    AgentMessage {
        role: Role::User,
        blocks: vec![ContentBlock::Text { text }],
        source: Some(MessageSource::AnswerToPending { call_id: call_id.to_string() }),
    }
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
