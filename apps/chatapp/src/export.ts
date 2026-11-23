// 会话导出：拉取 durable 事件 → 复用消息流聚合规则还原条目 → 生成 Markdown 并下载。
// 只经 ctx.agent 的公开能力（getMessages），不新增后端端点。
import type { AgentChannel, SessionListItem } from "@ava-ai/app-sdk";
import type { ChatItem } from "./chat-types";
import { framesToItems } from "./frame-reducer";
import { firstLine } from "./format";
import { sessionLabel, sessionTime } from "./session-store";

/** 单条工具步骤 → markdown 列表行（失败标注，结果只取首行摘要） */
function stepLine(name: string, isError: boolean | undefined, summary: string): string {
  const tail = summary ? `：${summary}` : "";
  return `- ${isError ? "失败 " : ""}${name}${tail}`;
}

/** 条目流 → Markdown（用户 / 助手分段，助手过程折叠为引用块）。
 *  导出为公开函数便于用真实 transcript 做离线校验。 */
export function itemsToMarkdown(items: ChatItem[]): string {
  const blocks: string[] = [];
  for (const item of items) {
    if (item.kind === "user") {
      blocks.push(`## 我\n\n${item.text.trim()}`);
      continue;
    }
    // notice 为本地回执，不进导出；命令与压缩标记保留为引用块
    if (item.kind === "command") {
      const outcome = item.outcome?.text.trim() ?? "";
      blocks.push(`> \`/${item.name}${item.args}\`${outcome ? `\n>\n> ${outcome.replace(/\n/g, "\n> ")}` : ""}`);
      continue;
    }
    if (item.kind === "compaction") {
      const counts =
        item.shadowedItemCount !== null && item.shadowedTokenCount !== null
          ? `（已压缩 ${item.shadowedItemCount} 条历史记录，约 ${item.shadowedTokenCount} tokens）`
          : "";
      blocks.push(`> 上下文已压缩${counts}`);
      continue;
    }
    if (item.kind !== "turn") continue;
    if (item.steps.length > 0) {
      const lines = item.steps.map((s) => stepLine(s.name, s.isError, firstLine(s.resultText ?? s.inputText ?? "", 120)));
      blocks.push(`> 工具调用\n>\n${lines.map((l) => `> ${l}`).join("\n")}`);
    }
    if (item.answer.trim()) blocks.push(`## 助手\n\n${item.answer.trim()}`);
  }
  return blocks.join("\n\n");
}

/** 会话 → Markdown 文档（含元信息头） */
export async function sessionToMarkdown(agent: AgentChannel, session: SessionListItem): Promise<string> {
  const events = await agent.getMessages(session.id);
  const items = framesToItems(events as unknown[]);
  const body = itemsToMarkdown(items);
  const head = [
    `# ${sessionLabel(session)}`,
    "",
    `- 会话：${session.id}`,
    `- 工作区：${session.workingDir ?? "（未指定）"}`,
    `- 最近活跃：${sessionTime(session.updatedAtMs) || "未知"}`,
    `- 导出时间：${new Date().toLocaleString("zh-CN")}`,
    "",
    "---",
    "",
  ].join("\n");
  return `${head}${body || "（该会话暂无消息）"}\n`;
}

/** 触发浏览器下载（文件名剔除路径非法字符） */
export function downloadText(fileName: string, text: string): void {
  const safe = fileName.replace(/[\\/:*?"<>|]+/g, "_").slice(0, 60) || "session";
  const blob = new Blob([text], { type: "text/markdown;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = `${safe}.md`;
  document.body.appendChild(link);
  link.click();
  link.remove();
  URL.revokeObjectURL(url);
}
