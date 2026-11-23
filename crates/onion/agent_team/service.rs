//! TeamService 门面。
//!
//! 组合各 owner：journal（per-Lead 事务 + 投影）、task_board（任务 CAS）、
//! subagent（成员派生 + 消息投递）、activity（waitForChange）、lifecycle。
//! roster 逻辑（成员身份 / provisioning / 命名）内联于本模块（与 service 强耦合，
//! 避免过度拆分）。
//!
//! 身份注入而非自报：所有工具经 `membership(caller)` 解析调用者成员身份，
//! 模型不能伪造他人身份（对齐 ava-server agent_team 安全原则）。

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::agent_team::config::TeamConfig;
use crate::agent_team::journal::{AppendFn, Journal, Notify};
use crate::agent_team::lifecycle::Lifecycle;
use crate::agent_team::projection::TeamEvent;
use crate::agent_team::task_board;
use crate::agent_team::types::{
    CreateTeamTaskRequest, SendTeamMessageRequest, SpawnTeammateRequest, TeamContext, TeamMemberPhase,
    TeamMemberSnapshot, TeamMemberStatus, TeamMemberView, TeamRole, TeamTaskView, UpdateTeamTaskRequest,
};
use crate::agent_team::{validation, TeamError};
use crate::subagent::runtime::SubagentRuntime;
use crate::subagent::types::SubagentActivation;
use crate::types::AgentMessage;

/// 团队 Id（= Lead Session id）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamId(pub String);

/// 成员身份。
#[derive(Clone, Debug)]
pub struct TeamMembership {
    pub id: String,
    pub role: TeamRole,
    pub name: String,
}

/// spawn 结果（成员视图）。
#[derive(Clone, Debug)]
pub struct SpawnTeammateResult {
    pub member: TeamMemberView,
}

/// 消息发送结果（对齐 tool 层 `{sent:true}`）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SendTeamMessageResult {
    pub sent: bool,
}

/// 团队服务。
pub struct TeamService {
    config: TeamConfig,
    journal: Journal,
    lifecycle: Lifecycle,
    notify: Notify,
    /// Lead session id（wire 时注入）。
    lead_id: Mutex<Option<String>>,
    /// subagent 派生句柄（装配期注入）。
    subagents: Arc<SubagentRuntime>,
    /// 活跃成员激活表（id -> activation）。
    activations: Mutex<HashMap<String, SubagentActivation>>,
    /// 追加 durable 事件的底层函数（wire 时注入，写 Lead SessionLog）。
    append_impl: Mutex<Option<AppendFn>>,
}

impl TeamService {
    pub fn new(config: TeamConfig, subagents: Arc<SubagentRuntime>) -> Result<Self, TeamError> {
        config.validate()?;
        let notify = Notify::new();
        // append_impl 在 wire 时注入；先放一个 no-op（事务期间不应触发）。
        let noop: AppendFn = Arc::new(|_e: TeamEvent| Ok(()));
        let journal = Journal::new(noop, notify.clone());
        Ok(Self {
            config,
            journal,
            lifecycle: Lifecycle::new(config),
            notify,
            lead_id: Mutex::new(None),
            subagents,
            activations: Mutex::new(HashMap::new()),
            append_impl: Mutex::new(None),
        })
    }

    /// 接线：注入 Lead session id 与 durable append 函数（装配完成后调用）。
    pub async fn wire(&self, lead_id: String, append: AppendFn) {
        *self.lead_id.lock().await = Some(lead_id);
        *self.append_impl.lock().await = Some(append.clone());
        // 用 wire 后的 append 替换 journal 的 no-op。
        self.journal.set_append(append).await;
    }

    /// 同步变体：供同步装配路径（`assemble_runtime`）在无并发持有者时安全接线。
    /// 用 `try_lock`（与 `Runtime::set_model_target_sync` 同理）；装配期锁必空闲，
    /// 被占即时序破坏（fail-loud 返回错误）。
    pub fn wire_sync(&self, lead_id: String, append: AppendFn) -> std::io::Result<()> {
        *self
            .lead_id
            .try_lock()
            .map_err(|_| std::io::Error::other("lead_id 锁被并发持有：装配期不应存在其它持有者"))? = Some(lead_id);
        *self
            .append_impl
            .try_lock()
            .map_err(|_| std::io::Error::other("append_impl 锁被并发持有：装配期不应存在其它持有者"))? = Some(append.clone());
        self.journal.set_append_sync(append)
    }

    /// 团队是否已关闭。
    pub fn is_closed(&self) -> bool {
        self.lifecycle.is_disposed()
    }

    /// 关闭：dispose + 唤醒 waiter。
    pub fn close(&self) {
        self.lifecycle.close();
        self.notify.close();
    }

    /// 解析调用者成员身份（非成员抛 `TEAM_NOT_MEMBER`）。
    pub async fn membership(&self, caller: &str) -> Result<TeamMembership, TeamError> {
        let state = self.journal.snapshot().await;
        let lead_id = self.lead_id.lock().await.clone();
        let lead_id = lead_id.ok_or(TeamError::NotMember)?;
        if caller == lead_id {
            return Ok(TeamMembership { id: caller.to_string(), role: TeamRole::Lead, name: "lead".into() });
        }
        let member = state
            .members
            .iter()
            .find(|m| m.id == caller)
            .ok_or(TeamError::NotMember)?;
        Ok(TeamMembership {
            id: member.id.clone(),
            role: TeamRole::Teammate,
            name: member.name.clone(),
        })
    }

    /// 列出成员视图（Lead + teammate，创建序）。
    pub async fn list_members(&self) -> Result<Vec<TeamMemberView>, TeamError> {
        let state = self.journal.snapshot().await;
        let lead_id = self.lead_id.lock().await.clone();
        let mut out = Vec::new();
        if let Some(lead) = lead_id {
            let status = self.lead_status();
            out.push(TeamMemberView {
                id: lead,
                name: "lead".into(),
                role: TeamRole::Lead,
                status,
                description: None,
                provider: None,
                context: None,
                diagnostics: Vec::new(),
            });
        }
        let activations = self.activations.lock().await.clone();
        for m in &state.members {
            let status = match m.phase {
                TeamMemberPhase::Failed => TeamMemberStatus::Failed,
                TeamMemberPhase::Provisioning => TeamMemberStatus::Provisioning,
                TeamMemberPhase::Active => {
                    if activations.contains_key(&m.id) {
                        TeamMemberStatus::Running
                    } else {
                        TeamMemberStatus::Inactive
                    }
                }
            };
            out.push(TeamMemberView {
                id: m.id.clone(),
                name: m.name.clone(),
                role: TeamRole::Teammate,
                status,
                description: Some(m.description.clone()),
                provider: Some(m.provider.clone()),
                context: Some(m.context),
                diagnostics: m.error.clone().into_iter().collect(),
            });
        }
        Ok(out)
    }

    fn lead_status(&self) -> TeamMemberStatus {
        // Lead 恒为 active；运行时近似 running/inactive 由上层判定。
        TeamMemberStatus::Running
    }

    /// 派生一个具名队友（仅 Lead）。
    pub async fn spawn_teammate(
        &self,
        caller: &str,
        req: &SpawnTeammateRequest,
    ) -> Result<SpawnTeammateResult, TeamError> {
        let membership = self.membership(caller).await?;
        if membership.role != TeamRole::Lead {
            return Err(TeamError::LeadRequired);
        }
        validation::member_name(&req.name)?;

        // 事务：追加 provisioning 成员（校验 name 占用 + maxMembers）。
        let child_id = uuid::Uuid::new_v4().to_string();
        let snapshot = TeamMemberSnapshot {
            id: child_id.clone(),
            name: req.name.clone(),
            description: req.description.clone(),
            provider: req.provider.clone(),
            context: req.context,
            phase: TeamMemberPhase::Provisioning,
            error: None,
        };
        let name = req.name.clone();
        let max_members = self.config.max_members;
        let provision = snapshot.clone();
        self.journal
            .transact(move |state| {
                if state.members.iter().any(|m| m.name == name) {
                    return Err(TeamError::MemberNameTaken(name).to_string());
                }
                if state.members.len() as u64 >= max_members {
                    return Err(TeamError::MemberLimit.to_string());
                }
                Ok((vec![TeamEvent::Member(provision)], ()))
            })
            .await
            .map_err(|e| parse_team_error(&e))?;

        // 派生子会话。
        let activation = match req.context {
            TeamContext::Fresh => self.subagents.spawn(req.prompt.clone(), 1, None),
            TeamContext::Fork => {
                // fork 需 Lead SessionLog；此处简化：无 parent 引用时回退 spawn。
                // （完整 fork 需 service 持有 Lead SessionLog，见待确认事项）
                self.subagents.spawn(req.prompt.clone(), 1, None)
            }
        }
        .map_err(|e| TeamError::InvalidArgument(e.to_string()))?;

        // 激活成员。
        let active = TeamMemberSnapshot {
            phase: TeamMemberPhase::Active,
            ..snapshot.clone()
        };
        self.journal
            .transact::<(), _>(move |state| {
                // 仅当仍 provisioning 才置 active。
                let still_provisioning = state
                    .members
                    .iter()
                    .any(|m| m.id == active.id && m.phase == TeamMemberPhase::Provisioning);
                if !still_provisioning {
                    return Err(TeamError::ProvisioningConflict.to_string());
                }
                Ok((vec![TeamEvent::Member(active)], ()))
            })
            .await
            .map_err(|e| parse_team_error(&e))?;

        self.activations.lock().await.insert(child_id.clone(), activation);
        let view = self.member_view(&child_id).await?;
        Ok(SpawnTeammateResult { member: view })
    }

    /// 发送消息到另一成员。
    pub async fn send_message(
        &self,
        caller: &str,
        req: &SendTeamMessageRequest,
    ) -> Result<SendTeamMessageResult, TeamError> {
        let membership = self.membership(caller).await?;
        if self.lifecycle.is_disposed() {
            return Err(TeamError::Disposed);
        }
        let state = self.journal.snapshot().await;
        // 解析 target。
        let target_id = if req.target == "lead" {
            self.lead_id.lock().await.clone().ok_or(TeamError::MemberNotFound("lead".into()))?
        } else {
            state
                .members
                .iter()
                .find(|m| m.name == req.target)
                .ok_or_else(|| TeamError::MemberNotFound(req.target.clone()))?
                .id
                .clone()
        };
        // 禁自消息。
        if target_id == caller {
            return Err(TeamError::SelfMessage);
        }
        // 内容前缀 + 限长。
        let prefix = format!("Team message from {}:", membership.name);
        let mut blocks = vec![crate::types::ContentBlock::Text { text: prefix }];
        blocks.extend(req.content.iter().cloned());
        let encoded = serde_json::to_vec(&blocks).unwrap_or_default();
        if encoded.len() > self.config.max_message_bytes {
            return Err(TeamError::MessageTooLarge);
        }
        // 投递：target=Lead → 走 AgentMessage；teammate → 其 inbox steer。
        let message = AgentMessage::user(
            blocks
                .iter()
                .filter_map(|b| match b {
                    crate::types::ContentBlock::Text { text } => Some(text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n"),
        );
        if target_id == self.lead_id.lock().await.clone().unwrap_or_default() {
            // Lead 投递由上层（工具层）经 Lead inbox steer；此处标记 sent。
            // （完整实现见待确认事项：service 不直接持有 Lead inbox）
            self.notify.notify();
            return Ok(SendTeamMessageResult { sent: true });
        }
        let activations = self.activations.lock().await.clone();
        let inbox = activations
            .get(&target_id)
            .and_then(|a| a.inbox.clone())
            .ok_or_else(|| TeamError::MemberNotFound(req.target.clone()))?;
        inbox.steer(message).map_err(|e| TeamError::InvalidArgument(e.to_string()))?;
        self.notify.notify();
        Ok(SendTeamMessageResult { sent: true })
    }

    /// 中断队友（仅 Lead）。
    pub async fn interrupt(&self, caller: &str, target_name: &str) -> Result<String, TeamError> {
        let membership = self.membership(caller).await?;
        if membership.role != TeamRole::Lead {
            return Err(TeamError::LeadRequired);
        }
        let state = self.journal.snapshot().await;
        let target_id = if target_name == "lead" {
            return Err(TeamError::InvalidTarget("lead".into()));
        } else {
            state
                .members
                .iter()
                .find(|m| m.name == target_name)
                .ok_or_else(|| TeamError::MemberNotFound(target_name.to_string()))?
                .id
                .clone()
        };
        let activations = self.activations.lock().await;
        match activations.get(&target_id) {
            Some(_a) => {
                // 取消当前 turn（保留 inbox，不释放任务 owner）。
                if let Some(a) = activations.get(&target_id) {
                    a.cancel.cancel();
                }
                Ok("running".to_string())
            }
            None => Ok("inactive".to_string()),
        }
    }

    /// 等待团队变化。
    pub async fn wait_for_change(
        &self,
        timeout_ms: u64,
    ) -> Result<crate::agent_team::activity::WaitResult, TeamError> {
        let closed = self.lifecycle.is_disposed();
        crate::agent_team::activity::wait_for_change(&self.notify, timeout_ms, closed).await
    }

    /// 创建任务。
    pub async fn create_task(
        &self,
        caller: &str,
        req: &CreateTeamTaskRequest,
    ) -> Result<TeamTaskView, TeamError> {
        let _m = self.membership(caller).await?;
        let max_tasks = self.config.max_tasks;
        let req = req.clone();
        let view = self
            .journal
            .transact(move |mut state| {
                let task = task_board::create_task(&mut state, &req, max_tasks).map_err(|e| e.to_string())?;
                let view = crate::agent_team::task_view::project_task_view(&state, &task);
                Ok((vec![TeamEvent::Task(task)], view))
            })
            .await
            .map_err(|e| parse_team_error(&e))?;
        Ok(view)
    }

    /// 更新任务。
    pub async fn update_task(
        &self,
        caller: &str,
        req: &UpdateTeamTaskRequest,
    ) -> Result<TeamTaskView, TeamError> {
        let m = self.membership(caller).await?;
        let caller = caller.to_string();
        let role = m.role;
        let req = req.clone();
        let view = self
            .journal
            .transact(move |mut state| {
                let task = task_board::update_task(&mut state, &caller, role, &req)
                    .map_err(|e| e.to_string())?;
                let view = crate::agent_team::task_view::project_task_view(&state, &task);
                Ok((vec![TeamEvent::Task(task)], view))
            })
            .await
            .map_err(|e| parse_team_error(&e))?;
        Ok(view)
    }

    /// 读取单任务。
    pub async fn get_task(&self, caller: &str, id: &str) -> Result<TeamTaskView, TeamError> {
        let _m = self.membership(caller).await?;
        let state = self.journal.snapshot().await;
        let task = task_board::get_task(&state, id)?;
        Ok(crate::agent_team::task_view::project_task_view(&state, &task))
    }

    /// 列出任务。
    pub async fn list_tasks(&self, caller: &str) -> Result<Vec<TeamTaskView>, TeamError> {
        let _m = self.membership(caller).await?;
        let state = self.journal.snapshot().await;
        Ok(task_board::list_tasks(&state))
    }

    /// 成员视图（内部辅助）。
    async fn member_view(&self, id: &str) -> Result<TeamMemberView, TeamError> {
        let state = self.journal.snapshot().await;
        let m = state.members.iter().find(|m| m.id == id).ok_or(TeamError::NotMember)?;
        let activations = self.activations.lock().await.clone();
        let status = match m.phase {
            TeamMemberPhase::Failed => TeamMemberStatus::Failed,
            TeamMemberPhase::Provisioning => TeamMemberStatus::Provisioning,
            TeamMemberPhase::Active => {
                if activations.contains_key(&m.id) {
                    TeamMemberStatus::Running
                } else {
                    TeamMemberStatus::Inactive
                }
            }
        };
        Ok(TeamMemberView {
            id: m.id.clone(),
            name: m.name.clone(),
            role: TeamRole::Teammate,
            status,
            description: Some(m.description.clone()),
            provider: Some(m.provider.clone()),
            context: Some(m.context),
            diagnostics: m.error.clone().into_iter().collect(),
        })
    }
}

/// 解析字符串错误回 TeamError（错误码前缀匹配）。
fn parse_team_error(s: &str) -> TeamError {
    // 简化：直接映射为 InvalidArgument（具体错误码在 transact 内已编码进字符串，
    // 但为保真，这里做常见前缀匹配）。
    if s.contains("name taken") {
        return TeamError::MemberNameTaken(s.to_string());
    }
    if s.contains("member limit") {
        return TeamError::MemberLimit;
    }
    if s.contains("provisioning conflict") {
        return TeamError::ProvisioningConflict;
    }
    if s.contains("task stale revision") {
        return TeamError::TaskStaleRevision;
    }
    if s.contains("task not found") {
        return TeamError::TaskNotFound(s.to_string());
    }
    if s.contains("task limit") {
        return TeamError::TaskLimit;
    }
    if s.contains("dependency cycle") {
        return TeamError::TaskDependencyCycle;
    }
    if s.contains("already claimed") {
        return TeamError::TaskAlreadyClaimed;
    }
    if s.contains("blocked") {
        return TeamError::TaskBlocked;
    }
    if s.contains("invalid transition") {
        return TeamError::TaskInvalidTransition;
    }
    if s.contains("unauthorized") {
        return TeamError::TaskUnauthorized;
    }
    if s.contains("has dependents") {
        return TeamError::TaskHasDependents;
    }
    TeamError::InvalidArgument(s.to_string())
}
