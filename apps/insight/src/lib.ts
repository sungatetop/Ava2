// 数据叙事台纯逻辑层：CSV 解析 / 统计洞察 / 工作区枚举与跨视图共享类型（不 import React）。
import type { AgentChannel, AppContext } from "@ava2/app-sdk";

export interface ParsedCsv {
  headers: string[];
  rows: string[][];
}

export function parseCsv(text: string): ParsedCsv {
  const lines = text.split(/\r?\n/).filter((l) => l.trim().length > 0);
  if (lines.length === 0) return { headers: [], rows: [] };
  const split = (line: string): string[] => {
    const out: string[] = [];
    let cur = "";
    let inQ = false;
    for (let i = 0; i < line.length; i++) {
      const c = line[i];
      if (inQ) {
        if (c === '"') {
          if (line[i + 1] === '"') { cur += '"'; i++; } else inQ = false;
        } else cur += c;
      } else if (c === '"') inQ = true;
      else if (c === ",") { out.push(cur); cur = ""; }
      else cur += c;
    }
    out.push(cur);
    return out;
  };
  const headers = split(lines[0]);
  const rows = lines.slice(1).map(split);
  return { headers, rows };
}

export function isNumeric(v: string): boolean {
  return v !== "" && !Number.isNaN(Number(v));
}

export function fmtNum(n: number): string {
  if (!Number.isFinite(n)) return "—";
  return Math.abs(n) >= 1000 ? n.toLocaleString("zh-CN") : String(Math.round(n * 10) / 10);
}

export interface Dataset {
  name: string;
  csv: string;
  parsed: ParsedCsv;
}

export interface Workdir {
  workingDir: string;
  sessionId: string;
}

/** 枚举可用工作区（按会话 workingDir 去重）——本 App 的可读范围全集。 */
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

/** 选中数据集跨视图广播（主区选 CSV → 画布/面板即时同步）。 */
export const SELECT_EVENT = "insight.select";

export interface Selection {
  workingDir: string;
  sessionId: string;
  path: string;
  name: string;
}

export type Events = AppContext["events"];
export type StorageLike = AppContext["storage"];

/** 从解析结果计算洞察（含真实直方图与趋势序列；桶数随样本量自适应，样本少时减少空桶）。 */
export function computeInsights(parsed: ParsedCsv) {
  const { headers, rows } = parsed;
  if (headers.length === 0 || rows.length === 0) return null;
  const rowCount = rows.length;
  // 找数值列（第一个有数值的列做统计）
  let numCol = headers.findIndex((h) => rows.some((r) => isNumeric(r[headers.indexOf(h)] ?? "")));
  if (numCol < 0) numCol = headers.length - 1;
  const nums = rows.map((r) => Number(r[numCol])).filter((n) => Number.isFinite(n));
  const sum = nums.reduce((a, b) => a + b, 0);
  const avg = nums.length ? sum / nums.length : 0;
  const max = nums.length ? Math.max(...nums) : 0;
  const min = nums.length ? Math.min(...nums) : 0;
  const spikes = rows.filter((r) => Number(r[numCol]) > avg * 2).length;

  // 真实直方图：桶数自适应（3~9 桶），区间等比例映射，最后一桶含 max
  const bucketCount = Math.min(9, Math.max(3, Math.ceil(nums.length / 2)));
  const buckets = new Array(bucketCount).fill(0);
  if (nums.length > 0 && max > min) {
    const span = max - min;
    for (const n of nums) {
      const idx = Math.min(bucketCount - 1, Math.floor(((n - min) / span) * bucketCount));
      buckets[idx] += 1;
    }
  } else if (nums.length > 0) {
    buckets[0] = nums.length;
  }
  // 真实趋势：按行序取数值前 12 个点
  const series = nums.slice(0, 12);

  return { headers, rowCount, numCol, avg, max, min, spikes, numName: headers[numCol], buckets, series, valid: nums.length };
}
