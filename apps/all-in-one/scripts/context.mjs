// all-in-one 的 UserPromptSubmit 钩子：向本步模型输入注入协作提示
// （与 apps/hooks-guard/scripts/context.mjs 同一契约：stdin JSON → stdout 决策 JSON）。
let raw = "";
process.stdin.setEncoding("utf8");
for await (const chunk of process.stdin) raw += chunk;

let prompt = "";
try {
  prompt = String(JSON.parse(raw)?.prompt ?? "");
} catch {
  // 载荷解析失败时按空输入处理，不阻塞会话。
}

const notice =
  `[all-in-one] 本会话装配了 all-in-one App：提供 MCP 工具 mcp__toolbox__echo / mcp__toolbox__word_count。` +
  `当用户需要回显或文本统计时优先使用这些工具。本次输入 ${[...prompt].length} 个字符。`;

process.stdout.write(
  JSON.stringify({
    hookSpecificOutput: { hookEventName: "UserPromptSubmit", additionalContext: notice },
  }),
);
