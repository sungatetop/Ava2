// RepoAtlas 纯逻辑层：类型、常量与扫描实现（不 import React、不碰 DOM）。
import type { AgentChannel } from "@ava2/app-sdk";

export const ACCENT = "#c2632a";

function escRe(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

export interface ScanFile {
  path: string;
  name: string;
  dir: string;
  hits: number;
  sample: string[];
}

export interface ScanResult {
  symbol: string;
  root: string;
  scanned: number;
  totalFiles: number;
  direct: ScanFile[];   // 命中且非测试
  tests: ScanFile[];    // 命中且路径含 test/spec
  indirect: ScanFile[]; // 未命中但同目录有命中（模块内波及）
  byDir: { name: string; hits: number }[];
}

export const SCAN_CAP = 80;

export interface Workdir {
  /** 会话权限根（该范围内文件才可读）。 */
  workingDir: string;
  /** 代表会话 id（readWorkspaceFile 的 session 参数）。 */
  sessionId: string;
}

/** 扫描广播载荷：Main 完成扫描后写 storage("scan") 并经 events 广播（Canvas 消费）。 */
export interface ScanPayload {
  root: string;
  symbol: string;
  result: ScanResult | null;
}

/** 枚举可用工作区（按会话 workingDir 去重）——这是本 App 的可读范围全集。 */
export async function listWorkdirs(agent: AgentChannel): Promise<Workdir[]> {
  try {
    const sessions = (await agent.listSessions()) || [];
    const seen = new Map<string, Workdir>();
    // live 优先占位（同 workingDir 时用 live 会话作代表）
    for (const s of [...sessions].sort((a, b) => Number(b.live) - Number(a.live))) {
      if (!s.workingDir || seen.has(s.workingDir)) continue;
      seen.set(s.workingDir, { workingDir: s.workingDir, sessionId: s.id });
    }
    return [...seen.values()];
  } catch {
    return [];
  }
}

/** 真实扫描：枚举根下子目录与文件，读内容统计符号引用（全带 sessionId 满足权限根）。 */
export async function scanRepo(agent: AgentChannel, root: string, symbol: string, sessionId?: string): Promise<ScanResult | null> {
  const sym = symbol.trim();
  if (!sym) return null;
  try {
    const listing = await agent.listDirs(root, { files: true });
    const subdirs = listing.entries || [];
    let files: { path: string; name: string; dir: string }[] = [];
    files.push(...(listing.files || []).map((f) => ({ path: f.path, name: f.name, dir: "" })));
    for (const d of subdirs) {
      try {
        const sub = await agent.listDirs(d.path, { files: true });
        files.push(...(sub.files || []).map((f) => ({ path: f.path, name: f.name, dir: d.name })));
      } catch {
        /* 单目录不可读跳过 */
      }
      if (files.length >= SCAN_CAP) break;
    }
    const totalFiles = files.length;
    const scannedFiles = files.slice(0, SCAN_CAP);
    const rx = new RegExp(escRe(sym), "g");
    const hitFiles: ScanFile[] = [];
    for (const f of scannedFiles) {
      try {
        const res = await agent.readWorkspaceFile(f.path, sessionId);
        const text = res.text ?? "";
        const lines = text.split(/\r?\n/);
        const matched = lines.filter((l) => l.includes(sym));
        if (matched.length > 0) {
          hitFiles.push({ path: f.path, name: f.name, dir: f.dir, hits: matched.length, sample: matched.slice(0, 3) });
        }
      } catch {
        /* 二进制/不可读/权限外跳过 */
      }
    }
    const direct = hitFiles.filter((f) => !/test|spec|_tests|\.test\.|\.spec\./i.test(f.path));
    const tests = hitFiles.filter((f) => /test|spec|_tests|\.test\.|\.spec\./i.test(f.path));
    // 间接：有命中文件的目录下、自身未命中的兄弟文件（限每目录 3 个）
    const hitDirs = new Set(hitFiles.map((f) => f.dir).filter(Boolean));
    const indirect: ScanFile[] = [];
    for (const d of subdirs) {
      if (!hitDirs.has(d.name)) continue;
      try {
        const sub = await agent.listDirs(d.path, { files: true });
        const siblings = (sub.files || []).filter((sf) => !hitFiles.some((h) => h.path === sf.path)).slice(0, 3);
        indirect.push(...siblings.map((sf) => ({ path: sf.path, name: sf.name, dir: d.name, hits: 0, sample: [] })));
      } catch {
        /* 跳过 */
      }
    }
    const byDirMap = new Map<string, number>();
    for (const f of hitFiles) {
      const key = f.dir || "(根)";
      byDirMap.set(key, (byDirMap.get(key) || 0) + f.hits);
    }
    const byDir = [...byDirMap.entries()].map(([name, hits]) => ({ name, hits })).sort((a, b) => b.hits - a.hits);
    return { symbol: sym, root, scanned: scannedFiles.length, totalFiles, direct, tests, indirect, byDir };
  } catch {
    return null;
  }
}
