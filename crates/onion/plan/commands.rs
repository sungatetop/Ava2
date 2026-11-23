//! plan 命令层：`/plan [off|<msg>]` 生命周期。
//!
//! 职责：
//! - 解析 `/plan` 命令，区分 `off`（退出）/ 附带消息（进入并记录）/ 裸激活；
//! - 写入 `plan/wanted`（宿主期望）并驱动 `PlanMode` 切换（busy 挂 pending、
//!   空闲立即生效）；
//! - 维护 plan 回合的 `running` 轨（进入回合置 running，`run-done` 复位），
//!   使四轨投影可重放推导。

use std::sync::atomic::{AtomicBool, Ordering};

use super::service::{SetResult, PlanMode};
use super::store::{append_plan_run_done, append_plan_wanted};

/// `/plan` 命令解析结果。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanCommand {
    /// `/plan` 或 `/plan <msg>`：进入 plan 模式（附带消息仅记录）。
    Enter { message: Option<String> },
    /// `/plan off`：退出 plan 模式。
    Exit,
}

/// 解析 `/plan [off|<msg>]` 命令行。
/// 返回 None 表示非 `/plan` 命令。
pub fn parse_plan_command(line: &str) -> Option<PlanCommand> {
    let trimmed = line.trim();
    if trimmed == "/plan" {
        return Some(PlanCommand::Enter { message: None });
    }
    let rest = trimmed.strip_prefix("/plan ")?.trim();
    if rest == "off" {
        return Some(PlanCommand::Exit);
    }
    if rest.is_empty() {
        return Some(PlanCommand::Enter { message: None });
    }
    Some(PlanCommand::Enter { message: Some(rest.to_string()) })
}

/// 运行一条 plan 命令：写入 wanted + 驱动 PlanMode 切换。
/// busy=true（open-turn）→ 挂 pending 待 step 边界追加；空闲 → 立即生效。
/// 附带消息返回给宿主（`Some(msg)`），由宿主 followup 进 inbox，
/// 使任务进入会话历史；裸激活/退出返回 None（不投递消息）。
pub fn run_plan_command(
    plan_mode: &PlanMode,
    busy: &AtomicBool,
    command: PlanCommand,
) -> Option<String> {
    match command {
        PlanCommand::Enter { message } => {
            apply_switch(plan_mode, busy, true);
            message.filter(|m| !m.is_empty())
        }
        PlanCommand::Exit => {
            apply_switch(plan_mode, busy, false);
            None
        }
    }
}

/// 应用 plan 模式切换（busy 挂 pending / 空闲立即生效）+ 持久化 wanted。
fn apply_switch(plan_mode: &PlanMode, busy: &AtomicBool, active: bool) {
    persist_wanted(plan_mode, active);
    if busy.load(Ordering::SeqCst) {
        match plan_mode.set(active) {
            SetResult::Queued => {
                println!("（已在轮内切换 plan 模式，将在下一 step 边界生效）");
            }
            SetResult::Noop => println!("（plan 模式未变化）"),
            SetResult::Committed => unreachable!("busy 时 set 应返回 Queued"),
        }
    } else if active {
        plan_mode.enter();
        println!("（已进入 plan 模式：产出计划后用 exit_plan_mode 提交审批）");
    } else {
        plan_mode.exit();
        println!("（已退出 plan 模式）");
    }
}

/// 持久化宿主期望（`plan/wanted`），使 pending 可重放推导。
fn persist_wanted(plan_mode: &PlanMode, active: bool) {
    if let Some(session) = plan_mode.session_arc() {
        append_plan_wanted(session, active);
    }
}

/// plan 回合结束：追加 `plan/run-done`，投影 running 复位。
pub fn notify_run_done(plan_mode: &PlanMode) {
    if let Some(session) = plan_mode.session_arc() {
        append_plan_run_done(session);
    }
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
