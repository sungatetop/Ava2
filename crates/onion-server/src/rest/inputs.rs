//! 人工输入通道：待回填列表 + 审批 / plan 裁决 / 问询三类回填。
//!
//! 请求由会话循环（`spawn_session_loops`）挂入 `SessionChannels` 待回填表，
//! `requestId` 随 SSE 帧下发；回填幂等（重复回填 409 `already_resolved`）。

use axum::extract::{Path, State};
use axum::Json;

use crate::rest::dto::{
    ApprovalReply, PendingItem, PendingListResponse, PlanReviewReply, QuestionInfo, QuestionReply,
    ReplyAck,
};
use crate::rest::error::ApiError;
use crate::rest::require_session;
use crate::router::RouterState;
use crate::session::{PendingInput, PendingKind, ReplySender, ResolveError, ServerSession};

/// `GET /v1/sessions/{id}/pending`：待审批 / 待 plan 裁决 / 待问询列表。
pub(crate) async fn pending(
    State(state): State<RouterState>,
    Path(id): Path<String>,
) -> Result<Json<PendingListResponse>, ApiError> {
    let session = require_session(&state, &id).await?;
    let items = session
        .channels
        .pending_views()
        .into_iter()
        .map(|(request_id, kind, created_at_ms)| pending_item(request_id, &kind, created_at_ms))
        .collect();
    Ok(Json(PendingListResponse { pending: items }))
}

fn pending_item(request_id: String, kind: &PendingKind, created_at_ms: u64) -> PendingItem {
    let mut item = PendingItem {
        request_id,
        kind: kind.label().to_string(),
        created_at_ms,
        tool: None,
        input: None,
        reason: None,
        plan: None,
        call_id: None,
        questions: None,
    };
    match kind {
        PendingKind::Approval { tool, input, reason } => {
            item.tool = Some(tool.clone());
            item.input = Some(input.clone());
            item.reason = reason.clone();
        }
        PendingKind::PlanReview { plan } => item.plan = Some(plan.clone()),
        PendingKind::Question { call_id, questions } => {
            item.call_id = Some(call_id.clone());
            item.questions = Some(
                questions
                    .iter()
                    .map(QuestionInfo::from_question)
                    .collect(),
            );
        }
    }
    item
}

/// 取走待回填输入（404 / 409 统一映射）。
fn resolve(
    session: &ServerSession,
    request_id: &str,
    decision: &str,
) -> Result<PendingInput, ApiError> {
    session
        .channels
        .resolve(request_id, decision)
        .map_err(|error| match error {
            ResolveError::NotFound => ApiError::not_found(
                "request_not_found",
                format!("request `{request_id}` not found"),
            ),
            ResolveError::AlreadyResolved => {
                let previous = session
                    .channels
                    .resolved_decision(request_id)
                    .unwrap_or_default();
                ApiError::conflict(
                    "already_resolved",
                    format!("request `{request_id}` already resolved as `{previous}`"),
                )
            }
        })
}

fn kind_mismatch(kind: &str, request_id: &str, expected: &str) -> ApiError {
    ApiError::bad_request(
        "request_kind_mismatch",
        format!("request `{request_id}` is {kind}, not {expected}"),
    )
}

/// `POST /v1/sessions/{id}/approvals/{requestId}`：`approve` / `approve_for_session` / `deny`。
pub(crate) async fn approval(
    State(state): State<RouterState>,
    Path((id, request_id)): Path<(String, String)>,
    Json(body): Json<ApprovalReply>,
) -> Result<Json<ReplyAck>, ApiError> {
    let session = require_session(&state, &id).await?;
    let decision = match body.decision.as_str() {
        "approve" => onion::approval::ApprovalDecision::AllowedOnce,
        "approve_for_session" => onion::approval::ApprovalDecision::AllowedForSession,
        "deny" => onion::approval::ApprovalDecision::Rejected,
        other => {
            return Err(ApiError::bad_request(
                "invalid_decision",
                format!("unknown decision `{other}` (approve|approve_for_session|deny)"),
            ));
        }
    };
    let input = resolve(&session, &request_id, &body.decision)?;
    let kind = input.kind.label();
    match input.reply {
        ReplySender::Approval(sender) => {
            if sender.send(decision).is_err() {
                // 请求已被撤回（内核等待端已退出）：幂等窗口改记 `cancelled`，
                // 避免后续重复回填的 409 报文声称「已按 approve 处理」。
                session.channels.override_decision(&request_id, "cancelled");
                return Err(ApiError::conflict(
                    "request_cancelled",
                    format!("request `{request_id}` requester cancelled"),
                ));
            }
        }
        _ => return Err(kind_mismatch(kind, &request_id, "approval")),
    }
    Ok(Json(ReplyAck { request_id, decision: body.decision }))
}

/// `POST /v1/sessions/{id}/plan-reviews/{requestId}`：`approve` / `keep_planning` / `dismissed`。
pub(crate) async fn plan_review(
    State(state): State<RouterState>,
    Path((id, request_id)): Path<(String, String)>,
    Json(body): Json<PlanReviewReply>,
) -> Result<Json<ReplyAck>, ApiError> {
    let session = require_session(&state, &id).await?;
    let verdict = match body.verdict.as_str() {
        "approve" => onion::plan::service::PlanVerdict::Approve,
        "keep_planning" => onion::plan::service::PlanVerdict::KeepPlanning,
        "dismissed" => onion::plan::service::PlanVerdict::Dismissed,
        other => {
            return Err(ApiError::bad_request(
                "invalid_verdict",
                format!("unknown verdict `{other}` (approve|keep_planning|dismissed)"),
            ));
        }
    };
    let input = resolve(&session, &request_id, &body.verdict)?;
    let kind = input.kind.label();
    match input.reply {
        ReplySender::PlanReview(sender) => {
            if sender.send(verdict).is_err() {
                session.channels.override_decision(&request_id, "cancelled");
                return Err(ApiError::conflict(
                    "request_cancelled",
                    format!("request `{request_id}` requester cancelled"),
                ));
            }
        }
        _ => return Err(kind_mismatch(kind, &request_id, "plan_review")),
    }
    Ok(Json(ReplyAck { request_id, decision: body.verdict }))
}

/// `POST /v1/sessions/{id}/questions/{requestId}`：ask_user 答复。
pub(crate) async fn question(
    State(state): State<RouterState>,
    Path((id, request_id)): Path<(String, String)>,
    Json(body): Json<QuestionReply>,
) -> Result<Json<ReplyAck>, ApiError> {
    let session = require_session(&state, &id).await?;
    let answer = onion::ask_user::types::AskUserAnswer {
        answers: body
            .answers
            .into_iter()
            .map(|item| onion::ask_user::types::AskUserAnswerItem {
                id: item.id,
                selected: item.selected,
                custom: item.custom,
            })
            .collect(),
    };
    let input = resolve(&session, &request_id, "answered")?;
    let kind = input.kind.label();
    match input.reply {
        ReplySender::Question(sender) => {
            if sender.send(answer).is_err() {
                session.channels.override_decision(&request_id, "cancelled");
                return Err(ApiError::conflict(
                    "request_cancelled",
                    format!("request `{request_id}` requester cancelled"),
                ));
            }
        }
        _ => return Err(kind_mismatch(kind, &request_id, "question")),
    }
    Ok(Json(ReplyAck { request_id, decision: "answered".to_string() }))
}
