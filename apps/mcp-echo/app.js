// 纯 MCP App 的占位入口：安装器要求每个 App 至少有一个可加载入口（app_entry_missing）。
// 本 App 无 UI 贡献，setup 不注入任何 slot；真正的产出是 contributes.mcpServers
// ——边车在会话装配期把它注册为名为 `echo` 的 MCP server（工具名 mcp__echo__*）。
export function setup() {
  return undefined;
}
