---
name: toolbox-echo
description: 演示调用 all-in-one App 贡献的 MCP server `toolbox` 的 echo 工具，验证「App → MCP → 会话」链路可用。
---

# toolbox-echo

本技能由 all-in-one App 经 `contributes.skills` 贡献。用于验证 App 贡献的 MCP server 在会话中可用。

## 步骤

1. 调用 MCP 工具 `mcp__toolbox__echo`，参数 `{"text": "hello-from-all-in-one"}`。
2. 确认工具结果将输入文本原样回显——说明边车已成功拉起 App 贡献的 stdio MCP server。
3. 需要时调用 `mcp__toolbox__word_count` 统计任意文本的字符 / 单词 / 行数。

## 说明

- 工具名遵循 `mcp__<serverName>__<toolName>` 约定；`serverName` 来自 App manifest 的 `contributes.mcpServers[].name`。
- server 进程由边车按需启动（`node ${CLAUDE_PLUGIN_ROOT}/server.mjs`，`${CLAUDE_PLUGIN_ROOT}` 在装配期替换为 App 安装目录），无需手动拉起。
- 本技能与工具均来自同一个 App：UI 页之外，App 还可以只贡献能力。
