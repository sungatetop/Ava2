//! 斜杠命令执行面（命令执行 + 结果落盘回执）。
//!
//! 命令由宿主执行并落 durable 事件，前端只消费事件流——刷新 / 重开会话后
//! 命令行与结果仍在时间线。命令不进入模型上下文（log-only）。

use axum::extract::{Path, State};
use axum::Json;

use crate::rest::dto::{CommandResultInfo, RunCommandRequest};
use crate::rest::error::ApiError;
use crate::rest::require_session;
use crate::router::RouterState;
use crate::session::ServerSession;

use onion::goal::types::{GoalBlockReason, GoalError, GoalRef};
use onion::session::SessionEvent;

/// 命令执行结果（`kind` = `success` / `error`）。
struct Outcome {
    kind: &'static str,
    text: String,
    /// 权威领域事件下标（命令自带精确指针时优先于「首条产物事件」推断）。
    source_seq: Option<u64>,
}

fn ok(text: impl Into<String>) -> Outcome {
    Outcome { kind: "success", text: text.into(), source_seq: None }
}

fn fail(text: impl Into<String>) -> Outcome {
    Outcome { kind: "error", text: text.into(), source_seq: None }
}

/// 命令名合法字符集（`/^[a-z][a-z0-9_-]*$/`）。
fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

/// 可切换的审批预设（`for_preset` 精确分支；`custom` 组合参数未开放）。
///
/// `/permission` 命令与 `PUT /v1/sessions/{id}/approval` 共用这一张表，
/// 避免两处字面量分叉。
pub(crate) const APPROVAL_PRESETS: [&str; 3] =
    ["workspace-write", "auto", "danger-full-access"];

/// 已注册的内置命令目录。
const COMMANDS: [&str; 5] = ["compact", "goal", "plan", "feedback", "permission"];

/// 内置命令的中文别名。
///
/// 解析顺序：先按注册名精确匹配，再查别名表映射到一等命令；
/// onion 无插件注册面，别名表即全部内置命令的中文拼写。
const COMMAND_ALIASES: [(&str, &str); 5] = [
    ("目标", "goal"),
    ("计划", "plan"),
    ("反馈", "feedback"),
    ("压缩", "compact"),
    ("权限", "permission"),
];

/// 输入拼写 → 规范命令名（精确名优先，其次中文别名）。
fn canonical_name(token: &str) -> Option<&'static str> {
    if let Some(name) = COMMANDS.iter().find(|name| **name == token) {
        return Some(name);
    }
    COMMAND_ALIASES
        .iter()
        .find(|(alias, _)| *alias == token)
        .map(|(_, name)| *name)
}

/// `POST /v1/sessions/{id}/commands`：执行一条斜杠命令。
pub(crate) async fn run(
    State(state): State<RouterState>,
    Path(id): Path<String>,
    Json(body): Json<RunCommandRequest>,
) -> Result<Json<CommandResultInfo>, ApiError> {
    let session = require_session(&state, &id).await?;
    let token = body.name.trim().trim_start_matches('/').trim().to_lowercase();
    let args = body.args.trim().to_string();
    // 拼写解析：精确注册名优先，其次中文别名。
    // 别名命中即用规范名执行与落盘，时间线里只出现规范名。
    let name = match canonical_name(&token) {
        Some(name) => name.to_string(),
        None => {
            // 非 ASCII 拼写（如未注册的中文别名 `/导出`）按未知命令处理：
            // 它不是「格式非法的命令名」，而是「没有这个命令」。
            let ascii = token.is_ascii();
            // 命令名格式校验：非法名直接拒绝，不落 `command/run`。
            if ascii && !valid_name(&token) {
                return Err(ApiError::bad_request(
                    "invalid_command_name",
                    format!("非法命令名 /{token}：须匹配 ^[a-z][a-z0-9_-]*$"),
                ));
            }
            // 未注册命令：只回执、不落盘（避免时间线被拼写错误污染）。
            return Ok(Json(CommandResultInfo {
                command_id: String::new(),
                kind: "error".to_string(),
                text: format!("未知命令 /{token}"),
                source_event_seq: None,
            }));
        }
    };
    let command_id = format!("cmd-{}-{}", onion::session::epoch_millis(), name);
    let log = session.handle.runtime.session();

    log.append(SessionEvent::CommandRun {
        command_id: command_id.clone(),
        name: name.clone(),
        args: if args.is_empty() { None } else { Some(args.clone()) },
    });

    // 领域事件区间起点：命令执行期间新落的 durable 事件即本命令的产物。
    let before = log.len();
    let outcome = execute(&session, &name, &args).await;
    // `sourceEventSeq` = 命令的权威领域事件下标：命令自带精确指针
    // （如 `/compact` 的 `result.summarySeq`）时优先采用，否则退化为「首条产物
    // 事件下标」；无产物时为 None。
    let source_event_seq = outcome
        .source_seq
        .or_else(|| (log.len() > before).then_some(before as u64));

    log.append(SessionEvent::CommandDone {
        command_id: command_id.clone(),
        kind: outcome.kind.to_string(),
        text: Some(outcome.text.clone()),
        source_event_seq,
    });

    Ok(Json(CommandResultInfo {
        command_id,
        kind: outcome.kind.to_string(),
        text: outcome.text,
        source_event_seq,
    }))
}

/// 命令分发（调用前已校验 [`known`]）。
async fn execute(session: &ServerSession, name: &str, args: &str) -> Outcome {
    match name {
        "compact" => compact(session).await,
        "goal" => goal(session, args),
        "plan" => plan(session, args).await,
        "feedback" => feedback(session, args),
        "permission" => permission(session, args),
        other => fail(format!("未知命令 /{other}")),
    }
}

/// 归一化反馈正文（去首尾空白；空文本视为缺失）。
fn feedback_text(args: &str) -> Option<String> {
    let trimmed = args.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// `/feedback <text>`：把用户反馈记为一条 log-only durable 事件，不触发模型工作。
fn feedback(session: &ServerSession, args: &str) -> Outcome {
    let Some(text) = feedback_text(args) else {
        return fail("需要反馈内容。用法：/feedback <文本>");
    };
    session.handle.runtime.session().append(SessionEvent::FeedbackRecord {
        text: Some(text),
        category: None,
    });
    ok(format!("已记录反馈（会话 {}）", session.id))
}

/// `/permission [<preset>]`：查看 / 切换审批预设。
///
/// 写路径与 `PUT /v1/sessions/{id}/approval` 同源（`build_approval_config`）：
/// 只改 ApprovalGate 共享配置，下一工具调用生效；`auto` 预设升级 LLM 复审裁决者。
fn permission(session: &ServerSession, args: &str) -> Outcome {
    let current = session
        .handle
        .approval_config
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .preset
        .name
        .clone();
    let name = args.trim();
    if name.is_empty() {
        return ok(format!(
            "当前预设 {current}（可选：{}）",
            APPROVAL_PRESETS.join(", ")
        ));
    }
    if !APPROVAL_PRESETS.contains(&name) {
        return fail(format!(
            "未知预设「{name}」（可选：{}）",
            APPROVAL_PRESETS.join(", ")
        ));
    }
    let model = session
        .handle
        .model_target
        .current()
        .map(|target| target.model)
        .unwrap_or_default();
    let config = crate::session::build_approval_config(name, &session.handle.llm, &model);
    *session
        .handle
        .approval_config
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = config;
    // durable 记录：冷恢复/子会话可据此重建有效预设。
    session.handle.runtime.session().append(SessionEvent::ApprovalPreset {
        preset: name.to_string(),
        source: None,
    });
    ok(format!("预设 {name}"))
}

/// `/compact`：空闲直驱压缩事务并回报计数。
async fn compact(session: &ServerSession) -> Outcome {
    if session.handle.gate.is_busy() {
        // 在飞轮次：投递信号，由 pre-step 边界执行（busy 语义）。
        return match session.handle.compact_tx.send(()) {
            Ok(()) => ok("已请求压缩上下文（当前轮次结束后执行）"),
            Err(_) => fail("压缩通道已关闭"),
        };
    }
    let log = session.handle.runtime.session();
    match session.handle.compaction_listener.compact_now_result(&log).await {
        // 成功回执指向 `CompactionSummary` 事件（`sourceEventSeq` =
        // `result.summarySeq`），客户端据此渲染压缩卡片而非回执文本。
        Ok(result) => Outcome {
            source_seq: Some(result.summary_seq),
            ..ok(format!(
                "已压缩 {} 条历史记录（约 {} tokens）",
                result.shadowed_count, result.shadowed_tokens
            ))
        },
        // 无可压缩区域
        Err(onion::compaction::types::CompactionError::Nothing) => ok("暂无可压缩的历史记录"),
        Err(error) => fail(format!("压缩未执行：{}", error.label())),
    }
}

/// `/goal [<objective>|clear|edit <objective>|pause|resume]`。
fn goal(session: &ServerSession, args: &str) -> Outcome {
    let service = &session.handle.goal_service;
    let current = service.view();
    let input = args.trim();
    let lower = input.to_lowercase();

    let reference = |view: &onion::goal::types::GoalView| GoalRef {
        id: view.snapshot.id.clone(),
        revision: view.snapshot.revision,
    };

    let render = |title: &str, view: &onion::goal::types::GoalView| {
        let mut lines = vec![
            title.to_string(),
            format!("状态：{}", view.snapshot.phase.label()),
            format!("目标：{}", view.snapshot.objective),
            format!("轮次：{}/{}", view.rounds_started, view.snapshot.max_goal_rounds),
        ];
        if let Some(reason) = &view.snapshot.blocked_reason {
            lines.insert(3, format!("受阻：{}: {}", reason.code, reason.message));
        }
        ok(lines.join("\n"))
    };

    if input.is_empty() {
        return match current {
            Some(view) => render("目标", &view),
            None => ok("未设置目标（用法：/goal <目标>）"),
        };
    }

    let result: Result<Option<onion::goal::types::GoalSnapshot>, GoalError> = match lower.as_str() {
        "clear" => match current {
            Some(view) => service.clear(reference(&view)).map(|()| None),
            None => Ok(None),
        },
        "pause" | "resume" | "complete" | "block" => match current {
            Some(view) => {
                let reference = reference(&view);
                match lower.as_str() {
                    "pause" => service.pause(reference).map(Some),
                    "resume" => service.resume(reference).map(Some),
                    "complete" => service.complete(reference).map(Some),
                    _ => service
                        .block(
                            reference,
                            GoalBlockReason::new("operator", "blocked by operator"),
                        )
                        .map(Some),
                }
            }
            None => return fail("未设置目标，无法执行该操作"),
        },
        _ => {
            // `edit <objective>` 显式编辑；裸文本在无目标时创建、有目标时编辑
            //（按 CAS 直接编辑，比「有 active 目标必须显式 edit」更宽松）。
            let objective = input
                .strip_prefix("edit ")
                .or_else(|| input.strip_prefix("edit\t"))
                .map(str::trim)
                .unwrap_or(input);
            if objective.is_empty() {
                return fail("用法：/goal edit <目标>");
            }
            match current {
                Some(view) => service
                    .edit(reference(&view), Some(objective.to_string()), None)
                    .map(Some),
                None => service.create(objective.to_string(), 5).map(Some),
            }
        }
    };

    match result {
        Ok(Some(snapshot)) => {
            let title = match lower.as_str() {
                "pause" => "目标已暂停",
                "resume" => "目标已恢复",
                "complete" => "目标已完成",
                "block" => "目标已标记受阻",
                _ => "目标已更新",
            };
            match service.view() {
                Some(view) => render(title, &view),
                None => ok(format!("{title}（rev {}）", snapshot.revision)),
            }
        }
        Ok(None) => ok("目标已清除"),
        Err(error) => fail(format!("目标操作失败：{error}")),
    }
}

/// `/plan [off|<message>]`：进入 / 退出 plan 模式；附带消息作为引导消息投递。
async fn plan(session: &ServerSession, args: &str) -> Outcome {
    let message = args.trim();
    let busy = session.handle.gate.is_busy();
    let plan_mode = &session.handle.plan_mode;

    if message.eq_ignore_ascii_case("off") {
        if busy {
            plan_mode.set(false);
            return ok("正在退出 plan 模式（当前轮次结束后生效）");
        }
        plan_mode.exit();
        return ok("plan 模式已关闭");
    }

    if busy {
        plan_mode.set(true);
        return ok("正在进入 plan 模式（当前轮次结束后生效）");
    }
    plan_mode.enter();
    if message.is_empty() {
        return ok("plan 模式已开启");
    }
    // 引导消息按普通用户消息投递（进入会话历史，模型可见）。
    let user = onion::types::AgentMessage::user(message.to_string());
    match session.handle.runtime.inbox().followup(user) {
        Ok(()) => {
            crate::session::spawn_turn(&session.handle);
            ok("plan 模式已开启")
        }
        Err(error) => fail(format!("引导消息投递失败：{error}")),
    }
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
