// socratic 共享库：类型、导师约束提示词与 agent 数据枚举（不 import React）。
// 全部数据来自 agent 通道真实接口（listSessions / listDirs / durable 历史），无 mock。
import type { AgentChannel, AppContext, FileEntry } from "@ava2/app-sdk";

export type Events = AppContext["events"];
export type StorageLike = AppContext["storage"];

export interface Workdir { workingDir: string; sessionId: string; }
export interface Turn { role: "mentor" | "student"; text: string; ts: number; }
export interface Session {
  sessionId: string;
  questionPath: string;
  questionName: string;
  questionText: string;
  turns: Turn[];
}

/** 导师约束提示词：只提问不给答案（真实注入到首轮消息）。 */
export const MENTOR_RULE =
  "你是一位苏格拉底式导师。规则：只提出下一个引导性问题，绝不直接给答案或完整解法；" +
  "每次只问一个问题，简短（不超过 60 字）；若学生连续两次卡住，给一句最小提示而不是答案。";

export async function listWorkdirs(agent: AgentChannel): Promise<Workdir[]> {
  try {
    const sessions = (await agent.listSessions()) || [];
    const seen = new Map<string, Workdir>();
    for (const s of [...sessions].sort((a, b) => Number(b.live) - Number(a.live))) {
      if (!s.workingDir || seen.has(s.workingDir)) continue;
      seen.set(s.workingDir, { workingDir: s.workingDir, sessionId: s.id });
    }
    return [...seen.values()];
  } catch {
    return [];
  }
}

export async function listQuestionFiles(agent: AgentChannel, w: Workdir): Promise<FileEntry[]> {
  const out: FileEntry[] = [];
  const keep = (list?: FileEntry[]) => {
    for (const f of list || []) {
      const n = f.name.toLowerCase();
      if ((n.endsWith(".md") || n.endsWith(".txt")) && !out.some((x) => x.path === f.path)) out.push(f);
    }
  };
  try {
    const root = await agent.listDirs(w.workingDir, { files: true });
    keep(root.files);
    for (const d of (root.entries || []).slice(0, 10)) {
      if (out.length >= 50) break;
      try {
        const sub = await agent.listDirs(d.path, { files: true });
        keep(sub.files);
      } catch {
        /* 跳过 */
      }
    }
  } catch {
    /* 忽略 */
  }
  return out;
}

/** 从 assistant 帧提取文本（durable 历史）。 */
export function textOf(message: unknown): string {
  const m = message as Record<string, unknown> | undefined;
  const blocks = Array.isArray(m?.blocks) ? (m!.blocks as unknown[]) : [];
  return blocks
    .map((b) => {
      const r = b as Record<string, unknown> | undefined;
      return r?.type === "text" ? String(r.text ?? "") : "";
    })
    .join("");
}
