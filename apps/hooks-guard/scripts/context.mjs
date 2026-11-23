// UserPromptSubmit 钩子：向本步模型输入注入一条守卫提示。
// 契约：stdin 收 JSON 载荷（{"prompt": "..."}）；stdout 输出决策 JSON。
// additionalContext 经 hookSpecificOutput 下发（onion codec 兼容 Claude Code 格式）。
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
  `[hooks-guard] 本会话由 hooks-guard App 保护：本次输入 ${[...prompt].length} 个字符。` +
  `此提示由 App 贡献的 UserPromptSubmit 钩子注入，无需模型执行任何操作。`;

process.stdout.write(
  JSON.stringify({
    hookSpecificOutput: { hookEventName: "UserPromptSubmit", additionalContext: notice },
  }),
);
