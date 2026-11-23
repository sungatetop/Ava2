//! 工具审批判定内核：sandbox × approval 两维正交模型。
//!
//! 核心契约：`permission-presets`（sandbox 与 approval 策略捆绑为预设）与
//! approval seam（`allowed-once` 是唯一放行结果，rejected/cancelled/unavailable
//! 一律 fail-closed）。职责切分：
//!
//! - 工具只自描述**风险等级**（事实元数据，`ToolPort::risk()`）；
//! - 沙箱定义自动放行的操作允许集（`workspace-write` 下工作区写入免审批）；
//! - 审批策略只处理允许集之外的**升级操作**（ask → 裁决 / never → 拒绝）；
//! - 策略属部署侧安全配置，不进工具定义。
//!
//! 设计依据：docs/onion/onion-tool-approval-alignment-plan.md §3。

use std::sync::Arc;

use futures_util::StreamExt;
use serde_json::Value;

use crate::types::{AgentMessage, ContentBlock, ToolCall};

/// 工具风险等级（元数据自描述）：工具实现声明事实，部署侧沙箱/策略据此判定。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolRisk {
    /// 只读或无副作用（read / read_image / glob / grep / job_list / goal 工具）。
    ReadOnly,
    /// 沙箱允许集内的写入（write_file、subagent 派生/投递——产出经 relay 回父会话）。
    WorkspaceWrite,
    /// 沙箱允许集之外的升级操作（区外写/删除/执行，未来扩展预留判据）。
    Escalating,
}

/// 沙箱级别：定义自动放行的操作允许集，取值
/// `read-only` / `workspace-write` / `danger-full-access`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxLevel {
    /// 仅只读操作放行。
    ReadOnly,
    /// 工作区内写放行（默认：工作区写入免人工审批，越界操作升级）。
    WorkspaceWrite,
    /// 全部放行。
    DangerFullAccess,
}

impl SandboxLevel {
    /// 沙箱允许集判定：该级别下此风险等级是否自动放行。
    pub fn allows(self, risk: ToolRisk) -> bool {
        match self {
            SandboxLevel::ReadOnly => risk == ToolRisk::ReadOnly,
            SandboxLevel::WorkspaceWrite => matches!(risk, ToolRisk::ReadOnly | ToolRisk::WorkspaceWrite),
            SandboxLevel::DangerFullAccess => true,
        }
    }
}

/// 审批策略（会话级，取值 `ask` / `never`）：处理沙箱允许集之外的升级操作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalPolicy {
    /// 升级操作请求裁决（人工，或 Phase B 的 canUseTool reviewer seam）。
    Ask,
    /// 从不裁决：升级操作直接拒绝（fail-closed）。
    Never,
}

/// 审批回执：`allowed-once` / `allowed-for-session` 是放行结果；
/// `rejected` / 通道关闭 / 答复丢失一律按拒绝处理（fail-closed）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalDecision {
    /// 仅本次放行。
    AllowedOnce,
    /// 本会话内同键调用持续放行（键见 [`session_grant`]；不落盘，随会话销毁）。
    AllowedForSession,
    Rejected,
}

impl ApprovalDecision {
    /// 任意放行回执（一次性或本会话）。
    pub fn is_allowed(self) -> bool {
        matches!(self, ApprovalDecision::AllowedOnce | ApprovalDecision::AllowedForSession)
    }

    /// 收敛到审计用的结局闭集（本会话授权与一次性放行同为 `allowed-once`）。
    pub fn outcome(self) -> ApprovalOutcome {
        match self {
            ApprovalDecision::AllowedOnce | ApprovalDecision::AllowedForSession => {
                ApprovalOutcome::AllowedOnce
            }
            ApprovalDecision::Rejected => ApprovalOutcome::Rejected,
        }
    }
}

/// 审批结局闭集：`allowed-once` 是**唯一**放行结果；
/// `rejected` 明确拒绝；`cancelled` 请求被撤回（中断 / 答复通道被丢弃）；
/// `unavailable` 无可用审批通道（fail-closed）。审计事件 `decision` 只允许取这四个值。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalOutcome {
    /// 本次放行（一次性；会话级授权亦记为一次性放行）。
    AllowedOnce,
    /// 明确拒绝。
    Rejected,
    /// 请求被撤回：会话中断、答复方提前放弃。
    Cancelled,
    /// 无审批通道可用（通道关闭 / 无裁决者且无人可问），fail-closed。
    Unavailable,
}

impl ApprovalOutcome {
    /// 闭集全量（顺序固定）。
    pub const ALL: [ApprovalOutcome; 4] = [
        ApprovalOutcome::AllowedOnce,
        ApprovalOutcome::Rejected,
        ApprovalOutcome::Cancelled,
        ApprovalOutcome::Unavailable,
    ];

    /// 词汇表字面值（落盘与 API 均用小写连字符）。
    pub fn as_str(self) -> &'static str {
        match self {
            ApprovalOutcome::AllowedOnce => "allowed-once",
            ApprovalOutcome::Rejected => "rejected",
            ApprovalOutcome::Cancelled => "cancelled",
            ApprovalOutcome::Unavailable => "unavailable",
        }
    }

    /// 词汇表归一：闭集之外的输入一律收敛为 `unavailable`（fail-closed）。
    pub fn parse(text: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|outcome| outcome.as_str() == text)
            .unwrap_or(ApprovalOutcome::Unavailable)
    }

    /// 是否为放行结局（仅 `allowed-once`）。
    pub fn is_allowed(self) -> bool {
        self == ApprovalOutcome::AllowedOnce
    }
}

/// 会话级授权不可覆盖的破坏性命令首词：这类命令即使操作者点了「本会话允许」，
/// 也只按一次性放行处理，避免一条 `rm` 授权放行后续所有删除。
const SESSION_GRANT_DENIED_COMMANDS: &[&str] = &[
    "rm", "del", "erase", "rmdir", "rd", "format", "mkfs", "dd", "shutdown", "reboot", "sudo",
    "doas", "diskpart", "reg", "stop-process", "remove-item", "kill",
];

/// 会话级授权键：`工具:命令标识`；返回 `None` 表示该调用不可被会话级授权
/// （破坏性命令 / 空命令 → 永远逐次审批）。
///
/// 粒度：**简单命令**（无链式 / 管道 / 重定向 / 命令替换）按命令**首词**授权，
/// 使 `sleep 0.5` 的授权覆盖轮询循环里的 `sleep 1`；含组合符号的命令按**完整命令**
/// 授权，避免 `sleep 0.5 && rm -rf build` 借首词授权绕过审批。非 shell 工具（输入
/// 无 `command` 字段）按工具名授权。
pub fn session_grant(tool: &str, input: &Value) -> Option<String> {
    let Some(command) = input.get("command").and_then(Value::as_str) else {
        return Some(tool.to_string());
    };
    let normalized = command.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    let first = normalized.split(' ').next().filter(|first| !first.is_empty())?;
    let composite = ["&&", "||", ";", "|", ">", "<", "`", "$("].iter().any(|m| normalized.contains(m));
    if composite {
        // 组合命令：仅当完整命令串与会话授权一致时放行（不按首词扩散）。
        return Some(format!("{tool}:{normalized}"));
    }
    if SESSION_GRANT_DENIED_COMMANDS.contains(&first) {
        return None;
    }
    Some(format!("{tool}:{first}"))
}

/// 工具准入判定三态（`decide` 的输出；`Escalate` 交裁决者——人工或 reviewer）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// 沙箱允许集内，直通。
    Allow,
    /// 升级操作：交裁决者按审批策略处理。
    Escalate,
    /// 直接拒绝（policy = `Never` 且操作在允许集外；fail-closed 语义）。
    Deny,
}

/// 判定内核（纯函数）：沙箱允许集内直通；允许集外按审批策略升级或拒绝。
pub fn decide(risk: ToolRisk, sandbox: SandboxLevel, policy: ApprovalPolicy) -> Decision {
    if sandbox.allows(risk) {
        return Decision::Allow;
    }
    match policy {
        ApprovalPolicy::Ask => Decision::Escalate,
        ApprovalPolicy::Never => Decision::Deny,
    }
}

/// 工具可见性限制：按工具名强制放行/拒绝。
/// `deny` 优先于 `allow`（判定顺序 deny > allow > 沙箱允许集）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolRestriction {
    pub allow: Vec<String>,
    pub deny: Vec<String>,
}

impl ToolRestriction {
    pub fn is_denied(&self, name: &str) -> bool {
        self.deny.iter().any(|n| n == name)
    }

    pub fn is_allowed(&self, name: &str) -> bool {
        self.allow.iter().any(|n| n == name)
    }
}

/// 预设：沙箱 + 审批策略 + 工具限制捆绑为会话级可切换
/// 配置。内建预设表：workspace-write / danger-full-access / auto；`custom` 开放组合。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresetSpec {
    pub name: String,
    pub sandbox: SandboxLevel,
    pub approval: ApprovalPolicy,
    pub restriction: ToolRestriction,
}

impl PresetSpec {
    /// 默认预设 `workspace-write`：工作区写入免审批，升级操作交人工复决。
    pub fn workspace_write() -> Self {
        Self {
            name: "workspace-write".to_string(),
            sandbox: SandboxLevel::WorkspaceWrite,
            approval: ApprovalPolicy::Ask,
            restriction: ToolRestriction::default(),
        }
    }

    /// `auto` 预设：工作区写入免审批；升级操作交 canUseTool
    /// reviewer seam 裁决（规则/启发式），无人值守场景可用。
    pub fn auto() -> Self {
        Self {
            name: "auto".to_string(),
            sandbox: SandboxLevel::WorkspaceWrite,
            approval: ApprovalPolicy::Ask,
            restriction: ToolRestriction::default(),
        }
    }

    /// `danger-full-access` 预设：全部放行，审批不参与。
    pub fn danger_full_access() -> Self {
        Self {
            name: "danger-full-access".to_string(),
            sandbox: SandboxLevel::DangerFullAccess,
            approval: ApprovalPolicy::Never,
            restriction: ToolRestriction::default(),
        }
    }

    /// `custom` 预设：开放组合沙箱 / 审批策略 / 工具限制。
    pub fn custom(sandbox: SandboxLevel, approval: ApprovalPolicy, restriction: ToolRestriction) -> Self {
        Self { name: "custom".to_string(), sandbox, approval, restriction }
    }
}

/// 裁决者 seam（程序化裁决入口）：审批策略为 `Ask` 且会话配置了
/// reviewer 时，升级操作交其自动裁决而非挂起人工复决。`auto` 预设启用。
/// 返回 boxed future：`Arc<dyn ApprovalReviewer>` 动态分发要求方法对象安全，
/// 原生 RPITIT 不满足；参数取 owned `ToolCall` 避免 `'static` 生命周期传染。
pub trait ApprovalReviewer: Send + Sync {
    /// 对一次升级调用给出裁决；`allowed-once` 是唯一放行结果，其余一律拒绝。
    fn review(&self, call: ToolCall) -> crate::waterfall::BoxFuture<ApprovalDecision>;
}

/// 规则驱动的启发式裁决者（教学实现）：`allow` 名单命中放行，未命中拒绝
/// （fail-closed）。`auto` 预设的缺省 reviewer——体现「规则 + 启发式」而非
/// 真实 LLM 复审（真实 LLM 复审见 [`LlmReviewer`]）。
pub struct HeuristicReviewer {
    allow: Vec<String>,
}

impl HeuristicReviewer {
    pub fn new(allow: Vec<String>) -> Self {
        Self { allow }
    }
}

impl ApprovalReviewer for HeuristicReviewer {
    fn review(&self, call: ToolCall) -> crate::waterfall::BoxFuture<ApprovalDecision> {
        let allow = self.allow.clone();
        Box::pin(async move {
            if allow.iter().any(|n| n == &call.name) {
                ApprovalDecision::AllowedOnce
            } else {
                ApprovalDecision::Rejected
            }
        })
    }
}

/// LLM 复审裁决者（Phase D2：`auto` 预设升级）：把升级的工具调用（名称 + 输入）
/// 交真实模型复审，模型回复 `allow` 才放行。fail-closed 铁律贯穿——请求失败、
/// 超时（30s）、空回复、无法解析，一律 `Rejected`。
pub struct LlmReviewer {
    llm: Arc<dyn crate::llm::LlmPort>,
    model: String,
}

impl LlmReviewer {
    pub fn new(llm: Arc<dyn crate::llm::LlmPort>, model: impl Into<String>) -> Self {
        Self { llm, model: model.into() }
    }

    /// 从落定的 assistant 文本解析裁决；`allow` 前缀才放行。
    fn parse_verdict(text: &str) -> ApprovalDecision {
        let trimmed = text.trim().to_lowercase();
        if trimmed.starts_with("allow") {
            ApprovalDecision::AllowedOnce
        } else {
            ApprovalDecision::Rejected
        }
    }
}

impl ApprovalReviewer for LlmReviewer {
    fn review(&self, call: ToolCall) -> crate::waterfall::BoxFuture<ApprovalDecision> {
        let llm = self.llm.clone();
        let model = self.model.clone();
        Box::pin(async move {
            let prompt = format!(
                "工具准入复审。沙箱策略将以下工具调用升级待裁决：\n工具: {}\n输入: {}\n\n\
                 若该调用在开发助手场景下合理且无破坏性，仅回复 allow；否则仅回复 deny。",
                call.name, call.input
            );
            let draft = crate::context::RequestDraft {
                model,
                system: Some("你是工具准入裁决者。只回复 allow 或 deny，不要输出其他内容。".to_string()),
                messages: vec![AgentMessage::user(prompt)],
                tools: vec![],
                max_tokens: 512,
                temperature: Some(0.0),
                starts_request_series: false,
            };
            // 总超时兜底（建连 + 流落定）；取消语义由会话退出时 drop 本 future 承担。
            let review = async {
                let mut stream = llm.stream(&draft, tokio_util::sync::CancellationToken::new()).await;
                while let Some(item) = stream.next().await {
                    if let crate::llm::LlmStreamItem::Done(turn) = item {
                        // fail-closed：错误落定的 turn 不可信，直接拒绝。
                        if turn.error.is_some() {
                            return ApprovalDecision::Rejected;
                        }
                        let text = turn
                            .message
                            .blocks
                            .iter()
                            .find_map(|block| match block {
                                ContentBlock::Text { text } => Some(text.clone()),
                                _ => None,
                            })
                            .unwrap_or_default();
                        return Self::parse_verdict(&text);
                    }
                }
                ApprovalDecision::Rejected
            };
            tokio::time::timeout(std::time::Duration::from_secs(30), review)
                .await
                .unwrap_or(ApprovalDecision::Rejected)
        })
    }
}

/// 完整判定（含工具限制）：deny > allow > 沙箱允许集 > 审批策略。
/// 在 [`decide`] 之前先应用 `ToolRestriction` 的强制放行/拒绝。
pub fn decide_full(name: &str, risk: ToolRisk, preset: &PresetSpec) -> Decision {
    if preset.restriction.is_denied(name) {
        return Decision::Deny;
    }
    if preset.restriction.is_allowed(name) {
        return Decision::Allow;
    }
    decide(risk, preset.sandbox, preset.approval)
}

/// 会话级审批配置：预设 + 裁决者（可运行时经共享句柄切换）。
/// `reviewer` 为 `Some` 时（`auto` 预设），升级操作交其自动裁决而非挂起人工复决。
#[derive(Clone)]
pub struct ApprovalConfig {
    pub preset: PresetSpec,
    pub reviewer: Option<Arc<dyn ApprovalReviewer>>,
}

impl ApprovalConfig {
    /// 按预设名构造配置；`auto` 预设附带 `HeuristicReviewer`（allow 名单取自
    /// 预设的工具限制），其余预设无 reviewer（升级操作走人工复决或直接拒绝）。
    pub fn for_preset(name: &str) -> Self {
        let preset = match name {
            "danger-full-access" => PresetSpec::danger_full_access(),
            "auto" => PresetSpec::auto(),
            "custom" => PresetSpec::workspace_write(), // 默认组合；custom 组合参数由装配侧覆盖
            _ => PresetSpec::workspace_write(),
        };
        let reviewer = if preset.name == "auto" {
            Some(Arc::new(HeuristicReviewer::new(preset.restriction.allow.clone())) as Arc<dyn ApprovalReviewer>)
        } else {
            None
        };
        Self { preset, reviewer }
    }

    /// 可切换的预设名集合（CLI 提示用）。
    pub const PRESET_NAMES: [&'static str; 4] =
        ["workspace-write", "auto", "danger-full-access", "custom"];
}

/// 委派来源标记（审批审计的 `source` 字段取 `delegation`）。
pub const DELEGATION_SOURCE: &str = "delegation";

/// 从 durable 事件折叠出会话最后记录的审批预设（**最后一条**生效）。
///
/// 返回 `None` = 该会话没有记录过预设（走装配期默认），或记录值不可解析。
pub fn preset_from_events(events: &[crate::session::SessionEvent]) -> Option<String> {
    events.iter().rev().find_map(|event| match event {
        crate::session::SessionEvent::ApprovalPreset { preset, .. } if !preset.is_empty() => {
            Some(preset.clone())
        }
        _ => None,
    })
}

/// 会话当前是否处于**打开的轮次**内（审批请求只允许在轮次内发起）。
///
/// 理由：`approval/asked` + `approval/decided` 审计对必须被 durable 日志的
/// 提交/回放边界包住；轮次之间落下的裸事件在重载时是「崩溃尾巴垃圾」。判据是
/// 最后一条 `TurnStart` 之后没有 `TurnEnd`。
pub fn turn_open(events: &[crate::session::SessionEvent]) -> bool {
    let mut open = false;
    for event in events {
        match event {
            crate::session::SessionEvent::TurnStart { .. } => open = true,
            crate::session::SessionEvent::TurnEnd { .. } => open = false,
            _ => {}
        }
    }
    open
}

#[cfg(test)]
#[path = "approval_tests.rs"]
mod tests;
