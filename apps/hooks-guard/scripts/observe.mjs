// PostToolUse 钩子：观测每次工具调用结果，把工具名写到 stderr
// （stderr 摘要会进入会话 transcript 的 hook_result 审计事件，可在轨迹视图看到）。
let raw = "";
process.stdin.setEncoding("utf8");
for await (const chunk of process.stdin) raw += chunk;

let tool = "unknown-tool";
try {
  const payload = JSON.parse(raw);
  tool = String(payload?.tool_name ?? payload?.tool ?? "unknown-tool");
} catch {
  // 载荷解析失败时按未知工具处理，不阻塞会话。
}

process.stderr.write(`[hooks-guard] PostToolUse 观测到工具调用：${tool}\n`);
// 输出空对象 = 中性结果（不 deny / 不注入）。
process.stdout.write("{}");
