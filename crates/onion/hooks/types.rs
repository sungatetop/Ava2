//! hooks 协议层共享类型（dialect 无关）。
//!
//! 命令钩子、matcher 组、解码后的输出、
//! 折叠后的合并结果。payload 构造 / env 规则 / matcher 模式 / 决策映射由各
//! dialect 桥（`hooks_claude.rs`）自持，本层只共享「中性词汇」。

/// 钩子方言。当前仅 `claude-code`；`codex` 预留扩展点。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HookDialect {
    ClaudeCode,
    Codex,
}

/// 一条配置的命令钩子（`{ type: 'command', command, timeout? }` 形态，两方言共享）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandHook {
    /// 待执行的 shell 命令行。
    pub command: String,
    /// 单钩子超时（秒，wire 单位；runner 转 ms）。
    pub timeout_sec: Option<u64>,
}

/// 一个 matcher 组：matcher 模式（缺失 / `''` / `*` = 匹配全部）+ 命中的命令钩子。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatcherGroup {
    pub matcher: Option<String>,
    pub hooks: Vec<CommandHook>,
}

/// matcher 模式如何解释。Claude Code 用「纯字面 pipe」精确匹配或 regex；
/// Codex 恒为 regex。桥按方言选模式。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatcherMode {
    ClaudeCode,
    Codex,
}

/// 解码后的单一决策（规范化）。
/// `approve`/`allow` 视为允许；`block`/`deny` 视为拒绝。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HookDecision {
    Approve,
    Allow,
    Block,
    Deny,
    Ask,
}

impl HookDecision {
    /// 是否表达「拒绝/阻塞」（block/deny）。
    pub fn is_block(self) -> bool {
        matches!(self, Self::Block | Self::Deny)
    }
}

/// 一条命令钩子运行后的解码输出（全字段可选——
/// 钩子可能只行使任一子集；桥决定哪些字段对它的钩子点有意义）。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HookOutput {
    /// 进程退出码；`None` = 未能运行（spawn 失败/信号死亡）。
    pub exit_code: Option<i32>,
    /// 裁剪后的 stderr（exit 2 的阻塞 reason 来源）。
    pub stderr: String,
    /// 裁剪后的 stdout（原样保留；干净退出时可能承载结构化 JSON）。
    pub stdout: String,
    /// `false` = 请求停止（对齐 CC/Codex `continue:false`）。
    pub continue_: Option<bool>,
    /// `continue:false` 时展示的人类可读原因。
    pub stop_reason: Option<String>,
    /// 中性阻塞决策（由两通道折叠：顶层 decision + permissionDecision）。
    pub decision: Option<HookDecision>,
    /// 伴随 decision 的 reason/explanation。
    pub reason: Option<String>,
    /// 事件判别符（`hookSpecificOutput.hookEventName`）；不匹配时丢弃事件域字段。
    pub hook_event_name: Option<String>,
    /// 注入下一次模型请求的额外上下文（CC `additionalContext`）。
    pub additional_context: Option<String>,
    /// 面向用户的警告（CC `systemMessage`）。解析但忽略（faithful-but-degraded）。
    pub system_message: Option<String>,
    /// 工具输入改写（CC `updatedInput`）。解析但忽略。
    pub updated_input: Option<serde_json::Value>,
}

/// 合并后的决策。缺省 `None`。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MergedDecision {
    Allow,
    Ask,
    Deny,
    #[default]
    None,
}

/// 一个钩子点所有命中钩子折叠后的结果。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MergedHookOutcome {
    /// 最严格权限决策（deny > ask > allow），无钩子表态时为 `None`。
    pub decision: MergedDecision,
    /// 所有拒绝/阻塞钩子的 reason，`\n\n` 连接；无则缺省。
    pub reason: Option<String>,
    /// 任一钩子请求停止（`continue:false`）。
    pub stop: bool,
    /// 首个停止钩子的 `stopReason`。
    pub stop_reason: Option<String>,
    /// 每个钩子的 `additionalContext`（按钩子顺序，不 join——桥决定）。
    pub additional_context: Vec<String>,
    /// 每个钩子的 `systemMessage`（按钩子顺序）。
    pub system_messages: Vec<String>,
}
