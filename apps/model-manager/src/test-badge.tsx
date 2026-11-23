// 连接测试结果徽标：测试中 / 连通延迟 / 失败原因（截断 + title 全文）。
import type { TestState } from "./types";

interface TestBadgeProps {
  state?: TestState;
}

export function TestBadge({ state }: TestBadgeProps) {
  if (!state) return null;
  if (state.loading) return <span style={{ fontSize: 11, color: "var(--ava-label-tertiary)" }}>测试中…</span>;
  const r = state.result;
  if (!r) return null;
  if (r.ok) {
    return <span style={{ fontSize: 11, color: "var(--ava-state-success)" }}>连通 {r.latencyMs}ms</span>;
  }
  return (
    <span
      title={r.error}
      style={{
        fontSize: 11,
        color: "var(--ava-state-danger)",
        maxWidth: 260,
        overflow: "hidden",
        textOverflow: "ellipsis",
        whiteSpace: "nowrap",
        display: "inline-block",
        verticalAlign: "bottom",
      }}
    >
      失败：{r.error || "未知错误"}
    </span>
  );
}
