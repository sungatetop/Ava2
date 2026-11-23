//! onion demo 入口：装配 Runtime + 6 个 listener + 终端交互。
//!
//! 会话装配经 `onion::assemble` 共享单元（onion-cli 与 onion-tui 同源，不漂移）。
//!
//! 交互命令：
//! - 普通文本：闲时直达开轮，忙时 followup 排队（一条排队消息 = 一轮）
//! - `/steer <msg>`  运行中引导（next-step 分区 + 唤醒）
//! - `/inject <msg>`  不唤醒注入（next-step 分区）
//! - `/followup <msg>`  显式排队成轮
//! - `/compact`  请求上下文压缩：忙时下一 step 准入前执行，空闲时直接执行压缩事务
//! - `/agent [id]`  切换 Agent（coding / work / 自定义预设）：执行侧立即生效，装配侧下一轮生效
//! - `/model [id]`  模型目标切换：带参校验目录切换（写回 settings + 记 model_selection）；无参列出模型目录
//! - `/approve` / `/deny`  审批复决（沙箱允许集外的升级操作执行前挂起等待）
//! - `/answer <id> <选项编号/标签或文本>`  ask_user_question 问询答复；单问题可省略 id 直接 `/answer <编号/文本>`
//! - `/exit`  优雅退出（SessionEnd）

use onion::assemble::{build_approval_config, assemble_runtime, FrontDoor};
use onion::ask_user::types::{AskUserAnswer, AskUserAnswerItem, AskUserQuestion};
use onion::config::Config;
use onion::goal::driver::GoalRoundDriver;
use onion::goal::service::GoalService;
use onion::inbox::Inbox;
use onion::kernel::TurnReport;
use onion::listeners::ApprovalRequest;
use onion::plan::service::PlanMode;
use onion::runtime::Runtime;
use onion::types::{AgentMessage, ContentBlock, MessageSource, Role};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

/// 会话切换目标（内层循环 break 后由外层消费）。
enum Switch {
    New,
    Resume(String),
}

/// 在飞 ask_user 问询的宿主侧状态：一次性答复句柄 + 待答问题 + 已答项 +
/// timed 元数据（call_id / timeout）+ 注入锚点（迟到答复转为 user 消息注入）。
struct PendingAskUser {
    reply: oneshot::Sender<AskUserAnswer>,
    questions: Vec<AskUserQuestion>,
    answered: Vec<AskUserAnswerItem>,
    /// 发起该调用的 tool call id（timed 迟到回答按 AnswerToPending 注入）。
    call_id: String,
    /// 前台等待秒数（`Some(-1)` = timed 无限；`Some(n)` = timed n 秒）。
    timeout: Option<i64>,
    /// 迟到答复注入锚点（timed 超时后，oneshot 已 drop，答复改为 user 消息注入）。
    inbox: Arc<Inbox>,
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let config = Config::from_env();
    println!("onion demo — model={} base={}", config.model, config.base_url);
    println!("命令：");
    println!("  /exit 退出");
    println!("  /new 新会话");
    println!("  /resume [id] 恢复会话");
    println!("  /steer <msg> 中途引导");
    println!("  /inject <msg> 不唤醒注入");
    println!("  /followup <msg> 排队成轮");
    println!("  /compact 请求压缩");
    println!("  /agent [id] 切换 Agent");
    println!("  /approve|/deny 审批复决");
    println!("  /goal [objective|pause|resume|complete|clear] 目标管理");
    println!("  /plan [off|<msg>] 计划模式");
    let root = std::env::current_dir()?;
    // 会话目录经 paths 模块解析（ONION_HOME → ~/.onion，按项目 cwd 隔离），
    // 不落工作空间——布局为 `<root>/sessions/<normalized-cwd>/<id>/session.vN.jsonl`。
    let log_dir = onion::paths::sessions_dir(&onion::paths::data_home(), &root);

    // stdin 转发进 async 通道（进程级：会话切换不重建读取线程）
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        loop {
            let mut line = String::new();
            match stdin.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    // Windows 管道/重定向输入可能携带 UTF-8 BOM（U+FEFF），
                    // 剥离后再识别斜杠命令，避免 "/mode" 被误判为普通消息。
                    let line = line.trim_start_matches('\u{FEFF}').trim().to_string();
                    if !line.is_empty() && tx.send(line).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    // 外层会话循环：/new 或 /resume 切换会话 = 整体重建装配
    //（先收尾当前会话，再经完整启动路径恢复目标会话）。
    let mut next_session: Option<String> = None; // None = 新会话
    'sessions: loop {
        let mut a = assemble_runtime(&root, &config, next_session.as_deref(), FrontDoor::Cli)?;
        for note in &a.notes {
            println!("{note}");
        }
        println!(
            "会话：{}（/resume 列出历史，/resume <id> 恢复）",
            a.runtime.session().session_id()
        );
        // 在飞审批的一次性答复句柄：同一时刻至多一个（内核串行保证）；
        // 会话退出前 drop → 等待端 recv Err → fail-closed 拒绝。
        let mut pending_approval: Option<oneshot::Sender<onion::approval::ApprovalDecision>> = None;
        // 在飞 plan-review 的一次性答复句柄（与工具审批共用 /approve /deny 通道）。
        let mut pending_plan_review: Option<oneshot::Sender<onion::plan::service::PlanVerdict>> = None;
        // 在飞 ask_user 问询状态（宿主 /answer 回填；timed 迟到答复转注入 inbox）。
        let mut pending_ask_user: Option<PendingAskUser> = None;
        // 会话切换目标：内层循环的两个 break 点（/new、/resume <id>）必然赋值。
        let switch: Switch;

        // 常驻 kick 的唤醒通知源：绑定一次避免 select 内临时值生命周期问题
        //（notified() 借用 inbox，需跨 select 分支存活）。
        let inbox = a.runtime.inbox();

        loop {
        tokio::select! {
            line = rx.recv() => {
                let Some(line) = line else {
                    // stdin EOF：等待在飞轮次与排队消息消化完再优雅退出，
                    // 避免 runtime drop 杀掉在飞 turn（压缩窗口不丢失）；
                    // 挂起审批按拒绝处理（fail-closed），解除审批等待。
                    drop(pending_approval.take());
                    drop(pending_plan_review.take());
                    drop(pending_ask_user.take());
                    while a.busy.load(Ordering::SeqCst) {
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                    if let Some(cancel) = &a.hook_cancel {
                        cancel.cancel();
                    }
                    let _ = a.runtime.end("stdin eof").await;
                    break 'sessions;
                };
                if line == "/exit" || line == "exit" {
                    drop(pending_approval.take());
                    drop(pending_plan_review.take());
                    drop(pending_ask_user.take());
                    if let Some(cancel) = &a.hook_cancel {
                        cancel.cancel();
                    }
                    let _ = a.runtime.end("user exit").await;
                    break 'sessions;
                }
                if line == "/new" {
                    if a.busy.load(Ordering::SeqCst) {
                        println!("（轮进行中，暂不能切换会话；/exit 可退出）");
                        continue;
                    }
                    if a.subagent_runtime.inflight() > 0 {
                        println!("（有 subagent 在飞，暂不能切换会话）");
                        continue;
                    }
                    // 挂起审批随会话丢弃 → 等待端 recv Err → fail-closed。
                    drop(pending_approval.take());
                    drop(pending_plan_review.take());
                    drop(pending_ask_user.take());
                    let _ = a.runtime.end("switch new").await;
                    switch = Switch::New;
                    break;
                }
                if line == "/resume" || line.starts_with("/resume ") {
                    if let Some(id) = line.strip_prefix("/resume ").map(str::trim).filter(|s| !s.is_empty()) {
                        // 预检：非当前会话 + 日志文件存在（不静默新建替代会话）
                        // + 无在飞轮次/子代理。
                        if id == a.runtime.session().session_id() {
                            println!("（{id} 已是当前会话）");
                            continue;
                        }
                        if !onion::paths::session_log_file(&onion::paths::session_dir(
                            &log_dir,
                            id,
                        ))
                        .exists()
                        {
                            println!("（会话不存在：{id}）");
                            continue;
                        }
                        if a.busy.load(Ordering::SeqCst) || a.subagent_runtime.inflight() > 0 {
                            println!("（轮进行中或有 subagent 在飞，暂不能切换会话）");
                            continue;
                        }
                        drop(pending_approval.take());
                        drop(pending_plan_review.take());
                        drop(pending_ask_user.take());
                        let _ = a.runtime.end("switch resume").await;
                        switch = Switch::Resume(id.to_string());
                        break;
                    }
                    // 无参 /resume：列出历史会话（最近活动倒序）。
                    let summaries = onion::session::list_sessions(&log_dir);
                    if summaries.is_empty() {
                        println!("（暂无历史会话）");
                        continue;
                    }
                    println!("历史会话（最近优先）· /resume <id> 恢复：");
                    for s in summaries {
                        println!("  {} · {} 事件 · goal={} · {}", s.id, s.events, s.goal_phase, s.title);
                    }
                    continue;
                }
                if let Some(rest) = line.strip_prefix("/agent ") {
                    let id = rest.trim();
                    let Some(agent) = a.agent_catalog.get(id).cloned() else {
                        println!("（未知 Agent {id}，可选：{}）", a.agent_catalog.ids().join(" | "));
                        continue;
                    };
                    a.tools_handle.state().set_agent(agent.tools);
                    a.active_agent = id.to_string();
                    if let Some(bridge) = &a.hooks_bridge {
                        bridge.emit_config_change("agent", id).await;
                    }
                    println!("（已切换 Agent → {id}：执行侧立即生效，装配侧下一轮生效）");
                    continue;
                }
                if line == "/agent" {
                    println!(
                        "（当前 Agent：{}；可选：{}）",
                        a.active_agent,
                        a.agent_catalog.ids().join(" | ")
                    );
                    continue;
                }
                if line == "/permission" {
                    let current = a.approval_config.read().unwrap_or_else(std::sync::PoisonError::into_inner);
                    let name = current.preset.name.clone();
                    drop(current);
                    println!(
                        "（当前审批预设：{}；可选：{}）",
                        name,
                        onion::approval::ApprovalConfig::PRESET_NAMES.join(" | ")
                    );
                    continue;
                }
                if let Some(rest) = line.strip_prefix("/permission ") {
                    let preset_name = rest.trim();
                    if !onion::approval::ApprovalConfig::PRESET_NAMES.contains(&preset_name) {
                        println!(
                            "（未知预设 {preset_name}，可选：{}）",
                            onion::approval::ApprovalConfig::PRESET_NAMES.join(" | ")
                        );
                        continue;
                    }
                    let mut guard = a.approval_config.write().unwrap_or_else(std::sync::PoisonError::into_inner);
                    *guard = build_approval_config(preset_name, &a.reviewer_llm, &a.reviewer_model);
                    drop(guard);
                    println!("（已切换审批预设 → {preset_name}：沙箱允许集与审批策略即时生效）");
                    continue;
                }
                if line == "/model" || line.starts_with("/model ") {
                    // 模型目标切换：带参目录校验后置换
                    // current + 写回 settings + append ModelSelection 事件；无参列出目录。
                    let model = line.strip_prefix("/model ").map(str::trim).filter(|s| !s.is_empty());
                    let Some(model) = model else {
                        if a.provider_catalog.providers.is_empty() {
                            println!("（模型目录为空：配置 ONION_PROVIDER_CATALOG 或 <root>/providers.json）");
                        } else {
                            println!("模型目录（/model <id> 切换）：");
                            for p in &a.provider_catalog.providers {
                                println!("  {}（{}）", p.name, p.base_url);
                                for m in &p.models {
                                    let effort = m
                                        .reasoning
                                        .as_ref()
                                        .map(|r| r.default_effort.clone().unwrap_or_else(|| "…".into()))
                                        .unwrap_or_else(|| "-".into());
                                    println!("    - {}\tname={}\teffort={}", m.id, m.name, effort);
                                }
                            }
                        }
                        continue;
                    };
                    let Some(target) = a.provider_catalog.target_for(model) else {
                        println!("（模型目录中无 {model}；/model 列出可用模型后重试）");
                        continue;
                    };
                    let label = format!("{} @ {}", target.model, target.base_url);
                    a.model_target.swap_current(target.clone());
                    // 写回 settings.json（失败仅 warn，不影响本会话）。
                    let persisted = onion::settings::PersistedModelTarget {
                        model: target.model.clone(),
                        base_url: target.base_url.clone(),
                        effort: target.effort.clone(),
                    };
                    if let Err(error) = onion::settings::SettingsFile::save_model_target(
                        &onion::paths::data_home(),
                        &persisted,
                    ) {
                        println!("（模型写回 settings 失败，仅本次会话生效：{error}）");
                    }
                    // append durable ModelSelection 事件（log-only）。
                    a.runtime.session().append(onion::session::SessionEvent::ModelSelection {
                        model: target.model.clone(),
                        base_url: target.base_url.clone(),
                        effort: target.effort.clone(),
                    });
                    println!("（已切换模型目标 → {label}，下一个请求生效）");
                    continue;
                }
                if line == "/approve" || line == "/deny" {
                    // plan-review 优先（工具审批与 plan-review 共用命令通道）。
                    if let Some(reply) = pending_plan_review.take() {
                        let verdict = if line == "/approve" {
                            onion::plan::service::PlanVerdict::Approve
                        } else {
                            onion::plan::service::PlanVerdict::KeepPlanning
                        };
                        let _ = reply.send(verdict);
                        continue;
                    }
                    match pending_approval.take() {
                        Some(reply) => {
                            let decision = if line == "/approve" {
                                onion::approval::ApprovalDecision::AllowedOnce
                            } else {
                                onion::approval::ApprovalDecision::Rejected
                            };
                            let _ = reply.send(decision);
                        }
                        None => println!("（当前无待审批请求）"),
                    }
                    continue;
                }
                if let Some(rest) = line.strip_prefix("/answer ") {
                    // ask_user 问询答复：/answer <id> <选项标签或自由文本>。
                    // 多问题一次只答一个，其余保持待答；全部答完合并回填宿主。
                    // timed 场景：前台已超时（oneshot 已 drop，send 失败）时，答复转为
                    // AnswerToPending user 消息注入 inbox。
                    let Some(pending) = pending_ask_user.take() else {
                        println!("（当前无待答问询）");
                        continue;
                    };
                    let PendingAskUser { reply, mut questions, mut answered, call_id, timeout, inbox } = pending;
                    let rest = rest.trim();
                    // 单问题快捷答复：/answer <编号> 或 /answer <标签/文本>（省略问题 id）。
                    // 仅当 rest 不是任何待答问题的 id 时，将其整段作为唯一问题的值。
                    let single_question = questions.len() == 1;
                    let single_id = single_question
                        .then(|| questions[0].id.clone())
                        .filter(|id| id != rest);
                    let (qid, value) = match rest.split_once(char::is_whitespace) {
                        Some((id, value)) => (id.trim().to_string(), value.trim().to_string()),
                        None => match single_id {
                            Some(id) => (id, rest.to_string()),
                            None => (rest.to_string(), String::new()),
                        },
                    };
                    let Some(question) = questions.iter().find(|q| q.id == qid) else {
                        println!("（无此问题 id：{qid}）");
                        pending_ask_user = Some(PendingAskUser { reply, questions, answered, call_id, timeout, inbox });
                        continue;
                    };
                    if value.is_empty() {
                        println!("（用法：/answer <id> <选项编号/标签或自由文本>，或单问题直接 /answer <编号/文本>）");
                        pending_ask_user = Some(PendingAskUser { reply, questions, answered, call_id, timeout, inbox });
                        continue;
                    }
                    let selected = if let Some(options) = question.options.as_ref().filter(|o| !o.is_empty()) {
                        // 有选项：答复可用选项编号（1-based）或完整标签；多选逗号分隔。
                        let labels: Vec<&str> = value.split(',').map(str::trim).collect();
                        let unknown: Vec<&str> = labels
                            .iter()
                            .filter(|l| {
                                // 编号：解析为 1..=len 视为合法；否则须匹配某个标签。
                                match l.parse::<usize>() {
                                    Ok(n) => n < 1 || n > options.len(),
                                    Err(_) => !options.iter().any(|o| o.label == **l),
                                }
                            })
                            .copied()
                            .collect();
                        if !unknown.is_empty() {
                            println!("（未知选项：{}）", unknown.join(", "));
                            pending_ask_user = Some(PendingAskUser { reply, questions, answered, call_id, timeout, inbox });
                            continue;
                        }
                        labels
                            .into_iter()
                            .map(|l| {
                                // 编号 → 对应标签（回填引擎的 selected 语义为标签）。
                                match l.parse::<usize>() {
                                    Ok(n) => options[n - 1].label.clone(),
                                    Err(_) => l.to_string(),
                                }
                            })
                            .collect()
                    } else {
                        // 无选项：整段作为自定义答复。
                        vec![value.to_string()]
                    };
                    answered.push(AskUserAnswerItem { id: qid.to_string(), selected, custom: None });
                    questions.retain(|q| q.id != qid);
                    if questions.is_empty() {
                        // 全部答完：优先经 oneshot 回填宿主（legacy/未超时 timed）。
                        // send 失败 = 前台已超时 drop → 转为 AnswerToPending 注入 inbox。
                        let answer = AskUserAnswer { answers: answered };
                        let serialized = serde_json::to_string(&answer.answers).unwrap_or_default();
                        if reply.send(answer).is_ok() {
                            println!("（问询已答复）");
                        } else {
                            let text = format!(
                                "answer_to_pending_question callId={call_id} answers={serialized}"
                            );
                            let msg = AgentMessage {
                                role: Role::User,
                                blocks: vec![ContentBlock::Text { text }],
                                source: Some(MessageSource::AnswerToPending { call_id }),
                            };
                            match inbox.steer(msg) {
                                Ok(()) => println!("（问询已超时，迟到答复已作为 answer_to_pending_question 注入）"),
                                Err(e) => println!("（问询已超时，答复注入失败：{e}）"),
                            }
                        }
                    } else {
                        // 逐一显示：答完当前问题，展示下一个待答问题。
                        let total = questions.len() + answered.len();
                        let next = answered.len() + 1;
                        println!("（已记录答复 {next}/{total}，继续下一个）");
                        print_ask_question(&questions[0], next, total);
                        pending_ask_user = Some(PendingAskUser { reply, questions, answered, call_id, timeout, inbox });
                    }
                    continue;
                }
                if let Some(rest) = line.strip_prefix("/steer ") {
                    // 统一经 inbox 投递 + 唤醒；空闲时由主循环 select 的 kick 分支开轮。
                    match a.runtime.inbox().steer(AgentMessage::user(rest)) {
                        Ok(()) => {
                            if a.busy.load(Ordering::SeqCst) {
                                println!("（已 steer，下一个 step 领取）");
                            } else {
                                println!("（空闲：已 steer，将由 kick 自动开轮）");
                            }
                        }
                        Err(error) => println!("（steer 失败：{error}）"),
                    }
                    continue;
                }
                if let Some(rest) = line.strip_prefix("/inject ") {
                    match a.runtime.inbox().inject(AgentMessage::user(rest)) {
                        Ok(()) => println!("（已 inject，不唤醒）"),
                        Err(error) => println!("（inject 失败：{error}）"),
                    }
                    continue;
                }
                if let Some(rest) = line.strip_prefix("/followup ") {
                    // 统一经 inbox 投递 + 唤醒；空闲时由 kick 分支开轮。
                    match a.runtime.inbox().followup(AgentMessage::user(rest)) {
                        Ok(()) => {
                            if a.busy.load(Ordering::SeqCst) {
                                println!("（已 followup 排队，一轮后独立成轮）");
                            } else {
                                println!("（空闲：已 followup 排队，将由 kick 自动开轮）");
                            }
                        }
                        Err(error) => println!("（followup 失败：{error}）"),
                    }
                    continue;
                }
                if line == "/compact" {
                    if a.busy.load(Ordering::SeqCst) {
                        match a.compact_tx.send(()) {
                            Ok(()) => println!("（已请求压缩：下一个 step 准入前执行）"),
                            Err(_) => println!("（compaction listener 未在运行）"),
                        }
                    } else {
                        // 空闲直驱：无 pre_step 窗口，main 直接执行压缩事务
                        //（串行 await，避免与开轮并发破坏 bracket 原子性）。
                        a.compaction_listener.compact_now(&a.runtime.session()).await;
                    }
                    continue;
                }
                if line == "/goal" || line.starts_with("/goal ") {
                    handle_goal_command(&a.goal_service, &a.goal_driver, &a.runtime, &line, &a.busy).await;
                    continue;
                }
                if line == "/plan" || line.starts_with("/plan ") {
                    if let Some(command) = onion::plan::commands::parse_plan_command(&line) {
                        // run_plan_command 只做 plan 模式切换 + 附带消息 push inbox；
                        // 空闲时由 kick 分支开轮。
                        if let Some(msg) = onion::plan::commands::run_plan_command(&a.plan_mode, &a.busy, command) {
                            if a.runtime.inbox().followup(AgentMessage::user(msg)).is_err() {
                                println!("（plan 附带消息入 inbox 失败）");
                            }
                        }
                    }
                    continue;
                }
                // 普通消息：统一经 inbox 投递 + 唤醒（followup），
                // 空闲时由主循环 select 的 kick 分支自动开轮。
                let _ = a.runtime.inbox().followup(AgentMessage::user(line));
                if a.busy.load(Ordering::SeqCst) {
                    println!("（busy，已 followup 排队）");
                } else {
                    println!("（已 followup 排队，kick 将自动开轮）");
                }
            }
            request = a.approval_rx.recv() => {
                match request {
                    Some(request) => {
                        let ApprovalRequest { name, input, reply, .. } = request;
                        let summary = input.to_string();
                        let summary: String = if summary.chars().count() > 80 {
                            format!("{}…", summary.chars().take(80).collect::<String>())
                        } else {
                            summary
                        };
                        println!("⏸ 待审批 {name}({summary}) — /approve 放行 /deny 拒绝");
                        pending_approval = Some(reply);
                    }
                    // 通道关闭（gate 已随 runtime 销毁）：防御性退出（正常运行不可达）。
                    None => break 'sessions,
                }
            }
            review = a.plan_review_rx.recv() => {
                match review {
                    Some(review) => {
                        let onion::plan::service::PlanReviewRequest { plan, reply } = review;
                        let summary: String = if plan.chars().count() > 80 {
                            format!("{}…", plan.chars().take(80).collect::<String>())
                        } else {
                            plan
                        };
                        println!("⏸ 待审批 exit_plan_mode — /approve 放行 /deny 继续规划");
                        println!("    {summary}");
                        pending_plan_review = Some(reply);
                    }
                    None => break 'sessions,
                }
            }
            ask = a.ask_user_rx.recv() => {
                match ask {
                    Some(ask) => {
                        let onion::ask_user::service::AskUserRequest { questions, reply, timeout, call_id } = ask;
                        let total = questions.len();
                        // timed 标记：显示前台等待超时（若有），提示迟到仍可答。
                        let timed_hint = match timeout {
                            Some(-1) => "（timed 无限阻塞）".to_string(),
                            Some(n) if n > 0 => format!("（timed 前台等待 {n}s，超时返回 pending，迟到仍可答）"),
                            _ => String::new(),
                        };
                        println!("⏸ 待答复 ask_user_question{timed_hint}（共 {total} 问，逐一作答；/answer <id> <选项编号或文本>）");
                        print_ask_question(&questions[0], 1, total);
                        pending_ask_user = Some(PendingAskUser {
                            reply,
                            questions,
                            answered: Vec::new(),
                            call_id,
                            timeout,
                            inbox: inbox.clone(),
                        });
                    }
                    None => break 'sessions,
                }
            }
            report = a.done_rx.recv() => {
                if let Some(report) = report {
                    println!(
                        "\n\x1b[36m[turn]\x1b[0m t{} · {} steps · {}",
                        report.turn, report.steps, report.reason_label()
                    );
                }
            }
            // 常驻 kick：inbox 被 followup/steer 唤醒，
            // 空闲（无在飞轮次）且有排队消息时开轮。busy 期间由 spawn_turn 内循环
            // 的续跑逻辑接管（活跃 driver 自己 claim 队列）。
            _ = inbox.notified() => {
                if !a.busy.load(Ordering::SeqCst) && inbox.has_pending() {
                    spawn_turn(a.runtime.clone(), a.done_tx.clone(),
                               a.busy.clone(), a.goal_driver.clone(), a.plan_mode.clone());
                }
            }
        }
        }

        // 内层循环 break = 会话切换：按目标装配下一会话（EOF/exit 已 break 'sessions）。
        next_session = match switch {
            Switch::New => None,
            Switch::Resume(id) => Some(id),
        };
    }
    Ok(())
}

/// 启动 goal 轮（create / edit 替换 / resume 共用）。goal 首轮消息 followup
/// 进 inbox（带 GoalRound source 锚点 + continuation prompt），由 kick 空轮
/// 消费；CAS 预留（record_round）在 followup 前建立，由 kernel pre-step
/// 边界校验。
fn start_goal_round(
    goal_driver: &Arc<GoalRoundDriver>,
    runtime: &Arc<Runtime>,
    busy: &Arc<AtomicBool>,
) {
    if busy.load(Ordering::SeqCst) {
        // 轮进行中设置/恢复目标：turn 循环边界的 should_continue 检查会接管
        // （此时目标已 armed），无需立即拉起。
        println!("（当前轮进行中，结束后自动续跑 goal 轮）");
        return;
    }
    goal_driver.service_enter();
    let Some(message) = goal_driver.continuation_input() else {
        goal_driver.service_exit();
        println!("（无法启动 goal 轮：轮预算耗尽或目标相位非 active）");
        return;
    };
    // 进入 inbox 前 CAS 预留 round；失败说明目标被并发变更，放弃本轮。
    if goal_driver.record_round().is_err() {
        goal_driver.service_exit();
        println!("（目标已被并发变更，放弃启动本轮）");
        return;
    }
    // followup 进 inbox + 唤醒；kick 分支会以空轮消费。
    if runtime.inbox().followup(message).is_err() {
        goal_driver.service_exit();
        println!("（goal 消息入 inbox 失败）");
        return;
    }
    println!("（goal 轮已入队，kick 将自动开轮）");
}

/// 常驻 kick 驱动（`kick()` → `while(turn())` 循环）：以空轮启动，
/// 由 kernel 内部 `claim_open()`/`drain_next_step()` 消费 inbox 排队消息。
/// 所有外部输入（普通消息、/goal、/plan、/steer、/followup）统一经 inbox
/// 投递，由本驱动（或主循环 select 的 kick 分支）开轮，无任何带 input 直驱。
fn spawn_turn(
    runtime: Arc<Runtime>,
    done: mpsc::UnboundedSender<TurnReport>,
    busy: Arc<AtomicBool>,
    goal_driver: Arc<GoalRoundDriver>,
    plan_mode: Arc<PlanMode>,
) {
    busy.store(true, Ordering::SeqCst);
    tokio::spawn(async move {
        loop {
            // Ok(None) = 无轮可开（空输入且无排队消息，正常空转）；
            // Err = 状态机拒绝开轮（如恢复会话投影异常），必须给用户可见反馈，
            // 否则 spawn 静默死亡、busy 悬挂为 true。
            let report = match runtime.run(Vec::new()).await {
                Ok(Some(report)) => report,
                Ok(None) => break,
                Err(error) => {
                    println!("（turn 启动失败：{error}）");
                    break;
                }
            };
            // D2：轮因 max-tokens 结束（模型未收敛）→ disarm goal，停止自动续轮，
            // 防止无限续轮（turn/end max-tokens → disarm）。
            // 仅当 goal 原本 armed（处于活动 goal 续轮语境）才提示，避免误导性打印。
            if matches!(report.reason, onion::kernel::TurnEndReason::MaxTokens)
                && goal_driver.disarm()
            {
                println!("（goal 因 max-tokens 结束已 disarm，停止自动续轮）");
            }
            // 每轮边界：消费 open-turn 内挂起的 plan 模式切换（pre-step 追加）。
            plan_mode.flush_pending();
            let turn = report.turn;
            if done.send(report).is_err() {
                break;
            }
            if runtime.inbox().has_next_turn() {
                println!("（next-turn 有排队消息，开新一轮 t{}）", turn + 1);
                continue;
            }
            // 空闲唤醒自续：Stopping 窗口后到达的 steer
            // 残留在 next-step，自续判据据此开新轮；inject 不置唤醒、不会触发。
            if runtime.inbox().has_next_step() && runtime.inbox().take_wakeup() {
                println!("（next-step 有引导消息，开新一轮 t{}）", turn + 1);
                continue;
            }
            // goal 自动续轮：目标 active + armed +
            // 无排队人类消息。续轮消息 followup 进 inbox（带 GoalRound source 锚点），
            // 由下一轮 kernel claim 消费；CAS 预留（record_round）在 followup 前建立，
            // 由 kernel pre-step 边界校验。
            if goal_driver.should_continue() {
                if let Some(message) = goal_driver.continuation_input() {
                    println!("（goal 自动续轮，开新一轮 t{}）", turn + 1);
                    goal_driver.service_enter();
                    // followup 前 CAS 预留 round；失败说明目标已被并发变更（revision/
                    // round 失配），放弃本轮，不向模型发送过期续轮。
                    if goal_driver.record_round().is_err() {
                        goal_driver.service_exit();
                        break;
                    }
                    if runtime.inbox().followup(message).is_err() {
                        goal_driver.service_exit();
                        break;
                    }
                    continue;
                }
            }
            break;
        }
        goal_driver.service_exit();
        // Phase B（kick 收尾兜底）：
        // 收敛（busy 复位）瞬间若 inbox 仍有排队消息（如竞态窗口内的 followup/steer），
        // 再触发一次 kick，避免「队列有 pending 但无人开轮」的唤醒丢失。
        if runtime.inbox().has_pending() {
            busy.store(false, Ordering::SeqCst);
            spawn_turn(runtime, done, busy, goal_driver, plan_mode);
            return;
        }
        busy.store(false, Ordering::SeqCst);
    });
}

/// `/goal` 命令分发：创建 / 状态迁移 / 查看。
async fn handle_goal_command(
    goal_service: &Arc<GoalService>,
    goal_driver: &Arc<GoalRoundDriver>,
    runtime: &Arc<Runtime>,
    line: &str,
    busy: &Arc<AtomicBool>,
) {
    let rest = line.strip_prefix("/goal ").map(str::trim).unwrap_or("");
    if rest.is_empty() {
        match goal_service.view() {
            Some(view) => {
                println!(
                    "当前目标：{}（{} · round {}/{}）",
                    view.snapshot.objective,
                    view.snapshot.phase.label(),
                    view.rounds_started,
                    view.snapshot.max_goal_rounds
                );
                if let Some(reason) = &view.snapshot.blocked_reason {
                    println!("  阻塞：{} — {}", reason.code, reason.message);
                }
            }
            None => println!("（当前无目标）"),
        }
        return;
    }
    match rest {
        "edit" => println!("（用法：/goal edit <objective>）"),
        _ if rest.starts_with("edit ") => {
            let objective = rest[5..].trim();
            if objective.is_empty() {
                println!("（用法：/goal edit <objective>）");
                return;
            }
            let Some(view) = goal_service.view() else {
                println!("（当前无目标，请用 /goal <objective> 创建）");
                return;
            };
            // 当前 complete 视为替换（重新 create）；否则 edit 修订 objective。
            if view.snapshot.phase.label() == "complete" {
                match goal_service.create(objective.to_string(), view.snapshot.max_goal_rounds) {
                    Ok(snapshot) => {
                        println!("（目标已替换：{} · round 0/{}）", snapshot.objective, snapshot.max_goal_rounds);
                        start_goal_round(goal_driver, runtime, busy);
                    }
                    Err(e) => println!("（替换失败：{e}）"),
                }
            } else {
                let reference = view.snapshot.ref_of();
                match goal_service.edit(reference, Some(objective.to_string()), None) {
                    Ok(snapshot) => println!("（目标已编辑：{}）", snapshot.objective),
                    Err(e) => println!("（编辑失败：{e}）"),
                }
            }
        }
        "pause" => {
            if let Some(reference) = goal_service.view().map(|v| v.snapshot.ref_of()) {
                match goal_service.pause(reference) {
                    Ok(_) => println!("（目标已暂停）"),
                    Err(e) => println!("（暂停失败：{e}）"),
                }
            } else {
                println!("（当前无目标）");
            }
        }
        "resume" => {
            if let Some(reference) = goal_service.view().map(|v| v.snapshot.ref_of()) {
                match goal_service.resume(reference) {
                    Ok(_) => {
                        println!("（目标已恢复）");
                        start_goal_round(goal_driver, runtime, busy);
                    }
                    Err(e) => println!("（恢复失败：{e}）"),
                }
            } else {
                println!("（当前无目标）");
            }
        }
        "complete" => {
            if let Some(reference) = goal_service.view().map(|v| v.snapshot.ref_of()) {
                match goal_service.complete(reference) {
                    Ok(_) => println!("（目标已完成）"),
                    Err(e) => println!("（完成失败：{e}）"),
                }
            } else {
                println!("（当前无目标）");
            }
        }
        "clear" => {
            if let Some(reference) = goal_service.view().map(|v| v.snapshot.ref_of()) {
                match goal_service.clear(reference) {
                    Ok(_) => println!("（目标已清除）"),
                    Err(e) => println!("（清除失败：{e}）"),
                }
            } else {
                println!("（当前无目标）");
            }
        }
        _ => {
            // 视为创建/替换目标：objective = rest，max_goal_rounds 默认 256。
            match goal_service.create(rest.to_string(), 256) {
                Ok(snapshot) => {
                    println!(
                        "（目标已创建：{} · round 0/{}）",
                        snapshot.objective, snapshot.max_goal_rounds
                    );
                    start_goal_round(goal_driver, runtime, busy);
                }
                Err(e) => println!("（创建失败：{e}）"),
            }
        }
    }
}

/// 打印单个 ask_user_question（逐一显示模式；`idx`/`total` 为第几问/共几问）。
fn print_ask_question(q: &AskUserQuestion, idx: usize, total: usize) {
    let prefix = if total > 1 {
        format!("[{idx}/{total}]")
    } else {
        String::new()
    };
    if let Some(header) = &q.header {
        println!("  {} [{}] {}", prefix, q.id, header);
    }
    println!("  {} [{}] {}", prefix, q.id, q.question);
    if let Some(options) = &q.options {
        for (i, opt) in options.iter().enumerate() {
            let num = i + 1;
            match &opt.description {
                Some(desc) => println!("      [{num}] {}：{}", opt.label, desc),
                None => println!("      [{num}] {}", opt.label),
            }
        }
        if options.len() > 1 {
            println!("      输入编号（如 {}）或完整选项文本即可", options.first().map(|_| "1").unwrap_or(""));
        }
    } else {
        println!("      直接输入回答文本（/answer {} <文本>）", q.id);
    }
}
