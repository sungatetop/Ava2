//! 解析 Claude Code 的 event-to-matcher-group 钩子配置为共享 `MatcherGroup`。
//!
//! 仅 command hooks 运行；其他钩子类型
//! 作为 skipped 返回（桥告警）。plugin-root / project-directory 变量替换在解析期
//! 应用于命令字符串。

use crate::hooks::matcher::matcher_diagnostic;
use crate::hooks::types::{CommandHook, MatcherGroup};

/// 桥支持的 CC 事件（12/30）。
pub const CLAUDE_EVENTS: &[&str] = &[
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "Stop",
    "SubagentStart",
    "SubagentStop",
    "SessionEnd",
    "Notification",
    "ConfigChange",
    // 预留扩展点：onion 无 cwd 切换 / 文件 watcher，无 dispatch 点（解析注册但不触发）。
    "CwdChanged",
    "FileChanged",
];

/// 解析后的 CC 配置：事件名 → 其 matcher 组（仅 command hooks）。
pub type ClaudeCodeHookConfig = std::collections::HashMap<String, Vec<MatcherGroup>>;

/// 一个被跳过的非 command 钩子（桥据以告警）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkippedHook {
    pub event: String,
    pub hook_type: String,
}

/// 解析一个配置文件的输出：可运行组 + 被跳过项。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ParsedClaudeConfig {
    pub config: ClaudeCodeHookConfig,
    pub skipped: Vec<SkippedHook>,
}

/// 应用于每条 `command` 字符串的替换变量。
#[derive(Clone, Debug, Default)]
pub struct SubstitutionVars {
    /// 替换 `${CLAUDE_PLUGIN_ROOT}`。
    pub plugin_root: Option<String>,
    /// 替换 `${CLAUDE_PROJECT_DIR}`。
    pub project_dir: Option<String>,
}

/// 对命令字符串应用 `${CLAUDE_PLUGIN_ROOT}` / `${CLAUDE_PROJECT_DIR}` 替换。
/// 变量未设置时该 token 原样保留。
pub fn substitute_command(command: &str, vars: &SubstitutionVars) -> String {
    let mut out = command.to_string();
    if let Some(root) = &vars.plugin_root {
        out = out.split("${CLAUDE_PLUGIN_ROOT}").collect::<Vec<_>>().join(root);
    }
    if let Some(dir) = &vars.project_dir {
        out = out.split("${CLAUDE_PROJECT_DIR}").collect::<Vec<_>>().join(dir);
    }
    out
}

/// 解析 settings `hooks` 值或裸 `hooks.json` 事件 map。
/// 畸形条目被忽略而非使启动失败；不支持的事件在解析其组之前被忽略；
/// 非 command 钩子在 `skipped` 返回；替换应用于每个幸存的命令。
/// UserPromptSubmit 与 Stop 的 matcher 字段被丢弃（这些事件无 matcher 主体）。
/// 带 matcher 的可运行组若 regex 无效则抛 `SyntaxError`，使桥在 listener 注册前
/// 拒绝整个配置。
///
/// - `raw`：解析后的 JSON 配置（带 `hooks` 键的 settings 对象，或裸事件 map）。
/// - `vars`：应用于每条幸存命令的替换值（缺省无）。
pub fn parse_claude_code_config(
    raw: &serde_json::Value,
    vars: &SubstitutionVars,
) -> Result<ParsedClaudeConfig, String> {
    let mut config: ClaudeCodeHookConfig = std::collections::HashMap::new();
    let mut skipped: Vec<SkippedHook> = Vec::new();

    // 接受 `{ hooks: { … } }`（settings 文件）或裸事件 map。
    let root = raw.as_object();
    let hooks_map = root
        .and_then(|obj| obj.get("hooks").and_then(serde_json::Value::as_object))
        .or(root)
        .cloned();
    let Some(hooks_map) = hooks_map else {
        return Ok(ParsedClaudeConfig { config, skipped });
    };

    for event in CLAUDE_EVENTS {
        let Some(raw_groups) = hooks_map.get(*event).and_then(|v| v.as_array()) else {
            continue;
        };
        let mut groups: Vec<MatcherGroup> = Vec::new();
        for raw_group in raw_groups {
            let Some(group) = raw_group.as_object() else {
                continue;
            };
            let Some(raw_hooks) = group.get("hooks").and_then(|v| v.as_array()) else {
                continue;
            };
            let mut commands: Vec<CommandHook> = Vec::new();
            for raw_hook in raw_hooks {
                let Some(hook) = raw_hook.as_object() else {
                    continue;
                };
                let hook_type = hook
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("command");
                if hook_type != "command" {
                    skipped.push(SkippedHook {
                        event: event.to_string(),
                        hook_type: hook_type.to_string(),
                    });
                    continue;
                }
                let Some(command) = hook.get("command").and_then(|v| v.as_str()) else {
                    continue;
                };
                commands.push(CommandHook {
                    command: substitute_command(command, vars),
                    timeout_sec: hook.get("timeout").and_then(serde_json::Value::as_u64),
                });
            }
            if commands.is_empty() {
                continue;
            }
            let matcher = if *event == "UserPromptSubmit" || *event == "Stop" {
                None
            } else {
                group.get("matcher").and_then(|v| v.as_str()).map(ToString::to_string)
            };
            if let Some(diagnostic) = matcher_diagnostic(matcher.as_deref(), crate::hooks::types::MatcherMode::ClaudeCode) {
                return Err(format!("{diagnostic} on event {event:?}"));
            }
            groups.push(MatcherGroup { matcher, hooks: commands });
        }
        if !groups.is_empty() {
            config.insert(event.to_string(), groups);
        }
    }

    Ok(ParsedClaudeConfig { config, skipped })
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
