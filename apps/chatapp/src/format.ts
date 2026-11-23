// 展示层格式化助手（消息流 / 轨迹视图共用）：文本单行摘要与 token 数值。
import type { TurnReport, TurnUsage } from "./chat-types";

export function truncate(text: string, max: number): string {
  return text.length > max ? text.slice(0, max) + "…" : text;
}

/** 单行摘要：折叠换行与多余空白（工具输入/思考首行） */
export function firstLine(text: string, max: number): string {
  const line =
    text
      .trim()
      .split("\n")
      .map((s) => s.trim())
      .find(Boolean) ?? "";
  return truncate(line.replace(/\s+/g, " "), max);
}

/** 紧凑 token 数（517 / 12.2K / 517K / 1.2M） */
export function formatTokens(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return "0";
  const scaled = (candidate: number): string =>
    candidate >= 100 ? String(Math.round(candidate)) : String(Math.round(candidate * 10) / 10);
  if (value < 1000) return String(value);
  if (value < 1_000_000) return `${scaled(value / 1000)}K`;
  return `${scaled(value / 1_000_000)}M`;
}

/** 精确整数（千分位分组） */
export function formatExactTokens(value: number): string {
  return value.toLocaleString("en-US");
}

/** 缓存命中百分比（部分命中不向上取整到 100） */
export function formatCacheHitPercent(
  cacheReadTokens: number,
  promptTokens: number,
): string | null {
  if (promptTokens <= 0 || cacheReadTokens <= 0) return null;
  if (cacheReadTokens >= promptTokens) return "100";
  const pct = (cacheReadTokens / promptTokens) * 100;
  for (let places = 0; places < 6; places++) {
    const text = pct.toFixed(places);
    if (Number(text) < 100) return text.replace(/\.?0+$/, "");
  }
  return "99.999999";
}

/** 一轮的总消耗（输入 + 输出；缓存读写已含在输入口径内，不重复计） */
export function usageTotal(usage: TurnUsage | undefined): number {
  if (!usage) return 0;
  return usage.inputTokens + usage.outputTokens;
}

/** 轮次用量：以 turn_report 汇总为准，回放态（无 report）回落到各步累加 */
export function turnUsageOf(item: {
  report?: TurnReport;
  usage?: TurnUsage;
}): TurnUsage | undefined {
  return item.report?.usage ?? item.usage;
}
