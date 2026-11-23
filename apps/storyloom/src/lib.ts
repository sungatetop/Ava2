// storyloom 数据层：工作区/文稿枚举与真实解析（类型 + 纯函数，不依赖 React）。
// 全部数据来自 agent 通道的真实文件内容，无 mock。
import type { AgentChannel, FileEntry } from "@ava2/app-sdk";

export interface Workdir { workingDir: string; sessionId: string; }
export interface Beat {
  no: string;
  title: string;
  words: number;
  text: string;
  /** 该拍相对最长拍的长度占比（0-1），用于张力条 */
  ratio: number;
}
export interface Character { name: string; count: number; }

export interface Script {
  name: string;
  path: string;
  beats: Beat[];
  chars: Character[];
  totalWords: number;
}

/** 枚举可用工作区（会话 workingDir 去重）。 */
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

/** 枚举工作区内的文稿文件（md / txt / markdown）。 */
export async function listScripts(agent: AgentChannel, w: Workdir): Promise<FileEntry[]> {
  const out: FileEntry[] = [];
  const keep = (list?: FileEntry[]) => {
    for (const f of list || []) {
      const n = f.name.toLowerCase();
      if ((n.endsWith(".md") || n.endsWith(".txt") || n.endsWith(".markdown")) && !out.some((x) => x.path === f.path)) {
        out.push(f);
      }
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
        /* 跳过不可读目录 */
      }
    }
  } catch {
    /* 忽略读取失败 */
  }
  return out;
}

/** 真实统计中文 2-3 字人名候选：高频复现的连续汉字串（排除常见虚词）。 */
const STOP = new Set([
  "我们", "你们", "他们", "什么", "这个", "那个", "自己", "已经", "没有", "可以", "因为", "所以",
  "但是", "如果", "就是", "还是", "时候", "现在", "知道", "觉得", "一个", "这样", "那样", "然后",
  "于是", "而且", "虽然", "对于", "关于", "这样", "起来", "出来", "看到", "听见", "一样", "只是",
  "忽然", "突然", "终于", "只是", "不是", "还有", "所有", "每个", "一直", "刚才", "之前", "之后",
]);

export function extractCharacters(text: string, limit = 5): Character[] {
  const counts = new Map<string, number>();
  const re = /[\u4e00-\u9fa5]{2,3}/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text)) !== null) {
    const w = m[0];
    if (STOP.has(w)) continue;
    counts.set(w, (counts.get(w) || 0) + 1);
  }
  return [...counts.entries()]
    .filter(([, c]) => c >= 3)
    .sort((a, b) => b[1] - a[1])
    .slice(0, limit)
    .map(([name, count]) => ({ name, count }));
}

/** 真实切分文稿为节拍：优先 Markdown 标题，其次双空行分段。 */
export function parseScript(name: string, path: string, text: string): Script {
  const lines = text.split(/\r?\n/);
  const chunks: { title: string; text: string }[] = [];
  let cur: { title: string; lines: string[] } | null = null;
  const push = () => {
    if (cur && cur.lines.join("\n").trim()) {
      chunks.push({ title: cur.title, text: cur.lines.join("\n").trim() });
    }
  };
  for (const line of lines) {
    const heading = /^(#{1,4})\s+(.*)$/.exec(line);
    if (heading) {
      push();
      cur = { title: heading[2].trim(), lines: [] };
      continue;
    }
    if (!cur) cur = { title: name.replace(/\.[^.]+$/, ""), lines: [] };
    cur.lines.push(line);
  }
  push();
  // 无标题结构：按双空行分段
  if (chunks.length <= 1) {
    const blocks = text.split(/\n\s*\n+/).map((b) => b.trim()).filter(Boolean);
    chunks.length = 0;
    blocks.forEach((b, i) => {
      const first = b.split(/\r?\n/)[0] ?? "";
      chunks.push({ title: first.slice(0, 26) || `段落 ${i + 1}`, text: b });
    });
  }
  const counted = chunks.map((c) => ({
    ...c,
    words: c.text.replace(/\s/g, "").length,
  }));
  const maxWords = Math.max(1, ...counted.map((c) => c.words));
  const beats: Beat[] = counted.map((c, i) => ({
    no: `BEAT ${String(i + 1).padStart(2, "0")}`,
    title: c.title,
    words: c.words,
    text: c.text,
    ratio: c.words / maxWords,
  }));
  const totalWords = beats.reduce((a, b) => a + b.words, 0);
  return { name, path, beats, chars: extractCharacters(text), totalWords };
}
